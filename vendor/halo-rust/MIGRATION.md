# HaloBattery Python → Rust 迁移状态

> 目标：把 `vendor/halo-battery/` 的 Python 组件（33,177 行 / 104 个 provider）全量替换为 Rust。
> 分支：`feat/halobattery-rust`。主程序在另一个分支，本分支不改动。

## 架构（已实现）

```
岛仔主体 (Rust)  --stdin/stdout JSON-->  daozai-halo (Rust worker)
                 <--devices JSON------      │
                                           ├ cfgmgr32 按 USB instance 定位 HID 集合
                                           ├ Provider trait: name + poll(instance, collections)
                                           └ 观测缓存 / 稳定 key / 256 设备上限
```

- `protocol.rs`：与 Python 组件 VERSION=1 完全一致的 wire format（ready / poll / reply）
- `hid.rs`：复用自主程序 `src/bt/usb_hid.rs`，cfgmgr32 + 重叠 I/O
- `providers.rs`：Provider trait + Engine（缓存、key、去重）
- `providers/simple.rs`：声明式 Simple provider

## 迁移进度

| 协议族 | 状态 | 备注 |
|---|---|---|
| 骨架 / 协议 / HID | ✅ 完成 | 4 单测通过，协议握手实测正确 |
| BEITONG KP40D | ✅ 完成 | 首个迁移协议 |
| CompX 系列 | ✅ 完成 | compx.py + 5 个厂商，517 行 Rust |
| mchose 系列 | ✅ 完成 | 主协议 + k99/kb/v9/a7/audio，1216 行 Rust |
| HyperX/Kingston | ✅ 完成 | 8 个 provider，1112 行 Rust；部分私有会话跳过 |
| Razer | ✅ 完成 | 通用 0x07 帧协议，565 行；8 个子协议跳过 |
| SteelSeries 系列 | ✅ 完成 | 1150 行，45+ PID，11 测试 |
| big_vendors (Corsair/Asus/Logitech) | ✅ 完成 | 2465 行 Rust |
| misc (~30 个简单协议) | ✅ 完成 | 1021 行 Rust |
| 其余 ~50 个 | ⬜ 待迁移 | 见下方分组 |

## 剩余协议分组（按 Python 源文件精确核对，2026-11-27）

Python 侧 99 个设备协议文件（已排除 base/hidlist/worker/registry 等基础设施与 test_*）。
Rust 已覆盖 43 个（含 beitong；blackshark2020 仅通用帧兜底），**未移植 56 个**：

- **多步握手 / 状态机**（12）：plantronics、wraith、fractal、fantech、endgame_we、jabra、
  atk_bitmouse、zowie、turtlebeach、ajazz、sony_inzone、gamesir
- **需产品字符串放行**（5，Rust Collection API 不暴露）：gs3075t、urx、attackshark_x66、
  redsquare、atk_keyboard
- **非 vendor 集合 / 特殊传输**（2）：jbl_quantum350（0001:0000）、moondrop（0x000C）
- **Razer 专用会话 / 私有帧**（7 个 provider，razer.rs 的 SKIP_PIDS 另含 Barracuda 系列 PID）：
  BlackShark 2020 / 2023 PA / V3 / V3 Pro / V3 X、MediaTek 接收器、Barracuda 系列
- **Logitech / Corsair 未移植子协议**（3）：logitech_centurion（bridge 协议）、
  logitech_g435_rtk（bootloader 切换）、Corsair --probe 诊断路径
- **其他未覆盖**（28）：akko_s9、atk_nearlink、audeze、aula、aula_f108、barracuda_x、
  dji、eightbitdo、eightbitdo_ultimate2、eweadn_ydx、finalmouse、flydigi、glorious、
  gwolves、inphic、irocks、jbl、keychron、kraken_v4、lofree、machenike、msi、
  ninjutso_sora_v3、nzxt、rapoo、skullcandy、sony_serial、steam、vaxee、wlmouse

**功能级偏差**（协议已移植但 Python 侧行为未完全复刻）：
- mchose：G7 中值滤波、K99/A7 输入唤醒门控、休眠保持读数
- razer：跨 PID 同型号灰显去重
- Logitech：unit id 跨接收器去重、G535 的 000C 集合、ui_message 文案
- HyperX：Kingston 的 GET_REPORT(Input) 预读（结果在 Python 侧被忽略）

## 验证状态

- `cargo test --offline`：79 passed / 0 failed（8 个协议族 / 8171 行 Rust）
- `cargo build --release`：8.4s，二进制 ~1 MB（PyInstaller onefile 约 10 MB+ 且启动需解压）
- 协议握手：`ready` + `poll` 回复格式正确
- **硬件验证**：未在真实设备上进行。每个协议的字节格式必须在实机上确认。

## 待办

1. 各 agent 完成协议族后统一整合 `all_providers()` 注册
2. 补齐剩余协议（优先常见厂商）
3. 为每个协议加单元测试（请求构造 + 回复解析）
4. 构建 `daozai-halo.exe`，改 `halo_component.rs` 指向 Rust 组件
5. 实机验证：逐个设备确认电量/充电状态与 Python 版一致
6. 全量测试后合并到 main

## 风险

- 协议里很多是实机抓包逆向的结果（源码注释标注了抓包文件和 SHA），没有真实设备时只能做静态移植，无法验证正确性
- Python 侧有复杂异步调度（hid_transport 861 行 + io_scheduler 485 行）处理设备忙/超时/取消，Rust 版目前用同步调用 + deadline，需要在实机确认不会卡住
- 单实例保护（InstanceGuard）Rust 版尚未实现

## 接线与发布（2026-11-27）

改为**随应用分发**，与 `daozai-bridge.exe` 同一模式，不再走 Gitee 按需下载。

- `build-release.ps1`：构建 `dist/daozai-halo.exe` 并输出 size + sha256，供发布记录使用。
- `halo_component.rs`：组件目录改为「主程序 exe 同目录」，优先直接激活 `daozai-halo.exe`，无需网络、无需 receipt；仅当内置缺失（开发运行/旧版免安装包）时才回退到缓存与 Gitee 下载。wire 协议 VERSION=1 与 Python 组件完全一致。
- 构建顺序：`cargo build --offline --release --manifest-path vendor/halo-rust/Cargo.toml` → 主程序 release 构建 → `installer\build-innosetup.ps1`（脚本会检查内置 exe 是否存在）。
- 安装包：`daozai.iss` 已把 `vendor\halo-rust\target\{profile}\daozai-halo.exe` 复制到 `{app}\daozai-halo.exe`，并加入卸载清理；免安装 ZIP 也必须把该 exe 放在 `daozai.exe` 同目录。
- 实测产物：release 525,824 B（Python PyInstaller onefile 约 10 MB+）。release（显式 LTO/strip）SHA256 `dd2a8cd355ac9bd758542d9b03cd6604a181381312676e4fe9605c06ce99a217`；dev-release `fc603486034945025835fb0594c0c9b2f9bbab5717bbfb17d1f7db0527ef5a00`。构建参数变化会导致 sha 变化，以实际产物为准。
- 未做：尚未合并到 main、尚未发布新版本；当前只在 `feat/halobattery-rust` 分支。
