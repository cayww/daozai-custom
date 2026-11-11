# 岛仔自定义补丁

基于 `Renly1994/xiaotang-daozai` v0.6.63，自定义版本号自动升到 v0.6.64。
所有改动都集中在 `apply.py`，可以重放到以后的官方版本，不必手工比对。

## 这次改了什么

| 文件 | 改动 |
|---|---|
| `src/phone_battery.rs`（新增） | 每 30 秒读一次手机连接写给开始菜单的 `StartMenuCompanion.json`，解析电量、设备名、连接状态；按电量升降推测充电状态，支持外部文件精确覆盖 |
| `src/app.rs` | 常驻电量槽位**只认手机电量**（不回退本机电量）；播放音乐/歌词接管胶囊时仍把电量环画在最左侧；`lyric_lane()` 把电量环、封面、宠物、歌词统一到胶囊左边缘基准，左侧固定通道互不重叠；绘制、布局、担忧表情、侧边停靠同步改用同一份数据 |
| `src/side_dock.rs` | 侧边停靠的电量绘制改用同一份数据 |
| `src/model.rs` | 新增设置项 `phone_battery`（默认开）与 `update_check`（默认关），写入 `settings.txt` |
| `src/extras.rs` | 「页面管理 → 状态组件」新增手机电量开关；「关于」页新增检查更新开关与自定义构建标识 |
| `src/updates.rs` | 检查更新入口受开关控制；更新弹窗显示「会覆盖自定义修改」的警告 |
| `src/lib.rs` | 注册新模块与 `CUSTOM_BUILD` 常量 |
| `Cargo.*` / `app.rc` / `CHANGELOG.md` | 版本号 +1，便于在关于页确认跑的是自定义版 |

## 数据源

```
%LOCALAPPDATA%\Packages\Microsoft.YourPhone_8wekyb3d8bbwe\LocalState\StartMenu\StartMenuCompanion.json
```

这是 Windows 开始菜单里「手机连接」小组件的 Adaptive Card。手机连接会把 `电池电量剩余 NN%` 写进去，实测几十秒刷新一次。本补丁只读这个文件，不联网、不调用手机连接私有接口。

判断规则：
- 卡片 `speak` 明确写「已连接到你的电脑 / connected」→ 显示电量，**不受文件刷新时间影响**（手机息屏时卡片可能长时间不刷新）
- 明确写「未连接 / disconnected」→ 立即收起电量
- 卡片没有连接信息时，才回退到「文件超过 2 小时没更新视为断开」
- 卡片里**没有充电字段**（实测 `charging` / `充电` / `plugged` 在整份卡片和 LocalState 里都是 0 次）→ 电量上升视为充电、下降视为未充电、持平保持上次状态；充电中若 20 分钟没再上涨且未满 100%，自动退出充电态，避免拔电后一直绿色
- 只认紧跟在「电池 / 剩余 / 电量」等关键词后的百分比，避免误读布局数字

## 充电状态能不能读到

**不能直接读到。** 手机连接写给开始菜单的 Adaptive Card 只有电量百分比、设备名、连接状态，
没有 `charging` / 充电中 / 电源接入等字段；整个 `LocalState` 扫下来也是 0 次命中，
卡片内嵌的电池图标也没有可解析的充电标识。所以给两层方案：

1. **推测（默认）**：电量升 1% → 变绿并显示闪电；降 1% → 恢复白色数字；持平保持上次状态；
   充电中若 20 分钟没再上涨且未满 100%，自动退出充电态，避免拔电后一直绿。
   程序重启会丢失推测状态，需等下一次电量变化。
2. **精确覆盖（可选）**：写 `%APPDATA%\Daozai\phone_charging.txt`，内容 `1`（或 `charging` / `充电`）
   表示充电中，`0`（或 `discharging` / `未充电`）表示未充电；删掉文件或写 `auto` 回到推测。
   覆盖态由外部负责，不参与超时回落；撤销后下一轮轮询（≤30 秒）就回到推测，不会残留绿色。
   手机端 Tasker / MacroDroid 在插拔充电器时同步该文件，就能得到真实充电状态，
   也方便以后对接自己写的脚本。

界面层无需改动：官方 `paint::battery()` 在 `charging=true` 时已经把整圈染成
`theme::CHARGE = RGB(33,235,84)`，并把百分比数字换成闪电图标。

## 设置位置

- 托盘 → 设置 → **页面管理** → 状态组件 → 「电量」（总开关，关闭后完全不显示电量）
- 同上 → 「手机电量」（关闭后同样不显示任何电量，**不会回退到本机电量**）
- 托盘 → 设置 → **关于** → 「检查更新」（默认关闭）

## 台式机 / 常驻语义

用户是台式机，没有本机电量，因此补丁**彻底去掉了 `env.battery` 回退**：
`resident_power` 只来自手机连接。手机断开、卸载手机连接或卡片显示未连接时，
电量槽位自动收起，不会显示一个假的 100% 或本机电量。

「常驻」指官方 `shared_header()` 里歌词完全淡入时会 `return` 整条跳过电量/时钟；
补丁在这条 `return` 之前单独把手机电量环画在胶囊最左侧。

custom-4 修的是**两套坐标基准错位**：官方把宠物放在「胶囊中心 + 60」，歌词放在
`left + content_width - 150`。`content_width` 会被专注模式（`lyric_focus_space()`）缩小，
也会被宽度动画改变，歌词就整体左移撞到宠物；而宠物表情贴图实际宽 48px（`cx±24`），
比槽位宽度 32px 更宽，右移 32px 的旧修法仍会压到歌词。
现在 `lyric_lane()` 把四者统一到胶囊左边缘 `left` 这一个基准（相对 `left` 的偏移）：

| 有手机电量 | 无手机电量 | 元素 |
|---|---|---|
| `left+20`（环） | — | 电量环 |
| `left+36..56` | `left+12..32` | 专辑封面 |
| `left+84`（贴图 `60..108`） | `left+60`（贴图 `36..84`） | 宠物 |
| `left+132` 起，通道宽 `content_width-162`，居中偏移最多 16px | `left+108` 起，通道宽 `content_width-138` | 歌词 |

宠物槽位用 `pet_center - width/2` 换算回「相对中心」的官方坐标，`width` 与歌词分支用的
`self.w.pos.max(content_width)` 完全一致，专注模式 / 宽度动画下两者同步移动，不会再错位。
歌词排版宽度由 `lyric_text_width()` 同步给出，`shared_header()` 再按实际 galley 宽度把文字在通道内居中，
左边界始终 ≥ 宠物贴图右缘 + 24px。

**间距微调结论**（用户问「需不需要缩小一点」）：先试过把硬余量 24→14，实测因为歌词
在通道内居中，通道左移 10px 同时变宽 10px，文字只左移 5px，几乎看不出变化 ——
视觉间隙的大头是**居中偏移**，不是硬余量。最终改法：硬余量保持 24px 不变，
给居中偏移加 `LANE_CENTER_CAP=16` 上限，间隙稳定在 24~40px；短歌词（「间奏」等）
不再被推到通道正中间，长歌词基本不受影响。

## 布局参数：手写 flex lane

项目是 egui 纯手绘，没有 CSS flex；胶囊头部叠了共享元素过渡、缩放与裁剪，也没有用
egui 的 `ui.horizontal()` 线性布局（换成布局引擎要重写整块，还会破坏 UI 测试）。
`lyric_lane()` 就是等价的手写实现：左侧 3 个固定宽项 + 右侧一块 `flex:1` 剩余区。

```
电量环(固定) → 封面(固定) → 宠物(固定) → 歌词(剩余区，文字居中) → 播放指示(固定)
```

间距集中在 `app.rs` 四个常量，以后加图标或调间距只改这里，不用翻绘制代码：

| 常量 | 值 | 含义 |
|---|---|---|
| `LANE_PET_HALF` | 24 | 宠物表情贴图半宽（实际 48px，比槽位 32px 宽） |
| `LANE_ART_GAP` | 4 | 封面右缘 → 宠物左缘 |
| `LANE_TEXT_GAP` | 24 | 宠物右缘 → 歌词通道的**硬余量**，保证不重叠（试过 14px，对居中布局几乎无效，已恢复） |
| `LANE_CENTER_CAP` | 16 | 歌词在通道内居中的最大偏移；控制「宠物到歌词」的视觉间隙为 24~40px |
| `LANE_RIGHT_PAD` | 30 | 歌词通道右侧留给播放指示 |

`content_width` 变化时 `lyric_lane()` / `lyric_text_width()` 与宠物槽位共用同一个
`width = self.w.pos.max(content_width)` 公式，因此专注模式、宽度动画下两侧同步伸缩。

**暂停音乐 + 长歌词的另一个坑**：`update_island_lyric_layout()` 原本设 `job.halign = Center`，
egui 对超长单行 galley 会算出**负的行内居中偏移**，把整行文字向左推出 `position.x`、压到封面上
（偏移发生在 galley 内部，只锁绘制起点没用）。release 改为 `Align::Min`（egui 没有 `Align::Left`），由 `shared_header()`
按 `galley.size().x` 手动居中；再用 `c.clipped(...)` 把绘制范围限制在通道内兜底。
测试构建仍用官方 `Center`，不影响 UI 测试。

**长歌词自动缩字号**（用户提的替代方案，已采用）：`update_island_lyric_layout()` 的
`layout(text, size, probe)` 现在带字号参数，release 从 11pt 起按
`[10.5, 10.0, 9.5, 9.0, 8.5, 8.0]` 逐级探测，选能单行放进通道（`lyric_text_width()`）的
最大字号，长歌词完整可读、不用分页也不硬截断。`probe=true` 时临时去掉 `max_rows`
以测自然排版；超长歌名 8pt 仍放不下才回到官方单行省略。测试构建固定 11pt 并保留官方
`max_rows`，UI 测试布局不变。`shared_header()` 按缩小后的 `galley.size().x` 重新居中，
裁剪兜底保留。短文本只排版一次，和官方开销相同。

`effective_minimal()` 触发的极简模式（全屏浏览器）仍沿用官方行为，不强制画电量。

关于「更新检查」：官方 0.6.63 本来就是**纯手动**检查，只有点「检查更新」才联网。
本补丁加的总闸关闭后，入口消失、不会联网、也不会弹安装窗口；打开后行为与官方一致。
即使打开，弹窗也会警告安装官方包会覆盖自定义修改。

## 测试与回归

```bash
cargo test --locked --release -j 2 --lib      # 全量：760 passed / 0 failed / 85 ignored
cargo test --locked --release -j 2 --lib -- phone_battery::   # 只跑新增模块：6 passed
```

两层测试隔离，缺一不可：

1. 新增模块默认开启，`poll()` 会读本机真实文件，破坏官方 UI 测试「无电池设备」的确定性前提
   （表现为 4 个布局 / 吉祥物断言失败）。所以 `card_path()` 在 `cfg(test)` 下返回 `None`。
2. 改成「只认手机电量、不回退本机电量」后，官方测试用 `h.power` 注入的电量又进不了
   `resident_power`，同样会让 4 个测试假失败。所以 `app.rs` 在 `cfg(test)` 下把
   `(env.battery, env.charging)` 当作手机电量回填，让测试保持可控。

3. `lyric_lane()` 在 `cfg!(test)` 下返回 `None`：宠物槽位回到官方 `60 - lyric_bar_width()/2`、
   歌词位置回到官方 `content_width - LYRIC_BAR_W/2`、排版宽度回到官方 220px，
   不影响 UI 测试里的宠物位置与歌词布局断言；release 构建才启用固定通道。

这几段都在 release 构建里被编译掉或保持官方值，实际行为不受影响。以后加任何会读本机环境、
会切断官方测试注入路径、或会改动官方槽位坐标的功能，都要补上对应的测试适配，否则 UI 测试会假失败。

## 以后官方发了新版本怎么办

在本目录打开 PowerShell：

```powershell
.\reapply.ps1                # 用 main 分支最新源码重放补丁、编译、替换
.\reapply.ps1 -Ref v0.6.70   # 也可以指定 tag
.\reapply.ps1 -Install "D:\other\daozai.exe"   # 自定义安装路径
```

它会：下载官方源码 → 应用补丁 → 编译 → 备份旧 exe → 替换 → 保留新源码目录。
执行前请从托盘完全退出岛仔。旧的 `daozai.exe.bak-<版本号>` 可以随时还原。

如果官方源文件结构变了，`apply.py` 会明确报出「哪一处在哪个文件失配」，不会静默产出半套补丁。
这时打开 `apply.py` 里对应那条 `EDITS` 记录，按新源码调整 `old` 片段即可。

## 想加自己的功能

1. 直接改 `dz-phone/src/` 里的代码，编译验证；
2. 稳定后把改动补进 `daozai-patches/apply.py`：
   - 新增文件放进本目录，在 `main()` 里加一行 `shutil.copyfile(...)`；
   - 修改已有文件就在 `EDITS` 里加一条 `(相对路径, 原文片段, 新片段)`；
3. 把 `PATCH_ID` 改个号（当前是 `custom-4`，下次可用 `custom-5`），下次 `reapply.ps1` 会对新源码应用全套改动。

`apply.py` 是幂等的：对同一份源码重复运行会跳过 `EDITS`；每处替换要求原文唯一匹配。
但**新增模块文件每次都会重新同步**，所以只改 `phone_battery.rs` 这类新增文件时，
在已打过补丁的源码上再跑一次 `apply.py` 也能把改动带过去，不必改 `PATCH_ID`。

## 注意

- 岛仔设置存在 `%APPDATA%\Daozai\settings.txt`，补丁不会清空原有设置；新增字段缺省值由 `Default` 提供。
- 手机连接必须处于已配对、能显示电量的状态；卸载手机连接或换账号后文件可能不存在，电量图标会自动消失。卡片明确写了断开文案时立即收起（见下方 custom-7）。
- 台式机没有本机电量：关闭「手机电量」后不会有任何电量显示，这是预期行为。
- 别用应用内「下载并安装」更新——那会用官方 exe 覆盖自定义版。要走 `reapply.ps1`。
- 编译需要 Rust（MSVC 工具链）与 VS Build Tools；`reapply.ps1` 会自动把 `~/.cargo/bin` 加进 PATH。

## 手机连接通知「只上岛」（custom-5）

目标：Phone Link 通知不再弹右下角，只在灵动岛显示。

**原理**：Windows 按通知通道（AUMID）分别控制横幅。`ShowBanner=0` 只关横幅，通知照常生成并进
通知中心；岛仔 `system.rs` 的 `GetNotificationsAsync(Toast)` 读的正是通知中心，所以不受影响。
当前主程序 `!App` 的 `ShowBanner` 已是 0，但 12 个 `YourPhoneNotifications_<包名>` 子通道
未设置、默认弹横幅，这才是右下角的来源。

**岛仔侧**：`system.rs` 内置白名单 `PHONE_CHANNELS`（短信、通话、设备状态、电话、微信、
QQ、支付宝、Gmail），只对这些 `Microsoft.YourPhone_*` AUMID 放行。
另外放行主 AUMID `Microsoft.YourPhone_*!App`（标签「连接」）作兜底：实测 WinRT 的
`AppInfo().AppUserModelId()` 有时只报主通道、拿不到子通道包名，纯按包名过滤会漏掉真实通知。
若发现主通道通知太多，可在 `notification-apps.json` 里把该条目 `enabled` 改成 `false`。三处 `qq_only` 过滤点
（读取层 `system.rs`、入口与 tick 的 `extras.rs`）以及 `is_wechat_toast` 都放行白名单通道；
其余应用仍受 `qq_only=1` 约束，不会刷屏，**不需要改用户任何设置**。
**App 级白名单靠正文首段**：真实通知的 `AppInfo` 只报主 AUMID `!App`，
按 AUMID 包名过滤的 `PHONE_CHANNELS` 实际上拿不到子通道，`notification-apps.json`
里也只会登记主通道一条。实测 Toast 正文格式是 `QQ · 小号 · 这是一条测试正文内容`，
首段就是来源 App 名，所以 `system.rs::phone_app()` 按首段识别：
QQ / 微信 / 支付宝 / Gmail / 短信 / 电话 / 验证码 → 精确标签「手机 · QQ」；
Steam / Discord / 12306 / 大麦 / BOSS直聘 / 明日方舟 → 静默；
**识别不到一律按「连接」放行**，避免漏掉来电等重要通知。
正文去掉首段 App 名（已在标题里），其余作一行 80 字内摘要；即使 `notification_preview=0`
也显示，且不对验证码打码（为后面的卡片留数据）。「手机 · 」前缀用来和岛仔自带 IMAP
邮件的「新邮件」（`app.rs` 的 `self.alert`，不经过 Windows 通知、也不写
`notification-apps.json`）一眼可分。
`NotificationApps::observe()` 会自动把新通道登记进
`%APPDATA%\Daozai\notification-apps.json` 并默认启用，可在设置里单独关某个 App。

**系统侧操作**（先单通道试，再批量）：

```powershell
$b='HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings'
# 单通道试验：Gmail（低频，可自己发邮件触发）
$k="$b\Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.google.Gmail"
New-ItemProperty -LiteralPath $k -Name ShowBanner -Value 0 -PropertyType DWord -Force
# 验证通过后批量关掉全部 17 个 Phone Link 通道（含来电/短信/设备状态）
Get-ChildItem -LiteralPath $b | Where-Object { $_.PSChildName -like 'Microsoft.YourPhone*' } | ForEach-Object {
  New-ItemProperty -LiteralPath $_.PSPath -Name ShowBanner -Value 0 -PropertyType DWord -Force }
# 批量回滚（主 !App 原本就是 0，用 -like '*YourPhone*' 手动排除）
Get-ChildItem -LiteralPath $b | Where-Object { $_.PSChildName -like 'Microsoft.YourPhone*!*' } | ForEach-Object {
  Remove-ItemProperty -LiteralPath $_.PSPath -Name ShowBanner -ErrorAction SilentlyContinue }
# 或只回滚单个通道
Remove-ItemProperty -LiteralPath $k -Name ShowBanner
```

**坑**：只能关 `ShowBanner`；千万别动 `Enabled` / `ShowInActionCenter`，否则通知不产生或
不进通知中心，岛仔就瞎了。关横幅可能仍留提示音/锁屏显示，要一并处理。新装手机 App 会
新建 AUMID 通道，需按前缀定时补漏。

**第二阶段待做**：验证码大号数字卡片 + 一键复制（复用 `otp::extract`）、同 App 多条聚合、
用现成 `app_icons` 图标、显示后 `RemoveNotification` 清通知中心、未接来电常驻（读 `calling.db`
的 `is_read`）、通用状态总线（D 盘应用 / ADB 共用一套槽位）。

**已验证通过**：17/17 通道 `ShowBanner=0` 后，QQ / 微信 / 来电 / 电话仍在横幅关闭状态下
产生通知并被岛仔显示，右下角不再弹 —— 「关横幅不影响 `GetNotificationsAsync`」这个前提成立。

## 第二阶段功能（custom-6）

**验证码卡片**：`draw_notice` 的 External 分支调用 `otp::extract("", &a.body, &[])`
（有 65 分门槛，普通消息不会误判），命中就画大号数字 + 「复制」按钮（`Cmd::Copy`），
不命中才画普通摘要。

**同 App 聚合**：`enqueue_alert` 的 `same_event` 对「手机 · 」标题只比较 source+title
（忽略 body），同一 App 的新消息替换旧提示、不排队；`receive_notification` 用
`phone_counts` 统计 10 秒窗口内条数，正文前缀「共 N 条 · 」。

**手机 App 图标**：`phone_icon_file()` 把「手机 · QQ / 微信 / 支付宝 / Gmail / 电话」
映射到手机连接自带的 `TempState\app_icons\*.jpg`，首次读盘后缓存 `TextureHandle`，
没有图标时回退铃铛。

**通知中心清理**：`system.rs` 的 `PHONE_CLEAR`（默认 true）在手机上岛后调用
`UserNotificationListener::RemoveNotification(number)` 从通知中心移除。岛仔未运行时
不会执行，通知仍留在通知中心；想保留副本把该常量改成 `false`。

**未接来电常驻**：电话类手机通知累计到 `missed_calls`，提示保留 30 秒并把铃铛红点设为常驻
（`notification_bell_until=f64::MAX`），点「关闭」清零。**没有引入 SQLite 依赖**，
未直接读 `calling.db` 的 `is_read`（要读需要新增 rusqlite，会改动 `Cargo.lock`）。

**通用状态总线**：写 `%APPDATA%\Daozai\island_status.json` 即可上岛：

```json
[{"id":"build","text":"构建中 45%","detail":"cargo build --release","seconds":0}]
```

`Reader` 每 2 秒轮询，同一 `id` 只推一次（从数组删掉再加回来会重新推），`seconds=0`
默认 8 秒。`push_status()` 转成 `AlertSource::External("island-status")`，`tick_alerts`
对该 id 单独放行（不受 `notification_apps` 白名单和 `qq_only` 限制）。ADB / 下载 /
构建状态都能通过这个文件接上，不需要再改 Rust。

## 断开连接后收起电量（custom-7）

**现象**：手机与电脑断开后，灵动岛仍一直显示断开前的电量数值。

**根因（两个叠加）**，都用真实的断开卡片 `StartMenuCompanion.json` 复现过：

1. 卡片里的中文是 JSON 的 `\uXXXX` 转义。已连接时 `speak` 是
   `Xiaomi 17 Pro Max - 已连接到你的电脑`，断开后变成
   `Xiaomi 17 Pro Max - 已从电脑断开连接`，并且**电量百分比整段消失**、
   变成「电池处于未知状态」。旧 `parse_linked()` 只认明文的「未连接 / disconnected」和
   `\u672A\u8FDE\u63A5`，对「已从电脑断开连接」完全无感，于是返回 `None`，
   落到「文件 2 小时内更新过就算可用」的兜底上，仍然判定为已连接。
2. 更致命的是 `poll()` 的顺序：旧代码先 `let Some(level) = parse_level(&text) else
   { return (self.level, self.charging); };` —— 断开后卡片没有电量数值，
   这一句**直接返回缓存的旧电量并提前退出**，连接状态判断根本没机会执行。

**修法**（只改新增模块 `phone_battery.rs`，EDITS 保持 43 处不变）：

- `decode_escapes()` 先把 `\uXXXX` 还原成真实字符，连接文案、电量、设备名都基于解码后的文本判断。
- `parse_linked()` 增加「断开连接 / 已断开 / 已从电脑断开 / not connected / offline」等断开词，
  断开优先于已连接；`card_usable()` 在没有连接文案时，还要求能读到电量数值且文件新鲜，
  否则视为断开（原来只看新鲜度）。
- `poll()` 调整为「先判连接态、清空缓存，再解析电量」；明确断开立即返回 `(None,false)`，
  连接正常但这一帧没写电量时保留 3 分钟宽限防止刷新闪烁。
- 轮询从固定 30 秒改为「10 秒定期 + 文件 mtime 变化时 1 秒内提前重读」，
  这样断开后 1 秒内收起、重新连上也能很快恢复；`Reader` 加 `last_modified` / `missing_since` 两个字段。
- `evaluate()` 与文件读取 / 节流解耦，新增 5 个单测，其中
  `运行中手机断开会清空已缓存电量` 直接复现用户场景（先缓存 80% → 断开 → 重新连上）。
  全量 `768 passed / 0 failed / 85 ignored`。

**验证**：干净官方源码重放 custom-7，43 处补丁命中、76 个 src 文件逐字节一致。
注意此修复只影响新增模块文件；`reapply.ps1` 每次都会重新同步模块副本，
所以升级官方版本时会自动带上。

## 覆写式状态总线（custom-8）

**目标**：让 `island_status.json` 真正成为「状态镜像」——文件里有什么岛上就有什么，
而不是一条 8 秒就消失的一次性通知。

**旧行为**：`Reader::poll()` 只推「新出现的 id」，`push_status()` 转成
`AlertSource::External("island-status")` 的一次性 Alert；同 id 内容不变就不再推，
8 秒后淡出，脚本想维持状态只能不停换 id 或删掉重加。

**新语义**：

- `poll()` 每 2 秒返回 `(当前存活的全部状态, 本次消失的 key)`；`parse()` 只在 JSON
  解析成功时更新 `seen`，脚本写文件写一半不会把岛上的状态全撤掉。
- `push_status()` 把状态 key 编进来源 `island-status:<key>`，每轮重推；
  `enqueue_alert` 的 `same_event` 对这类来源按 key 替换，不排队，顺带刷新 `at` = 续期。
- 条目删掉时 `drop_status()` 立刻从 `notice` / `alerts` / `hud_notice` 撤下，不用等 8 秒。
- `Alert` 新增 `status: Option<Status>`（全项目 6 个构造点都补了 `None`）；
  `draw_notice` 的 External 分支对状态独立绘制，手机通知 / 验证码卡片路径完全不变。
- 新字段：`progress`（0~1，卡片底部细进度条）、`color`（red/green/blue/amber/gray）、
  `click`（`copy:` / `url:` / `file:` 三种白名单动作，复用 `Cmd::Copy` / `OpenUrl` / `Open`，
  **不执行任意命令**）。`allowed` 与正文掩码放行 `island-status:` 前缀。

**不动的东西**：常驻区坐标、UI 布局、`Cargo.lock`、任何依赖。测试构建 `path()` 仍返回
`None`，所以 768 个官方 UI 测试的输入完全不变。

**验证**：`771 passed / 0 failed / 85 ignored`（新增 3 个状态总线单测：覆写/撤回/半写文件）；
干净官方源码重放 custom-8：56 处 EDITS 命中、76 个 src 文件逐字节一致。

### 脚本库 `daozai-patches/scripts/`

| 文件 | 用途 |
|---|---|
| `send_status.py` | 核心写入器 + CLI；`StatusBus` 带文件锁与原子替换，多脚本并发写不互相覆盖 |
| `send_status.ps1` | 同上，PowerShell 版（UTF-8 BOM，无需 Python） |
| `status_console.py` | 本地调试台，`py status_console.py` → `http://127.0.0.1:8765`，表单直接上岛/撤下 |
| `examples.py` | `demo` / `battery` / `adb` / `download` 四个零依赖数据源示例 |
| `phone_link_banner.ps1` | Phone Link 通道体检；默认只读，`-Fix` 才把新通道补 `ShowBanner=0` |

常用命令：

```powershell
py send_status.py set --id build --text "构建中 45%" --detail "cargo build" --progress 0.45 --color blue
py send_status.py set --id build --text "构建完成" --color green --click "copy:cargo build --release"
py send_status.py remove --id build
py examples.py battery --once
py status_console.py
.\phone_link_banner.ps1          # 只读体检
.\phone_link_banner.ps1 -Fix     # 补漏
```

脚本库全部零第三方依赖，且不参与 `apply.py` 的 EDITS，升级官方版本时原样保留。

**已知边界**：状态目前仍占 `notice` 单槽位（与通知共用），同时写多条状态只有最前面一条可见；
要「多个常驻槽位并列」得改常驻区坐标，属于下一轮 UI 改动，本轮未做。
`click` 只接受三种前缀，`file:` 建议只指向自己可控的路径。



## 手机 App 名单配置化（custom-12）

**目标**：新装手机 App 想上岛或静默时，不用改 Rust 重编译，直接改配置。

**做法**：`PHONE_APP_NAMES` / `PHONE_IGNORED` 两个硬编码常量删除，换成
`%APPDATA%\Daozai\phone-apps.json`：

```json
[
  {"name": "QQ",   "label": "QQ",   "allow": true},
  {"name": "小红书", "label": "小红书", "allow": true},
  {"name": "Steam", "label": "连接", "allow": false}
]
```

- `name` 匹配 Toast 正文首段（`phone_app()` 仍要求首段 ≤12 字）；`label` 是岛上显示的
  「手机 · {label}」；`allow=false` 表示静默。
- `model::PhoneApps` 负责加载 / 保存 / 查询：首次启动自动生成默认 19 条
  （12 个允许 + 7 个静默），文件缺失或解析失败时回落到内置名单，不会把通知全静默。
- **热重载**：`receive_notification` 每次进来检查文件 mtime，变化就重新读取，
  改完约 1 秒内生效，不用重启岛仔；测试构建完全不读盘（`cfg(test)` 返回内置名单）。
- 写入用 `atomic_write`，UTF-8 无 BOM；用记事本或 VS Code 直接编辑即可。

**验证**：`771 passed / 0 failed / 85 ignored`；干净源码重放 custom-12，65 处 EDITS、
76 个 src 文件逐字节一致。实机：往 JSON 加一条「小红书」后发模拟通知，岛上直接显示
「手机 · 小红书 / 有人回复了你的笔记」，全程未重新编译。

**边界**：设置页还没有可视化勾选（需改设置页布局，留作下一步）；名单按顺序匹配，
`name` 之间不要互相包含（例如同时配「微信」和「微信读书」时，短的那个要放后面）。


## 电量事件 / 验证码自动复制 / 设置页名单（custom-13）

三项一起做：

**1. 手机电量关键变化提示**（`extras.rs::phone_events`）
`app.rs` 记录上次电量 `phone_last: Option<Option<u8>>`（`None` = 尚未初始化），
每次轮询比较前后值，只在**跨越阈值**时提示一次：

| 变化 | 提示 |
|---|---|
| 有电量 → 无（断开） | 手机已断开 · 电量信息已收起 |
| 无 → 有电量（重连） | 手机已连接 N% · 手机连接已恢复 |
| >20% → ≤20% | 手机电量偏低 N% · 记得给手机充电 |
| <100% → 100% | 手机已充满 · 可以拔掉充电器了 |

首轮只记录不提示，避免每次启动弹一条「已连接」；`notifications` 关闭时全部静默。

**2. 邮件验证码自动复制**（`settings.auto_copy_otp`，默认关闭）
`app.rs` 的 `mail_event` 新邮件分支把 `item.code` 取出来，开关打开时
`pending.push(Cmd::Copy(code))` 直接写剪贴板；复用 `mail::code_of` → `otp::extract`
的 65 分门槛，不会把订单号当验证码复制。设置页「邮件提醒」分类里可开关。

**3. 设置页勾选手机 App 名单**
复用 `notification_apps` 的 `setting_row` 列表：显示 `label · name`（两者相同时只显示一个），
勾选写回 `phone-apps.json`；旁边一行提示「增删条目改 phone-apps.json」。
只加了一个 `ui.separator()` + 若干 `setting_row`，没有改动任何布局常量，771 个测试全部通过。

**验证**：`771 passed / 0 failed / 85 ignored`；干净源码重放 custom-13，76 处 EDITS、
76 个 src 文件逐字节一致。

**踩坑**：`apply.py` 里同一段路径在不同上下文需要的反斜杠层数不同——
Rust **字符串**里要 2 个（`\\` 写入后解析成 `\`），Rust **注释**里 1 个就够；
统一替换会同时改错，必须按行精确定位。


## 短信验证码自动复制 + 识别诊断（custom-14）

**问题**：custom-13 只把「自动复制」接在邮件路径（`mail_event`），手机短信验证码
上岛后不会写剪贴板；用户反馈「短信上岛了但没有自动复制」。

**修法**：

- `extras.rs::receive_notification` 在正文确定后加：`phone && auto_copy_otp` 时
  用 `otp::extract("", &body, &[])` 提取验证码，命中就 `pending.push(Cmd::Copy(code))`，
  由 `main.rs` 的 `Cmd::Copy` → `ctx.copy_text` 写剪贴板并提示「已复制」。
  与邮件共用同一个开关，不额外增加设置项。
- 新增 `extras::log_otp(source, code)`：只在开关开启时向
  `%APPDATA%\Daozai\diagnostics\otp.log` 追加一行 `时间 | mail/phone | 验证码或 -`。
  邮件和短信两条路径都写，这样「没复制」时能区分是**没收到**还是**识别失败**。
  超过 256KB 自动清空。

**验证**：`771 passed / 0 failed / 85 ignored`；干净源码重放 custom-14，79 处 EDITS、
76 个 src 文件逐字节一致。实机发一条「短信 · 10691234 · 【测试】您的验证码 834921」的
模拟通知：剪贴板变成 `834921`，`otp.log` 记录 `16:05:25 | phone | 834921` ✓。

**排查提示**：如果 `otp.log` 里出现 `mail | -`，说明邮件到了但 `otp::extract` 没达到
65 分门槛（正文里缺少「验证码/校验码」等强关键词，或数字前 20 字有「手机号/订单」等词）；
如果完全没有 mail 行，说明没有新邮件到达（历史邮件不会触发自动复制）。


## 移除邮件验证码自动复制（custom-15）

用户实测邮件验证码自动复制不生效（`otp.log` 里没有任何 mail 行，说明没有新邮件触发或识别未过门槛），
决定砍掉邮件那条，只保留短信。

- `app.rs` 的 `mail_event` 恢复原样（保留 `item.code` 的掩码显示与「新邮件」提示），
  删除 `let code=item.code.clone()` 和 `auto_copy_otp` 分支。
- 短信路径（`receive_notification`）完全不动，仍共用 `auto_copy_otp` 开关与 `otp.log`。
- 设置页文案改为「手机验证码自动复制 / 手机短信验证码上岛时直接写入剪贴板」。
- EDITS 79 → 77；`771 passed / 0 failed`；干净源码重放 77 处、76 个文件逐字节一致。
- 实机复测：模拟短信验证码 → 剪贴板 `662048`，日志 `phone | 662048` ✓。


## 状态总线多槽位并列（custom-16）

**目标**：状态不再只靠一次性的 notice 横幅，而是在常驻区并排显示（最多 3 条）。

**实现**：

- `bar_layout.rs`：`Layout` 新增 `status: Option<usize>` 与 `add_status(width)`。
  不动 `WIDTHS` 和 `resident_order` 的既有索引——状态槽位在 `x` 末尾动态追加，
  整体左移半个宽度保持其它组件相对位置；`Motion` 的弹簧数组会自动 `resize`。
- `app.rs`：`status_live: Vec<Status>` 保存存活状态；`status_item_width`（中文按 11px/字，
  夹 48~140）与 `status_total_width`（最多 3 条、条间 6px）；`Layout::new` 之后调 `add_status`。
- 绘制在 `shared_header` 末尾：圆角底 + 状态色圆点 + `fit` 截断文本 + 可选 1.5px 进度条。
- **横幅与常驻的配合**：只有「新 key 出现」才闪一次横幅，启动/清空后的首轮不闪，
  内容变化不重复闪；否则状态每 2 秒重推会把 `notice` 一直占住，常驻区永远不显示。

**踩坑**：`Reader::poll()` 有 2 秒节流，非检查帧返回空列表；把返回值直接写回 `status_live`
会让状态条只闪一帧就消失。改成「只有 `statuses` 或 `removed` 非空时才更新」。

**验证**：`771 passed / 0 failed / 85 ignored`；干净源码重放 custom-16，86 处 EDITS、
76 个 src 文件逐字节一致。实机写 3 条状态：胶囊右侧并排
「构建中 45%（蓝，带进度条）/ 下载 3 个（绿）/ 沪指 +1.2%（红）」，
左侧电量环、宠物、时钟位置正常。


## 歌词态验证码铃铛延长（custom-17）

**现象**：放歌（歌词态）+ 关闭正文预览时，验证码通知被压缩成一个带红点的铃铛；
铃铛只保留 4.5 秒，鼠标来不及移上去看。

**根因**：`compact_notices()` 在歌词态且 `notification_preview=0` 时，把所有外部通知压成铃铛；
`receive_notification` 又统一把 `notification_bell_until` 设为 `t + 4.5`。

**修法**：`receive_notification` 里对验证码类通知（`otp::extract` 命中）把铃铛保留 **90 秒**，
其余通知仍是 4.5 秒。**没有**强制展开横幅——避免打断歌词，保持「悬停查看」的现有交互。

**验证**：`771 passed / 0 failed / 85 ignored`；干净源码重放 custom-17，87 处 EDITS、
76 个 src 文件逐字节一致。

**已验证**：放歌时发一条模拟验证码，12 秒后铃铛仍在（旧逻辑 4.5 秒就消失），歌词不受影响。
注意面板展开时（`open=true`）本来就不画铃铛，这是官方设计。


## 通知「重复查看」开关（custom-18）

**现象**：放歌时鼠标移到铃铛上能展开一次，第二次移上去就不再展开。

**根因**：不是 `hover_lock`（鼠标离开会自动解锁），而是**通知 4.5 秒就过期**——
`tick_alerts` 把 `notice` 清掉后，第二次悬停已经没有内容可显示。

**修法**：新增设置项 `notice_keep`（设置页「通知 → 重复查看通知」，默认开启）。

| 模式 | 行为 |
|---|---|
| 开启（默认） | 普通通知保留 **60 秒**，期间可反复悬停查看；点「关闭」立即清除 |
| 关闭 | 维持官方行为，4.5 秒后消失（看一次就没了） |

验证码类通知不受此开关影响，仍单独保留 **90 秒**。

**测试隔离**：`cfg!(test)` 下仍用官方 4.5 秒，否则 5 个「通知时长」类 UI 断言会失败
（`app_notifications_show_temporary_bell_in_minimal_mode` 等）。

**验证**：`771 passed / 0 failed / 85 ignored`；干净源码重放 custom-18，94 处 EDITS、
76 个 src 文件逐字节一致。


## 设置页独立「短信」分类（custom-19）

**问题**：手机验证码自动复制的开关放在「邮箱」分类里，容易被误认为是邮件验证码功能。

**改动**：

- `titles` 新增第 13 项「短信」（索引 12）；`SETTINGS_GROUPS` 加 `("短信",12,12)`，
  排在「通知、邮箱与日程」之后，导航位置紧邻通知类设置；`titles[..].min(11)` 改为 `.min(12)`。
- 新分类内容：「手机短信」卡片 = 说明文字 + 验证码自动复制开关 + 一行指引
  （未接来电 / 通知保留时长 / 手机 App 名单在「通知」页）。
- 「邮箱」分类移除该开关，保留邮件自己的「验证码高亮」「验证码隐藏」。
- 分组测试 `settings_groups_keep_every_page_reachable` 同步加第 13 项。

**验证**：`771 passed / 0 failed / 85 ignored`；干净源码重放 custom-19，100 处 EDITS、
76 个 src 文件逐字节一致。


## A 组功能集：状态总线遥控 + 勿扰 + 剪贴板搜索（custom-28）

这一批是「便宜顺手」的九项，全部不新增页面、不改常驻槽位顺序，所以没有牵动那些
断言具体坐标的 UI 测试。

**1. 状态总线加 `priority` 与 `ttl`**

脚本以前只能「写进去就一直在」，进程被 kill 后状态条会永远挂在胶囊上；
三条槽位也没有优先级概念，先到先得。

- `priority`（整数，缺省 0）：解析后按它降序稳定排序，常驻区仍然只取前 3 条，
  于是「构建中」可以稳稳压过「下载完成」。
- `ttl`（Unix 秒，0 = 不过期）：解析时直接丢掉已过期的条目，脚本写一次就能自己消失。
- 老脚本不写这两个字段行为不变（缺省 0 / 不过期）。

**2. 状态总线可以反向指挥岛仔**

`click` 新增 `action:` 前缀，白名单只有五个词，不执行任意命令：

| 值 | 效果 |
|---|---|
| `action:focus_start` / `action:focus_stop` | 开始 / 暂停番茄专注 |
| `action:quiet30` / `action:quiet60` / `action:quiet_off` | 勿扰 30 / 60 分钟 / 关闭 |
| `action:page:5` | 跳到第 5 页（0 音乐 … 7 设置） |

原来 `copy:` / `url:` / `file:` 仍然交给外壳，新增的这几类由岛仔自己处理，
所以「构建脚本开始 → 自动进专注」「CI 失败 → 勿扰 30 分钟」这类联动不用再改代码。

**3. 勿扰**

设置「通知」页新增勿扰三档（关 / 30 / 60 / 120 分钟）。生效期间横幅不弹、铃铛不闪、
系统通知不发，但**通知历史照记、验证码照提醒**——只静音打扰，不丢信息。
到点自动解除并把设置项归零；只在本次运行内有效，重启后恢复关闭。

**4. 低电提醒可配置 + 本机电量也提醒**

原来手机低电阈值写死 `20`，本机既不提醒也不提示充满。

- 新增设置 `battery_alert`（关 / 10% / 20% / 30%，默认 20，手机与本机共用）。
- 新增 `pc_events`：本机跨阈值提醒一次、本机充满提醒一次；首轮只记录不提示，
  避免每次启动弹一条。手机原有的断开 / 重连 / 低电 / 充满逻辑不变，阈值改为读设置。

**5. 通知关键词高亮**

设置里一个开关，命中词表（默认 `验证码,面试,转账,取款,取件,欠费`，可在
`settings.txt` 的 `notice_keywords` 改）时，提示卡片最左侧亮一条琥珀色竖条。
只加一条竖条，不动文字和控件坐标，所以不影响既有断言。

**6. 剪贴板搜索**

置物架「剪贴板」分栏右上新增一个放大镜（钉在「清空」右侧那一小段固定位置，
不参与表头的动态排布）。点开后输入框浮在列表卡片上沿：比卡片亮一档、带描边，
聚焦时描边变蓝、光标闪烁；下方记录顺延一行显示（4 行变 3 行）。
Esc 或输入框右侧小叉收起并清空关键词。

两个必须注意的细节：

- 输入框**最后画**。放在卡片底色之前会被卡片整块盖掉（第一版就是这么消失的）。
- `is_editing()` 把「搜索打开」也算进去：点放大镜会请求窗口焦点，窗口被激活的一瞬间
  鼠标状态会丢，面板会立刻收起；算作编辑态后 `want` 条件里有 `is_editing() && open`，
  面板才会稳住。顺带把左右方向键让给光标，不再翻页。

过滤只作用于显示，索引仍指向原列表，复制 / 固定 / 删除的行为完全没变。
回归测试 `clipboard_search_toggles_on_the_active_tab_and_filters_lines`
覆盖：放大镜开合、关键词清空、历史条数不变、命中行留下 / 未命中行不再绘制。

**7. 邮件账号筛选持久化 + 回收站补选框**

- 选过的账号写进 `settings.txt` 的 `mail_filter`，重启后沿用；账号被删或停用时自动清空。
- 回收站页顶部补上同一个账号选框（原来只有收件箱有，但回收站其实也遵守筛选）。

**顺带修一个测试**：`ignored_mail_event_hides_stops_reminders_persists_and_restores`
用「现在 + 10 分钟」造日程、再用「今天」断言，23:5x 跑测试时那条日程已经落到明天，
必然假失败。改为按日程实际落点取日期，与零点无关。

**验证**：`781 passed / 0 failed / 86 ignored`（新增 3 个单测：状态总线优先级/ttl、剪贴板搜索）；
干净源码重放 custom-28，209 处 EDITS、跳过 0 处，src 逐字节一致。


## 电量解耦：本机电量与手机电量各自独立（custom-27）

**问题**：设置页「电量」的说明写的是「显示手机电量」，两个开关其实是同一件事的两半——
`phone_battery` 决定读不读手机电量、`show_battery` 决定画不画，关掉任一另一个就没意义。
定制版还把本机电量整个丢掉了（常驻槽位只认手机电量），台式机上「电量」等于空开关。

**改动**：

- `resident_power` 恢复为**本机电量**（`env.battery / env.charging`），不再被手机电量覆盖；
  新增 `phone_resident` 专门存「手机连接」上报的电量。
- 新增 `battery_rings() / battery_ring_count() / battery_visible()`：两个来源各自判断、互不影响，
  都开就**并排画两个环**（本机在左）。
- `bar_layout::widen_battery()`：双环时电池槽 +20px，其余组件自动让开。
- 歌词接管胶囊、侧边停靠（新增 `SideItem::PhoneBattery`）、常驻排列预览都改成双环 / 独立显示。
- 吉祥物「担忧」改为任一来源低电即触发。
- 设置页：「电量」改为「显示本机（电脑）的电量环；台式机没有电池时不显示」；
  「手机电量」改为「读取「手机连接」的电量，与本机电量各自独立、并排显示」。
- 新增 `phone_battery::installed()`：检查 `%LOCALAPPDATA%\Packages\Microsoft.YourPhone_8wekyb3d8bbwe`
  是否存在；不存在时「手机电量」渲染成置灰不可点的 `setting_row_locked`，
  提示「未检测到「手机连接」，安装后才能开启」。

**验证**：`778 passed / 0 failed / 86 ignored`（新增 2 个单测：开关互不影响、双环加宽不重叠）；
干净源码重放 custom-27，178 处 EDITS、逐字节一致；实机设置页两行文案已分开、开关正常。
本机是台式机没有电池，因此只显示手机环（65%），符合预期。

**踩坑**：`phone_battery.rs` 是补丁链**新增的模块文件**，由 `apply.py` 在 EDITS **之后**整文件复制，
不能写成 EDITS 条目（重放会 `FileNotFoundError`）。要改这个模块，直接改 `daozai-patches/phone_battery.rs`。

## 去掉歌词缩字号（custom-26）

**需求**：用户要求「删除缩小歌词的功能，不需要缩小，滚动就行了」。

**改动**：

- 删除 `SHRINK_SIZES` 候选表与 `fit_lyric_size()`，歌词固定 `LYRIC_BASE_SIZE = 11pt`。
- 单行放不下（`galley.rows.len() > 1`）直接换单行不折行 galley + 横向滚动，
  不再有「先缩到 8pt」这一步。
- `layout` 闭包去掉 `probe` 参数（它只用于量字号），歌名仍用官方单行省略。
- 删掉对应单测 `shrink_prefers_largest_single_line_size`。

**效果**：中等长度的句子（如 21 字）以前会缩到 8–9pt，现在保持 11pt 并滚动；短句不受影响。

**验证**：`776 passed / 0 failed / 86 ignored`；干净源码重放 custom-26，151 处 EDITS、逐字节一致；
实机注入 21 字句子，确认 11pt 原尺寸、句首停留约 1 秒后逐帧左移
（`提到…` → `到了一位…` → `到一位…`）。

## 歌词通道左右留白（custom-25）

**问题**：截图里歌词左边离宠物很远、右边直接贴上播放指示竖条，两头不平衡。

**根因**（按代码算，不是眼估）：

- 左：`LANE_TEXT_GAP=24` 是从宠物 **48px 布局盒**右缘算的，但表情字形比盒子窄约 19px，
  所以视觉留白变成约 39px。
- 右：通道右界在 `content_width-30`，而 5 根播放竖条以 `content_width-21` 为中心、
  向左占 7.8，左缘在 `content_width-28.8` —— 两者只剩 **1.2px**。

**改动**：`LANE_TEXT_GAP` 24 → 4、`LANE_RIGHT_PAD` 30 → 48。
实测左右留白从 39 / 3 变成约 16 / 21，两侧都有空隙。

**验证**：`777 passed / 0 failed / 86 ignored`；干净源码重放 custom-25，144 处 EDITS、逐字节一致；
实机注入 40 字长句确认滚动状态下文字与播放竖条之间仍有约一个字宽的空隙。

## 长歌词横向滚动（custom-24）

**问题**：歌词通道只有 138–210px 宽、胶囊 30px 高，长句放不下。原方案是 11pt 逐级缩到 8pt，
8pt 仍放不下就按播放进度**分页**——一句被切成几段轮播，跳变明显、读起来断断续续。

**改动**：

- 缩字仍是第一选择（11pt → 8pt，选能单行放下最大字号）。
- 8pt 仍放不下时**不再分页**，改为 11pt 单行 galley + 横向滚动：句首停 1 秒，
  速度按本句时长推导并夹在 22–90px/s（保证可读，也尽量在本句内滚完），到句尾停住。
- 换句复位；副歌同一句重复出现时也复位（滚动基准记的是「行号+文本」）。
- 歌名不滚动，仍用官方单行省略。
- 排版缓存：按（文本、通道宽度、颜色、是否歌名）缓存 galley，同一句只排一次。
- 滚动期间强制 30fps 重绘——**暂停时也要**，因为人正是这时候才想读完一整句。
- 测试构建保持官方 11pt + 分页，且**绕过缓存**：分页结果依赖播放进度，
  缓存会把上一段当成这一段的排版结果（这条是跑测试时才发现的，两个 UI 测试因此先挂了）。

**性能实测**（`cargo test --lib -- --ignored --nocapture bench_long_line`）：

| 场景 | 耗时 |
|---|---|
| 命中缓存 | 0.4µs |
| 未命中（真排版） | 4.8µs |
| 每帧 7 次（全部命中） | 1.0µs |

结论要修正一个直觉：**egui 自带 galley 缓存**，原来每帧 7 次 `layout_job` 几乎全是命中，
真正浪费只有约 0.02ms/s，可以忽略。所以「缩字 vs 滚动」每帧成本基本持平，
滚动真正的代价是**暂停时也要 30fps 重绘**，而不是排版。

**验证**：`777 passed / 0 failed / 86 ignored`（新增 4 个测试 + 1 个 ignored 基准）；
干净源码重放 custom-24，142 处 EDITS、76 个 src 文件逐字节一致；
实机注入 40 字长句，确认文字从「非常长」→「测试」→「滚动」→「到底」持续左移、
在句尾「显示出来」停住不截断；恢复真实歌词后短句仍居中完整。

## 账号下拉展开时的悬停焦点（custom-23）

**问题**：账号下拉展开后，鼠标移到菜单项上，高亮却还落在下面的邮件行上，菜单项本身没有任何反馈。

**根因**：邮件行的悬停高亮不是 egui 的交互态，而是每帧按鼠标坐标手绘的——

```rust
let hovered = ui.ctx().pointer_hover_pos().is_some_and(|p| rect.contains(p) && ui.clip_rect().contains(p));
if hovered { row.fill(...); }
```

它绕过了 egui 的交互判定，所以 `mail-account-dismiss` 遮罩只能挡住**点击**，挡不住这层手绘的 hover。

**改动**：

- 行悬停判定加 `!self.mail.manage.account_menu`：下拉展开期间列表行不再画高亮。
- 菜单项用自己的 `Response::hovered()` 画一块内缩圆角的浅灰背景（`from_gray(52)`），
  悬停反馈从「邮件行」回到「账号行」。

**验证**：`773 passed / 0 failed / 85 ignored`；干净源码重放 custom-23，127 处 EDITS、
76 个 src 文件逐字节一致。实机三态确认：菜单展开时鼠标在菜单项上 → 该行高亮、邮件行不高亮；
菜单展开时鼠标在列表上 → 邮件行也不高亮；关掉菜单后 → 邮件行悬停恢复正常。

## 邮件按账号筛选（custom-22）

**需求**：邮件列表把三个邮箱的邮件混在一起，看不出哪封是哪个账号的，想按账号分开看。

**改动**：

- 邮件页顶部「全部 / 未读」右侧新增**账号选框**，可选「全部账号」或任一已添加的邮箱。
  按钮上显示账号的本地部分（`@` 前面那段），太长的用 `fit` 自动缩字号。
- 列表、`全部 N` / `未读 N` 计数、验证码大卡都跟着筛选走；筛选不区分大小写。
- 点选框展开下拉（画在列表之上），第一项「全部账号」，当前选中项用蓝色标出；
  点列表外任何位置收起。
- 原来的「同步 / 仅本机」小字并入选框的悬停提示，位置让给选框。
- 账户被删除或改名时自动清掉失效的筛选，避免列表永远为空。

**实现细节**：`State` 加 `account_filter:Option<String>` 和 `account_menu:bool`；
判定走 `mail_account_match()`，计数与 `indices` 过滤统一调用它。

**验证**：`773 passed / 0 failed / 85 ignored`（新增 `account_filter_only_matches_selected_mailbox`）；
干净源码重放 custom-22，125 处 EDITS、76 个 src 文件逐字节一致。
实机：选 `2293212816@qq.com` 后列表只剩该账号邮件、`全部` 从 60 变 20；切回「全部账号」恢复。

**注意**：选框占位到 x=288，是因为官方测试 `mail_row_opens_summary_and_back_returns`
会点 `local(300, 50)` 这一片原来的空白区，选框再宽就会抢走这次点击。

## 「全部已读」点不动（custom-21）

**问题**：邮件面板右上角的「全部已读」点了完全没反应。

**根因**（用 `%APPDATA%\Daozai\diagnostics\mail.log` 抓到的实证）：

```
19:47:32 | read-all ready=true  data=true pending=0 accounts=3   ← 空闲时点击，正常执行
19:48:07 | read-all ready=false data=true pending=3 accounts=3   ← 同步中点击，被拒
19:48:10 | read-all ready=false data=true pending=1 accounts=3
```

按钮的可用条件是 `pending.is_empty() && 数据齐备`，而邮件每 60 秒会自动 Load 一轮，
三个账户在管理线程里是**串行**处理的，一轮常常要十几到几十秒。这段时间里按钮变灰，
旧代码是 `.clicked() && ready`——不满足就**静默丢弃**，所以看起来像「点了没反应」。
另外管理线程原本没有异常保护，一旦某次协议处理 panic，线程直接退出，之后所有请求都发不出去，
`pending` 永远清不掉，按钮就永久禁用。

**改动**：

- 管理线程用 `catch_unwind` 包住 `protocol::run`，单次异常回一个错误结果而不是让线程死掉。
- `pending` 增加起始时间 `pending_at`，超过 40 秒没有结果就自动复位，避免永久卡住。
- 账户增删/开关变化时顺带清理 `pending` 里已不存在的账户。
- 同步中点「全部已读」不再丢弃：记下 `read_all_queued`，等这一轮同步结束自动补执行，
  面板提示「正在同步邮件，完成后自动标记已读」。
- 数据本身没就绪时点击提示「邮件状态还没同步完，请稍后再试」。
- 每次点击写一行 `mail.log`（`ready / data / pending / accounts`），以后再遇到「点了没反应」可直接查。
- 新增单测 `stuck_pending_resets_after_timeout` 覆盖超时复位。

**验证**：`772 passed / 0 failed / 85 ignored`；干净源码重放 custom-21，116 处 EDITS、
76 个 src 文件逐字节一致；实机点击后底部显示「已将邮箱未读邮件标为已读」，未读清零、按钮变蓝。

## 邮件验证码文案明确化 + 短信页精简（custom-20）

**问题**：用户问邮箱页的「验证码高亮 / 隐藏」是不是短信功能。它们其实只作用于邮件页
（`mail_code` 用在 `pages.rs` 的邮件列表验证码卡片，`mail_code_hide_seconds` 用在
`mail_code_hidden`），短信验证码走通知横幅、不读这两项；但「验证码」三个字容易误会。

**改动**：

- 「验证码高亮」→「邮件验证码高亮」，描述补「邮件里」；「验证码隐藏」→「邮件验证码隐藏」。
- 同步 `ui_tests.rs` 的居中断言（`("验证码隐藏","60 秒")` → `("邮件验证码隐藏","60 秒")`），
  否则测试找不到旧文案会失败。
- 删除「短信」分类里那行「未接来电、通知保留时长与手机 App 名单在「通知」页」的指引文案
  （用户要求去掉，连同上面的分隔线）。

**验证**：`771 passed / 0 failed / 85 ignored`；干净源码重放 custom-20，103 处 EDITS、
76 个 src 文件逐字节一致。
