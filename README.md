<div align="center">

# 岛仔 · 二次开发版

**基于 [Renly1994/xiaotang-daozai](https://github.com/Renly1994/xiaotang-daozai) 的社区修改版**

[![License: MIT](https://img.shields.io/badge/License-MIT-3cbf91)](LICENSE)
[![Upstream](https://img.shields.io/badge/上游-Renly1994%2Fxiaotang--daozai-7955b8)](https://github.com/Renly1994/xiaotang-daozai)
[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011-0078D4)](#构建)

</div>

> ### 这不是官方项目
>
> 「岛仔 · Daozai」由 **[Renly1994](https://github.com/Renly1994/xiaotang-daozai)** 开发，
> 版权归原作者及贡献者所有。本仓库是基于其 **v0.6.63** 源码的个人修改版，**与原作者无关，也未获其背书**。
>
> 项目名称、图标与品牌归原作者所有，此处仅用于如实指明来源。
> 使用中遇到的问题请提到本仓库的 Issues，**不要反馈给上游**。
>
> 需要原版请前往官方仓库：<https://github.com/Renly1994/xiaotang-daozai>

---

## 这是什么

岛仔是一款 Windows 桌面伙伴：平时收在屏幕边缘，鼠标靠近就展开，用来查看消息、音乐、日程和 AI 额度。用 Rust + egui 构建，不依赖 Electron。

本仓库是它在**个人使用场景**下的一版深度定制：在原版基础上调整了通知、歌词、邮件和电量这几块的行为，并把每一处改动都固化成可重放的补丁链。

## 与上游的差异

| 模块 | 改动 |
|---|---|
| **通知** | 「手机连接」通知只上岛、不进通知中心；新增「重复查看」开关（普通通知保留 60 秒）；设置页拆出独立的「短信」分类 |
| **验证码** | 短信验证码自动复制到剪贴板并写 `otp.log` 诊断；移除邮件验证码的自动复制（只保留短信） |
| **歌词** | 手写 flex lane 重排左侧通道（电量环 / 封面 / 吉祥物 / 歌词互不重叠）；超长歌词固定 11pt **横向滚动**（不再缩字号）；修正通道左右留白 |
| **邮件** | 面板按邮箱账号筛选；「全部已读」在同步期间改为排队执行（原来会被静默丢弃）；邮件管理线程加异常保护与超时复位 |
| **电量** | 本机电量与「手机连接」手机电量**彻底解耦**：两个开关互不影响，同时开启时并排显示两个环；未安装「手机连接」时手机电量开关置灰 |
| **状态总线** | 覆写式状态显示：脚本写 JSON 即可让岛仔显示进度 / 状态色 / 点击动作，支持多槽位并列（最多 3 条） |
| **手机 App 名单** | 白名单配置化到 `phone-apps.json`，支持热重载 |

完整改动记录见 [`daozai-patches/PATCHES.md`](daozai-patches/PATCHES.md)，逐条版本说明见 [`dz-phone/CHANGELOG.md`](dz-phone/CHANGELOG.md)。

## 目录结构

```
.
├── dz-phone/               # 完整可构建源码（在上游基础上修改）
│   ├── src/                # 76 个源文件
│   ├── vendor/             # 5 个打过补丁的依赖（Cargo.toml 用 [patch.crates-io] 指向本地）
│   ├── assets/  docs/      # 资源与文档（来自上游）
│   └── Cargo.toml
└── daozai-patches/         # 补丁链
    ├── apply.py            # 全部 178 条改动，可从上游干净源码重放
    ├── PATCHES.md          # 每个补丁的来龙去脉
    ├── phone_battery.rs    # 新增模块（由 apply.py 整文件复制）
    ├── island_status.rs    # 新增模块
    └── scripts/            # 状态总线示例脚本
```

## 构建

需要 Rust 工具链（`cargo`）：

```bash
cd dz-phone
cargo build --locked --release --bin daozai
cargo test  --locked --release -j 2 --lib
```

产物在 `dz-phone/target/release/daozai.exe`。`--locked` 配合仓库内的 `vendor/` 可以离线构建。

## 从上游源码重放补丁

补丁链的作用是：上游发新版本时不必手工比对，直接把改动重新打上去。

```bash
python daozai-patches/apply.py <上游源码根目录>
```

它会按顺序执行 178 条替换，最后把 `phone_battery.rs` / `island_status.rs` 复制进 `src/`。

## 协议与致谢

本项目沿用上游的 **MIT License**，详见 [`LICENSE`](LICENSE)（原始版权声明未作任何改动）。

- 原始项目：**岛仔 Daozai** — <https://github.com/Renly1994/xiaotang-daozai>
- 原始版权：Copyright (c) 2026 小唐的Token and Daozai contributors
- 本仓库的修改部分同样以 MIT 协议发布，版权归各自的修改者

按照 MIT 协议要求，本仓库保留了完整的版权声明与许可文本，并在本文件与 [`ATTRIBUTION.md`](ATTRIBUTION.md) 中显著标注来源与修改说明。

第三方组件与素材的许可见 [`dz-phone/THIRD_PARTY_NOTICES.txt`](dz-phone/THIRD_PARTY_NOTICES.txt)（含 [fly-island](https://github.com/flyl1u/fly-island) 等参考实现）以及 `dz-phone/assets/*/LICENSE*`。

## 免责声明

本仓库为个人修改版，按「现状」提供，不附带任何明示或暗示的担保。使用前请自行评估风险；由此产生的任何后果由使用者承担。
