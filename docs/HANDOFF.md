# 岛仔（Daozai）交接文档

> **最后更新**：2026-11-25 01:56
> **本文档是唯一权威交接入口**（`C:\Users\cay\Desktop\HANDOFF.md`）。
> 仓库内同步副本：`D:\code\daozai-custom\docs\HANDOFF.md`；凌晨版原件在
> `Daozai-液态玻璃预览\_archive\交接文档-2026-11-16.md`。

---

## 0. 30 秒速览

| 项目 | 当前值 |
|---|---|
| **egui pin（2026-11-25）** | `90431d979ab1faae2de6d9f736e9d33ca0d31956`（提交 `ec19bd1`，0.37.0，1412 passed；未打包部署） |
| **文字缩放（2026-11-25）** | 静态倍率走 `TextScale::Reflow` 重排缓存（15 处）；动画文字恒字号、`inset_text_uv` 已删；1413 passed，未部署 |
| 正式版（日常使用） | `D:\software\Daozai` 仍跑 custom-129（exe `58432d96c07d9d0e`）。**custom-134 已打包、未部署（最新，2026-11-19）**：`installer\daozai-setup-custom-134.exe`（11,109,904 B，包 sha `1f2f9df72d88b08d`，0.6.66，内含 exe `2ebc1313b2241ad6` + bridge `9832e44f133f5ac0`，构建自 main `912d636`= 在 950433b 基础上仅更新版本号；1373 passed；静默安装验证通过，详见 §7.0ad）。**custom-133**：`installer\daozai-setup-custom-133.exe`（11,241,513 B，包 sha `a9455e0ad655bb9f`，内含 exe `02786373d87ba563` + bridge `f1ae28626c9db18e`，构建自 main 工作树（告警清理 `2ea229a` + 液态玻璃按钮缩小 + 关配置弹窗收起 + INSTALL-NOTES 反馈日志说明，提交 `14bb37f`）= 在 custom-132 基础上：液态玻璃控制按钮略微缩小避开分页指示条、安装说明新增反馈日志清单；**custom-132 已打包、未部署**：：`installer\daozai-setup-custom-132.exe`（11,241,512 B，包 sha `6afb7ff8c6b5039d`，内含 exe `e342ccf062574afc` + bridge `1da2199b59ef5653`，构建自 main 工作树 = liquid_shadow 开关 + 液态玻璃参数弹窗（去重，仅 D3D12 兼容渲染开启时显示）+ 折叠三角指示 + phone-apps 移除内置种子）；**custom-131 已打包、未部署**：`installer\daozai-setup-custom-131.exe`（11,238,544 B，包 sha `c46e227a3eabf850`，内含 exe `74b854afddc88c5c` + bridge `1da2199b59ef5653`，构建自 `294d483` = 液态玻璃 DirectComposition 透明交换链 + 静置 20s 回收工作集 + 告警清理；**未部署，装前请先确认 DComp 路径在目标机可用**）；**custom-130 测试包（含上游合并）已生成未部署**：`installer\daozai-setup-custom-130.exe`（11,234,872 B，包 sha `bdb59ab5109d81e1`，内含 exe `5ebee4d9eb20c5fa` + bridge `5d81fa3a6f66703f`，构建自 merge-upstream-v0665 HEAD `2e4b64b` = 液态玻璃 + 上游 v0.6.65 + 全部定制 + 描述符堆优化；已覆盖 17:18 的合并前旧包，旧包 sha `7e469dd5db85cd7f` 见 git 历史）；默认安装 `C:\Program Files\Daozai`（admin），老 HKCU 卸载条目与 D:\software\Daozai 装完需手动清理；用户数据 `%APPDATA%\Daozai` 不受影响，装完需手动选液态玻璃皮肤；本轮 D3D12 轮廓裁剪修复只进预览（`liquid-test-46`），custom-130 包未重打 |
| 液态玻璃源码 | 分支 `directcomp-spike` 与 `main` 已于 2026-11-18 10:44 快进合并同步，HEAD = `21b55ee`（新增 liquid_shadow 岛体投影开关；该功能**尚不在任何安装包里**，custom-131 早于它） |
| **工作树** | **干净**；HEAD = `2342d3c`，检出分支 = `merge-v0666-selective`（v0.6.66 选择性合并，见 §7.0x）；`main` 仍在 `6a25af7`。`2342d3c` = 液态玻璃配置移入「外观」页 + 修「配置液态玻璃」按钮无反应 + 弹窗内切收起/展开态（见 §7.0aa），release exe `dfd191cc498aed8c8799c42ca822f4`（未部署，PR 用源码改动）。custom-133 包已生成（见 §7.0w，指纹被后续文案/合并更新，见 §7.0x）。**【2026-11-21 16:00 更新】当前活跃分支=`research/egui035-migration`（egui 0.35 迁移研究线），HEAD=`6ca92e0`，本行上方旧值（2342d3c/merge-v0666-selective）是主线历史快照、非当前检出；研究分支 release exe sha `60ca4a035ec5eb10`（高 DPI 背景放大修复后，未部署）。详见 §7.0zzg.13。** **【2026-11-22 11:45 更新】同一研究分支 HEAD=`ccff129`（修设置大小/刘海形状/非液态皮肤玻璃残影，见 §7.0zzn），release exe sha `0f2dec64b11eb4c1`，全量 1389 passed；未部署。** **【2026-11-22 15:40 更新】同一研究分支 HEAD=`20d9f6a`，当前 release exe sha `711166f844695b3c`（target/release + 桌面预览目录，已启动 PID 37788）：修刘海展开玻璃铺不满——上轮改 notch_sdf 圆角时 qy 写成 abs(p.y)+(b.y-r)，底部整条判为岛体外致玻璃透明；改 -(p.y)-(b.y-r) 后铺满（数值验证）。同时删掉已移到 shader 的死代码 shape::shadow_ring + 测试。light_follow 测试通过。** **【2026-11-22 21:05 更新】同一研究分支 HEAD=`83459b6`（文字缩放 6% 死区，隔离阻尼比尾振，见 §7.0zzp），release exe sha `136be0d614d9eddec2`（已启动 PID 15688），全量 1391 passed / 0 failed。** **【2026-11-22 20:40 更新】同一研究分支 HEAD=`7cf3b9f`（分栏容器改高斯模糊磨砂、去暗底，见 §7.0zzp），release exe sha `c6b38aea44220ef9`（已启动 PID 21532）；全量 1391 passed / 0 failed / 131 ignored。** **【15:18 历史】HEAD=`81099ba`，exe `560394fee86cb218`（`D:\code\daozai-custom\dz-phone\target\release\daozai.exe` + 桌面预览目录 daozai.exe，已启动）：对照迁移前 5f5b428 找到两个真正根因并修复——① 删除 paint_gpu_glass 里迁移时多加的 rect_stroke 圆角白边（与 shader 刘海轮廓双影不重叠）；② 投影从 egui 移进 HLSL（D3D11 先玻璃后 egui，egui 阴影必然盖住玻璃；迁移前 wgpu 顺序相反才没事），用 SDF 高斯衰减画外阴影，set_shadow→lightcfg.z 传开关。此前 c263a3d/855f56c/c0830c6 修了刘海圆角方向、左右停靠控件 ppp、投影去肩部。教训：迁移 bug 必须先 git 对照迁移前完整绘制流程（含顺序），别自己推导。全量 1392 passed / 2 failed（messenger 基线 flaky）/ 131 ignored；未再全测。早前安装包 daozai-liquid-setup-63 已过时勿用。**|
| 预览版（液态玻璃试验） | 桌面预览目录 `daozai-liquid-setup.exe` = **`liquid-test-62-two-modes`（包 sha `ddc1516892d83117`，9,295,747 B；内含 exe `91424b6863533eca` + bridge `859a91518af04c91`；安装到 `%LOCALAPPDATA%\Programs\DaozaiLiquidTest`，独立数据目录）**，**2026-11-21 00:02 打包，待实机验证**。构建自 main `7200ee7`（`fix/wgc-recreate` 合并）：取消 GDI 实验、只保留零拷贝/非零拷贝两模式（`liquid_gpu_share` 控制）、去掉 EVENT query 非法 Begin、device-lost 冷却退避。换分辨率问题实测用非零拷贝（`liquid_gpu_share=0`）解决；零拷贝独有的 event query 风暴保留此前修复。编译通过、全量测试未跑（用户要求）。此前 -61（彝语字体）、-60 等见 `_archive/README.md` 指纹表 |
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

### 7.0ac 彝语歌词支持（2026-11-18 19:37–19:50，提交 `ce4a34f`）

用户反馈「彝语歌不行」。诊断：彝文音节 U+A000–A4CF 在 Segoe UI / 雅黑 / Malgun / 喜马拉雅体里**都没有字形**，歌词显示成方框——不是 RTL 方向问题（彝文是 LTR，不需连写）。
Windows 自带 `C:\Windows\Fonts\msyi.ttf`（Microsoft Yi Baiti），实测 cmap 覆盖 U+A000–A48C 音节 + U+A490–A4C6 部首（用 cmap format4 段直接核对过）。
修法：和藏文喜马拉雅体完全同构——加进 `theme.rs` 的字体回退链（`yi` key，三档字重都加），缺字体自动跳过；`rtl_text::display()` 对无阿拉伯字母文本 `Cow::Borrowed` 直接返回，排版链路零改动。
测试 `yi_syllables_have_glyphs_in_all_weights`（抽样 A000/A185/A4C6/A490，三档字重；测试机无该字体则跳过）；RTL 10 测试 + 韩文/彝文字形测试全过；全量 **1373 passed / 0 failed / 132 ignored**。
打包 `liquid-test-61-yi-lyrics`：包 `97a615f06917c561`（11,005,199 B）、exe `7b1e8d804011cce0`；桌面 `daozai-liquid-setup.exe` = 该包，-60 已移入 `_archive`。

### 7.0ad 打正式包 custom-134（2026-11-19 00:13–00:25，提交 `912d636`）

用户要求在本地定制版仓库打正式包。`D:\code\daozai-custom` main 工作树 HEAD 原为 `950433b`（干净），iss 版本 old=0.6.65/custom-133 → 新 **0.6.66 / custom-134**（版本跟上 PR 侧，构建号顺延）。
- 全量测试（定制版）：**1373 passed / 0 failed / 132 ignored**（比 PR 侧 1293 多，定制功能更多的预期）。
- 双 bin release：`daozai` exe `2ebc1313b2241ad6`（22,539,776 B）、`daozai-bridge` exe `9832e44f133f5ac0`（378,880 B）。
- ISCC（Inno Setup 7.1.1，`D:\software\Inno Setup 7\ISCC.exe`）编译成功（HKCU/admin 警告是历史固有、无害）；包 `installer\daozai-setup-custom-134.exe` = **11,109,904 B、sha256 前16 `1f2f9df72d88b08d`**。
- 静默安装验证：`/DIR=D:\code\_custom134_verify` 退出码 0、无 Runtime error、`Installation process succeeded`；产物含主 exe / bridge / extension / com.daozai.island.json / CUSTOM_BUILD.txt / INSTALL-NOTES；验证后卸载 + 清目录。
- **只打包未部署**：`D:\software\Daozai` 日常部署版未动（仍是旧版）。custom-133 包仍在 installer 目录（未删）。
- 提交 `912d636`（只改 iss 版本号），工作树干净；未推送 origin/main。

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

### 7.0zzn 【已修复】设置大小/液态形状失效 + 非液态皮肤残留玻璃（2026-11-22 11:30–11:45，提交 `ccff129`）

- **症状**：设置里的「大小」滑块无效；液态玻璃形状切换无效，只有胶囊没有刘海；切到悬浮云/毛玻璃后背景里仍有一块收起大小的液态折射。
- **根因（都是 egui 0.35 自管 winit 栈迁移时的参数断线）**：
  1. `Settings::island_scale()` 旧路径走 eframe `set_root_viewport_scale`，迁移后只剩 `// migration: viewport scale handled by new stack` 注释，从未实际下发缩放；
  2. `paint_gpu_glass` 把 `flare/flare_h` 写成 `_flare/_flare_h` 直接忽略，`gpu_glass_stub::set_shape` 无任何调用，`build_glass_params` 里 `p.notch.z` 恒为 0 → HLSL 永远走圆角矩形（胶囊）SDF，不会进 `notch_sdf`；
  3. `gpu_glass::set_body` 只把 `active` 置 true，切到非液态皮肤时没有清零 → 渲染循环继续拿最后一次（往往是收起态）的 `body_rect` 画折射。
- **修复**：
  - `main.rs`：启动即 `egui_ctx.set_zoom_factor(island_scale())`；每帧检测目标缩放变化后下发，用 `applied_zoom` 避免重复重排。
  - `app.rs`：`paint_gpu_glass` 按 `settings.island_shape` 发布 `[flare, flare_h, is_notch]`；非液态皮肤每帧 `set_active(false)`。
  - `gpu_glass_stub.rs`：新增 `set_active`，`set_shape` 带刘海标志，新增 2 个单元测试。
  - `winit_runner.rs`：玻璃 pass 同时按「当前皮肤是 Liquid」和 `u.active` gate；shape 参数按 ppp 转物理像素写入 `p.notch`。
- **验证**：`cargo check --all-targets --locked` 全绿；全量 `cargo test --locked --release --lib -- --test-threads=1` = **1389 passed / 0 failed / 131 ignored**；release exe sha `0f2dec64b11eb4c1`。未部署、未打包，main 不动。

## 8. 版本时间线

| 版本 / 提交 | 内容 |
|---|---|
| `ce4a34f` | **彝语歌词支持**：加 Microsoft Yi Baiti（msyi.ttf）字体回退，彝文音节 U+A000–A4CF 不再显示成方框；LTR 无需连写；1373 passed；`liquid-test-61-yi-lyrics`（exe `7b1e8d804011cce0`），已部署桌面 |
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

### 7.0ae 恢复网易云快捷方式卡片 + 「自定义与集成」页重组（2026-11-19 13:06–13:20，提交 `64c4496`）

用户反馈：合并上游后在设置页找不到「修复网易云快捷方式」的入口。
- **追根因**：卡片原在音乐页（旧 `extras.rs` tab 1），标题「网易云播放进度条修复」（含状态显示 + 选择快捷方式按钮）；在 `12a9329`「移植上游 v0.6.66 设置页重组（保留全部本地定制）」重写 `extras.rs` 时被整块删除——提交信息说"保留全部本地定制"但实际漏了这张卡。底层 `netease_control.rs` 一直在 main 上、启动自动扫描修复仍在跑，只是 UI 入口丢了。（`pr/netease-cdp-seek` 分支上有另一张类似的 CDP 进度条卡片，不是原来那张。）
- **修复**：把卡片加回「自定义与集成」页；新增 `netease_control::fix_all_shortcuts()`（包装私有 `ensure_shortcut`，同步修全部、返回条数）供"修复快捷方式"按钮用。
- **顺便重组该页**：按"跟谁集成"分组，相关卡片挨在一起——网易云音乐 → 手机（连接状态 + 手机功能）→ 浏览器（安装 + 下载提示 + 工作原理）→ MAA（状态 + 上岛提醒 + 路径）；每组前加组标题（复用带蓝色竖条的 `settings_label`），手机组加一句说明两项如何配合。未用折叠 `settings_section`（默认折叠反而让用户找不到内容）。
- 验证：`cargo check --all-targets` 通过；全量 **1373 passed / 0 failed / 132 ignored**。未打包、未部署、未推送。

### 7.0af 打并安装 custom-135 正式包（2026-11-19 13:19–13:26，提交见下）

用户要求打正式包并安装到 `D:\software\Daozai`。决策：版本 custom-135（0.6.66），装到日常部署目录。
- 打包前确认：`D:\software\Daozai` **原本不存在**（HANDOFF 旧记录说跑 custom-129，实际已被清理过，常见位置/注册表均无岛仔安装）——无需备份，直接装。
- 基础源码 main `39cf9d3`（含 §7.0ae 网易云卡片恢复 + 设置页重组），全量测试 1373 passed（§7.0ae 已验证，复用）。
- 提交 `88f163c`（iss 版本 0.6.65→0.6.66 保持、custom-134→**custom-135**）。
- 双 bin release：exe `c66336a6c0cb10a7`（22,526,976 B）、bridge `9ee551099d3f8871`（376,320 B）。
- 包 `installer\daozai-setup-custom-135.exe` = 11,111,784 B、sha256 前16 `0f1feadfe4cc3415`。
- **已安装到 `D:\software\Daozai`**：`/VERYSILENT /DIR=D:\software\Daozai`，退出码 0、无 Runtime error；产物含主 exe / bridge / extension / CUSTOM_BUILD.txt / INSTALL-NOTES；`CUSTOM_BUILD.txt`=custom-135、exe/bridge 指纹与构建一致；HKCU Run 启动项 `D:\software\Daozai\daozai.exe` 已写入。日志已清理。
- 自定义玻璃皮肤需在设置里手动选（官方默认经典）。

### 7.0ag 自有发布线 v1.0.0 + GitHub release（2026-11-19 14:10–14:40，提交 `c80ac50`/`f15b9a7`）

用户要求：版本从 1.0.0 起、发布到自己的 releases（像上游那样）、更新检查改指本仓库、关于页去捐赠保留上游介绍。
- **源码改动**（提交 `c80ac50`）：`Cargo.toml` 0.6.66→**1.0.0**；`updates.rs` REPOSITORY/API/RELEASES 全改 `cayww/daozai-custom`，按钮文案「检查更新」→「检查更新我的仓库」，去掉指向别人网盘的 BACKUP_DOWNLOAD 和「网盘下载」入口；关于页 `support_author`（微信/支付宝二维码）→ `about_upstream`（原仓库链接 + 简短介绍），连带删 `release_support_codes`/`support_code_image` 及二维码测试、旧预览测试。提交 `f15b9a7` 把 Cargo.toml 的 `repository` 也改成 `cayww/daozai-custom`（元数据，不进二进制）。
- 验证：`cargo check --all-targets` 通过、全量 **1372 passed / 0 failed / 131 ignored**（比 1373 少 1 是删了旧预览测试）。
- **NSIS 包**：把上游 `setup.nsi` 复制进 `installer/`（PROJECT_DIR 要传 `D:\code\daozai-custom\dz-phone` 才有 assets/LICENSE/THIRD_PARTY_NOTICES），`/INPUTCHARSET UTF8` + `/DAPP_VERSION=1.0.0`；安装器 exe `2df39c291a7d6cec`（FileVersion 1.0.0）；包 `installer\Daozai-v1.0.0-windows-x64.exe` sha `19d22dede8a86f58`（9,121,464 B）。
- **GitHub release**：tag **`v1.0.0`**（用户要求像上游那样用版本号），name「岛仔 v1.0.0（自定义发布线）」，id 450962032，页面 https://github.com/cayww/daozai-custom/releases/tag/v1.0.0 ，body=「与上游不同的部分」清单（用户指定）。
- **asset 两个**（对齐上游发布惯例 + 更新模块）：`Daozai-Setup-v1.0.0-x64.exe`（NSIS 安装器，sha 19d22ded）+ `Daozai-v1.0.0-windows-x64.zip`（便携 zip，内含 `Daozai-v1.0.0-windows-x64/daozai.exe`=2df39c29，sha 4f277493，11,245,228 B）。**zip 是关键**：`updates.rs::release_info` 硬编码期望 `Daozai-v{ver}-windows-x64.zip` + URL + sha256 digest，只有它匹配才能让「检查更新我的仓库 → 下载并安装」真正工作；已用 API 验证匹配通过。
- 坑：从 Bash 启动系统脚本加载器被安全策略拦（改用专门的命令工具）、Bash 的 /tmp 与 Python 路径不互通（用 C:/Users/.../AppData/Local/Temp）。
- 未打包 Inno custom-136；日常安装仍 custom-135（§7.0af）。未推送 origin/main。

### 7.0ah 清理 liquid-test 目录（2026-11-19 14:41，提交 `ad23629`）

用户确认液态玻璃测试已完成，不再需要测试内容。删掉 `installer/liquid-test/` 整个目录：6 个追踪文件（iss / 两个启动 cmd / 测试说明 / 反馈与卸载 / .gitignore）+ 57 个未追踪测试 exe，合计 **519MB**。删前确认全仓库无源码/脚本引用该目录。追踪文件可从 git 历史恢复；测试 exe 是构建产物。提交 `ad23629`。installer 目录现只剩正式发布相关（daozai.iss / setup.nsi / INSTALL-NOTES.txt / .gitignore + 各正式包 exe）。

### 7.0ai 修复更新按钮 + 镜像自动下载 + 设置页图标（2026-11-19 14:50–15:10，提交 `61954e4`）

用户反馈：v1.0.0 版本页只显示「更新检查已关闭 · 自定义构建保护」、没有按钮；且国内用户难访问 GitHub，要求配置可用镜像自动侦测；另要求换「自定义与集成」图标。
- **根因**：`Settings::default()` 里 `update_check: false`（自定义构建历史默认，当初防官方更新覆盖本地改动）；现在已是自有 release 线，应默认开。改 `model.rs` 默认 true + 注释；关于页加 `setting_row` 开关（旧配置 update_check=0 的用户可自行开启）；`updates.ui()` 去掉 `!enabled` 时直接 return 的逻辑——关掉自动检查仍保留手动按钮，参数改 `_enabled`。
- **镜像**：实测 `ghfast.top` / `gh-proxy.com` / `ghproxy.net` 三个能真实下载 v1.0.0 asset（Range 请求前 4 字节均为 `4d5a9000`=MZ exe 头；gh.llkk.cc / ghproxy.cc 超时）。`updates.rs` 加 `MIRRORS` 常量 + `probe_source()`（4 字节 MZ 校验、记录耗时）+ `pick_download_url()`（4 线程并发探测三个镜像+直连，最多等 4s，选最快；全失败直连兜底）；`download()` 用它替换 `package.url`。**安全**：探测只是一层前缀，下载后原 sha256 校验不变，镜像篡改过不了。清理了指向别人网盘的 BACKUP_DOWNLOAD 和残留文案。
- **图标**：`SETTINGS_TAB_CUSTOM=15` 原来落到 `_` 用 `about.svg`；改成 `browser.svg`（这页是浏览器/手机/MAA/网易云集成）。
- 验证 `check --all-targets` 通过、全量 **1372 passed / 0 failed / 131 ignored**。
- **覆盖 v1.0.0 asset**（用户选不升 v1.0.1）：重构建 exe `0bb5b14b628bbfa9`、新 NSIS 安装器 sha `4c7dae0773cc8fb8`（9,124,200 B）、新 zip sha `28527d058abf44bd`（11,252,934 B，内含 exe=0bb5b14b）。删旧 asset（id 766547390/766550691）后重传同名；API 验证更新链路仍匹配（asset 名/URL/size/sha256 digest 齐全）。
- 注：已装 v1.0.0 的用户不会收到"新版本"提示（版本号没变），但重新下载得到的是修复版。未推送 origin/main。

### 7.0aj 网易云残留进程诊断 + 退出时提示（2026-11-19 15:19–15:38，提交 `66337d6`）

用户反馈：网易云从托盘退出后再启动起不来，截图显示残留 2×cloudmusic.exe + 1×cloudmusic_reporter.exe，怀疑是岛仔改快捷方式加 debug 参数导致。
- **实测诊断**：两个 cloudmusic.exe 命令行都是 `"D:\software\CloudMusic\cloudmusic.exe" --remote-debugging-port=9222`（15:17:26 与 15:17:47 启动，同一父进程），但 **9222 端口无监听** = 僵死残留。岛仔代码只改 .lnk 参数 + 作为 CDP **客户端**连接端口，不启动/不杀网易云进程。结论：网易云"托盘退出不干净"是它自身固有行为（不带 debug 也有），debug 参数放大了后果（残留实例占调试端口 → 新实例启动异常）。用户猜测基本对，是诱因非唯一根因。
- **立即处理**：复核 PID 26312/6580/22472 身份后仅结束这三个残留进程（未动其他程序），确认网易云进程清空。
- **代码方案**（关键：用户指出"岛仔开机自启时，启动一次性检测没用"→ 改事件驱动）：挂钩 `app.rs::update_media()` 里「SMTC 会话消失」分支（就是歌词/音乐页自动关闭的地方），仅当退出前 `self.media.source` 是 `cloudmusic.exe` 时调用检测；`netease_control.rs` 新增 `stale_instance_warning()` 用 Toolhelp 快照数 cloudmusic.exe 主进程，≥2 判为残留并提示（15 分钟冷却，cfg!(test) 直接返回 false 不影响 UI 测试）。**只提示，不自动杀进程**。此方案不依赖岛仔/网易云谁先启动。
- 验证：check --all-targets + 全量 1372 passed。提交 66337d6。
- **推送 + 覆盖 v1.0.0**（用户要求，15:37–15:43）：`git push origin main` 成功（70f12de→15345be，2 提交）。重构建 exe `74f83162af68bd1b`、NSIS 安装器 sha `1abd184ca1767647`（9,125,779 B）、zip sha `54837b94ba0ee90f`（11,255,959 B，内含 exe=74f83162）。删旧 asset（766637577/766637740）后同名重传；API 验证 zip 仍符合更新模块（名字/URL/size/sha256）。发布页 https://github.com/cayww/daozai-custom/releases/tag/v1.0.0 。本地与 origin/main 均 15345be，工作树干净。

### 7.0ak 融合 e992b31 截图省 CPU 补丁到定制版 vendor（2026-11-19 15:47–16:00，提交 `700e4d4`）

用户问 PR 文案「vendor 更新到官方最新」与定制版差异、哪个更好、要求融合以"性能优化到最佳"。
- **盘点结论（纠正记忆）**：两边 vendor 版本完全相同（eframe/egui/egui-wgpu/egui-winit 0.31.1、wgpu-hal 24.0.4），差异不是版本而是**各自补丁不同**。关键性能补丁归属：`e992b31`（截图读回省 CPU 约四成，**上游/PR 有、custom 无**）；`cf76bbb`（自适应帧率静止30/动时60 + eframe Poll 补丁，**custom 有、上游/PR 无**）；`9c91ebb`（DComp 透明交换链，custom 有）；`beda038`（液态主体，PR 有）。两边各有对方没有的优化。
- **不整体 cherry-pick**：`e992b31` 是混合提交（引入 egui-wgpu vendor + 改 layered.rs/main.rs/resources.rs 应用层 + Cargo.lock），与 custom 独立的 DComp 线（`6475d44`→`9c91ebb`）和 `cf76bbb` 会冲突。改为**手工移植三处核心补丁**（与 DComp 无关、可共存）：
  1. `vendor/egui-wgpu/src/capture.rs`：`ctx.request_repaint()` → `request_repaint_after(1ms)`（截图交付只多画一帧，原逻辑连画两帧）；
  2. `vendor/egui/src/context.rs`：`send_viewport_cmd_to` 里 Screenshot 命令且是本 viewport 时不 `request_repaint_of`；
  3. `vendor/egui/src/input_state/mod.rs`：`wants_repaint_after` 里 Screenshot 事件不算用户输入（其余事件仍触发立即重绘）。
- **收益边界（重要，别高估）**：三处补丁只作用于**兼容渲染（render_compat）截图读回**路径。液态皮肤走 DComp/WGC 不经过读回、经典皮肤 OpenGL 下 `opaque_surface=false` 本就不走读回——**只在 D3D12 + 经典皮肤 + 兼容渲染**组合下有收益。custom 应用层 `schedule_repaint`+`glass_tick`（cf76bbb）已解决大部分帧率问题，此次是补齐 vendor 层、让两边优化全集重合。
- 验证：`cargo check --all-targets` + 全量 **1372 passed / 0 failed**。diff 仅 3 文件 / 10 行插入 / 3 行删除。提交 700e4d4。
- **推送 + 覆盖 v1.0.0**（用户要求，16:05–16:12）：`git push origin main` 成功（4bc70ed→2af861f，2 提交）。重构建 exe `5533ba64c55997a7`（本次 cargo 确实重编了 egui——日志出现 `egui (lib) generated 31 warnings`，证明 vendor 补丁进二进制）。NSIS 安装器 sha `8cf6999e2c14c13d`（9,126,102 B）、zip sha `7469cb4c8a8f3edf`（11,256,143 B，内含 exe=5533ba64）。删旧 asset（766734127/766734323）后同名重传；API 验证 zip 仍符合更新模块（名字/URL/size/sha256）。本地与 origin 均 2af861f，工作树干净。

### 7.0zb 主题取样越界卡退修复 + 单测（2026-11-19 22:00–22:30，分支 `spike/wgc-gpu-share`，未提交）
- **问题**：零拷贝抓屏（`liquid_gpu_share=1`）路径下，主题亮度取样此前复用随抓屏变化的 staging，尺寸变化时读回宽度可能超出分配内存导致越界卡退。上一个 agent 已改出修复方案（独立固定 1024×256 staging + 源 box 裁剪到帧内 + pitch 校验 + 失败静默），但卡退过，本轮谨慎接管。
- **本轮改动**（`wgc.rs` + `gpu_glass.rs`，工作树未提交）：
  - 把边界计算从 `sample_theme_luma` 抽成纯函数 `theme_sample_box`（i64 中间运算防溢出，rw/rh 夹到 1024×256，源 box 必须在 `[0,sw)×[0,sh)`）；`sample_theme_luma` 改为调用它。
  - 新增 `#[cfg(test)] mod theme_sample_tests` 9 个单测（正常/原点/夹取/越界/负坐标/退化/极端i32/贴边/多尺寸不变量），锁定「尺寸变化绝不越界」。
- **验证**：新单测 9 passed；全量 **1381 passed / 0 failed / 131 ignored**（修复前 1372，+9 即新单测）。rebuild release exe `4873119d7f454121`（22,253,056 B）。
- **踩坑**：前两个单测失败是测试期望写错——夹取先生效，写「越界」用例必须保证夹取后仍越界（起点 1500+夹到1024=2524>1920）。
- **速度答疑**：测试本身恒 95s；另一 agent 的 6 分钟=冷编译(2m51s实测)+测试91s+后台轮询。本轮热缓存命中故快。
- **部署预览目录**（`C:\Users\cay\Desktop\Daozai-液态玻璃预览\`，未碰正式目录）：桌面预览目录本无运行时 exe（bat 指向的 %LOCALAPPDATA% 运行时目录不存在）→ 按用户选择直接放裸 exe：新 exe 复制到该目录（原无同名无需备份）+ 新建 `启动（主题取样防崩溃验证）.bat`（`DAOZAI_DATA_DIR=%~dp0data`，不碰正式版）。data/CUSTOM_BUILD.txt 标记 `liquid-test-41-lowsleep`→`liquid-test-theme-luma-boundary`；settings.txt 备份后加 `liquid_gpu_share=1`（不开则测不到修复点）。
- **实机初步验证**：开 gpu-share 后进程存活 15s、内存 200MB，日志 `导入共享纹理 705x126→713x126`（尺寸持续变化=原崩溃场景，现稳定），无 panic/crash。进程在观察窗结束后消失，但 last-startup.log(22:25:02) 正常、last-error.log 是 11-15 旧记录 → 判断是 PowerShell 会话回收子进程，非崩溃。**最终结论需用户在桌面用 bat 手动稳定观察确认。**
- **未做**：未提交 main / 未打包安装器 / 未部署正式目录 / 未发 GitHub。

### 7.0zc 主题采样节流 按帧→按时间（2026-11-19 22:35–22:48，分支 `spike/wgc-gpu-share`，未提交）
- **动因**：用户质疑「30帧一次」是否更优。分析：跟"零拷贝每帧采样"比是优化（省 Map(READ) 同步读回），但按帧节流在低帧率（静止深休眠个位数 fps）下把主题响应延迟放大到数秒。
- **改动**（`wgc.rs`）：`THEME_TICK` → `THEME_LAST_SAMPLE: Mutex<Option<Instant>>` + `THEME_SAMPLE_INTERVAL_MS=500`；抽纯函数 `theme_sample_due`（saturating_sub 防时钟回拨）+ `THEME_EPOCH: OnceLock<Instant>` 做时间戳原点。按时间节流后响应延迟封顶 500ms、静止态不空转。
- **验证**：新增 2 个节流单测（间隔/帧率无关/时钟回拨），主题相关共 11 passed；全量 **1383 passed / 0 failed / 131 ignored**。新 exe `ae9c62c8c354a576`（22,253,568 B）。
- **部署预览目录**（未碰正式目录）：停 PID 33708（用户同意）→ 备份 `daozai.exe.bak-pre-timethrottle` → 替换 → 标记 `liquid-test-theme-time-throttle`。启动验证 15s 存活、205MB、gpu-share frame 600、纹理尺寸变化无崩溃。
- **行尾符**：全程 Edit 工具改，wgc.rs 保持 LF（CRLF=0），diff 284/1 干净，未复现上次 Python 写文件 LF→CRLF 的坑。
- **未做**：未提交 main / 未打包 / 未部署正式目录 / 未发 GitHub。

### 7.0zd 展开态翻页所有控件轻微颤动（2026-11-19 23:00–23:50，提交 `185ef1d`/`d7739c9`，未打包）
- **现象**：展开态切页，所有控件在滑入到位后各自轻微颤动；用户明确与主题/外观/液态玻璃开关无关（不开也抖）；"平移快到结束时开始抖"。
- **定位过程（教训）**：先按代码推理怀疑"高度弹簧→reveal→ctl_scale"链，错了；写了 4 版诊断日志（jitter.log）抓 p/reveal/h/slide/cscale/px/old_a 等，**全部证明父级量单调收敛无振荡**（推翻亚像素取整、叠加渲染、缩放颤动三个假设）。关键转折：HANDOFF 坑 9——agent 工具直接启动的 GUI 进程会被会话清理（PowerShell Start-Process 不行），必须在同一 Python 进程内用 `subprocess.Popen(creationflags=0x08000000|0x0200)` 启动并就地截图。坑 9+冻结模式（FREEZE_BG 预热24帧后取消排除）让我终于抓到可见岛仔的翻页帧。
- **真根因**：控件入场弹簧沿用用户可调 `liquid_zeta`（默认 0.5）严重欠阻尼，capp 序列 0.78→1.034→0.995→1.001→1.0 振荡近 0.9s；每个控件以自身中心 scaled(cx,cy) 缩放 → 各自颤动；高 DPI(ppp=1.56) 放大可见。冻结模式看不到是因为等预热+hover 时 capp 早已收敛。
- **修法**（app.rs）：新增 `CTL_APPEAR_ZETA=0.85`，入场弹簧 tune 用它替代 zt（初始化也一致）。按压果冻仍用 liquid_zeta，用户可调手感不变。ui_tests.rs +1 单测锁定入场不过冲。
- **实机抓帧验证**（60fps 追踪专注页秒表红环）：过冲回弹 **4.28px → 0.04px**，收敛 0.9s → 0.15s。全量 **1384 passed / 0 failed / 131 ignored**。
- **部署**：exe `d40f3aa63f0d1e28` 到预览目录，标记 `liquid-test-calm-appear`。诊断脚本/日志/拼图已清理。
- **未做**：未 push / 未打包 / 未部署正式目录 / 未发 GitHub。

### 7.0ze 抖动修复全部撤销丢弃（2026-11-20 00:28）
- **结论：抖动问题未修复，用户要求全部撤销。** 从 7.0zd 起共 6 次尝试均未解决：入场弹簧 ζ 0.55→0.85→1.0、切页弹簧全改临界阻尼、高度弹簧临界阻尼、翻页锁 60fps、progress 分母 snap。
- **排除的证据**：帧间隔日志 521 行全部 16.7ms（60fps 恒定、零丢帧）→ 排除丢帧/卡顿；弹簧全 ζ=1.0 仍抖 → 排除过冲。用户实证"只有番茄页(高度不变)不抖，其余跨高度页都抖"，最后怀疑 progress 分母突变，但改后仍未解决。
- **回退**：`git reset --hard 2050b94`，丢弃 6 个抖动提交（185ef1d/d7739c9/bca1ed7/c57c83b/f433656/ddfe001）。源码回到抖动修复前原值（slide ζ=0.92、ctl_appear ζ=0.55、诊断代码全清）。
- **保底**：备份分支 `backup/shake-attempts-dropped-20261120` 指向 ddfe001，需找回时 `git cherry-pick` 即可。
- **部署**：重编干净 exe `3493eb8f34c92561` 部署预览目录，标记回退为 `liquid-test-theme-time-throttle`（保留有效的主题取样越界+节流修复）。
- **状态**：main 领先 origin 10 个提交（= 9 spike + 1 主题取样修复），工作区干净。未 push / 未打包 / 未部署正式。
- **教训**：抖动根因始终没定位到；多轮"看起来合理"的推理（弹簧/丢帧/progress）都被实机否证。以后此类问题先要用户录屏 + 精确复现条件，不要靠推理连续试改。

### 7.0zf 优先独显/零拷贝加「立即重启」按钮（2026-11-20 00:39，提交 `e6666de`）
- **需求**：用户问①"关闭优先独显是否退回集显、怎么验证"，②把 D3D12 那处的「立即重启」复制到「优先独显」「零拷贝」下方。
- **①答**：关闭 `liquid_prefer_dgpu` = 不设自定义 wgpu adapter selector（main.rs 1136），走 egui-wgpu 默认策略（HighPerformance 偏好），混合显卡机器上**仍可能落到核显，并非必然退回集显**。验证：`data\diagnostics\last-graphics.log` 的「渲染器」「设备类型」（DiscreteGpu=独显 / IntegratedGpu=集显）；当前实测 `NVIDIA GeForce RTX 4070 Ti SUPER / Dx12 / DiscreteGpu`。
- **②实现**：App 加 `prefer_dgpu_running`/`gpu_share_running` 启动快照字段；液态玻璃配置弹窗（extras.rs）里对应设置改了与快照不同 → 显示橙色提示 + 「立即重启」按钮（`Cmd::RestartApp`），与 D3D12 一致。
- **验证/部署**：按用户要求未跑全量测试，仅编译通过。exe `fc73ef23f00e089b` 部署预览目录，标记 `liquid-test-restart-buttons`。
- **状态**：main 领先 origin 11 个提交，工作区干净。未 push / 未打包 / 未部署正式目录。

### 7.0zg 正式安装包 v1.0.1 重打（2026-11-20 00:54）
- 用户要求打正式安装包、覆盖 v1.0.1、不跑全量测试。
- **打包命令**（NSIS，注意必传 SOURCE_EXE，且 /DIR 要紧跟 exe 才能生效）：
  `makensis /INPUTCHARSET UTF8 /DPROJECT_DIR=D:\code\daozai-custom\dz-phone /DAPP_VERSION=1.0.1 /DSOURCE_EXE=<target\release\daozai.exe> /DOUTPUT_FILE=installer\Daozai-v1.0.1-windows-x64.exe installer\setup.nsi`
  - 坑1：漏传 `/DSOURCE_EXE` → `File: "${SOURCE_EXE}" -> no files found`（setup.nsi:267）。
  - 坑2：`/DIR=xxx` 没紧贴 exe 时被忽略，装到默认 `AppData\Local\Programs\Daozai`。
- **产物**：`installer\Daozai-v1.0.1-windows-x64.exe`（9,253,646 B，包 sha256 `02e7b5eb11815742`），内含主 exe `fc73ef23f00e089b`（= 含重启按钮的最新版）+ bridge + extension + LICENSE + THIRD_PARTY_NOTICES。旧包备份 `Daozai-v1.0.1-windows-x64.exe.bak-pre-restart-btn`。
- **静默安装验证**：退出码 0，装到默认位置后主 exe 指纹与包内一致、产物齐全；随后卸载（退出码 0）+ 手动清残留 bridge 文件，验证目录已清理。正式目录 `D:\software\Daozai` 未触碰（仍 `c66336a6`）。
- **未做**：未 push / 未部署正式目录 / 未更新 GitHub release asset（v1.0.1 asset 仍是旧的，如需反映本包要重传）。

### 7.0zh 去掉重启按钮的多余快照字段（2026-11-20 01:05，提交 `6bb8ecb`）
- 用户指出 `prefer_dgpu_running`/`gpu_share_running` 多余：值来自 settings 对应键、比较对象也是那两个键。
- 改法：main.rs 启动期写 `DAOZAI_LIQUID_PREFER_DGPU` 环境变量（gpu_share 原已有 `DAOZAI_LIQUID_GPU_SHARE`）；extras.rs 弹窗比较 settings.X vs 环境变量启动值，改了才显示立即重启；app.rs 删两字段及初始化。保持"改了才显示"UX，去掉冗余中间层。
- 净删 3 行。按用户要求未跑全量测试，仅编译通过（exe a5489ce0）。
- **注意**：刚打的 v1.0.1 安装包（02e7b5eb，含旧字段版本）已过时；用户选择暂不重打。当前包内重启按钮逻辑用的是字段版（功能等价，只是实现冗余）。后续若要分发以新代码为准需重打包。

### 7.0zi 液态浅背景白边真因 + 修复（2026-11-20 02:15，提交 `8fbc7a1`）
- **现象**：液态玻璃浅色背景下岛体四周一圈半透明白色外围，只在液态+D3D12出现；用户蓝标指出是整圈（只画了一段代表）。
- **排查走了很多弯路**（教训）：先误判为高光/投影/分层窗口/白边亮度配置。关键纠正：① `SetWindowRgn` 是上游原仓库代码（`8371c26`/`6e3150b`），DComp 透明交换链（`9c91ebb`）是本项目定制替代方案；② 用户指出"是绿线外面那层、绿线范围内缩没包含它"→ 锁定 inset。
- **真因**：`glass-diagnostics.txt` 日志实证 `透明合成：opaque=true dcomp=true outline_clip=false` —— **DComp 已激活、根本没有 SetWindowRgn 硬裁**，但 shader 的 `inset=3px`（本为硬裁规避锯齿而设）仍生效 → 玻璃画得比窗口小 3px，露出的环带用桌面色回补 → 浅背景上显白，且高光（绿线）画在内缩边界没包住它。红绿棋盘实验证明该线不来自 GPU 内容；关投影实验证明不来自投影；`liquid_edge_op=0` 能消但不是用户要的（用户拒绝改配置）。
- **修法**（gpu_glass.rs:375）：`let inset = select(3.0, 0.0, u.notch.w > 0.5)` —— DComp 下 0（玻璃画满、高光包住整圈、真实 alpha 无锯齿），非 DComp 保持 3.0（硬裁路径不变）。
- **验证**：冻结抓帧，左侧边缘从 162→225 突跳变 194→215 平滑过渡（白边消失）；圆角 x 序列平滑递减无硬跳；dcomp=true 路径生效。exe `8cffa5ce` 部署预览目录。按用户要求未跑全量测试，仅编译通过。
- **坑**：DComp 需冻结模式截图才正常；手动设 `DAOZAI_DCOMP=1` + NO_EXCLUDE 会出现蓝块+抓屏失败（不要手动设，graphics.rs 会自动检测启用）。
- **状态**：main 领先 origin 13 提交。未 push / 未打包 / 未部署正式目录。v1.0.1 包已过时（早于重启字段简化 + 本修复），需分发要重打。

### 7.0zj 修 inset=0 后的边缘锯齿（2026-11-20 02:25，提交 `07f08f2`）
- **背景**：7.0zi 取消 DComp 的 inset 后白边消失，但用户反馈有轻微锯齿。
- **原因**：岛体 aa 仍用写死 `smoothstep(-1.2,1.2,d)`，该宽度是按非 DComp 路径（抓屏滞后1-2帧、再宽会毛躁）调的；DComp 画满后不够。
- **社区方案**：SDF 抗锯齿标准做法 = 用 `fwidth(d)` 按屏幕导数自适应过渡宽度（搜索确认：fwidth 取像素邻域 SDF 变化率，让 smoothstep range 随缩放动态调整）。
- **修法**（gpu_glass.rs:403）：DComp 用 `aa_w = max(fwidth(d)*0.75, 1.0)`，非 DComp 保持 1.2。
- **验证**：WGSL fwidth 正常编译运行、shader 无错误；收起态边缘平滑无白边无蓝块。展开态远程未抓到，视觉改善待用户实机确认。exe `076f096a` 部署预览，标记 liquid-test-restart-buttons。按用户要求未跑全量测试，仅编译通过。
- **状态**：main 领先 origin 14 提交。未 push / 未打包 / 未部署正式目录。

### 7.0zl 高光跟随可配置 + 手机 App 自动登记 + 正式包（2026-11-20 03:15，提交 `2f28772`）
- **需求**：① 液态高光"鼠标光源"做成配置项；② 手机连接 phone-apps.json 检测通知自动加入新 App（默认开启）、删掉输入框只靠检测；③ 统一编译+全量测试（含之前未测的提交）、提交、push、打 v1.0.1 正式包。
- **光源配置项**：`liquid_light_follow`（0~1，默认1.0），0=固定左上光不跟鼠标、1=完全跟随（原行为）。新增 uniform `lightcfg`（WGSL/Rust struct 同步，uniform 大小 544→560，测试断言同步）；WGSL 的 Ld 跟随权重乘 follow。设置页"高光跟随鼠标"滑杆。
- **手机 App 自动登记**：`system::phone_new_app()` 判定新 App（名单未命中、非来电/短信/号码/固定标签词/长文本）→ `notify_phone` 里 `add_if_missing`+`save`（默认 allow=true）→ 删除设置页输入框/添加按钮和死字段 `phone_app_new`，只留勾选列表。+单测。
- **行尾符坑**：Python read_text/write_text 把 app.rs/gpu_glass.rs 从 LF 误转 CRLF（diff 虚增到 13898 行），用二进制 replace 改回 LF，收敛到 86+/25-。extras.rs/model.rs 原本就是 CRLF 保持不变。
- **验证**：全量 **1384 passed / 0 failed / 131 ignored**；双 bin 编译通过。（中途新App单测失败1次：head="来电"没拦住，加固定标签词排除后通过。）
- **正式包**：`installer\Daozai-v1.0.1-windows-x64.exe`（9,255,524 B，包 sha `feddcfd1bad66fe2`，内含 exe `7d7775e1` + bridge `3511a123`）。静默安装验证退出码0、产物齐全、指纹一致；已卸载清理。
- **push 失败**：`git push origin main` 两次失败（先 502 CONNECT tunnel、后 schannel server closed abruptly）——网络/代理问题，非代码。main 仍 ahead origin 16 提交，**待网络恢复后重推**。
- **状态**：main `2f28772` 领先 origin 16，工作区干净。正式包已打（未部署，用户自行安装）。未部署正式目录 / 未更新 GitHub release asset。

### 7.0zk aa 过渡带加宽（2026-11-20 02:26，提交 `8b2ca64`）
- 用户反馈"还可以再调点"→ fwidth 系数 0.75→1.1、最小值 1.0→1.4（过渡带宽约+40%），展开态边缘更柔。仅编译通过、未跑全量测试。exe b20f9ce 部署预览。


### 7.0zm 开机液态玻璃变黑修复（2026-11-20 11:10，提交 `4ea952a`）
- **现象**：真开机后进桌面，液态玻璃失效变纯黑底、一直不恢复；手动重启岛仔就正常。用户澄清：彩虹条纹/红绿箭头是正常色散效果，不是 bug。
- **排查弯路**：先误判为零拷贝坏帧/高色散（做了稳定期修复、回退；做了色散调低对照——彩条仍在且旧版一样，证明是正常效果）。教训同白边那次：用户"重点不是X而是Y"的澄清 > 推断。
- **真因**：开机早期桌面合成未完成，抓屏拿到过渡暗帧（日志 luma 采到 0.2~0.5，正常 0.8+）→ 主题采样采纳 → liquid_k→0 → 液态玻璃判深色材质黑底；之后采样被节流、暗帧持续有效 → 不恢复。手动重启时桌面已稳定 → 正常。
- **修法**：wgc.rs 新增 pub(crate) `theme_luma_commit(prev,luma,pending)`：大幅浅→深突变(>0.30)要求连续两次采样都低才采纳（真深色壁纸持续、过渡帧短暂），平滑/变亮立即采纳。零拷贝(THEME_LUMA)与普通抓屏(BG_LUMA)两条路径共用；+单测 开机过渡暗帧不立即采纳（4情形）。
- **验证**：全量 1385 passed / 0 failed / 131 ignored；双 bin 编译通过。exe c17ae0fc。开机场景本身无法在会话内复现，待用户下次开机实机确认。
- **正式包**：installer\Daozai-v1.0.1-windows-x64.exe 重打，9,257,582 B，包 sha `e8d2b7349f236a54`，exe c17ae0fc + bridge 68b95a15。静默安装验证指纹一致。
- **push**：`git push origin main` 失败（502 CONNECT tunnel，网络/代理问题）。main ahead origin 1（之前 16 个提交已成功 push，origin 已在 2f28772）。待网络恢复推 4ea952a。


### 7.0zn 壁纸文件锚点根治开机黑底（2026-11-20 12:40，提交 `c5f8827`）
- **需求**：用户不要系统主题兜底（液态玻璃必须只跟壁纸走），要综合最优解。
- **方案 = 壁纸文件真值锚点 + 抓屏校验三层防护**：
  1. 新模块 `wallpaper.rs`：读 `%APPDATA%\Microsoft\Windows\Themes\TranscodedWallpaper`
     （Windows 转码壁纸，登录前就绪），image crate 解码缩 64×64 算平均亮度，
     缓存+文件修改时间检测换壁纸。本机实测壁纸亮度 0.89~0.93，与正常抓屏一致、
     与开机暗帧 0.2~0.5 完全区分。零新依赖（项目已有 image crate jpeg/png/webp）。
  2. 启动播种：App::new 用壁纸亮度初始化 THEME_LUMA(gpu_glass)/BG_LUMA(wgc)，
     开机第一帧就正确、不依赖抓屏。（注意 seed 方向：gpu_glass.seed→BG_LUMA，wgc.seed→THEME_LUMA）
  3. 锚点校验：`theme_luma_commit(prev,luma,pending,anchor)` 新增 anchor 参数，
     抓屏值与锚值差 >0.25 判过渡帧/污染帧直接拒绝；保留连续两次确认作兜底。
     两条抓屏路径（零拷贝 sample_theme_luma + 普通 capture）共用此函数。
- ** decisão**：完全不碰系统主题——用户壁纸浅色卡通图但系统可能深色，系统主题不能当判据。
  抓屏仍是显示区域真相来源（壁纸裁剪/动态效果），锚点只做初值和异常帧过滤。
- **验证**：编译双 bin 通过；相关单测 theme 13 + wallpaper 1 passed（新增锚点用例覆盖
  浅壁纸过渡帧/深壁纸/边界）。按用户要求未跑全量测试。exe `1570446d`。
- **正式包**：installer\Daozai-v1.0.1-windows-x64.exe 重打，9,291,345 B，包 sha
  `5f6764de3464b6e7`，exe 1570446d。静默安装指纹一致。已部署到正式目录。
- **状态**：main ahead origin 2（4ea952a + c5f8827）。push 未再尝试（此前一直 502）。
  用户自行部署测试开机黑底是否根治。
- **本轮教训**：用户明确"不要系统主题兜底、只跟壁纸走"——需求边界要先听清，别自作主张
  引入系统主题。之前误把正常色散当 bug 也是同类错误。


### 7.0zo 手机App自动登记不生效 + DComp边缘露缝（2026-11-20 13:15，提交 `964b77a`）
- **问题1**：用户手机来消息、岛上也显示，但 phone-apps.json 不生成、也没 AUTO-ADD 日志。
  - 根因：WinRT 监听器实际走 `main.rs:513 → receive_notification_from()`，**绕过 notify_phone()**，
    而自动登记只写在 notify_phone 里 → 从没触发。（之前白忙一场，代码在但路径不对。）
  - 修法：提取 `maybe_register_phone_app(id,body)`，在 receive_notification_from 开头也调用
    （notify_phone 内复用），两条路径都覆盖。+单测验证 receive 路径登记/不重复/非手机不登记。
- **问题2**：DComp 玻璃边缘还露一点。
  - 根因：inset=0 时阈值仍是 `d > inset-0.25 = -0.25`，玻璃内侧 -0.25..0 的 aa 带被当成
    "岛体外"提前 return 透明 → 边缘露缝。
  - 修法：阈值改 `select(inset-0.25, 1.0, dcomp)`，DComp 下 d>1.0 才算外部，
    整条 aa 带完整渲染成玻璃。
- **验证**：相关单测 phone 5 + theme 13 + 新登记1 passed；编译双 bin。exe 383ebcab。未跑全量。
  抓图边缘扫描：壁纸(246,239)→1px 跳到(212,219)→平滑过渡，aa 带约8px（之前加宽的），无裸缝。
- **包**：sha `419686379f3317a4`（9,292,913 B），静默安装指纹一致。用户部署测试：
  ① 手机来消息后检查 %APPDATA%\Daozai\phone-apps.json 是否生成并含来源 App；
  ② 看边缘缝是否消失。
- **教训**：修 bug 前先确认"实际执行路径"，不要假设入口函数就是调用者。本次和之前色散误判同类。


### v1.1.0 正式发布（2026-11-20 13:35）
- **全量测试**：1388 passed / 0 failed / 131 ignored（通过）。
- **版本**：Cargo.toml + Cargo.lock 1.0.1→1.1.0（注意 --locked 下 lock 要手动改，否则 build 报 "cannot update lock file"）。提交 e67326a。
- **push**：`git push origin main` 成功（网络恢复），4 提交（4ea952a/c5f8827/964b77a/e67326a）全推上；tag v1.1.0 也 push 成功。
- **包**：installer\Daozai-v1.1.0-windows-x64.exe，9,293,804 B，sha `6b4bf2b2b3389c9e`，内含 exe f54aa3e7。用户要求"先不安装"，跳过静默安装验证（之前一次失败仅因 daozai.exe 进程占用）。
- **GitHub Release**：用 git credential manager 取 token（gho_ 前缀 OAuth），REST API 创建 release + curl 上传 asset。
  - Release: https://github.com/cayww/daozai-custom/releases/tag/v1.1.0 （draft=false, prerelease=false）
  - asset: Daozai-v1.1.0-windows-x64.exe 9,293,804 B uploaded。
  - 坑：Python urllib 传 9MB 大 POST 被代理 RemoteDisconnected；curl --data-binary + --retry-all-errors 成功（HTTP 201）。
  - GitHub MCP 无 create_release/upload_asset 工具，只能查询；创建/上传走 REST API + git credential token。
- **更新文案**：用户两轮确认删减——① 开机黑底合并进零拷贝条目；② 删掉"基于v1.0.0..."前缀句与"同步修复了背景放大/错位..."细节句。只写净新增最终结果。
- **status**：工作区干净，main 与 origin/main 同步，release v1.1.0 已发布。


### 7.0zr 版本线收尾：关于页改版 + 粗体文案解析 + v1.1.0 覆盖重打（2026-11-20 14:10，提交 `6d70348`）
- **背景**：接手其他 agent 残留的未提交改动（`extras.rs` / `updates.rs`），按要求继续完成。
- **关于页**（用户明确要求）：
  - 设置页标签 `关于上游` → `关于岛仔`（与托盘菜单一致）。
  - 内容改为本项目介绍：「本项目由 cayww 基于小唐维护的岛仔深度定制而来，保留并扩展了本地功能」，作者 = **cayww**，主链接 `github.com/cayww/daozai-custom`；保留原上游链接（弱色）供查看。
  - **删掉** agent 先前塞进来的长「功能亮点」清单（液态玻璃/零拷贝/手机连接等罗列）——用户不要。
- **`updates.rs`**：release notes 的 `**粗体**` 现在解析成真粗体（`rich_segments` + `LayoutJob`，列表 `•` 前缀保留），新增单测 `release_notes_bold_is_parsed`。属残留改动，保留。
- **全量测试**：**1389 passed / 0 failed / 131 ignored**（`--test-threads=1` + 数据目录隔离），通过。
- **包**：双 bin 重编（daozai exe `40ca290e90fb0640`、bridge `0d387e9bec39f88`）。
  - NSIS 安装器 `installer\Daozai-v1.1.0-windows-x64.exe`，9,295,319 B，sha `0a97e5fb489909a1`（旧包 `6b4bf2b2b3389c9e` 备份为 `.bak-pre-about`）。
  - 便携 zip `installer\Daozai-v1.1.0-windows-x64.zip`，11,295,078 B，sha `99cc373b651c957f`（内含 exe = `40ca290e`）。**首次补上 zip**：此前 v1.1.0 release 只有 exe，更新模块 `release_info` 硬编码期望 zip，无 zip 则「检查更新→下载」不工作。
- **GitHub Release 覆盖**（版本仍 v1.1.0）：REST API 删旧 exe asset（770604260）→ 同名重传（新 id 770683848），并新增 zip asset（id 770684544）。API 复核两个 asset 的 name/size/sha256/browser_download_url 与本地一致、URL 符合更新模块期望。发布页 https://github.com/cayww/daozai-custom/releases/tag/v1.1.0 。
- **status**：提交 `6d70348`；**未 push / 未部署正式目录**（用户只要打包上传 release）。工作树干净。


### 7.0zs v1.2.0 测试版 + 更新链路两处 bug + 零拷贝换屏修复（2026-11-20 14:10–15:50）
- **v1.2.0 测试版**（用户要测「检查更新→下载→安装」，测完自删）：版本 1.1.0→1.2.0（Cargo.toml/lock），release id `452343951`，exe+zip 都传。
- **bug1 过时提示**：更新弹窗常驻「当前是自定义构建：安装官方版会覆盖…先运行 reapply.ps1」（updates.rs:293，CUSTOM_BUILD 硬编码）。更新源早指向自有仓库、不会覆盖本地修改；reapply.ps1 随补丁链 11-17 退役已删。用户不要 → 删该提示 + CUSTOM_BUILD 常量（提交 `bb39ee6`）。
- **bug2 版本不匹配**（用户实测更新报「安装包版本不匹配」）：根因 = `dz-phone/app.rc` VERSIONINFO 写死 0.6.66，自有发布线把 Cargo 版本升到 1.x 没同步资源版本；`installer/update.ps1:39` 拿 zip 内 exe 的 FileVersion 与目标版本比对，0.6.66≠1.2.0 必失败。**修法**：app.rc 改占位符 @DAOZAI_VERSION@，build.rs 用 CARGO_PKG_VERSION 渲染 + rerun-if-env-changed，以后升版本自动同步（提交 `9eaaa7b`）。验证 exe/zip 内 FileVersion 随版本双向生效。
- **版本回退**：用户要最终本地版本 = 1.1.0（保留上面两处修复），覆盖发布 v1.1.0 用于测 1.1.0→1.2.0。版本 1.2.0→1.1.0（提交 `d3bf5b6`），重打 v1.1.0：exe `16857da253bb4a79`（9,294,543 B）、zip `7006e37bed8c26ea`（11,294,846 B），删旧 asset 同名重传。v1.2.0 release 保持不动。
- **bug3 零拷贝换屏玻璃异常**（用户实测：仅2→仅1 背景放大/切回定格静态壁纸；仅2→扩展 卡顿后重绘正常；仅2→复制 没铺满/发黑）：
  - **根因**：零拷贝在 wgpu 侧缓存导入的共享纹理，缓存键只有 (宽,高)。换屏/切显示模式重建 WGC 会话与共享纹理（新 NT handle），但岛仔 UI 决定的抓屏尺寸常不变 → `need_import=false` → 继续采样旧纹理，就是上述症状。非零拷贝每帧全量上传、无缓存句柄，故不受影响（这是"结合非零拷贝解法"的落点）。
  - **修法**（提交 `c097c4d`）：`SharedHandle` 带 wgc 的 `EPOCH`（拓扑/显示器变化必递增）；`gpu.shared` 缓存元组加 epoch，`shared_needs_import()` 用 (w,h,epoch) 做键；导入失败时清空 shared/bind_group 真正回退 staging，不再拿旧纹理画。新增单测 `shared_texture_reimports_on_epoch_change_even_when_size_is_same`。
  - **验证**：全量 **1390 passed / 0 failed**（新增 1 项）。**实机换屏效果待用户验证**（本机无法复现多屏拓扑）。exe 重编（指纹见 git）；**未打包/未覆盖 release**（用户未要求，先要源码修复）。
- **status**：main 本地 `c097c4d`（bug3 修复），前序 `d3bf5b6`/`9eaaa7b`/`bb39ee6` 已 push；bug3 提交**未 push**。工作树另有一处用户删的 `installer/V1.0.1-NOTES.txt`（未纳入我的提交，未处理）。

### 7.0zt v1.1.1 打包发布 —— 零拷贝换屏修复实机测试版（2026-11-20 15:48–16:00）
- 用户要打包测试零拷贝换屏修复，先说覆盖 v1.1.0、随即改为**打包 v1.1.1**（v1.1.0 保留作更新测试起点，不动）。
- 版本 1.1.0→1.1.1（提交 `5fb735c`）；build.rs 动态版本资源随升，exe/zip 内 FileVersion 均 = 1.1.1（update.ps1 校验通过）。
- 全量 **1390 passed / 0 failed**；exe `fd92c1fb639c01d4`、bridge `d572d28ade6cb2e`。
- 新 release **v1.1.1**（id `452411859`，非 draft/prerelease）：exe `4a1652d703aae23e`（9,292,417 B）、zip `8432a68351742e97b`（11,291,552 B）。API 复核 name/size/sha256/URL 与本地一致。
- **v1.2.0 release 已被用户删除**（404，用户之前说测完自删，符合预期）；v1.1.0 保持不动。
- main 已 push 到 `5fb735c`；工作树仅剩用户删的 `installer/V1.0.1-NOTES.txt`（未处理）。
- **待用户实机确认**零拷贝换屏四种场景（仅2→仅1放大、切回定格、仅2→扩展卡顿、仅2→复制没铺满/发黑）是否修复。

### 7.0zu v1.1.1 实测仍复现 → v1.1.2 二次修复（2026-11-20 16:00–16:15，提交 `949c028`）
- 用户反馈 v1.1.1 换屏 bug 仍在。社区调研（scrying 0.3.0 / AWJimage SDK / D3DShot issue）：① resize/recreate/device-lost 后不得沿用旧代资源；② 1×1 copy 刷 shader cache 是驱动行为、非契约 → 绑定到旧纹理是真实故障模式。
- **第二处根因**：零拷贝成功时 `gpu.bind_group` 指向共享纹理，导入失败时清空为 None；切屏 settling 期间 `grab_shared` 返回 NoFrame 落到 staging，但 staging 只在 `gpu.texture.is_none()` 时才建 bind → texture 已存在+bind 指旧共享纹理=采样旧内容（放大/定格壁纸），bind=None=paint 直接 return（没铺满/黑屏）。这解释了症状的时有时无。
- **修法**：staging 路径开头 `if gpu.shared.is_some() || gpu.bind_group.is_none()` → 清 shared、强制 `gpu.texture=None` 重建 texture+bind，保证 bind 指向本路径写入的纹理。版本升 1.1.2。
- **按用户要求本轮不跑全量测试**，只编译双 bin（exe `2d89c30b96cf5038`）。
- v1.1.2 release（id `452420750`）：exe `bc4547984fd937e4`（9,294,671 B）、zip `78ada1405d8835c8`（11,291,939 B），API 复核 name/size/sha256/URL 一致。v1.1.0/v1.1.1 release 未动，v1.2.0 已由用户删除。
- main push 到 `949c028`。注意：提交 4f0534a 曾因 Python 写文件把 Cargo.toml/lock 的 LF 改成 CRLF 造成整文件 diff，已 soft-reset 并用编辑器重做，最终干净（15+/2-）。
- **待用户再次实机验证**；仍未改好之前用户要求继续不跑全量测试、继续打包。

### 7.0zv v1.1.3：非0拷贝复制模式铺不满 + 移除 settle 自学习（2026-11-20 16:21–16:40，提交 `60b4eed`）
- 用户两个问题：① 非0拷贝下「仅2号屏 → 复制这些显示器」铺不满（之前修复没覆盖）；② settle-learned 学习代码不需要，固定 1s。并明确 **16:26 起不要往 release 上传**，只本地打包。
- **问题1 根因**：`wgc.rs::grab()` 的 `src_box` 按显示器几何 `(s.sw,s.sh)`（GetMonitorInfoW 的 rcMonitor）算，但 WGC 实际帧尺寸在复制模式下常小于 rcMonitor（克隆合成面尺寸不同）。`CopySubresourceRegion` 会把超出的 box **静默裁剪**到源纹理边界，staging 其余像素保持 0/黑 → shader 采样黑区 = 铺不满。之前所有修复都集中在零拷贝缓存/bind_group，判断"非0拷贝每帧全量上传不受影响"，**漏掉了 staging 路径这一层**。
  - 修法：拿到 `src_tex` 后用 `GetDesc(&mut D3D11_TEXTURE2D_DESC::default())` 取真实宽高（注意 windows 0.58 的 GetDesc 是 out-param、返回 `()`，不是 Result），抓屏框超出真实帧就 `drop(frame); return Grab::NoFrame`（不产出半张图）。`grab()` 和 `grab_shared()` 两条路径都加。
- **问题2**：删除 `LEARNED_SETTLE_MS` / `TOPO_CHANGED_AT` / `note_plate_ready()` / `learned_settle_path` / load/save，poll_topology 不再写 TOPO_CHANGED_AT，gpu_glass 删 note_plate_ready 调用。稳定期固定 `SETTLE_MS=1000`，保留 `DAOZAI_SETTLE_MS` 环境变量覆盖。理由：学习值全局共享，快机器学出的 400ms 不够慢机器/复制模式用。
- **按用户要求本轮不跑全量测试**，只编译双 bin（exe `1afc09b1dc21f48`、FileVersion 1.1.3）。
- **本地包，未上传 release**：`installer\Daozai-v1.1.3-windows-x64.exe` sha `2890bfb86a80f167`（9,294,050 B）、zip `e495d16d0626360b`（11,294,491 B）。v1.1.0/1.1.1/1.1.2 release 均保持原样未动。
- main push 到 `60b4eed`。注意：用户之前删的 `installer/V1.0.1-NOTES.txt` 本次已被 staged 并随提交删除（用户本就有意删，工作树现干净）。
- **待用户本地验证**两个问题；改好前继续不跑全量。

### 7.0zw v1.1.4：铺不满根治——几何 uniform 每帧同步视口（2026-11-20 16:43–17:15，提交 `61daa50`）
- v1.1.3 用户实测铺不满仍在，**零拷贝/非0拷贝都中招** → 说明根因不在抓屏纹理层，而在两条路径共用的几何/uniform。
- **社区调研**（用户要求）高度吻合：egui PR #8280「Stretched content during fullscreen, **stale content in bottom left**」根因=「first presented frame uses **stale surface/viewport state**」；tetanes PR #315 surface 尺寸没跟上布局→拉伸，修法=每帧把布局尺寸传进 paint；Microsoft WGC 指引「**Use item's Size as frame size**, recreate pool when item size changes」。
- **真正根因**：`gpu_glass.prepare()` 只在抓屏成功时写几何 uniform（`uniforms(f,tex)`→u.extra.y/z=bounds 尺寸）。换屏/复制模式抓屏失败的那几帧，viewport 已按新 capture 每帧重算，uniform 却停在旧值 → shader `p=uv*tex` 用旧几何画进新视口 = 铺不满/拉伸/角落残留。**而 v1.1.3 的 `GetDesc 丢帧` 让复制模式持续 NoFrame，反而把纹理永久停在换屏前**，加剧问题。
- **修法**：① 抽 `write_geom`（按视口 cw/ch 写 uniform），所有返回路径（节流/抓屏失败/超限/共享成功）都调用，与抓屏成败解耦；② wgc.init() 帧池改用 `item.Size()`（微软指引），不再硬用 rcMonitor；③ grab()/grab_shared() 的 src_box 夹到真实帧内，**删掉 v1.1.3 的丢帧逻辑**（避免永久旧纹理）。
- 全量测试**未跑**（用户要求）；编译通过，exe `0ef6e9e1f9764e9`。
- 本地包未上传 release：`installer\Daozai-v1.1.4-windows-x64.exe` sha `556ffdf54b2eaa53`（9,295,752 B）、zip `c2cc450086392e86`（11,294,298 B）。main push 到 61daa50。
- **待用户验证**；若仍铺不满，下一步需用户提供实测 `liquid-glass.log`（看 item.Size vs rcMonitor、是否有持续 NoFrame）再定点。

### 7.0zx 背景放大真正根因：D3D11 device-lost + 版本回 1.1.1（2026-11-20 17:24–17:40，提交 `409e9c0`/`de4da1a`）
- v1.1.4 用户实测：铺满已好，但复制模式下玻璃背景**被放大**。用户要求看社区/继续修、仍不上传 release。
- **实测日志定位**（`%APPDATA%\Daozai\diagnostics\liquid-glass.log`）：满屏 `wgc: staging/shared resize FAILED Error { code: HRESULT(0x887A0005), message: "GPU 设备实例已经暂停" }` + `[gpu-share] event query 超时`。`item.Size()=2560x1440 rcMonitor=2560x1440`（相等，非尺寸问题）。
- **根因**：换屏到复制模式导致 D3D11 **device lost（DXGI_ERROR_DEVICE_REMOVED 0x887A0005）**，此后所有纹理 resize/Copy 永久失败、event query 永不 signal，会话卡在死 device 上：纹理定格在换屏前尺寸，视口几何每帧变 → 旧纹理画进新视口 = 背景被拉伸放大。之前所有修复都没处理 device lost。
- **修法**（409e9c0）：加 `is_device_lost()` 识别 0x887A0005/0x887A0006；grab()/grab_shared() 的纹理 resize 失败或 event query 超时若判 device lost，销毁整个 Wgc 会话（RAII 释放死 device/pool/session）+ 清 TRIED，下一帧重 init 新 device。
- **用户明确指示不要再自增版本号，改回 1.1.1**（de4da1a）：版本 1.1.5→1.1.1，保留全部修复代码。exe FileVersion=1.1.1，新 exe `5092bcda9aa28da1`（与 GitHub release 上旧 v1.1.1 的 `1ce60bc7` 不同——那些 release asset 是修复前代码，用户只本地测试、不动 release）。
- 本地包（**未上传 release**，覆盖本地 v1.1.1 文件，旧文件已备份后清理）：`installer\Daozai-v1.1.1-windows-x64.exe` sha `84e3e7680b7a014b`（9,293,180 B）、zip `b7d881b327208dd2`（11,291,643 B）。
- 全量测试**未跑**（用户要求）。main push 到 de4da1a，工作树干净。**待用户本地验证**放大是否消失。

### 7.0zy 展开动画变慢根治：共享纹理固定尺寸复用（2026-11-20 20:45–21:00，提交 `87c39e6`）
- **背景**：§7.0zx 之后又有两个未进文档的提交：`4603803` 消除 GPU 等待忙自旋（修改分辨率后键盘变慢/悬停不展开）、`8b6c0e3` 共享 D3D12 fence 跨 API 同步（`DAOZAI_GPU_FENCE=1` 开关）。用户实测：主题色采样仍在跑、**fence 和 yield 两版展开都会变慢** → 锁定一个与 fence/yield 都无关的共同瓶颈。
- **排查结论**（上一个 agent 的推理，已确认）：① `AcquireSync(0,500)` 不是元凶——D3D12 侧从 keyed-mutex 句柄打开的资源无法 QI `IDXGIKeyedMutex`，consumer 从不 ReleaseSync，producer 自己 Acquire→Release(0)，正常立即返回；② 主题色同步 Map 已按时间节流（≥500ms）+ 独立 1024×256 staging，非主因。**真凶**：展开动画 capture 尺寸每帧微变（日志 `629x122→631x122→632x122`，导入 4 次），零拷贝路径按帧尺寸重建共享纹理 → 每帧 `CreateTexture2D+CreateSharedHandle+CreateQuery+OpenSharedFence` 一串同步 COM，wgpu 侧每帧 `OpenSharedHandle` 重新导入。
- **修法**（87c39e6，对齐旧 staging 路径 dd57451 已验证的"固定纹理+左上角有效区"设计）：
  - `wgc.rs`：共享纹理分配固定 `SHARED_TEX_W/H=1280×512`，不再随帧微变重建；仅 device-lost 会话重建才新建；加有效帧超尺寸保护（>分配尺寸回退 staging）。
  - `gpu_glass.rs`：`shared_needs_import` 缓存键去掉 (w,h) 只留 epoch（帧尺寸变化只是 uniform 的 tex，shader 靠 `scale=tex/alloc` 映射有效区）；`import_shared_handle` 的 size 改用固定分配尺寸（必须与 D3D12 resource 一致，不能传有效帧尺寸）；回归单测改 `shared_texture_reimports_only_on_epoch_change`。
  - **未动** AcquireSync/主题色 Map/fence（排查已排除，最小改动原则）。
- 编译通过（仅既有 warning，无 error）；全量测试**未跑**（用户明确要求）。新 exe `0454bd5acde67031c9e7914c`（22,401,536 B）/ bridge `d8996c660642bb5962c5`。
- 已备份预览旧 exe 为 `Daozai-液态玻璃预览\daozai.exe.prev-before-texreuse`（`b20f9ce771048cc8a`），新 exe 已部署到预览目录、清空 `data/diagnostics/liquid-glass.log` 待验证。**待用户实机验证**：零拷贝已开（预览 data `settings.txt liquid_gpu_share=1`），悬停展开动画应不再逐帧"导入共享纹理"、展开不再变慢；验证方法见下。
- main HEAD=`87c39e6`（ahead origin 1，未 push）。

### 7.0zz 换分辨率内存持续上涨根治——WGC session/pool 显式 Close（2026-11-20 21:05–21:15，提交 `815932b`）
- **用户实测**：换分辨率后内存持续上涨（上一个修复 87c39e6 只解决展开变慢，与内存无关）。用户抓 VMMap 快照 `C:\Users\cay\Desktop\daozai.mmp`（PID 40132，exe=预览版）供分析。
- **快照实证**（只读解析 .mmp XML）：Commit **3,859MB** / PrivateBytes **3,352MB**（历史基线独显 257MB、memprof 622MB，严重异常）；`Private Data` 中 **70 个 32MB 块=2,240MB** + 1 个 783MB 块；**Thread Stack 2,074 块 / 2,055 个唯一线程 ID**（正常进程几十个）。这是会话反复重建却没释放干净的典型指纹。
- **根因**：`Wgc` 持有 `GraphicsCaptureSession`/`Direct3D11CaptureFramePool`/`GraphicsCaptureItem`，WinRT 对象内部有引用环（session↔pool、item 被 session 持有），仅靠 Rust drop 递减引用计数无法归零——捕获线程与底层 DXGI 资源一直存活。换分辨率 → device lost / 拓扑变化走 `*guard = None` 重建会话，旧会话只 drop 不 Close 就泄漏整套捕获管线。原 806 行注释误以为"RAII 析构会自动停止旧屏捕获"，是错的。微软 WGC 指引明确要求停捕获时显式 Close。
- **修法**（815932b）：`impl Drop for Wgc` 显式 `session.Close()` + `pool.Close()` 断开引用环；清理三处对 `&mut Wgc` 无效的 `drop(s)` no-op（真正释放靠 `*guard = None` 触发 Drop，NLL 已验证借用合法）。
- 编译通过（无 error）；全量测试**未跑**（用户要求）。新 exe `ca8923b6e02d3043` / bridge `6afcdde3c358bdf3`，已备份上一版为 `daozai.exe.prev-before-wgc-close` 并部署到预览目录。**待用户实机验证**：反复换分辨率后内存不再持续上涨（线程数、32MB 块不再累积）。
- main HEAD=`815932b`（ahead origin 3，未 push）。

### 7.0zza 换分辨率后 20 秒工作集回收不触发——cursor_far 按岛仔命中区判定（2026-11-20 21:15–21:30，提交 `afdf877`/`e32b343`）
- 用户实测：WGC Close 修复后内存不再涨，但换分辨率后 **20 秒工作集回收不触发**，且光标闪/跳。
- **诊断**（`afdf877`，pill-sleep.log 挡因行加 cursor/win 坐标）：`blocked=cursor` 恒真；实测 `cursor=(625,168)`、`win=(0,0,640,270)`。
- **根因**：`cursor_far=!env.cursor.is_some_and(|p|win.expand(24).contains(p))`——用整个**透明画布 win(640×270)** 判"鼠标在岛上"，而岛仔命中区 `hit` 收起时只有顶部约 300×50。换分辨率后窗口变大/移位，鼠标离开岛仔、只要还在画布内就算"近" → `workset_quiet` 永不满足 → 回收永不触发。
- **修法**（e32b343）：`cursor_far` 改按 `hit.expand(24)` + 未读徽章判定；通知/吐司已有独立挡因不重复。诊断坐标日志保留。
- 新 exe `0fcbf892ab7f1c46c`，已部署预览目录、清空 pill-sleep.log。编译通过，全量测试未跑（用户要求）。**待用户验证**：鼠标移离岛仔后 20 秒回收触发（WS 275MB→~30MB），挡因不再有 cursor。
- main HEAD=`e32b343`（ahead origin 5，未 push）。

### 7.0zzb device-lost 重建风暴加退避——根治换分辨率后光标持续闪（2026-11-20 21:33–21:50，提交 `415ffe9`/`7c53175`）
- 用户实测：内存/泄漏/20 秒回收三项都已修好，但换分辨率后**鼠标指针持续闪、岛反复展开收起**（分辨率稳定、鼠标不动也闪）。
- **定位**（诊断 `afdf877`/`415ffe9` + 日志实证）：
  - pill-sleep.log：`blocked=cursor` 已消失，回收修复生效；区域日志显示 `expanded` 随鼠标 y 变化（正常 hover），非自激。
  - **真凶**：`liquid-glass.log` 连续 **54 次** `event query 超时 device_lost=true → init OK → 又超时` 循环；`qdc: topology=SINGLE targets=[src0:2560x1440] sources=[1920x1080]`（换分辨率期 source/target 不一致，device 反复 lost）。每轮 init 都 StartCapture + DWM 合成 + 捕获黄框重启 → 可见闪烁 + GPU 空转。
- **修法**（7c53175）：`wgc.rs` 加 `LOST_STREAK` 递增退避（1s→2s→4s→8s），device-lost 后设 `SETTLE_UNTIL`，期间 `settling()` 让 grab/grab_shared 直接 NoFrame、不再 StartCapture；成功出帧 `note_capture_ok()` 清零。三处 device-lost 分支统一接入。从 54 次风暴降到等设备稳定后 1-2 次重建。
- 新 exe `1a8ad6fba96c790d`，已部署预览目录、清空 liquid-glass.log/glass-diagnostics.txt。编译通过，全量测试未跑（用户要求）。**待用户验证**：换分辨率后光标不再持续闪；日志里"device-lost"行只有个位数而非几十次。
- main HEAD=`7c53175`（ahead origin 8，未 push）。

### 7.0zzc 社区调研根治重建风暴：EVENT query 非法 Begin + 退避过早清零（2026-11-20 21:50–22:00，提交 `1d03311`）
- 用户实测：退避让闪变慢，但岛仔也闪、液态玻璃没了、分辨率改回来仍闪。日志每轮停在「连续 1 次、冷却 1000ms」→ 退避被单帧成功清零，永远升不上去。
- **社区调研**（用户问"社区有没有提到"）：
  - 微软 `D3D11_QUERY` 文档：`D3D11_QUERY_EVENT` 的 **`Begin()` 是 disabled 的**，正确用法只 `End()` 标记 GPU 时间线点 + `GetData` 轮询 BOOL（Chromium ANGLE `Renderer11::finish()` / Dawn `QueueD3D11` 均如此）。
  - 微软 Screen capture 文档：device lost/尺寸变化用 `FramePool.Recreate` 而非整体重建（未采纳，改动大风险高）。
  - Dawn 源码注释：命令未 flush 时 `GetData` 可能永不 true（我们已显式 Flush，非主因）。
- **修法**（1d03311）：① 去掉 `s.context.Begin(&shared.done)`（EVENT query 非法调用，疑为 GetData 永不 signal、误判 device lost 的元凶）；② `note_capture_ok` 改为连续成功 30 帧(~1s)才清退避，任何 lost 清零成功计数——解决"好一帧坏一帧"时每轮从头 1s 的问题。
- 新 exe `587b86b71a490633a`，已部署预览目录、清空日志。编译通过，全量测试未跑。**待用户验证**：换分辨率后 device-lost 行是否归零、玻璃是否恢复、是否不再闪。
- main HEAD=`1d03311`（ahead origin 10，未 push）。

### 7.0zzd 【方向重置】回退到 de4da1a + FramePool.Recreate 官方方案（2026-11-20 22:00–22:15，分支 `fix/wgc-recreate`）
- **用户决定**：§7.0zy~7.0zzc 这一串（展开变慢/内存/回收/光标闪）越修越乱、"污染太严重"，**全部回退**，从干净点用微软官方 `FramePool.Recreate` 方案重做。
- **回退操作**：
  - 当前 HEAD `9d23e4e` 已用 tag **`backup/before-recreate-rework-20261120`** 完整备份（随时可恢复）。
  - 从 `de4da1a`（§7.0zx，版本 1.1.1，"放弃修复复制模式放大"锚点，用户确认）新建分支 `fix/wgc-recreate`，丢弃那 12 个本地提交（均未 push，origin/main 停在 `8b6c0e3`）。**main 分支仍停在 9d23e4e 未动**，新方案在 `fix/wgc-recreate` 上。
- **新方案**（提交 `8683321`，对齐微软 Screen capture 文档）：
  - 根因：device lost 时 `*guard=None` 销毁整个 Wgc → 每轮 StartCapture + DWM 重合成 + 黄框重启，换分辨率期反复 lost 形成 54 次重建风暴 → 闪 + 玻璃消失。
  - `recreate_after_device_lost()`：新建 D3D11 device/context → IDirect3DDevice → **在原 pool 上 `Recreate(新device, item.Size())`**，保留 session/item、不重新 StartCapture；重建 staging + 共享纹理，递增 EPOCH。失败兜底整体重建。
  - grab/grab_shared 三处 device-lost 分支统一改用。
- **注意**：此分支基于 de4da1a，**不含** fence(8b6c0e3)/固定纹理尺寸(87c39e6)/WGC Close/cursor_far/退避等后续修复——那些症状若本是重建风暴的连锁反应， hopefully 自然消失；若复现再针对性处理。
- 新 exe `e552a5b908d37f3b`（22,401,536 B 级），已部署预览目录（备份 `daozai.exe.prev-before-recreate`），清空日志。编译通过，全量测试未跑。**待用户验证**：换分辨率后不闪、玻璃正常。
- 分支 `fix/wgc-recreate` HEAD=`8683321`；main 仍 `9d23e4e`（本地 ahead 12，未 push）。

> ⚠️ 后续 agent：当前工作分支是 `fix/wgc-recreate`，不是 main。验证通过后需决定是否把它合回 main / 或继续在此分支迭代。旧链在 tag `backup/before-recreate-rework-20261120` 可查。

#### 7.0zzd.1 首轮验证：重建风暴根治，但 Recreate 后帧不到达（提交 `e2e8c7a`）
- 用户实测日志（`%APPDATA%\Daozai\diagnostics\liquid-glass.log`）：换分辨率 device-lost **从 54 次降到 1 次**，`pool.Recreate` 成功、之后 `perf tick` 稳定运行无再 lost —— **重建风暴/闪烁根治**。
- **但**：Recreate 之后**0 次"导入共享纹理"/出帧**，玻璃定格。原因：微软 Screen capture 文档 + sekin.in/macmyths 明确 Recreate 会丢弃池内所有旧帧，之后必须 "resume the session" 才会重新驱动帧到达；我们只 Recreate 没重启 session。
- **修法**（`e2e8c7a`）：`recreate_after_device_lost` 在 `pool.Recreate` 后加 `session.StartCapture()`（session/item 对象保留，只在新 pool 上激活，非整体重建），错误做容错（幂等，避免误触发整体重建风暴）。
- 新 exe `4b90a59c4ffe8163`，已部署（部署前先 taskkill 停进程——exe 被占用）。用户实测：不闪了，但玻璃仍静态——Recreate+StartCapture 后帧还是 0 到达。

#### 7.0zzd.2 真因：EVENT query 非法 Begin 误判 device lost（提交 `e3055de`）
- **真因链**（日志 + 社区实证）：`s.context.Begin(D3D11_QUERY_EVENT)` 非法（微软文档明确 "Begin is disabled"）→ `GetData` 永不 signal → 200 万 spin 超时 → `signaled==0` → `GetDeviceRemovedReason().is_err()` 误判 device lost → Recreate 用 item.Size() 1920x1080 错配有效帧 420x105 → 抓屏搞坏、玻璃定格。**设备其实没真 lost**（Recreate 能建纹理、StartCapture 不报错）。
- **修法**（e3055de，在干净分支重做）：去掉 Begin，只 End + GetData（Chromium ANGLE/Dawn 实证）；spin 加 `yield_now` + 每 256 次查 device-lost 提前退出（顺带消除 200 万纯 spin 卡 UI）。Recreate 保留为 device 真 lost 的兜底。
- 新 exe `4df89c1959ff49d4`，已部署。**待用户验证**：换分辨率后玻璃正常更新、不闪、无 device-lost 误判。
- ⚠️ 这是污染链 1d03311 里的同一修复，但这次在干净分支（de4da1a + Recreate）上重做，不含其它污染提交。

#### 7.0zzd.3 精确 device-lost 判定 + HRESULT 诊断（提交 `e702e6e`）
- 用户实测 e3055de：正常启动完全正常（60+ 帧、展开收起动画都有），**只有换分辨率后** event query 超时判 device_lost → Recreate → 玻璃定格。去 Begin 有效但没根治换分辨率场景。
- **怀疑**：`GetDeviceRemovedReason().is_err()` 判定过宽——任何 Err 都被当成真 lost，可能设备其实没真 removed。
- **修法**（e702e6e）：只有明确 `0x887A0005`(DEVICE_REMOVED)/`0x887A0006`(DEVICE_RESET) 才算真 lost 触发 Recreate；其它码当偶发超时丢帧、保住会话。日志加 `reason=0x{:08X}` + spins 数。
- 新 exe `aa9159853347292b`，已部署。**这次日志会给出决定性数据**：换分辨率超时时 reason 是 0（判决过宽，本次修复即生效）还是 0x887A0005（真 lost，需从 device 重建源头解决）。待用户实测并附日志。

#### 7.0zzd.4 决定性数据：真 device removed + 冷却后重建（提交 `7ddd492`/`a29934d`）
- 用户日志：`event query 超时 spins=256，device_lost=true reason=0x887A0005` —— **真 device removed**（非误判），且 spins=256 就检出（我加的早退生效），说明**每次重建后下一帧立刻又 lost**：重建撞在 DWM 仍重置设备的窗口里。Recreate+StartCapture 也救不回帧。
- 社区权威（microsoft/Windows.UI.Composition-Win32-Samples #139 + 微软 Screen capture 文档）：device removed 需新建 device+重建资源+Recreate pool；微软也承认 WGC 内部在时机不对时不冒泡错误。
- **修法**（7ddd492）：device-lost 不再当帧立即 Recreate/init，而是**销毁死会话 + 递增冷却 2s→4s→8s**（`cooldown_after_device_lost`），期间 `settling()` 挡住抓屏；冷却结束后走正常 init 建全新会话，让 DWM/显卡彻底稳定。成功出帧 `note_capture_ok` 清计数。删除不再用的 `recreate_after_device_lost` 函数。
- 新 exe `756504f8f4e4355d`（Deployed & 校验一致）。注意：a29934d 修正了 7ddd492 误把 wgc.rs 换行改 CRLF 的问题（逻辑 diff 仅 32+/85-）。编译通过，测试未跑。
- **实测失败**：用户反馈液态玻璃回退、岛仔仍闪。日志是固定 2 秒循环：`init OK → "抓屏恢复清计数" → 立刻 0x887A0005 → 冷却 2000ms → init OK → ...`（32 次超时/33 次冷却/17 次导入）。**我重犯了污染链踩过的坑**：`note_capture_ok()` 在恢复第一帧就清 LOST_STREAK，冷却永远停在"第 1 次=2s"升不到 4s/8s；且冷却期似乎没真正挡住 init。
- **⛔ 停止打补丁**：这条线已连续十几轮"改一个机制试一次"、且反复重犯同类错误（退避清零、CRLF）。继续下去对用户不负责。**待用户决策**（见下）。

#### 7.0zze 转向：GDI 抓屏实验开关（提交 `176f3a8`，2026-11-20 23:00）
- 用户问"液态玻璃能否不依赖 D3D11"。社区调研（用户要求）找到：
  - **glass-backdrop-rs**（GitHub/MCheng404，MIT）：液态玻璃 + **纯 GDI BitBlt** 抓屏 + WGSL shader，明确不依赖 WGC/D3D11；用后台低优先级线程 + unchanged-frame detection 避免卡渲染线程。
  - **flamingo 项目 2026-10-05 实测**：Win10 2004+ 下 `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` 后用 BitBlt 抓屏，窗口区域抓到背后窗口（黑 0.000），分层窗口同样——推翻本项目 `wgc.rs:3` "GDI 下 WDA 只会变黑"的旧注释（可能基于旧 Windows/时序）。
  - **Electron-Liquid-Glass**：走 DXGI Desktop Duplication + D3D11（不用 WGC），有成熟 device-lost 流程。
- **修法**（176f3a8）：加实验开关 `DAOZAI_LIQUID_GDI_CAPTURE=1`——跳过零拷贝/WGC，`capture()` 直接用 `grab_raw`(GDI BitBlt)，保留 WDA 排除。默认关闭。
- 预览目录加 `start-gdi-test.bat`（设 GDI_CAPTURE=1 + GPU_SHARE=0 双保险）。新 exe `e0f4b7ddb7858c71`，已部署、清日志。

##### ✅ 7.0zze 实测成功——十几轮折腾后第一个真正根治的方案（2026-11-20 23:16）
- 用户用 `start-gdi-test.bat` 跑，肉眼确认正常，让我读日志判断。日志在**预览目录** `data\diagnostics\liquid-glass.log`（脚本设了 DAOZAI_DATA_DIR，不是 %APPDATA%）。
- **确证走 GDI**：全文无 `wgc:`/`init OK`/`导入共享纹理`/`device_lost`/`0x887A0005`（WGC 整条路径没运行）；有持续 `tick=... cap=... mean_r=... tex=...`（capture 正常出帧）；`exclude: 重设 0 个`（WDA 排除稳定）；无"黑岛/条纹/丢弃"诊断。
- **换分辨率扛住了**：日志有 `sig=2b2bd67f...(2560x1440) → sig=33ac4e7f...(1920x1080)` + `显示器拓扑变化，等待 500ms 稳定后重抓底片`；换后**异常关键词 0 条**，出帧 `tick=13080→15000` 完全连续无中断，`mean_r` 稳定 160-180，`plate: new` 持续更新。
- **结论**：换分辨率时是 WGC 的 D3D11 device 被 DWM 移除（0x887A0005），与液态玻璃 shader 无关；GDI 不碰 D3D11，问题从源头消失。项目 `wgc.rs:3` "GDI 下 WDA 只会变黑"的旧注释在新 Windows 上不成立。
- **⚠️ 23:35 用户实测更正（重要）**：**非零拷贝（staging，`liquid_gpu_share=0`）换分辨率也没问题**。即风暴是**零拷贝独有的 event query 超时误判**导致（会话重建后 query 未 signal 就超时→判 lost→再重建），不是所有 WGC 路径都中招。staging 的 device-lost 只在纹理 resize 失败时检测（`wgc.rs:695`），普通换分辨率不触发。之前 §7.0zx 的 "staging resize FAILED" 是**复制模式**（真重置 device），与普通换分辨率不同场景，我错误地推广了。
- **简化结论**：关掉零拷贝（`liquid_gpu_share=0`）本身就是换分辨率问题的现成解法，根本不需要 GDI 开关。GDI 只是"绕开 D3D11"的更彻底备选。详见桌面 `岛仔-抓屏与透明架构梳理.md`。
- **待用户决策**：是否转正（做 `liquid_capture=gdi|wgc` 设置项、默认仍 WGC）；是否做性能优化（GDI BitBlt 搬后台线程 + unchanged-frame detection，参考 glass-backdrop-rs，当前在主线程每帧抓可能有开销）。

##### 7.0zze.1 GDI 缺鼠标投影/边缘彩边——补画光标（提交 `9c5327f`）
- 用户实测 GDI 稳定，但对比 WGC 少两个效果：玻璃里无鼠标投影、鼠标靠近边缘无逐渐扩大的彩虹带。
- **根因**：GDI BitBlt 抓桌面 DC **不含鼠标光标**（WGC 帧含）；shader 色散 `tap5` 采 R/B 偏移，靠指针高对比像素才明显，没指针就弱到看不见。两者同源。
- **修法**（9c5327f）：`grab_raw` 加 `draw_cursor` 参数，GDI 早退传 true——BitBlt 后用 `GetCursorInfo` + `DrawIconEx` 把系统光标画进抓屏 buffer（相对矩形坐标）；底片构建传 false（干净壁纸不烙光标）、WGC 兜底传 false。光标不是窗口、不受 WDA 排除影响。
- 新 exe `e2ee484e3374e58d`，已部署（编译前停了用户正在用的进程）。用户后续决定取消 GDI 方案（见下）。

##### 7.0zze.2 GDI 方案取消，回到两模式（提交 `3e349a6`，2026-11-20 23:50）
- 用户实测：非零拷贝（staging，`liquid_gpu_share=0`）换分辨率没问题 → GDI 非必需。决定**不单独做 GDI**。
- **移除**（3 增 42 删）：`gdi_capture_enabled()` / `DAOZAI_LIQUID_GDI_CAPTURE` 配置键、capture() 的 GDI 早退、grab_raw 的 draw_cursor 参数 + DrawIconEx 补光标、零拷贝判断里的 GDI 开关；删 `start-gdi-test.bat`。grab_raw 恢复原始签名。
- **最终模式**：只有零拷贝（`liquid_gpu_share=1`）/ 非零拷贝（`=0`），由原配置控制。换分辨率问题用非零拷贝解决；零拷贝 device-lost 风暴保留此前修复。
- **耦合点解耦评估**：7 个耦合点逐个评估后决定不解耦（6 个设计必需/系统限制，1 个触发场景极窄）。避免重蹈十几轮越改越乱的覆辙。
- 新 exe `f7b93b47366235fa`，已部署。编译通过、全量测试未跑。分支 `fix/wgc-recreate` HEAD=3e349a6。

### 7.0zze.3 合并 fix 分支到 main + 打预览包 -62（2026-11-20 23:57 – 11-21 00:02）
- **合并**：`fix/wgc-recreate` → `main`，merge commit `7200ee7`。冲突只在 `wgc.rs`（main 污染链 vs fix 干净线），按约定**整个 `dz-phone/src/` 取 fix 分支**（用户确认可用的线），docs 保留 main（含最新 HANDOFF）。合并后源码与 fix 分支逐字节一致，编译通过。合并前备份 tag：`backup/main-before-merge-20261120` / `backup/fix-recreate-before-merge-20261120`。
  - 坑：只覆盖 wgc.rs 不够——main 的 gpu_glass.rs/main.rs 引用 fence API（`init_shared_fence` 等）而 fix 的 wgc.rs 没有 → 编译失败。必须整个 src 一起覆盖。
- **打包**：NSIS `makensis /INPUTCHARSET UTF8 /DPROJECT_DIR=...\dz-phone /DAPP_ID=DaozaiLiquidTest /DAPP_VERSION=1.1.1 /DSOURCE_EXE=...\daozai.exe /DOUTPUT_FILE=...\daozai-liquid-setup-62.exe installer\setup.nsi`。产物 9,295,747 B，包 sha `ddc1516892d83117`，内含 exe `91424b6863533eca`/bridge `859a91518af04c91`。已同步为桌面 `daozai-liquid-setup.exe`。
- **未做**：未 push（main ahead origin 33）、未部署正式目录、未跑全量测试（用户一贯要求）。指纹表 `_archive/README.md` + HANDOFF 速览区已更新。

### 7.0zzf 换分辨率自动重启选项（提交 `29bb757`，2026-11-21 00:10–00:17）
- 用户提议：零拷贝换分辨率时延迟重启岛仔。思路正确——进程内重建 D3D11 device 反复失败（0x887A0005），但进程重启能**同时重建 D3D11 + wgpu D3D12 两侧 device**，更彻底。
- 用户要求：做成设置页选项放零拷贝下方 + 文案提示用户（不是闪退、会自动恢复）。
- **实现**（默认关闭，行为不变）：
  - wgc.rs：poll_topology 检测拓扑变化时登记待重启（仅零拷贝 + 开关开），延迟 2s（环境变量 `DAOZAI_RESTART_DELAY_MS` 可覆盖 500–30000ms），`take_auto_restart_request()` 到点返回 true；防抖（sig 连变只重启一次）。
  - app.rs：App::frame 查询，true → `pending.push(Cmd::RestartApp)`，走已有的 spawn_replacement（保存窗口/拉新进程/等单实例锁释放）。
  - model.rs：`liquid_auto_restart` 设置项（默认 false）+ load/save；main.rs 启动时写 `DAOZAI_AUTO_RESTART_ON_RESOLUTION` 快照。
  - extras.rs：零拷贝开关下方加选项，文案"零拷贝模式专属：切换分辨率约2秒后自动重启一次，避免玻璃卡住/闪烁；这不是闪退，重启后自动恢复"；改后有"立即重启"按钮。
- **测试流程**：开零拷贝 + 开此选项 → 立即重启 → 切分辨率 → 约2s 后岛仔消失再出现（正常）。
- 新 exe `a6a899fd54e35898`，已部署预览目录。编译通过、全量测试未跑。**待用户验证**：换分辨率自动重启是否生效、玻璃是否恢复动态。未重新打包（仍是 -62 包，本改动只在预览目录 exe）。

### 7.0zzf.1 文案补充 + 撤销纯分辨率判定（提交 `3ce0dd0`，2026-11-21 00:25）
- 用户反馈切换显示器模式（复制/扩展/仅单屏）也会改变桌面签名、触发重启。曾试做"纯分辨率变化"判定（数量/位置不变仅宽高变才触发），用户最终决定算了——保持所有拓扑变化都触发。
- 判定改动已撤销（`git checkout HEAD -- wgc.rs`），逻辑回到 29bb757 状态。
- 只改文案（extras.rs）：标题「切换分辨率/显示器模式后自动重启」；说明加"切换显示器模式（复制/扩展/仅单屏）约2秒后自动重启一次"；提示补"不开时切换分辨率可能导致玻璃卡住/背景不更新（需手动重启岛仔）"。
- 新 exe `cfd17f60fccfafb0`，已部署。编译通过、测试未跑。未重新打包（预览包仍 -62）。

### 7.0zzg 社区方案研究分支（2026-11-21 00:32–00:40）
- 用户要求单开分支重新研究社区方案（重点 fence），不动 main。分支 **`research/community-fence`**（从 main `65d7380` 分出），研究文档 `docs/research-fence-device-lost.md`（桌面副本 `岛仔-fence与device-lost社区方案研究.md`），提交 `b1b7361`。**研究分支只有文档，无代码改动；main 不受影响。**
- 核心结论：
  1. **fence 不解决 device-lost**：fence 是 CPU/GPU 同步（替 event query CPU spin），device-lost 时 fence 调用照样返回 0x887A0005（Rive Flutter issue #86 佐证）。
  2. **wgpu-scry**（与本项目链路几乎一样：WGC+D3D11共享纹理→wgpu D3D12）把 explicit D3D12 fence 从 future work 升级为 shipped：host 创建 fence→producer D3D11 OpenSharedFence→每帧 `Signal(n+1)` + **新纹理**→consumer `Queue.Wait(n+1)`；每帧 fresh resource + fence_value + fail-fast + fallback。
  3. scrying/微软**都没有会话内 device-lost 恢复银弹**；健壮性来自每帧新资源 + stale/dim-match guard + 可诊断 metrics。微软 robmikh 承认 WGC 内部 device-lost bug。
  4. 结论：保持 main 的自动重启 workaround（进程级重建能同时重建 D3D11+D3D12，最可靠）；可选增强 A=stale/dim-match guard（低风险）、B=fence 替 event query（中风险、消除主线程空转）；不建议继续堆会话内恢复机制。

### 7.0zzg.1 【重要更正】社区确有成功案例——OBS Studio（2026-11-21 00:45）
- 用户追问"有没有人用新建 device + 重建资源 + frame pool.Recreate 成功过"。深挖确认：**有，生产级——OBS Studio**（`libobs-winrt/winrt-capture.cpp` + `libobs-d3d11/d3d11-subsystem.cpp`，2016 年起稳定多年）。
- **成功要素**（我们缺的）：
  1. **统一 D3D device 所有者**（OBS graphics subsystem `gs_get_device_obj`）：device lost 时统一重建 device + `gs_register_loss_callbacks` 回调所有消费者。我们的 wgpu(D3D12) 与 WGC(D3D11) 各自独立、无统一重建 → D3D11 重建了 D3D12 没跟上，共享纹理句柄失效（印证"双 device"怀疑）。
  2. **两段式** `device_loss_release`（撤销事件 + Close pool/session + 清空 COM 引用）→ 重建 device → `device_loss_rebuild`。
  3. **rebuild 时重建 `GraphicsCaptureItem`**（用保存的 window/monitor 重新 CreateForWindow/CreateForMonitor），不复用旧 item；再 `FramePool::Create` + `CreateCaptureSession` + 重注册 FrameArrived + `StartCapture`。我们 Recreate 保留旧 item，可能帧不到达。
  4. **do-while 重试直到拿到可用 device**（微软 ScreenCapturePlayer 示例同款）；我们重建一次失败就冷却/放弃。
- **结论更正**：之前"没有银弹"过于悲观。进程重启 workaround 本质=强制统一重建两侧 device，方向正确；进程内做对需模仿 OBS 统一 device 管理（大改）；低风险改进=重启前先试一次 OBS 式完整会话重建（Close→重建item→新device→Create pool→StartCapture）+重试，失败再重启。
- 研究文档 `docs/research-fence-device-lost.md` §6（研究分支 commit `4f764e8`），桌面副本同步。

### 7.0zzg.2 egui-directx11 最新版本实证（2026-11-21 10:07–10:15，研究分支）
- 用户要求"继续 egui-directx11 尝试 + 直接拉最新代码/读最新 commit/用最新版本"。已 clone `nekomaru-pku/egui-directx11` 到 `D:\code\egui-directx11`，最新 commit **`38201f9`**（2026-07-17，版本 `0.13.0+egui-0.35.0`，egui 0.35 + windows 0.62 + edition 2024；无 GitHub Release、tag 止于 0.12.1）。比 §20 推测的 0.7.0/egui0.31 又跨 4 个大版本，**版本差更大**。
- **实证通过**：`cargo build --locked --release --examples` 44s 全过；D3D11 渲染 egui UI 正常；新增 `examples/desktop_spike.rs`（独立工程，不碰岛仔主项目/Cargo.lock）验证单 D3D11 device 抓屏→`CopyResource`→`register_user_texture`→egui Image。本机 Desktop Duplication 返回 `0x80070005`（管理员权限仍拒绝，远程/虚拟会话常见，与库无关；岛仔用 WGC 不走 DDU）；改用官方 `user_texture` example 实证 **D3D11 纹理经 register_user_texture 完整显示为 egui Image**（exe sha `6ea18259e61a19ac`）。即"单 device 零拷贝展示"链路 100% 可用，可根治换分辨率 device-lost。
- **致命障碍（源码实锤）**：最新 `src/lib.rs` 的 `Renderer::render()` 对 `Primitive::Callback(..)` 直接 `warn!("paint callbacks are not yet supported")` 丢弃。岛仔液态玻璃核心 `app.rs:3701` 的 `new_paint_callback` + `GlassCallback: egui_wgpu::CallbackTrait`（`gpu_glass.rs:1441`）在该渲染器下**完全不执行**。WGSL shader `gpu_glass.rs:190-649` 约 460 行/12 函数（Snell 折射+七波长色散+Fresnel）须全部重写 HLSL，外加 eframe→winit、egui 0.31→0.35（vendor 全套回归）、windows 0.58→0.62 = **架构级重写**。
- **结论**：主项目 main 不动（保持自动重启 + `liquid_gpu_share=0` 非零拷贝，已实测解决换分辨率）；egui-directx11 作长期备选留档，除非用户明确接受重写渲染栈+玻璃重做。详见 `docs/research-fence-device-lost.md` §21；截图工作区 `spike_full.png`/`user_texture_full.png`。分支 `research/obs-capture-study`。

### 7.0zzg.3 【架构突破】HLSL玻璃+egui单D3D11 device实机跑通（2026-11-21 10:16–10:25，研究分支）
- 用户"写，你要相信自己的能力"——不再停在 §7.0zzg.2 的"架构级重写/不建议启动"，直接把可运行渲染栈写出来。**成功**。
- **推翻 zzg.2 的致命障碍判断**：之前认为 Renderer 丢弃 Primitive::Callback = 玻璃无法迁移，错在把玻璃当成必须走 egui callback。真相：玻璃本质是 UI 背后的一层背景。正确架构=单 D3D11 device：①自定义 HLSL 管线先把玻璃画到 RT（采样抓屏纹理）②egui_directx11::Renderer 再把控件画到同一 RT 上层③Present。callback 限制彻底绕过，无 eframe/wgpu、无 D3D12 跨 API 共享/fence，根治换分辨率 device-lost。
- 实证（独立工程 D:\code\egui-directx11，不碰岛仔主项目/Cargo.lock）：`shaders/glass.hlsl` 忠实移植 gpu_glass.rs WGSL 岛体光学核心（圆角 SDF、5点分通道色散、Snell 折射、Schlick 方向白边、倒角暗带、内导光+Beer-Lambert、scrim、抖动），fxc 编译 vs/ps_5_0 通过；`examples/glass_spike.rs` 单 device 每帧写 cbuffer→Draw(3)玻璃→egui 画 UI。实机截图（`glass_spike_v2_crop.png`）：圆角玻璃胶囊白边高光清晰、内部折射/压暗与外侧壁纸鲜明对比，深色状态条 egui 文字清晰可读（FPS 228）；点按钮切 FLAT 模式（`glass_spike_flat.png`）画面变回原始壁纸、按钮交互正常=玻璃是 shader 实时算的。
- 结论更新：渲染栈可行且有实证，值得作下一代方向；剩余=接 WGC 抓屏（wgc.rs 现成）+补 HLSL 刘海/控件玻璃+eframe→winit+窗口透明+egui 0.31→0.35 适配测试回归 = **明确工程清单**而非未知。建议单开 research/egui-dx11-stack 分支推进，main 保持现状。详见 docs/research-fence-device-lost.md §22；提交 egui-directx11 commit 2f1d495。

### 7.0zzg.4 HLSL 功能对齐 WGSL：刘海 SDF + 岛内控件玻璃跑通（2026-11-21 10:26–10:30）
- 用户"继续完成"——补齐 §7.0zzg.3 的两个省略项。`glass.hlsl` 新增 `notch_sdf`（刘海肩部外扩，fs 里按 notch.z 选用）、`ctrl_glass`（小尺寸透镜，材质补偿/倒角/Beer-Lambert/白边按短边缩放，位移 0.55 折）、`pick_ctl`（后覆盖前命中、0.5px 内缩）；cbuffer 按 WGSL 顺序插入 `crect[13]`/`cprm[13]` = 35×float4 = 560B（与 WGSL 注释字节数完全一致）；FLAT 与正常分支末尾都合成控件玻璃。
- 实机（`glass_ctl_crop.png`）：大玻璃胶囊内出现两块独立小玻璃卡片（各自圆角/白边/折射），状态条文字清晰 FPS 233。fxc vs/ps_5_0 通过；提交 egui-directx11 `682510c`。
- 结论：HLSL 版功能已对齐 WGSL 的岛体玻璃+刘海+控件玻璃三大块。剩余仅集成工程（接 WGC、eframe→winit、窗口透明、egui 0.31→0.35 测试回归），光学核心全部验证可用。详见 docs/research-fence-device-lost.md §23。

### 7.0zzg.5 【端到端闭环】WGC 实时抓屏 → HLSL 玻璃 → egui 单 device 零拷贝跑通（2026-11-21 10:30–10:40）
- 用户"继续"——spike 接真实桌面。新增 `examples/wgc_capture.rs`（windows 0.62 WGC 轮询式，无 async）：`RoInitialize(MTA)`（缺这步 TryGetNextFrame 拿不到帧）→ DXGI device 经 `CreateDirect3D11DeviceFromDXGIDevice` 包 WinRT `IDirect3DDevice` → `factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()` + `CreateForMonitor` → `CreateFreeThreaded` + `StartCapture`；每帧 `TryGetNextFrame` → `Surface` → **`cast::<IDirect3DDxgiInterfaceAccess>()` + `GetInterface::<ID3D11Texture2D>()`**（不能直接 cast，这是之前 CopyResource 无效根因）→ CopyResource。背景纹理 B8G8R8A8 2560x1440，HLSL `.bgr` + `quad.x` Y 翻转 flag。
- 实机 `glass_wgc_fixed.png`：状态条绿字 `BG: WGC live desktop (GPU CopyResource, zero readback)`；玻璃下方是真实桌面（浅色壁纸+QQ/Chrome 图标+中文文字方向正常），大玻璃胶囊+两块控件玻璃叠加、白边+折射清晰，FPS 227。提交 egui-directx11 `f71df6e`。
- 结论：**单 D3D11 device 端到端闭环——WGC 抓屏→GPU CopyResource→HLSL 玻璃→egui→Present，零 CPU 回读/零跨 API fence**，根治换分辨率 device-lost（main 的自动重启 workaround 在下一代不再需要）。剩余仅集成工程（eframe→winit、窗口透明/DComp、egui 0.31→0.35 测试回归），无架构级未知。详见 docs/research-fence-device-lost.md §24。

### 7.0zzg.6 集成工程：DComp 透明浮窗跑通（2026-11-21 10:40–10:55）
- 用户"集成工程"。**透明路线定案**：`CreateSwapChainForHwnd`+FLIP_DISCARD/PREMULTIPLIED 本机反复 `0x887A0001`（换参数/去 no_redirection/去 DEBUG/换 Factory2 均无效）；正确=主项目验证过的 DComp：winit `with_no_redirection_bitmap(true)`+`with_decorations(false)`（WS_EX_NOREDIRECTIONBITMAP）→ `CreateSwapChainForComposition` → `DCompositionCreateDevice2(IDXGIDevice)`→`CreateVisual`→`CreateTargetForHwnd`→`visual.SetContent(swap)`→`target.SetRoot`→每帧 `Commit`；blend 预乘、清屏 [0,0,0,0]、shader notch.w=1 逐像素 alpha；`quad.yz` 传窗口屏幕原点对齐 WGC 全屏帧。
- 实机 `glass_dcomp_full.png`：左上角 640×220 无边框窗口**透明浮在桌面**（透出壁纸/图标），内有 WGC live 玻璃+两控件玻璃，FPS 220。提交 egui-directx11 `677a7fe`。
- 集成状态：✅ egui-directx11 最新版/WGC/HLSL 玻璃全型/单 device 零拷贝/DComp 透明浮窗全跑通；⬜ 剩余=岛仔 `Shell::update` UI 迁到 winit+egui 0.35 vendor 升级+1100 测试回归+置顶/点穿/多显示器 native 行为迁移。main 不动，独立工程 `egui-directx11` commit `677a7fe` 为完整可运行参考。详见 docs/research-fence-device-lost.md §25。

### 7.0zzg.7 UI 迁移可行性验证：egui 0.31→0.35 仅 3 类破坏性变更（2026-11-21 10:51–11:00）
- 用户"继续"。写 `examples/ui_api_probe.rs` 把 app.rs/main.rs 全部关键 egui API 在 0.35 编译，5 轮编辑后全过，**仅 3 类破坏性变更**：①`i.raw_scroll_delta`→`i.smooth_scroll_delta`（app.rs 2 处）②`ColorImage::new([w,h],Color32)`→`Vec<Color32>`（1 处）③`ctx.fonts(|f|f.layout_job())`→`fonts_mut`（0.35 fonts() 给 `&FontsView`；排版处较多但纯机械）。其余全兼容（含 `send_viewport_cmd`/`StrokeKind`/`Ui::input`）。**UI 迁移=机械替换非重写**。
- glass_spike 换岛仔风格 UI（通知胶囊+居中大时间+fonts_mut 状态行），`glass_ui_crop.png` 实机显示"3 条未读"胶囊/`10:54`/状态行/玻璃折射，中文方块=默认字体无中文字形（岛仔 vendor 字体有）。提交 egui-directx11 `053466e`。
- 剩余：`Shell::update` 500 行+`app.rs` 5107 行按 3 类规则替换、vendor 字体/纹理迁移、1100+ 测试回归——**大但确定**，无架构未知。`ui_api_probe.rs` 是迁移检查清单。详见 docs/research-fence-device-lost.md §26。


### 7.0zzg.8 迁移工具落地：egui 0.35 自动替换脚本（2026-11-21 10:56–11:05）
- 用户"继续完成"。精确扫描：`app.rs` 共 10 处变更、`main.rs` 0 处。`raw_scroll_delta` 3 处（L2631读/L2675 input_mut/L3149读）；`ColorImage::new([TEX_W,TEX_H],Color32)` 1 处（L3715）；`fonts(|f|layout_job)`→`fonts_mut` 6 处。**关键例外**：main.rs:387 `fonts(|f|num_galleys_in_cache())` 是只读，0.35 fonts() 仍可用，不改（脚本只重写闭包含 layout_job 的 `.fonts(`）。
- 工具 `egui-directx11/tools/migrate_egui035.py`（commit `dcb7a0a`/`fde5f17`）：dry-run 打印 before/after，`--apply`/`--out` 写；input_mut 特殊处理（删 raw 保留 smooth）。验证：10 处精确命中、副本重跑 0 变更（幂等）、行数不变、无残留 0.31 API。探针补验 input_mut 写 smooth 与只读 fonts 在 0.35 可编译。
- 剩余：高版本分支 `--apply` → vendor 0.31→0.35 全套升级 → eframe Frame 替换 → 1100+ 测试回归。脚本+探针已把最易错的 API 变更固化为可重复步骤。详见 docs/research-fence-device-lost.md §27。


### 7.0zzg.9 【真搬落地】岛仔主代码迁 egui 0.35 编译通过并实机启动（2026-11-21 10:58–11:25，用户"让你搬啊"）
- 开 `research/egui035-migration` 分支实搬（commit `675334b`，23 files +799/-659）。vendor patch 移除、egui/eframe/egui_extras 换官方 0.35、去 wgpu feature；应用迁移脚本 + 手修 §26 探针漏报的 10+ 类变化（style_of/all_styles_mut、content_rect、ImeEvent/SizeHint struct variant、TextEdit::Frame::NONE、MouseWheel.phase、Id Hash+Debug、load_svg 3参、popup 状态私有化→stub no-op、eframe App::update→ui、Panel::show(&mut Ui) 等，详见 tools/migrate_egui035.py docstring）。
- 渲染栈降级占位：`gpu_glass_stub.rs` + paint_gpu_glass fallback + graphics.rs 简化 + main.rs 删 wgpu_setup 44 行 + native.rs wgpu 存根；真玻璃由独立工程 egui-directx11 HLSL/D3D11 层承担。修 CRLF 噪音，diff 34k→1390 行。
- 实证：`cargo check` 通过、`cargo build --release` 成功、**实机启动**（岛仔+托盘窗口，截图 `daozai_egui035_migrated.png` 顶部胶囊 11:22/19° 正常渲染；玻璃为纯色 stub 属迁移占位）。
- 剩余：①测试代码 207 个 cfg(test) 错误（171 处 ctx.run→ctx.run_ui + 36 零散）；②把 WGC+HLSL+DComp 真正接入岛仔替换 stub。main 不动。详见 docs/research-fence-device-lost.md §28。

### 7.0zzg.10 测试代码适配 egui 0.35：77 编译错误清零 + 行为回归修复（2026-11-21 11:31–12:20，接手续做）
- 前 agent 余额耗尽，报告"lib test 应已能编译"——实测仍有 **77 个编译错误**。三轮脚本逐类修到 0（viewport_ui_cb 改 &mut Ui 53 处、ColorImage new→filled/source_size、ImageData::Font 删除、MouseWheel.phase、load_svg 双参、style_mut→all_styles_mut、fonts_mut 等；examples 去 wgpu）。
- **四类 0.31→0.35 行为回归（探针漏报）**：①`raw_scroll_delta` 移除改逐帧平滑→新增 `app::raw_scroll_y()` 复刻旧语义覆盖 7 处（wheel 11→0 失败）；②theme/style 按 Theme 分离且全局→设置窗口 set_theme+set_style_of+settings_color(ctx.theme())；③popup memory API 私有化→`Popup::open_id/is_id_open/open_memory`，天气城市下拉生产功能同步恢复；④RichText 拆多 Text shape→测试 helper 按 y 分组拼接匹配。
- 实证：`cargo check --all-targets` 全过；`cargo test --lib` 从 19 failed 降到 **8 failed / 1377 passed / 131 ignored**。commit `d522521`（35 files +420/-351）+ 文档 `a5cec4c`；egui-directx11 工具脚本 3 个已提交。main 不动，未部署（研究分支渲染层仍 glow stub）。
- 剩余 8 个：2 native 黑底裁剪（glow stub 预期）+ 6 个 0.35 深度交互差（DragValue Esc/加号、tooltip 长路径省略、qweather ComboBox 点击路由）。详见 docs/research-fence-device-lost.md §29。

### 7.0zzg.11 渲染接线落地：WGC+HLSL+DComp 接进岛仔替换 glow stub（2026-11-21 12:30–14:15，用户"下一步完成渲染接线"）
- 用户选"直接完整接线 + 保持 windows 0.58 移植渲染代码"（避免升 0.62 动 6 万行 native/wgc）。
- 移植 egui-directx11 到主项目：`src/d3d11/{renderer,texture,wgc_capture,runtime}.rs` + `shaders/*.bin/hlsl`（整块复制，仅适配 0.58 的 `D3D11CreateDevice` 泛型差异、`BOOL` 路径）；新增 `egui`/`egui-winit`/`winit` 依赖（lock 只 +4 行，无版本漂移）。
- 新 `src/winit_runner.rs`：winit 0.30 事件循环替换 `eframe::run_native`；`Shell::new(egui_ctx,hwnd)` 取代 CreationContext；每帧 WGC CopyResource→HLSL 玻璃→egui→Present→DComp Commit。commit `d6d7800`。
- **已知限制：设置页仍内嵌本体**（用户观察到"设置页跑到本体"）。尝试独立窗口踩 egui 官方已知坑 issue #5229/#8326/#7776，在 draw 上下文 create_window 会让事件循环卡在第二帧，回退内嵌保稳定。功能完整可用。详见 docs/research-fence-device-lost.md §30。main 不动。

### 7.0zzg.12 液态玻璃接通（2026-11-21 14:20–15:15，用户"液态玻璃效果没有"→"岛仔把自己采样进去了"）
- commit `1fad5e0`。承接 d6d7800 的渲染栈，把 HLSL 玻璃真正显示出来。四个根因（用户观察链定位）：①stub 底色 alpha=225 遮住玻璃→20；②默认 Classic 皮肤未启用液态→测试数据设 liquid_glass；③egui(顶原点) 与 D3D11(底原点) Y 相反——bounds.y/pointer.y 翻转；④WGC 抓整屏把岛仔自己抓进去（递归自采样）→ SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)。另修 stub 的 set_* 全改真实桥接、extra.yz 传窗口尺寸、quad.x flip 关掉。验证 test --lib 1379/6/131 无回归。副作用：GDI 截图工具抓不到岛仔（与原版 wgc 一致）。

### 7.0zzg.13 接手收尾：亮度播种修复 + 高 DPI 背景放大修复（2026-11-21 15:20–16:00，用户"继续完成"→"背景被放大一倍多"）
- 接手时状态：`research/egui035-migration` 分支，HEAD `3d9268c`（7.0zzg.12 文档），工作树有 1 处未提交改动（gpu_glass_stub.rs 亮度播种真实实现），release exe sha `14fd6c6563a7f131`（含该改动，15:23 编译）。
- **① 提交遗留改动 `32c504c`**：stub 的 background_luma()/seed_theme_luma() 从恒返回 0.5/空实现改为 AtomicU32 + DAOZAI_LIQUID_FORCE_LUMA 覆盖，与真 gpu_glass.rs/wgc.rs 语义一致——否则迁移后 app 按壁纸亮度播种的 material 失效、玻璃不随壁纸深浅切换。
- **② 液态玻璃实机确认**：用 `skin=liquid_glass` 数据目录（merge-work/dpgpu-off）启动，日志 `WGC OK 2560x1440`+`glass pipeline OK`+`got first frame OK`，glass-diagnostics.txt 实时记录岛体矩形/dcomp=true/系统透明效果开，确认玻璃真在渲染（此前用默认 Classic 目录验证是错的）。
- **③ 高 DPI 背景放大 bug（本轮重点）**：用户报"玻璃内背景被放大大概一倍"。用一次性数值诊断（DAOZAI_GLASS_DIAG）定位：本机 scale_factor=1.25，物理渲染目标 `1200×338`，但 build_glass_params 把 egui 逻辑点/CSS px 直接塞进 shader，shader 认为画布只有 `960×270`（=1200/1.25），采到桌面区域缩小 1.25 倍再拉伸→背景放大 1.25 倍。**根因=移植时漏了旧栈 gpu_glass::build_uniform 的 ×ppp 换算**。修复 `6ca92e0`：build_glass_params 接收 egui_ctx.pixels_per_point()，body_rect/radius/pointer/chroma/refract/blur/band/edge_w 全乘 ppp，morph/material/edge_op 无量纲不缩放。诊断实证：extra(tex) `960×270`→`1200×337.5`（=物理 RT）、refract `48`→`60`。main 不动。
- **验证**：release 编译通过（仅既有 warning）；按用户要求未跑全量测试。exe sha 更新见速览区，未部署（研究分支）。


### 7.0zzg.14 修玻璃偏蓝不透：D3D11 栈接通实时背景亮度采样（2026-11-21 16:00–16:15，用户"玻璃不够透偏蓝"）
- 用户报玻璃偏蓝不透。诊断(DAOZAI_GLASS_DIAG)实测 `material=0.74` 卡在半深不浅区：matdark=(1-0.74)*0.7 把背景压暗叠冷白雾/青绿导光，闷蓝。
- 根因：shader 常数与旧栈逐行一致，差异在 material 来源。旧 wgpu 栈 background_luma 每帧从 WGC 帧玻璃背后实时采样(theme_rect+theme_luma_commit 过滤过渡帧)；新 D3D11 栈迁移时 stub 只读启动时静态壁纸平均值(0.74)，注释留的"后续接入 WGC 帧亮度"没做，material 被钉死。
- 修复 `6dec689`：①wgc.rs 暴露 sample_theme_luma_d3d11()(复用 staging/500ms 节流/commit 状态)；②d3d11/runtime.rs 抓屏后按玻璃矩形换算采样框(屏幕上缘锚定)读回亮度；③stub background_luma() 改读 wgc::theme_luma() 实时值，seed 转发 wgc；保留 FORCE_LUMA 覆盖。
- 实证：material 0.74→1.0(纯浅材质，matdark=0 不压暗，通透)。release 编译通过，未跑全量测试(用户要求)，未部署。main 不动。


### 7.0zzg.15 修液态玻璃严重偏色：shader 采样通道 .bgr→.rgb（2026-11-21 16:16–16:26，用户照片"偏色很严重"）
- 用户实拍 4 张照片：展开态音乐面板整体偏青蓝（暖米黄壁纸在玻璃里变冷蓝），收起态小胶囊正常。
- 排查排除项：①采样坐标/ppp（已修，位置对应一致——玻璃里能看到壁纸的金色罗盘）；②shader bin 过期（fxc 重编译哈希与 .bin 一致）；③材质层（实时亮度已接通，material=1.0 浅材质，常数与旧栈逐行一致）。
- **根因**：`glass.hlsl::sample_px` 的 `.bgr` 交换错误。照搬独立工程 egui-directx11 的写法（那套 pipeline 手动上传 BGRA 字节，需交换）；新栈 bg_tex 来自 WGC CopyResource，D3D11 SRV 采样 B8G8R8A8 已映射成正确 RGBA 语义（.r=红），再交换就把 RGB 翻一次 → 暖色(R高B低)变青蓝(B高R低)。
- 修法：`.bgr` → `.rgb`（commit 待填）。A/B 实证：改后用户确认"这下正常了"。shader 重编译 bin sha `a4fc18b5d7c74809`，release exe sha 见速览。
- 教训：移植 shader 时，采样通道语义取决于纹理来源 pipeline（手动上传 vs CopyResource），不能照抄 swizzle。


### 7.0zzg.16 清调试残留 + 删 D3D12/wgpu/WGSL 旧栈（2026-11-21 17:48–18:20，用户"继续审查重构"）
- **先调查后动手**：`gpu_glass.rs` 是孤儿文件（`lib.rs` 用 `#[path]` 把 `gpu_glass` 指向 stub），1834 行 WGSL/wgpu/D3D12 零拷贝从未参与编译；`graphics.rs` 只在非 Windows 分支运行；`wgc.rs::grab()/grab_shared()` 只被孤儿文件调用，现行 D3D11 抓屏在 `d3d11/wgc_capture.rs`。vendor 的 egui/wgpu 补丁未生效。唯一生效的 `[diag]` 是 `d3d11/runtime.rs` 的 `DAOZAI_GLASS_DIAG`。
- **删除**：`src/gpu_glass.rs`、`src/graphics.rs`；`wgc.rs` 约 700 行旧栈（SharedFrame/SharedHandle/NT handle+keyed mutex、adapter_by_luid、grab/grab_shared、自动重启、抓屏 staging）；`DAOZAI_GLASS_DIAG`、`OPEN_SETTINGS`、`OUTLINE_TEST`、`TRAY_DUMP`、`DCOMP`、零拷贝与自动重启设置桥接及两组 UI；`wgpu = 24` 直接依赖 + `Win32_Graphics_Direct3D12` feature（`Cargo.lock` -407 行）。
- **保留**：`gpu_glass_stub.rs` 数据接口（MAX_CTL/Ctl/controls/uniforms/background_luma 含 `DAOZAI_LIQUID_FORCE_LUMA`）、`wgc.rs` 亮度采样+拓扑稳定期与 11 个单测、`DAOZAI_DATA_DIR`、测试/录制/剖析类开关、产品诊断日志。
- **修正**：`wgpu_format` 占位字段删除后，液态皮肤的 `poll_topology()` 不再被恒为 None 的字段挡住（换分辨率稳定期终于生效）；`winit_runner` 删除未使用且 panic 后留悬垂指针的 unsafe 裸指针 TLS（`event_loop_context`），并去掉同一帧在 `new_events`/`about_to_wait` 双次绘制；删掉 wgpu 依赖后 windows crate 的 `Win32_System_Kernel` feature 显式补回（`SetUnhandledExceptionFilter` 曾靠传递依赖意外启用）。
- **7 个遗留测试已修复（同日追加）**：①2 个 native 用例改为断言 D3D11+DComp 的矩形窗口区域 + 逐像素 alpha（旧 wgpu 二值轮廓硬裁已整体停用）；②宠物图集 UV 断言改为半纹素内缩 `0.5/144`（commit ac177bb 修白线的预期值）；③设置页数字框用聚焦时设置快照恢复 Esc 取消语义，并在点击 +/- 后清掉 DragValue 缓存文本，避免下一帧把键入值写回覆盖步进（egui 0.35 行为差）；④置物架长路径提示改用 `Tooltip::for_widget(&r).width(900)` + `TextWrapMode::Extend`，不再被 500px 默认宽度截成省略号；⑤和风城市搜索在 0.35 下自己处理上下键/回车选中（Popup 不再代劳），测试里直接驱动省份与显式聚焦，去掉对 headless popup 状态查询的依赖。
- **验证**：`cargo check --all-targets` 清零错误；`cargo build --release` 成功（2m36s）；`cargo test --lib -- --test-threads=1` **1385 passed / 0 failed / 131 ignored**（全绿，基线 8 failed 全部清零）。研究分支未部署、main 不动。
- **遗留**：①`liquid_prefer_dgpu` 设置与环境变量保留，但 D3D11 runtime 尚未实现选卡；②设置页「兼容渲染(D3D12)」与液态玻璃配置窗 `render_compat` 门禁仍是迁移残留；③固定 16ms WaitUntil 未用 `repaint_delay`，隐藏态仍跑 60fps；④WGC 帧尺寸变化未重建 `bg_tex`；⑤vendor 旧栈目录（约 4.4MB）未删，作为归档保留。详见审查报告与 docs/research-fence-device-lost.md 后续章节。


### 7.0zzg.17 修「新建日程页」无液态玻璃：D3D11 ScissorRect 状态泄漏（2026-11-21 19:49–21:49，用户报「添加日程页控件不显示、背景也没有，仅此页」）
- **症状**：新建/编辑日程页控件无玻璃透镜、面板无玻璃底，文字压壁纸；**其他所有页正常**。
- **最终根因**：`egui renderer` 每画一个 mesh 都 `RSSetScissorRects` 到该 mesh 的 clip_rect，且该状态**跨 pass / 跨帧持续有效**。而 `d3d11::runtime::draw_glass` 画全屏玻璃三角形前只重置了 Viewport，**没重置 ScissorRect**，于是玻璃被上一帧 egui 最后一个 mesh 的小 clip 裁切。新建日程表单的最后一个 mesh（TextEdit/按钮）clip 偏小 → 玻璃整层被裁掉 → 透壁纸；其他页最后一个 mesh 的 clip 恰好覆盖面板 → 正常。这解释了「参数全对却仅此页没玻璃」的所有矛盾。
- **修复**：`draw_glass` 里画三角形前 `ctx.RSSetScissorRects(Some(&[RECT{left:0,top:0,right:win_w,bottom:win_h}]))`；import 补 `Foundation::RECT`。
- **验证**：编译通过（exe `3603f791f6387af5`），用户实机确认**玻璃已回来**。按用户要求未跑全量测试。
- **排查弯路（留痕）**：先误判 125% 缩放控件 y 翻转基准（`inner_size().height` 270 vs `extra[2]` 337.5）——坐标验算虽合理但没有实机效果，因翻转只影响控件、解释不了「背景也没有」。随后用运行时诊断证实 app 发布/bounds/crect/cbuffer/egui 形状/RT 尺寸全部正确；最后靠「shader 全屏染红编辑页仍透明」锁定问题在 GPU 状态而非参数，顺藤摸瓜找到 scissor 泄漏。染红/棋盘/日志等临时诊断代码已全部删除，shader bin 已还原。


### 7.0zzg.18 文字毛刺/不锐利（排查中，2026-11-21 21:52–22:50，用户报「所有主题文字有毛刺，之前架构没有」）
- **症状**：egui 0.35 迁移后出现，用户实拍 4 张照片（新建日程/AI 额度/健康提醒/专注页）。放大判读：整体发虚 + 小字笔画粗细不均/边缘毛躁（「不提醒」「专注时间」「日期 年-月-日」最明显），大字号（25:00）边缘毛躁；**无明显红蓝彩边**。手机拍屏有摩尔纹，已排除其干扰。**100% 缩放同样毛刺** → 排除 DPI/非整数缩放/坐标系重采样。所有主题都有 → 排除液态玻璃 shader。
- **已试无效并全部撤销**：①恢复历史误删的进程级 `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`（`main()`，曾在 29bb757 顺手删掉）——用户实测无变化；②纹理 `R8G8B8A8_UNORM`→`_SRGB`——无变化；③关闭 egui 0.35 新增的 `font_hinting`（`ctx.all_styles_mut(|s| s.visuals.text_options.font_hinting=false)`）——无变化。三次改动均已撤销，源码无残留。
- **静态排除（关键证据）**：把本工程 `d3d11/renderer.rs`+`texture.rs`+shader（`vs/ps_egui.bin` 哈希一致）与文字清晰的独立工程 `D://code//egui-directx11` 逐行/归一化对比——**逻辑完全一致**，仅 import 路径差异。sampler（`MIN_MAG_MIP_LINEAR`）、blend（`ONE, INV_SRC_ALPHA` 预乘）、rasterizer（ScissorEnable=1）、顶点换算（`pos*zoom/frame_size_scaled`）、纹理上传（R8G8B8A8_UNORM + MAP_WRITE_DISCARD）全部相同。egui 版本同为 0.35.0 同来源。
- **字体配置排除**：`theme.rs` 字体代码迁移前后一字未改（msyh.ttc/segoeui.ttf/`FontData::from_static`/`theme::font()` 全同，`git diff 8b6c0e3..e5b00b5 -- theme.rs` 只有测试里 `run`→`run_ui` 机械替换）；`paint.rs` 文字绘制生产代码零改动（diff 全是测试 + trait bound）。字号、字体文件、字形布局逻辑均未变。
- **运行时数据**：首帧 `scale_factor=1.25 ctx_ppp=1`（egui-winit 0.35 改由 `RawInput.viewports[].native_pixels_per_point` 传 ppp），`run_ui` 后 `ctx_ppp=1.25 out_ppp=1.25`（正确）；窗口物理 960×270。100% 缩放也毛刺说明 ppp 非根因。
- **剩余唯一怀疑**：egui 0.31→0.35 内部 epaint 字体栅格化实现差异（0.32 起有 `Improve text sharpness #5838`、`Improve texture filtering by doing it in gamma space #7311`、`Make the font atlas use a color image #7298`；0.35 新增 `font_hinting`/`HintingTarget::Smooth`、`subpixel_binning`）。已关闭 hinting 无效，但 gamma/图集格式机制变化未逐一验证。
- **版本回退评估（用户问「只降版本改动多吗」）**：在 `D://code//daozai-custom//merge-work//egui031-test`（独立工程副本）实测：Cargo.toml 三个版本号 0.35→0.31 + 3 处 API 适配（`OrderedViewportIdMap`→`ViewportIdMap`；`ImageData::Font` 两个 match 分支）→ lib 编译过；example 再 4 处（`run_ui`→`run`、`fonts_mut`→`fonts`、包 CentralPanel）→ **全编译过，0 错误**。结论：**只降版本改动量小、可行**。但独立工程用 egui 默认字体无中文（显示方框），无法直接对比中文毛刺；要公平对比需把岛仔 theme.rs 中文字体加载逻辑（约 100 行 + memmap2）搬进测试工程，或直接在本项目副本上只降版本。
- **下一步（待用户定）**：①在测试工程搭中文字体后对比 0.31 vs 0.35 字形（干净、不碰岛仔）；②在本项目副本只降版本看毛刺是否消失（最贴近真实，但要处理 Cargo.lock + API 适配）；③搁置。
- **现场状态**：主项目工作区干净，HEAD `e5b00b5`（7.0zzg.17 玻璃修复，用户已确认生效）；磁盘 exe 已从诊断版重编译恢复干净版。临时诊断/测试代码全删，shader bin 还原原始 `a4fc18b5`。未跑全量测试、未部署、main 不动。

### 7.0zzg.19 【验证结论】独立工程 0.31 vs 0.35 中文字形对比 —— 排除 egui 版本本身（2026-11-21 22:58–23:10）
- **用户指定**：不走降版本改岛仔，先在独立工程做纯验证："把 egui 降到 0.31 渲染同样的中文，直接对比 0.31 vs 0.35 字形——纯验证，不碰岛仔"。
- **实验搭建**（两个工程都只加了 example，未动任何生产代码）：
  - 0.35 工程 `D:\code\egui-directx11\examples\font_compare.rs`；0.31 副本 `D:\code\daozai-custom\merge-work\egui031-test\examples\font_compare.rs`（后者版本早已降好，HANDOFF §7.0zzg.18 建的）。
  - 两版用**完全相同的字体加载**：`segoeui.ttf`(index 0) + `msyh.ttc`(index 1)，`FontData::from_owned` + `d.index`，与岛仔 `theme.rs` 一致。
  - 两版画**完全相同的文本**（"不提醒 专注时间 2026年-11月-21日" / 14号正文 / 12号小字 / 28号 "25:00"），同样 `CentralPanel` + 白底、**离屏 RTV 渲染 → CopyResource 读回 → 自写 24bit BMP**（绕开手机拍屏摩尔纹，直接取 GPU 像素）。
  - 坑：字体图集纹理 delta 只在第 0 帧产生，必须**每帧都跑 `renderer.render`**（含 clear）才能把字体纹理灌进 pool；只 render 最后一帧 → 文字全丢（首次跑出全白 BMP，已修正）。
  - 产物：`egui-directx11ontcmp-035.bmp` 与 `egui031-testontcmp-031.bmp`、**`cmp-side-by-side.png` / `cmp-side-by-side-3x.png`（严格同条件：两版同背景 RGB(27,27,27)、同字色、同裁剪 (0,0)-(820,180)、左右并排，最终采用的公平对比图）**；早期 `fontcmp-compare.png` 因 0.35 那版当时未套 CentralPanel（白底 vs 深底）不公平，已弃用。`line-031/035.png`、`diff-x8.png` 为辅助。
- **结论（确定性）**：
  1. **两版字形都清晰锐利，0.35 不比 0.31 糊**。放大4倍肉眼无可见差异；0.35 那组甚至略锐。
  2. **逐像素 diff**：平均通道差 1.58/255，仅 4.58% 像素有差异，`diff-x8` 显示差异只落在文字笔画轮廓上、背景全一致、**无整行重影/偏移**。属 hinting/subpixel 灰阶微观差别，不是字形或几何问题。
  3. **抗锯齿过渡带占比**：0.31 中间灰 10.5% vs 0.35 8.8%——0.35 实心笔画更多、过渡带更窄，**理论上更锐**，`color_transfer_function`(gamma 图集) 没有把字变糊。
  4. **⇒ egui 0.31→0.35 的字体栅格化/图集机制不是毛刺根因**。版本降级（下一步②）大概率**无效**，不必去动岛仔 Cargo.lock。§7.0zzg.18「剩余唯一怀疑=epaint 栅格化差异」被证伪。
- **新方向（下一步）**：既然纯 egui 文字清晰，毛刺只能来自岛仔特有管线——重点排查 ①岛仔 UI 的**实际 ppp/zoom**（HANDOFF 记 `ctx_ppp=1.25`，但独立工程这版 ppp=1，需在岛仔实机打印 `content_rect`/`tessellate` 传入的 ppp 并对比物理窗口尺寸）；②**液态玻璃 pass 与 egui pass 之间的 RT/采样状态**（scissor 泄漏已在 7.0zzg.17 修过一次，同类状态问题仍可疑）；③岛仔是否对 egui 输出做了额外缩放/采样（DComp surface 尺寸 vs 逻辑尺寸）。建议下一步在岛仔做 A/B：临时把 glass pass 整个跳过，只留纯 egui，实机截图看毛刺是否消失——一刀切定位是"egui 本身"还是"玻璃管线叠加"。
- **现场状态**：两工程仅新增 `examples/font_compare.rs`（验证用，未提交）；岛仔主工程 `.workbuddy` 源码与 HEAD `e5b00b5` 未动。未跑全量测试、未部署。

### 7.0zzg.20 岛仔内排查 + 字形度量探针（2026-11-21 23:10–23:52，用户"跟液态玻璃无关，查社区/迁移删了啥"）
- **用户澄清症状**（关键，推翻此前"毛刺=抗锯齿"的判断）：不是发虚彩边，而是**"文字变形、有些部位变小、跟其他字一个高一个低"**；**所有文字/所有字号**都这样，**一启动就这样**（非运行后），**迁移后立刻出现**。⇒ 排除图集 fill_ratio>0.8 重建、排除歌词 Galley 跨帧缓存（非仅歌词、非延时）。
- **社区排查结论**：egui 0.35 相关已知坑有三类——①`Fonts::begin_pass` 在 `TextOptions` 变化或 `atlas.fill_ratio()>0.8` 时整张重建图集 + 清空 galley_cache（#219/#0004/#0005）；②`Visuals::dark()` 带 `color_transfer_function=TwoCoverageMinusCoverageSq`，light=Off；③0.34 起 ab_glyph→skrifa+vello_cpu。但均不解释"一启动全量变形"。
- **岛仔内诊断**（临时开关，已全部撤销、源码恢复 HEAD e5b00b5）：加过 `DAOZAI_SKIP_GLASS`/`DUMP_RT`/`NO_EXCLUDE`/`DIAG`/`ATLAS_SIDE`/`TF_OFF`/`NO_GALLEY_CACHE` 做 A/B。
  - **决定性证据**：从 D3D11 RT 直接 `CopyResource` 读回 GPU 原始像素（绕开 WDA_EXCLUDEFROMCAPTURE 与手机拍屏摩尔纹），收起态/展开态中文（"杀死那个石家庄人""专注时间""25:00"）**全部清晰锐利**，无变形。诊断值 `win_inner=1200×338 rt_phys=1200×338 egui_ppp=1.25 zoom=1`，`max_texture_side=None`(=2048)。
  - transfer function `DARK→Off` A/B：字形灰阶有变（中间灰 69.9%→62.9%），但**清晰度无差**，非根因。跳过 glass pass、放大图集到 4096、ppp=1.25 独立工程复现——全部清晰。
- **字形度量探针** `examples/font_probe.rs`（0.35 与 0.31 各一份）：用岛仔同款字体链（segoeui idx0 + msyh.ttc idx1）layout"专注时间不提醒 2026年11月21日"，逐字打印 pos.y/ascent/uv/advance。**结果：0.31 与 0.35 行为一致**——中文 `pos.y=15.0 face_asc=14.2`，数字 `pos.y=16.0 face_asc=15.1`，同句混排本就差 1px（两版都如此，非迁移引入）。差异仅在 advance 14.2→14.0、uv_h 14→12/13（skrifa 栅格化略小），不足以造成变形。
- **【已解决，见 §7.0zzg.21】** 上述"困境"是当时的误判。用户后续澄清症状是"变形/大小不一/高低不齐"而非抗锯齿毛刺，并确认"配置全一样、一启动就所有字、迁移后出现"。真正根因是 0.35 的 skrifa hinting + subpixel_binning（字形压小）+ 液态皮肤按钮弹簧导致字号连续变化（字形重排抖动），均已修复并提交 `7a579d2`。环境/DPI 假设作废。

### 7.0zzg.21 【已修复】文字变形 + 液态皮肤控件抖动的真正根因（2026-11-21 23:55–2026-11-22 01:05，提交 `7a579d2`）
- **用户关键澄清**（推翻此前所有"毛刺=抗锯齿"方向）：症状是**文字变形、有些部位变小、字高低不齐**；所有文字/所有字号、**一启动就**、**迁移后出现**；配置与 agent 机器完全相同。后又补充："液态玻璃下抖、其他皮肤不抖"，且"跟之前翻页后控件抖动是同一个问题"。
- **两个独立根因**（都是 0.35 引入，都已有实证）：
  1. **字形变形**：0.35 用 skrifa 替换 ab_glyph，默认 `font_hinting=true`（把中文字形垂直压小，探针实测 uv_h 14→12）+ `subpixel_binning=true`（笔画粗细不均）。修复：`App::new` 的 `all_styles_mut` 里关闭二者，`#[cfg(not(test))]` 隔离（UI 测试断言坐标，保持基线）。用户实机确认字形**完全恢复正常**。
  2. **液态皮肤控件抖动**：`Card::font()` 用 `font(size * s)`，s=弹簧缩放（按压/入场）。液态皮肤下 `pages.rs` 按钮走 `c.scaled(..., ctl_scale(ctl))`，弹簧 s 振荡（逐帧日志实测 0.95↔1.0，约 2 秒），字号 `size*s` 每帧连续变化 → egui 0.35 为每帧新字号重新栅格化字形 → 字位左右抖 1px。其他皮肤走 `capsule()` 不缩放，故不抖。修复：`Card::font()` 把字号量化到 1/8pt（`#[cfg(not(test))]`），字形缓存命中稳定。用户实机确认**抖动小了很多**，仅"歌"字长按残余极轻微。
- **为何 §7.0ze 的 6 次抖动修复全失败**：改 ζ / 锁 60fps / progress snap 都在因果链最左端（弹簧参数）打转，真正的放大器是中间的"字号连续变化→egui 重排"，之前没人碰 `Card::font()`。
- **排除记录**（避免重走）：egui 版本降级（独立工程 0.31 vs 0.35 同字体字形一致，无效）、DPI/缩放环境（用户配置相同）、GPU 玻璃 pass（SKIP_GLASS 仍抖）、控件 GPU 透镜（NO_CTL_GLASS 仍抖）、RT 读回分辨率、transfer function、text 起点 snap（字形未量化时无效）。
- **社区更优方案（技术债，暂不采纳）**：egui 官方讨论 #3197、Bevy 警告 B0005 都指出正解是**固定字号排版 + 用 Transform::scale 缩放 mesh 顶点**，而非改字号。我试过用 `Shape::scale`，但 `clock`/`weather_ui` 大量依赖 `galley.mesh_bounds`（与字号成正比）做垂直对齐，只改 text 会破坏对齐测试；统一改造需动 paint.rs/weather_ui.rs 等多文件，风险大于当前收益。当前 1/8pt 量化是"廉价版 transform"，抓住同一根因，已验证有效。将来若系统重构文字系统可切换至官方方案。
- **验证**：全量 `cargo test --locked --release --lib -- --test-threads=1` = **1385 passed / 0 failed / 131 ignored**（两处修复均 cfg(not(test))，测试构建=基线）。exe sha（量化版）`3a4e68c885940a22`。未部署、未打包、main 不动（在 research/egui035-migration 分支提交 `7a579d2`）。


### 7.0zzm 【已修复】设置页串渲染到岛仔本体 → 独立原生窗口（2026-11-22 01:05–02:32，提交 `9ebb6c0`）
- **症状**：打开设置页后，设置内容画在岛仔本体小窗上（肉眼看屏幕，非抓屏问题）。用户明确要独立窗口。
- **根因**：迁移 D3D11 自绘（d6d7800）时漏接多窗口。`winit_runner.rs:137` 原注释写明"因担心 draw 上下文 create_window 死锁，暂保持 egui 默认内嵌：设置页画在本体"。两层原因：①egui 0.35 `embed_viewports` **默认 true**（eframe 会设 false，我们自己管却没设）；②未注册 `set_immediate_viewport_renderer`。两者叠加 → `show_viewport_immediate` 走 `show_embedded_viewport`，把设置页当 egui Window 画进主岛 RT。
- **社区/官方调研**：OBS 的 WDA_EXCLUDEFROMCAPTURE 线索排查后排除（用户肉眼可见，非捕获问题）。正解来自 eframe 0.35 官方源码：`native/glow_integration.rs` 的 `set_immediate_viewport_renderer` 注册 + `render_immediate_viewport` 同步渲染，`native/event_loop_context.rs` 的 thread_local ActiveEventLoop 机制（解决 draw 上下文建窗口死锁）。另确认 egui#3197 的 viewport 多窗口语义。
- **关键设计约束（踩过的坑）**：`show_viewport_immediate` 的 `viewport_ui_cb` 是 `Box<dyn FnMut + 'a>`，egui 在 `show_viewport_immediate` 内**同步**要求 renderer 回调执行它（`out.expect`），**不能跨帧存**（一开始想存进队列导致生命周期编译错误）。它在主窗口 draw 上下文执行、借用 `&mut App`——但通过 `run_ui(input, |ui| viewport_ui_cb(ui))` 同步调用是合法的（嵌套 reborrow），**不需要把 Shell 改成 Rc<RefCell>**。
- **修复**（winit_runner.rs + d3d11/runtime.rs）：
  1. thread_local `CURRENT_EVENT_LOOP` + guard（eframe 同款），事件方法入口 `with_event_loop_context`。
  2. `set_embed_viewports(false)` + 注册 immediate renderer：回调里 create_window → 同步 run_ui → FullOutput 存 `VIEWPORT_REQ` 队列。
  3. 窗口按 viewport id 复用（`Rc<Window>` + `VIEWPORT_WINDOWS`）：否则 immediate 回调每帧触发、每帧新建 invisible 窗口（builder `with_visible(false)`）。
  4. 子窗口输入：`window_event` 经 egui_winit 累积 → `VIEWPORT_INPUTS` 缓存 → 回调取出喂给 run_ui（含鼠标键盘，交互正常）。
  5. 主窗口 output 里子 viewport 的命令（`Visible`/位置/尺寸）：`FrameStats.viewport_commands` 传出 → `process_viewport_commands` 应用到子窗口。首帧 Visible 命令因窗口未建而丢失，drain 建窗后主动 `set_visible(true)` 兜底。
  6. `PlainWindow::paint_output`（只画 output 命令+render，不再 run_ui）+ 每帧绘制**所有**子窗口（原"只画 invisible"是内嵌 workaround，独立窗口后必须每帧画，否则内容区白屏）。
- **验证**：实机截图确认设置页独立窗口正常显示（侧栏通用/外观/位置/音乐… + 右侧设置项），点击"外观"成功切换内容。全量 `cargo test --locked --release --lib -- --test-threads=1` = 1385 passed / 0 failed / 131 ignored。诊断开关全清。诊断版 last-error 曾报 `raw_input.rs:110`（RawInput 缺 viewports 里的当前 viewport），已修（回调 input 填 viewports[id] ViewportInfo）。
- **状态**：提交 `9ebb6c0`（分支 research/egui035-migration）。未部署、未打包、main 不动。其他 3 个 `show_viewport_immediate`（邮件阅读器/更新/天气教程）同样受益于该机制，未逐一实机验证。

**【后续修复】右上角 X 关不掉（提交 `bf6f177`）**：上版窗口能开能交互，但 X 无效。诊断证实事件链全通（CloseRequested→注入 ViewportEvent::Close→设置页 `close_requested()` 读到并置 `settings_open=false`），但窗口对象仍残留。根因两层：①`window_event` 的 CloseRequested 只 push 到**从未消费**的 `pending_close`，没转成 egui close 事件；②`settings_open=false` 后不再 show_viewport，但 `subs`/`VIEWPORT_WINDOWS` 里的窗口对象没销毁。修复：CloseRequested → thread_local `CLOSE_REQ` → immediate 回调注入 `ViewportInfo.events[Close]`；`FrameStats.active_viewports` 传出本帧活跃 viewport，不活跃的窗口销毁并清理缓存（eframe `remove_viewports_not_in` 同款）。实测 WM_CLOSE 后窗口销毁、进程存活，全量 1385 passed。

- **【后续修复】打开设置页后文字错乱+收起展开闪退（提交 `ab2d67d`）**：last-error 定位 `texture.rs:171` `panic_bounds_check`（图集 partial 更新越界）。根因：egui 字体图集是 Context 级单例，但主窗口与每个 PlainWindow 各自建了独立 device + Renderer + TexturePool。设置页栅格化新字形扩展图集 → egui 给所有 viewport 发 partial TexturesDelta → 主窗口旧图集尺寸小、pos 套上去越界。修复：主窗口 egui_renderer 改 `Rc<RefCell<Renderer>>`，新增 device()/device_context()/shared_renderer()；PlainWindow::new 接收共享 device/ctx/renderer（从 device 取所属 factory 建 swapchain，不再自建），render 走 borrow_mut。实测设置页文字正常、20 次展开/收起无崩溃、全量 1385 passed。经验：egui 多 viewport 必须共用一个 Renderer/TexturePool（eframe 同款），各建一个是隐患。

### 7.0zzp 分栏容器高斯模糊磨砂、去掉暗底（2026-11-22 20:20–20:40，提交 `7cf3b9f`）
- **来源**：用户从分享页「查看液态玻璃效果层次」继续任务；上一轮对话里用户先说「可以不要暗底」，队列消息「做成高斯模糊的玻璃」。接手时工作树是半成品：`app.rs` 已把 ShelfTabBar style 改成 0，`glass.hlsl` 已加 `gauss_blur` 但没在 fs 里用，bin/exe 未重编。
- **实现**：容器 style=1 不再 `×0.55` 压暗、加深色 tint；在 `fs` 里用新增的 13 点高斯采样 `gauss_blur(p, 6.0)`（中心 + 内环4 + 对角4 + 外环4，按距离加权）取模糊背景，以内深权重 `smoothstep(0,7,-cs.d)*0.85` 替换 `ce.rgb` 内部，边缘仍走 `ctrl_glass` 的白边/倒角；模糊底套用同款浅材质补偿但不加 scrim。指示器 style=2 保持提亮 + 强白边，层次仍在。
- **坑**：HLSL 编译成功只说明语法对，函数没接进 fs 就是死代码——这次接手第一步就是检查函数是否真被调用。
- **验证**：fxc bin `e6065fb0`→`f0c07099`，exe `33d42186`→`c6b38aea44220ef9`；全量 `cargo test --locked --release --lib -- --test-threads=1` = **1391 passed / 0 failed / 131 ignored**。已启动 `target/release/daozai.exe`（PID 21532）供用户实机查看；桌面预览目录 `daozai.exe` 已同步为新 exe（旧版备份 `daozai.exe.prev-before-frost`）。未打正式安装包，`main` 未动。
- **【20:55 追加修正·文字抖动的阻尼归因】**：用户实测「阻尼比调到最大，文字抖动就不明显」。此前 20:45 的审查结论「`c.scaled()+centered_text` 不会造成上下抖动」**只对了一半**：文字*中心*确实是不动点（`b.pos(124,18)≡c.pos(124,18)`），但字形*尺寸*随 `b.s` 连续缩放，所以上下边缘会胀缩，肉眼看就是上下抖。完整链路：`extras.rs` 阻尼比滑杆 → `settings.liquid_zeta`（默认 0.5，范围 0.1–1.0）→ `app.rs:3683-3705` 每帧 `ctl_press[i].tune(300*sk, zt)` / `ctl_appear[i].tune(320*sk, zt)` → `anim.rs::Spring::tune` 的 `c=2ζ√k` 决定过冲/振荡 → `ctl_scale()`（appear=0.82+0.18·pos，press=0.14·sk·pos）→ `c.scaled(124,18,k)` → `Card::paint_galley` 的 `Shape::transform(origin,s)` 缩放 mesh 顶点。**ζ<1 过冲振荡（appear ζ=0.55 约 13%、press ζ=0.5 约 16%），ζ=1 单调收敛**，所以调大阻尼能消除抖动。c2a826a 把旧的 1/8pt 字号量化换成 mesh 连续缩放后，尾振不再被死区吞掉，故重新可见。非 GPU 分支 `paint::capsule` 用 `hover_t`/`animate_bool_with_time`，不吃 `liquid_zeta`。修法候选：文字用未缩放 `c` 画（彻底解耦但按钮放大时字不跟着大）／给文字的 s 加死区或量化（推荐，保留按下放大、吞掉尾振）／文字单独用 ζ=1 弹簧。
- **【已实施 21:05，提交 `83459b6`】** 采用「给文字的 s 加死区」：`Card::scaled` 记录 `(不动点局部坐标, 基准 s)` 到新字段 `text_pivot`；`Card::paint_galley` 在 `|k-1| < 0.06`（`TEXT_SCALE_DEADZONE`）时把文字锁回基准缩放，并重算 origin 使不动点屏幕位置不变（否则 s=1 会让文字跑偏）。UV 内缩判断也改用生效后的 s。效果：ζ 的 13%/16% 尾振（|k-1|≈0.02）被吞掉，按下 14% 放大仍保留。全局生效，所有经 `scaled()` 的卡片文字都受益，调用点无需改。全量 1391 passed / 0 failed / 131 ignored；exe `136be0d614d9eddec2`，桌面预览目录已同步（旧版备份 `daozai.exe.prev-before-text-deadzone`），PID 15688。

### 7.0zzq paint 分层重构 + 像素对齐 + 文字缩放策略显式化（2026-11-22 21:17–21:35，提交 `ff02623`）
- **用户诉求**：先审查 `paint.rs`（是干嘛的、是不是缩放/动画、是不是无损缩放），认为它带来的抖动 bug 太多该重构；批准阶段①的 1+2+3（原话「123开始，不进行全量测试」）。
- **定位**：`paint.rs` 是**绘制层**——`Card` 把面板局部逻辑点映射到屏幕（裁剪/整体透明度/`live` 交互开关），外加全部矢量图标；动画在 `anim.rs`（Spring）+ `app.rs:update_ctl_springs`（`ctl_press`/`ctl_appear`），不在这里。所谓「无损缩放」在位图字形图集上不存在：epaint 0.35 `texture_atlas.rs` 图集恒 `TextureOptions::LINEAR`（无 mipmap），`TextShape::transform` 只缩 mesh 顶点不改 UV，任意 `s` 都是重采样；社区唯一认可做法=固定字号排版 + Transform 缩放（`c2a826a` 已采用，egui #3197 / Bevy B0005），真正无损要 MSDF 或 DirectWrite 矢量文字（egui Issue #56 多年未做），成本大，不做。
- **改动**（`dz-phone/src/paint/`）：
  1. **拆文件**：`mod.rs` = Card/文字/图形/交互/玻璃控件；`icons.rs` = 全部矢量图标 + `FileKind`/`file_kind`/`file_glyph` + `clock_width`/`clock` + `pin`/`clipboard_glyph`；`chase.rs` = `CHASE_ROWS` + `chase`。`mod.rs` 末尾 `pub use icons::*; pub use chase::chase;`，`paint::battery(...)`/`paint::chase(...)` 等外部路径全部不变；Card 公有字段继续 pub（`pages.rs:1611 row.s=c.s` 这类直接赋值保留）。
  2. **像素对齐**：Card 新增私有 `ppp`（`Card::new` 时取 `ctx.pixels_per_point()`），公开 `snap`/`snap_center`/`snapped_pos`。`rect()` 把屏幕边对齐到整像素边界（填充/渐变/裁剪/命中框统一）；`stroke()` 对 ≤2 物理像素的细线把路径对齐到像素中心，1px 白边不再被两个像素各吃一半。`circle/ring/path/seg` 暂不对齐——圆形与斜线本来靠抗锯齿，硬对齐反而破坏图标形状。
  3. **文字缩放**：新增 `pub enum TextScale { Auto, Follow, Damped, Frozen }` + `Card::text_scale(mode)`；默认 `Auto`（发生过 `scaled()` 时等价 `Damped`，行为与旧 6% 死区一致）。旧实现在死区边界从 1.0 硬跳回 k（字号瞬跳 6%），现改为 `damped_text_scale()`：死区内冻结；`[死区,2×死区]` smoothstep 平滑过渡；更远处跟随 k（按压 14% 完整保留）并量化到 1/64，稳定重采样相位。`text_pivot` 更名 `text_anchor`，语义不变（后写覆盖=只补偿最内层弹簧）。
- **验证**：`cargo check --lib` 通过（paint 无新增警告）；`cargo test --lib paint::gradient_tests -- --test-threads=1` = **5 passed**（新增 `text_scale_damps_tail_without_boundary_jump`）；`cargo build --release` 成功，exe sha256 前 16 位 `66cd6f33540c2a91`。旧进程（PID 12784，构建时锁住 exe 导致首次 remove 失败）停止后，用新 exe 启动（PID 31496，`target\release\daozai.exe`）。**按用户要求未跑全量测试**，视觉待实机确认。
- **风险/后续**：像素对齐会让绘制位置移动 ≤0.5 物理像素，布局类 UI 测试未跑；若实机发现整体偏半像素，先检查 `snap`/`snap_center` 的选择。`chase` 每帧重建 mesh、`vgrad` 每帧 `tessellate`、`wrap` O(n²) 未动（阶段②）；无损缩放需 MSDF/DirectWrite（阶段③）。

### 7.0zzr paint 阶段②：追光缓存 + 折行二分（2026-11-22 21:36–21:42，提交 `ad04048`）
- **背景**：阶段①合入后用户「继续」，按既定阶段②处理 paint 内的两处热点；`vgrad` 每帧 `ctx.tessellate` 经评估只对可见渐变块调用一次，未改（避免为不可测量收益引入回归）。
- **追光 mesh 缓存**（`src/paint/chase.rs`）：
  - 新增 `ChaseKey{x0,head,y,thick,tint,hot,s,origin,ppp}` + `ChaseCache{key,mesh}`，存 `ctx.data`（id = `("paint::chase", ui.id())`）。
  - 播放头量化 `HEAD_QUANTUM=0.25` DIP、`hot` 量化 1/256 后再**同时用于缓存键和建 mesh**——命中与冷重建的顶点完全一致（有测试 `chase_mesh_cache_reuses_subpixel_steps`：0.1 DIP 命中、跨格重建、冷热 mesh 相等）。
  - 收益：播放时头部每帧只走 ~0.02 DIP，原来每帧重算 ~7000 顶点 × 8 个 `exp/powf`；现在每跨 0.25 DIP 才重建一次，其余帧只克隆已有 mesh。
- **折行改二分**（`src/paint/mod.rs` `wrap`）：每行二分定位折行点（O(log n) 次排版/行），旧实现每加一个字符重排整行，长错误信息 O(n²)。贪心语义逐条保持（行内尽量满、单字符超宽保留、`\n` 保留空行、`max_lines` 截断走 `fit`）；`max_lines==0` 显式返回空。
  - 新增测试 `wrap_matches_char_by_char_reference`：把旧逐字符算法留在测试里当基准，对 9 段文本（中文/长 URL/连续换行/表情/混排）× 5 种宽度 × 4 种行数断言结果**完全一致**。
- **验证**：`cargo test --lib paint:: -- --test-threads=1` = **16 passed**（paint 7 个 + skin_paint 9 个，0.92s）；`cargo check --lib` paint 无警告；`compact_` 像素回归 3 个测试在阶段①后已通过（本轮仅改折行/网格缓存，未再跑）。release 构建成功，exe sha256 前 16 位 `dd4ce428b646eb17`；旧进程停止后新 exe 启动（PID 32032），桌面预览目录已同步（旧版备份 `daozai.exe.prev-before-phase2`）。
- **剩余（阶段③）**：`vgrad` 预烘焙、`circle/ring/seg` 像素对齐、`inset_text_uv` 每帧深拷贝 galley、MSDF / DirectWrite 矢量文字（真无损）。

### 7.0zzs paint 像素对齐补全：圆点/圆环/轴对齐细线（2026-11-22 21:44–21:50，提交 `2f95230`）
- **背景**：阶段①只对齐了矩形填充与描边，圆形与斜线当时判断「硬对齐会破坏图标形状」而跳过；用户继续推进后，只补安全的那一半：
  - `circle`/`ring`：圆心走 `snapped_pos`（像素中心），**半径保持连续**——静态圆点覆盖比例稳定，进度圆环动画不会被量化成 1px 台阶。
  - `seg`：新增 `seg_points()`，仅当线段轴对齐（端点差 <0.01）且 `width*s*ppp<=2` 时把横向坐标放到像素中心；斜线端点原样返回（不影响图标斜率）。
  - `stroke`：抽成 `stroke_rect()`，语义未变（薄线中心、粗线边界），便于测试。
- **新增测试** `pixel_snapping_lands_on_device_pixels`（ppp 1.0/1.25/1.5/2.0）：矩形四边落在设备像素网格、最小 1px 不被吞成 0、薄描边落像素中心、垂直细线中心对齐而斜线端点不动。
- **验证**：`cargo test --lib paint:: -- --test-threads=1` = **17 passed**；`compact_` 像素回归 3 个测试仍通过（73.75s，共 8 passed）；release 构建成功；exe sha256 前 16 位 `10d7734798ff3394`，新进程 PID 17428；桌面预览目录同步，旧版备份 `daozai.exe.prev-before-snap2`。
- **剩余**：`vgrad` 预烘焙、`inset_text_uv` 每帧 deep copy galley 缓存、MSDF/DirectWrite（阶段③）。

### 7.0zzt 锁定「量化格内字形不动」不变量（2026-11-22 21:52–21:54，提交随后）
- `paint_galley` 拆成 `paint_galley` + 纯函数 `galley_shape(pos, galley, color) -> Shape`：后者返回已按当前 `s`/`TextScale` 变形（含 `inset_text_uv`）的图元，绘制函数只剩 `painter.add`。
- 新增测试 `scaled_text_mesh_holds_still_inside_one_quantum_cell`（ppp 1.0/1.5，中英混排文本）：
  - 死区内 `k=1.0` 与 `k=1.0+死区/2` 的行网格/顶点逐位相同；
  - 同一量化格内 `k=1.2` 与 `k=1.2+1/256` 逐位相同；
  - 跨两格 `k=1.2+1/32` 必须不同（防止死区+量化把动画吞掉）。
  - 这是抖动的可执行定义：**相邻帧同一段文字生成了不同顶点**。
- 验证：`cargo test --lib paint:: -- --test-threads=1` = **18 passed**；release 构建 2m47s；exe sha256 前 16 位 `205b1e2375b4c578`，PID 26072；预览目录同步，旧版备份 `daozai.exe.prev-before-galley-shape`。

### 7.0zzu 去掉文字缩放量化：修「长按新增按钮松手文字上下抽动」（2026-11-22 22:00–22:06）
- 用户反馈：日程页「＋ 新增」长按再松手，文字上下抖动。
- 层级排查（`calendar_ui.rs:274-288`）：命中区 `c.hit(101,9,46,18,"calendar-add-local")` 画在**未缩放**页卡上；玻璃本体要么由 GPU 透镜画（`Ctl::CalAdd` 已在 app.rs:3580 注册给 `pick_ctl`），要么由 `paint::capsule()` 返回 `(Response, Card)`；文字 `b.centered_text(124,18,...)` 画在 `b = c.scaled(124,18,ctl_scale(CalAdd))` 上——只有这张缩放卡带 `text_anchor`，才走 `Damped`。
- 根因：`CTL_PRESS=(300,0.5)` 欠阻尼 + `CTL_PRESS_SCALE=0.14*sk`，松手时 k 在 1.16 ↔ 0.84 之间摆；`Damped` 里我加的 1/64 量化每档 ≈0.016 缩放比，回弹一次跨 ~20 档，60fps 下几乎每帧跳一档 → 文字逐帧整档变大变小，看起来就是上下抽动。量化只在死区附近有意义，而死区本来就把那一带锁死了。
- 修法：`damped_text_scale` 只保留 `|k-1|<6% → 1.0` + `[6%,12%]` smoothstep 过渡 + 更远**连续跟随** k；删掉 `TEXT_SCALE_QUANTUM`。同步更新测试：`text_scale_damps_tail_without_boundary_jump` 增加「死区外连续单调、相邻采样差 ≤0.01」；`scaled_text_mesh_holds_still_inside_deadzone` 改为「死区内逐位相同、死区外连续变化」。
- 验证：paint 测试 18 passed；release 构建 2m45s；exe sha256 前 16 位 `e2b83718140d08da`，PID 11536；预览目录同步，备份 `daozai.exe.prev-before-noquant`。

### 7.0zzv 文字 UV 内缩硬开关 → 连续内缩（2026-11-22 22:07–22:12）
- 用户反馈去掉量化后「还是一样的」：说明抽动不是量化造成的。
- 第二嫌疑：`inset_text_uv` 原先按 `|s-1|>1%` **硬开关**，每次内缩整整 1 texel（每边 0.5 texel）。弹簧回弹时 s 在 1.16↔0.84 间反复穿过阈值，每次穿过字形都突然缩瘦一圈又恢复——正好表现成文字上下抽动。
- 修法：
  - 只在放大侧（`s>1`）触发，缩小不再动 UV（缩小时本来就不会采到相邻 texel）；
  - 内缩量 `amount=((s-1)/0.12).clamp(0,1)` 连续增长，`inset_text_uv(shape, atlas, amount)` 按 amount 缩放每边 0.5 texel；
  - `amount<=1e-3` 直接跳过，避免 `Arc::make_mut` 的无谓深拷贝。
- 新增测试 `text_uv_inset_is_one_sided_and_continuous`：`k=0.9` 时 UV 与 `k=1.0` 完全相同；`k=1.01` 只内缩一点点（相对 <2%），`1.02` 比 `1.01` 更收，单调。
- 验证：paint 测试 19 passed；release 构建 2m59s；exe sha256 前 16 位 `37378ea0fbe42150`，PID 32748；预览目录同步，备份 `daozai.exe.prev-before-uvinset`。

### 7.0zzw 文字缩放与按压弹簧解耦（2026-11-22 22:20–22:26）
- A/B 结论：把「＋ 新增」的 GPU 透镜换成矢量胶囊、文字仍由 `ctl_scale` 驱动 → **照样抖**。
  非液态皮肤不抖是因为 `paint::capsule` 按压缩放只有 5%，落在 6% 死区内文字根本不重排，
  不是有效对照。
- 机制：`TextShape::transform` 只缩顶点不改 UV，位图字形被每帧不同的 k 重采样，
  边缘覆盖率连续变化 → 9pt 小字看起来上下抽。
- 修法：
  - `Card` 新增私有 `text_k: Option<f32>` 与 `pub fn scaled_text(cx,cy,k_shape,k_text)`：
    玻璃按 `k_shape` 缩放，文字按显式 `k_text` 缩放；`scaled()` 仍写 `None`（跟随 s）。
  - `galley_shape` 取值顺序：`Frozen`→1.0；`Some(explicit)`→explicit；`Damped`→死区函数；否则 k。
  - `app.rs` 新增 `ctl_text_scale(ctl)` = `0.82+0.18*appear`（只跟入场，不含按压果冻）。
  - `calendar_ui.rs` 的「＋ 新增」改用 `scaled_text(ctl_scale, ctl_text_scale)`：玻璃照旧弹 14%，
    文字按压时 k=1、字形 mesh 逐帧一致。
- 测试 `scaled_text_ignores_press_spring_when_text_k_fixed`：k_shape 1.0/1.14/0.9 且 k_text=1.0
  时行 pos 与全部顶点逐位相同；k_text=1.08 时必须不同。paint 测试 20 passed。
- 部署 exe `41ab30c53797941d`，PID 33296，备份 `daozai.exe.prev-before-statictext`。

### 7.0zzx 入场也不缩放文字（2026-11-22 22:24–22:30）
- 用户实测：解耦按压后「长按不抖了」，但**翻页到日程页**入场时文字仍上下抖。
- 机制：`ctl_appear` 目标 = `reveal`，而 `reveal` 由高度弹簧 `h.pos` 归一化后 smoothstep 得到；
  翻到日程页 `expanded_height()` 从 PH 变 CALENDAR_H，高度弹簧换目标并回振 → `appear` 跟着回振
  → 文字 k 每帧变化重采样 → 入场也抖。按压与入场是同一个病根，只是激励源不同。
- 修法：`＋ 新增` 的 `scaled_text(k_shape, 1.0)` —— 文字恒按 1.0 画，玻璃照旧吃 `ctl_scale`
  （按压 14% + 入场 spring）。字形 mesh 逐帧完全一致，物理上不会再抖。
- 相应删掉 `App::ctl_text_scale`（已无调用点，避免 dead_code）。
- 部署 exe `6e78b71047f52a61`，PID 21796，备份 `daozai.exe.prev-before-frozentry`。

### 7.0zzy 补齐 eframe 0.35 自研事件循环缺口：截图动作回灌 + 精确重绘（2026-11-23 20:18–20:40，提交 `e34f70f`）
- **来源与接手状态**：用户给 `C:\Users\cay\WorkBuddy AI\2026-11-23-17-18-54\HANDOFF-交接给新Agent.md` 要求继续。接手时 `main` HEAD=`a3b37c4`（tag `v1.2.4` 已推、release 已发），工作树只有 `winit_runner.rs` 的**设置页缩放冻结修复**（上一轮未提交、未实机验证）。本轮把该修复与下面两项一起提交为 `e34f70f`，**本地提交，未 push**。
- **① ActionRequested 回灌（桌面 HANDOFF §4①，`d3d11/runtime.rs`）**：`process_viewport_commands` 只把 `Screenshot/Cut/Copy/Paste` 写进 `actions`，原代码从不读取。照 eframe 0.35 `glow_integration.rs:715-747` 补消费端：主窗口 `render()` 和 `PlainWindow::paint_output()/render()` 都在 **Present 之前** drain；Screenshot 用 staging texture `CopyResource` 读回，BGRA 自下而上 → 顶原点 RGBA 预乘 `ColorImage`，注入下一帧 `RawInput.events`；Paste 读系统剪贴板并规范化 CRLF。新增单测锁行翻转/通道序/RowPitch padding。
- **②③ repaint 调度（`winit_runner.rs`）**：user event 从 `()` 改为 `Wakeup::{Repaint{viewport_id,when,cumulative_pass_nr}, LayoutDirty}`，照 eframe `run.rs:299-332` 用 `ctx.cumulative_pass_nr_for(id)` 丢弃过期请求。`about_to_wait` 只在 `next_repaint` 到期时画，无请求时 `WaitUntil(now+60s)` 长睡；旧固定 16ms 心跳删除 → 空闲不再 60fps 空转，app 的隐藏 1fps / 静止 15fps 请求终于生效。
- **④ 主窗口 resize（`winit_runner.rs`）**：`window_event` 不再丢弃 `repaint`；`Resized`/`ScaleFactorChanged` 强制即时重绘。子窗口输入 repaint 同时请求子窗口 Redraw 与父窗口绘制（immediate viewport UI 只在父窗口 `draw_main` 里跑）。
- **缩放冻结修复保留**：子窗口 resize 置 `layout_dirty` → 发 `Wakeup::LayoutDirty` → 在 `user_event` 里用 `with_event_loop_context` 直接补跑父窗口（Windows 模态 sizing 循环里 `about_to_wait` 会停摆，不能只约下一帧）；`draw_main` 末尾清标记。
- **验证**：`cargo check --locked --lib --tests --bins` 通过；全量 `cargo test --locked --release --lib -- --test-threads=1`（隔离 `DAOZAI_DATA_DIR`）= **1402 passed / 0 failed / 131 ignored**；主程序 bin 10 passed（含新增 2 项单测：`readback_tests::screenshot_readback_flips_rows_and_swaps_bgra_to_rgba`、`root_scale_tests::repaint_scheduling_keeps_the_earliest_deadline`）；release exe sha `b7c92525f185de5f`（`dz-phone/target/release/daozai.exe`）。换行符已核对：改动文件仍为纯 LF。
- **状态与待办**：**未打包、未部署、未实机验证**——需用户拖设置页边框确认缩放跟手、观察空闲 CPU 是否下降；兼容渲染路径当前 `native.wants_frames()` 恒 false，截图回灌属机制补齐，暂无实机触发点。剩余 eframe 缺口见桌面 HANDOFF §4⑤⑥⑦：设备丢失/suspended 恢复、子窗口位置持久化（`process::exit` 不跑 Drop）、viewport info 缺 `monitor_size/outer_rect`；低优先级 `device_event`/accesskit/首帧 viewport 命令未做。工作日志在 WorkBuddy 工作区 `.workbuddy/memory/2026-11-23.md`（**不在项目仓库**）。

### 7.0zzz 补齐 §4⑤⑥⑦：设备恢复 / 位置持久化 / 子窗口几何（2026-11-23 20:43–21:10，提交 `38ee776`）
- **⑤ 设备丢失 / suspended 恢复**：`d3d11/runtime.rs` 新增 `new_with_ctx()`，重建 D3D 栈时复用原 egui Context；`FrameStats.device_lost` 由 Present 的 `DXGI_ERROR_DEVICE_REMOVED/RESET` 触发。`winit_runner.rs` 的 `suspended` 只丢 window/runtime/viewport 缓存，**保留 Shell 与 Context**；`resumed` 重建 D3D 栈、重绑 `native::Window`（subclass/hook/拖放按 HWND 注册）、重置 `placed`，并用 `device_lost_pending` 在事件方法末尾（避开 `&Window` 与 `&mut self` 借用冲突）重建；失败按 500ms 退避。
- **⑥ 设置窗口位置持久化**：`settings_window.rs` 加 `last_persist`，拖动中位置变化最多每秒原子写一次，防崩溃/强杀丢位置；关闭/退出仍立即落盘（main.rs 两处 `process::exit` 已调 `save_window_before_exit()`）。坑：`open()` 每帧调用，节流起点只能在首次打开时重置，否则自动保存永不触发。新增单测 `moving_settings_window_autosaves_after_a_second`。
- **⑦ ViewportInfo 字段**：新增 `child_viewport_geometry()`，每帧按子窗口原生 DPI 填 `outer_rect` / `monitor_size` / `title`；最小化返回 None。`settings_window::restore` 不再因 `outer_rect=None` 退化为硬编码边框估算。新增单测 `child_viewport_info_gets_outer_rect_and_monitor_size`。
- **验证**：全量 `cargo test --locked --release --lib -- --test-threads=1` = **1403 passed / 0 failed / 131 ignored**；bin **11 passed / 0 failed**；`cargo check --locked --lib --tests --bins` 通过；release exe sha `a5970d2bcdf538f7`。三份改动文件仍为纯 LF（已与 HEAD 对比 CRLF/LF 计数）。
- **状态**：提交 `38ee776`（本地，**未 push**）；未打包、未部署。**⑤ 的设备丢失路径只做了代码级与单测验证，需实机显卡重置 / 远程桌面断开复现确认**；⑥⑦ 亦需跨屏/高 DPI 实机确认。低优先级 `device_event` / accesskit / 首帧 viewport 命令仍未做。工作日志路径同上（不在项目仓库）。

### 7.0zza 本地重打 1.2.4 安装包（2026-11-23 21:15–21:16）
- 用 `dz-phone/scripts/package.ps1` 从当前 HEAD `c2dd732` 重打（版本沿用 1.2.4，用户早前明确要求）。首次运行被 2 分钟默认超时中断在 `Building 321/322`，**不是脚本/代码错误**；加大超时重跑成功。
- 产物（`dz-phone/dist/`，gitignored）：`Daozai-Setup-v1.2.4-x64.exe` 8,884,310 B，sha `8c660650a94b0cd6e`（完整值见 `SHA256SUMS.txt`）；`Daozai-v1.2.4-windows-x64.zip` 11,033,907 B，sha `4b85385d15c3a0eb`；包内 exe sha `c09afb55ec86b584`。安装包版本信息 `FileVersion=1.2.4`。
- **重要**：这是**本地新包**，与 GitHub release v1.2.4 上已发布的旧包（setup `74879703097f5a36`、ZIP `21bbb3943b64a2df`）**不是同一份**，勿混用；本轮未上传 release、未部署。下次打包前须先删 `dz-phone/dist/Daozai-v1.2.4-windows-x64` 暂存目录，否则脚本报 "Package directory already exists"。

### 7.0zzb 1.2.5 发布准备暂停 + WinRT 短信验证码修复（2026-11-23 21:20–21:42）
- 用户要求打包上传 1.2.5、上传前检查文案；随后实测发现**短信验证码按「手机连接」普通通知显示、没有验证码卡片、也不自动复制**，明确要求先不发布。
- 根因：WinRT 监听器直连 `receive_notification_from`（`main.rs:497`），该路径把正文按普通通知 `otp::extract → mail_privacy::mask` 打码，卡片取不到明文；`notify_phone` 里的 `auto_copy_otp → Cmd::CopyOtp` 从未执行。用户设置 `auto_copy_otp=1` / `phone_alerts=1`，非开关问题。
- 修复：`receive_notification_from` 对 `phone_channel(&id) && unread.is_none()` 改走 `notify_phone`；新增单测 `winrt_phone_sms_otp_uses_card_and_auto_copy`（标题「手机 · 短信」、正文保留明码、`CopyOtp("834921")`）。提交 `8118370`，CHANGELOG `60a5e20`。
- 版本升级：Cargo.toml/Cargo.lock 1.2.4→1.2.5（`7f5a467`，**Cargo.lock 只同步包版本号，未加依赖**）。全量 lib **1404 passed / 0 failed / 131 ignored**、bin **11 passed**。
- **【21:49 已发布】** 用户确认文案后已推送 `main` 与 tag `v1.2.5`，并发布 https://github.com/cayww/daozai-custom/releases/tag/v1.2.5 ，含 2 个资产：`Daozai-Setup-v1.2.5-x64.exe` 8,888,312 B / sha `53465b8fe84b98`、`Daozai-v1.2.5-windows-x64.zip` 11,038,401 B / sha `68d8ea9d3ff552a`；包内 exe sha `5de9f2ec556f355d`。API 复核资产数量/大小/sha 一致；文案草稿留档 WorkBuddy 工作区 `release-notes-v1.2.5.md`。**仍未部署到本机**。

### 7.0zzc egui 0.37 研究分支（2026-11-23 23:20–00:10，分支 `research/egui-main-56f3a0e`）
- 用户要求单独分支升级到 emilk/egui commit `56f3a0e09308c96c117d1532bbede2f15d47eeab`（workspace 0.37.0，edition 2024，rust-version 1.95）。`eframe/egui/egui_extras/egui-winit` 全部改为 git rev；`log` 因 eframe 要求 `^0.4.35` 从 0.4.34 升到 0.4.35。
- 主要 API 迁移：`ColorImage.pixels/size` 字段 → `pixels()/pixels_mut()/size()`；`TexturesDelta.set` 改为 `HashMap<TextureId, SmallVec<[ImageDelta;1]>>` 且 Drop 前有未应用 delta 会 debug_assert，渲染端改为借用遍历 + clone ImageData；`DroppedFile` 变 trait（测试加 `TestDroppedFile`）；`Event::Screenshot` 改为 `ScreenshotCallback`，兼容渲染用 `Shell::compat_shot` 跨帧接结果；`PaintWhileHidden` no-op；`schedule_repaint` 不再补 `predicted_dt`（0.37 已不预减一帧）。
- `Primitive::Clear` 已实现：D3D11 后端新增无混合 blend state，把 Clear rect 与 clip_rect 相交后生成透明黑四边形，按原始 op 顺序与 Mesh 交替绘制并恢复普通 blend state（不再忽略 Shape::Clear）。
- `examples/lyrics_switch_memory.rs` 的 `font_image_size()` 已改为 `font_image_resolution()`；`cargo check --examples` 通过。
- 验证：`cargo check --locked --lib --tests --bins --examples` 通过；播放图标连续性测试把画布从 64pt 扩到 96pt，避开 0.37 严格 clip_rect 边界采样。`compact_font_atlas_preserves_settings_pixels_at_multiple_scales` 曾偶发一次图集上限 2048 / scale2 大面积差异，复测稳定通过；测试加了 `DAOZAI_FONT_DEBUG_DIR` 可选导出 before/after PNG 以便复现。最终 release lib **1404 passed / 0 failed / 131 ignored**，release bin **11 passed / 0 failed**。debug 测试仍有旧 helper 未 clear TexturesDelta 的 debug_assert 和布局差异失败，属测试适配未完成，不是发行构建问题。
- 版本升至 **1.2.6**（仅本分支；与 main 已发布 1.2.5 区分）。状态：**研究分支，未合并 main、未发布、未部署**。
- **本地包已打（2026-11-24）**：`dz-phone/dist/Daozai-Setup-v1.2.6-x64.exe` 8,645,885 B / sha `0589de891dac74b9`；`dz-phone/dist/Daozai-v1.2.6-windows-x64.zip` 10,851,948 B / sha `542dccbacdcc75d2`；包内 exe sha `1473042ce598bad6`。完整 sha 见 `dz-phone/dist/SHA256SUMS.txt`。未上传 release、未部署。
- 后续按用户要求直接启用 egui 0.37 的系统字体回退：`Cargo.toml` 增加同 rev 的 `egui_system_font_provider`，`theme::load_fonts` 用 `ctx.set_font_providers(vec![Arc::new(SystemFontProvider::new())])` 安装；自定义 runner 不走 eframe native 集成，必须显式安装。该改动只做了 `cargo check --lib --tests --bins --examples`，按用户要求未跑全量测试；未重新打包。
- 按用户要求继续启用其他“比现状好”的可选项：`eframe` 增加 `system_fonts` feature（examples/eframe 路径也一致），`egui` 增加 `rayon`（并行 tessellation）与 `monochrome_emoji_fonts`（emoji 兜底）；D3D11 sampler 从 `MIN_MAG_MIP_LINEAR` 改为 `MIN_MAG_LINEAR_MIP_POINT`（字体图集无有用 mip 链，避免采样到更糊的 mip 层）。仅 `cargo check --lib --tests --bins --examples` 通过，未跑全量测试、未重新打包。
- 结论：0.35 → 56f3a0e 的提交里没有专门修“文字毛刺/抗锯齿”的提交；系统字体提供者只改善缺字/CJK 回退，毛刺是否改善需实机观察。
- **1.2.7 本地包（2026-11-24，重打包）**：包含系统字体 provider / `system_fonts`、`rayon`、`monochrome_emoji_fonts`、无 mip 采样、过时 API 清理、设置页阴影修复与 messenger 测试修复。`Daozai-Setup-v1.2.7-x64.exe` 9,185,938 B / sha `6d186749d1c07d5a3`；`Daozai-v1.2.7-windows-x64.zip` 11,508,101 B / sha `04d18ebd0e8aca9e3`；包内 exe sha `a8225fe067f505e8`。提交 `1b3633e`。未上传 release、未部署。
- 过时的 egui 0.35 API 已清理：`Id::new` → `Id::unique`、`TextEdit::vertical_align(Align)` → `align(Align2::LEFT_CENTER)`、`Ui::id` → `scope_id`、`TextureHandle/ImageData/ColorImage::size` → `resolution`、`size_vec2` → `resolution_vec2`。`ImeEvent::Enabled/Disabled` 在 0.37 仅 deprecated 但仍保留语义，测试与 app 处理处加局部 `#[allow(deprecated)]`，避免改坏输入法模拟。`cargo check --lib --tests --bins --examples` 的 deprecated warning 从 121 降到 0。
- 设置页窗口阴影闪动：根因是每帧都发 `ViewportCommand::SetTheme`，DWM 反复重描窗口阴影。新增 `App::settings_window_theme_sent`，只在深浅主题真正变化时发送；相关测试改为断言主题未变时不重复 SetTheme。
- 验证：release bin 11 passed；release lib 1403 passed / 1 failed / 131 ignored，唯一失败 `messenger::windows::tests::exited_messenger_accounts_are_removed_individually`，与真实企业微信/微信进程列表有关，和本次改动无关。
- D3D11 选卡策略已完善：`main.rs` 启动时把 `Settings::load().liquid_prefer_dgpu` 显式传给 `winit_runner::run(prefer_dgpu)`，不再写 `DAOZAI_LIQUID_PREFER_DGPU` 环境变量；`D3d11Runtime::create_device_swap` 按快照选择适配器。关闭时保持原 `EnumAdapters(0)`；开启时枚举前 16 个 `IDXGIAdapter1`，跳过软件适配器与 Microsoft Basic Render Driver，按「独立显存更大 + 独显优先（NVIDIA/AMD，Intel 独立显存 ≥1GiB 视为独显）」排序；找不到独显则回退默认适配器。设备重建沿用同一快照。设置页「优先独立显卡」文案已同步改为该行为说明。
- 验证：`cargo check --lib --tests --bins --examples` 通过；release lib **1405 passed / 0 failed / 131 ignored**，bin **11 passed / 0 failed**。
- 下一步若继续：①清掉所有测试 helper 的 TexturesDelta；②实机验证 D3D11 Clear、兼容渲染截图、系统字体回退、设置页阴影与混合显卡选卡。

### 7.0zzc 剪贴板历史支持图片（2026-11-24 14:30–15:05，提交 `eb0b215`，本地未 push）

- **缘起**：用户升级到 egui 0.37 后看到 `Context::copy_image` / `Event::PasteImage` 相关提交（#5533/#8472/#8715），问能否接进岛仔剪贴板。核查发现三条提交**早已在锁定的 `56f3a0e` (0.37.0) 里**，但走 egui-winit 的 `clipboard` feature（arboard），与岛仔独立的 `AddClipboardFormatListener` + `clipboard-win` 链路是两回事；`copy_image` 是「应用主动写」方向，且打不上 `HISTORY_COPY_FORMAT` 自我记录标记。用户确认目标是「历史支持图片」并授权动 Cargo.lock，但评估后**决定不开 feature、不动 lock**，纯 Win32 实现（见下）。
- **实现**：
  - `clipboard.rs`：`Clip` 加 `image: Option<ImageMeta>`（文件名+宽高，纯 basename）；图片落盘 `%APPDATA%\Daozai\clip-images\*.png`，JSON 只存元数据；上限 4000×4000 / 12MB RGBA；`push_image` 与文字共用 24h/固定/MAX_RECENT 规则，清理时删对应 PNG。
  - Windows 监听线程：`read_clipboard()` 一次 OpenClipboard 内先读 `CF_UNICODETEXT`，无文字才读 `CF_DIB`/`CF_DIBV5`（只认 BI_RGB 的 24/32bpp），BGRA→RGBA、`biHeight` 负值处理 top-down；沿用 `excluded()` 敏感过滤与 `HISTORY_COPY_FORMAT` 自我记录标记。
  - 再次复制：`copy_history_image()` 从 PNG 还原为 CF_DIB（32bpp BGRA 自下而上）+ 打 `HISTORY_COPY_FORMAT`，避免截图→复制→又记一条。
  - `pages.rs`：剪贴板分栏给图片条目画 16×16 缩略图（`App.clip_thumbs` 按文件名缓存纹理），文字区从 x=68 右移到 88；新增 `App::copy_clip()` 统一文字/图片的复制入口；空态文案改为「文字和图片」。
  - 兼容：旧 `clipboard.json` 无 `image` 字段照常读取；JSON 里带 `..` / 斜杠的文件名被拒（路径穿越防护）。
- **验证**：`cargo check --locked --lib --tests` 通过；全量 `cargo test --locked --release --lib -- --test-threads=1`（隔离 `DAOZAI_DATA_DIR`）= **1409 passed / 0 failed / 131 ignored**（新增 4 个 clipboard 单测 + 1 个 UI 测试）；release exe sha `3f62fe69a679ecf5`（`dz-phone/target/release/daozai.exe`）。改动 4 文件：`app.rs +3`、`clipboard.rs +294/-29`、`pages.rs +45/-17`、`ui_tests.rs +23`；换行符与 HEAD 一致（前三 LF、`ui_tests.rs` CRLF）。**Cargo.toml / Cargo.lock 未改**。
- **状态**：提交 `eb0b215` 在 main 本地，**未 push、未打包、未部署、未实机验证**——部署时用户正在运行预览版（PID 16700，`%LOCALAPPDATA%\Programs\Daozai\daozai.exe`），为避免打断使用未停进程。**待实机验证**：真正复制一张截图 → 置物架→剪贴板分栏出现缩略图 → 点条目把图粘到画图/微信能还原。工作日志在 `C:\Users\cay\WorkBuddy\2026-11-24-14-23-45\.workbuddy\memory\2026-11-24.md`（**不在项目仓库**）。

### 7.0zzf 剪贴板图片「不生效」真因 + hover 预览（2026-11-24 15:10–15:45，提交 `52b7c70`，本地未 push）

- **现象**：用户跑 `dz-phone/target/release/daozai.exe` 后复制图片，历史里没出现。诊断日志（`DAOZAI_CLIPBOARD_DEBUG=1`，写 `diagnostics/clipboard.log`，只记格式/尺寸不记内容）显示：`formats text=false dib=true dibv5=true bitmap=true` → `dib=OK 1569x909` → `content=Image` → 落盘成功。说明用户当时跑的是修复前的 exe；真因是上一版只认 `BI_RGB` 压缩的 DIB，而截图工具给的是 `BI_BITFIELDS`（compression=3），被 `compression != 0` 直接拒绝。
- **修复**（`clipboard.rs`）：`read_dib_locked` 支持 `BI_BITFIELDS=3`（跳过 12B 掩码）/`BI_ALPHABITFIELDS=6`（16B）；其他压缩（RLE/JPEG/PNG）仍跳过并写诊断。新增 `read_bitmap_locked`：只提供 `CF_BITMAP`（HBITMAP）时用 `GetObjectW`+`GetDIBits` 取 32bpp BGRA 兜底，全透明 alpha 当不透明。诊断日志改为环境变量 `DAOZAI_CLIPBOARD_DEBUG=1` 才写，默认零开销。
- **hover 预览**（`pages.rs`）：剪贴板条目从 `on_hover_text`（400 字截断）改为 `on_hover_ui`——图片条目显示等比放大预览（最大 340×240，用已缓存的 `clip_thumbs` 纹理）+ 尺寸/点击提示；文字条目用 `ScrollArea(max_height=200)` 显示全文可滚动，不再截断。`TextureHandle::size_vec2` 改 `resolution_vec2`（0.37 已 rename）。
- **验证**：grep 编译 `cargo check --locked --lib` 通过；剪贴板相关 14 个测试全过（含之前的 1409 全量基线，本轮未再跑全量）；release exe `fb7693cade1388b5`（诊断版）/ 最终源码待重建。构建踩坑：用户正在跑的 exe 占用导致 `failed to remove file ... 拒绝访问 (os error 5)`，停掉 PID 后重建成功。
- **状态**：提交 `52b7c70` 本地 main，**未 push、未打包、未部署**。已启动 `fb7693cade1388b5`（PID 33504）供用户实机验证 hover 预览与滚动。**待确认**：① 图片 hover 显示放大图；② 长文字 hover 可滚动；③ 复制图片进历史（已由日志实证）。

### 7.0zzg 剪贴板 tooltip 滚动不再被列表翻页拦截（2026-11-24 15:47–16:00，提交 `bad5555`，本地未 push）

- **现象**：hover 剪贴板条目弹出的长文 ScrollArea 滚动时，滚轮被岛仔自己的列表翻页/翻页逻辑抢走，tooltip 内容滚不动。
- **根因**：`App::frame` 的自定义滚轮处理（2737 行起）在 UI 绘制**之前**执行，直接 `raw_scroll_y(&ctx)` 读原始 `MouseWheel` 事件并 `retain` 清空，完全不管 tooltip；`pages.rs` 的 `on_hover_ui` 在同一次 UI pass 的更晚阶段才注册 ScrollArea，事件已被消费。
- **修复**：`pages.rs` 文字 tooltip 的 ScrollArea 在 `ui.response().contains_pointer()` 为真（鼠标真的移入滚动区）时，把当前毫秒时间戳写进 `ctx.data` 的 `CLIP_TOOLTIP_KEY`；`app.rs` 的滚轮分支先查该标记（`tooltip_open_last_frame`，120ms 内有效），命中则跳过翻页、不清空 `MouseWheel` 事件，让 egui 的 ScrollArea 正常消费。鼠标还在列表上未移入 tooltip 时标记不写，滚轮照常翻列表——避免「tooltip 刚弹出就滚不动」的死角。
- **坑**：egui 0.37 没有公开的全局 `is_tooltip_open()` / `prev_pass_state()`（后者是 pub(crate)），`Context::frame_nr()` 也不存在；只能用 `ctx.data` 自己传状态 + `chrono::Local::now().timestamp_millis()` 作时间基准。`ScrollArea::show()` 的 `inner` 是闭包的返回值（`()`），不能取 `.response.hovered()`，要在闭包内用 `ui.response()`。
- **验证**：`cargo check --locked --lib` 通过；剪贴板相关 14 个测试全过；release exe sha `eea7dd5fe7194d08`，已启动 PID 27716 供用户实机验证。
- **状态**：提交 `bad5555` 本地 main，**未 push、未打包、未部署**。待用户确认：hover 长文 tooltip 内滚动只滚文字、不触发列表翻页；鼠标在列表上时滚轮仍翻列表。

### 7.0zzh 撤销 tooltip 滚动方案，长文字改省略号（2026-11-24 16:01–16:09，提交 `90145a2`，本地未 push）

- **用户决定**：`bad5555` 的 tooltip 滚动隔离实测仍不能可靠滚动，明确要求撤销——「保证 hover 弹窗完整就行，多出的内容省略号」。
- **回退**：`app.rs` 完全恢复到 `bad5555^`（diff 为空）：删 `CLIP_TOOLTIP_KEY` / `tooltip_open_last_frame` 及滚轮分支的让行逻辑；`pages.rs` 移除 ScrollArea 与滚轮时间戳标记，文字改 240 字 + `…` 截断。图片放大预览保留（无滚动需求，不受影响）。
- **结论**：岛仔自定义滚轮翻页（`App::frame`，UI 绘制前执行，直接读并清空 `MouseWheel`）与 egui ScrollArea 的事件竞争难彻底隔离；放弃滚动，以弹窗完整为优先。
- **验证**：`cargo check --locked --lib` 通过；剪贴板相关 14 个测试全过；release exe `cf585159c13d4ead`，已启动 PID 16316。
- **状态**：提交 `90145a2` 本地 main，**未 push、未打包、未部署**。此前 `eb0b215`/`52b7c70`/`bad5555`/`90145a2` 等均在本地。

### 7.0zzi 缩小剪贴板 tooltip 高度（2026-11-24 16:10–16:15，提交 `c570cfa`，本地未 push）

- **现象**：用户实测 hover tooltip 底部阴影被裁。根因：画布 `WIN_H=270`，条目在 y≈44–124，tooltip 向下展开时 340×240 图片 + 两行文字（约 276 高）必然超出窗口底部。
- **调整**：图片预览压到 220×120、说明合并一行（`{尺寸} · 点击复制`）；文字条目截断 240→180 字。tooltip 完整显示、阴影不再被裁。
- **验证**：`cargo check --locked --lib` 通过；release exe `9612bdb06f8f4ff0`，已启动 PID 22704 供实机验证。
- **状态**：提交 `c570cfa` 本地 main，**未 push、未打包、未部署**。

### 7.0zzj 重打 1.3.0 安装包（含剪贴板图片功能 + 用户微调 tooltip）（2026-11-24 16:18–16:22）

- 用户自己把图片预览尺寸从 220×120 改成 180×100（`pages.rs` 一行），提交 `50f9a5a`；随后要求重新打包。
- 打包前：停掉运行中的 daozai（PID 22704）；删 `dz-phone/dist/` 里旧 v1.3.0 产物（staging 目录 + setup + zip + SHA256SUMS.txt）——脚本明确要求 staging 目录不存在，否则报 "Package directory already exists"。
- 打包：`dz-phone/scripts/package.ps1`（默认从当前工作区全量构建 + check-runtime + NSIS + ZIP + sha），约 2m59s 完成，运行库检查通过（无外部 C/C++ 运行库依赖）。
- **产物**（`dz-phone/dist/`，gitignored）：
  - `Daozai-Setup-v1.3.0-x64.exe` 9,222,840 B，sha `09b3e3e9bfa1ef48`（完整 `09b3e3e9bfa1ef48ab4e3e67f8a37652e7fe8c15756908f8ac455c68cfa2b710`）
  - `Daozai-v1.3.0-windows-x64.zip` 11,556,201 B，sha `ab2a50b919b165a1`（完整 `ab2a50b919b165a1e7a38ab9bb286f27bdb06b337b4406eefa07eb6df0441cab`）
  - 包内 exe sha `e9c7e724282e363c`，FileVersion=1.3.0（安装包本身也是 1.3.0）。
- **注意**：这是**本地重打的同版本包**，与 10:53 那次 v1.3.0（旧 exe）不同，sha 已变；未做静默安装验证、未部署、未发 GitHub release。旧包 sha 见 git 历史/旧 SHA256SUMS。
- **状态**：本地 main 领先 origin 11 个提交，未 push；工作树干净。未部署到 `D:\software\Daozai`（避免打断用户）；要装请用上面的新包并核对 sha。

### 7.0zzk 收起态歌词居中 + 关宠物收空位（2026-11-24 17:20–17:30，提交 `652e9f2`，本地未 push）

- **现象**：用户实测收起态短歌词（如「间奏」）非常偏右；关宠物后歌词前面仍有大空隙。
- **根因一（空位）**：`lyric_lane()` 无条件按宠物占位算 `text_left`（`pet_center+LANE_PET_HALF+LANE_TEXT_GAP`），关宠物时 48px 贴图位照留。
- **根因二（偏右）**：上一版居中用 `local_cx - text_w/2`，但 `local_cx` 是相对 **base 原点**的坐标，而 `text_left/lane_right` 是相对 **岛体左缘 left** 的偏移——两套坐标系混用，把歌词整体右移约 `left`（≈width/2）距离，短歌词尤其明显。
- **修复**：`lyric_lane_geometry(rings, pet_on)` 关宠物时 `text_left=art_right+LANE_TEXT_GAP`（封面后缘直接接歌词，收回约 52px）；`lyric_centered_left(center_in_lane, text_left, lane_right, text_w)` 以 `width/2`（岛体中心相对 left 的偏移）为基准居中，夹在封面/宠物与播放条之间，长歌词仍横向滚动。两个纯函数 + 单测（`lyric_lane_leaves_no_gap_when_pet_is_off` / `lyric_text_centers_on_island_midline`，含「间奏」短文本用例）。`cfg!(test)` 仍返回 None 沿用官方坐标。
- **验证**：`cargo check --locked --lib --tests` 通过；坐标系修复前全量 1409 passed / 0 failed；**坐标系修复后按用户要求未跑全量测试**（用户说「先不测试」），仅编译通过。release exe `57fe17678d1e56f5`，已启动 PID 24016 供用户实机验证。
- **注意**：本次 `pages.rs` 也被卷入一行——用户自己把 tooltip 图片预览从 180×100 又改到 160×80（工作树未提交改动），一并提交。
- **状态**：提交 `652e9f2` 本地 main 未 push（origin/main 已在 `7971165`）；未打包、未部署。待用户确认短歌词居中与关宠物空位回收效果。

### 7.0zzl 部署歌词居中修复到安装版（2026-11-24 17:38–17:42）

- **关键发现**：用户连续反馈「还是没对齐」，根因是用户看的是 `C:\Users\cay\AppData\Local\Programs\Daozai\daozai.exe`（16:21 打的包，sha `e9c7e724`），而歌词修复在 target 新版（sha `57fe17678d1e56f5`）里——**用户一直在跑旧 exe**。之前每次我启动 target 版后，用户又启动安装版，看到的仍是旧代码。
- **部署**（用户明确同意）：停 PID 8704 → 旧 exe 备份到安装目录 `daozai.exe.bak-20261124`（sha `e9c7e724`）→ 复制 target 新版到安装目录 → 启动 PID 30812（sha `57fe1767`）。**可回滚**：把 .bak 改回 daozai.exe 即可。
- **状态**：安装目录现在是含歌词居中修复的新版（也含全部剪贴板图片功能）；未重新打包 installer/ZIP（dist 里 v1.3.0 包仍是 16:21 的旧 exe `e9c7e724`，**不含歌词修复**）。待用户实机确认短歌词居中与关宠物空位回收。
- **教训**：用户反馈「没生效」时，第一步必须先核对**用户实际运行的 exe 路径与 sha**，别假设他在跑刚构建的版本。

### 7.0zzn 歌词对齐可用区中点，消除右侧大空隙（2026-11-24 17:42–17:48，提交 `463c2b9`，本地未 push）

- **现象**：部署 §7.0zzl 后用户反馈「右边空隙又大了」。截图量化：歌词中心已对齐岛体中心，但歌词右缘到播放条约 100px、左缘到封面仅约 30px——左侧被图标占位，视觉不平衡。
- **修复**：歌词中心改为对齐「左侧图标右缘 `text_left` → 播放条左缘 `content_width-29`」这段可用区的中点；右边界同步从 `LANE_RIGHT_PAD=48` 放宽到 `content_width-29`，否则中点被夹回去。
- **验证**：`cargo check --locked --lib --tests` 通过；release exe `17e8c0e55a768033`，已备份安装版到 `daozai.exe.bak-lyric-mid` 并部署、启动 PID 23100。未跑全量测试（用户此轮要求先不测）。
- **状态**：本地 main 未 push；dist 的 v1.3.0 包仍不含歌词系列修复，需重打。待用户确认左右空隙平衡效果。

### 7.0zzp 收起态岛歌词 KTV 逐字高亮（2026-11-24 17:59–18:09，提交 `24d7ae8`，本地未 push）

- **需求**：用户要 KTV 效果——唱到哪个字哪个字变深色，没唱的浅色，且要跟随主题色（液态玻璃深浅材质）。范围：**只收起态岛上歌词**；时间精度：**按行时长均分**（无逐字时间戳词源）。
- **实现**：
  - `lyric_sung_chars(count, progress)` 纯函数：`floor(progress*count)`，0→0、1→全部，夹异常值。
  - `karaoke_galley(ctx,text,size,sung,wrap_width,single_line)`：`LayoutJob::append` 两段——已唱 `Color32::WHITE`（实心）、未唱 `from_white_alpha(110)`（半透白）。同 font/wrap/halign 保证字形位置与原 galley 一致，所以居中/滚动/裁剪几何完全不动。
  - App 新增 `island_lyric_karaoke: Option<(String,f32)>`，`update_island_lyric_layout` 里只对同步歌词分句（非 title、非测试构建）保存 text+progress；`shared_header` 绘制时生成双色 galley 替代纯色。
  - 歌名/前奏/间奏不逐字高亮；`cfg!(test)` 不走 KTV，UI 布局断言不变。
- **配色说明**：液态玻璃深浅材质靠 shader 画背景，egui 文字一直是白字体系；「跟随主题色」在此体现为白字的深浅两档（alpha 110 vs 255），任何背景都可读。若用户要的是「亮白→主题蓝」之类需再调。
- **验证**：`cargo check --locked --lib --tests` 通过；加 1 个单测 `karaoke_sung_chars_follow_line_progress`。**未跑全量测试**（用户本轮未要求，且每轮构建 3 分钟）。release exe `775279ee3246bf93`，已备份安装版到 `daozai.exe.bak-pre-karaoke` 并部署、启动 PID 31728。
- **状态**：提交 `24d7ae8` 本地 main 未 push；dist 的 v1.3.0 包仍不含 KTV 及歌词系列修复，需重打。待用户实机确认逐字高亮节奏与配色。

### 7.0zzq 撤销 KTV 逐字高亮（2026-11-24 18:23–18:27，revert `f970754`，本地未 push）

- 用户实测后要求撤销，不需要逐字高亮。`git revert 24d7ae8` 精确回滚功能代码（2 insertions/73 deletions），`lyric_sung_chars`/`karaoke_galley`/`island_lyric_karaoke` 全部清除；文档提交 `a171ce0` 保留作为历史。
- 编译 `cargo check --locked --lib --tests` 通过；release exe `9c5268385450535b`，已部署安装目录、启动 PID 35980（未备份——回滚后就是 KTV 前的 `463c2b9` 状态，旧备份 `daozai.exe.bak-pre-karaoke` 仍在）。
- 歌词居中系列修复（`463c2b9` 可用区居中、关宠物收空位）保留。
- **状态**：本地 main 未 push；dist 的 v1.3.0 包仍旧，需重打。

### 7.0zzr 输入语言/大小写提示状态修复（2026-11-24 18:23–18:56，提交 `7ce6bee`，本地未 push）

- **用户反馈**：切换中英文和大小写的逻辑有问题，岛上显示的切换状态可能对不上；快速切换时响应停留在上次状态。
- **根因 1（大小写对不上）**：`system.rs` 后台采样线程用 `GetKeyState(VK_CAPITAL)`——该 API 读**调用线程消息队列**的键盘状态，后台线程从不处理键盘消息，故滞后/恒旧。改用 `GetAsyncKeyState`（全局异步状态）。
- **根因 2（快速切换滞后）**：`main.rs` 用实时 `keys.allows()` 判断授权、发出提示后 `clear()`。快速连按（中→英→中）时第二次按键覆盖第一次授权，第一条 observation 发出提示后 `clear()` 又误清第二次授权，第二条状态变化被吞 → 停在「英语」。
- **修复**：`Observation` 新增 `input_switch` 字段，system 采样线程在 `State::read()` **之前**取 `switch_recent()` 存入（读 IME 耗时可能使窗口过期）；main 用 `observation.input_switch` 作授权，移除实时 `allows()` 与 `clear()`。`SwitchKeys::allows/clear` 保留供测试与 hook 使用。
- **副作用权衡**：不再按 context 校验按键授权，但 `Monitor::observe` 的 context 变化分支仍静默同步切窗口，故切窗口瞬间不弹提示，可接受。
- **验证**：`cargo check --locked --lib --bins` 通过；`input::` 相关 17 passed / 0 failed / 1 ignored（含新增 `each_observation_keeps_its_own_switch_authorization`）。**未跑全量测试**。release exe `5e3c9d5392e6f171`，备份安装版到 `daozai.exe.bak-pre-inputfix` 并部署、启动 PID 22120。
- **状态**：提交 `7ce6bee` 本地 main 未 push；dist 的 v1.3.0 包仍不含此修复。待用户真机验证（需实际按 Shift / Ctrl+Shift / Caps Lock）。

### 7.0zzs 重打 1.3.0 包（含全部修复；回答用户「安装包在哪」）（2026-11-24 19:06–19:11）

- **背景**：用户反馈「试了没区别」，且 dist 里的 v1.3.0 安装包被清空（只剩旧 `SHA256SUMS.txt`，setup/zip 都不在）。查明：之前几轮我一直直接更新**安装目录** `AppData\Local\Programs\Daozai\daozai.exe`，而 dist 安装包自 16:21 起未重打，用户若装旧包自然没修复。
- **确认**：`setup.nsi` 的 `InstallDir="$LOCALAPPDATA\Programs\${APP_ID}"`，即安装目标 = 我一直部署的目录，两者一致。无 Program Files 下的残留安装。
- **重打**：`dz-phone/scripts/package.ps1` 约 3m17s 成功，MSVC 目标正式构建，运行库检查通过。
- **新产物**（`D:\code\daozai-custom\dz-phone\dist\`，19:10）：
  - `Daozai-Setup-v1.3.0-x64.exe` 9,222,554 B，sha `568082cf093a8a52`（完整 `568082cf093a8a52a155481f39d30a629e694a5d00981fe6174ad0f25264ca58`）
  - `Daozai-v1.3.0-windows-x64.zip` 11,556,248 B，sha `112443ac78f296f0`（完整 `112443ac78f296f09a12faded3e202397c14b9fa71db48a04b348b0f7235fd14`）
  - 包内 exe sha `6aee44290bf0ea945`（= `target/x86_64-pc-windows-msvc/release/daozai.exe`）
  - 注意：之前直接部署到安装目录的是 GNU target 构建 sha `5e3c9d53`，与包内 MSVC 构建 `6aee4429` **代码相同、编译器不同**，都含全部修复。
- **状态**：本地 main 未 push；未部署这个新包（安装目录当前是 5e3c9d53 旧部署版，需更新请用新包安装或复制）。用户「没区别」的真因待确认——可能是装的时机在修复前，或进程未重启。

### 7.0zzt 实测定位：Caps Lock 假提示刷屏才是真因（2026-11-24 19:16–19:45，提交 `bff7785`，本地未 push）

- **用户反馈三轮「没改善」后停止盲改，加 `DAOZAI_INPUT_DEBUG=1` 毫秒日志实测**（`input.log` 只记布局/模式/授权元数据，不记输入内容）。
- **实测结论（推翻前两轮推测）**：
  - 中英文切换**本就正常且快**：`KEY Shift按下 → ~100ms 后采样读到 native 变化 → 弹出「英语/中文」`，内容全对。所以 §7.0zzr 的 input_switch 授权改造并未解决用户问题（但逻辑合理，保留）。
  - Caps Lock **每次按键弹两条假提示**：日志 6 次「已开启/已关闭」变化 vs 用户只按 3 次；且按 Shift 时也跟着弹。假提示覆盖掉输入语言提示 → 用户看到「内容不对、很慢」。
- **真因**：§7.0zzr 把 `GetKeyState(VK_CAPITAL)` 改成 `GetAsyncKeyState`。后者低位是「自上次调用以来按过没」（**调用会清标志**），连续采样 1/0 交替产生假 toggle；`GetKeyState` 低位才是锁定状态、调用不清标志。已回退。
- **教训**：Windows 键盘状态 API 语义差异——GetKeyState 的 VK_CAPITAL 低位=锁定状态（toggle），GetAsyncKeyState 低位=自上次调用以来按下过（一次性）。改这个 API 前必须查语义 + 实测，别靠推测。
- **验证**：`cargo check --locked --lib --bins` 通过；input:: 17 passed / 0 failed；release exe `e5fffdb0e2484832`，部署安装目录、启动 PID 29320（未跑全量测试）。诊断日志代码已全部移除。
- **状态**：提交 `bff7785` 本地 main 未 push；dist 的 v1.3.0 包（19:10 打的）**不含此修复**，需重打。待用户真机确认 Caps 不再刷屏。

### 7.0zzu 实测定位：HUD 通知排队才是"状态延迟"真因（2026-11-24 20:02–20:22，提交 `2e331f1`，本地未 push）

- **用户反馈**：状态对了但延迟大——切大写后切回小写，仍显示大写直到消失；中英文也一样。用户建议「复用旧通知刷新状态」（正解）。
- **实测（通知层日志 ALERT/ENQUEUE/REPLACE/QUEUE/SHOW）**：
  - 同类 HUD 之间即时 REPLACE：`键盘输入语言/中文` → `英语` 每 200ms 正确替换。
  - 跨类型排队：`ENQUEUE 键盘输入语言/英语 current_notice=大写锁定/已关闭 -> QUEUE (wait)`，两条输入语言通知都排队，直到 `SHOW from queue` 才显示——旧 HUD 2.5s 过期期间新状态不显示。
- **修复**：`enqueue_alert` 的 `same_event` 加一条 `_ if a.is_hud() && old.is_hud() => true`——所有 HUD 瞬时状态共用通知槽，新状态直接替换当前 notice，不排队。用户「复用旧通知刷新状态」的建议即此。
- **诊断排除**：TSF compartment（`tsf_compartments()` 探针）在后台采样线程恒读 `(0,0)`，社区推荐的 TSF 路线在岛仔架构下不可用，已移除探针函数；IMM32 的 `imm_native` 随 Shift 100ms 内正确翻转，检测没问题。所有临时诊断代码已清除。
- **验证**：`cargo check --locked --lib --bins` 通过；全量 **1412 passed / 0 failed / 131 ignored**；release exe `c67fc2074e3246b0`，部署安装目录、启动 PID 24464。
- **状态**：提交 `2e331f1` 本地 main 未 push；dist 的 v1.3.0 包（19:10）不含此修复，需重打。待用户实机确认快速切换即时刷新。

### 7.0zzv hud_notice 按内容同步，修同类通知不刷新（2026-11-24 20:24–20:36，提交 `e864f8e`，本地未 push）

- **用户反馈**：§7.0zzu 的 HUD 共用通知槽只让「大写↔中英文」跨类型切换实时，但「大写↔小写」「中文↔英文」同类切换通知仍不更新。
- **根因**：`app.rs` 的绘制快照 `hud_notice` 只在 **title 变化**时从 `notice` 同步（原注释「避免每帧双重 clone」）。同类 HUD 切换 title 相同、body/at 不同 → 绘制层永远显示第一次的内容。日志显示 notice 层明明 REPLACE 了，但绘制没跟上，完全吻合。
- **修复**：同步条件改为按 `title + body + at` 比较，任一变化即同步；仍只在变化时 clone，无每帧开销。
- **验证**：`cargo check --locked --lib --tests` 通过；全量 **1412 passed / 0 failed / 131 ignored**；release exe `f70a3cc0c64aab82`，部署安装目录、启动 PID 28608。
- **状态**：本地 main 未 push；dist 的 v1.3.0 包（19:10）不含此修复，需重打。待用户实机确认同类切换即时刷新。
- **教训**：通知「入队/替换」与「绘制快照同步」是两层；只修入队层不够，绘制层的 hud_notice 同步条件（只比 title）是隐藏的第二道闸。

### 7.0zzw 代码审查 3 条建议（2026-11-24 20:45–20:55，提交 `441a89c`，本地未 push）

- 用户对输入/通知链路改动问「有无性能或安全问题」，用 code-review-assistant 做专项审查：无 Blocker、无已确认安全漏洞；1 个 Warning（切换授权放弃 context 校验，用户未要求改，保留）+ 3 条 Suggestion（用户要求全采用）。
- **S1**：`Observation` 去掉 derive PartialEq/Eq 改为手动实现，`eq` 只比 system/input/fullscreen，排除瞬时 `input_switch`——切换窗口 500ms 后标志翻转不再多发 observation + repaint。
- **S2**：`main.rs` 的 `input_keys` 加 RAII 注释 + `#[allow(dead_code)]`：该句柄必须活到 Shell 结束（Drop 卸载 WH_KEYBOARD_LL，钩子维护 SWITCH_AT），此前构建有误导性 dead_code 警告。
- **S3**：`app.rs` 的 `hud_notice` 同步比较从 title+body+at 简化为 title+at——先验证所有 notice 更新都是整条 `Some(Alert)` 替换并刷新 `at=self.t`（alert 518 行 / 队列取出 1704 行），无原地改 body 不动 at 的路径，故比 at 足够且避免每帧比较长正文。
- **验证**：`cargo check --locked --lib --bins` 通过且 input_keys 警告消除；全量 **1412 passed / 0 failed / 131 ignored**；release exe `8e064bc7606447e6`，部署安装目录、启动 PID 20000。
- **状态**：提交 `441a89c` 本地 main 未 push；dist 的 v1.3.0 包（19:10）仍不含本轮所有修复，需重打。

### 7.0zzx 实验分支：文字改回原项目重排方案（2026-11-24 21:00–21:15，分支 `research/text-rescale-like-upstream` 提交 `e5885ef`）

- **用户问题**：原项目 Renly1994/xiaotang-daozai 没有文字黑线，定制版有，为什么？用户要求单开分支用原项目方法实验。
- **根因对比（已查证，非推测）**：
  - 原项目 `src/paint.rs`：`Card::font() = font(size * s)`，galley 按**最终字号**栅格化，直接 `self.pos()` 绘制，**不 transform mesh** → 无 UV 越界。
  - 定制版 `dz-phone/src/paint/mod.rs`：`galley()` 用基准字号 `font(size)`，`galley_shape()` 用 `TSTransform` 缩放 mesh + `inset_text_uv` 补丁。放大时顶点被拉开、UV 不变 → 线性采样采到图集相邻黑色像素 = 文字上方黑线（`inset_text_uv` 注释原文）。
  - 黑线是**定制版文字缩放死区/弹簧隔离机制**（`TEXT_SCALE_DEADZONE`、`7a579d2`、`ba7fa43`）的副产品，原项目无此机制自然无此问题。
- **实验改动**：`galley_shape` 去掉 transform/inset，改为从 `g.job` 重建 LayoutJob、font size 乘 s 后重排，直接画在 `self.pos(p)`。死区/弹簧隔离本分支暂不生效（预期代价：弹簧时文字每帧重排，可能重现高低不齐/抖动）。构建通过（4 个 unused 警告为预期），未跑全量（研究分支）。
- **产物**：exe `785b1e966f38705e`，桌面 `daozai-text-rescale-test\daozai.exe` 供实机对比。ui_probe 冻结背景截图未捕到岛仔（透明/DComp 窗口抓屏限制），交由用户实机验证。
- **状态**：实验在独立分支，**main 未受影响**（仍 `1bfc4c6`，ahead 20 未 push）；安装目录已恢复 main 正式版。待用户对比黑线是否消失 + 是否引入抖动，再决定合并/放弃/折中。

### 7.0zzza egui 升到上游 HEAD 90431d（2026-11-25 00:06–00:20，提交 `ec19bd1`，本地 main 未 push）

- **用户要求**：把 egui 升到最新提交 `90431d979ab1faae2de6d9f736e9d33ca0d31956`（emilk/egui HEAD，workspace 版本仍 0.37.0）。
- **上游差异**：旧 pin `56f3a0e` → 新 pin 只多 3 个提交：①`eframe::try_get_value`（纯新增，容错读取持久化数据）；②web canvas Ctrl+A 阻止原生拖拽（不涉及桌面）；③`Fix CJK glyphs sitting too high and getting clipped (#8804)`（`epaint/src/text/text_layout.rs`：回退字体字形统一按主字体 `font_ascent` 落基线，修中英/emoji 混排时 CJK 被顶高裁切）。**无公开 API 破坏，应用源码零改动。**
- **改动**：`dz-phone/Cargo.toml` 6 处 rev（含 Linux eframe / Windows egui-winit）+ 注释；`Cargo.lock` 11 个 egui 系 crate 的 source 换 rev，版本均 0.37.0，**无其他依赖版本漂移**。
- **验证**：`cargo build --locked --release --bin daozai --bin daozai-bridge` 通过；release lib 全量 **1412 passed / 0 failed / 131 ignored**（5m56s，串行隔离数据目录）；release bin **11 passed / 0 failed**；`cargo check --locked --release --examples` 通过。exe sha256 前16：`b4ac9366954efeed`（daozai）、`543c598181b8a33b5`（daozai-bridge）。
- **状态**：提交 `ec19bd1` 在本地 main，**未 push、未打包、未部署**；构建前停掉运行中的 daozai.exe（PID 19872），未重启；dist 的 v1.3.0 安装包不含此升级。新 CJK 基线是否改善文字观感/是否影响布局像素需用户实机确认。

### 7.0zzzb 静态缩放文字改按最终字号重排（2026-11-25 01:18–01:32，提交 `5e831e0`，本地 main 未 push）

- **用户选择**：在四条方案里选「静态改重排」——固定倍率文字按最终字号重排并缓存，弹簧动画继续走 transform + 6% 死区，兼顾锐利与稳定。
- **实现**：`TextScale` 新增 `Reflow` 变体；新增 `Card::scaled_static(cx,cy,k)`。`galley_shape()` 对 `Reflow` 从 `g.job` 克隆 LayoutJob、各 section `font_id.size *= s`、`wrap.max_width *= s`，经 `fonts_mut(layout_job)` 得到最终字号 galley，直接画在 `self.pos(p)`；**不做 mesh transform、不调用 `inset_text_uv`**。egui `GalleyCache` 哈希完整 LayoutJob + ppp，固定 k 命中缓存（新增测试断言同一 `Arc<Galley>` 且字号 12→14.4）。
- **调用点**：15 处字面量固定倍率（铃铛、通知徽标、图标、筛选按钮等）改用 `scaled_static`；弹簧动画（`ctl_scale`/press/appear/fit 等）保持 `scaled()`，逐帧变化的 k 绝不走重排，避免重现抖动。
- **不变量**：调用方仍传局部坐标 `p`，重排 galley 画在 `self.pos(p)`，`galley.size()` / `mesh_bounds()` 仍是局部单位——`centered_text`、时钟图标、天气对齐、UI 测试坐标无需改动。
- **验证**：`cargo check --locked --lib --tests --bins --examples` 通过；release 双 bin 构建通过；全量 lib **1413 passed / 0 failed / 131 ignored**（5m42s，串行隔离数据目录）；bin **11 passed / 0 failed**。exe sha256 前16 `65126b5017340cf8`（daozai）、`65776db024c9ec04`（bridge）。
- **状态**：提交 `5e831e0` 在本地 main，**未 push、未打包、未部署**；实机需确认静态放大文字黑线是否消失、居中/布局是否偏移。弹簧期间的文字缩放仍维持既有死区策略（用户此前确认观感可接受）。

### 7.0zzzc 冻结残余动画文字缩放 + 移除 UV 内缩（2026-11-25 01:37–01:56，提交 `bf996f8`，本地 main 未 push）

- **用户要求**：继续优化，且「有问题的地方先找社区方案」。
- **问题清单与社区对应**：
  1. 饮水进度环 `k≤1.22`（`pages.rs` 饮水页）、待办计数 `k≤1.10`（待办页）仍让文字走 transform 放大。egui [discussion #3197](https://github.com/emilk/egui/discussions/3197) 是用户自贴的 mesh 缩放方案且明确承认 aliasing；[#5374](https://github.com/emilk/egui/issues/5374)、[#7664](https://github.com/emilk/egui/issues/7664) 仍 open，无精确缩放 API；Apple HIG 与 Bevy B0005 都支持「空间层缩放、内容层文字不重栅格化」。→ 两处改 `scaled_text(k, 1.0)`，玻璃/容器照常弹，文字恒定字号。
  2. `scaled()` 若接到 `Reflow` 卡片上，逐帧变化的 k 会触发每帧重排。这是本地 API 组合漏洞，无社区对应。→ `scaled()` 检测到 `Reflow` 自动退回 `Auto`（即 Damped），新增测试断言字号仍为 12。
  3. `inset_text_uv` 最多内缩 1 texel 且每帧 `Arc::make_mut` 深拷贝字形 mesh。查 egui `TextureAtlas::allocate` 当前仍有 `PADDING = 1`（每块 glyph 间留 1px），说明 1 texel 内缩是过度补偿；第 1 项落地后生产路径不再有放大的文字。→ 删除函数、调用与旧测试，简化约 80 行。
  4. Flutter [#162949](https://github.com/flutter/flutter/issues/162949)（Impeller 下缩放文字抖动）确认这类问题在别的引擎也存在：部分是像素吸附的预期行为，部分是渲染浮点误差（其修复 PR #162480）。进一步佐证「不要靠连续缩放文字做动画」。
- **验证**：`cargo check --locked --lib --tests --bins --examples` 通过（修掉删除函数后的 unused import）；release 双 bin 构建通过（首次因进程占用失败，第二次成功）；全量 lib **1413 passed / 0 failed / 131 ignored**（删 1 个 UV 测试、加 1 个防护测试，总数持平）；bin **11 passed / 0 failed**。exe sha256 前16 `11f2bfef84c5cde9`（daozai）、`70489a398818b58e`（bridge）。
- **状态**：提交 `bf996f8` 在本地 main，**未 push、未打包、未部署**；构建期间检测到 daozai.exe PID 21808（疑似用户启动的新版实机测试），未停止该进程；如需运行最终 exe 请重启进程。
