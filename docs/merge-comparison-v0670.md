# 岛仔 v0.6.70 上游合并对照表

> 更新时间：2026-11-25 21:30（3.1 节已按优融合，提交 `1f88d6c`，全量 1471 passed / 0 failed）
> 生成时间：2026-11-25 20:30  
> 分支：`merge-upstream-v0670`，合并提交 `9dad7c8`（双亲：`017108d` 本地 main + `0615f19` 上游 v0.6.70）  
> 上游区间：`e080728`（v0.6.65，上次合并基线）→ `0615f19`（v0.6.70），**418 commit**  
> 验证：release 构建通过；全量 1456 passed / 0 failed / 138 ignored  
> 说明：表中「定制侧」= 本地 main `017108d`；「上游侧」= `upstream/main` `0615f19`。

---

## 一、合并进来的内容（按模块）

| 模块          | 上游内容                                             | 纳入方式                                                       | 关键文件 / 目录                                                                                                   |
| ----------- | ------------------------------------------------ | ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| 插件体系        | 插件集市（排序/头像/本地包导入）、已安装管理、独立功能入口、插件画布、通知/状态插件、设置目录 | 新增文件全纳入（特性门控 `sdk-js`）                                     | `dz-phone/src/plugin_canvas*`、`plugin_notice*`、`plugin_status*`、`quota_plugin*`、`plugin_setting_catalog.rs` |
| 插件 SDK      | GPU 材质 SDK、JS/WASM SDK、示例与任务清单                   | 源码纳入，特性 `sdk-material`/`sdk-js`/`sdk-wasm` 声明；定制版默认不启用材质宿主 | `dz-phone/src/material/`、`spike/plugins/`、`docs/plugins/`、`spike/examples/`                                 |
| 系统性能 / 无线设备 | 性能详情与常驻指标页、蓝牙设备状态/电量、设备兼容包                       | 新增模块纳入                                                     | `dz-phone/src/performance*.rs`、`bt/`、`bluetooth_ui*.rs`、`assets/device-icons/`（17 个图标）                      |
| 日历          | 按日期浏览、农历/节气、日期常驻、本机日期时间选择器、CalDAV 空日历修复          | 新增模块纳入；UI 主文件保留定制侧                                         | `dz-phone/src/lunar_calendar.rs`、`resident_date.rs`、`plugin_timezone_cities.tsv`                            |
| 邮箱          | 仅验证码提醒、Gmail Windows 手动代理、代理路由                   | 新增模块纳入；邮件主逻辑保留定制侧                                          | `dz-phone/src/mail_proxy*.rs`、`module_mail_tests.rs`                                                        |
| 音乐          | 版本适配表、自动学习进度、事件唤醒采样、波点 trk 编号修复                  | 新模块纳入；播放器主逻辑保留定制侧                                          | `dz-phone/src/music_compat.rs`、`native_capture.rs`、`netease_*`（适配后）                                         |
| AI 额度       | Antigravity 官方 CLI、中转站彩色图标、HTTP 查询、联网感谢墙         | 新模块纳入                                                      | `dz-phone/src/antigravity_quota/`、`supporters.rs`、`assets/ai-icons/`（7 个）、`scripts/supporters.py`           |
| 通知 / 提醒     | 通用宿主接口、健康提醒音效                                    | 新模块纳入                                                      | `dz-phone/src/plugin_notice*`、`assets/health-reminder.wav`                                                  |
| 布局 / 窗口     | 底部停靠、跨屏拖动、截图录屏隐藏、窗口边界                            | 新模块纳入（未接线到定制主流程）                                           | `dz-phone/src/window_bounds.rs`、`crop_front_buffer.rs`、`display_layout.rs`（后回退）                             |
| 文本          | RTL 阿拉伯语连写、时区城市表                                 | 纳入                                                         | `dz-phone/src/rtl_text.rs`（双方新增一致）                                                                          |
| 文档 / 资源     | 上游 README、蓝牙与插件文档、readme 图片                      | 文档以定制版为底改写；资源纳入                                            | `README.md`（融合）、`docs/bluetooth/`、`docs/plugins/`、`assets/support/*.webp`                                   |

统计：合并提交相对 main **新增 346 文件、修改 62 文件**；逐文件比对中，**378 文件完全取上游侧**、**135 文件保留定制侧**、**22 文件为真实融合**、374 文件双方一致。

---

## 二、冲突取舍对照（共 141 处）

### 2.1 文件位置冲突（101 处）

| 类别                                                                        | 数量 | 选择            | 原因                                                            |
| ------------------------------------------------------------------------- | -- | ------------- | ------------------------------------------------------------- |
| 上游新增文件落在被本仓库重命名的目录（`src/`→`dz-phone/src/`，`assets/`→`dz-phone/assets/` 等） | 93 | **取上游侧新增文件**  | 纯新增，无内容分歧；git 已自动映射到 `dz-phone/` 下，直接暂存                       |
| 双方各自新增且内容完全一致                                                             | 8  | **任一侧（内容相同）** | blob 哈希一致：`health-reminder.wav`、7 个 settings 图标、`rtl_text.rs` |

代表文件：`assets/device-icons/*.svg`（17）、`src/plugin_canvas*`（15）、`src/bt/*`（6）、`src/material/*`（6）、`src/antigravity_quota/*`（3）、`assets/ai-icons/*`（7）、`src/supporters.rs`、`src/window_bounds.rs`、`scripts/supporters.py` 等。


### 2.2 内容冲突（30 处）

| 文件                                                                                                          | 选择                 | 优劣对比与理由                                                                                                                                                          |
| ----------------------------------------------------------------------------------------------------------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml`                                                                                                | **融合**             | 保留定制 egui 0.37 git pin、版本 1.3.0、`memprof`；纳入上游 `ab_glyph`/`svgtypes`、GATT windows 特性、`sdk-*` 特性。`sdk-material` 去掉 `eframe/direct-composition`（定制 pin 无此 feature） |
| `Cargo.lock`                                                                                                | **融合**             | 不手工拼冲突块：以上游锁为底跑 `cargo metadata` 重算，egui 仍锁 0.37/90431d9                                                                                                         |
| `README.md`                                                                                                 | **融合**             | 上游文档完整丰富，以它为底；加回「本仓库是个人定制版」声明、HANDOFF 入口与构建命令                                                                                                                    |
| `.gitignore`                                                                                                | **定制侧**            | 定制侧额外忽略 `merge-work/`、`dz-phone.tar.gz`、Python 缓存；上游只忽略两个 `.cmd`                                                                                                 |
| `app.rc`                                                                                                    | **定制侧**            | 定制侧用 `@DAOZAI_VERSION@` 模板由 build.rs 注入版本；上游硬编码 0.6.70                                                                                                           |
| `app.rs`                                                                                                    | **定制侧**（后做 API 适配） | 上游含 41 处改动（11 页、插件导航、性能页等），但会连带引入定制侧缺失的 100+ 字段；先保留定制侧，再单独适配 messenger API                                                                                       |
| `main.rs`                                                                                                   | **定制侧**（后做 API 适配） | 保留 D3D11 自绘构造；适配上游 messenger：`TaskbarEvent::Message(unread,target)`、`register_taskbar`、`tray_opening` 接收 `Result<Target,_>`（顺带修钉钉 9.x 账号确认）                      |
| `model.rs` / `pages.rs`                                                                                     | **定制侧**            | 整文件改写级分歧（定制 9 页 + 全面板布局 vs 上游 11 页 + 设置重设计）；保留定制，避免 100+ 字段连锁                                                                                                    |
| `lib.rs`                                                                                                    | **定制侧**            | 保留定制模块树（`gpu_glass_stub`/`d3d11`/`fluid` 等）；新增模块以特性门控方式另行纳入                                                                                                      |
| `ui_tests.rs`                                                                                               | **定制侧**（补初始化）      | 上游测试期望新布局；保留定制 9576 行测试，只给 `bt::Device` 初始化补 `..Default::default()`                                                                                              |
| `src/avoidance.rs`                                                                                          | **定制侧**            | 定制用 `Id::unique` + 深色弹窗框；上游用共享 `settings_modal_frame` + 遮罩，属样式分歧                                                                                                 |
| `src/calendar_editor.rs` / `calendar_ui.rs`                                                                 | **定制侧**            | 上游引入 `ControlRole`/材质控件与新版日期选择器；定制侧布局与坐标断言不同                                                                                                                     |
| `src/layered.rs`                                                                                            | **定制侧**            | 与 D3D11 玻璃/分层窗口实现强耦合                                                                                                                                             |
| `src/mail.rs`                                                                                               | **定制侧**            | 保留定制邮件筛选、排队「全部已读」、超时复位                                                                                                                                           |
| `src/quota_ui.rs`                                                                                           | **定制侧**            | 保留 ZCode 连接入口；上游已删除 ZCode                                                                                                                                        |
| `src/resident_settings.rs` / `side_dock.rs` / `skin_paint.rs` / `system.rs` / `task_picker.rs` / `theme.rs` | **定制侧**            | 均为定制渲染/布局/常驻栏行为，上游改动与之互斥                                                                                                                                         |
| `src/updates.rs`                                                                                            | **定制侧**            | 保留定制诊断链接与更新提示逻辑                                                                                                                                                  |
| `src/weather_source_ui_tests.rs`                                                                            | **定制侧**            | 上游测试依赖新天气源 API                                                                                                                                                   |
| `vendor/eframe/src/native/run.rs`                                                                           | **定制侧**            | 定制 egui 0.35→0.37 迁移后的事件循环修改，不能回退                                                                                                                                |
| `installer/setup.nsi`                                                                                       | **定制侧**            | 保留定制安装路径、快捷方式与诊断脚本清理逻辑                                                                                                                                           |
| `examples/lyrics_render_soak.rs` 等 3 个                                                                      | **定制侧**            | 上游 `native.update()` 新增 `blur`/`resident_clip` 参数；定制侧签名不同，保留原调用                                                                                                  |

### 2.3 删除 / 新增 / 重命名冲突（10 处）

| 文件                                                                                 | 选择               | 理由                                             |
| ---------------------------------------------------------------------------------- | ---------------- | ---------------------------------------------- |
| `CHANGELOG.md`（上游修改 / 定制侧移到 `dz-phone/`）                                           | **定制侧为准 + 上游摘要** | 保留定制 CHANGELOG，新增一节写 v0.6.66–v0.6.70 上游更新摘要    |
| `dz-phone/src/zcode_quota.rs`（上游删除 / 定制侧修改）                                        | **定制侧**          | ZCode 连接是定制功能，`quota_ui.rs` 与 `quota.rs` 仍引用   |
| `dz-phone/scripts/collect-diagnostics.ps1`（上游删除 / 定制侧保留）                           | **定制侧**          | `diagnostics_tool.rs` 用 `include_bytes!` 内嵌该脚本 |
| `dz-phone/编译并测试-Claude.cmd`、`编译测试版-Claude.cmd`（上游删除 / 定制侧保留）                       | **定制侧**          | 开发辅助脚本，无害                                      |
| `src/extras.rs`、`src/graphics.rs`、`src/native.rs`、`src/paint.rs`（上游旧路径修改 / 定制侧已移动） | **删除旧路径**        | 内容是上游旧目录结构；定制侧对应文件在 `dz-phone/src/`            |
| 4 处 rename/delete（同上文件的重命名形态）                                                      | **同上**           | 保持 `dz-phone/` 单一路径                            |



---

## 三、未合并 / 保留定制侧的内容（对照）

### 3.1 更新：已按优融合（2026-11-25 21:30，提交 `1f88d6c`）

对 3.1 节的 58 个文件做三方试算，40 个零冲突、17 个有实质改动。按「能融则融、互斥则保留」处理：

| 文件 | 处理 | 融合内容与取舍 |
|---|---|---|
| `diagnostics_tool.rs` | **已融合** | 上游原生 ZIP 诊断替代 PowerShell 脚本（`environment.rs` 双方已一致）；页面数校验按本地 9 页改写，测试 fixture 适配 |
| `quota.rs` / `api_quota.rs` | **已融合** | 纳入 Antigravity CLI 来源、RelayIcon 中转站彩色图标（7 种）、ZCode→GLM 重连提示、账号重连保留偏好；**保留定制 ZCode 本机登录**（模块声明 + add/fetch 走本地实现，测试按定制行为改写） |
| `weather_source_ui.rs` / `weather_ui.rs` / `weather_source_ui_tests.rs` | **已完整融合** | 上游和风天气三步配置、本机/账号额度卡、来源切换防闪（当帧 `request_discard`）、IME 组字防误保存、来源配置区始终绘制、测试视口适配；`ime_composing`（egui 0.31 vendor 专有）改用 egui 0.37 的 `Event::Ime` 判断，`page_weather` 保留本地 3 参签名 |
| `bar_layout.rs` / `performance.rs` | **未融合** | 上游 `DEFAULT_ORDER` 改为 8 槽（加 PERFORMANCE），本地 `resident_settings.rs` 与 1100+ 布局测试仍是 7 槽，试融合导致 15 个测试失败；收益不抵连锁改动 |
| `display_layout.rs` / `shape.rs` / `composition.rs` | **未融合** | 依赖 `Dock::Bottom`、`native::apply_outline` 等定制版不存在的能力，属上游底部停靠特性，未接线 |
| `mail_manage_ui.rs` / `mail_fetch.rs` / `mail_manage.rs` | **未融合** | 依赖 `use_windows_proxy`、`bell_at` 等上游新增字段，与定制邮件模型互斥 |

验证：`cargo build --locked --release` 通过；全量 **1471 passed / 0 failed / 139 ignored**（比融合前多 15 passed）。

### 3.1 原始清单：上游有改动但保留定制侧的核心文件（58 个 `dz-phone/src` 文件，部分已按上表融合）

| 类别    | 文件                                                                                                                                                                                                                       | 未采纳的上游改动                            | 保留定制侧的原因                     |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------- | ---------------------------- |
| 渲染栈   | `native.rs`、`layered.rs`、`composition.rs`、`shape.rs`、`glass.rs`、`fluid.rs`、`display_layout.rs`、`bar_layout.rs`                                                                                                           | 通用 GPU 材质、毛玻璃、跨屏/底部停靠布局             | 定制 D3D11＋液态玻璃栈，API 与绘制顺序完全不同 |
| 主界面   | `app.rs`、`pages.rs`、`model.rs`、`lib.rs`、`main.rs`                                                                                                                                                                        | 11 页导航、设置重设计、新字段体系                  | 连锁 100+ 字段，保留定制 9 页与布局       |
| 通知    | `system.rs`、`avoidance.rs`、`updates.rs`                                                                                                                                                                                  | 上游通知/避让/更新提示                        | 定制通知只上岛、日志打码、避让行为            |
| 邮件    | `mail.rs`、`mail_fetch.rs`、`mail_manage.rs`、`mail_manage_ui.rs`                                                                                                                                                           | 上游 Gmail 代理字段 `use_windows_proxy` 等 | 定制账号筛选、排队已读、超时复位             |
| 音乐    | `lyrics.rs`、`netease_playback.rs`、`player_process.rs`、`player_seek.rs`、`soda_cache.rs`、`music_gaps.rs`、`music_volume.rs`                                                                                                 | 上游播放适配状态机                           | 定制歌词 lane 重排与音量/进度行为         |
| 额度    | `quota.rs`、`quota_ui.rs`、`api_quota.rs`、`claude_quota.rs`、`kiro_quota.rs`、`deepseek_quota.rs`、`workbuddy_quota.rs`                                                                                                       | 上游 `RelayIcon`、中转站彩色图标、HTTP 源       | 保留 ZCode 与定制额度面板             |
| 日历/天气 | `calendar_ui.rs`、`calendar_editor.rs`、`local_calendar.rs`、`weather_ui.rs`、`weather_source_ui.rs`                                                                                                                         | 上游日期选择器、天气源交互                       | 定制布局与测试坐标断言                  |
| 其他    | `otp.rs`、`clipboard.rs`、`focus_bar.rs`、`health_reminder_ui_tests.rs`、`input.rs`、`settings_theme.rs`、`side_dock.rs`、`skin_paint.rs`、`task_picker.rs`、`theme.rs`、`tray.rs`、`qweather.rs`、`cover_search.rs`、`netease_*` 测试等 | 各类上游行为调整                            | 与定制行为/测试冲突                   |

### 3.2 上游特性未接线（已纳入源码但定制主流程未启用）

| 上游特性                       | 状态                                                                                           | 影响                     |
| -------------------------- | -------------------------------------------------------------------------------------------- | ---------------------- |
| GPU 材质插件宿主（`sdk-material`） | 源码纳入，`Cargo.toml` 声明特性，默认不启用；`main.rs` 的 `reconcile_material` / `sync_capture_exclusion` 未移植 | 外观材质项不会出现；定制液态玻璃继续工作   |
| 底部停靠 / 跨屏拖动                | `window_bounds.rs` 等纳入，`display_layout.rs`/`app.rs` 保留定制侧                                    | 上游停靠 UI 不生效            |
| 11 页导航（性能/无线设备作为岛页）        | `performance.rs`/`bluetooth_ui.rs` 纳入，`app.rs`/`pages.rs` 保留 9 页                             | 性能页与无线设备页源码存在但未接入岛体主界面 |
| 插件画布/通知/状态插件 UI            | 源码纳入，`lib.rs` 中以 `sdk-js` 门控                                                                 | 默认构建不编译这些模块            |
| 上游设置重设计与新测试                | `settings_redesign_ui_tests.rs` 等纳入但测试主文件回退                                                  | 不采用上游设置页布局             |
| 上游 ZCode 删除                | 未采纳，恢复 `zcode_quota.rs` 与 `quota.rs` 定制版                                                     | ZCode 连接 continue 可用   |
| 上游诊断脚本删除                   | 未采纳，恢复 `collect-diagnostics.ps1`                                                             | 内嵌诊断脚本继续可用             |

### 3.3 上游 vendor 变更

| 目录                         | 处理      | 说明                                                                                       |
| -------------------------- | ------- | ---------------------------------------------------------------------------------------- |
| `dz-phone/vendor/*`（64 文件） | **定制侧** | egui/eframe/wgpu-hal/winit 等 vendored 代码是定制迁移线（egui 0.37 pin、D3D11 相关补丁），不能回退到上游 0.31 实现 |

---

## 四、补充：文字与翻页抖动的处理（用户问询）

| 主题 | 上游做法 | 定制版现状 | 是否接入 |
|---|---|---|---|
| 文字缩放抖动 | 上游 egui 0.31 用 `c.scaled()` 每帧重排文字（`TextShape::transform` 只缩顶点不改 UV），动画时字号连续变化会抖 | 定制版**更优**：`paint::TextScale::{Auto,Follow,Damped,Frozen}`，`scaled()` 自动走阻尼死区 + 1/64 量化 + 1/256 相位稳定；动画文字用 `scaled_static()` 恒字号隔离弹簧尾振（`app.rs` 中通知铃铛等已改用） | **不需要接入**——定制方案优于上游，HANDOFF §7.0zzq–zzt 有完整推导 |
| 翻页箭头位置抖动 | 上游 `0c216b6` 按 `Dock` 决定箭头锚点（Bottom/侧边偏移） | 定制版保留本地箭头逻辑（9 页、顶部停靠为主）；上游的 Bottom/侧边锚点依赖 `Dock::Bottom`，定制版无此停靠模式 | **不适用**（无 Bottom 停靠） |
| 滚轮翻到矮页误收起 | 上游 `6156474` 引入 `PAGE_EDGE_OVERLAP=8.0` 续滚带，切页后保留旧命中区一条重叠带 | 定制版 `app.rs:138-139`、`640`、`3182-3192` **同源实现已在**（`page_scroll_area` + 8px 重叠带） | **已接入**（合并前就有） |
| 兼容渲染 60fps / DX12 读回复用 | 上游 `89421ba`/`a7d923b` 改 `vendor/egui-wgpu/src/capture.rs`、eframe 帧调度 | 定制版走自研 D3D11 + WGC 自绘栈（`native.rs`/`d3d11.rs`），无 wgpu/DX12 读回链 | **不适用**（渲染栈不同） |
| 输入法组字误保存/误选城市 | 上游 `042f938` 用 `viewport.ime_composing`（egui 0.31 vendor 专有字段） | 定制版 `app.rs:2224-2228` 有等价的 `todo_ime_active`/`todo_ime_frame` 事件判断；本次融合中天气/城市的 IME 判断改用 egui 0.37 `Event::Ime` 检测 | **已接入**（等价实现 + 本次融合适配） |

## 四、结论

- 合并策略：**新增全收、冲突定制优先、基础设施（Cargo/lock）融合、messenger API 定向适配；3.1 节能融则融（诊断/额度/天气已融，布局互斥项保留）**。
- 未合并项主要集中在**与定制渲染栈和 9 页布局强耦合的 UI 主文件**，以及为避免连锁重构而保留的 vendor 代码。
- 已纳入但未接线的新特性（材质宿主、底部停靠、性能/无线设备页）如要启用，需要单独做适配提交；当前分支保持与合并前一致的定制行为，构建与测试全绿。
