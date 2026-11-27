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
- `hid.rs`：复用自主程序 `dz-custom/src/bt/usb_hid.rs`，cfgmgr32 + 重叠 I/O
- `providers.rs`：Provider trait + Engine（缓存、key、去重）
- `providers/simple.rs`：声明式 Simple provider

## 迁移进度

| 协议族 | 状态 | 备注 |
|---|---|---|
| 骨架 / 协议 / HID | ✅ 完成 | 4 单测通过，协议握手实测正确 |
| BEITONG KP40D | ✅ 完成 | 首个迁移协议 |
| CompX 系列 | ✅ 完成 | compx.py + 5 个厂商，517 行 Rust |
| mchose 系列 | 🔄 进行中 | mchose.py / k99 / kb / v9 / a7 |
| HyperX/Kingston | ✅ 完成 | 8 个 provider，1112 行 Rust；部分私有会话跳过 |
| Razer | ✅ 完成 | 通用 0x07 帧协议，565 行；8 个子协议跳过 |
| 其余 ~85 个 | ⬜ 待迁移 | 见下方分组 |

## 剩余协议分组

- 大型状态机：corsair.py (1440)、asus.py (770)、logitech.py (721)、steelseries.py (626)、turtlebeach.py (610)、ajazz.py (603)
- 中型：pulsar / audeze / lofree / dji / inphic / steam / rapoo / gwolves / msi 等
- 简单声明式：~50 个 <200 行的 provider（可按 Simple 模式批量迁移）

## 验证状态

- `cargo test --offline`：12 passed（骨架 + Razer 阶段）
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
