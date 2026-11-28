# HaloBattery Python → Rust 迁移状态

> 目标：把 `vendor/halo-battery/` 的 Python 组件（33,177 行 / 104 个 provider）全量替换为 Rust。
> **2026-11-28 状态：56 个剩余协议已全部静态移植并注册，代码已合并 main；构建、单测、全量测试与安装包均通过。**
> **旧 Python 组件目录 `vendor/halo-battery/` 已从工作树删除（git 历史可恢复）；上游 LICENSE / credits.md 迁到
> `vendor/halo-rust/upstream-licenses/`，`daozai-halo.exe --licenses` 继续可用。**
> 所有读数正确性仍需在真机上逐设备确认。

## 架构（已实现）

```
岛仔主体 (Rust)  --stdin/stdout JSON-->  daozai-halo (Rust worker)
                 <--devices JSON------      │
                                           ├ cfgmgr32 按 USB instance 定位 HID 集合
                                           ├ Provider trait: name + poll(instance, collections)
                                           └ 观测缓存 / 稳定 key / 256 设备上限
```

- `protocol.rs`：与 Python 组件 VERSION=1 完全一致的 wire format（ready / poll / reply）
- `hid.rs`：复用自主程序 `src/bt/usb_hid.rs`，cfgmgr32 + 重叠 I/O；
  新增 `Collection.product`（`HidD_GetProductString`），并谨慎放行 JBL / MOONDROP /
  akko_s9 / eightbitdo / eweadn_ydx / machenike / msi PRO / zowie 的低 usage page 集合。
- `providers.rs`：Provider trait + Engine（缓存、key、去重、256 上限），统一 `all_providers()` 注册。
- `instance_guard.rs`：Python `InstanceGuard` 等价的全局 mutex 单实例保护（已接进 worker 主循环）。

## 迁移进度

| 协议族 | 状态 | 备注 |
|---|---|---|
| 骨架 / 协议 / HID | ✅ 完成 | 协议握手实测正确；HID 集合放行规则已就协议精确匹配收敛 |
| 原 43 个协议族 | ✅ 完成 | compx / mchose / HyperX / Razer 通用帧 / SteelSeries / big_vendors / misc 等 |
| Razer 专用帧 8 个 | ✅ 完成 | blackshark 2020/2023/v3/v3x/mtk、barracuda/x、kraken_v4；约 1777 行新增 Rust |
| 状态机 A 5 个 | ✅ 完成 | turtlebeach / ajazz / lofree / audeze / dji（DJI 见下方残留风险） |
| 状态机 B 10 个 | ✅ 完成 | inphic / rapoo / gwolves / steam / plantronics / wraith / fractal / fantech / endgame_we / jabra |
| 产品字符串与特殊集合 7 个 | ✅ 完成 | gs3075t / urx / attackshark_x66 / redsquare / atk_keyboard / jbl_quantum350 / moondrop |
| 长尾 26 个 | ✅ 完成 | akko_s9 / atk_nearlink / atk_bitmouse / aula / aula_f108 / eightbitdo / eightbitdo_ultimate2 / eweadn_ydx / finalmouse / flydigi / gamesir / glorious / irocks / jbl / keychron / machenike / msi / ninjutso_sora_v3 / nzxt / skullcandy / sony_inzone / steam / vaxee / wlmouse / zowie 等 |
| Logitech 未移植子协议 | ✅ 完成 | logitech_centurion / logitech_g435_rtk 在 longtail.rs |
| 功能级偏差 | ✅ 已补 | mchose 中值滤波/80% 确认/休眠保持、Logitech/Corsair/HyperX/Asus 近似规则、单实例保护 |

## 2026-11-28 05:26 修复：雷蛇 Viper V3 Pro 不显示电量

用户反馈「鼠标雷蛇 V3 Pro 原本能显示电量，迁移后不显示」。定位到两个叠加根因，均已修复：

1. **内置接收器目录是错的**：上一轮从 Python 源码用正则抽 VID/PID 时，把每个文件的首行
   `from __future__ import annotations` 当成设备名，89 条新增记录里 88 条名字错误，而且只收进
   0536/053A/0550/0552 等 Python 明确**跳过**的 Razer PID，**没有 00C0/00C1**。主程序只把目录里的
   USB ID 发给 worker，所以 Viper V3 Pro 根本没被轮询。现已用 AST 从 git 历史里的 Python 表重建：
   原 23 条中文条目保留，新增 422 条来自各 provider 的 (VID, PID, 名称)，共 445 条、无错误名字。
2. **HID 集合白名单漏掉雷蛇鼠标集合**：Viper V3 Pro 的电量 feature report 在标准 HID 鼠标集合
   `0001:0002` 上，而 `hid.rs` 原先只放行 `usage_page >= 0xFF00` 与少数例外（page 1 只放行
   usage 0/4/5，单测还明确断言 usage 2 不放行）。现已按社区逆向资料放行 `VID_1532` 的
   `0001:0002`（interface 3 仍拒绝），且该集合被系统独占时先用 `dwDesiredAccess=0` 打开，
   只做 Feature IOCTL；再补上 Python `maybe_wireless` 的产品名门控，未知 Razer PID 只要名字含
   `hyperspeed/wireless/receiver/dongle/dock/blackshark/barracuda/nari` 也会探测。

社区依据（协议事实，非复制代码）：
- OpenRazer `razermouse_driver.c` / 社区整理 https://github.com/Skesov/rigbat/blob/master/docs/razer.md ：
  90 字节请求、`07:80` 读电量、`07:84` 读充电、事务号 0x1F、CRC 为 [2..88) XOR、Viper V3 Pro 61ms 等待。
- https://github.com/linglochen/RazerV3BatteryTray ：`1532:00C1` 无线 / `00C0` 有线、`0001:0002` 集合、
  `CreateFile` 用 `dwDesiredAccess=0` 绕过独占锁、插线时两条链路只有一条响应（工作树已覆盖该场景）。
- https://github.com/JimmyRespawn/vigil-watt/blob/main/docs/hardware/razer-viper-se.md ：91 字节 report、
  事务号 0x1F、SetFeature 后 60ms 读取、响应 status/CRC 校验细节（与本实现一致）。

## 残留风险与已知偏差（必须如实交付）

- **DJI**：WinUSB bulk 通道已接（`winusb.rs`，按 USB instance 找 MI_06 的 WinUSB 接口，后台线程只读 EP 0x86
  并喂给已实现的 DUML 分帧/CRC/状态机）；无真机，需在用户设备上确认帧推送与读数。
- **mchose**：K99（10s 活动窗 / 300s 刷新）与 A7 V3 Ultra+（250ms / 60s）的输入唤醒门控已由 `activity.rs`
  的 Raw Input 监听实现；Status/Reading wire format 没有 `polling_paused` 字段，门控期间输出缓存的最后读数。
- **razer**：跨 PID 同型号灰显去重已在 Engine 汇总后实现（另一 PID 本轮在线时丢弃灰显副本）。
- **Logitech**：unit id 跨接收器去重已在 Engine 按 `status.key` 生成稳定 key；`ui_message` 文案键已接线到
  Reading wire 与主程序悬停提示，`ui_params` 的具体档位/插电前数值未传；G535 的 `000C:0001` 集合仍受 HID 放行规则限制。
- **HyperX**：Kingston 的 `get_input_report(0x06, 62)` 已用 `HidD_GetInputReport` 精确复刻（HeadsetControl 的 prepareDevice），不再用输入管道 read() 近似。
- **Corsair**：Bragi SABRE v3「线缆出现即充电」用跨轮询 `sabre_v3_cable_seen` 近似；`--probe` 诊断路径未移植（主程序不发）。
- **Asus**：Harpe II 接收器已按 Python/G-Helper 的 `mi_02&col03` 路径子串精确匹配（`Collection::path()`），不再只靠 interface/usage。
- **HID 放行扩展**：低 usage page 集合现可被所有 provider 看到，但各协议均按 VID/PID + interface + usage 精确匹配；
  新设备插入时仍需观察是否误触发。
- **无真机验证**：所有协议均为静态移植，只保证与 Python 逻辑/字节格式一致，读数正确性必须逐设备实机确认；
  Python 侧复杂异步调度在 Rust 侧用同步调用 + deadline，实机需确认不卡住。

## 2026-11-28 05:49 补齐剩余项

- **DJI bulk**：新增 `src/winusb.rs`，按 Python `_CfgMgr.find` 的顺序找 MI_06 的 WinUSB 设备接口
  （`CM_Get_DevNode_Registry_PropertyW` 验驱动 = winusb、读 `DeviceInterfaceGUID(s)`、枚举接口路径），
  用 `WinUsb_Initialize/QueryPipe/SetPipePolicy/ReadPipe` 只读 EP 0x86；`state_a.rs` 的 `Dji` 首次 poll
  起后台线程，分帧后喂 `absorb()`，拔插自动清理状态。协议来源：
  https://github.com/ShadowBitBasher/DJI-Mic-Control/blob/main/PROTOCOL.md （Unlicense）。
- **输入唤醒门控**：新增 `src/activity.rs`，用 `RegisterRawInputDevices(RIDEV_INPUTSINK)` + 消息专用窗口，
  只读 RAWINPUTHEADER 与设备路径（不保留按键内容），K99/A7 按 Python 的活动窗与刷新周期门控探测。
  Raw Input 不可用时退化为「不门控」，不会比迁移前更差。
- **Razer 去重 / Logitech key**：`Engine::poll` 汇总后丢掉「同型号另一 PID 已在线」的 Razer 灰显副本；
  Logitech 的最终 key 只由 provider + `status.key` 生成，实现跨接收器 unit id 合并。
- 单测新增 5 个（GUID 解析、接收器实例门控、Raw Input 路径归一化、Razer 灰显去重 ×2）。

## 2026-11-28 09:08 未做项复核：用途与社区方案

用户追问「未做的有什么用，社区有没有方案」。逐项结论：
- **Corsair `--probe`**：Python 自己的诊断 CLI（issue #28），做 6 秒先听后写、打印每次写入返回值/帧耗时/报告长度、
  必要时切厂商软件模式。它明确写着「比托盘该做的更深」，且会向控制集合试探写入，放进 worker 会阻塞托盘轮询，
  社区里对应的是 OpenLinkHub / ckb-next 这类完整控制守护进程（不是诊断补丁）。**不做是正确取舍**。
- **Logitech `ui_message`**：只是 Python UI 的提示文案键（`state.approx_grade` / `charging_was` /
  `charging_level_unknown`），社区无标准，属于本组件自身 UI 约定。已端到端接线：worker `Status`/`Reading`
  增加 `ui_message`，主程序 Reading/BatteryReading 透传，悬停提示翻译为中文；`ui_params` 的具体数值未传。
- **HyperX Kingston 预读**：社区 HeadsetControl PR #519 的 `prepareDevice`，写前用 `get_input_report(0x06, 62)`
  清掉陈旧输入帧。已用 `HidD_GetInputReport` 精确实现，不再是管道 read 近似。
- **Asus Harpe II 路径**：G-Helper `HarpeIIAce.cs` 的路径 `mi_02&col03` 判据（`asus.py` 引 upstream #364）。
  已用 `Collection::path()` 做大小写无关子串匹配。

## 2026-11-28 09:27 实机排查：雷蛇 V3 Pro 系列仍读不到电量（根因找到并修复）

用户反馈 DeathAdder V3 Pro（`1532:00B6/00B7`）读不到电量。诊断步骤：
1. 当前运行的是 `C:\Program Files (x86)\DaozaiCustom\`，exe `47cfea52`（05:49 包），**不是最新版**。
2. 用新增的 `hid-probe` 诊断 bin 枚举该 USB 实例下全部 HID 集合，发现：
   - 真正能收 90 字节 feature report 的是 `MI_00 / usage 0001:0002 / feature_len=91`（interface 0）；
   - `MI_01 / usage 0001:0002` 的 `FeatureReportByteLength=0`，不承载电量；
   - **`admitted=false`**：`hid::collections()` 的节点级 `supported_collection()` 只按 `supported_usage()`
     过滤，`0001:0002` 在这里就被丢弃，之前在 `collection_for()` 里加的 Razer 鼠标特判根本没机会执行。
3. 修复：`supported_collection()` 从硬件 ID 解析 VID/PID，节点级过滤也调用
   `razer_mouse_feature_collection()`；两个层级的判据终于一致。诊断确认修复后 `admitted=true`。
4. 实机 feature 查询：所有事务号均回 `status=04`（Python/OpenRazer 定义为「睡眠 / 无响应」）；
   `RazerV3BatteryTray` 社区文档要求「移动鼠标确认不在睡眠」。已交付新包，请用户移动鼠标后复测。

新增 `src/bin/hid-probe.rs`（`--wait` 模式可轮询等待唤醒）作为后续诊断工具，不进安装包。

## 验证状态（2026-11-28 09:27）

- `vendor/halo-rust`：`cargo test --offline` — **169 passed / 0 failed**（新增硬件 ID VID/PID 解析回归）。
- 主程序：`cargo test --locked --release --lib -- --test-threads=1` — **1624 passed / 0 failed / 141 ignored**。
- release 构建：主程序双 bin（独立 target 目录，避开运行中的 exe）+ `daozai-halo.exe` 均成功。
- 安装包：`installer\daozai-setup-custom-isolated.exe`（12,680,333 B，包 sha256 前16 `46a69b20321eeb17`，
  exe `94ed6f5fe60d8ca1`，bridge `dcd243eed0448f88`，halo `ea1db245840ea3b8`），版本 1.4.1；未部署、未推 origin。
- 内置接线：`daozai.iss` 把 `vendor\halo-rust\target\{profile}\daozai-halo.exe` 复制到 `{app}\daozai-halo.exe`，
  卸载时清理；无需 Gitee 下载。

## 后续待办

1. 实机逐设备验证读数与充电状态（尤其 DJI WinUSB、Viper V3 Pro、mchose 输入门控）。
2. Corsair `--probe` 按设计不移植（诊断专用，会试探写入且阻塞托盘）；Logitech `ui_params` 具体数值未传（仅提示文案已接线）。
3. 发布前确认是否需要升版本号 / 发 GitHub Release / 部署（当前仅为本地测试包）。
