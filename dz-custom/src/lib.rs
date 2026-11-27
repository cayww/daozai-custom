//! 岛仔：Windows 顶部「灵动岛」。
//!
//! - `app` / `pages`：与平台无关的界面与动效（外壳与录制器共用）
//! - `anim` / `paint` / `shape` / `theme`：弹簧、矢量绘制、面板轮廓、配色字体
//! - `model` / `lyrics` / `smtc` / `icons`：数据、歌词、系统媒体、文件图标
//! - `native` / `tray`：仅 Windows 的窗口区域、命中穿透与托盘

#[cfg(feature="sdk-js")]
#[path="local_plugins_root.rs"]
pub(crate) mod local_plugins_root;
#[cfg(feature="sdk-js")]
pub use local_plugins_root::local as local_plugins;
#[cfg(feature="sdk-js")]
pub use local_plugins_root::local as sdk_spike;
pub mod display_layout;
pub mod diagnostics_tool;
pub mod rtl_text;
mod mail_privacy;
// 上游 v0.6.70 插件体系（集市/通知/状态插件）；GPU 材质插件（sdk-material）不启用，
// 液态玻璃仍走定制 D3D11 栈。
#[cfg(windows)]
mod plugin_notice;
#[cfg(windows)]
mod plugin_notice_settings;
#[cfg(windows)]
mod plugin_notice_ui;
#[cfg(feature = "sdk-js")]
mod quota_plugin;
#[cfg(feature = "sdk-js")]
mod plugin_canvas;
#[cfg(feature = "sdk-js")]
mod plugin_canvas_ui;
#[cfg(feature = "sdk-js")]
mod plugin_setting_catalog;
#[cfg(feature = "sdk-js")]
mod plugin_status;
#[cfg(feature = "sdk-js")]
mod plugin_status_ui;
pub mod poll_clock;
pub mod resources;
pub mod anim;
pub mod oled_shift;
pub mod app;
pub mod messenger;
pub mod child_process;
mod settings_window;
pub mod bt;
mod bluetooth_ui;
pub mod calendar;
mod calendar_ui;
mod local_calendar;
mod calendar_editor;
pub mod glass;
pub mod skin_paint;
pub mod icons;
pub mod input;
pub mod lyrics;
pub mod mail;
pub mod mail_format;
mod mail_reader;
pub mod mail_manage;
mod mail_manage_ui;
pub mod clipboard;
pub mod side_dock;
mod task_picker;
pub mod model;
pub mod island_status;
pub mod maa;
pub mod otp;
pub mod phone_battery;
pub mod quota;
mod quota_ui;
pub mod weather;
mod weather_ui;
pub mod wallpaper;
mod weather_source_ui;
mod qweather;
mod pages;
mod extras;
mod pet;
mod pet_expressions;
mod bar_layout;
mod resident_settings;
pub mod lunar_calendar;
pub mod resident_date;
pub mod performance;
mod performance_ui;
mod avoidance;
mod focus_bar;
#[cfg(windows)]
pub mod backdrop;
#[cfg(windows)]
pub mod composition;
#[cfg(windows)]
pub mod layered;
#[cfg(windows)]
pub mod system;
pub mod paint;
pub mod shape;
mod fluid;
#[cfg(windows)]
// egui 0.35 / D3D11 migration: WGSL/wgpu paint-callback replaced by HLSL layer.
pub mod gpu_glass_stub;
#[path = "gpu_glass_stub.rs"]
pub mod gpu_glass;
pub mod wgc;
#[cfg(windows)]
// D3D11 自绘栈（egui-directx11 移植到 windows 0.58）：WGC+HLSL 玻璃+egui+DComp
pub mod d3d11;
pub mod smtc;
pub mod music_volume;
pub mod theme;
pub mod settings_theme;
mod updates;
#[cfg(windows)]
pub use updates::record_install_size;

#[cfg(windows)]
pub mod bridge;
#[cfg(windows)]
pub mod native;
#[cfg(windows)]
pub mod wechat;
#[cfg(windows)]
pub mod tray;

#[cfg(test)]
mod ui_tests;
#[cfg(test)]
#[path = "bin/daozai-record/raster.rs"]
mod preview_raster;
