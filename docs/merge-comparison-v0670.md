# 岛仔 v0.6.70 上游合并对照表（最终结果）

> 分支：`merge-upstream-v0670`；定制基线 `017108d`，上游 `0615f19`（tag v0.6.70），区间 418 commit
> 最终代码提交 `a63b422`；release 构建通过；全量 **1536 passed / 0 failed / 140 ignored**
> 测试包：`daozai-setup-merge-v0670c.exe`；未部署、未推 origin
> 说明：「融合」= 双方都有改动、按优取舍后的结果；「合入」= 上游新增直接纳入；「未融合」= 最终保留定制侧或未采纳。

---

## 一、融合（双方改动，按优取舍）

| 文件 / 模块 | 最终结果 |
|---|---|
| `Cargo.toml` / `Cargo.lock` | 保留定制 egui 0.37 pin（90431d9）、版本 1.3.0、memprof；纳入上游 `ab_glyph`/`svgtypes`、GATT windows 特性、`sdk-*` 特性；`sdk-material` 去掉 `eframe/direct-composition`。锁文件 egui 仍锁 0.37 |
| `README.md` | 上游文档为底 + 定制声明、HANDOFF 入口、构建命令 |
| `app.rs` / `main.rs` | 定制 D3D11 自绘构造 + 上游 messenger API（`TaskbarEvent::Message(unread,target)`、`register_taskbar`、`tray_opening` 收 `Result<Target,_>`）；含钉钉 9.x 账号确认修复 |
| `model.rs` | 保留定制 9 页；纳入 `performance_bar`/`performance_metrics`/`resident_date` 字段与序列化、7 槽→8 槽配置迁移 |
| `lib.rs` | 定制模块树 + 插件模块声明（plugin_notice/status/canvas/quota_plugin） |
| `ui_tests.rs` | 定制测试主体 + `bt::Device` 初始化补全；拖动切边测试限 Top/Left/Right（Bottom 仅静态停靠） |
| `diagnostics_tool.rs` | 上游原生 ZIP 诊断（替代 PowerShell 脚本）；页面数校验按本地 9 页 |
| `quota.rs` / `api_quota.rs` | 上游 Antigravity CLI、RelayIcon 七种中转站彩色图标、ZCode→GLM 提示、重连保留偏好；保留 ZCode 本机登录 |
| `weather_source_ui.rs` / `weather_ui.rs` / `weather_source_ui_tests.rs` | 上游和风三步配置、本机与账号额度卡、来源切换防闪、IME 组字防误保存；按 egui 0.37 适配 |
| `bar_layout.rs` / `performance.rs` / `resident_settings.rs` | 上游 8 槽常驻栏（含 PERFORMANCE/BLUETOOTH）、`add_performance`/`append_pinned`、`set_clock_width`/`date_group_clearance`；常驻日期模式纳入 |
| `display_layout.rs` / `shape.rs` / `side_dock.rs` / `winit_runner.rs` | 上游 `Display.work`/`auto_bottom`/`dpi`、跨屏共享边与拖动目标、`Dock::Bottom` 变体及分支；拖动切边仍限三边 |
| `mail.rs` / `mail_fetch.rs` / `mail_manage.rs` / `mail_manage_ui.rs` | 上游 `use_windows_proxy` + Gmail 代理连接、`MailState.bell_at`；保留本地 20 秒连接超时 |
| 插件市场 | `sdk-js` 默认启用；插件通知/状态/画布/额度插件接入；`theme::ORANGE`/`canvas_font`、`paint::coffee`/`track`、`AlertSource::Plugin`、`Cmd::OpenPluginNotice`、`App.plugins` 已补；插件通知不挤占内置提醒队列 |
| `CHANGELOG.md` | 定制 CHANGELOG 为准 + 上游 v0.6.66–v0.6.70 更新摘要 |
| 性能页接入 | 主界面扩为 10 页（系统性能，默认隐藏、可在页面管理开启）；App 接 `performance::Monitor` + tick + 绘制；Settings 加 `show_performance`/`performance_view` 及配置迁移 |
| 翻页箭头 | 按 `Dock` 计算锚点（Bottom 用剪裁区底边、侧边用高度弹簧补偿），沿用定制玻璃/弹簧绘制 |

---

## 二、合入（上游新增，直接纳入）

| 模块 | 内容 | 关键文件 / 目录 |
|---|---|---|
| 插件体系 | 插件画布、通知/状态插件、设置目录、已安装管理、额度插件 | `src/plugin_canvas*`、`plugin_notice*`、`plugin_status*`、`quota_plugin*`、`plugin_setting_catalog.rs` |
| 插件 SDK 资源 | GPU 材质 SDK、JS/WASM SDK 源码与示例（材质不启用） | `src/material/`、`spike/`、`docs/plugins/` |
| 系统性能 / 无线设备 | 性能采样与指标、蓝牙设备状态/电量、设备图标 | `src/performance*.rs`、`bt/`、`bluetooth_ui*.rs`、`assets/device-icons/`（17 个） |
| 日历 | 农历/节气、常驻日期、时区城市表 | `src/lunar_calendar.rs`、`resident_date.rs`、`plugin_timezone_cities.tsv` |
| 邮箱 | Gmail 代理路由与测试 | `src/mail_proxy*.rs`、`module_mail_tests.rs` |
| 音乐 | 版本适配表、自动学习进度、事件唤醒采样 | `src/music_compat.rs`、`native_capture.rs` |
| AI 额度 | Antigravity CLI、联网感谢墙、AI 图标 | `src/antigravity_quota/`、`supporters.rs`、`assets/ai-icons/`（7 个）、`scripts/supporters.py` |
| 通知 / 资源 | 健康提醒音效、设置图标、支持页图片 | `assets/health-reminder.wav`、`assets/settings-icons/system/`、`assets/support/*.webp` |
| 文本 / 布局 | RTL 阿拉伯语连写、窗口边界、截屏裁剪 | `src/rtl_text.rs`、`window_bounds.rs`、`crop_front_buffer.rs` |
| 文档 | 蓝牙与插件文档 | `docs/bluetooth/`、`docs/plugins/` |

统计：相对定制 main **新增 346 文件、修改 62 文件**；141 处冲突中 101 处为目录重命名引起的位置冲突（93 处取上游新增文件、8 处双方内容一致）。

---

## 三、未融合 / 未合并（保留定制侧或未采纳）

| 项 | 最终状态 |
|---|---|
| 主界面 9 页布局 | 保留定制；未采用上游 11 页导航与设置页重设计 |
| 渲染栈 | `native.rs`/`layered.rs`/`glass.rs`/`fluid.rs`/`composition.rs`/`skin_paint.rs` 保留定制 D3D11＋液态玻璃；未采纳上游通用 GPU 材质实现 |
| 通知 / 避让 / 更新 | `system.rs`/`avoidance.rs`/`updates.rs` 保留定制（只上岛、日志打码、定制避让） |
| 日历 UI | `calendar_ui.rs`/`calendar_editor.rs`/`local_calendar.rs` 保留定制布局 |
| 邮件主逻辑 | 保留定制账号筛选、排队「全部已读」、超时复位（代理字段已融） |
| 音乐播放 | `lyrics.rs`/`netease_playback.rs`/`player_process.rs`/`player_seek.rs`/`music_volume.rs`/`soda_cache.rs` 保留定制歌词与音量行为 |
| 额度 UI | `quota_ui.rs` 保留定制面板；ZCode 连接保留（上游删除 ZCode 未采纳） |
| 诊断脚本 | `collect-diagnostics.ps1` 保留（上游删除未采纳） |
| vendor 代码 | `vendor/*`（64 文件）保留定制 egui 0.37 pin 与 D3D11 补丁 |
| 其他定制文件 | `.gitignore`、`app.rc`、`installer/setup.nsi`、`examples/` 3 个保留定制 |
| 无线设备页 | 源码在库，未接入岛体主界面导航（依赖整套 wireless 状态机） |
| GPU 材质插件宿主 | `sdk-material` 不启用；`reconcile_material`/`sync_capture_exclusion` 未移植；液态玻璃继续走定制 D3D11（该材质插件源于用户自己的 PR） |
| 插件单元测试 | 6 个依赖 egui 0.31 API 与 spike 资源路径的测试模块不编译（不影响产品功能） |
| 文字缩放抖动方案 | 未采纳上游每帧重排；定制 `TextScale` 阻尼死区 + 量化方案保留 |
| 兼容渲染 60fps / DX12 读回 | 未采纳：属于架构切换前的 wgpu/eframe 旧栈优化，定制 D3D11＋WGC 栈无对应读回链 |
| 滚轮翻到矮页误收起 | 定制版已同源具备（`PAGE_EDGE_OVERLAP` 续滚带），无需合入 |
| IME 组字防误保存 | 定制版已有等价事件判断，天气/城市已按 egui 0.37 接入 |
| 通知 / 避让 / 更新、日历 UI | 保留定制：上游为函数级重构（如 `avoidance_for_window` 参数 HWND→RECT）且依赖 App 新字段，与定制行为互斥；已查社区无对应方案 |
