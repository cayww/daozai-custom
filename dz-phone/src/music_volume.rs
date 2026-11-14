//! Windows 音量混合器中的播放器音量；只操作匹配的应用会话。
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct Reader {
    source: String,
    at: Option<Instant>,
    value: Option<(u32, bool)>,
}
impl Reader {
    pub fn poll(&mut self, source:&str)->Option<(u32,bool)> {
        if self.source!=source || self.at.is_none_or(|at|at.elapsed()>=Duration::from_millis(500)) {
            self.source=source.into();self.at=Some(Instant::now());self.value=read(source);
        }
        self.value
    }
}

fn executable(source:&str)->Option<String> {
    let source=source.to_lowercase();
    match source.as_str() {
        "kugou"=>Some("kugou.exe".into()),
        "qqmusic"=>Some("qqmusic.exe".into()),
        "spotify"=>Some("spotify.exe".into()),
        "汽水音乐"=>Some("sodamusic.exe".into()),
        // QQ 音乐的系统会话用 AUMID `Tencent.QQMusic`，进程名是 qqmusic.exe。
        _ if source.contains("tencent") && source.contains("qqmusic")=>Some("qqmusic.exe".into()),
        _ if source.ends_with(".exe") && !source.contains(['\\','/'])=>Some(source),
        _=>None,
    }
}

#[cfg(windows)]
mod platform {
    use windows::{core::{Interface,PWSTR},Win32::{
        Foundation::CloseHandle,
        Media::Audio::{AudioSessionStateExpired,IAudioSessionControl2,IAudioSessionManager2,IMMDeviceEnumerator,ISimpleAudioVolume,MMDeviceEnumerator,DEVICE_STATE_ACTIVE,eRender},
        Storage::Packaging::Appx::GetApplicationUserModelId,
        System::{Com::{CoCreateInstance,CoInitializeEx,CLSCTX_ALL,COINIT_MULTITHREADED},Threading::{OpenProcess,PROCESS_QUERY_LIMITED_INFORMATION}},
    }};

    fn matches(pid:u32,source:&str)->bool {
        if source.eq_ignore_ascii_case("kwmusic.exe") && crate::smtc::player_process::image_is(pid,"KwService.exe"){return crate::smtc::kuwo::is_service(pid);}
        if let Some(exe)=super::executable(source){return crate::smtc::player_process::image_is(pid,&exe);}
        if pid==0 || !source.contains('!'){return false;}
        unsafe {
            let Ok(process)=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid) else{return false;};
            let mut id=[0u16;512];let mut length=id.len() as u32;
            let result=GetApplicationUserModelId(process,&mut length,PWSTR(id.as_mut_ptr()));
            let _=CloseHandle(process);
            result.is_ok() && length>0 && (length as usize)<=id.len()
                && String::from_utf16_lossy(&id[..length as usize-1]).eq_ignore_ascii_case(source)
        }
    }

    pub fn sessions(source:&str)->Vec<ISimpleAudioVolume> {
        if source.is_empty(){return Vec::new();}
        unsafe {
            // 采样、控制各用自己的 COM 线程与枚举器，不跨线程传递音频接口。
            thread_local!{static ENUMERATOR:Option<IMMDeviceEnumerator>=unsafe{
                let _=CoInitializeEx(None,COINIT_MULTITHREADED);
                CoCreateInstance(&MMDeviceEnumerator,None,CLSCTX_ALL).ok()
            };}
            ENUMERATOR.with(|enumerator|{
                let mut volumes=Vec::new();
                let Some(enumerator)=enumerator else{return volumes;};
                // 应用可能被路由到非默认输出，遍历当前可用的播放设备。
                let Ok(devices)=enumerator.EnumAudioEndpoints(eRender,DEVICE_STATE_ACTIVE) else{return volumes;};
                for d in 0..devices.GetCount().unwrap_or(0) {
                    let Ok(device)=devices.Item(d) else{continue;};
                    let Ok(manager)=device.Activate::<IAudioSessionManager2>(CLSCTX_ALL,None) else{continue;};
                    let Ok(sessions)=manager.GetSessionEnumerator() else{continue;};
                    for s in 0..sessions.GetCount().unwrap_or(0) {
                        let Ok(session)=sessions.GetSession(s) else{continue;};
                        if session.GetState().ok()==Some(AudioSessionStateExpired){continue;}
                        let Ok(control)=session.cast::<IAudioSessionControl2>() else{continue;};
                        if !control.GetProcessId().ok().is_some_and(|pid|matches(pid,source)){continue;}
                        if let Ok(volume)=session.cast::<ISimpleAudioVolume>(){volumes.push(volume);}
                    }
                }
                volumes
            })
        }
    }
}

/// QQ 音乐与网易云音乐的音量都是应用内部的，读不到；记住最近一次由岛仔设置的值用于显示，
/// 并落盘到 `music-volume.json`。`authoritative` 打开时，启动后把记住的值推回播放器，
/// 让两边以岛仔为准。
#[cfg(windows)]
#[derive(Clone)]
struct VolumeState{
    authoritative:bool,
    volumes:std::collections::HashMap<String,u32>,
}
#[cfg(windows)]
impl Default for VolumeState{
    fn default()->Self{Self{authoritative:true,volumes:std::collections::HashMap::new()}}
}

#[cfg(windows)]
static STATE:Mutex<Option<VolumeState>>=Mutex::new(None);

#[cfg(windows)]
fn volume_file()->std::path::PathBuf{crate::model::config_dir().join("music-volume.json")}

#[cfg(windows)]
fn state()->VolumeState{
    if let Ok(mut guard)=STATE.lock(){
        if guard.is_none(){
            let mut loaded=VolumeState::default();
            if let Ok(text)=std::fs::read_to_string(volume_file()){
                if let Ok(value)=serde_json::from_str::<serde_json::Value>(&text){
                    if let Some(object)=value.as_object(){
                        if let Some(volumes)=object.get("volumes").and_then(|item|item.as_object()){
                            loaded.authoritative=object.get("authoritative").and_then(|item|item.as_bool()).unwrap_or(true);
                            for (name,percent) in volumes{
                                if let Some(percent)=percent.as_u64(){loaded.volumes.insert(name.clone(),percent.min(100) as u32);}
                            }
                        }else{
                            // 旧格式是直接映射，忽略非数字字段。
                            for (name,percent) in object{
                                if let Some(percent)=percent.as_u64(){loaded.volumes.insert(name.clone(),percent.min(100) as u32);}
                            }
                        }
                    }
                }
            }
            *guard=Some(loaded);
        }
        guard.clone().unwrap_or_default()
    }else{
        VolumeState::default()
    }
}

#[cfg(windows)]
fn save_state(state:&VolumeState){
    let path=volume_file();
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    let value=serde_json::json!({"authoritative":state.authoritative,"volumes":state.volumes});
    if let Ok(text)=serde_json::to_string_pretty(&value){let _=std::fs::write(path,text);}
}

#[cfg(windows)]
pub fn authoritative()->bool{state().authoritative}

#[cfg(windows)]
pub fn set_authoritative(value:bool){
    if let Ok(mut guard)=STATE.lock(){
        let mut current=guard.clone().unwrap_or_default();
        current.authoritative=value;
        *guard=Some(current.clone());
        save_state(&current);
    }
}

#[cfg(windows)]
fn internal_key(source:&str)->Option<&'static str>{
    if crate::smtc::qq_music(source){Some("qqmusic")}
    else if source.eq_ignore_ascii_case("cloudmusic.exe"){Some("cloudmusic")}
    else{None}
}

#[cfg(windows)]
fn internal_volume(source:&str)->Option<u32>{
    let key=internal_key(source)?;
    state().volumes.get(key).copied()
}

#[cfg(windows)]
fn remember_internal(key:&str,percent:u32){
    let percent=percent.min(100);
    let mut saved=None;
    if let Ok(mut guard)=STATE.lock(){
        let mut current=guard.clone().unwrap_or_default();
        current.volumes.insert(key.to_string(),percent);
        *guard=Some(current.clone());
        saved=Some(current);
    }
    if let Some(state)=saved{save_state(&state);}
}

#[cfg(windows)]
fn apply_internal(source:&str,percent:u32)->bool{
    if crate::smtc::qq_music(source){
        crate::smtc::qq_volume_command(percent)
    }else if source.eq_ignore_ascii_case("cloudmusic.exe"){
        crate::smtc::netease_control::set_volume(percent)
    }else{
        false
    }
}

/// 播放器本次运行的标识。QQ 用**最小 PID**：主进程 PID 最小，我们推送时产生的
/// `QQMusic.exe /volumeto` 交接进程 PID 更大，不干扰判断；重启后 PID 必然变化。
/// 网易云没有稳定 PID 可看，用调试端口是否打开。
#[cfg(windows)]
fn player_token(key:&str)->Option<u32>{
    if key=="qqmusic"{
        crate::smtc::player_process::pids_named("qqmusic.exe").into_iter().min()
    }else{
        crate::smtc::netease_control::port_open().then_some(1)
    }
}

#[cfg(windows)]
fn player_running(key:&str)->bool{player_token(key).is_some()}

/// 一次运行会话的推送状态。
#[cfg(windows)]
#[derive(Clone,Copy,PartialEq,Eq,Debug)]
enum PushStep{
    /// 没在运行、也没有记录。
    Idle,
    /// 没在运行但有记录：清掉。
    Reset,
    /// 出现了一次新的运行会话（首次，或 PID 变了）：开始计时。
    Start,
    /// 在运行，但还没到稳定时间。
    Waiting,
    /// 在运行且已稳定、本次还没推过：可以推。
    Ready,
    /// 本次运行已经推过：不重复推。
    Done,
}

/// `record` 是历史记录：(本次是否已推送, 已等待时长, 记录时的 PID)。
/// 关键点：**PID 一变就当成新的一次启动**，不再依赖“是否碰巧抓到进程消失的那一帧”。
/// QQ 退出时会有进程消失得慢，快速重启时 `running` 始终为 true，旧实现会一直停在 Done。
#[cfg(windows)]
fn push_step(token:Option<u32>,record:Option<(bool,std::time::Duration,u32)>,stable:std::time::Duration)->PushStep{
    match (token,record){
        (None,None)=>PushStep::Idle,
        (None,Some(_))=>PushStep::Reset,
        (Some(_),None)=>PushStep::Start,
        (Some(pid),Some((_,_,old))) if pid!=old=>PushStep::Start,
        (Some(_),Some((true,_,_)))=>PushStep::Done,
        (Some(_),Some((false,elapsed,_)))=>if elapsed>=stable{PushStep::Ready}else{PushStep::Waiting},
    }
}

/// 独立的后台通道：每秒检测一次。播放器每次启动（含快速重启）只推一次。
#[cfg(windows)]
pub fn spawn_push_watcher(){
    use std::sync::Once;
    static ONCE:Once=Once::new();
    ONCE.call_once(||{
        std::thread::spawn(||{
            loop{
                std::thread::sleep(std::time::Duration::from_secs(1));
                watch_tick();
            }
        });
    });
}

#[cfg(windows)]
pub fn watch_tick(){
    use std::sync::{Mutex,MutexGuard};
    use std::time::{Duration, Instant};
    const STABLE_QQ:Duration=Duration::ZERO;
    const STABLE:Duration=Duration::from_secs(4);
    // 每个播放器：(本次运行是否已推送, 记录时间, 记录时的 PID)
    static TRACK:Mutex<Option<std::collections::HashMap<&'static str,(bool,Instant,u32)>>>=Mutex::new(None);
    static LAST:Mutex<Option<Instant>>=Mutex::new(None);
    if cfg!(test){return;}
    fn guard<T>(lock:&'static Mutex<T>)->MutexGuard<'static,T>{
        match lock.lock(){Ok(guard)=>guard,Err(poisoned)=>poisoned.into_inner()}
    }
    {
        let mut last=guard(&LAST);
        if last.is_some_and(|at|at.elapsed()<Duration::from_secs(1)){return;}
        *last=Some(Instant::now());
    }
    if !state().authoritative{
        let mut track=guard(&TRACK);
        if let Some(map)=track.as_mut(){map.clear();}
        return;
    }
    for (key,source) in [("qqmusic","Tencent.QQMusic"),("cloudmusic","cloudmusic.exe")]{
        let mut should_push=false;
        let mut detected:Option<Duration>=None;
        {
            let mut track=guard(&TRACK);
            if track.is_none(){*track=Some(std::collections::HashMap::new());}
            if let Some(map)=track.as_mut(){
                let token=player_token(key);
                let record=map.get(key).map(|(pushed,at,pid)|(*pushed,at.elapsed(),*pid));
                let stable=if key=="qqmusic"{STABLE_QQ}else{STABLE};
                match push_step(token,record,stable){
                    PushStep::Idle=>{}
                    PushStep::Reset=>{map.remove(key);}
                    PushStep::Start=>{
                        map.insert(key,(false,Instant::now(),token.unwrap_or(0)));
                        detected=Some(stable);
                    }
                    PushStep::Waiting=>{}
                    PushStep::Ready=>{
                        map.insert(key,(true,Instant::now(),token.unwrap_or(0)));
                        should_push=true;
                    }
                    PushStep::Done=>{}
                }
            }
        }
        if let Some(stable)=detected{
            let seconds=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value|value.as_secs()).unwrap_or(0);
            crate::resources::append_log(&crate::model::config_dir().join("diagnostics"),"music-volume.log",
                &format!("{seconds} detect key={key} pid={:?} 等待{stable:?}后推送",player_token(key)));
        }
        if !should_push{continue;}
        let Some(percent)=state().volumes.get(key).copied() else{continue;};
        let source=source.to_string();
        let key=key.to_string();
        // 发命令只要几十毫秒，放一次性线程，避免拖慢调用方。
        std::thread::spawn(move||{
            if !player_running(&key){return;}
            let applied=apply_internal(&source,percent);
            let seconds=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value|value.as_secs()).unwrap_or(0);
            crate::resources::append_log(&crate::model::config_dir().join("diagnostics"),"music-volume.log",
                &format!("{seconds} push key={key} percent={percent} applied={applied}"));
        });
    }
}

#[cfg(windows)]
pub fn read(source:&str)->Option<(u32,bool)> {
    // QQ 音乐和网易云调的都是应用内音量，会话音量不会跟着变，显示以记住的值为准。
    if internal_key(source).is_some() {
        if let Some(percent)=internal_volume(source){
            // 静音仍走系统会话，所以静音状态单独读。
            let muted=platform::sessions(source).iter().filter_map(|session|unsafe{session.GetMute().ok()}).any(|state|state.as_bool());
            return Some((percent,muted));
        }
    }
    platform::sessions(source).iter().filter_map(|session|unsafe {
        Some(((session.GetMasterVolume().ok()?*100.0).round() as u32,session.GetMute().ok()?.as_bool()))
    }).reduce(|a,b|(a.0.max(b.0),a.1 && b.1))
}

#[cfg(windows)]
pub fn set_volume(source:&str,percent:u32)->bool {
    if let Some(key)=internal_key(source) {
        if apply_internal(source,percent) {
            remember_internal(key,percent);
            return true;
        }
    }
    apply(source,|session|unsafe {
        session.SetMasterVolume(percent.min(100) as f32/100.0,std::ptr::null()).is_ok()
            && session.SetMute(false,std::ptr::null()).is_ok()
    })
}

#[cfg(windows)]
pub fn set_mute(source:&str,muted:bool)->bool {
    apply(source,|session|unsafe{session.SetMute(muted,std::ptr::null()).is_ok()})
}

#[cfg(windows)]
fn apply(source:&str,change:impl Fn(&windows::Win32::Media::Audio::ISimpleAudioVolume)->bool)->bool {
    let sessions=platform::sessions(source);
    let mut success=!sessions.is_empty();
    for session in &sessions{success=change(session) && success;}
    success
}

#[cfg(not(windows))]
pub fn authoritative()->bool{false}
#[cfg(not(windows))]
pub fn set_authoritative(_value:bool){}
#[cfg(not(windows))]
pub fn read(_source:&str)->Option<(u32,bool)>{None}
#[cfg(not(windows))]
pub fn set_volume(_source:&str,_percent:u32)->bool{false}
#[cfg(not(windows))]
pub fn set_mute(_source:&str,_muted:bool)->bool{false}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_names_require_an_exact_executable_or_packaged_identity(){
        for (source,exe) in [("QQMusic.exe","qqmusic.exe"),("kugou","kugou.exe"),("汽水音乐","sodamusic.exe")]{assert_eq!(executable(source).as_deref(),Some(exe));}
        for source in ["","music","QQMusic.exe.bak",r"C:\QQMusic.exe","SpotifyAB.SpotifyMusic_123!Spotify"]{assert_eq!(executable(source),None);}
    }
    #[test]
    fn unknown_player_cannot_change_system_volume(){
        assert_eq!(read("daozai-missing-player.exe"),None);
        assert!(!set_volume("daozai-missing-player.exe",100));
        assert!(!set_mute("",true));
    }

    #[test]
    #[cfg(windows)]
    fn volume_push_happens_once_per_player_run(){
        use std::time::Duration;
        let stable=Duration::from_secs(4);
        // 没在运行、没记录：什么都不做。
        assert_eq!(push_step(None,None,stable),PushStep::Idle);
        // 首次出现：开始计时。
        assert_eq!(push_step(Some(10),None,stable),PushStep::Start);
        // 未到稳定时间。
        assert_eq!(push_step(Some(10),Some((false,Duration::from_secs(1),10)),stable),PushStep::Waiting);
        // 到时间且未推过。
        assert_eq!(push_step(Some(10),Some((false,Duration::from_secs(5),10)),stable),PushStep::Ready);
        // 已推过：同一 PID 不再推。
        assert_eq!(push_step(Some(10),Some((true,Duration::from_secs(600),10)),stable),PushStep::Done);
        // 关键：PID 变了就是新一次启动。QQ 退出时进程消失得慢、快速重启时抓不到 false，
        // 旧实现会一直停在 Done（这就是“只有重启岛仔后第一次有效”的原因）。
        assert_eq!(push_step(Some(20),Some((true,Duration::from_secs(600),10)),stable),PushStep::Start);
        assert_eq!(push_step(Some(20),Some((false,Duration::from_secs(30),10)),stable),PushStep::Start);
        // 进程消失：清记录。
        assert_eq!(push_step(None,Some((true,Duration::from_secs(600),10)),stable),PushStep::Reset);
    }

    #[test]
    #[cfg(windows)]
    #[ignore="需要本机音频输出设备；创建无声会话验证 Windows 混音器读写"]
    fn live_windows_sessions_change_together_without_changing_master_volume(){
        use windows::Win32::{Media::Audio::{IAudioClient,IMMDeviceEnumerator,ISimpleAudioVolume,MMDeviceEnumerator,AUDCLNT_SHAREMODE_SHARED,eRender,eConsole},System::Com::{CoCreateInstance,CoCreateGuid,CoInitializeEx,CoTaskMemFree,CLSCTX_ALL,COINIT_MULTITHREADED}};
        unsafe {
            let _=CoInitializeEx(None,COINIT_MULTITHREADED);
            let enumerator:IMMDeviceEnumerator=CoCreateInstance(&MMDeviceEnumerator,None,CLSCTX_ALL).unwrap();
            let device=enumerator.GetDefaultAudioEndpoint(eRender,eConsole).unwrap();
            let create=||{
                let client:IAudioClient=device.Activate(CLSCTX_ALL,None).unwrap();
                let format=client.GetMixFormat().unwrap();let guid=CoCreateGuid().unwrap();
                let result=client.Initialize(AUDCLNT_SHAREMODE_SHARED,0,1_000_000,0,format,Some(&guid));
                CoTaskMemFree(Some(format as _));result.unwrap();
                let volume:ISimpleAudioVolume=client.GetService().unwrap();(client,volume)
            };
            let (_first,a)=create();let (_second,b)=create();
            let source=std::env::current_exe().unwrap().file_name().unwrap().to_str().unwrap().to_owned();
            a.SetMasterVolume(0.31,std::ptr::null()).unwrap();b.SetMasterVolume(0.52,std::ptr::null()).unwrap();
            let before=crate::system::Snapshot::read().volume;
            assert_eq!(read(&source),Some((52,false)));
            assert!(set_volume(&source,27));
            assert_eq!(read(&source),Some((27,false)));
            assert!((a.GetMasterVolume().unwrap()-0.27).abs()<0.001);
            assert!((b.GetMasterVolume().unwrap()-0.27).abs()<0.001);
            assert!(set_mute(&source,true));assert!(a.GetMute().unwrap().as_bool() && b.GetMute().unwrap().as_bool());
            assert!(set_volume(&source,19));assert_eq!(read(&source),Some((19,false)));
            assert!(!set_volume("daozai-missing-player.exe",100));assert_eq!(read(&source),Some((19,false)));
            assert!(set_volume(&source,999));assert_eq!(read(&source),Some((100,false)));
            assert_eq!(crate::system::Snapshot::read().volume,before,"应用音量不能改变系统主音量或静音");
            println!("双会话音量、静音、解除静音、边界和系统音量隔离验证通过");
        }
    }
}
