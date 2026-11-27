//! 无线电量增强的可选安装与版本校验。
use crate::updates::{self, Package};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use url::Url;

const REPOSITORY_PATH: &str = "/xiaotangtoken/xiaotang-daozai-plugins";
const MANIFEST_LIMIT: u64 = 1024;
const MAX_SIZE: u64 = 96 * 1024 * 1024;
const RECEIPT: &str = "component.json";
const CHOICE: &str = "enabled";
const TEST_FEED: &str = "wireless-battery-test.json";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) struct Component {
    directory: PathBuf,
    /// 随应用分发的组件目录：主程序 exe 同目录（与 daozai-bridge 一致）。
    bundled_dir: PathBuf,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    enabled: bool,
    work: Option<Arc<AtomicBool>>,
    active: Option<PathBuf>,
    error: Option<String>,
}

pub(crate) struct Status {
    pub enabled: bool,
    pub busy: bool,
    pub installed: bool,
    pub error: Option<String>,
}

impl Component {
    pub(crate) fn load(directory: PathBuf) -> Self {
        let directory = std::path::absolute(&directory).unwrap_or(directory);
        let bundled_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| directory.clone());
        let enabled = File::open(directory.join(CHOICE))
            .and_then(|file| {
                let mut choice = Vec::new();
                file.take(2).read_to_end(&mut choice)?;
                Ok(choice == b"1")
            })
            .unwrap_or(false);
        let component = Self {
            directory,
            bundled_dir,
            state: Arc::new(Mutex::new(State {
                enabled,
                ..State::default()
            })),
        };
        if enabled {
            component.begin(false, false, || {});
        }
        component
    }

    pub(crate) fn status(&self) -> Status {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        Status {
            enabled: state.enabled,
            busy: state.work.is_some(),
            installed: state.active.is_some(),
            error: state.error.clone(),
        }
    }

    pub(super) fn executable(&self) -> Option<PathBuf> {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.active.clone().filter(|_| state.enabled)
    }

    pub(crate) fn set_enabled(
        &self,
        enabled: bool,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<(), String> {
        {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if enabled == state.enabled {
                return Ok(());
            }
            fs::create_dir_all(&self.directory)
                .and_then(|_| {
                    crate::model::atomic_write(
                        &self.directory.join(CHOICE),
                        if enabled { b"1" } else { b"0" },
                    )
                })
                .map_err(|_| "无法保存电量增强开关")?;
            state.enabled = enabled;
            state.error = None;
            if !enabled {
                if let Some(work) = state.work.take() {
                    work.store(true, Ordering::Relaxed);
                }
                state.active = None;
            }
        }
        if enabled {
            self.begin(true, false, wake);
        }
        Ok(())
    }

    pub(crate) fn check_update(&self, wake: impl FnOnce() + Send + 'static) {
        self.begin(true, true, wake);
    }

    fn begin(&self, allow_download: bool, refresh: bool, wake: impl FnOnce() + Send + 'static) {
        let work = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state.work.is_some() || !state.enabled {
                return;
            }
            let work = Arc::new(AtomicBool::new(false));
            state.work = Some(work.clone());
            state.error = None;
            work
        };
        let component = self.clone();
        let result = std::thread::Builder::new()
            .name("daozai-battery-component".into())
            .spawn(move || {
                // 内置组件随应用分发，优先直接使用：无需网络、无需 receipt。
                // 仅当内置缺失（开发运行/旧版免安装包）时才走缓存与下载兜底。
                let prepared = bundled_executable(&component.bundled_dir)
                    .map(|path| Prepared {
                        path,
                        installation: None,
                    })
                    .ok_or_else(|| "未找到内置电量组件".to_owned())
                    .or_else(|error| {
                        if !refresh {
                            cached(&component.directory, &work).map(|path| Prepared {
                                path,
                                installation: None,
                            })
                        } else {
                            Err("需要检查组件更新".into())
                        }
                    })
                    .or_else(|error| {
                        if allow_download && !work.load(Ordering::Relaxed) {
                            download(&component.directory, &work)
                        } else {
                            Err(error)
                        }
                    });
                {
                    let mut state = component
                        .state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    if state
                        .work
                        .as_ref()
                        .is_some_and(|active| Arc::ptr_eq(active, &work))
                    {
                        state.work = None;
                        match prepared.and_then(|prepared| prepared.install(&component.directory)) {
                            Ok(path) => state.active = Some(path),
                            Err(error) => state.error = Some(error),
                        }
                    }
                }
                wake();
            });
        if result.is_err() {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.work = None;
            state.error = Some("无法启动组件安装任务".into());
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    size: u64,
    hash: String,
}

impl Receipt {
    fn package(self, url: String) -> Option<Package> {
        (self.size > 0
            && self.size <= MAX_SIZE
            && self.hash.len() == 64
            && self.hash.bytes().all(|ch| ch.is_ascii_hexdigit()))
        .then(|| Package {
            url,
            size: self.size,
            hash: self.hash.to_ascii_lowercase(),
        })
    }
}

fn manifest_url() -> Url {
    Url::parse(&format!(
        "https://gitee.com{REPOSITORY_PATH}/raw/master/wireless-battery/component.json"
    ))
    .expect("固定的电量组件清单地址")
}

fn read_manifest(reader: impl Read) -> Result<Package, &'static str> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        version: String,
        size: u64,
        sha256: String,
    }

    const INVALID: &str = "电量组件发布信息无效";
    let mut bytes = Vec::new();
    reader
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取电量组件发布信息")?;
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err(INVALID);
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| INVALID)?;
    let mut parts = manifest.version.split('.');
    for _ in 0..3 {
        let part = parts.next().ok_or(INVALID)?;
        if !part
            .parse::<u32>()
            .is_ok_and(|value| value.to_string() == part)
        {
            return Err(INVALID);
        }
    }
    if parts.next().is_some() {
        return Err(INVALID);
    }
    Receipt {
        size: manifest.size,
        hash: manifest.sha256,
    }
    .package(format!(
        "https://gitee.com{REPOSITORY_PATH}/releases/download/wireless-battery-v{}/daozai-halo.exe",
        manifest.version
    ))
    .ok_or(INVALID)
}

#[derive(Clone, Copy)]
enum Resource {
    Manifest,
    Asset,
}

fn https_on(url: &Url, host: &str) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some(host)
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

fn numbered_file(path: &str, prefix: &str, suffix: &str) -> bool {
    path.strip_prefix(prefix)
        .and_then(|tail| tail.strip_suffix(suffix))
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
}

fn gitee_attachment(url: &Url) -> bool {
    https_on(url, "gitee.com")
        && url
            .path()
            .strip_prefix(REPOSITORY_PATH)
            .is_some_and(|path| numbered_file(path, "/attach_files/", "/download/daozai-halo.exe"))
}

fn redirect_target(resource: Resource, first: &Url, from: &Url, location: &str) -> Option<Url> {
    let target = from.join(location).ok()?;
    let trusted = match resource {
        Resource::Manifest => {
            from == first
                && https_on(&target, "raw.giteeusercontent.com")
                && target.path() == first.path()
                && target
                    .query_pairs()
                    .any(|(key, value)| key == "signature" && !value.is_empty())
        }
        Resource::Asset if from == first => gitee_attachment(&target),
        Resource::Asset => {
            gitee_attachment(from)
                && https_on(&target, "foruda.gitee.com")
                && numbered_file(target.path(), "/attach_file/", "/daozai-halo.exe")
        }
    };
    trusted.then_some(target)
}

fn request_component(
    agent: &ureq::Agent,
    first: &Url,
    resource: Resource,
    cancel: &AtomicBool,
) -> Result<ureq::Response, String> {
    const REDIRECT_ERROR: &str = "电量组件下载地址发生非预期跳转";
    let manifest = matches!(resource, Resource::Manifest);
    let network_error = if manifest {
        "无法获取电量组件，请检查网络后重试"
    } else {
        "无法下载电量组件，请重试"
    };
    let mut url = first.clone();
    // 发布附件最多经过仓库附件页和文件服务器两跳。
    for _ in 0..3 {
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消下载".into());
        }
        let mut request = agent
            .get(url.as_str())
            .set("User-Agent", concat!("Daozai/", env!("CARGO_PKG_VERSION")))
            .set("Accept", "*/*");
        if manifest {
            request = request.set("Cache-Control", "no-cache");
        }
        let response = request.call().map_err(|error| match error {
            ureq::Error::Status(404, _) if manifest => "官方电量组件暂未发布",
            _ => network_error,
        })?;
        match response.status() {
            200 => return Ok(response),
            301 | 302 | 303 | 307 | 308 => {
                url = response
                    .header("Location")
                    .and_then(|location| redirect_target(resource, first, &url, location))
                    .ok_or(REDIRECT_ERROR)?;
            }
            _ => return Err(network_error.into()),
        }
    }
    Err(REDIRECT_ERROR.into())
}

fn binary_path(directory: &Path, package: &Package) -> PathBuf {
    directory.join(format!("daozai-halo-{}.exe", package.hash))
}

/// 随应用分发的组件固定叫 daozai-halo.exe，位于主程序 exe 同目录。
fn bundled_executable(directory: &Path) -> Option<PathBuf> {
    let path = directory.join("daozai-halo.exe");
    path.is_file().then_some(path)
}

fn cached(directory: &Path, cancel: &AtomicBool) -> Result<PathBuf, String> {
    let invalid = || "电量组件需要重新下载".to_owned();
    let receipt = File::open(directory.join(RECEIPT)).map_err(|_| invalid())?;
    let package = serde_json::from_reader::<_, Receipt>(receipt.take(1024))
        .ok()
        .and_then(|receipt| receipt.package(String::new()))
        .ok_or_else(invalid)?;
    let path = binary_path(directory, &package);
    if !fs::symlink_metadata(&path)
        .is_ok_and(|meta| meta.file_type().is_file() && meta.len() == package.size)
    {
        return Err(invalid());
    }
    let mut executable = File::open(&path).map_err(|_| invalid())?;
    updates::transfer(&mut executable, &mut io::sink(), &package, cancel, |_| {})
        .map_err(|_| invalid())?;
    Ok(path)
}

fn download(directory: &Path, cancel: &AtomicBool) -> Result<Prepared, String> {
    if option_env!("DAOZAI_TEST_BUILD").is_some() {
        if let Some((package, file)) = preview_asset()? {
            return prepare(directory, package, cancel, file);
        }
    }
    let agent = updates::http_agent_builder()?.redirects(0).build();
    let response = request_component(&agent, &manifest_url(), Resource::Manifest, cancel)?;
    let package = read_manifest(response.into_reader())?;
    if cancel.load(Ordering::Relaxed) {
        return Err("已取消下载".into());
    }
    if let Ok(path) = cached(directory, cancel) {
        if path == binary_path(directory, &package) {
            return Ok(Prepared {
                path,
                installation: None,
            });
        }
    }
    let asset_url = Url::parse(&package.url).map_err(|_| "电量组件下载地址无效")?;
    let response = request_component(&agent, &asset_url, Resource::Asset, cancel)?;
    prepare(directory, package, cancel, response.into_reader())
}

fn preview_asset() -> Result<Option<(Package, File)>, String> {
    // 本地验收源由测试版程序旁的文件指定，与插件导入的目录无关。
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Feed {
        artifact: PathBuf,
        package: Receipt,
    }
    let path = std::env::current_exe()
        .map_err(|_| "无法找到测试版程序")?
        .with_file_name(TEST_FEED);
    let feed = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("无法读取本地验收源".into()),
    };
    let feed: Feed = serde_json::from_reader(feed.take(4096)).map_err(|_| "本地验收源格式无效")?;
    if !feed.artifact.is_absolute()
        || feed.artifact.file_name().and_then(|name| name.to_str()) != Some("daozai-halo.exe")
    {
        return Err("本地验收组件路径无效".into());
    }
    let package = feed
        .package
        .package(String::new())
        .ok_or("本地验收组件校验信息无效")?;
    let file = File::open(feed.artifact).map_err(|_| "无法读取本地验收组件")?;
    Ok(Some((package, file)))
}

struct Temporary(PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct Prepared {
    path: PathBuf,
    installation: Option<(Temporary, Receipt)>,
}

impl Prepared {
    fn install(self, directory: &Path) -> Result<PathBuf, String> {
        if let Some((staged, receipt)) = self.installation {
            crate::model::replace_file(&staged.0, &self.path).map_err(|_| "无法安装电量组件")?;
            let metadata = serde_json::to_vec(&receipt).map_err(|_| "无法保存电量组件版本")?;
            crate::model::atomic_write(&directory.join(RECEIPT), &metadata)
                .map_err(|_| "无法保存电量组件版本")?;
        }
        Ok(self.path)
    }
}

fn prepare(
    directory: &Path,
    package: Package,
    cancel: &AtomicBool,
    mut input: impl Read,
) -> Result<Prepared, String> {
    let path = binary_path(directory, &package);
    // 组件可能正在运行。相同版本经过校验后直接复用安全缓存。
    let available = cached(directory, cancel).is_ok_and(|known| known == path);
    if available {
        return Ok(Prepared {
            path,
            installation: None,
        });
    }
    fs::create_dir_all(directory).map_err(|_| "无法创建电量组件目录")?;
    let temporary = Temporary(directory.join(format!(
        ".download-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
    )));
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary.0)
        .map_err(|_| "无法写入电量组件目录")?;
    updates::transfer(&mut input, &mut output, &package, cancel, |_| {})
        .map_err(|_| "电量组件下载不完整或校验失败")?;
    output.sync_all().map_err(|_| "无法保存电量组件")?;
    Ok(Prepared {
        path,
        installation: Some((
            temporary,
            Receipt {
                size: package.size,
                hash: package.hash,
            },
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn optional_battery_validates_bounded_gitee_manifest() {
        let value = json!({
            "version": "1.19.1", "size": 30, "sha256": "A".repeat(64)
        });
        let parse = |value: &serde_json::Value| read_manifest(value.to_string().as_bytes());
        let package = parse(&value).unwrap();
        assert_eq!(package.size, 30);
        assert_eq!(package.hash, "a".repeat(64));
        assert_eq!(
            package.url,
            "https://gitee.com/xiaotangtoken/xiaotang-daozai-plugins/releases/download/wireless-battery-v1.19.1/daozai-halo.exe"
        );
        for (field, invalid) in [
            ("version", json!("v1.19.1")),
            ("version", json!("1.19")),
            ("version", json!("1.19.1.0")),
            ("version", json!("1.19.01")),
            ("version", json!("1.19.1/other")),
            ("version", json!("1.19.4294967296")),
            ("sha256", json!("sha256:short")),
            ("sha256", json!("z".repeat(64))),
            ("size", json!(0)),
            ("size", json!(MAX_SIZE + 1)),
            ("url", json!("https://example.com/daozai-halo.exe")),
        ] {
            let mut changed = value.clone();
            changed[field] = invalid;
            assert!(parse(&changed).is_err(), "错误字段 {field}");
        }
        assert!(read_manifest(&b""[..]).is_err());
        assert!(read_manifest(&b"<html>Not Found</html>"[..]).is_err());
        let mut oversized = value.to_string().into_bytes();
        oversized.resize(MANIFEST_LIMIT as usize + 1, b' ');
        assert!(read_manifest(&oversized[..]).is_err());
    }

    #[test]
    fn optional_battery_only_accepts_expected_gitee_redirect_paths() {
        let feed = manifest_url();
        let signed = format!(
            "https://raw.giteeusercontent.com{}?signature=test",
            feed.path()
        );
        let raw = |from, target: &str| redirect_target(Resource::Manifest, &feed, from, target);
        let signed_url = raw(&feed, &signed).unwrap();
        assert_eq!(signed_url.as_str(), signed);
        assert!(raw(&signed_url, &signed).is_none());
        for unexpected in [
            signed.replace("https://", "http://"),
            signed.replace(".com/", ".com:444/"),
            signed.replace(".com/", ".com.example/"),
            signed.replace("://", "://user@"),
            signed.replace("/master/", "/other/"),
            signed.replace("signature=test", "signature="),
            format!("{signed}#fragment"),
        ] {
            assert!(raw(&feed, &unexpected).is_none(), "{unexpected}");
        }

        let release = Url::parse(&format!("https://gitee.com{REPOSITORY_PATH}/releases/download/wireless-battery-v1.19.1/daozai-halo.exe")).unwrap();
        let attach_path = format!("{REPOSITORY_PATH}/attach_files/123/download/daozai-halo.exe");
        let object = "https://foruda.gitee.com/attach_file/456/daozai-halo.exe?attname=daozai-halo.exe&token=test&ts=1";
        let next = |from, target: &str| redirect_target(Resource::Asset, &release, from, target);
        let attach = next(&release, &attach_path).unwrap();
        let final_url = next(&attach, object).unwrap();
        assert_eq!(final_url.as_str(), object);
        assert!(next(&release, object).is_none());
        assert!(next(&final_url, &attach_path).is_none());
        for unexpected in [
            attach_path.replace(REPOSITORY_PATH, "/another/repo"),
            attach_path.replace("/123/", "/abc/"),
            attach_path.replace("/123/", "/"),
            attach_path.replace("daozai-halo.exe", "other.exe"),
            format!("https://untrusted.example{attach_path}"),
        ] {
            assert!(next(&release, &unexpected).is_none(), "{unexpected}");
        }
        for unexpected in [
            object.replace("/456/", "/456/extra/"),
            object.replace("foruda.gitee.com", "gitee.com"),
            object.replace(".com/", ".com:444/"),
            object.replace("daozai-halo.exe?", "another.exe?"),
        ] {
            assert!(next(&attach, &unexpected).is_none(), "{unexpected}");
        }
    }

    #[test]
    fn optional_battery_requests_each_hop_without_account_credentials() {
        let release = Url::parse(&format!("https://gitee.com{REPOSITORY_PATH}/releases/download/wireless-battery-v1.19.1/daozai-halo.exe")).unwrap();
        let attachment = format!("{REPOSITORY_PATH}/attach_files/123/download/daozai-halo.exe");
        let object = "https://foruda.gitee.com/attach_file/456/daozai-halo.exe?token=fixture";
        let steps = [
            (release.to_string(), Some(attachment.clone())),
            (
                format!("https://gitee.com{attachment}"),
                Some(object.into()),
            ),
            (object.into(), None),
        ];
        let calls = Arc::new(AtomicU64::new(0));
        let seen = calls.clone();
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .middleware(move |request: ureq::Request, _: ureq::MiddlewareNext| {
                let (expected, location) = &steps[seen.fetch_add(1, Ordering::Relaxed) as usize];
                assert_eq!(request.url(), expected);
                assert!(request.header("Authorization").is_none());
                assert!(request.header("Cookie").is_none());
                match location {
                    Some(location) => {
                        format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\n\r\n").parse()
                    }
                    None => ureq::Response::new(200, "OK", "fixture"),
                }
            })
            .build();
        let cancel = AtomicBool::new(false);
        let response = request_component(&agent, &release, Resource::Asset, &cancel).unwrap();
        assert_eq!(response.into_string().unwrap(), "fixture");
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn optional_battery_reports_missing_or_invalid_gitee_responses() {
        let feed = manifest_url();
        for (status, headers, hint) in [
            (404, "", "暂未发布"),
            (500, "", "无法获取"),
            (204, "", "无法获取"),
            (302, "", "跳转"),
            (
                302,
                "Location: https://untrusted.example/private\r\n",
                "跳转",
            ),
        ] {
            let expected = feed.to_string();
            let agent = ureq::AgentBuilder::new()
                .redirects(0)
                .middleware(move |request: ureq::Request, _: ureq::MiddlewareNext| {
                    assert_eq!(request.url(), expected.as_str());
                    assert_eq!(request.header("Cache-Control"), Some("no-cache"));
                    format!("HTTP/1.1 {status} Fixture\r\n{headers}\r\n").parse()
                })
                .build();
            let error =
                request_component(&agent, &feed, Resource::Manifest, &AtomicBool::new(false))
                    .unwrap_err();
            assert!(error.contains(hint), "{error}");
            assert!(!error.contains("untrusted.example"));
        }
    }

    #[test]
    fn builtin_component_is_activated_without_download() {
        use std::{sync::mpsc, time::Duration};
        let root = std::env::temp_dir().join(format!(
            "daozai-battery-builtin-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
        ));
        let directory = root.join("data");
        let bundled = root.join("app");
        fs::create_dir_all(&bundled).unwrap();
        let exe = bundled.join("daozai-halo.exe");
        fs::write(&exe, b"builtin fixture").unwrap();
        let component = Component {
            directory: directory.clone(),
            bundled_dir: bundled,
            state: Arc::new(Mutex::new(State::default())),
        };
        let (finished, ready) = mpsc::channel();
        component
            .set_enabled(true, move || {
                let _ = finished.send(());
            })
            .unwrap();
        ready.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(component.executable(), Some(exe));
        assert!(
            !directory.join(RECEIPT).exists(),
            "内置组件不应产生下载 receipt"
        );
        component.set_enabled(false, || {}).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn optional_battery_default_creates_nothing_and_rejects_tampered_component() {
        let directory = std::env::temp_dir().join(format!(
            "daozai-optional-battery-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
        ));
        let component = Component::load(directory.clone());
        assert!(!component.status().enabled);
        assert!(component.executable().is_none());
        assert!(!directory.exists());
        let bytes = b"component fixture";
        let package = || Package {
            url: String::new(),
            size: bytes.len() as u64,
            hash: ring::digest::digest(&ring::digest::SHA256, bytes)
                .as_ref()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        };
        let cancel = AtomicBool::new(false);
        assert!(prepare(&directory, package(), &cancel, &b"corrupted fixture"[..]).is_err());
        assert!(!directory.join(RECEIPT).exists());
        let installed = prepare(&directory, package(), &cancel, &bytes[..])
            .unwrap()
            .install(&directory)
            .unwrap();
        assert_eq!(cached(&directory, &cancel).unwrap(), installed);
        fs::write(&installed, b"corrupted fixture").unwrap();
        assert!(cached(&directory, &cancel).is_err());
        assert!(component.executable().is_none());
        fs::remove_file(installed).unwrap();
        fs::remove_file(directory.join(RECEIPT)).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    #[ignore = "需要在测试二进制旁配置本地验收组件"]
    fn optional_battery_preview_installs_and_runs_only_after_opt_in() {
        assert!(option_env!("DAOZAI_TEST_BUILD").is_some());
        assert!(preview_asset().unwrap().is_some());
        verify_installation(std::time::Duration::from_secs(25));
    }

    #[test]
    #[ignore = "需要连接已公开发布电量组件的 Gitee 仓库"]
    fn optional_battery_gitee_installs_and_runs_only_after_opt_in() {
        assert!(
            option_env!("DAOZAI_TEST_BUILD").is_none(),
            "请使用正式构建参数"
        );
        verify_installation(std::time::Duration::from_secs(90));
    }

    fn verify_installation(download_timeout: std::time::Duration) {
        // 这是下载链路验收测试：测试二进制旁若已有内置组件，会优先走内置路径。
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
        {
            assert!(
                !dir.join("daozai-halo.exe").is_file(),
                "下载验收测试不能在内置组件存在时运行"
            );
        }
        use super::super::halo_worker::{Receiver, Worker};
        use std::{
            sync::mpsc,
            time::{Duration, Instant},
        };

        let directory = std::env::temp_dir().join(format!(
            "daozai-battery-install-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
        ));
        let component = Component::load(directory.clone());
        assert!(!directory.exists());
        let (finished, ready) = mpsc::channel();
        component
            .set_enabled(true, move || {
                let _ = finished.send(());
            })
            .unwrap();
        ready.recv_timeout(download_timeout).unwrap();
        let path = component
            .executable()
            .unwrap_or_else(|| panic!("{:?}", component.status().error));
        assert_eq!(cached(&directory, &AtomicBool::new(false)).unwrap(), path);
        let worker = Worker::spawn(path.clone()).unwrap();
        worker.set_targets(
            1,
            vec![Receiver {
                vid: 1,
                pid: 2,
                usb_instance: r"USB\VID_0001&PID_0002\DAOZAI-COMPONENT-PREVIEW".into(),
            }],
        );
        let deadline = Instant::now() + Duration::from_secs(25);
        let snapshot = loop {
            if let Some(snapshot) = worker.snapshot(Instant::now()) {
                break snapshot;
            }
            assert!(Instant::now() < deadline, "组件没有返回验收快照");
            std::thread::sleep(Duration::from_millis(50));
        };
        assert_eq!(snapshot.generation, 1);
        assert!(snapshot.devices.is_empty());
        component.set_enabled(false, || {}).unwrap();
        worker.shutdown().unwrap().join().unwrap();
        assert!(component.executable().is_none());
        assert!(!Component::load(directory.clone()).status().enabled);
        // 测试退出后 Windows 任务对象负责释放子进程的文件句柄。
        for _ in 0..20 {
            if fs::remove_file(&path).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!path.exists());
        fs::remove_file(directory.join(CHOICE)).unwrap();
        fs::remove_file(directory.join(RECEIPT)).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
