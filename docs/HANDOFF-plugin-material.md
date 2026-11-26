# 上游 v0.6.70 对齐状态

> 更新：2026-11-27 00:00。基线 `v0.6.70`；分支 `merge-upstream-v0670`。
> 已提交：`9318765`（验证码两项）、`c6868f3`（其余对齐项）、`3372f12`（交接文档）。工作树干净。
> **最终全量测试：1545 passed / 0 failed / 138 ignored**（release，`--test-threads=1`；日志 `merge-work/full-test-20261127-final.log`）。

---

## 〇、2026-11-27 追加三项（用户要求）

1. **删除 ZCode 本地凭据路径**（用户判定无用）：删除 `zcode_quota.rs`、`quota.rs` 的 zcode 模块/add/fetch 分支改回上游 `ZCODE_RECONNECT`、`quota_ui.rs` 删除"连接本机 ZCode"按钮与连接方式单选、`dz-phone/README.md` 同步文案。GLM 额度解析不受影响。配套测试 `quota_acceptance_grouped_connections_preserve_saved_sources` 已按上游预期修正（ZCode 旧账号重连时 provider 变 Glm、source=cn）。
2. **恢复官方集市测试索引夹具**：`spike/market_state.rs` 的 `test_index` 字段/`set_official_test_index`/test 分支索引注入恢复为上游 `#[cfg(test)]` 形态。调用它的 spike 集市测试模块仍整批关闭，故当前无启用测试使用它；真正生效需重开整批测试，风险大，未做。
3. **设置侧栏可滚动（对齐上游截图）**：`extras.rs` 新增 `SETTINGS_BOTTOM_ROWS`、`settings_sidebar_builtin`；侧栏用 `egui::ScrollArea::vertical().id_salt("settings-sidebar-scroll").max_height(main_height)` 包住功能列表，插件/关于/自定义与集成固定在底部并加分隔线；新增回归测试 `settings_sidebar_scrolls_main_and_keeps_bottom_items_fixed`。

## 〇之二、构建加速（2026-11-27，提交 `43f09ba`）

- **新增 `[profile.dev-release]`**：继承 release（opt-level 3、行为一致），但 `lto=false` + `codegen-units=8`。测试命令改用 `--profile dev-release`，增量重编从约 6 分钟降到约 **1 分 18 秒**（实测），全量测试结果与 release 一致（1545 passed）。**正式包仍用 release，LTO 不变**。
- **启用 rust-lld**：`.cargo/config.toml` 加 `linker-flavor=lld-link` + 本机 `rust-lld.exe` 绝对路径（换机器需改路径，注释已写；删掉两条 linker flag 即回退 MSVC link.exe）。sccache 原本已配好且实测 release 命中率 100%，未改动。
- **新测试命令**：`cargo test --locked --profile dev-release --lib -- --test-threads=1`（不用 `--release`）。

## 一、已完成（代码）

| # | 项目 | 上游提交 | 本地落地 | 验证 |
|---|---|---|---|---|
| 1 | 邮件验证码大字横幅 | `b553ee2`/`1acc03a` | `AlertSource::MailCode`、`mail_code_popup`、避让时 270×60 大字卡、复制按钮、隐私（历史/通知无明文） | 已提交 `9318765`；4 个针对性测试通过 |
| 2 | 仅提醒验证码邮件开关 | `0cdf0f1` | `mail_code_only_notifications` 字段、配置读写、设置页行、通知过滤（跨账号） | 同上；回归测试通过 |
| 3 | 通知被任务栏遮挡点击修复 | `507b394` | `FrameOut.top_bar_notice` 通知命中区；`native::keep_above_top_bar(bounds, notice)` 按通知矩形抬升；主循环按停靠/开关传参；避开点击穿透窗口与无关弹窗 | `cargo check` 通过；顶栏相关 7 个测试通过；native 窗口测试通过 |
| 4 | 逐帧诊断细分 | `ea40714` 等 6 个 | `PREVIOUS_PARTS` 四段（前置/设置/其余/材质提醒）、前置 marks 细分、`requested_repaint_last_pass()` 纳入动画帧、过渡/交互起点/兼容标志 | `cargo check` 通过；未单独实机验证（仅诊断可观测性） |
| 5 | 集市/插件测试编译 | — | 复查确认 `__spike_tests_off__` 门控已不存在（`6308d85` 已重新启用），无需改动 | 全量测试已覆盖 |
| 6 | 翻页箭头锚点 | `0c216b6` 等 4 个 | 新增 `nav_arrow_y()`，`nav()` 与 `ctl_geom()` 共用同一锚点，修复 GPU 透镜与点击区错位 | `hold_open_pin_keeps_shelf_expanded_and_nav_arrows_match_the_lens` 通过 |
| 7 | 保持展开整套 | `1918f17` | `hold_open` + `page_height` 弹簧 + 图钉按钮 + `hold_slot` 动画 + 顶栏时钟 x/fit 插值 + 避让豁免 + 过渡标记 + 音量浮条让位 + 置物架/剪贴板 header 左移 | 全量测试通过 |

### 本地适配说明（非上游原样）
- 验证码铃铛时长保留本地 `otp_seconds` 定制（上游是固定 6s），测试里用 `h.t += 5` 先让它过期。
- `interactive_frame_start` 本地 pinned egui 0.37 没有，改用 `out.animating` + `requested_repaint_last_pass()` + `render_compat_running` 表达。
- `native::canvas_rect()` 本地没有 `crop/paint_offset`，直接取 `GetWindowRect(self.hwnd)`。
- 顶栏时钟 fit 只在置物架/日程/天气/额度这类「窄页」启用动画；其它页即时布局，否则电量开关后单帧内动画不收敛（曾导致 1 个测试失败）。
- 图钉只移植了 `hold_pin` 绘制与图钉交互；`actions = header.shifted(-header_music_shift())` 目前只用在置物架页，其它页右侧操作仍用原 header（功能可用性不受影响，视觉上可能与图钉略挤，待实机确认）。
- 性能页 `PERFORMANCE_PAGE => 190` 锚点分支：上游最终版 `nav()` 已统一为固定 `anchor_y=68`，本地沿用该做法，不再单独设 190。

---

## 二、未完成 / 有意不合

### 有意保留的定制差异（未按上游改动）
1. ~~**上游删除 ZCode 本地凭据路径**（`9b539fe`）~~ **已按上游删除**（2026-11-27）：删除 `zcode_quota.rs` 与"连接本机 ZCode"入口，ZCode 账号改为 GLM Key 重连提示；GLM 额度解析不受影响。
2. **集市材质包白名单**（`ae57460`）：上游只放行固定 sha256 的 `cayww.liquid-glass` 1.0.2，本地放宽为所有校验通过的包（见 §7.0zzze「只按 id 屏蔽 `cayww.liquid-glass`」）。**已按用户要求保留放宽**（本轮未改）。
3. **喝水页逐帧交付**（`ed035e7`）：D3D11 的 `transitioning` 已含 `water_view_t`，上游 `FrameOut.water_view_transition` 只服务 DX12 读回，有意不合。
4. **DX12/WGSL 材质渲染链**：本地用 D3D11 自绘栈替代，架构定制。
5. ~~**官方集市测试索引夹具**~~ **已恢复为上游形态**（2026-11-27）：`spike/market_state.rs` 的 `test_index` 字段/`set_official_test_index`/test 分支索引注入已改回 `#[cfg(test)]`。注意：调用它的 spike 集市测试模块仍整批关闭（`__spike_tests_off__`），所以夹具当前不被任何启用测试使用；真正生效需连同 `market.rs`/`local.rs` 的测试模块一起重开，风险较大，本轮未做。
6. **手机连接短信/来电转发**：按你此前要求删除，仅保留验证码卡片。

### 尚未做的收尾
- **提交**：工作树 8 个文件尚未提交（本轮收尾待办）。
- **文档同步**：桌面 `HANDOFF.md` / 仓库副本、CHANGELOG、WorkBuddy 工作日志尚未更新。
- **打包/部署**：本轮未打包；按惯例等你确认后再打包部署。

---

## 三、提交记录

```
3372f12 docs: 记录上游 v0.6.70 对齐收尾（§7.0zzzm）
c6868f3 feat: 合入顶栏遮挡点击修复、逐帧诊断与保持展开，统一翻页箭头锚点
9318765 feat: 合入避让验证码大字横幅与仅提醒验证码开关
```

工作树干净。

---

## 四、说明

- 上述状态以 `v0.6.70` 为基线，按上游 commit diff 对照当前代码得出。
- 未列项为已对齐或无需对齐的定制差异。
- 测试命令必须 `--test-threads=1`；Release 全量编译约 6 分钟。
