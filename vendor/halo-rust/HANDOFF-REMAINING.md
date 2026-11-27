# HaloBattery → Rust 迁移：剩余协议交接文档

> 写于 2026-11-27，供接手 agent 直接执行。本文自包含；不需要回看历史对话。
> 当前分支：`feat/halobattery-rust`（提交 `41bda45`）。**未合并 main、未发布、未在真机验证。**

> ## ⚠️ 状态更新（2026-11-28 05:02，接手先读）
>
> 本文 §1–§8 是 **2026-11-27 的原始 backlog 快照**，保留用于追溯当时的分析与决策。
> **§4 的 56 个协议、§5 的功能级偏差已在 2026-11-28 全部落地并合并 main**：
> - 新增 `state_a.rs`（turtlebeach / ajazz / lofree / audeze / dji）、`state_b.rs`（10 个状态机）、
>   `product_string.rs`（7 个产品字符串 / 特殊集合协议）、`longtail.rs`（26 个长尾 + 2 个 Logitech 子协议），
>   `razer.rs` 补齐 8 个专用帧；`hid.rs` 新增 `Collection.product` 并谨慎放行低 usage page 集合，
>   `instance_guard.rs` 实现单实例保护并接入 worker 主循环。
> - `vendor/halo-rust` 单测 **161 passed**；主程序 release 全量 **1624 passed / 0 failed**；
>   `daozai-halo.exe` 952,320 B（sha 前16 `fd6e66739fc62bcd`）；
>   测试安装包 `installer\daozai-setup-custom-isolated.exe`（包 sha 前16 `9ea481ed2efa1172`，exe `5df90ebdd649ead8`）。
> - **尚需处理的真实残留**：DJI 解析已实现但读数走 interface 6 的 WinUSB bulk，现有 `Provider` 只拿 HID 集合，
>   `poll()` 暂返空，需后续接 bulk 通道；mchose 输入唤醒门控、razer 跨 PID 去重、Logitech unit id 去重 /
>   `ui_message`、Corsair `--probe` 未复刻；所有协议均未在真机验证。
> - **旧 Python 组件 `vendor/halo-battery/` 已从工作树删除**（用户 2026-11-28 确认不再使用；git 历史可恢复）。
>   其上游 `LICENSE` / `credits.md` 迁到 `vendor/halo-rust/upstream-licenses/`，`--licenses` 路径已同步更新；
>   下文 §2.3 提到的 Python 源码路径仅供追溯，需查看时用 `git show <旧提交>:vendor/halo-battery/...`。
> - 最新状态与验证记录以 `vendor/halo-rust/MIGRATION.md` 和桌面 `HANDOFF.md` §7.0zzzzz-j 为准。

---

## 1. 目标与现状

把 `vendor/halo-battery/` 的 Python 无线电量组件（33,177 行 / 104 个 provider）全量替换为 Rust worker `daozai-halo.exe`，与主程序随包分发（不再从 Gitee 下载）。

**已完成**：Rust worker 骨架 + 43 个协议，8,175 行 Rust，79 个单元测试通过，协议握手实测正确，release 产物 525 KB（Python onefile 10 MB+），已内置进安装包（`daozai.iss` + `halo_component.rs`）。

**剩余**：Python 侧 99 个设备协议文件（已排除 `base.py`/`hidlist.py`/`worker.py`/`provider_registry.py`/`mouse_receivers.py` 等基础设施与 `test_*.py`），Rust 覆盖 43 个，**未移植 56 个**。逐项见 §4。

**此外还有功能级偏差**（协议已移植但行为未完全复刻），见 §5。

---

## 2. 代码结构与迁移机制（接手必读）

```
vendor/halo-rust/
├── Cargo.toml                  # 独立 crate，windows 0.58；有 release/dev-release profile
├── src/
│   ├── main.rs                 # stdin/stdout JSON 主循环（--stdio / --licenses）
│   ├── protocol.rs             # VERSION=1 wire format：ready / poll / reply，USB instance 解析
│   ├── hid.rs                  # HID 枚举与读写，复用自主程序 src/bt/usb_hid.rs（cfgmgr32 + 重叠 I/O）
│   ├── providers.rs            # Provider trait + Engine（观测缓存、稳定 key、256 上限）+ all_providers()
│   └── providers/
│       ├── simple.rs           # 声明式 Simple（一写 + 一定长读 + 字节解析），93 行
│       ├── compx.rs            # CompX report-8 共享协议 + 5 厂商，517 行
│       ├── hyperx.rs           # HyperX/Kingston 8 provider，1127 行
│       ├── mchose.rs           # mchose 主协议 + k99/kb/v9/a7/audio，1215 行
│       ├── razer.rs            # 90 字节 feature report 通用帧 + 89 PID 表，565 行
│       ├── steelseries.rs      # SteelSeries 三类交换 + Elite，1168 行
│       ├── big_vendors.rs      # Corsair / Asus / Logitech 三大状态机，2469 行
│       └── misc.rs             # 声明式 Spec 批量简单协议（~17 个），1021 行
├── build-release.ps1           # 构建 dist/daozai-halo.exe 并输出 size + sha
├── MIGRATION.md                # 迁移总表
└── HANDOFF-REMAINING.md        # 本文
```

### 2.1 核心接口（在 `src/providers.rs`）

```rust
pub trait Provider {
    fn name(&self) -> &'static str;
    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status>;
}
```

- `Collection` 字段：`usage_page`、`usage`、`interface`、`input_len`、`output_len`、`feature_len`；方法 `open() -> Option<Handle>`、`supports_report(id, output)`、`supports_feature(id)`。
- `Handle`（需 `&mut`）：`write(bytes, deadline) -> bool`、`read(deadline) -> Option<Vec<u8>>`、`set_feature(bytes, deadline) -> bool`、`get_feature(report_id, deadline) -> Option<Vec<u8>>`。
- `Status::new(key, name)`，字段 `level: Option<u8>`、`charging: Option<bool>`、`approximate`、`charging_known`、`online`、`kind`、`source`。
- `Engine` 按 receiver instance 调用每个 provider，负责观测缓存、稳定 key、256 设备上限。

### 2.2 注册新协议

1. 在 `src/providers/` 新建模块或加到已有模块；
2. 在 `src/providers.rs` 加 `pub mod xxx;` 并在 `all_providers()` 中 `all.extend(xxx::all_providers());`；
3. **简单协议**优先用 `misc.rs` 的声明式 `Spec`（VID/PID + 集合筛选 + Transport：`WriteRead`/`FeatureRead`/`FeatureInput` + parse fn），或 `simple.rs` 的 `Simple`；
4. **复杂协议**在专属模块实现 `Provider` trait，参考 `big_vendors.rs` / `mchose.rs` 的状态机写法；
5. 每个协议至少 1 个单元测试（请求构造 + 回复解析，用 Python 源码里的实测报文样例）；
6. `cd vendor/halo-rust && cargo test --offline` 全绿后再提交。

### 2.3 Python 参考源码位置

`vendor/halo-battery/upstream/providers/<name>.py`；共享基础设施在 `vendor/halo-battery/upstream/halo/`（hid_transport.py 861 行、io_scheduler.py 485 行、native_io.py 501 行、polling.py 603 行）和 `vendor/halo-battery/win_hid.py`。

**关键约束**：
- 严格按 Python 源码的字节格式、VID/PID、校验实现，**不猜字节格式**。很多协议是实机抓包逆向的结果，源码注释里标了抓包文件和 SHA-256。
- 没有真实设备只能做静态移植，**读数正确性必须实机确认**。这是本任务最大的固有风险，交接时务必如实说明。
- 不要改 `protocol.rs` / `hid.rs` / 主程序 `src/`，除非有明确证据需要扩展（如新增集合类型）。

---

## 3. 已覆盖的 43 个协议（不要重复移植）

alienware、am_infinity、arbiter、areson、astro、asus、attackshark_r11、attackshark_r5、beitong、cherry、compx、coolermaster、corsair、dareu、darmoshark、eweadn、hyperx、hyperx_alpha2、hyperx_alpha_wireless、hyperx_cloud3、hyperx_cloud3s、hyperx_cloud_core、hyperx_cloud_flight、hyperx_ngenuity、io_aurora、jbl_hidv3、lamzu、logitech、logitech_g325、mchose、mchose_a7、mchose_feedback、mchose_k99、mchose_kb、mchose_v9、monsgeek、ninjutso_sprime、pulsar、raikiri、razer、royal_kludge、royuan、sony_h9ii、steelseries、steelseries_elite。

注：`blackshark2020` 的 PID 由 razer.rs 通用帧部分覆盖（VID 1532 下匹配 product string），但其专用帧仍需单独实现；`logitech_centurion` / `logitech_g435_rtk` 未移植。

---

## 4. 未移植的 56 个协议（逐项）

按移植难度和建议顺序分组。每个条目的 `L` 数为 Python 源文件行数，hex 为源码中出现的关键常量（含 VID/PID/usage，需读源码确认）。

### 4.1 【优先】Razer 专用会话 / 私有帧（7 个，razer.rs 已有 SKIP_PIDS 表）

这些设备 VID 同为 0x1532、已在 razer.rs 的 `SKIP_PIDS` 中保守跳过。源码注释和 PID 表都在，风险相对可控，建议优先。

| 协议 | 行数 | 关键 PID | 说明 |
|---|---|---|---|
| `blackshark2020` | 135 | 0528 / 052E | BlackShark V2 Pro 2020 + 充电线 |
| `blackshark` | 222 | 0555 / 0556 / 0583 | V2 Pro 2023 "PA" output/input 协议（FF14） |
| `blackshark_v3` | 213 | 0577 / 057A | V3 / V3 Pro，ff14 会话 |
| `blackshark_v3x` | 230 | 057C / 057D | V3 X，FF82 |
| `blackshark_mtk` | 220 | 0565 / 056E | MediaTek 接收器 / 有线耳机（VID 1532 + FF14） |
| `barracuda` | 264 | 053A | Barracuda（VID 1532 + FF00） |
| `barracuda_x` | 285 | 0550 / 0552 | Barracuda X（VID 1532 + FF00） |

`razer.rs` 的 `SKIP_PIDS` 还含 0x0536（Barracuda 系列其它 PID）。`kraken_v4.py`（220L，056B/056C/1532/FF14）也是 Razer 系但未覆盖，可一并处理。

### 4.2 【优先】多步握手 / 状态机（12 个）

这些不是「一请求一回复」，需要按 Python 源码实现命令序列与状态机。参考 `big_vendors.rs` 的 Logitech HID++ 写法。

| 协议 | 行数 | 关键常量 | 说明 |
|---|---|---|---|
| `turtlebeach` | 610 | 10F5 / 2232 / 2250 / 2251 / 2253 / 225E / 2285 | 最大一个，多设备状态机 |
| `ajazz` | 603 | 248A / 249A / 3151 / 402D / 5031 / 5C2F | 大状态机 |
| `lofree` | 522 | 024F / 05AC / 0092 等 | 多协议族 |
| `audeze` | 523 | 0CD6 / 3329 / 4B18-4B1A / 4B28-4B29 | 耳机协议族 |
| `dji` | 415 | 2CA3 / 3692 / 4011 / 8408 / BB01 | DJI 设备 |
| `inphic` | 373 | 1D57 / FA60 / FA65 | 多步 |
| `rapoo` | 347 | 1413 / 14A9 / 185A / 1924 | 多步 |
| `gwolves` | 336 | 2708 / 2717 / 2719 / 3508 / 3517 / 3519 | 多型号 |
| `steam` | 357 | 28DE / 1142 / 1302 / 1304 / 1305 | Steam 控制器 |
| `plantronics` | 190 | 047F / 02EE / 02F7 / FFA2 | 多步握手 |
| `wraith` | 186 | 093A / 522C / 622C / FF05 | 多步 |
| `fractal` | 139 | 2001 / 36BC / FF00 | 多步 |
| `fantech` | 161 | 25A7 / FA7B / FA7C / FA7E | 多步 |
| `endgame_we` | 160 | 1961 / 3367 / FF01 / FF02 | 多步 |
| `jabra` | 181 | 0B0E / 24C7-24CA / 24E9 / 2E57 | 多步 |

（表中实际列了 15 个，因为 §4 的 56 个里这些都属于状态机类；按你自己的节奏挑。）

### 4.3 【中】需产品字符串放行（5 个，Rust `Collection` API 不暴露 product string）

Python 侧按 `product string` 区分型号才放行；Rust `Collection` 无此字段。需要先确认能否用 VID/PID + interface + usage page 组合等价替代；如果不能，要扩展 `hid.rs` 读取设备属性（注意：改 `hid.rs` 影响所有协议，需谨慎并跑全量测试）。

| 协议 | 行数 | 关键常量 |
|---|---|---|
| `gs3075t` | 175 | 0C45 / FEFE / FF60 |
| `urx` | 189 | 3002 / 36B0 / FF60 |
| `attackshark_x66` | 161 | 0C45 / FEFE / FF60 |
| `redsquare` | 162 | 342D / E411 / FF60 |
| `atk_keyboard` | 201 | 320F / 5055 / 5088 / FF1C |

### 4.4 【中】非 vendor 集合 / 特殊传输（2 个）

- `jbl_quantum350.py`（150L，0ECB:206B，usage page `0001:0000`）：`hid.rs` 目前只放行 `usage_page >= 0xFF00` 的集合，该消费集合不会传入。要么扩展过滤规则（风险：所有 provider 都会看到新集合），要么确认有 vendor 备用集合。
- `moondrop.py`（149L，0133:35D8，usage page `000C`）：同上。

### 4.5 【低】其它未覆盖（28 个）

这些多为小众设备或已在 Python 侧被其它 provider 覆盖的长尾。按源码逐个判断是否值得移植。

| 协议 | 行数 | 关键常量 | 协议 | 行数 | 关键常量 |
|---|---|---|---|---|---|
| `akko_s9` | 254 | 1087 / 36EB | `atk_nearlink` | 277 | 108B / FF B2 |
| `atk_bitmouse` | 138 | 373B / FF05 | `aula` | 154 | 0C45 / 38A6 / FF67 |
| `aula_f108` | 162 | 0C45 / FF60 | `eightbitdo` | 222 | 2DC8 / 6000-6009 |
| `eightbitdo_ultimate2` | 213 | 2DC8 / 310B / FF7A | `eweadn_ydx` | 272 | 026F / 04F3 |
| `finalmouse` | 208 | 361D / FF00 | `flydigi` | 211 | 2401 / 37D7 / FFA0 |
| `gamesir` | 185 | 100B / 1053 / 10B7 | `glorious` | 257 | 2011-2025 |
| `irocks` | 243 | 14A5 / C009 / C010 | `jbl` | 223 | 0ECB / 2088 / FF13 |
| `keychron` | 284 | 3434 / D026 / D028 / FF60 | `kraken_v4` | 220 | 056B / 056C / FF14 |
| `logitech_centurion` | 385 | 046D / 0AF7 | `logitech_g435_rtk` | 252 | 046D / 0ACB / FFC0 |
| `machenike` | 133 | 248A / FF4C / FF4E | `msi` | 315 | 0D3B / 0D3C / 0D4B |
| `ninjutso_sora_v3` | 203 | 093A / E010 / EB02 | `nzxt` | 257 | 1E71 / 2101 / FFCA |
| `skullcandy` | 251 | 0CD6 / 3210 / 5310 | `sony_inzone` | 214 | 054C / 0E53 / 0EBF |
| `steam` | 357 | 28DE / 1302 / 1304 | `vaxee` | 221 | 3057 / FF05 |
| `wlmouse` | 272 | 36A7 / A868 / A880 | `zowie` | 193 | 04A5 / 8005-800A |

注：`sony_serial.py`、`pulsar_models.py`、`dji_winusb.py` 等属于基础设施/测试，不计入设备协议。

---

## 5. 功能级偏差（协议主体已移植，行为未完全复刻）

即使 56 个协议全部移植，以下 Python 侧行为目前仍未复刻。接手时按价值决定是否补：

1. **mchose**：`_smooth_g7` 中值滤波 + 80% 启动/低电量确认状态机；K99/A7 V3 的 `KeyboardActivity`/`MouseActivity` 输入唤醒门控（依赖 Python 输入监听线程）；`_last` 休眠保持读数（Rust 用 `fresh`/`age_ms`，无回复时不产出 Status）。
2. **razer**：跨 PID 同型号灰显去重（需全局视图，单次 poll 拿不到）。
3. **Logitech**：unit id 跨接收器去重、`ui_message` 文案、G535 的 `000C:0001` 消费集合（hid.rs 只放行 >= FF00）。
4. **HyperX**：Kingston 修订的 `get_input_report(0x06, 62)` 预读（结果在 Python 侧被忽略，Rust 用 10ms 短 deadline 轮询代替 `set_nonblocking`）。
5. **Corsair**：Bragi SABRE v3 跨实例「线缆出现即视为充电」由 `hs80_cable_seen` 近似；`--probe` 诊断路径未移植（tray 不发）。
6. **Asus**：Harpe II 接收器的 `mi_02&col03` 路径子串判断改用 (pid, interface=2, FF01:0001) 匹配。
7. **单实例保护**：Python 侧 `InstanceGuard` Rust 版尚未实现。

---

## 6. 验证与发布流程（每个批次必做）

```bash
# 1) Rust worker 单元测试（在 vendor/halo-rust/）
cd vendor/halo-rust
cargo test --offline                    # 当前 79 passed，新增后应只增不减

# 2) 协议握手（确认 JSON wire format 没被破坏）
printf '{"version":1,"id":42,"command":"poll","receivers":[]}\n' \
  | ./target/release/daozai-halo.exe --stdio
# 期望输出：{"version":1,"event":"ready",...} + {"version":1,"id":42,"devices":[]}

# 3) 主程序全量测试（在仓库根，Cargo.toml 已在根）
cd /d/code/daozai-custom
DAOZAI_DATA_DIR=D:/code/daozai-custom/merge-work/test-data \
  MSYS_NO_PATHCONV=1 cargo test --offline --locked --profile dev-release --lib -- --test-threads=1
# 当前基线：1620 passed / 0 failed / 141 ignored

# 4) release 构建 + 打包
cargo build --offline --release --manifest-path vendor/halo-rust/Cargo.toml
cargo build --offline --locked --release --bin daozai --bin daozai-bridge
# PowerShell: installer\build-innosetup.ps1 -CargoProfile release -InnoCompiler 'D:\software\Inno Setup 7\ISCC.exe'
```

**注意**：PowerShell 5.1 下调 `build-innosetup.ps1` 不要加 `| Select-Object -First N`，管道截断会中断 ISCC（历史上踩过坑）；用 `2>&1 | Out-String` 拿完整输出。

---

## 7. 当前 git 状态与注意事项

- 分支 `feat/halobattery-rust`，HEAD `41bda45`，工作树干净。
- 已把 `dz-custom/` 层级拍平到仓库根（提交 `3d307bf`），Cargo.toml/src/spike/vendor 都在根。构建命令不再有 `--manifest-path dz-custom/Cargo.toml`。
- 安装包随包分发 Rust 组件（`daozai.iss` 把 `vendor/halo-rust/target/{profile}/daozai-halo.exe` 复制到 `{app}`），不再走 Gitee 下载。
- 本地 `installer/daozai-setup-custom-isolated.exe` 是验证产物（sha `63574b58`），**与已发布的 v1.4.1（`c392da4e`）不同**，不要误当成发布版。
- 历史文档（HANDOFF/CHANGELOG/PATCHES）保留原始路径叙述，不要为了「一致性」去改它们。

---

## 8. 建议的执行顺序（给接手 agent）

1. **先做 §4.1 Razer BlackShark/Barracuda 7 个**：PID 表现成、`SKIP_PIDS` 已标注，改动集中在 `razer.rs`，风险最可控。
2. 再做 §4.2 中源码里有明确实测报文样例的简单状态机（如 `fractal` 139L、`fantech` 161L、`endgame_we` 160L、`plantronics` 190L），用 `misc.rs` 的 Spec 或专属模块。
3. 然后处理 §4.3 的 5 个产品字符串协议：先评估能否用 VID/PID + interface 等价替代，**不要贸然改 `hid.rs`**。
4. §4.4 的 2 个特殊集合协议放到最后，需要扩展集合过滤规则时单独评估。
5. §4.5 的 28 个长尾按用户实际设备反馈决定，不必盲目全做。
6. 每完成一批就更新 `MIGRATION.md` 的状态表并跑 §6 全套验证。
7. **全程如实标注哪些没在真机验证**；最终交付时必须说明「静态移植只能保证与 Python 逻辑一致，读数正确性需实机确认」。
