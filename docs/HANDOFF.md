# 岛仔（Daozai）交接文档

> **最后更新**：2026-11-17 13:40
> **本文档是唯一权威交接入口**（`C:\Users\cay\Desktop\HANDOFF.md`）。
> 仓库内同步副本：`D:\code\daozai-custom\docs\HANDOFF.md`；凌晨版原件在
> `Daozai-液态玻璃预览\_archive\交接文档-2026-11-16.md`。

---

## 0. 30 秒速览

| 项目 | 当前值 |
|---|---|
| 正式版（日常使用） | `D:\software\Daozai`，补丁链 **custom-129**，exe sha256 前 16 = `58432d96c07d9d0e` |
| 液态玻璃源码 | `main` = `80d1c90`（…→`dabc21b`→`6a2bd1b`→`80d1c90` 静止降帧+深休眠） |
| **工作树** | **干净**（2026-11-17 13:40 最后一次提交 `68df96d`） |
| 预览版（液态玻璃试验） | 已部署到 `%LOCALAPPDATA%\Programs\DaozaiLiquidTest`，标记 `liquid-test-42-mailglass`，exe sha256 前 16 = `a701a6bf12110dfd`。本轮（13:14–13:40）**邮箱空态隐藏全部/未读透镜 + 去添加按钮上 GPU 玻璃并文字居中**；上轮（11:30–13:10）静止降帧（15fps 档）+ 收起深度休眠（1Hz，宠物冻结不挡）；再上轮 MemoryHints A/B 实测后保持默认（留 DAOZAI_WGPU_MEMHINTS 开关）；更早（19:00–00:25）：主题随壁纸明暗自动切换、刘海形状真实轮廓、视频页控件玻璃化、圆钮按 Pack-v2、图标缩小、自适应帧率（静止 30fps）、可选优先独显（548→257MB）、显示器拓扑切换不再花屏 |
| 补丁链 | `daozai-patches/apply.py`，`PATCH_ID = "custom-129"`，EDITS 237 / NEW_FILES 12 |
| 测试 | 主线 1176 passed；液态玻璃线最近 **1189 passed / 0 failed / 119 ignored**；**必须 `--test-threads=1`** |
| 单实例锁 | `Local\Daozai.SingleInstance` —— 正式版与预览版**不能同时运行** |
| GitHub 发布 | **Beta**：https://github.com/cayww/daozai-custom/releases/tag/liquid-test-18（tag 指向 `f415518`）；**本地新包 liquid-test-42 未发 GitHub**：`C:\Users\cay\Desktop\Daozai-液态玻璃预览\daozai-liquid-setup-42.exe`（= `daozai-liquid-setup.exe`），9,678,334 B，sha256 前16 `bbb18246ab89a2a6`，内含 exe `a701a6bf12110dfd`。-41 已不在桌面（包本体在 `installer\liquid-test\daozai-liquid-test-41-lowsleep.exe`），-40 及更早已移到 `_archive\` |

**一句话**：日常用的正式版停在 custom-129；`main` 已吃进液态玻璃（50 个 commit，HEAD=`bf116c2`），
但液态玻璃**没进补丁链**，只在预览目录里跑。上一轮把「视觉细节、帧率、内存」三件收尾了
（静止 30fps / 动时 60fps，548MB→257MB）；本轮修「每翻一次页控件白边就黑一次」：
根因是主题采样框跟着岛体尺寸变（翻页就重新判定深浅），以及玻璃控件的白边被皮肤换色换成了墨色。

---

## 1. 红线（先看这个）

| 项目 | 规则 |
|---|---|
| `main` 分支 | 已含液态玻璃，**不要 reset 回 `4b0b427`**，不要把工作树丢弃 |
| 补丁链 `apply.py` | 仍是 `custom-129`，**不要改**；液态玻璃进链是另一套流程（custom-130），需用户明确同意 |
| 正式部署 `D:\software\Daozai` | **不要覆盖**，不要在正式目录里跑预览版 |
| `Cargo.lock` | **不要动**，加依赖会破坏 `--locked` 构建 |
| 用户数据 | 验证时不要真发邮件 / 标已读 / 改账户；`%APPDATA%\Daozai\settings.txt` 是用户的 |
| 工作日志 | `.workbuddy/memory/*.md` 是追加式，不要删、不要改写历史 |

---

## 2. 两条线的关系（最容易搞混的地方）

```
上游 xiaotang-daozai v0.6.64
        │
        ├── 主线：补丁链 daozai-patches/apply.py  →  custom-129  →  D:\software\Daozai（用户日常用）
        │        └ 所有定制固化成 EDITS，可从干净源码重放
        │
        └── 液态玻璃：feat/liquid-glass-a  →  已快进合并进 main（现为 234ff52）
                 └ 直接改源码、不走补丁链；只在预览目录跑，未部署到正式目录
```

- 主线**只能**通过补丁链改，交付前必须重放校验。
- 液态玻璃**直接改 main 源码**，不需要补丁链；将来若要进链，是把净差异转成 EDITS（另开一轮）。
- 两条线共用同一个 `D:\code\daozai-custom` 代码库，但分支不同、交付路径不同。

---

## 3. 路径与指纹

```
D:\code\daozai-custom\                       源码库（main = 液态玻璃已合并）
  dz-phone\src\                              Rust 源码
  dz-phone\src\gpu_glass.rs                  液态玻璃核心（WGSL shader + 抓屏 + 组帧）
  dz-phone\src\wgc.rs                        Windows Graphics Capture 封装
  daozai-patches\apply.py                    补丁链（custom-129）
  daozai-patches\PATCHES.md                  逐版详细记录（2963 行，查历史看这里）
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

- [5. 主线：补丁链规则与流程](#5-主线补丁链规则与流程)
- [6. 液态玻璃：架构与硬约束](#6-液态玻璃架构与硬约束)
- [7. 待办（接手先做这个）](#7-待办接手先做这个)
- [8. 版本时间线](#8-版本时间线)
- [9. 接手注意（操作层面）](#9-接手注意操作层面)
- [附录 A：常用命令速查](#附录-a常用命令速查)
- [附录 B：调试开关](#附录-b调试开关)

---

## 5. 主线：补丁链规则与流程

### 5.1 补丁链结构

```python
PATCH_ID = "custom-129"

EDITS = [
    ("src/app.rs",
     '''<原样文本>''',
     '''<替换后文本>'''),
    ...
]
NEW_FILES = [("island_status.rs", "island_status.rs"), ...]   # 新增模块放副本，不生成 EDITS
```

按顺序逐条 `replace`，每条要求**在应用那一刻恰好出现一次**。

**五条铁律**：

1. `old` 在应用那一刻唯一；同一段文本可被多条 EDITS 先后使用，后一条必须排在后面。
2. 字符串里的反斜杠要转义：Rust 的 `\n` 写成 `\\n`，路径 `\D` 写成 `\\D`（`append_edits.py` 会处理）。
3. `old`/`new` 里不能出现 `'''`。
4. 改 `apply.py` **不要用无差别 replace**（会误伤已有 EDITS 的 `new` 片段，重放出两份），改完查 `count`。
5. **新增模块走 `NEW_FILES`**（现 12 个）：`apply.py` 在 EDITS 之后才复制，副本放在 `daozai-patches/`。
   改这类文件要**同步副本**，不能生成 EDITS（重放时读不到文件）；新建模块必须登记进 `NEW_FILES`。

### 5.2 标准工作流（九步，一步都不能跳）

| 步骤 | 动作 | 关键命令 |
|---|---|---|
| 1 | 读代码定位改动点 | 先 `grep` 确认锚点唯一 |
| 2 | 改工作源码 | 脚本批量替换，每条断言命中 1 次 |
| 3 | 编译 | `cargo build --locked --release --bin daozai` |
| 4 | 全量测试 | `cargo test --locked --release --lib -- --test-threads=1` |
| 5 | 固化进补丁链 | `python <skill>/scripts/append_edits.py <spec.json>` |
| 6 | **干净源码重放校验** | `python <skill>/scripts/replay_check.py` |
| 7 | 部署 | 退进程 → 备份 `daozai.exe.bak-custom<N-1>` → 替换 → 同步 `CUSTOM_BUILD.txt` → 重启 |
| 8 | 实机验证 | `python <skill>/scripts/ui_probe.py` 截图/模拟操作 |
| 9 | 同步文档 | `PATCHES.md` / `CHANGELOG.md` / 本文档 / 工作日志 |

**第 6 步是硬门槛**：重放不一致不许部署。转义层级、重复条目、锚点漂移只有重放能发现。

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
| `daozai-patches/PATCHES.md` | 新章节：问题、根因、改动、验证 |
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
7. **是否进补丁链（custom-130）**：稳定后另议，需用户明确同意。

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
| `68df96d` | **邮箱空态隐藏全部/未读透镜 + 去添加按钮上 GPU 玻璃并文字居中** = liquid-test-42-mailglass；1193 passed；**当前 HEAD** |

> custom-33 ~ 119 的逐版记录见 `daozai-patches/PATCHES.md`（2963 行，按版本分节，含问题/根因/改动/验证）。
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

# 补丁链重放校验（硬门槛）
python <skill>/scripts/replay_check.py

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
- 主线逐版记录：`daozai-patches\PATCHES.md`
- 本工作区日志：`C:\Users\cay\WorkBuddy\2026-11-11-23-14-32\.workbuddy\memory\`
