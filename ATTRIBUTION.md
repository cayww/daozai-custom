# 来源与许可声明（Attribution Notice）

本仓库是对下列开源项目的二次开发，**不是官方项目**。此文件用于满足 MIT 协议的署名要求，并说明修改范围。

## 原始项目

| 项目 | 值 |
|---|---|
| 名称 | 岛仔 · Daozai（仓库名 `xiaotang-daozai`） |
| 地址 | <https://github.com/Renly1994/xiaotang-daozai> |
| 作者 | Renly1994 及 Daozai contributors |
| 版权 | Copyright (c) 2026 小唐的Token and Daozai contributors |
| 许可 | MIT License |

## 本仓库

| 项目 | 值 |
|---|---|
| 性质 | 基于上游 v0.6.63 源码的个人修改版（补丁后版本号 v0.6.64） |
| 与上游关系 | 无隶属关系，未获原作者背书 |
| 许可 | MIT License（与上游一致） |
| 修改部分版权 | 归各自的修改者 |

## MIT 协议合规说明

MIT 协议要求在软件的所有副本或实质性部分中保留版权声明与许可文本。本仓库的做法：

1. **完整保留** [`LICENSE`](LICENSE) 原文，包括原始版权行，未作任何改动。
2. 在 [`README.md`](README.md) 顶部与本文中**显著标注**上游项目地址、作者与版权信息。
3. 在 [`dz-phone/Cargo.toml`](dz-phone/Cargo.toml) 的 `license` 字段与 `repository` 字段保留上游信息。
4. 修改部分同样以 MIT 协议发布，任何人可继续按 MIT 条款使用、修改、再分发。

MIT 协议**不授予商标权**：项目名称「岛仔 / Daozai」、Logo 与图标归原作者所有，本仓库仅将它们用于如实指明来源，不表示原作者对本仓库的认可。

## 修改范围

本仓库相对上游的全部改动集中在补丁链 [`daozai-patches/apply.py`](daozai-patches/apply.py)（178 条替换），
逐条说明见 [`daozai-patches/PATCHES.md`](daozai-patches/PATCHES.md)。概括如下：

- 通知：手机连接通知只上岛；通知「重复查看」开关；独立「短信」设置分类
- 验证码：短信验证码自动复制 + 诊断日志；移除邮件验证码自动复制
- 歌词：左侧通道重排；超长歌词固定 11pt 横向滚动；通道留白调整
- 邮件：账号筛选；「全部已读」排队执行与线程异常保护
- 电量：本机电量与手机电量解耦，可并排显示；未装「手机连接」时锁定开关
- 状态总线：覆写式多槽位状态显示 + 示例脚本
- 手机 App 名单：配置化并支持热重载

## 第三方组件

本仓库包含的上游代码中引用了若干第三方开源组件与素材，其许可与署名见：

- [`dz-phone/THIRD_PARTY_NOTICES.txt`](dz-phone/THIRD_PARTY_NOTICES.txt) — 含 [fly-island](https://github.com/flyl1u/fly-island)（微信 / QQ 通知接入参考实现）等
- `dz-phone/assets/device-icons/LICENSE.txt`、`dz-phone/assets/settings-icons/LICENSE` — 图标素材许可
- `dz-phone/vendor/` — 5 个被 `[patch.crates-io]` 本地覆盖的依赖（egui / eframe / imap / imap-proto / ureq），各自保留原有许可

## 移除本声明？

如果你再分发本仓库的代码，请一并保留本文件与 `LICENSE`。这是 MIT 协议的要求，也是对上游客的基本尊重。
