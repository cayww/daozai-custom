//! 岛仔无线电量组件（Rust 版）—— stdin/stdout JSON 协议兼容 Python 组件。
mod hid;
mod protocol;
mod providers;

use protocol::{Poll, Ready, Reply, Reading, VERSION, UPSTREAM, MAX_LINE};
use std::io::{self, BufRead, Read as _, Write};
use std::time::Duration;

fn write_json<T: serde::Serialize>(stream: &mut impl Write, value: &T) -> io::Result<()> {
    let data = serde_json::to_vec(value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if data.len() >= MAX_LINE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "回复超出尺寸限制"));
    }
    stream.write_all(&data)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn licenses() -> io::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap_or_else(|| std::path::Path::new("."));
    let upstream = root.join("halo-battery/upstream");
    for name in ["LICENSE", "docs/credits.md"] {
        let path = upstream.join(name);
        if path.is_file() {
            let mut out = io::stdout().lock();
            writeln!(out, "{}", path.file_name().unwrap_or_default().to_string_lossy())?;
            let mut data = Vec::new();
            std::fs::File::open(&path)?.read_to_end(&mut data)?;
            out.write_all(&data)?;
            writeln!(out)?;
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("--licenses") {
        let _ = licenses();
        return;
    }
    if args.get(1).map(|s| s.as_str()) != Some("--stdio") {
        std::process::exit(2);
    }

    let mut engine = providers::Engine::new();
    // 统一注册入口：simple + compx 等全部协议族（见 providers::all_providers）。
    let providers = providers::all_providers();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    if write_json(&mut stdout, &Ready { version: VERSION, event: "ready", upstream: UPSTREAM }).is_err() {
        std::process::exit(1);
    }

    let mut input = stdin.lock();
    loop {
        let mut line = Vec::new();
        let n = input.read_until(b'\n', &mut line).unwrap_or(0);
        if n == 0 { break; }
        if line.len() > MAX_LINE {
            eprintln!("请求超出尺寸限制");
            break;
        }
        let poll: Poll = match serde_json::from_slice(&line) {
            Ok(p) => p,
            Err(e) => { eprintln!("无效请求: {e}"); continue; }
        };
        if let Err(e) = poll.validate() {
            eprintln!("无效请求: {e}");
            continue;
        }
        let devices: Vec<Reading> = engine.poll(&poll.receivers, &providers);
        let reply = Reply { version: VERSION, id: poll.id, devices };
        if write_json(&mut stdout, &reply).is_err() { break; }
    }
}

/// 预留：进程退出前的宽限期（与 Python 的 close timeout 语义一致）。
#[allow(dead_code)]
fn shutdown_grace() -> Duration { Duration::from_millis(500) }
