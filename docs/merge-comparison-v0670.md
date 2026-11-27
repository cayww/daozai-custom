# 岛仔 v0.6.70 上游合并对照表（最终结果）

> 分支：`merge-upstream-v0670`；定制基线 `017108d`，上游 `0615f19`（tag v0.6.70），区间 418 commit
> 最终代码提交 `44c8faf`；release 构建通过；全量 **1520 passed / 0 failed / 138 ignored**
> 测试包：`daozai-setup-merge-v0670e.exe`（sha 前 16 位 `cf8df9b6d77625ab`）；未部署、未推 origin
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
| 主界面 11 页 | 9 页→11 页（新增系统性能、无线设备；默认隐藏、可在页面管理开启），9→10→11 页配置迁移 |
| 无线设备页 | 接入 `page_wireless`，Settings 加蓝牙常驻/电量偏好；手机电量迁入此页 |
| 通知 / 避让 / 更新 | 采用上游 system/avoidance/updates；保留本地手机通道、文件选择；更新源保持 cayww/daozai-custom |
| 日历 UI | 采用上游 calendar_ui/calendar_editor/task_picker（含按日期浏览、滚动），egui 0.37 API 适配 |
| 插件市场 | 挂载完整 LocalPlugins；设置页「插件」入口 + 插件管理界面；**GPU 材质插件显式屏蔽**，液态玻璃为项目自维护 D3D11 实现 |
| 手机电量 UI | 删除设置页开关与岛仔电量环；连接状态不再显示电量，改由无线设备页展示 |
| 性能页接入 | 主界面扩为 10 页（系统性能，默认隐藏、可在页面管理开启）；App 接 `performance::Monitor` + tick + 绘制；Settings 加 `show_performance`/`performance_view` 及配置迁移 |
| 翻页箭头 | 按 `Dock` 计算锚点（Bottom 用剪裁区底边、侧边用高度弹簧补偿），沿用定制玻璃/弹簧绘制 |

---

## 二、合入（上游新增，直接纳入）

| 模块 | 内容 | 关键文件 / 目录 |
|---|---|---|
| 插件体系 | 插件画布、通知/状态插件、设置目录、已安装管理、额度插件 | `src/plugin_canvas*`、`plugin_notice*`、`plugin_status*`、`quota_plugin*`、`plugin_setting_catalog.rs` |
| 插件 SDK 资源 | JS/WASM SDK 源码与示例（GPU 材质 SDK 属已退役的 wgpu/WGSL 栈，未启用；2026-11-27 已删除） | `spike/`、`docs/plugins/` |
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

## 三、未融合 / 未合并

无。你列出的项已全部融合或启用。

仍按定制侧保留（非「未融合」，而是刻意保留的定制实现，无需上游版本）：

| 项 | 说明 |
|---|---|
| 液态玻璃 / GPU 材质 | 本地 D3D11 自绘栈（`glass.rs`/`fluid.rs`/`gpu_glass`/`d3d11/`）保留；上游 `src/material/`（wgpu/WGSL GPU 材质）从未接入编译，2026-11-27 已删除；插件市场里的 GPU 材质插件已显式屏蔽，不会接管外观 |
| egui 0.37 pin / 版本 1.3.0 / ZCode | 定制核心，保留 |
| vendor/* 64 文件 | egui 0.37 pin 与 D3D11 补丁，保留 |
| 上游 9 个不兼容单测模块 | 依赖 egui 0.31 API（`ctx.run`）与 spike 资源路径，不编译；不影响产品功能 |
| 部分设置页图集基准测试 | 新增性能/插件设置页无历史像素基准，跳过严格比对 |
