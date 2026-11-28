//! 可选电量组件的 Windows 子进程和逐行 JSON 会话。
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver as MessageReceiver, RecvTimeoutError, SyncSender},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread,
    time::{Duration, Instant},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, WAIT_TIMEOUT},
        System::{
            JobObjects::{
                CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Pipes::CreatePipe,
            Threading::{
                CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
                UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW,
                EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST,
                STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW,
            },
        },
    },
};

const VERSION: u16 = 1;
const MAX_LINE: usize = 1024 * 1024;
const MAX_RECEIVERS: usize = 256;
const MAX_DEVICES: usize = 512;
const CHECK: Duration = Duration::from_millis(100);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_secs(60);
const RETRY_INTERVAL: Duration = Duration::from_secs(10);
// 关闭后立刻开启时，旧组件先完成退出，避免两个实例同时采集。
static COMPONENT: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Receiver {
    pub vid: u16,
    pub pid: u16,
    pub usb_instance: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reading {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub level: Option<u8>,
    pub charging: Option<bool>,
    #[serde(default)]
    pub approximate: bool,
    #[serde(default)]
    pub ui_message: String,
    pub online: Option<bool>,
    pub vid: u16,
    pub pid: u16,
    pub usb_instance: Option<String>,
    fresh: bool,
    age_ms: Option<u64>,
    #[serde(skip)]
    pub sampled_at: Option<Instant>,
}

pub(super) struct Snapshot {
    pub generation: u64,
    pub observed: Instant,
    pub devices: Vec<Reading>,
}

#[derive(Default)]
struct State {
    stopped: bool,
    targets: Option<Arc<Targets>>,
    latest: Option<Arc<Snapshot>>,
}

struct Targets {
    generation: u64,
    receivers: Vec<Receiver>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn wait(&self, state: MutexGuard<'_, State>, duration: Duration) {
        drop(
            self.changed
                .wait_timeout(state, duration)
                .unwrap_or_else(|error| error.into_inner()),
        );
    }

    fn cancelled(&self) -> bool {
        let state = self.state();
        state.stopped || state.targets.is_none()
    }

    fn publish(&self, targets: &Arc<Targets>, snapshot: Snapshot) {
        let mut state = self.state();
        if !state.stopped
            && state
                .targets
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, targets))
        {
            state.latest = Some(Arc::new(snapshot));
        }
    }
}

pub(super) struct Worker {
    shared: Arc<Shared>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    pub(super) fn spawn(exe: PathBuf) -> io::Result<Self> {
        if !exe.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "须使用已校验的电量组件绝对路径",
            ));
        }
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        let thread = thread::Builder::new()
            .name("daozai-halo".into())
            .spawn(move || supervise(&exe, &worker))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub(super) fn shutdown(mut self) -> Option<thread::JoinHandle<()>> {
        self.stop();
        self.thread.take()
    }

    fn stop(&self) {
        self.shared.state().stopped = true;
        self.shared.changed.notify_all();
    }

    pub(super) fn set_targets(&self, generation: u64, mut receivers: Vec<Receiver>) {
        receivers.retain(valid_receiver);
        receivers.sort_unstable_by(|a, b| a.usb_instance.cmp(&b.usb_instance));
        receivers.dedup_by(|a, b| a.usb_instance == b.usb_instance);
        receivers.truncate(MAX_RECEIVERS);
        let mut state = self.shared.state();
        if state.targets.as_ref().is_some_and(|targets| {
            targets.generation == generation && targets.receivers == receivers
        }) || state.targets.is_none() && receivers.is_empty()
        {
            return;
        }
        state.targets = (!receivers.is_empty()).then(|| {
            Arc::new(Targets {
                generation,
                receivers,
            })
        });
        state.latest = None;
        self.shared.changed.notify_all();
    }

    pub(super) fn snapshot(&self, now: Instant) -> Option<Arc<Snapshot>> {
        let state = self.shared.state();
        let latest = state.latest.as_ref()?;
        (now.saturating_duration_since(latest.observed) <= super::FRESH_FOR).then(|| latest.clone())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn valid_receiver(receiver: &Receiver) -> bool {
    let hardware = format!(r"USB\VID_{:04X}&PID_{:04X}\", receiver.vid, receiver.pid);
    let Some(instance) = receiver.usb_instance.strip_prefix(&hardware) else {
        return false;
    };
    !instance.is_empty()
        && !instance.contains('\\')
        && receiver.usb_instance.encode_utf16().count() < 200
        && !instance.chars().any(char::is_control)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    version: u16,
    event: String,
    upstream: String,
}

impl Ready {
    fn valid(&self) -> bool {
        self.version == VERSION
            && self.event == "ready"
            && !self.upstream.is_empty()
            && self.upstream.len() <= 64
            && self
                .upstream
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte))
    }
}

#[derive(Serialize)]
struct Poll<'a> {
    version: u16,
    id: u64,
    command: &'static str,
    receivers: &'a [Receiver],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    version: u16,
    id: u64,
    devices: Vec<Reading>,
}

fn invalid_response() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "电量组件响应无效")
}

fn read_json<T: DeserializeOwned>(reader: &mut impl BufRead) -> io::Result<T> {
    let mut line = Vec::new();
    // 即使子进程不发送换行，也不能把整段输出读进内存。
    let len = reader
        .take((MAX_LINE + 1) as u64)
        .read_until(b'\n', &mut line)?;
    if len == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    if len > MAX_LINE || line.last() != Some(&b'\n') {
        return Err(invalid_response());
    }
    serde_json::from_slice(&line).map_err(|_| invalid_response())
}

fn validate_reply(
    reply: Reply,
    id: u64,
    targets: &Targets,
    started: Instant,
    observed: Instant,
) -> io::Result<Snapshot> {
    if reply.version != VERSION || reply.id != id || reply.devices.len() > MAX_DEVICES {
        return Err(invalid_response());
    }
    let expected: BTreeMap<_, _> = targets
        .receivers
        .iter()
        .map(|r| (r.usb_instance.as_str(), (r.vid, r.pid)))
        .collect();
    let mut seen = BTreeSet::new();
    let mut devices = Vec::new();
    for mut reading in reply.devices {
        let Some(instance) = reading.usb_instance.as_deref() else {
            continue;
        };
        if expected.get(instance) != Some(&(reading.vid, reading.pid)) {
            continue;
        }
        if reading.key.is_empty()
            || reading.key.len() > 128
            || reading.key.chars().any(char::is_control)
            || reading.name.trim().is_empty()
            || reading.name.len() > 512
            || reading.name.chars().any(char::is_control)
            || reading.kind.len() > 64
            || reading.kind.chars().any(char::is_control)
            || reading.level.is_some_and(|level| level > 100)
            || !seen.insert((instance.to_owned(), reading.key.clone()))
        {
            return Err(invalid_response());
        }
        if reading.fresh {
            reading.sampled_at = match reading.age_ms {
                Some(age) => {
                    let age = Duration::from_millis(age);
                    (age <= super::FRESH_FOR)
                        .then(|| observed.checked_sub(age))
                        .flatten()
                }
                None => Some(started),
            };
        }
        devices.push(reading);
    }
    Ok(Snapshot {
        generation: targets.generation,
        observed,
        devices,
    })
}

fn supervise(exe: &Path, shared: &Shared) {
    loop {
        let state = shared.state();
        if state.stopped {
            return;
        }
        if state.targets.is_none() {
            shared.wait(state, POLL_INTERVAL);
            continue;
        }
        drop(state);
        let failed = if let Ok(active) = COMPONENT.try_lock() {
            let failed = session(exe, shared).is_err();
            drop(active);
            failed
        } else {
            shared.wait(shared.state(), CHECK);
            continue;
        };
        shared.state().latest = None;
        if failed && !shared.cancelled() {
            let deadline = Instant::now() + RETRY_INTERVAL;
            while !shared.cancelled() && Instant::now() < deadline {
                shared.wait(shared.state(), CHECK);
            }
        }
    }
}

enum Message {
    Ready(Ready),
    Reply(Reply),
}

type Request = (u64, Arc<Targets>);

fn session(exe: &Path, shared: &Shared) -> io::Result<()> {
    if shared.cancelled() {
        return Ok(());
    }
    let Child {
        job,
        process,
        stdin,
        stdout,
    } = Child::start(exe)?;
    let (tx, requests) = mpsc::sync_channel(1);
    let (responses, rx) = mpsc::sync_channel(1);
    let io = thread::Builder::new()
        .name("daozai-halo-stdio".into())
        .spawn(move || {
            let _ = serve_pipes(stdin, stdout, requests, responses);
        })?;
    let result = exchange(shared, &process, &tx, &rx);
    drop(tx);
    drop(rx);
    // 关 Job 先关闭子进程持有的管道端，再等阻塞的读写退出。
    drop(job);
    let _ = io.join();
    result
}

fn serve_pipes(
    mut stdin: File,
    stdout: File,
    requests: MessageReceiver<Request>,
    responses: SyncSender<Message>,
) -> io::Result<()> {
    let mut stdout = BufReader::new(stdout);
    responses
        .send(Message::Ready(read_json(&mut stdout)?))
        .map_err(|_| io::ErrorKind::BrokenPipe)?;
    while let Ok((id, targets)) = requests.recv() {
        let mut bytes = serde_json::to_vec(&Poll {
            version: VERSION,
            id,
            command: "poll",
            receivers: &targets.receivers,
        })?;
        bytes.push(b'\n');
        stdin.write_all(&bytes)?;
        responses
            .send(Message::Reply(read_json(&mut stdout)?))
            .map_err(|_| io::ErrorKind::BrokenPipe)?;
    }
    Ok(())
}

fn receive(
    shared: &Shared,
    process: &OwnedHandle,
    rx: &MessageReceiver<Message>,
    timeout: Duration,
) -> io::Result<Message> {
    let deadline = Instant::now() + timeout;
    loop {
        if shared.cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if unsafe { WaitForSingleObject(handle(process), 0) } != WAIT_TIMEOUT {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match rx.recv_timeout(remaining.min(CHECK)) {
            Ok(message) => return Ok(message),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
}

fn exchange(
    shared: &Shared,
    process: &OwnedHandle,
    tx: &SyncSender<Request>,
    rx: &MessageReceiver<Message>,
) -> io::Result<()> {
    let Message::Ready(ready) = receive(shared, process, rx, STARTUP_TIMEOUT)? else {
        return Err(invalid_response());
    };
    if !ready.valid() {
        return Err(invalid_response());
    }
    let mut id = 1;
    let mut previous = None::<Arc<Targets>>;
    let mut next_poll = Instant::now();
    loop {
        let state = shared.state();
        let Some(targets) = state.targets.clone().filter(|_| !state.stopped) else {
            return Ok(());
        };
        if unsafe { WaitForSingleObject(handle(process), 0) } != WAIT_TIMEOUT {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let started = Instant::now();
        if started < next_poll
            && previous
                .as_ref()
                .is_some_and(|last| Arc::ptr_eq(last, &targets))
        {
            shared.wait(state, CHECK.min(next_poll - started));
            continue;
        }
        drop(state);
        tx.try_send((id, targets.clone()))
            .map_err(|_| io::ErrorKind::BrokenPipe)?;
        let Message::Reply(reply) = receive(shared, process, rx, POLL_TIMEOUT)? else {
            return Err(invalid_response());
        };
        let observed = Instant::now();
        shared.publish(
            &targets,
            validate_reply(reply, id, &targets, started, observed)?,
        );
        previous = Some(targets);
        id += 1;
        next_poll = observed + POLL_INTERVAL;
    }
}

fn handle(value: &impl AsRawHandle) -> HANDLE {
    HANDLE(value.as_raw_handle())
}

fn pipe() -> windows::core::Result<(OwnedHandle, OwnedHandle)> {
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    unsafe {
        CreatePipe(&mut read, &mut write, None, 0)?;
        Ok((
            OwnedHandle::from_raw_handle(read.0),
            OwnedHandle::from_raw_handle(write.0),
        ))
    }
}

struct Attributes {
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
    _storage: Vec<usize>,
}

impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.list) };
    }
}

struct Child {
    job: OwnedHandle,
    process: OwnedHandle,
    stdin: File,
    stdout: File,
}

impl Child {
    fn start(exe: &Path) -> io::Result<Self> {
        let stderr = OpenOptions::new().write(true).open("NUL")?;
        Self::start_with_stderr(exe, &stderr).map_err(io::Error::other)
    }

    fn start_with_stderr(exe: &Path, stderr: &File) -> windows::core::Result<Self> {
        let job =
            unsafe { OwnedHandle::from_raw_handle(CreateJobObjectW(None, PCWSTR::null())?.0) };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                handle(&job),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )?;
        }
        let (child_stdin, stdin) = pipe()?;
        let (stdout, child_stdout) = pipe()?;
        let inherited = [handle(&child_stdin), handle(&child_stdout), handle(stderr)];
        for value in inherited {
            unsafe { SetHandleInformation(value, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)? };
        }
        let jobs = [handle(&job)];
        let mut bytes = 0;
        let _ = unsafe {
            InitializeProcThreadAttributeList(
                LPPROC_THREAD_ATTRIBUTE_LIST::default(),
                2,
                0,
                &mut bytes,
            )
        };
        let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast());
        unsafe { InitializeProcThreadAttributeList(list, 2, 0, &mut bytes)? };
        let attributes = Attributes {
            list,
            _storage: storage,
        };
        // 指定继承句柄且在执行第一条指令前加入 Job，子进程及其后代都受管理。
        unsafe {
            UpdateProcThreadAttribute(
                attributes.list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(inherited.as_ptr().cast()),
                size_of_val(&inherited),
                None,
                None,
            )?;
            UpdateProcThreadAttribute(
                attributes.list,
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                Some(jobs.as_ptr().cast()),
                size_of_val(&jobs),
                None,
                None,
            )?;
        }
        let startup = STARTUPINFOEXW {
            StartupInfo: STARTUPINFOW {
                cb: size_of::<STARTUPINFOEXW>() as u32,
                dwFlags: STARTF_USESTDHANDLES,
                hStdInput: inherited[0],
                hStdOutput: inherited[1],
                hStdError: inherited[2],
                ..Default::default()
            },
            lpAttributeList: attributes.list,
        };
        let application: Vec<_> = exe.as_os_str().encode_wide().chain([0]).collect();
        let directory: Vec<_> = exe
            .parent()
            .unwrap_or(exe)
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect();
        let mut command: Vec<_> = [b'"' as u16]
            .into_iter()
            .chain(exe.as_os_str().encode_wide())
            .chain("\" --stdio".encode_utf16())
            .chain([0])
            .collect();
        let mut process = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessW(
                PCWSTR(application.as_ptr()),
                PWSTR(command.as_mut_ptr()),
                None,
                None,
                true,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
                None,
                PCWSTR(directory.as_ptr()),
                &startup.StartupInfo,
                &mut process,
            )?;
            drop(OwnedHandle::from_raw_handle(process.hThread.0));
            Ok(Self {
                job,
                process: OwnedHandle::from_raw_handle(process.hProcess.0),
                stdin: stdin.into(),
                stdout: stdout.into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const FIRST: &str = r"USB\VID_3367&PID_1961\PORT_A";
    const SECOND: &str = r"USB\VID_3367&PID_1961\PORT_B";

    fn receiver(usb_instance: &str) -> Receiver {
        Receiver {
            vid: 0x3367,
            pid: 0x1961,
            usb_instance: usb_instance.into(),
        }
    }

    fn reading(instance: Option<&str>, fresh: bool, age_ms: Option<u64>) -> Value {
        json!({ "key":"mouse", "name":"无线鼠标", "kind":"mouse", "level":0,
            "charging":null, "online":true, "vid":0x3367, "pid":0x1961,
            "usb_instance":instance, "fresh":fresh, "age_ms":age_ms })
    }

    fn reply(devices: Vec<Value>) -> Reply {
        serde_json::from_value(json!({ "version":1, "id":4, "devices":devices })).unwrap()
    }

    #[test]
    fn halo_stdout_has_a_line_limit_and_requires_a_complete_schema() {
        let mut excess = io::Cursor::new(vec![b'x'; MAX_LINE * 2]);
        assert_eq!(
            read_json::<Reply>(&mut excess).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(excess.position(), (MAX_LINE + 1) as u64);
        for line in [
            "{\"version\":1,\"id\":4,\"devices\":[]}",
            "{\"version\":1,\"id\":4,\"excess\":true,\"devices\":[]}\n",
            "{\"version\":1,\"id\":4}\n",
            "not json\n",
        ] {
            assert!(read_json::<Reply>(&mut line.as_bytes()).is_err());
        }
        let mut lines = &b"{\"version\":1,\"event\":\"ready\",\"upstream\":\"1.18.0\"}\r\n{\"version\":1,\"id\":4,\"devices\":[]}\n"[..];
        assert_eq!(read_json::<Ready>(&mut lines).unwrap().event, "ready");
        assert_eq!(read_json::<Reply>(&mut lines).unwrap().id, 4);
    }

    #[test]
    fn halo_handshake_accepts_new_upstream_with_same_wire_version() {
        let mut ready = Ready {
            version: VERSION,
            event: "ready".into(),
            upstream: "1.19.0-pre.2".into(),
        };
        assert!(ready.valid());
        ready.version += 1;
        assert!(!ready.valid());
        ready.version = VERSION;
        ready.upstream = "v\n1".into();
        assert!(!ready.valid());
    }

    #[test]
    fn halo_matches_physical_instances_and_rejects_bad_ack_or_level() {
        let now = Instant::now();
        let targets = Targets {
            generation: 102,
            receivers: vec![receiver(FIRST), receiver(SECOND)],
        };
        let mut wrong_pid = reading(Some(FIRST), true, Some(10));
        wrong_pid["pid"] = json!(0x1960);
        let data = vec![
            reading(Some(FIRST), true, Some(10)),
            reading(Some(SECOND), true, None),
            reading(None, true, None),
            reading(Some(r"USB\VID_3367&PID_1961\OTHER"), true, None),
            wrong_pid,
        ];
        assert!(validate_reply(reply(data.clone()), 3, &targets, now, now).is_err());
        let result = validate_reply(reply(data), 4, &targets, now, now).unwrap();
        assert_eq!(result.generation, 102);
        assert_eq!(result.devices.len(), 2);
        assert_eq!(result.devices[0].usb_instance.as_deref(), Some(FIRST));
        assert_eq!(result.devices[1].usb_instance.as_deref(), Some(SECOND));
        assert_eq!(result.devices[0].level, Some(0));
        assert_eq!(
            result.devices[0].sampled_at,
            now.checked_sub(Duration::from_millis(10))
        );
        assert_eq!(result.devices[1].sampled_at, Some(now));
        let mut invalid = reading(Some(FIRST), true, None);
        invalid["level"] = json!(101);
        assert!(validate_reply(reply(vec![invalid]), 4, &targets, now, now).is_err());
    }

    #[test]
    fn halo_expired_and_cached_readings_have_no_sample_time() {
        let now = Instant::now();
        let targets = Targets {
            generation: 1,
            receivers: vec![receiver(FIRST), receiver(SECOND)],
        };
        let data = vec![
            reading(Some(FIRST), true, Some(u64::MAX)),
            reading(Some(SECOND), false, Some(10)),
        ];
        let result = validate_reply(reply(data), 4, &targets, now, now).unwrap();
        assert_eq!(result.devices.len(), 2);
        assert!(result
            .devices
            .iter()
            .all(|device| device.sampled_at.is_none()));
    }

    #[test]
    fn halo_replug_discards_old_reply_even_if_the_pnp_id_is_unchanged() {
        let worker = Worker {
            shared: Arc::new(Shared::default()),
            thread: None,
        };
        worker.set_targets(1, vec![receiver(FIRST)]);
        let old = worker.shared.state().targets.as_ref().unwrap().clone();
        let now = Instant::now();
        worker.shared.publish(
            &old,
            Snapshot {
                generation: 1,
                observed: now,
                devices: Vec::new(),
            },
        );
        assert!(worker.snapshot(now).is_some());
        assert!(worker
            .snapshot(now + super::super::FRESH_FOR + Duration::from_secs(1))
            .is_none());
        worker.set_targets(2, vec![receiver(FIRST)]);
        worker.shared.publish(
            &old,
            Snapshot {
                generation: 1,
                observed: now,
                devices: Vec::new(),
            },
        );
        assert!(worker.snapshot(now).is_none());
        let current = worker.shared.state().targets.as_ref().unwrap().clone();
        worker.shared.publish(
            &current,
            Snapshot {
                generation: 2,
                observed: now,
                devices: Vec::new(),
            },
        );
        assert_eq!(worker.snapshot(now).unwrap().generation, 2);
    }
}
