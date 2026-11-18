# 岛仔（Daozai）交接文档

> **最后更新**：2026-11-18 19:26
> **本文档是唯一权威交接入口**（`C:\Users\cay\Desktop\HANDOFF.md`）。
> 仓库内同步副本：`D:\code\daozai-custom\docs\HANDOFF.md`；凌晨版原件在
> `Daozai-液态玻璃预览\_archive\交接文档-2026-11-16.md`。

---

## 0. 30 秒速览

| 项目 | 当前值 |
|---|---|
| 正式版（日常使用） | `D:\software\Daozai` 仍跑 custom-129（exe `58432d96c07d9d0e`）。**custom-133 已打包、未部署（最新）**：`installer\daozai-setup-custom-133.exe`（11,241,513 B，包 sha `a9455e0ad655bb9f`，内含 exe `02786373d87ba563` + bridge `f1ae28626c9db18e`，构建自 main 工作树（告警清理 `2ea229a` + 液态玻璃按钮缩小 + 关配置弹窗收起 + INSTALL-NOTES 反馈日志说明，提交 `14bb37f`）= 在 custom-132 基础上：液态玻璃控制按钮略微缩小避开分页指示条、安装说明新增反馈日志清单；**custom-132 已打包、未部署**：：`installer\daozai-setup-custom-132.exe`（11,241,512 B，包 sha `6afb7ff8c6b5039d`，内含 exe `e342ccf062574afc` + bridge `1da2199b59ef5653`，构建自 main 工作树 = liquid_shadow 开关 + 液态玻璃参数弹窗（去重，仅 D3D12 兼容渲染开启时显示）+ 折叠三角指示 + phone-apps 移除内置种子）；**custom-131 已打包、未部署**：`installer\daozai-setup-custom-131.exe`（11,238,544 B，包 sha `c46e227a3eabf850`，内含 exe `74b854afddc88c5c` + bridge `1da2199b59ef5653`，构建自 `294d483` = 液态玻璃 DirectComposition 透明交换链 + 静置 20s 回收工作集 + 告警清理；**未部署，装前请先确认 DComp 路径在目标机可用**）；**custom-130 测试包（含上游合并）已生成未部署**：`installer\daozai-setup-custom-130.exe`（11,234,872 B，包 sha `bdb59ab5109d81e1`，内含 exe `5ebee4d9eb20c5fa` + bridge `5d81fa3a6f66703f`，构建自 merge-upstream-v0665 HEAD `2e4b64b` = 液态玻璃 + 上游 v0.6.65 + 全部定制 + 描述符堆优化；已覆盖 17:18 的合并前旧包，旧包 sha `7e469dd5db85cd7f` 见 git 历史）；默认安装 `C:\Program Files\Daozai`（admin），老 HKCU 卸载条目与 D:\software\Daozai 装完需手动清理；用户数据 `%APPDATA%\Daozai` 不受影响，装完需手动选液态玻璃皮肤；本轮 D3D12 轮廓裁剪修复只进预览（`liquid-test-46`），custom-130 包未重打 |
| 液态玻璃源码 | 分支 `directcomp-spike` 与 `main` 已于 2026-11-18 10:44 快进合并同步，HEAD = `21b55ee`（新增 liquid_shadow 岛体投影开关；该功能**尚不在任何安装包里**，custom-131 早于它） |
| **工作树** | **干净**；HEAD = `2342d3c`，检出分支 = `merge-v0666-selective`（v0.6.66 选择性合并，见 §7.0x）；`main` 仍在 `6a25af7`。`2342d3c` = 液态玻璃配置移入「外观」页 + 修「配置液态玻璃」按钮无反应 + 弹窗内切收起/展开态（见 §7.0aa），release exe `dfd191cc498aed8c8799c42ca822f4`（未部署，PR 用源码改动）。custom-133 包已生成（见 §7.0w，指纹被后续文案/合并更新，见 §7.0x）。|
| 预览版（液态玻璃试验） | 桌面预览目录 `daozai-liquid-setup.exe` = `liquid-test-60-skin-button`（包 sha `3c10d33e4567b5c9`，11,006,160 B；内含 exe `e2afdb7cb93dda40`；安装到 `%LOCALAPPDATA%\Programs\DaozaiLiquidTest`，独立数据目录），**已打包部署、待用户实机验证**。本轮微调：配置按钮夹进皮肤卡（仅液态皮肤显示）+ 下方标题「岛体交互」；上一版 -59（配置归位/修按钮/预览形态切换）与 -58 已移入 `_archive`。此前 -57 起 DirectComposition 透明交换链替代上游分层读回：DX12 下窗口无 GDI 表面 + DComp visual 交换链 + 强制预乘 alpha，桌面真实透出；不再套 `SetWindowRgn`，经典皮肤圆角与 OpenGL 一样平滑，实机四态验证正常 |
| **DirectComposition 试验（spike）** | 分支 `directcomp-spike`（= 预览目录那版）。vendor `egui-winit` / `egui-wgpu` / `wgpu-hal`：DX12 期间 `WS_EX_NOREDIRECTIONBITMAP` + DComp visual surface + 强制 `PreMultiplied` + 手动 `Commit`；按 HWND 认领主窗口。包 `liquid-test-55-dcomp-hwnd`（exe `6eb9cdad9a585a09`，包 `8b4a7d8c4fa11818`，10,983,849 B）；详见 §7.0s |
| 补丁链 | **已退役**（2026-11-17 归档，见红线区与 §2）；逐版历史 = `docs/PATCHES.md` |
| 测试 | 主线 1176 passed；最近 **1351 passed / 0 failed / 131 ignored**（2026-11-18 13:25，按钮缩小改动后全量串行 `cargo test --locked --release --lib -- --test-threads=1`）；**必须 `--test-threads=1`** |
| 单实例锁 | `Local\Daozai.SingleInstance` —— 正式版与预览版**不能同时运行** |
| GitHub 发布 | **Beta**：https://github.com/cayww/daozai-custom/releases/tag/liquid-test-18（tag 指向 `f415518`）；**本地新包 liquid-test-55 未发 GitHub**：`C:\Users\cay\Desktop\Daozai-液态玻璃预览\daozai-liquid-setup-55.exe`（= `daozai-liquid-setup.exe`），10,983,849 B，sha256 前16 `8b4a7d8c4fa11818`，内含 exe `6eb9cdad9a585a09`。-53 已移入 `_archive\` |

**一句话**：正式版停在 custom-129（源码锚点 tag `custom-129`）；`merge-upstream-v0665`
已吃进液态玻璃 + 上游 v0.6.65 + 全部定制（HEAD=`bcf44e8`），预览目录跑的是更进一步的 `directcomp-spike`（`9c91ebb`）。
补丁链已退役（2026-11-17），定制记录 = git 提交 + `docs/PATCHES.md`。
最新一轮用 DirectComposition 透明交换链替掉上游分层读回：黑框和圆角锯齿都解决，且液态玻璃不再被矩形画面盖住。

---

## 1. 红线（先看这个）

| 项目 | 规则 |
|---|---|
| `main` 分支 | 已含液态玻璃，**不要 reset 回 `4b0b427`**，不要把工作树丢弃 |
| 补丁链 | **已退役（2026-11-17，用户同意归档）**：`daozai-patches/` 已从工作区移除，历史在 git（`git log -- daozai-patches` 可找回）；正式版源码锚点 = tag `custom-129`（`4b0b427`），重建正式版 `git checkout custom-129`；上游快照移到 `_archive\dz-phone-upstream-快照.tar.gz` |
| 正式部署 `D:\software\Daozai` | **不要覆盖**，不要在正式目录里跑预览版 |
| `Cargo.lock` | **不要动**，加依赖会破坏 `--locked` 构建 |
| 用户数据 | 验证时不要真发邮件 / 标已读 / 改账户；`%APPDATA%\Daozai\settings.txt` 是用户的 |
| 工作日志 | `.workbuddy/memory/*.md` 是追加式，不要删、不要改写历史 |

---

## 2. 两条线的关系（2026-11-17 起合并为一条）

```
上游 xiaotang-daozai（remote: upstream）
        │
        └── git main = 上游 + 全部 custom 定制（custom-28…129 都是 git 提交，4b0b427=custom-129）
                 └ + 液态玻璃（feat/liquid-glass-a 快进合并，现为 31c7a27）
```

- **补丁链（daozai-patches/apply.py，custom-N EDITS 重放）已退役**：git 历史已完整
  覆盖全部定制，重放校验验证过与 `4b0b427` 一致；上游合并实际一直走 git merge
  （v0.6.64 合并 = `8bb61d8`，45 冲突人工解决）。
- **升级上游**：`git fetch upstream && git merge upstream/main`，冲突里对照
  `git diff custom-129..HEAD` 检查定制是否被冲掉。
- **重建正式版**：`git checkout custom-129`（= 无液态玻璃的 custom-129）。
- **新改动**：直接提交 main，不再需要 append_edits + 重放双份劳动。
- 逐版历史记录：`docs/PATCHES.md`（2963 行，留作历史文档）；逐轮过程复盘看
  `.workbuddy/memory/` 工作日志。

---

## 3. 路径与指纹

```
D:\code\daozai-custom\                       源码库（main = 液态玻璃已合并）
  dz-phone\src\                              Rust 源码
  dz-phone\src\gpu_glass.rs                  液态玻璃核心（WGSL shader + 抓屏 + 组帧）
  dz-phone\src\wgc.rs                        Windows Graphics Capture 封装
  docs\PATCHES.md                           逐版详细记录（2963 行，补丁链退役后留作历史）
  dz-phone\CHANGELOG.md                      面向用户的中文更新说明
  merge-work\                                测试隔离数据目录 / 重建脚本 / 旧交接备份
  installer\                                 Inno Setup 工程与安装包

D:\software\Daozai\                          正式版部署目录（custom-129）
C:\Users\cay\Desktop\Daozai-液态玻璃预览\      预览版（液态玻璃）
  daozai.exe                                 预览二进制（独立数据目录）
  启动（独立数据目录）.bat                     实时模式（默认；截图看不到岛仔）
  启动（冻结背景·可截图）.bat                  冻结背景，截图/录屏可见
  启动（棋盘诊断·看折射）.bat                  网格诊断，看折射位移/色差
  说明.txt                                    给用户看的完整说明
  data\diagnostics\liquid-glass.log          诊断日志
  _archive\                                  旧交接文档备份（勿删）
C:\Users\cay\.workbuddy\skills\daozai-custom-build\   本项目的 skill（脚本在 scripts\）
```

---

## 4. 目录

- [5. 主线工作流（补丁链已退役）](#5-主线工作流补丁链已退役2026-11-17)
- [6. 液态玻璃：架构与硬约束](#6-液态玻璃架构与硬约束)
- [7. 待办（接手先做这个）](#7-待办接手先做这个)
- [8. 版本时间线](#8-版本时间线)
- [9. 接手注意（操作层面）](#9-接手注意操作层面)
- [附录 A：常用命令速查](#附录-a常用命令速查)
- [附录 B：调试开关](#附录-b调试开关)

---

## 5. 主线工作流（补丁链已退役，2026-11-17）

> 5.1/5.2/5.4 的补丁链流程已随链退役作废，历史全文见 git（`git log -- daozai-patches`）。
> 现行流程：改源码 → `cargo build --release --locked` → 全量测试 → 提交 main → 部署。
> 正式版重建 `git checkout custom-129`；升级上游 `git merge upstream/main`。

### 5.3 构建 / 测试 / 打包

```bash
# 编译（两个 bin 都要，`cargo test` 不编译 main.rs）
cd /d/code/daozai-custom
MSYS_NO_PATHCONV=1 cargo build --release --locked --manifest-path dz-phone/Cargo.toml \
  --bin daozai --bin daozai-bridge

# 全量测试（必须串行 + 数据目录隔离，约 5 分钟）
MSYS_NO_PATHCONV=1 DAOZAI_DATA_DIR=D:/code/daozai-custom/merge-work/test-data \
  cargo test --manifest-path dz-phone/Cargo.toml --locked --release --lib -- --test-threads=1

# 打包（Inno Setup 7 在 D:\software\Inno Setup 7）
cd installer/<对应工程> && MSYS_NO_PATHCONV=1 "/d/software/Inno Setup 7/ISCC.exe" <工程>.iss
```

**静默安装必须这样写**（Git Bash 会把 `/VERYSILENT` 当 POSIX 路径转换掉，安装器会弹向导等人点）：

```bash
MSYS_NO_PATHCONV=1 "<setup.exe>" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

### 5.4 交付清单（缺一不可）

| 文件 | 内容 |
|---|---|
| ~~`daozai-patches/PATCHES.md`~~ | 补丁链已退役，此清单作废；现行交付 = 改源码 → 构建 → 测试 → 提交 main → 部署 + HANDOFF/工作日志 |
| `dz-phone/CHANGELOG.md` | 面向用户的一条中文说明 |
| `~/Desktop/HANDOFF.md` | 标记 / EDITS 数 / 测试数 / 新章节 / 时间线 |
| `memory/YYYY-MM-DD.md` | 追加式工作日志，含踩坑与实证 |

另外：两处 `CUSTOM_BUILD.txt`（工作副本 + 部署目录）改成新的 `custom-N`；报告里给 exe sha256 前 16 位。

---

## 6. 液态玻璃：架构与硬约束

### 6.1 八条硬约束（改代码前必读）

> 这套「抓屏 → 显示 → 再抓屏」架构的地基。违反任一条立刻看到鬼影/黑边/条纹。

1. **窗口底是不透明黑，`alpha` 不能为 0**。兼容渲染（D3D12/wgpu）下形状靠 `SetWindowRgn` 裁，
   靠 alpha=0 透背景的写法在黑底上就是黑。
2. **边缘必须和背后的桌面 mix**：`return mix(edge_bg, col, cov)`、`alpha = 1`。
   用 `col * alpha` 会在轮廓外留一圈黑线。
3. **`SetWindowRgn` 是每像素硬裁、无抗锯齿**：GPU 形状必须比窗口小（`inset = 3.0px`），
   否则半透明抗锯齿像素被切掉 → 阶梯锯齿。
4. **白边必须有独立覆盖率且不能太窄**：`cov = max(aa, line)`；白边用高斯核
   `exp(-d²/3.61)`，不要用 1.6px 的 `smoothstep`（1px 采样下只有 2 个点 → 硬线）。
5. **`WDA_EXCLUDEFROMCAPTURE` 必须每帧刷新**。`apply_style` 和 `SetWindowRgn` 都会重置
   display affinity；降频会正反馈成满屏竖条纹。**绝对不能降频。**
6. **折射方向朝玻璃内部**（`uv - g*k*strength`）。朝外会把岛体外壁纸折进来，
   就是「岛内出现不属于该区域的壁纸像素块」。
7. **不能用「暗像素比例」判污染帧**。深色壁纸下会全判成污染 → 底片永不更新。
   改用平均亮度：只在 `inner<45 && outer-inner>80` 时丢帧。
8. **重绘必须主动驱动**。`vsync` 关闭时 egui 只靠输入事件重绘，鼠标不动就冻结。
   `Skin::Liquid` 时每帧 `request_repaint_after`。

### 6.2 每帧链路

```
App::frame
  ├─ request_repaint_after(16.7 - 净工作时间)     ← 48fps 的修法见 6.4 坑②
  ├─ background_luma → liquid_k（0.52/0.66 滞回）
  ├─ progress 变化率 → glass_morph（逐帧 ×0.74 衰减）
  └─ paint_gpu_glass → GlassCallback::prepare
       ├─ enable_capture_exclusion()   每帧检查 affinity，丢了才重设
       ├─ capture(capture_rect, body)
       │    ├─ wgc::grab → Frame / NoFrame / Unavailable
       │    ├─ 判污染（inner<45 && outer-inner>80 → 丢帧）
       │    └─ compose_with_plate（滚动底片 + 快路径）
       ├─ queue.write_texture（bytes_per_row 256 对齐）
       └─ uniforms → 432 字节 UBO（含 uR[10]/uS[10] 控件槽）
```

### 6.3 坐标链路（最容易被误解）

- **视口 = callback 的 `capture` 矩形**，着色器里 `p = uv * tex` 直接假设「视口尺寸 == capture 尺寸」。
- `bounds` 是岛体在 capture 内的物理像素坐标。
- **uniform 里的 tex 必须和视口尺寸一致**，不一致就出现「缩小的岛仔套在大轮廓里」或背景拉伸。
- 所以多控件玻璃要**一次抓屏、一次 pass 循环多块矩形**，不要每个控件一个 callback。

### 6.4 液态玻璃踩过的坑

| # | 坑 | 结论 |
|---|---|---|
| ① | GDI BitBlt 不遵守 WDA 排除 | `Grab::NoFrame` 只能 `return None` 复用旧纹理，**永远不要退回 GDI 抓一帧**（会抓到自己 → 竖条纹） |
| ② | 48fps 的真凶 | `request_repaint_after(16ms)` 从当前时刻算，叠加 4.8ms 净工作 → 20.8ms。要用上帧周期反推 `work_ms`，从 16.7 里扣掉。**「瓶颈在 EnumWindows + WGC Map」的旧推测是错的** |
| ③ | 测 fps 虚高 | 鼠标一动 egui 额外重绘，能虚高到 72fps。必须先把光标停远处、确认静止再读 `perf: tick=` |
| ④ | 测试必须单线程 | 多线程会挂 4~5 个（QQ/微信/钉钉窗口观察 + `quota::doubao::*` 共用临时目录） |
| ⑤ | morph 的 `band` 不能大 | 系数 1.55 会让折射带覆盖整块面板 → 像内容往下滑。band 只给 0.08，主要加 `strength` |
| ⑥ | 底片不能无条件填渐变 | 「非冻结模式就 fill_gradient」会把干净帧抹成竖直渐变 → 冻结截图永远是假背景 |
| ⑦ | Beer-Lambert 要乘在背景上 | 加在导光上等于把颜色加在光最暗处，肉眼看不见 |
| ⑧ | 磨砂挡不住后画的文字 | GPU pass 在 egui 内容之前跑，浮层类控件盖不住之后画的歌词 → 音量浮层改走 egui |
| ⑨ | 控件材质必须和岛体一字不差 | 给控件更轻的压暗系数会变成「一块白纸」，层次只能由边缘（白线 + 倒角暗带 + 导光）表达 |
| ⑩ | 控件矩形越界必须丢弃 | 着色器取样会 clamp 到边缘像素，越界矩形拉出色带（`frame_for` 里 `capture.contains_rect` 过滤，有单测） |
| ⑪ | 控件不要沿用岛体的 3px 内缩 | 那是为 `SetWindowRgn` 硬裁准备的；控件只留 0.5px |
| ⑫ | 浅色皮肤的换色会把压暗变提亮 | 进度条底槽保持中性灰（`glass_groove` 留着给竖向滑槽用） |
| ⑬ | **`request_repaint_after` 不能每帧调** | egui 的 delay 从「现在」算，而请求到帧末才被处理、已扣掉一帧 → 实际等待 = 设定值 − 帧周期，**正反馈**。设 30.3ms 实测 trace 里 delay 恒 13.6ms（=30.3−16.7），帧率锁死 58fps。**对照实验**：完全不调才能降到 20.9fps。正确做法是只写「目标档位」，由主循环唯一出口 `schedule_repaint` 调度（它补 `predicted_dt`） |
| ⑭ | **帧率锁 58fps 别怀疑 egui 版本** | 本项目 0.31.1 早已含 `#3978`（修 `#3109`）；但仍缺 `#8398`——`check_redraw_requests` 消费完到期项不撤销 `ControlFlow::Poll`，事件循环忙等。vendor 里已按上游思路打补丁（删 Poll、统一 `WaitUntil/Wait`） |
| ⑮ | **内存 5 倍差是跑错显卡** | 诊断日志 `渲染器：AMD Radeon / device_type=IntegratedGpu`。egui-wgpu 默认虽是 `HighPerformance`，混合显卡机器仍会落核显；核显无独立显存、纹理与帧缓冲全走系统内存 → 548MB（独显 257MB）。启动 3s 即 548MB 且不增长，**不是泄漏**。修法是 `native_adapter_selector` 显式挑 DiscreteGpu，并用 `is_surface_supported` 过滤防黑屏 |
| ⑯ | **拓扑检测不能只写在抓屏路径里** | 「仅主屏 → 扩展」时用户多半先**收起岛仔再切**，而收起态不抓屏 → 检测整段错过（日志证据：切换后只剩 `perf:`，没有 `capture:`）。必须由 `App::frame` 每帧调 `poll_topology`。另外只比「矩形中心属于哪块屏」也挡不住这类切换（主屏 rect 没变），要算**全桌面 rect 签名** |
| ⑰ | **稳定期要回退纯色，不能沿用旧纹理** | 拓扑变化后 `capture` 返回 `None`，如果上层继续画上一帧纹理，看到的就是旧拓扑画面 → 花屏且**不会自愈**。`paint_body` 与 `ctl_glass_ready` 都要在 `settling()` 期间回退纯色/矢量。切换时桌面有 1s 左右「向内收拢→发黑→复原」动画，窗口取 2s |
| ⑱ | **检测到变化必须「就地登记」** | `poll_topology` 被 `App::frame` 调用时检测到变化、打了 sig 日志，但**返回值被丢弃**；真正设置稳定期的代码在 `grab` 里，而那一帧 `grab` 又被 100ms 降频跳过 → **稳定期从未生效过**（`f0cc409` 的修复因此完全没起作用）。必须让 `poll_topology` 自己完成登记（原子标志 `NEED_RESET` + `EPOCH++` + 设窗口），`grab` 只负责消费标志去 drop 旧会话。**不能**在它内部直接拿 `WGC` 锁（调用它的 `grab` 可能正持锁 → 死锁） |
| ⑲ | **脏底片要丢弃重试，不能急着存** | 切换瞬间抓到底片 `inner=0`（全黑）或亮度只有周围 62% 以下，就是过渡态。旧保护要求 `inner<45`，黑帧的 `ring` 也是 0 时相对阈值算不出来 → 黑底片存下，玻璃**永久发黑**。判定可疑时先 `return None` 丢弃（60 帧 ≈1s 窗口），让过渡播完再抓 |
| ⑳ | **动态壁纸比系统动画晚 1~2s** | 只在小屏开动态壁纸、另一屏是 Windows 静态壁纸时，壁纸引擎切完显示模式要重新渲染，比系统过渡动画晚 1~2s 才画好。**不要用加长等待解决**（`1ee5795` 把重试拉到 3s，结果最坏纯色 5.5s 且仍不保证）——正确做法是**底片定期刷新**：换代后 5s 内每 500ms 强制重建，壁纸一画好就被顶上（`b39e3a8`） |
| ㉑ | **自适应学习值只认第一张干净底片** | 5s 刷新窗口里底片会反复重建，若每次都记「稳定耗时」并学习，耗时越累越大，学习值会被滚到 5 秒上限（越学越慢）。必须 `topo_changed_at.take()` 只取一次 |
| ㉒ | **主题采样框不能跟着岛体尺寸走** | 原来用「抓屏矩形减岛体」那圈像素判深浅。各页展开高度不同（音乐 140 / 视频 176 / 日历 216 / 天气 232），一翻页圈里就换了一批壁纸 → 深浅材质被重新判定。实测日志：收起 inner=147/outer=149（亮度 0.58）稳定不动，但翻页时圈的下沿往下挪 70~90px 就够跨过 0.52 阈值。改成 `theme_rect()`：以**岛体上缘中心**为锚点、固定 520×120 CSS px，翻页/换页/展开高度变化都不动它。顺带把「收起→展开再判一次」也消掉了 |
| ㉓ | **玻璃控件的白边是材质，不能进皮肤换色** | 矢量回退版 `glass_chip/disc/groove` 的白边画的是「白」，浅材质调色板把白映射成墨色 → 翻页过渡（`ctl_glass_ready` 关掉的那半秒）控件变成**黑边**，稳定后 GPU 玻璃又是白边 → 每次翻页黑白跳一次。用户会以为「主题色又被重新判定了一遍」。正解是 `Palette::glass_chrome`（只液态浅材质开）+ `tone_since(keep_glass_chrome)`：玻璃材质原样保留，只有字色/图标色跟深浅走。**别用「给控件更浅的压暗」这类改法**（见坑 ⑨） |
| ㉔ | **GPU 透镜没有透明度通道，不能跟淡出** | 翻页过渡想让控件也走 GPU 玻璃时，旧页（快速淡出的那层）绝不能直接给透镜——`gpu_glass::Ctl` 没有 alpha，透镜会以全强度留在原地不跟 `old_a` 淡出，变成一颗不消失的空玻璃。正解：旧页渲染期置 `old_page_render`，`ctl_on_gpu` 对它恒 false → 旧页控件回矢量玻璃（白边由坑 ㉓ 的 `glass_chrome` 保住）随卡片一起淡出；新页透镜在 `glass_controls` 里随 `page_slide_tx()`（= `slide.pos`，条件与 body 的 in_tx 严格一致）平移。齿轮/翻页箭头是全局 chrome 不参与滑移，别跟着平移 |

### 6.5 控件玻璃现状（`bf116c2`）

- uniform **544 字节**：`crect[13]` / `cprm[13]`（圆角, 启用, 放大倍数, 折射倍数）+ `notch`（flare, flare_h, 是否刘海）。
- 上 GPU 的 12 块：`Gear / Lyric / Prev / Play / Next / Volume / IslandLyrics / Seek / Focus / Rest / NavL / NavR`，
  一次抓屏一次 pass；`ctl_geom()` 是唯一几何来源。
- **视频页也上玻璃**（本轮）：`ctl_geom` 的 `media` 条件去掉 `!video_page()`——Lyric(打开位)/Prev/Play/Next/Seek
  在浏览器媒体页同样走玻璃；`Volume`/`IslandLyrics` 仍只限音乐页（视频页没有这两个控件）。
- 圆钮按 LiquidGlass-Pack-v2：未拖 28×20 浅玻璃胶囊 `mag=1`；拖动时尺寸与 `mag` 同步 ×1.9（放大镜）。
  拖动跟手靠新增的 `music_seek_drag` 实时通道（`music_seek_ui` 只在松手时写）。
- 图标比上一轮小一档（玻璃圆盘外圈已有描边）：播放 15→12.5、切歌 12→10、音量 ×0.8。
- 刘海形状：`notch_sdf` 复刻 `shape::sections` 的肩部几何（`wing=flare*(1-t)^2.4`、底部圆角、顶部直角），
  非刘海形状仍走 `rr_sdf`，用 `select` 切换。
- 折射倍数按短边缩放 `clamp(short/70, 0.30, 1.0)`（27px 圆片 0.39）——以前所有控件共用 0.55 折，
  小圆片会像「挖了个洞」。
- 弹簧：入场 k=320 ζ=0.55，按压 k=300 ζ=0.5（胀到 1.14 后过冲 1.16）；
  在 `update_ctl_springs()` 里、**paint_body 之前**更新，玻璃和图标共用 `ctl_scale()`。
- 门槛 `ctl_glass_ready()`：液态皮肤 + 完全展开（progress ≥ 0.985）+ reveal ≥ 0.6 +
  不在页切换过渡 + 无二级页/弹窗（过渡期内容平移 ±18px，玻璃会错位）。
- 没有 wgpu 设备时回退 `paint::glass_disc / glass_chip`。

### 6.6 主题与帧率（本轮新增）

**主题随壁纸明暗自动切换**：`liquid_k`（1=浅材质/深墨字，0=深材质/白字）由 `background_luma()` 驱动，
0.52/0.66 之间是滞回区防抖，再走指数平滑。`skin_palette` 据此在 `GLASS_LIGHT_READABLE`
（新增：白底近黑字，`lift` 0.45 让辅助灰字也够深）与 `GLASS_DARK` 之间切换——**手动浅色玻璃皮肤仍用
`GLASS_LIGHT`，两者不混**。零影响：`theme::WHITE/GRAY` 常量一行未动。

**主题判定采样框（`37e965d`）**：`gpu_glass::theme_rect()` 以岛体上缘中心为锚点取固定
520×120 CSS px（抓屏矮时自动裁到抓屏内），用 `mean_luma` 求平均亮度 → `BG_LUMA`。
只在 **WGC 干净帧**上更新（黑岛形帧、GDI 兜底帧、冻结截图模式全部跳过，宁可沿用旧值），
变化超过 0.06 才写 `theme: luma=0.58→0.22 框=[x,y wxh] 岛体高=245` 这条日志。
`liquid_k` 仍然是 0.52/0.66 滞回 + 指数平滑，不用动。

**帧率档位**：`App.glass_tick` 每帧写（1=60fps 动画/悬停/拖动、2=30fps 静止、0=不干预其它皮肤），
`out.repaint_after` 消费；液态玻璃**不能**用其它皮肤的「静止 1 秒」（背景要持续抓屏，停了会冻住）。
实测收起静止 30.4fps / CPU 13.2%，悬停 58.1fps。

**桌面拓扑切换（-30~-33 四轮才收住）**：链路是
`App::frame` 每帧 → `wgc::poll_topology()`（~100ms 降频，EnumDisplayMonitors 收全部 rect 排序后 FNV）
→ 变化则内部登记 `NEED_RESET` + `EPOCH++` + 清 `BOUND_MONITOR` + 设稳定窗口
→ `grab` 消费 `NEED_RESET` drop 旧会话
→ `settling()` 期间 `capture` 不出帧、`paint_body` 回退纯色、`ctl_glass_ready` 返回 false
→ 稳定后 `compose_with_plate` 建底片，可疑（全黑/低于周围 62%）就丢弃重试（60 帧）
→ `EPOCH` 变化触发**底片刷新窗口**：5s 内每 500ms 重建（对付晚到的动态壁纸）
→ 第一张干净底片建成时学习「实际稳定耗时 ×0.85」，落盘 `diagnostics/settle-learned.txt`，下次起步直接用它。

**稳定期参数**：`DAOZAI_SETTLE_MS`（环境变量，最高优先级）> 学习值 > 500ms 首次默认；范围 400~6000ms。
手动改默认值意义不大——每台机器的「过渡动画 + 壁纸重绘」耗时不同，交给自学习更准。

**显卡选择**：`Settings::liquid_prefer_dgpu`（默认开，仅液态玻璃生效）。启动期在 `main.rs` 建窗前独立
`Settings::load()`，满足条件才设 `wgpu_setup`；非液态皮肤或关掉时**完全不碰 wgpu 默认策略**。
UI 文案是简版（用户 `bf116c2` 定的），实测数据只留在本文档。

---

## 7. 待办（接手先做这个）

### 7.0 本轮已完成（2026-11-16 19:00–22:30，`5f5b428`→`bf116c2`）

| 主题 | 内容 | 提交 |
|---|---|---|
| 主题自适应 | `liquid_k` 驱动 `GLASS_DARK`↔`GLASS_LIGHT_READABLE`，滞回 0.52/0.66 + 平滑；`theme` 常量零改动 | `5f5b428` |
| 刘海形状 | GPU 玻璃新增 `notch` 通道 + `notch_sdf`，顶部肩部与 `shape::sections` 同几何 | `5f5b428` |
| 视频页玻璃化 | `ctl_geom` 的 `media` 条件去掉 `!video_page()`，进度条/切歌/播放/打开全上玻璃 | `5f5b428` |
| 圆钮 v2 | 未拖 28×20 胶囊 `mag=1`、拖动 ×1.9 放大镜；`music_seek_drag` 实时通道保证跟手 | `5f5b428` |
| 图标缩小 | 播放 15→12.5、切歌 12→10、音量 ×0.8 | `5f5b428` |
| 结构清理 | `paint::skip/speaker` 合并为单一实现（原包装已 0 调用）；`liquid_k` 收回私有 + `#[cfg(test)]` setter | `5f5b428` |
| 帧率（失败记录） | 先误判为 wgpu 抓屏回调锁定，整体回滚 | `c3e9b9b` |
| **帧率（真修）** | 真因是每帧调 `request_repaint_after` 自激 + vendor `Poll` 残留；改为 `glass_tick` + 主循环统一调度 → **静止 30fps** | `cf76bbb` |
| **内存** | 定位到跑核显，新增 `liquid_prefer_dgpu`（默认开，仅液态玻璃）→ **548MB→257MB** | `3ddcb30` |
| 文案 | 独显设置项文案由用户简化为「优先使用独显渲染玻璃，切换后重启岛仔生效」 | `bf116c2` |
| 交接文档 | 纳入仓库 `docs/HANDOFF.md` + README 挂入口；桌面/_archive 三处同步 | `2c199f1` |
| **显示器拓扑（1/4）** | 全桌面 rect 签名检测 + 每帧调 + 稳定期回退纯色 —— 方向对但**未生效**（见坑 ⑱） | `f0cc409` |
| **显示器拓扑（2/4）** | 真正修好：重置登记就地完成 + 全黑底片保护，`inner=0` 不再被当底片 | `a1a2669` |
| 显示器拓扑（3/4） | 试过「加长重试窗口到 3s」——**方向错误已作废**，最坏纯色 5.5s | `1ee5795` |
| **显示器拓扑（4/4）** | 改为「底片定期刷新」+ 稳定期自适应学习（落盘），残留 ≤0.5s | `b39e3a8` / `8d93c76` |

**验证**：1186 passed / 0 failed / 119 ignored；fps 探针实测（不是只看 CPU 均值——CPU 的 ±2 点方差会骗人）；
独显开关三态实测（开=257MB/NVIDIA、关=544MB/AMD、无键=默认开）。

### 7.0b 本轮已完成（2026-11-17 01:00–01:10，`37e965d`）

| 问题 | 根因 | 修法 |
|---|---|---|
| 每翻一页控件白边就「黑一次再白回来」（用户原话：既然背景没变，为什么不沿用之前的主题色） | ①主题采样用「抓屏矩形减岛体」那圈，翻页时页面高度变化 → 圈换了一批壁纸 → 深浅材质重判；②翻页过渡期 `ctl_glass_ready` 关掉，控件退回矢量版，而矢量版的白边被浅材质调色板映射成墨色 | `theme_rect()` 固定 520×120 CSS px、上缘锚定；`Palette::glass_chrome` + `tone_since(keep_glass_chrome)` 让玻璃材质不进换色 |

**验证**：
- 单测：`theme_rect_is_immune_to_page_height`（音乐 140 vs 天气 232 高度 → 采样框逐项相等）、
  `glass_chrome_keeps_its_white_edge_in_light_material`（白边/凹槽原样、字色仍映射成墨色、其它皮肤行为不变）。
- 实机（实时模式，立刻 5 次翻页）：日志 `theme:` 行 **0 条**——翻页不再触发深浅重判；
  之前收起→展开会记两条 `luma=0.85→0.58/0.22`。
- 实机（冻结截图 + `DAOZAI_LIQUID_FORCE_LUMA=0.9` 强制浅材质）：翻页过渡帧的分段按钮描边
  从原来的深灰（max 亮度 ≈135）变成白线（255），和稳定态一致。
- 全量测试 **1189 passed / 0 failed / 119 ignored**（`--test-threads=1`）。

→ 已在 7.0c 完成。

### 7.0c 本轮已完成（2026-11-17 01:15–01:50，翻页过渡也走 GPU 玻璃）

| 问题 | 根因 | 修法 |
|---|---|---|
| 过渡期控件质感差一档（矢量片没有折射/压暗，翻一次页「玻璃→贴纸→玻璃」） | `ctl_glass_ready` 在 `page_e()<0.55` 时放行矢量；且翻页引发面板高度弹簧收敛，`progress()<0.985` 也会挡 | `ctl_glass_ready` 放行翻页（`flip_glass_until=t+1.2` 盖住滑入+收敛期）；新页控件透镜在 `glass_controls` 里随 `page_slide_tx()` 平移对齐；旧页见坑 ㉔（矢量淡出） |

**验证**：
- 全量 1189 passed / 0 failed / 119 ignored。
- 棋盘格诊断（`DAOZAI_LIQUID_FLAT=2` + 冻结截图）：翻页中段帧里专注/休息分段按钮已带透镜边界（内部棋盘格被折射位移），透镜位置跟随内容滑移。
- 音乐播放页（用户实测在播）：翻回音乐页的过渡帧里媒体圆片带玻璃滑入、进度条圆钮是折射透镜（无矢量实心白点）；翻离时旧页控件随淡出保持白边玻璃。全程无黑边。
- 安装包 liquid-test-35-transition-glass（exe `68b119a6871d3965` / setup `faa45512144a49be`）。

### 7.0d 本轮已完成（2026-11-17 01:55–02:40，`f99ce67`，liquid-test-36）

用户点名：额度页「添加账号」、日历页「＋ 新增」、日程编辑器里的控件，「能液态的都要液态玻璃」。

- `Ctl` 新增 11 变体（CTL_N 12→23）：`QuotaAdd / CalAdd / CalAllDay / CalRem0..4 / CalDelete / CalCancel / CalSave`。同时可见数仍 ≤ `MAX_CTL=13`（编辑器页 9、音乐页 10）——**不动着色器**。
- 提醒分段数量/宽度随「全天」动态变（2 个 126 宽 ↔ 5 个 54 宽）：固定 5 槽，多余槽位几何返回 None。
- 编辑器整页替换 panel_body：齿轮/翻页箭头在 `ctl_geom` 里同步消失（否则出现无图标空透镜）；
  `ctl_glass_ready` 放行 draft，但 `glass_controls` 的翻页滑移对编辑器关闭（tx 恒 0）。
- 绘制侧沿用专注页分段按钮模式：GPU 分支只画命中区/悬停反馈/选中态淡填充/文字，
  保存按钮叠 `paint::alpha(accent,0.42)` 淡蓝强调；未上 GPU 时退回原实心胶囊（非液态皮肤不受影响）。
- 验证：1189 passed；FLAT=2 棋盘格实拍（额度页/日历页/编辑器/点全天后的动态分段）全部带透镜边界。

**坑**：验证脚本翻页时额度/天气页 arrow_y=88（其他页 68），固定 y 点击会漏；且双击步进会
跳过中间页——逐页单步截图定位后再交互。编辑器打开时岛仔可能因 hover/forced_open 过期收起，
跨脚本续测不可靠，全流程要在同一脚本里跑完。

### 7.0e 本轮已完成（2026-11-17 02:45–03:35，`9a0efc6`，liquid-test-37）

**邮件页**：「全部/未读」按 Pack-v2 demo 的 tab 重做——单块滑动指示器透镜（`Ctl::MailTab`，
弹簧 300/0.5，滑动中横向拉伸 `1+min(0.5,|v|*0.06)`、纵向略压，按压果冻 ×1.3）；
「全部账号」chip 上 GPU 透镜（`Ctl::MailAccount`，收件箱/回收站共用坐标）。

**修「选中淡填充被压成暗雾」**（用户报：新建日程提醒横排看不清）：GPU 分支的 white_alpha 
淡填充 premultiplied 后亮度极低，浅材质通用换色把它当深色压成墨雾埋掉文字。修法：
`chrome_mark/chrome_since` 改 pub(crate)，所有 GPU 分支的选中/悬停淡填充登记为玻璃材质
（编辑器全天/提醒分段、专注页、额度/日历新增）；编辑器选中态文字改用强调色。

- Ctl +2（CTL_N 23→25）；MailTab 几何含弹簧位移与形变，段中心 `list_x+26+56*pos` 与文字逐像素对齐；
  回收站/详情页 geom 返回 None。
- 验证：1189 passed；实拍连拍确认滑动+拉伸+停稳对齐；提醒横排文字全部清晰。

### 7.0f 本轮已完成（2026-11-17 09:25–09:50，`d63e0ca`，liquid-test-38）

用户反馈「音乐进度条圆钮弹簧效果不强」→ 新增**「弹簧强度」设置项**（液态玻璃参数区，
0.2–3.0，默认 1.0，立即生效）：update_ctl_springs 每帧按倍率 tune 各弹簧刚度
（按压 300 / 入场 320 / 圆钮放大 1000 / 邮件指示器 300，阻尼比不变），按压果冻幅度同乘。
**圆钮此前完全不参与按压果冻**（只做拖动放大）——现叠一半幅度的果冻
（按住进度条时圆钮回弹，峰值 ≈+27%@3.0），拖动放大不受影响。
验证：1189 passed；liquid_spring=3.0 实拍圆钮弹大→回缩→再弹→稳定。

### 7.0g 本轮已完成（2026-11-17 09:55–10:10，`8316e1e`，liquid-test-39）

- 「弹簧强度」改名**「弹簧刚度」**（存储键 liquid_spring 不变）；新增**「阻尼比」**liquid_zeta
  （0.1–1.0，默认 0.5）：按压/入场/邮件指示器弹簧的 ζ 统一可调（原 0.5/0.55/0.5 写死），
  小=来回弹跳更 Q 弹，1.0=不弹跳直接到位；圆钮拖动放大弹簧保持 ζ=1（干脆不过冲）。
- 设置页顺序：展开动效、悬停触发延时移到液态玻璃参数区**上方**（hint 之上），
  所有皮肤可见性不变。
- 验证：1189 passed；设置页实拍确认顺序与两个滑杆。

### 7.0h 本轮已完成（2026-11-17 10:05–11:05，`dabc21b`，liquid-test-40）

用户要做内存优化 → 新增**内存剖析基建**：`--features memprof`（dhat）+ `--profile memprof`
（带行号符号表）。`DAOZAI_MEMPROF_EXIT=<秒>` 到点自动优雅退出，报告写
`data/diagnostics/dhat-heap.json`。**三个坑**：① Profiler 不 Drop 没报告（taskkill /F 拿不到，
要 ViewportCommand::Close 优雅退出）；② 必须装 `dhat::Alloc` 全局分配器（否则 tg=0 空报告）；
③ release 的 strip=true 让 Rust 帧符号化全失败——要用带符号的 memprof profile + pdb 放 exe 旁边。
**实测结论（80s 会话）**：总分配 19.5GB（egui 每帧 ~4.5MB，框架固有），退出存活堆仅 9.1MB
（无泄漏），提交 622MB 的大头在 wgpu/驱动预留而非 Rust 堆。
**已优化**：CalDAV XML 的 9 个正则从每次调用重编译收敛到 re_cache() LazyLock 缓存
（单次同步临时分配 ~30MB → 编译一次）。
- 验证：1189 passed。

### 7.0i 本轮已完成（2026-11-17 11:30–13:10，`80d1c90`，liquid-test-41）

功能级降内存两项（依据：驱动池随抓屏+玻璃管线命令吞吐增长，降帧/休眠从源头限制）：

- **静止档 15fps**（glass_tick=3）：无动画/环境动效/歌词滚动时降半帧，有动静回升 30fps。
- **收起深度休眠**：收起+鼠标远离（24px 余量）+无动画/媒体/通知/吐司持续 2 分钟 →
  整条管线 1Hz；输入事件即时唤醒，媒体/通知变化 ≤1s 内回升。宠物不进门槛——
  pet_state 用系统级空闲判定（人用电脑就永远醒着），深休眠期间宠物动画冻结在
  入睡前姿势（pet_anim_t）。挡因诊断写 pill-sleep.log（10s 限频）。
- pill_still 抽纯函数 + 4 单测；1193 passed。
- 实测：collapsed idle 30fps（120s 计满）→ 1Hz（20s 窗口 tick 冻结），悬停唤醒
  立即回展开态。
- MemoryHints A/B（上轮遗留结论）：MemoryUsage 比 Performance 多 ~85MB 私有提交
  （egui 流式缓冲精确分配产生大量 D3D 对象），默认保持 Performance，
  实验开关 DAOZAI_WGPU_MEMHINTS=memory_usage（`6a2bd1b`）。

### 7.0j 本轮已完成（2026-11-17 13:14–13:40，`68df96d`，liquid-test-42-mailglass）

用户报邮箱页空态两个问题，都修了：

- **无邮箱时「全部/未读」透镜残留**：`Ctl::MailTab` 的 `ctl_geom` 没挡空态——CPU 侧
  page_mail 空态提前 return 不画文字，GPU 着色器却照常把指示器透镜画在
  (list_x+26, 42.5)，剩一片悬浮玻璃。修法：geom 守卫补 `|| self.mail.accounts.is_empty()`
  （与 `Ctl::MailAccount` 同款）。
- **「去添加」按钮上真玻璃 + 文字居中**：原先只有 CPU 矢量 `glass_capsule`（无折射），
  且文字画在 x=58（胶囊跨 42..126，中心是 84）。修法：新增 `Ctl::MailSetup`
  （CTL_N 25→26，geom=42,88,84×26 圆角 13，仅 page5 空态出现），pages.rs 按
  QuotaAdd 模式分 GPU/矢量/实心三路，文字改画 84。
- 验证：1193 passed / 0 failed / 119 ignored（--test-threads=1）；冻结背景模式实拍
  邮箱页空态——透镜消失、按钮玻璃质感、文字居中。实时模式已由用户日常使用验证。
- 部署：预览目录 exe 已替换（备份 `daozai.exe.prev-mailglass`），标记
  `liquid-test-42-mailglass`，exe sha256 前 16 = `a701a6bf12110dfd`。
- **顺手修文档**：发现 §7.0e–7.1 之间整段被上一轮写坏（每行被 `'` 包裹的 repr 伪影），
  已按规则剥离还原（`'` 游离行删、`     '` 前缀与行尾 `'` 剥）。

### 7.0k 本轮已完成（2026-11-17 13:55–14:25，`f3d493b`，liquid-test-43-idlefps）

用户反馈：静止降帧/深休眠没省到内存（大头在 Direct3D 12 抓屏管线），要求做成可配置项。
新增两个设置（常规 → 岛体外观，阻尼比之下）：

- **静止刷新率** `liquid_idle_fps`（默认 15，滑杆 1–30）：glass_tick==3 的重绘节拍，
  `repaint_after=1.0/fps`；30 = 静止不降档。立即生效。
- **收起深度休眠** `liquid_deep_sleep`（默认开）：关掉后静止态保持「静止刷新率」，
  不再进 1Hz；deep_sleep 判定加 `&& self.settings.liquid_deep_sleep`。

持久化走 settings.txt 新键 `liquid_idle_fps` / `liquid_deep_sleep`（load clamp 1–30），
设置页 Debug-diff 自动保存覆盖。验证：1193 passed / 0 failed / 119 ignored；
冻结模式实拍常规页——两个新条目完整可见、默认值正确。部署标记
`liquid-test-43-idlefps`，exe sha256 前 16 = `ba5d20976e277fbc`（备份
`daozai.exe.prev-idlefps`）。**未打包安装包**（42 号包仍有效，43 号待用户发话）。

### 7.0l 本轮已完成（2026-11-17 14:35–15:00，`31c7a27`，liquid-test-44-memopt）

用户报：D3D12 不开玻璃 ~160MB、开玻璃 ~210MB（GDI 路径 ~50MB），问社区有没有办法。
调研 + A/B 实测两项落地（**未打包**）：

- **wgpu `MemoryHints::Manual`（8–64MB 小块池）取代默认 Performance**——实测（展开态
  悬停 15s，同为新鲜进程）：Performance 工作集 303MB/提交 600MB；MemoryUsage 更差
  （上轮已证）；**Manual 230MB/288MB，提交砍半、工作集 −24%**。本负载资源集合小且
  稳定，块开销可忽略。回退：`DAOZAI_WGPU_MEMHINTS=performance`。
- **深休眠工作集裁剪**：进入深休眠即 `SetProcessWorkingSetSize(-1,-1)`（resources.rs
  新增 `trim_working_set`），之后每 5 分钟补一次；工作态重置计时保证下次进深休眠
  立即裁。实测深休眠工作集 **~30MB**（唤醒正常，页软缺页自动载回）。只影响驻留
  不影响提交。
- D3D12 的 ~110MB 固定差（vs GDI 50MB）是驱动/运行时 DLL 载入，砍不掉；
  WGC 帧池本来就是社区建议的 2 缓冲。
- 验证：1193 passed；部署标记 `liquid-test-44-memopt`，exe `f910282cb317dd83`
  （备份 `daozai.exe.prev-memopt`）。

### 7.0m 本轮已完成（2026-11-17 16:51–17:40，合并上游 v0.6.65）

用户要求开合并分支合并上游当前状态、保留全部定制、冲突让用户选择。**当前在
`merge-upstream-v0665` 分支，合并提交 `b1ce395`，未合回 main**（待用户实机验证后合）：

- **历史连通（关键技巧）**：上游是「快照导入」无共同祖先（直接 merge 报 unrelated
  histories）。实测确认 `6e3150b:dz-phone` 子树 == 上游 v0.6.64 tag 整树，用
  **`git replace --graft 6e3150b 232165f`（=v0.6.64 提交）** 接上血缘，再
  `git merge upstream/main -X subtree=dz-phone`：合并基=v0.6.64，157 个上游提交
  只有 **13 文件 30 处真冲突**。replace ref 是本地的，换机器/重克隆后要重建。
- **用户拍板三项**：媒体控制线=上游为主融合我方（QQ 音乐 22.82 适配本来自用户
  PR #162）；graphics.rs=保留我方 MemoryHints::Manual；CHANGELOG=两边合并。
- **其余 27 处融合**：我方独有字段/功能全保留（media_gap_until/focus 之外的字段、
  网易云 CDP、qq_volume_command、notice 历史、模态对话框 toast 对齐、液态玻璃
  content_origin、PhoneBattery、ui_tests 2000 视口等），上游新功能全收（时钟/
  天气常驻项、oled_shift、lift_halo/center_rim、诊断工具、weather_source、
  music_volume 异步 run 架构）。
- **合并后修复**：补 smtc.rs 闭合括号、music_volume 残留旧行、player_process 重复
  image_path、media_gap_until 字段被自动合并丢掉（重插）、focus_editing 被上游移除
  （跟随上游删引用）、is_qqmusic 补 AUMID 变体（Tencent.QQMusic.1，我方逻辑并入）、
  toast 过期回 TOAST_SECS（上游 2.4s 与我方 toast_now/淡出 6s 不一致，上游测试无依赖）。
- **验证：1351 passed / 0 failed / 131 ignored**（净增 158 个上游测试）。
- **graphics.rs 再吸收上游一项**：`max_non_sampler_bindings=65536`（DX12 描述符堆
  100 万项→65536，省 ~57MB），与我方 Manual 共存；上游的 MemoryUsage 策略未采
  （我方 A/B 实测更差）。构建部署预览版 `liquid-test-45-merge65`
  （exe `5ebee4d9eb20c5fa`，备份 `daozai.exe.prev-merge65`），冻结模式实机验证：
  展开媒体页渲染正常、设置窗口导航正常、收起态常驻条正常、展开态内存
  261MB/提交 293MB（上游新代码 +31MB 工作集，提交持平——描述符优化生效）；
  已恢复实时模式。设置页深滚自动化受窗口焦点限制未走完，设置项共存已由
  测试覆盖。

### 7.0n 本轮（2026-11-17 17:58–19:20，用户报胶囊外圈白线/收起黑块）——**已全部回退**

用户实机报胶囊形态问题。本轮尝试的修复经用户反馈**全部无效或找错位置**，
已按用户要求 `git reset --hard 7bd6bed` 整体回退（f8b330d/9583bda 及工作区
折腾全部丢弃）。用户补充的关键信息：

- **外圈白线只在开启 Direct3D 12（wgpu 渲染器）时出现，关闭则无；胶囊形状
  （IslandShape::Pill）与其它皮肤都有** → 指向 wgpu 渲染器的描边渲染差异或
  液态玻璃/毛玻璃的白边（liquid_edge/glass_rim）视觉，非合并引入的绘制调用
  （paint_center_rim/lift_halo/glass_rim/feather/pill_shadow 逐一排除或与
  base 相同）。待用户给出未缩放原始截图+精确复现条件后定向修。
- **收起黑块**：「自动避让」(browser_minimal) 触发收成刘海细线时与玻璃胶囊
  双画错位 ~40px（实机复现实证：关闭该设置黑块消失；机制 base 已有，非合并
  新增，但合并后错位加剧/暴露）。专项修复待办。
- 教训（坑 ㉞）：合并大重构后的视觉回归，禁止在未稳定复现用户场景前动手改
  绘制代码；本轮 4 次尝试（center_rim/歌词竖线/lift_halo/描述符堆）全部
  无效即证据。图片是手机翻拍+缩略图，摩尔纹会把玻璃高光放大成「白线」，
  定位必须用原始分辨率截图。
- 上游新增亮点：QQ 音乐 22.82 适配（/seekto 单实例通道）、酷狗本机接口跳转、
  时钟秒数/天气常驻开关、启动即诊断收集、天气代理回退。

### 7.0o 本轮（2026-11-17 19:25–19:52，D3D12 黑角/白线真因）——**已修复并部署**

§7.0n 当时因截图是缩略/翻拍图误判为绘制描边；本轮用户给出关键条件「只有开启 Direct3D 12 才出现」后，定位到**上游 v0.6.65 合并引入的分层窗口兼容路径**，不是液态玻璃 shader：

- 上游新增 `layered.rs` + `native.rs::present_frame`：D3D12/Vulkan/部分 AMD 走 `UpdateLayeredWindow` 显示读回画面，owner 改成 `WS_EX_LAYERED` + `alpha=1/255`，并把 `SetWindowRgn` 退回**矩形**。
- D3D12 翻转模型交换链**不遵守 owner 的 1/255 alpha**：矩形窗口区域把透明画布的黑底露出来 → 收起/展开态圆角外两块黑角、展开过程大黑底；岛体描边压在黑底上就显得像「矩形白线/多余部分」。关掉 D3D12 后走正常合成，问题立即消失，与用户观察一致。

修复（`8a5f63d`）：

| 文件 | 改动 |
|---|---|
| `native.rs` | `outline_clip` 去掉 `!Layering::Active(_)` 条件，`opaque_surface` 时**始终**按 `GlassShape` 轮廓 `SetWindowRgn`；上层分层显示窗口继续提供抗锯齿，硬裁只作用于 owner 的交换链 |
| `layered.rs` | 文档补注：本项目在 D3D12/Vulkan 下保留 owner 轮廓裁剪，分层窗口只负责上层画面 |
| `native.rs` 测试 | `opaque_readback_uses_outline_alpha_instead_of_jagged_clip` 改为断言切到分层窗口后**仍是轮廓区域**（原断言要求矩形，正是 bug） |

- 构建：`cargo build --locked --release --bin daozai --bin daozai-bridge` 成功（仅既有 warnings，无 error）。
- 测试：全量 `cargo test --locked --release --lib -- --test-threads=1` → **1351 passed / 0 failed / 131 ignored**。
- 实机：冻结背景模式截图（收起、展开、展开过程四帧）确认四角黑块、大黑底、矩形白线/多余部分全部消失；实时模式已恢复运行。
- 部署：备份 `daozai.exe.prev-outline-clip`（旧 `88cb962ae009bd8f`），新 exe `3783f00131d4ab61`，标记 `liquid-test-46-outline-clip`。
- 打包：`liquid-test-46-outline-clip` 安装包 `daozai-liquid-setup-46.exe`（10,988,147 B，包 sha `e2f799e24bd9862c`，内含 exe `3783f00131d4ab61`）已在桌面预览目录，`daozai-liquid-setup.exe` 同步指向它；-44 包移入 `_archive\`。
- 仍待办：§7.0n 的 `browser_minimal=1` 自动避让收成细线时的黑块是**另一个机制**，本轮未动；custom-130 安装包未重打，如果正式版也要此修复需另行重打包。

### 7.0p 本轮（2026-11-17 20:19–20:54，D3D12 外层圆角矩形）——**已修复并打包**

用户纠正：要去掉的不是液态玻璃着色器白边，而是**比玻璃本体更大、包围在胶囊外的一层圆角矩形**。按「合并前后分层渲染/外圈代码对比」定位到两个来源：

| 来源 | 证据 / 修法 |
|---|---|
| `native.rs` 模糊底板 | 合并前 `opaque_surface=true` 一律关掉独立模糊底板；合并后 `layered_alpha` 让 D3D12 也启用它。`blur_pieces` 带肩部/半径外扩，底板比岛体大一圈 → 外层矩形。改为兼容后端一律 `!self.opaque_surface`，并加回归断言 |
| `app.rs` 3px 内缩不匹配 | WGSL 把玻璃形状内缩 3 物理像素，但矢量兜底填充和 `GlassShape` 区域仍用原始 `rect` → 外露 3px 环形。新增 `GLASS_SHADER_INSET_PX`，兜底填充、窗口区域/分层蒙版同步内缩 |

- 误改提醒：一度按“白边”理解去删着色器高光，已在工作树**全部还原**（`git restore`），最终源码不含该改动。
- 构建：`cargo build --locked --release --bin daozai --bin daozai-bridge` 成功；全量串行 **1351 passed / 0 failed / 131 ignored**。
- 实机：原生 2560×1440 冻结截图收起/展开对比，先去掉填充外层，再去掉 3px 环形；最终胶囊边缘只由玻璃本体构成（仍保留 1px 抗锯齿细边，不再是外层矩形）。
- 打包：`liquid-test-49-backup-inset`，安装包含 exe `bd8c47058d975437`，包 sha `f99b9b48d7e82f4a`，10,988,840 B；-46 包移入 `_archive\`。

### 7.0q 本轮（2026-11-17 21:06–21:38，岛内棋盘定位真因）——**已修复并打包**

§7.0p 的“模糊底板 + 3px 内缩”判断不完整：用户指出要看**岛窗口内部的棋盘诊断**（`DAOZAI_LIQUID_FLAT=2`，不是全屏棋盘壁纸），并把左/右角圈出来要求标注对照。

A/B（同一份 settings、同一时刻壁纸、冻结模式）：

- `-44`（合并前）：棋盘被裁成**圆角胶囊**，无外层矩形；
- `-49` / `-50`：棋盘变成**直角矩形**，外面另有一圈圆角轮廓。
- 强制轮廓蒙版（`-51`）无效；诊断日志确认蒙版路径已启用，但棋盘仍是矩形。结论：上游的**分层读回显示窗口**把 GPU 整块矩形画面搬到 owner 之上，正是“多出的部分”。

最终修复（`bcf44e8`）：

- `native.rs::wants_frames()` 恒返回 false —— D3D12/AMD 兼容后端恢复合并前的 **owner 窗口 + `SetWindowRgn` 轮廓裁剪直出**，完全停用 `layered.rs` 的分层读回；
- 同步改写对应回归测试；§7.0p 的错误方向（3px 内缩对齐）已从源码回退，只保留“兼容后端不启用独立模糊底板”。
- 构建成功；全量串行 **1351 passed / 0 failed / 131 ignored**。
- 岛内棋盘对照 `-52` 与 `-44` 一致：棋盘圆角、外层矩形消失。
- 打包 `liquid-test-52-no-layered`：exe `bc04b8d12c581da4`，包 `b6d6c9db8b54c31b`，10,979,445 B；-49 包移入 `_archive\`。

### 7.0r DirectComposition 试验（2026-11-17 21:55–22:12，分支 `directcomp-spike`）

目标：不改液态玻璃 WGSL，用正确的透明合成链路解决上游的黑框/锯齿，而不是分层读回窗口。

实现：

- vendor `egui-winit`：`DAOZAI_DCOMP=1` 时**只给第一个窗口**加 `with_no_redirection_bitmap(true)`（创建期样式，之后无法补加；设置窗口仍走普通路径）。
- vendor `wgpu-hal`：给 DX12 `Device` 加只读 `raw_d3d12_device()`，用于同一个 ID3D12Device 创建 DComp device。
- vendor `egui-wgpu`：
  - `RenderState::create(..., compatible_surface=None)` 先建设备；
  - `DCompositionCreateDevice2` + `CreateVisual` + `CreateTargetForHwnd` + `SetRoot` + `Commit`；
  - `instance.create_surface_unsafe(SurfaceTargetUnsafe::CompositionVisual(...))`；
  - DComp device/visual/target 放在 thread_local 保活；
  - 只对 `ViewportId::ROOT` 生效，设置窗口不碰。
- `launch-live.cmd` / `launch-freeze.cmd` 增加 `set "DAOZAI_DCOMP=1"`。

验证：

- 编译成功；全量串行 **1351 passed / 0 failed / 131 ignored**。
- 实机冻结截图：
  - 岛内棋盘 `DAOZAI_LIQUID_FLAT=2`：圆角胶囊、桌面自然透出，无黑底/外层矩形；
  - 普通模式：无黑框、无整窗不透明。
- 包 `liquid-test-53-dcomp`：exe `200c0873698f00df`，包 `f1d360e720d64caa`，10,980,463 B；-52 包移入 `_archive\`。

待办：这是 spike，未合回 `merge-upstream-v0665`；先让用户实机确认，再决定是否替代上游 `layered.rs` 并整理 PR。

### 7.0s DirectComposition 跑通（2026-11-17 22:20–23:38，提交 `9c91ebb`）——**当前 HEAD / liquid-test-55**

§7.0r 那版实机暴露两个问题，逐个修掉：

1. **关 D3D（OpenGL/Glow）时整窗画不出来**：`WS_EX_NOREDIRECTIONBITMAP` 是无条件加的，而 OpenGL 正是靠 GDI 表面显示。
   → 改为只在 DX12 尝试期间置 `DAOZAI_DCOMP=1`（`graphics.rs::launch_compatible`），退回 Vulkan/OpenGL 前恢复；窗口重建后自然带 GDI 表面。
2. **开 D3D 后非液态皮肤圆角有锯齿**：`layered` 停用后仍套 `SetWindowRgn` 硬裁。
   → `native.rs` 改为 `outline_clip = opaque_surface && !direct_composition_active()`；该标志由 thread_local 改为**全局原子**（surface 创建与窗口更新不在同线程）。

链路补齐（都是 DComp 必须但上游/wgpu 24 缺的）：

- **DComp 设备**：`DCompositionCreateDevice2` 要 `IDXGIDevice`。AMD 驱动 over `ID3D12Device` 的 QI 直接失败 → 改用同一个 DXGI adapter 建 D3D11 设备取 `IDXGIDevice`（`wgpu-hal::Adapter::raw_dxgi_adapter`）。
- **Commit**：wgpu-hal 只对 visual 调 `SetContent`，从不 `Commit`；surface configure 后必须补一次，否则窗口全空。
- **alpha 模式**：wgpu 24 把 `map_acomposite_alpha_mode` 硬编码成 `DXGI_ALPHA_MODE_IGNORE` → 映射 `PreMultiplied`，并在 `SurfaceCapabilities` 里声明。
- **认领方式**：从“首个 surface 一次性布尔”改为**按 HWND 认领**（`DCOMP_HWND`），窗口重建可复用，也不会误认设置窗口。
- **WGSL**：真实 alpha 路径（DComp）输出预乘 `alpha`，岛体外 `alpha=0` 让桌面透出；旧兼容路径保持 `alpha=1` 不变。
- **清屏**：DComp 下 eframe 清屏色必须全透明，否则整窗一块浅灰矩形（以前被 `SetWindowRgn` 裁掉看不出来）。

实机四态验证（原生 2560×1440，冻结背景）：

| 场景 | 结果 |
|---|---|
| 液态玻璃 · 普通 | 玻璃胶囊、边缘平滑、无外层环/矩形 |
| 液态玻璃 · 岛内棋盘 | 棋盘裁成圆角胶囊，舱外无矩形 |
| 经典皮肤 · D3D(DX12/AMD) | 圆角平滑，与 OpenGL 无差异 |
| 经典皮肤 · OpenGL(NVIDIA) | 正常显示，无黑底 |

打包 `liquid-test-55-dcomp-hwnd`：exe `6eb9cdad9a585a09`，包 `8b4a7d8c4fa11818`，10,983,849 B；-53 包移入 `_archive\`。全量串行 **1351 passed / 0 failed / 131 ignored**。

仍未做：DComp 创建失败后的**运行期**兜底（目前靠 `Err(Wgpu)` → 重启 Vulkan 路径兜住）；`merge-upstream-v0665` 主线未同步，仍停在 52。

### 7.0t 设置页崩溃 + 关 D3D 岛体消失（2026-11-18 23:45–00:32，提交 `d0b737b`）——**当前 HEAD / liquid-test-57**

用户两个反馈，根因都不在“绘制层”，而在透明合成链路的边界条件：

**A. 打开设置页崩溃**（`wgpu_core.rs:3432` → `Surface::configure` 致命错）
设置页是**第二个 OS 窗口、独立 surface**。`direct_composition_active()` 是全局标志，于是设置窗口的普通 HWND 交换链也被强制 `PreMultiplied`，DXGI 直接拒绝。
→ `SurfaceState` 增加 `dcomp` 字段，**逐 surface** 决定 alpha 模式；`wgpu-hal` 只在 `SurfaceTarget::Visual` 的能力里声明 `PreMultiplied`。

**B. 关 D3D 后岛体完全不出现**
设置里「兼容渲染（Direct3D 12）」旁边就有「立即重启」；重启走 `spawn_replacement()`，**继承父进程环境**。而 `DAOZAI_DCOMP=1` 在 D3D 运行期间一直留在进程环境里 → 新进程用 OpenGL，窗口却仍被去掉 GDI 表面，OpenGL 正是靠它显示 → 岛体不见（进程甚至会直接退出）。
→ `graphics::run()` 启动即 `remove_var("DAOZAI_DCOMP")`；`spawn_replacement()` 额外 `env_remove`。两处双保险。

**顺带补的运行期兜底**（用户要求）：
- **建窗口前预检** `direct_composition_available()`：D3D11 → `IDXGIDevice` → `DCompositionCreateDevice2` → `CreateVisual`。探不通就**不去掉 GDI 表面**，直接走 HWND 交换链，不用整进程重开（这个样式建窗后无法补加，所以只能前置）。
- **建 surface 失败就地回退**：`mark_dcomp_failed()` + 退回普通交换链，`DCOMP_ACTIVE` 保持 false，`native.rs` 自动回到 `SetWindowRgn` 轮廓裁剪兜住形状。
- **DComp visual 按 HWND 复用**：一个窗口只能有一个 DComp target，surface 重建（换缩放/切屏/窗口重建）时再 `CreateTargetForHwnd` 会返回 `WINDOW_ALREADY_COMPOSED`，随后掉进兜底、圆角又变锯齿。
- 诊断：一次性记录「透明合成：opaque/dcomp/outline_clip」与每次区域设置来源，排障先看这两行。

验证：设置页可开、展开/收起正常、遗留 `DAOZAI_DCOMP=1` + OpenGL 也能出岛体；全量串行 **1351 passed / 0 failed / 131 ignored**。

打包 `liquid-test-57-dcomp-fallback`：exe `9fdcbdf1993b0cac`，包 `82dc760c02e3f5cf`，10,984,291 B。桌面预览目录 `daozai-liquid-setup-57.exe`（= `daozai-liquid-setup.exe`）。
注意：桌面上 `-55` 与旧的 `daozai-liquid-setup.exe` 已不在了（用户侧清过），发布脚本的归档分支因此跳过。

### 7.0u 收起后工作集回落慢（2026-11-18 00:56–01:16，提交 `4c991bb`）——**当前 HEAD / liquid-test-58**

用户反馈「从展开态到收起态，内存跳回得很慢」。实测确认：收起后 90 秒仍停在 230MB（基线 198MB），根本没回落。两个原因叠在一起：

1. **唯一回收路径挂在深休眠上**：`trim_working_set()` 只在 `deep_sleep` 成立时跑，而深休眠要求 `pill_still` 持续 **120 秒**；
2. **`pill_still` 要求 `media.playing == false`**：只要在放歌（用户常态），深休眠永远进不去 → **整台机器永远不会回收**。诊断日志 `pill-sleep.log` 里 `blocked=ambient,media` 连续几十条就是证据。

改法（`app.rs`）：

- 新增 `WORKSET_IDLE = 20.0`：收起 + 鼠标离开 + 无过渡动画/歌词滚动/吐司/通知，满 20 秒就 `trim_working_set()`，之后每 5 分钟补一次；
- **不再用 `ambient`/`media` 挡**：放歌、番茄钟属于常驻状态，挡掉等于永不回收；
- 深休眠（冻宠物 + 1Hz）保持原 120 秒门槛，两者解耦——用户能感知的是深休眠，感知不到的是工作集回收。

实测（收起态采样，独显 + 液态玻璃）：

| 时刻 | 改动前 WS | 改动后 WS |
|---|---|---|
| 收起基线 | 198 MB | 195 MB |
| 展开态 | 226 MB | 221 MB |
| 收起后 5–20s | 228–230 MB | 224 MB |
| 收起后 22s | 230 MB | **22.4 MB** |
| 收起后 90s | 231 MB | **31 MB** |

私有提交不变（~267MB，其中展开态多吃的 ~31MB 是 D3D12 堆水位，属正常不回收部分）。

打包 `liquid-test-58-fast-trim`：exe `8f61cb368954e198`，包 `162afa59339ab71c`，10,982,551 B；桌面 `daozai-liquid-setup-58.exe`（= `daozai-liquid-setup.exe`）。全量串行 **1351 passed**。

### 7.0v rust-analyzer 告警清理（2026-11-18 12:55-13:15，提交 `2ea229a`，已推送 origin/main，源码自身 warning 清零）

用户要求继续 rust-analyzer 分析、减少 warning。`cargo check --locked --all-targets` 口径下，
上一轮（02:57 打包 custom-131 时）用 `cargo fix` 清过一轮，工作树里还留着 8 个文件的 `#[cfg(test)]`
改动未提交；本轮把它收尾，并把剩下的 19 处一次改完。

- **根因两类**：① 测试专用项（函数/常量/字段）只在 `cfg(test)` 用 → 加 `#[cfg(test)]`（上一轮已做 8 文件）；
  ② 生产专用项写在 `#[cfg(not(test))]` 块里，`--all-targets` 的 test 编译把块裁掉 → 项变 dead_code → 给这些项补 `#[cfg(not(test))]`。
  两者互不重复：一个消除生产构建的 dead_code，一个消除测试构建的 dead_code。
- **本轮 19 处改动**（分布在 8 个文件）：
  - `daozai-bridge/main.rs`：`MAX_JSON_BYTES`/`SerdePrefs`/`config_dir`/`env_dir`/`read_capped` 补 `#[cfg(not(test))]`；
  - `model.rs`：`PhoneApps.modified`/`read_items`、`BridgePrefs::path`、`MaaPrefs::path` 补 `#[cfg(not(test))]`；
  - `phone_battery.rs:RELATIVE`、`qweather.rs:Error::Rate`（后者用 `#[cfg_attr(test,allow(dead_code))]` 因 match 臂仍引用）同类处理；
  - 真实瑕疵：`gpu_glass.rs` 删重复 `#[test]`、`display_layout.rs` 未用 `back`→`_back`、`ui_tests.rs` 三处多余 `mut`；
  - `ui_tests.rs:editing_local_calendar_clears_outdated_reminders` 漏写 `#[test]`（上游就漏）→ 补上并验证单独运行通过；
  - `bridge.rs` 两个中文测试名含大写字母触发 non_snake_case → 加 `#[allow(non_snake_case)]`。
- **结果**：`cargo check --locked --all-targets` 与 `cargo build --locked --release --bin daozai --bin daozai-bridge`
  下 daozai 自身 warning = **0 条**（剩余 240 条全在 vendor：egui/imap-proto/wgpu-hal/ureq/egui-wgpu，按约定不动）。
  全量串行测试 **1351 passed / 0 failed / 131 ignored**（新增测试已注册：总 test 1482，含 131 ignored）。
- **交付**：`2ea229a`（17 文件 55+/14-）已提交并推送 origin/main，工作树干净；未打包、未部署、未改 Cargo.lock。

### 7.0w 正式包 custom-133：液态玻璃按钮缩小 + 反馈日志说明（2026-11-18 13:25，未提交）

用户两条需求：① 液态玻璃下把《词》/上一首/播放暂停/下一首/音量按钮改小一点点、不与底部分页指示条重叠，且不影响其他皮肤；② 构建正式安装包，把「各项问题反馈该提供哪些 log 文件」写进反馈说明。

- **按钮缩小**（`app.rs` / `pages.rs`）：按钮可见大小 = 玻璃圆盘半径，原来 13.5 使下沿 131.5 压到指示条上沿 130.8（重叠约 0.7 点）。新增 `pub(crate) const MEDIA_CTL_R: f32 = 11.5`，圆盘下沿抬到 129.5、留约 1.3 点间隙。GPU 折射透镜（ctl_geom 五个按钮）与矢量兜底（pages.rs glass_disc + gpu 悬停圈）同用该常量，保持逐像素一致。其他皮肤走 `else (12.5,20.0)` 分支，未动；命中点击区未动，坐标测试不受影响。视频页共用这套控件会一起缩小（同属液态玻璃）。
- **反馈说明**：写进 `installer/INSTALL-NOTES.txt`（daozai.iss 里已标 `isreadme`，装完自动打开）。首选「设置→关于→收集诊断」生成桌面 zip；再按崩溃/黑屏/内存/歌词/天气/验证码/邮件/通知/QQ微信/浏览器桥/日历/卡顿分类，列出 `%APPDATA%\Daozai\diagnostics\` 下应附的具体文件。依据 = 源码实际写入点 + collect-diagnostics.ps1 收集清单；`pill-sleep.log`/`liquid-glass.log` 不在诊断包内，已单独点名。
- **打包**：`daozai.iss` 的 MyAppBuild custom-132 → custom-133；ISCC 编译成功（仅历史固有的 HKCU/admin 提示）。包 `daozai-setup-custom-133.exe` 最终 11,241,513 B、sha256 前16 `a9455e0ad655bb9f`；exe `02786373d87ba563`（22,782,464 B）、bridge `f1ae28626c9db18e`（含用户文案改动 dc639f9，见下方「文案重打包」行）（476,672 B）。编译前已停掉运行中的 daozai_bridge.exe（PID 34168）避免占用。
- **验证**：`cargo check --all-targets` 无 error；全量 `cargo test --locked --release --lib -- --test-threads=1` = 1351 passed / 0 failed / 131 ignored。
- **提交/重打包**（14:02）：`14bb37f`（7 文件 93+/19-）已提交、未推送；提交前还原了 `extras.rs` 的整文件 rustfmt 污染（5279 行，非本任务改动）。基于干净提交重新编译打包，包指纹更新为 sha `0a107d77bd19e6c3` / exe `a7a7cea6e1550007` / bridge `67b897d481b2650e`（覆盖首次打包的 `a41348b0804328ea`）。
- **多线程测试结论**：用户要求开多线程跑全量，`cargo test --locked --release --lib`（默认多线程）结果 1348 passed / 3 failed / 131 ignored，失败的 3 个是 messenger/wechat 通知状态测试（qq_account_observation...、qq_single_tray_account...、all_taskbar_clients_first_flash...），与本次改动无关；单独用 `--test-threads=1` 重跑这 3 个全过，再跑完整单线程全量 = **1351 passed / 0 failed**。再次确认这些测试依赖全局状态、必须串行（与 HANDOFF 既有结论一致）。
- **未做**：未推送 origin/main，未部署正式目录。
- **文案重打包**（14:16，用户改文案 `dc639f9`）：用户自行提交 `dc639f9`（extras.rs 液态玻璃配置提示文案精简，去掉两处重复的旧句、合并为「调参数时岛仔会保持展开」），本地已与 origin/main 同步。按用户要求**不跑测试**，停掉运行中的 daozai.exe（PID 36848）后重新 `cargo build --release` + ISCC 打包，覆盖 14:02 那版。最终包 `daozai-setup-custom-133.exe` 11,241,513 B、sha256 前16 `a9455e0ad655bb9f`；exe `02786373d87ba563`、bridge `f1ae28626c9db18e`。

### 7.0x v0.6.66 选择性合并（2026-11-18 14:30–17:00，分支 merge-v0666-selective）

用户规则：液态玻璃/内存优化/本地设置页/本地已修 bug 全保留；上游只吸收新增/bugfix；兼容渲染优化用本地方案。

**合入的上游提交**（8 个，均 cherry-pick 或手工移植、逐个编译验证）：
- `47a8671` 全屏判定改客户区（含 #175）；`4bed556` 兼容渲染全屏显示层；`b4dab19` 合唱歌词 #202；`15d9179` 更新检查 #186。
- `db34e27` 避让宿主/最小化（`96719cd`）：native.rs 整文件冲突→取 ours + 5 处上游增量（FG_CONTEXT/方法化 take_foreground_change）；system.rs 块1/3 取 theirs、块2 取 ours（保 focus_browser）。
- `9ecf30d` 提醒三选一（`ce5d601`）：model.rs Default 行补 system_notify:true；extras.rs 融合 theirs 三选一 + ours 会议勿扰；确认 Cmd::Notify 的 `&& system_notify` 拦截生效。
- `12a9329` 设置页重组（`efb8266`）**手工移植**：cherry-pick 因 20 块语法级交错冲突失败（4 次尝试均 abort），改用脚本以本地 extras.rs 为基准——用上游完整 settings_ui（850 行含新侧栏/功能卡/弹窗）替换本地旧函数（1231 行）、注入 7 个辅助函数、SETTINGS_GROUPS 扩展为 14 项（上游外观13/位置14 + 本地 tab15「自定义与集成」）、把本地定制页搬进 tab15、app.rs 两个 Form 加 open 字段、补 6 个 SVG。

**定制零丢失**（逐项核对）：液态玻璃 13 项配置弹窗、展开动效、悬停延时、手机连接/浏览器下载/MAA、55 个本地函数（maa/phone/backup/notice 等）全部保留；上游新骨架全部到位。

**9 个测试适配新布局**：display/top_bar 设 tab14、theme 去掉进弹窗的 tab7/8 检查、mail 3 个设 form.open=true、notification 适配合并页、weather_status 标题改「岛上显示」、qweather 开 config_open + 视口加大。

**验证**：`cargo check --all-targets` 通过；单线程全量 **1359 passed / 0 failed / 132 ignored**；双 bin release 构建通过。

**未做**：未推送 origin/main、未部署、未重新打包（custom-133 包是合并前的）；提醒三选一对应的上游「双层开关迁移」未合（保留本地细粒度控制，故未引入上游 settings_redesign_ui_tests.rs）。

**坑**：git diff3 在两个深度分叉的大文件上产生语法级交错冲突（自动合并区混入对方代码致括号不匹配），机械选边无解；手工按函数级组装才成功。桌面方案文档 `daozai-v0666-方案优劣对比.md` / `daozai-设置页重组融合方案.md` 有全过程。
### 7.0y 补合 5 项剩余上游改动（2026-11-18 17:26）

用户选 ①②③⑤⑨。5 个提交全部合入 merge-v0666-selective，HEAD = `83f3417`：

- `b6b5e0b` **声音提醒修复**（08250c5）：新增 `NotificationSound{Silent,System,Health}` 三态 + 内嵌 `health-reminder.wav`（18.5KB）+ `play_health_sound()`（PlaySoundW）。喝水/久坐提示音改为独立播放，不再依赖系统通知气泡是否弹出——修掉「开了声音只显示图标」。Cmd::Notify 由 2 参改 3 参；与已合的「提醒三选一」衔接一致。
- `6502f70` **裁切修复 #195**（a9fbc6b）：a9fbc6b 三处改动中两处（大小/位置滑杆 wrapped+clamp）已随设置页重组合入，只差 `settings_segmented` 的 `ui.horizontal`→`ui.horizontal_wrapped` 一行，补上。
- `4dc1672` **滚轮翻矮页误收起**（6156474，**只合 app.rs 部分**）：FrameOut/App 加 `page_scroll_area`，翻到矮页时沿缩短一侧保留 8px 续滚带，`inside`→`page_hover` 用于滚轮入口与 want。**native.rs 的 WH_MOUSE_LL 全局钩子未合**（系统级风险），故「旧页区域继续滚轮翻页」不完整，但「误收起」已解决。滚轮/翻页 69 测试通过。
- `83f3417` **版本号 0.6.66 + CHANGELOG**：Cargo.toml/Cargo.lock/app.rc（数字版+字符串版）四处 0.6.65→0.6.66；CHANGELOG 新增「吸收上游 v0.6.66 修复」段。

**验证**：`cargo check --all-targets` 通过；单线程全量 **1360 passed / 0 failed / 132 ignored**（比之前多 1 个声音测试）；双 bin release 构建通过。

**仍未合**（用户选择不合/暂缓）：⑥ RTL 歌词（价值≈0、动歌词布局）；⑦ 兼容渲染优化 e992b31/98496d3（本地已删该链路、方案更优）；④ 细条点穿 94fc97c（碰渲染核心、暂缓）；双层开关迁移（保留本地细粒度控制）。

**未做**：未部署；custom-133 包是 17:15 基于设置页重组版打的（不含本轮 5 项 + 版本号 0.6.66），如需体现本轮改动需重新打包。
### 7.0z RTL 歌词融合 + 极简细条点穿（2026-11-18 18:08）

两个提交，均在 merge-v0666-selective：

- `d803565` **RTL 歌词融合**（8d8a4cc）：引入 `rtl_text.rs`（阿拉伯字母连写+双向重排）+ 藏文字体回退（himalaya.ttf，scale 1.35）+ paint.rs 全局文字入口。**关键融合**：本地歌词是自研 layout_job + 固定 11pt + 单行横向滚动，故岛上歌词两个闭包用 `display()`（单行转换、不插 \n），**绝不用上游 display_wrapped()**（折行会毁横向滚动）；缓存 key 仍用原文，overflow/scroll_offset/不折行逻辑一行未动。音乐页（自动折行）用 display_wrapped。非 RTL 文本 display() 返回 Cow::Borrowed 零影响，中文歌词滚动完全不变。验证：横向滚动专项 118 + RTL 10 单测 + 全量 1370 passed。
- `1bd87d4` **极简细条点击穿透**（94fc97c 低风险简化版，含可选步骤）：app.rs FrameOut 加 click_through/strip_rect、拆 minimal_strip（视觉不变）；main.rs 收起无通知时命中清零→HTTRANSPARENT 点击落下层、按住点穿过滤光标；native.rs 用普通 bool passive（**不写 WS_EX_TRANSPARENT|WS_EX_LAYERED、不调 SetLayeredWindowAttributes**）、watch_drag 按细条精确矩形轮询唤醒（不用覆盖整窗的 GetWindowRgnBox）。明确未合上游的每帧样式翻转（与 DComp 逐像素 alpha 互斥、黑块风险）、glass 关 blur、layered first_frame（死代码）。全量 1370 passed + 双 bin 构建通过。

**坑**：本地歌词两条路径（岛上 layout_job、音乐页 painter.layout_job）**都不经过 Card::galley**，只在 galley 加 RTL 钩子对歌词无效，必须分别在 app.rs/pages.rs 接入；且岛上歌词必须用 display() 而非 display_wrapped()，否则插入 \n 破坏单行横向滚动。

**仍未合**：⑦ 兼容渲染优化 e992b31/98496d3（本地已删该链路、方案更优）；双层开关迁移（保留本地细粒度控制）。

**未做**：未部署；custom-133 包是 17:15 版（不含 v0.6.66 版本号之后的所有改动：声音/裁切/滚轮/RTL/细条点穿）。
### 7.0aa 液态玻璃配置归位 + 修按钮 + 预览形态切换（2026-11-18 18:30–19:00，提交 `2342d3c`）

用户三条需求（为把液态玻璃并入上游提 PR 做准备）：

| 需求 | 根因 | 修法 |
|---|---|---|
| 《液态玻璃与交互》位置错了，要从「自定义与集成」移到「岛体外观」 | 7.0x 设置页重组时本地定制页(tab15)建得太随意，这块其实是上游也有的外观/交互内容 | 整块搬到 `SETTINGS_TAB_APPEARANCE`(tab13) 的「岛体显示」卡之后，`SETTINGS_TAB_CUSTOM` 保留手机连接/浏览器/MAA |
| 「配置液态玻璃」按钮点了没反应 | `liquid_glass_config_window` 守卫与 `liquid_tuning` 都写死 `settings_tab==0`；块在 tab15，点按钮设 `liquid_cfg_open=true` 后下一帧被守卫清回 false | 两处改为 `SETTINGS_TAB_APPEARANCE`；弹窗调用从 tab 分支里挪到 `settings_ui` 视口顶层（`avoidance.dialog` 之前），离开外观页/关设置窗可靠清标志 |
| 调参时的常驻展开态，希望加进配置里能切收起/展开 | `morph` 的 `liquid_tuning` 分支无条件强制 `open=true` | App 新增 `liquid_preview_collapsed`（不落盘，默认 false=展开）；弹窗顶部加「预览形态」segmented（展开态/收起态）；morph 按它固定形态——收起态走 `tune_close` + 清媒体浮层/编辑态，展开态走 `tune_open` |

**关键坑**：
- 弹窗 `egui::Window` 的调用从 tab 分支挪到视口顶层后，守卫才能每帧跑到——否则离开外观页时函数不再被调用，`liquid_cfg_open` 留在 true，`liquid_tuning` 又因 tab 不符返回 false，会出现「标志脏着但形态不接管」的隐性状态。
- 用户实际配置 `settings.txt` 里 `render_compat=1`，守卫的 render_compat 条件成立；若 D3D 兼容渲染关闭，弹窗按设计不会开。
- 测试 `render_settings` helper 渲染 egui::Window 要跑多帧（≥4）才会出 text shape，首帧只建窗口不画内容。

**验证**：定向 3 测试通过（`liquid_tuning_keeps_island_expanded...` 改用 `SETTINGS_TAB_APPEARANCE`；新增 `liquid_tuning_preview_form_switches_collapsed_and_expanded`、`liquid_config_button_lives_in_appearance_tab`）；全量串行 **1372 passed / 0 failed / 132 ignored**（第一次跑有 1 个 `messenger::windows::tests::exited_messenger_accounts_are_removed_individually` 偶发失败——它创建真 OS 进程、PID 时序依赖，单独跑通过、重跑全量干净，与本次 UI 改动无关）。双 bin release 构建通过。
release exe `dfd191cc498aed8c8799c42ca822f4`（22,539,776 B）、bridge `e33f1e73d7447b3fd8b240949cd68c689e48792db340d322a20417be84f87aef`（378,880 B）。
**打包部署（19:05）**：`liquid-test-59-cfg-form`，Inno Setup 7 编译；包 `84c4dc1f473b0319`（11,005,484 B）、内含 exe `dfd191cc498aed8c`。桌面 `daozai-liquid-setup.exe` = 该包，旧 -58 备份到 `_archive/daozai-liquid-setup-58.exe`；`_archive/README.md` 指纹表 + HANDOFF 快照 `HANDOFF-2026-11-18-1905.md` 已同步。
**未做**：未替用户重装/启动（正在跑的是正式版 `D:\software\Daozai\daozai.exe` custom-129，单实例锁）、未推送 origin。CHANGELOG 已加一节。
### 7.0ab 配置按钮夹进皮肤卡 + 标题「岛体交互」（2026-11-18 19:15–19:26，提交见下）

用户在 59 包实测后追加两条微调：

| 需求 | 改法 |
|---|---|
| 「配置液态玻璃」按钮移到皮肤和形状之间，且仅当前是液态玻璃时才显示 | 按钮从「岛体交互」卡挪进「岛体外观」卡，夹在皮肤描述与「背景模糊」之间；显示条件用 `self.settings.skin==Skin::Liquid`。**坑**：`Skin::is_glass()` 只匹配 GlassLight/GlassDark，不含 Liquid，不能拿它判断 |
| 下方标题「液态玻璃与交互」改成「岛体交互」 | `settings_label(ui,"岛体交互")`；该卡只剩展开动效/悬停延时（对所有皮肤通用），按钮行和 separator 一并去掉 |

测试 `liquid_config_button_lives_in_appearance_tab_and_only_for_liquid_skin`：液态皮肤有按钮+弹窗+「岛体交互」标题、无旧标题；切 Classic 皮肤按钮消失；自定义页无按钮。全量 **1372 passed / 0 failed / 132 ignored**。
打包 `liquid-test-60-skin-button`：包 `3c10d33e4567b5c9`（11,006,160 B）、exe `e2afdb7cb93dda40`；桌面 `daozai-liquid-setup.exe` = 该包，-59 已移入 `_archive`。

### 7.1 已提交（`234ff52`，已推送 origin/main）

工作树干净。上一轮叠在一起的两批改动已合成一个提交，测试包 `liquid-test-15` 就是它的构建：

**A. 多显示器换屏修复**

| 文件 | 改动 |
|---|---|
| `wgc.rs` | `monitor_for_rect()` 每帧按抓屏矩形**中心点**实时查显示器身份；变化则 drop 旧会话重建、清 FAILED/TRIED；新增 `EPOCH` / `BOUND_MONITOR` / `epoch()` |
| `gpu_glass.rs` | CleanPlate 记录 epoch，换屏后失效重抓 |

- 根因：WGC 单例只在首次 grab 时按 (x,y) `MonitorFromPoint` 绑定显示器，`mon_x/mon_y/sw/sh` 永久固定；
  换屏后 `x - mon_x` 错位越界 → 回退 GDI+ 旧屏底片；GPU 层 CleanPlate 也不区分显示器。
- 状态更新（`f0cc409`）：这条只覆盖「岛仔被拖到另一块屏 / 主屏互换」；**「仅主屏 → 扩展」这类切换是它漏掉的**，已由 ⑯⑰ 与 `f0cc409` 补上。诊断日志搜「显示器变化」「重建会话」「显示器拓扑变化」「等待 2000ms 稳定」。
- **仍待办**：实机换屏验证待用户配合（agent 无法物理复现）。

**B. 三个需求（进度条 / 专注休息 / 描边固定白）**

| 文件 | 改动 |
|---|---|
| `app.rs` | `CTL_N` 9→12；新增 `Ctl::Seek/Focus/Rest`；`seek_bar_geom()` / `seek_geom()` / `focus_tab_geom()`；`skin_palette` 液态恒 `GLASS_DARK` |
| `gpu_glass.rs` | `MAX_CTL` 10→13，uniform 432→528 字节 |
| `pages.rs` | 进度条底槽改 `glass_groove`；专注/休息在 GPU 接管时不再叠矢量胶囊；圆钮让位 GPU |
| `paint.rs` | 新增 `press_spring()`；`glass_groove` 正式启用（去掉 `#[allow(dead_code)]`） |
| `extras.rs` / `model.rs` | 外观页六个滑块说明与字段注释去掉「参考包」措辞，直接写默认值 |
| `ui_tests.rs` | 音乐页控件数 9→10；新增 `control_glass_covers_focus_and_rest_segments`、`liquid_ink_stays_white_in_both_materials` |

- **踩过的坑**：这批改动之前**从没编译过** —— `paint.rs` 用了 `i.dt`（egui 里没有这个字段，应是 `unstable_dt`）。
- 交付：1185 passed / 0 failed / 119 ignored；`liquid-test-15` 已打包 + 静默安装 + 预览目录已替换（备份 `daozai.exe.prev8`）。
- 安装包后续只动分发内容：`-16` 去掉 LICENSE 与完整测试说明，`-17` 换成精简的 `反馈与卸载.txt`，`-18` 把它的章节号改成「一、二」。
- **仍待办**：**视觉确认待用户**（凹槽深浅、分段按钮边缘、描边是否恒白、滑块说明新文案）。

### 7.2 custom-129 的实机验证（等用户确认）

- 放一段网页视频 → 展开视频页点「打开」→ 确认 Chrome/Edge 真被带到前台（最小化会自动恢复）。
- 若失败看 `%APPDATA%\Daozai\diagnostics\focus-browser.log` 的「候选 / 选中 / 前台」三项。

### 7.3 液态玻璃的下一步（按优先级）

1. 其它页内部卡片（专注/健康/待办/邮箱/额度/日历）统一套 `glass_chip` 的边；
   页内按钮还剩「开始计时」那颗红胶囊（`f-start`）没上 GPU。
2. 音量浮层要真玻璃，需要一个「视口 ≠ capture」的偏移 uniform（第二个 callback），
   或者浮层打开时把该区域歌词让出来 —— 都要改坐标链路，单独一轮做。
3. 控件「按下变通透」（frost 1→0）没做，`cprm` 已没空位。
4. 启动首帧「硬弹出」没有淡入，建议给岛体 60~80ms alpha 淡入。
5. ~~抓屏节流 `CAPTURE_EVERY=2`~~ —— 已由自适应帧率部分覆盖（静止 30fps）；若还要再压，
   下一个方向是「隔帧抓屏」，但静止时观感未评估。
6. 侧边停靠（`Dock::Left/Right`）时 band/strength 撞上限，未专门调。
7. ~~是否进补丁链（custom-130）~~：补丁链已退役，此条作废。

### 7.4 已知限制（不是 bug，别当 bug 修）

- WGC 初始化失败（`0x887A0004`）会永久回退 GDI + 底片，此时玻璃抓到岛仔自己 → 条纹。
  要彻底解决只能关掉 GDI 路径。
- WGC 帧滞后 1~2 帧，边缘混合处偶尔看到滞后桌面色。
- 展开动画中播放控件被裁剪线「追着」露出来，只有 2~4 帧，评估后不值得修。
- 深色材质保留背景色相（暗绿壁纸 → 暗绿玻璃），是有意的。
- 磨砂是刻意保留的（五点采样 + 轻微白雾），不是 100% 像素级透视。

---

## 8. 版本时间线

| 版本 / 提交 | 内容 |
|---|---|
| `67cd837` 后微调 | **配置按钮夹进皮肤卡 + 标题「岛体交互」**：按钮移到「岛体外观」卡皮肤与形状之间、仅 `Skin::Liquid` 显示（`is_glass()` 不含 Liquid）；下方标题「液态玻璃与交互」→「岛体交互」（通用交互）；1372 passed；`liquid-test-60-skin-button`（exe `e2afdb7cb93dda40`），已部署桌面 |
| `2342d3c` | **液态玻璃配置归位 + 修按钮 + 预览形态切换**：《液态玻璃与交互》从 tab15 移到「外观」tab13；修「配置液态玻璃」点击无反应（守卫 tab 写死 0 + 弹窗调用在 tab 分支里）；弹窗新增「预览形态」开关（展开/收起），morph 按所选固定形态 = 用户要的「配置里切两个形态」；1372 passed；在 `merge-v0666-selective`，未部署 |
| main 合并 | 2026-11-18：`directcomp-spike` 快进合并进 `main`（42a57c9 → 21b55ee，80 个提交）并推送 origin；`21b55ee` 含 liquid_shadow 开关，未进包 |
| custom-132 | 正式包：液态玻璃参数弹窗（皮肤/形状之间按钮唤出，仅 D3D12 兼容渲染开启时显示，弹窗内已去重）+ 折叠指示三角（原字形缺失显示成方块）+ phone-apps 移除内置种子并加「添加 App」入口；1351 passed |
| custom-131 | 正式包：液态玻璃改 DirectComposition 透明交换链（真实预乘 alpha、无 SetWindowRgn 锯齿）+ 收起静置 20s 回收工作集 + 30 处可机修告警清理；`installer/daozai.iss` 0.6.65/custom-131；1351 passed |
| custom-129 | 「打开」切前台：进程名过滤 + `AttachThreadInput` 绕前台锁 + 返回值检查 + 诊断日志；EDITS 237、重放 239 处 diff 空、1176 passed / 已部署 |
| custom-128 | 修视频摘要溢出 + 跳转浏览器窗口 + 2 秒宽限消除空态闪烁；EDITS 233、1175 passed |
| custom-127 | 视频专用展开页 + 收起态视频摘要；EDITS 225、1174 passed |
| custom-126 | 浏览器播放的媒体上岛（`is_island_media` 只放开显示门槛）；EDITS 214、1173 passed |
| custom-125 | 通知/邮箱/日程设置页长区块可折叠；EDITS 204、1171 passed |
| custom-124 | 横幅操作逐条日志、失败不中断；EDITS 200、1171 passed |
| custom-123 | 一键关闭其它应用的系统通知横幅（可恢复 / 官方设置兜底）；EDITS 198 |
| custom-122 | 整体删除「自动托管」（开关 + 后台重启 + 配置字段 + 端口探测）；EDITS 196 |
| custom-121 | 取消周期补扫、修 toast 不弹、提示移到上半部分停留 6 秒；EDITS 193 |
| custom-120 | 设置页文案修订重新打包（无功能改动） |
| `129de67` | 液态玻璃：48 → 60fps（重绘预约算法错）+ 抓屏节流 + perf 探针 |
| `33876c8` | 修启动时玻璃抓到自己的正反馈条纹（撤 GDI 回退 + 启动期重扫） |
| `5ee0769` | 液态玻璃：岛内控件玻璃化（音乐页 + 共用 chrome） |
| `8abaaf5` / `ec6ca1d` | 折射/色散改用 CSS px 单位（48 / 8px）；外观六项全部可调 |
| `7db92b3` | 调参时强制展开岛仔（设置页开着就保持展开） |
| `901666b` | 小控件全部改真折射 + 控件弹簧，去掉歌词卡玻璃 |
| `93edd15` | 其他页面控件统一玻璃化 + 边缘抗颗粒 |
| `234ff52` | 进度条底槽玻璃凹槽 + 圆钮真折射；专注/休息分段上 GPU；液态描边恒白；多显示器换屏修复；文案去「参考包」；1185 passed（= liquid-test-15） |
| `aa0f562` / `dc94ffb` / `f415518` | 安装包只带 `反馈与卸载.txt`（精简说明，章节号「一、二」）；**当前 main**，测试包 liquid-test-18 |
| tag `liquid-test-18` | 已发布 GitHub **Beta** release，资产 `daozai-liquid-setup.exe`；tag 指向 `f415518` |
| `5f5b428` | 主题随壁纸明暗自动切换 + 刘海形状真实轮廓 + 视频页控件玻璃化 + 圆钮按 Pack-v2 + 图标缩小 + 结构清理；1186 passed（= liquid-test-24/25） |
| `c3e9b9b` | 移除无效的自适应帧率（当时误判为 wgpu 回调锁定）；工作树回到干净（= liquid-test-26） |
| `cf76bbb` | **自适应帧率真正生效**：每帧调 `request_repaint_after` 会自激 + vendor `Poll` 残留；改 `glass_tick` 统一调度 → 静止 30fps / 悬停 60fps（= liquid-test-27） |
| `3ddcb30` | **液态玻璃可选优先独显**：核显 548MB→独显 257MB；`native_adapter_selector` 挑 DiscreteGpu + `is_surface_supported` 过滤（= liquid-test-28） |
| `bf116c2` | 独显设置项文案简化（用户手改） |
| `2c199f1` | 交接文档纳入仓库（`docs/HANDOFF.md`）+ README 入口 |
| `f0cc409` | 显示器拓扑：全桌面签名 + 每帧检测 + 稳定期纯色回退（**未真正生效**）= liquid-test-29 |
| `2cbbd4a` | 交接文档补拓扑（`f0cc409` 那版） |
| `a1a2669` | **拓扑花屏的真正修复**：重置登记丢失（坑 ⑱）+ 全黑底片保护 = liquid-test-30 |
| `1ee5795` | 试「加长重试到 3s」= liquid-test-31（**方向错误，已被取代**） |
| `b39e3a8` | 改为「底片 5s 定期刷新」+ 重试收回 1s = liquid-test-32 |
| `8d93c76` | 稳定期自适应学习（×0.85，落盘 `settle-learned.txt`）= liquid-test-33；**当前 HEAD** |
| `37e965d` | **主题判定不再被翻页打断**：`theme_rect` 固定尺寸采样框 + `Palette::glass_chrome`（玻璃白边不进换色）= liquid-test-34；1189 passed；**当前 HEAD** |
| `67182be` | theme_rect 越界加固（展开首帧抓屏极小时防 clamp panic）+ 退化尺寸单测 = liquid-test-34 重打包（exe `e016e1fa38554d99`） |
| `ec122ce` | **翻页过渡期控件也走 GPU 玻璃**：`ctl_glass_ready` 放行翻页、透镜随 `page_slide_tx()` 平移、旧页矢量淡出（坑 ㉔）= liquid-test-35；1189 passed；**当前 HEAD** |
| `f99ce67` | **额度/日历/日程编辑器按钮上 GPU 玻璃**：Ctl +11 变体（添加账号、＋新增、全天、提醒分段×5、删除、取消、保存）= liquid-test-36；1189 passed；**当前 HEAD** |
| `9a0efc6` | **邮件页全部/未读滑动指示器 + 全部账号透镜；修选中淡填充浅材质暗雾** = liquid-test-37；1189 passed；**当前 HEAD** |
| `d63e0ca` | **「弹簧强度」设置项**（液态玻璃参数区 0.2–3.0）+ 圆钮按压果冻 = liquid-test-38；1189 passed；**当前 HEAD** |
| `8316e1e` | **弹簧刚度改名 + 阻尼比设置；展开动效/触发延迟上移** = liquid-test-39；1189 passed；**当前 HEAD** |
| `dabc21b` | **memprof 内存剖析基建 + CalDAV 正则缓存化** = liquid-test-40；1189 passed；**当前 HEAD** |
| `6a2bd1b` | MemoryHints A/B 后保持默认 Performance + 实验开关 |
| `80d1c90` | **静止降帧 15fps 档 + 收起深度休眠 1Hz（宠物冻结）** = liquid-test-41；1193 passed |
| `68df96d` | **邮箱空态隐藏全部/未读透镜 + 去添加按钮上 GPU 玻璃并文字居中** = liquid-test-42-mailglass；1193 passed |
| `f3d493b` | **静止刷新率（1–30Hz）与收起深度休眠做成设置项** = liquid-test-43-idlefps；1193 passed |
| `31c7a27` | **wgpu 内存块改 Manual（提交 600→288MB、工作集 −24%）+ 深休眠裁剪工作集（→~30MB）** = liquid-test-44-memopt；1193 passed |
| `reset 7bd6bed` | **用户要求回退**：丢弃 f8b330d（白线误修）/9583bda 及工作区折腾；当前 HEAD=7bd6bed（合并+内存优化干净态）；预览版已重部署 liquid-test-45-merge65（exe `88cb962ae009bd8f`），用户 settings.txt 已恢复原值（browser_minimal=1，备份 .bak-diag）；**当前 HEAD** |

| `daozai-patches 归档退役` | **补丁链退役**：tag `custom-129` 固化正式版锚点；`daozai-patches/` 移出工作区（历史在 git），`PATCHES.md` 移到 `docs/`，上游快照移 `_archive`；HANDOFF 红线/§2/§5 改写；重放校验验证过与 `4b0b427` 一致 |
| `正式版重打包` | `git checkout custom-129` 构建双 bin + ISCC 重打 `daozai-setup-custom-129.exe`（内含 exe `e9265cc0dc617bbb`，包 `f2b7fbf1ffb12e5e`）；与部署版 `58432d96` 二进制不同（构建环境差异）但同源码；部署目录与预览目录均未动；已切回 main |
| `custom-130 正式包（第 3 版，含上游合并+白线修复）` | merge-upstream-v0665 HEAD `f8b330d` 双 bin + ISCC → `installer\daozai-setup-custom-130.exe`（11,231,232 B，包 `a39e2b61d189f517`，exe `d788d2f95f5bdbc7`/bridge `df3560011ef8a9ab`）= 液态玻璃 + 上游 v0.6.65 + 全部定制 + 内存优化 + 居中辅助移除；**未部署**；已知问题：browser_minimal=1 时收起态黑块（§7.0n 规避：装完关「自动避让」）；默认装 `C:\Program Files\Daozai`；（已被 `8a5f63d` 取代） |
| `8a5f63d` | **修 D3D12 分层窗口黑角/白线**：`opaque_surface` 时 owner 始终按岛体轮廓 `SetWindowRgn`，不退回矩形；上游分层显示窗口继续管抗锯齿 = `liquid-test-46-outline-clip`（exe `3783f00131d4ab61`，安装包 `e2f799e24bd9862c`）；1351 passed；收起/展开/展开过程实机无黑底 |
| `29a3112` | **修 D3D12 外层圆角矩形（方向不完整）**：兼容后端关掉独立模糊底板 + GPU 3px 内缩对齐 = `liquid-test-49-backup-inset`；1351 passed |
| `bcf44e8` | **真因修复：停用上游分层读回窗口**，D3D12 恢复合并前 owner 轮廓裁剪直出；岛内棋盘对照 -44 一致（圆角、无外层矩形）= `liquid-test-52-no-layered`（exe `bc04b8d12c581da4`，包 `b6d6c9db8b54c31b`）；1351 passed |
| `6475d44` | DirectComposition 透明交换链 spike：DComp visual surface + `PreMultiplied` = `liquid-test-53-dcomp`（实机发现 OpenGL 整窗不可见、D3D 圆角锯齿） |
| `4c991bb` | **收起后 20 秒回收工作集**（`WORKSET_IDLE`，不再被放歌挡掉、不等 120 秒深休眠）：WS 224MB→22MB，稳定 30MB = `liquid-test-58-fast-trim`（exe `8f61cb368954e198`，包 `162afa59339ab71c`）；1351 passed；**当前 HEAD** |
| `d0b737b` | **修设置页崩溃 + 关 D3D 岛体消失**：逐 surface 决定 alpha 模式（设置窗口是独立 surface）；DComp visual 按 HWND 复用；建窗前 DComp 预检 + 建 surface 失败就地回退；启动与重启进程都清掉 `DAOZAI_DCOMP`（继承会让 OpenGL 窗口没有 GDI 表面）= `liquid-test-57-dcomp-fallback`（exe `9fdcbdf1993b0cac`，包 `82dc760c02e3f5cf`）；1351 passed；**当前 HEAD** |
| `9c91ebb` | **DComp 全链跑通**：只在 DX12 关 GDI 表面（Vulkan/OpenGL 回退可绘制）、按 HWND 认领主窗口、补 `Commit`、`PreMultiplied` 映射到 DXGI、D3D11 取 `IDXGIDevice`、WGSL 真实 alpha + 透明清屏、DComp 下不再 `SetWindowRgn` = `liquid-test-55-dcomp-hwnd`（exe `6eb9cdad9a585a09`，包 `8b4a7d8c4fa11818`）；1351 passed；**当前 HEAD**（分支 `directcomp-spike`） |

> custom-33 ~ 119 的逐版记录见 `docs/PATCHES.md`（2963 行，按版本分节，含问题/根因/改动/验证；补丁链退役后移到 docs）。
> custom-36（性能与安全六项）、custom-37~39（阴影与角部突起，误判两版才定位到悬浮云路径）、
> custom-40~42（动效与悬停延时）的完整复盘见 `_archive/HANDOFF-2026-11-15.md` 后半部分。

---

## 9. 接手注意（操作层面）

1. **布局改动前先 grep 测试坐标**：`grep -n "H::local(" dz-phone/src/ui_tests.rs`。
   新增热区盖住已有点击点，测试会以「点了没反应」失败，从报错看不出原因。
2. **视觉问题先问「哪些模式有、哪些没有」**，差异点直接指向代码分支，
   比反复调参数有效（custom-37/38 浪费两版在共用阴影上）。
3. **判断锯齿不要看缩略图**：沿边缘打扫描线打印亮度序列，单像素 Δ>100 才是硬边。
4. **判断卡顿先分清掉帧还是没重绘**：看日志 `tick=` 增长速度，静止 6 秒增长 ≈360 才是 60fps。
5. **实机打开设置窗口要先把岛保持展开**：`ui_probe.py hover` 会先离开导致收起；
   要在一个 Python 进程里先 `SetCursorPos(1000,30)` 等展开，再移到 `(1240,38)` 点击。
   翻页右箭头约 `(1240,110)`，「剪贴板」分栏约 `(784,26)`。
6. **设置窗口滚轮事件容易被 WorkBuddy 前台抢走**，验证配置优先改 `settings.txt` + 重启。
7. **测岛宽度**用 `ImageGrab.grab(bbox=...)` 抓小区域 + 多行取最长深色段；
   测前先把光标移开静置 ≥1.5s 并截图确认岛真的收起，基线每次现场取。
8. **部署脚本判断备份已存在时不要覆盖**，否则会把上一版原始 exe 冲掉。
9. **agent 工具直接启动的 GUI 进程会被会话清理**；要它活着就双击 `.bat`，
   或在同一个进程内用 `subprocess.Popen(creationflags=0x08000000|0x0200)` 并就地截图。
10. **由岛仔（用户手动启动的常驻进程）派生的子进程不受会话清理影响** —— 这条只针对 agent 直接启动的 GUI。

### 9.1 方法论（比具体修法更耐用）

- **改参数前先量**。很多「感觉」是错的：白边看起来「模糊加多了」，实际模糊只有 1.5px，
  真凶是 8px 的高斯白边。
- **抓帧比推理可靠**。形变增强「完全没生效」是靠抓帧发现「progress 一帧就到 1」。
- **测量条件必须固定**。壁纸是动态场景壁纸，每次启动冻结到的帧不同，背景亮度会在
  60~250 之间跳；比较时必须归一化到背景亮度，或者用同一帧。
- **写进日志的结论要可复现**。每个改动都留「实测 X → Y」的数字，下次接手不用重新摸。

---

## 附录 A：常用命令速查

```bash
# 定位项目
python <skill>/scripts/locate_project.py

# WGSL 语法校验（秒级，先跑这个再花 3 分钟编译）
python -c "import io,re,pathlib;t=io.open('dz-phone/src/gpu_glass.rs',encoding='utf-8').read();\
s=re.search(r'const SHADER: &str = r#\"(.*?)\"#;',t,re.S).group(1);\
pathlib.Path('C:/Users/cay/AppData/Local/Temp/wgsl-check/shader.wgsl').write_text(s,encoding='utf-8')"
cd /c/Users/cay/AppData/Local/Temp/wgsl-check && MSYS_NO_PATHCONV=1 cargo run --quiet

# ~~补丁链重放校验~~（链已退役，此步作废）

# 部署到预览目录（必须先停旧实例，否则文件占用）
Get-CimInstance Win32_Process -Filter "name='daozai.exe'" | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
Start-Sleep -Seconds 3
Copy-Item D:\code\daozai-custom\dz-phone\target\release\daozai.exe `
  "C:\Users\cay\Desktop\Daozai-液态玻璃预览\daozai.exe" -Force

# 实机验证
python <skill>/scripts/ui_probe.py shot --out top.png --crop 0,0,2048,260
python <skill>/scripts/ui_probe.py hover --x 1325 --y 24 --wait 1.4
python <skill>/scripts/ui_probe.py click --x 1293 --y 72
```

**测帧率**：光标先停远处确认静止 → 读日志最后一条 `perf: tick=N` → `sleep(20)` → 再读 → 差值 / 20。

**验证色散链路**：`DAOZAI_LIQUID_FLAT=2` 棋盘模式下，同一场景把 `liquid_chroma_px` 写 0 和 20
各启动一次，两张截图相减 —— 差异只出现在玻璃边缘（红/蓝两条带）就说明链路通。
普通模式下动态壁纸每次启动抓到的帧不同，跨进程 A/B 会被背景变化污染。

---

## 附录 B：调试开关

| 变量 | 作用 |
|---|---|
| `DAOZAI_DATA_DIR=<dir>` | **必带**，否则读写 `%APPDATA%\Daozai` 的正式设置 |
| `DAOZAI_LIQUID_FREEZE_BG=1` | 冻结背景 + 解除捕获排除（截图/录屏可见），预热 24 帧后自动切 |
| `DAOZAI_LIQUID_FLAT=1` | 玻璃内原样输出背景（查抓屏对齐） |
| `DAOZAI_LIQUID_FLAT=2` | 20px 网格 + AA 细线（查折射位移/色散） |
| `DAOZAI_LIQUID_FORCE_LUMA=0.2` | 强制背景亮度（验证深色材质） |
| `DAOZAI_LIQUID_NO_EXCLUDE=1` | 关闭 WDA 排除 |
| `DAOZAI_LIQUID_DUMP=1` | 前 8 帧写 PPM |
| `DAOZAI_LIQUID_MORPH_FORCE=1` | 把 morph 钉成常数，做确定性 A/B |
| `DAOZAI_LIQUID_CAPTURE_EVERY=<1~4>` | 抓屏节流，默认 1；设 2 省 34% CPU |
| `DAOZAI_SETTLE_MS=<毫秒>` | 覆盖切换显示模式后的稳定期（优先级最高，0~30000）。不设则用自学习值（首次 500） |

液态玻璃设置项（`settings.txt`，界面在「外观 → 大小」下方，仅液态皮肤显示）：

| 设置项 | key | 范围 | 默认 |
|---|---|---|---|
| 折射强度 | `liquid_refract_px` | 0~120 | 48 |
| 色散强度 | `liquid_chroma_px` | 0~30 | 8 |
| 内部模糊 | `liquid_blur_px` | 0~4 | 1.5 |
| 折射带宽 | `liquid_band_px` | 10~140 | 87 |
| 白边宽度 | `liquid_edge_w_px` | 0~4 | 1.5 |
| 白边亮度 | `liquid_edge_op` | 0~1 | 0.90 |
| 形变强度 | `liquid_morph` | 0~3 | 1.0 |
| **优先独立显卡** | `liquid_prefer_dgpu` | 0/1 | 1（开） |

> `liquid_prefer_dgpu` 是**启动期**生效（wgpu 适配器只选一次），改后必须重启岛仔。
>
> 稳定期学习值存在 `data\diagnostics\settle-learned.txt`（纯数字，毫秒）。删掉它即回到 500ms 重新学。
> 关掉 = 回到 eframe/wgpu 默认策略（混合显卡机器可能落核显，省电但更占内存）。

日志关键字（`data\diagnostics\liquid-glass.log`）：

```
wgc: 已关闭系统捕获黄框          ← SetIsBorderRequired(false) 成功
wgc: init OK monitor=(0,0 2560x1440)
capture: src=wgc inner=77 outer=70   ← 两个亮度接近才正常
capture: 丢弃黑岛形帧 inner=..        ← 竞态保护触发（偶发正常）
plate: new 1024x512 at (..) frozen=false inner=77
exclude: 冻结模式预热完成，已取消排除（截图可见）
```

---

## 附录 C：参考与致谢

- `LiquidGlass-Pack-v2`（画心人 · MIT）—— 折射算法与参数表来源，`shader/glass.frag` 是主要参考。
  解压位置：`C:\Users\cay\WorkBuddy\2026-11-13-22-38-27\liquid-pack\LiquidGlass-Pack-v2\`
- 逐轮实验记录（比本文档更细）：`C:\Users\cay\WorkBuddy\2026-11-13-22-38-27\.workbuddy\memory\2026-11-15.md`
- 主线逐版记录：`docs\PATCHES.md`（补丁链退役后移到 docs）
- 本工作区日志：`C:\Users\cay\WorkBuddy\2026-11-11-23-14-32\.workbuddy\memory\`
