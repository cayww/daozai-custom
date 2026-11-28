#[path = "../hid.rs"]
mod hid;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(instance) = args.get(1) else {
        eprintln!("usage: hid-probe <USB instance> [--battery]");
        std::process::exit(2);
    };
    if args.get(2).map(String::as_str) == Some("--wait") {
        for collection in hid::collections(instance) {
            if collection.vid != 0x1532 || collection.feature_len < 90 { continue; }
            let Some(mut handle) = collection.open() else { continue };
            println!("watching iface={:?} for wake...", collection.interface);
            for i in 0..40 {
                let mut req = [0u8; 90];
                req[1] = 0x1F; req[5] = 0x02; req[6] = 0x07; req[7] = 0x80;
                req[88] = req[2..88].iter().fold(0u8, |a, b| a ^ b);
                let mut frame = [0u8; 91];
                frame[1..].copy_from_slice(&req);
                if handle.set_feature(&frame, std::time::Instant::now() + std::time::Duration::from_millis(300)) {
                    std::thread::sleep(std::time::Duration::from_millis(65));
                    if let Some(reply) = handle.get_feature(0, std::time::Instant::now() + std::time::Duration::from_millis(300)) {
                        let data = if reply.len() >= 91 { &reply[1..91] } else { &reply };
                        if data[0] != 4 {
                            println!("[{i}] status={:02x} cmd={:02x}:{:02x} raw={} => {}%",
                                data[0], data[6], data[7], data[9],
                                ((data[9] as f64 / 255.0) * 100.0).round() as u8);
                        }
                        if data[0] == 2 && data[6] == 7 && data[7] == 0x80 { return; }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(435));
            }
            println!("no answer in 20s");
        }
        return;
    }
    if args.get(2).map(String::as_str) == Some("--battery") {
        for collection in hid::collections(instance) {
            if collection.vid != 0x1532 || collection.usage_page != 1 || collection.usage != 2 {
                continue;
            }
            println!(
                "candidate iface={:?} feature_len={} product={}",
                collection.interface, collection.feature_len, collection.product
            );
            let Some(mut handle) = collection.open() else {
                println!("  open failed");
                continue;
            };
            for tid in [0x1Fu8, 0x3F, 0xFF, 0x9F, 0x08] {
                for (class, id) in [(0x07u8, 0x80u8), (0x07, 0x84)] {
                    let mut req = [0u8; 90];
                    req[1] = tid;
                    req[5] = 0x02;
                    req[6] = class;
                    req[7] = id;
                    req[88] = req[2..88].iter().fold(0u8, |a, b| a ^ b);
                    let mut frame = [0u8; 91];
                    frame[1..].copy_from_slice(&req);
                    let now = std::time::Instant::now();
                    if !handle.set_feature(&frame, now + std::time::Duration::from_millis(500)) {
                        println!("  tid={tid:02x} {class:02x}:{id:02x} set_feature=false");
                        continue;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    for _ in 0..4 {
                        let Some(reply) = handle.get_feature(
                            0,
                            std::time::Instant::now() + std::time::Duration::from_millis(300),
                        ) else { break };
                        let data = if reply.len() >= 91 { &reply[1..91] } else { &reply };
                        println!(
                            "  tid={tid:02x} {class:02x}:{id:02x} -> status={:02x} tid'={:02x} cmd={:02x}:{:02x} raw={}",
                            data[0], data[1], data[6], data[7], data[9]
                        );
                        if data[0] == 2 && data[6] == class && data[7] == id {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(80));
                    }
                }
            }
        }
        return;
    }
    for line in hid::debug_raw_collections(instance) {
        println!("{line}");
    }
}
