//! 把岛仔图标作为 RT_GROUP_ICON #1 编进 exe，供 LoadImageW(hInst, 1, ...) 取用。
//! 不引入额外依赖：直接调用 Windows SDK 的 rc.exe，把 .res 交给链接器。

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn find_rc() -> Option<PathBuf> {
    let roots = [
        r"C:\Program Files (x86)\Windows Kits\10\bin",
        r"C:\Program Files\Windows Kits\10\bin",
    ];
    let mut found: Vec<PathBuf> = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let rc = dir.join("x64").join("rc.exe");
            if rc.exists() {
                found.push(rc);
            }
        }
    }
    found.sort();
    found.pop()
}

/// 版本号（Cargo.toml 的 version）必须同步进 exe 的 VERSIONINFO 资源：
/// 应用内更新 update.ps1 会拿资源里的 FileVersion 与目标版本比对，不一致就报「安装包版本不匹配」。
/// 所以这里从 CARGO_PKG_VERSION 生成 app.rc 的两个占位符，避免再次出现「Cargo 版本升了、资源版本没升」。
fn version_tokens() -> (String, String) {
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let mut parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    while parts.len() < 4 { parts.push(0); }
    parts.truncate(4);
    let dotted = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    let commas = format!("{},{},{},{}", parts[0], parts[1], parts[2], parts[3]);
    (dotted, commas)
}

fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=assets/daozai.ico");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("daozai.res");
    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe 未找到，跳过图标资源");
        return;
    };
    // 把版本占位符填进 app.rc 的渲染副本，原模板保持不变。
    let (dotted, commas) = version_tokens();
    let rendered = fs::read_to_string("app.rc")
        .unwrap_or_default()
        .replace("@DAOZAI_VERSION@", &dotted)
        .replace("@DAOZAI_VERSION_COMMAS@", &commas);
    let rc_src = PathBuf::from(env::var("OUT_DIR").unwrap()).join("app.rendered.rc");
    if fs::write(&rc_src, rendered.as_bytes()).is_err() {
        println!("cargo:warning=写 app.rendered.rc 失败，跳过版本资源更新");
        return;
    }
    let status = Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&out)
        // rc.exe 用源文件所在目录解析相对路径（assets/daozai.ico），故先 cd 到 crate 根，
        // 再把渲染后的 rc 以绝对路径传入。
        .current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
        .arg(&rc_src)
        .status();
    if !status.map(|s| s.success()).unwrap_or(false) {
        println!("cargo:warning=rc.exe 编译 app.rc 失败");
        return;
    }
    println!("cargo:rustc-link-arg={}", out.display());
}
