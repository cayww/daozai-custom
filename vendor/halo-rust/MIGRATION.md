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

## 残留风险与已知偏差（必须如实交付）

- **DJI**：解析与状态机已实现，但真实读数走 interface 6 的 WinUSB bulk；当前 `Provider` 只接收 HID 集合，
  `poll()` 在缺数据时返回空，需后续给 worker 增加 bulk 通道才能真正生效。
- **mchose**：K99/A7 的 `KeyboardActivity` / `MouseActivity` 输入唤醒门控依赖主程序输入状态，
  worker 侧未监听键盘鼠标；跨进程一致性待实机确认。
- **razer**：跨 PID 同型号灰显去重需要全局视图，单次 poll 拿不到，未实现。
- **Logitech**：unit id 跨接收器去重、`ui_message` 文案未复刻；G535 的 `000C:0001` 集合仍受 HID 放行规则限制。
- **HyperX**：Kingston `get_input_report(0x06, 62)` 预读在 Python 侧结果被忽略，Rust 用短 deadline 轮询代替，字节协议等价。
- **Corsair**：Bragi SABRE v3「线缆出现即充电」用跨轮询 `sabre_v3_cable_seen` 近似；`--probe` 诊断路径未移植（主程序不发）。
- **Asus**：Harpe II 接收器路径子串判断改为 (pid, interface=2, FF01:0001) 匹配。
- **HID 放行扩展**：低 usage page 集合现可被所有 provider 看到，但各协议均按 VID/PID + interface + usage 精确匹配；
  新设备插入时仍需观察是否误触发。
- **无真机验证**：所有协议均为静态移植，只保证与 Python 逻辑/字节格式一致，读数正确性必须逐设备实机确认；
  Python 侧复杂异步调度在 Rust 侧用同步调用 + deadline，实机需确认不卡住。

## 验证状态（2026-11-28 05:01，Python 组件删除后重验）

- `vendor/halo-rust`：`cargo test --offline` — **161 passed / 0 failed**（3 条告警：2 个 unused、1 个 dead_code，不影响功能）。
- 协议握手：`{"version":1,"id":42,"command":"poll","receivers":[]}` → `ready` + `poll` 回复正确（04:24 无实例时验证通过；05:02 用户已启动岛仔，第二次运行按设计被单实例保护拒绝，说明 guard 生效）。
- 主程序：`cargo test --locked --release --lib -- --test-threads=1` — **1624 passed / 0 failed / 141 ignored**。
- release 构建：主程序双 bin + `daozai-halo.exe` 均成功；`daozai-halo.exe` 952,320 B（sha256 前16 `fd6e66739fc62bcd`）。
- 安装包：`installer\daozai-setup-custom-isolated.exe`（12,650,777 B，包 sha256 前16 `9ea481ed2efa1172`，
  exe `5df90ebdd649ead8`，bridge `77ede6ed744346b9`），版本 1.4.1；未部署、未推 origin。
- 内置接线：`daozai.iss` 把 `vendor\halo-rust\target\{profile}\daozai-halo.exe` 复制到 `{app}\daozai-halo.exe`，
  卸载时清理；无需 Gitee 下载。

## 后续待办

1. 实机逐设备验证读数与充电状态（尤其新移植 56 个协议）。
2. 给 DJI 接 WinUSB bulk 通道，使 `state_a.rs` 中已实现的解析真正生效。
3. 视用户设备反馈补 mchose 输入唤醒门控、razer 跨 PID 去重、Corsair `--probe` 等边角行为。
4. 发布前确认是否需要升版本号 / 发 GitHub Release / 部署（当前仅为本地测试包）。
