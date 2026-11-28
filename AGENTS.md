# 分支与环境

| 环境 | 对应 | 内容 |
|---|---|---|
| 线上 | `main` 上的版本标签（如 `v0.6.66`） | 用户正在用的正式版 |
| 预发布 | `main` | 已验收、准备进下一版的改动 |
| 测试 | `test/acceptance` | `main` 加上所有待验收的功能分支 |

- 确定无争议的小修复直接提交到 `main`。
- 新功能和较大改动在 `feat/…`、`fix/…` 分支开发，做完先合进 `test/acceptance`，不进 `main`。
- 功能验收通过后才合进 `main`；被否掉的从 `test/acceptance` 撤出，`main` 保持干净。
- `main` 有新提交后，把 `main` 合进 `test/acceptance`。合并冲突在 `test/acceptance` 上解决，不回写 `main`。
- 发版只在 `main` 上进行：改版本号、写 CHANGELOG 和公告、打标签。线上急修而 `main` 有不想带出去的改动时，从线上标签拉 `hotfix/…`，发版后合回 `main`。
- 待验收的功能分支列在 `test/acceptance` 的合并记录里；临时测试分支用完就删，不长期保留。

# 对话工作方式

用户只说要做什么，分支、工作树和出包由对话按本文件处理。

- 开工前先判断：确定无争议的小修复在 `main` 工作树改；新功能和较大改动新建 `feat/…` 或 `fix/…` 分支和对应工作树，已有对应分支的接着用。同一时间只由一个对话改 `main`，`main` 正被其他对话修改时改用 `fix/…` 分支。开工时告诉用户放在哪条分支。
- 改完跑相关的定向检查或测试，提交到所在分支，然后直接出包：把这次的改动合进 `test/acceptance`（`main` 的提交先同步进去），跑测试，编测试版并替换桌面测试版，完成后告诉用户改了什么、在哪条分支、可以看了。用户明确说不用出包时只提交。
- 用户说「出包给我看」而没有新改动时，把 `main` 和所有待验收分支的新提交合进 `test/acceptance` 后出包。
- 用户验收说「可以」时，把对应功能分支合进 `main`，删除分支和工作树；说「不要」时，从 `test/acceptance` 撤出该分支的合并，删除分支和工作树，撤出后重新出包。
- 一个对话只做一件事；用户中途提出不相关的事，另开对话处理。

# 邮箱与日历兼容

- 单个服务商反馈触发的修复，先判断能否覆盖使用同一协议的其他服务。通用的认证、连接、同步、解析和交互改进放在共享层，内置平台和自定义接入共用。需要选择行为时，优先依据服务端能力或账号配置。
- 平台专用逻辑用于默认配置、授权指引和已确认的服务端差异。验证兼容性修复时，覆盖触发问题的平台、同协议的自定义入口及已有连接方式。

# 插件设置

插件提供的设置在所属设置页的标题旁显示「插件」小标。插件开关开启用紫色，关闭用灰色。新增或修改这类设置时复用宿主的小标和开关组件，显示位置开关只调整对应的显示偏好。验收以所属设置页的实际显示为准。 具体见[插件 SDK 的设置规范](docs/plugins/sdk-v1-design.md)。

# 工作树

一条分支对应一个固定工作树：

| 分支 | 工作树 |
|---|---|
| `main` | `D:\A-TangHao\REA\Windows小工具\00-主工程\daozai` |
| `test/acceptance` | `D:\daozai-build\worktrees\acceptance` |
| `feat/…`、`fix/…` | `D:\A-TangHao\REA\Windows小工具\01-专项工程\<功能中文名>` |
| 临时复测、审 PR | `D:\daozai-build\tmp\<事由>` |

- 功能分支用 `feat/` 或 `fix/` 开头，名字说清功能，不带日期。
- 功能合进 `main` 后，删除分支和它的工作树。
- 复测旧版本、审 PR 等临时任务在 `tmp` 下建工作树，任务结束时删除；需要提交的临时分支用 `test/…`，一并删除。
- 工作树不建在 `target`、`D:\Temp` 或其他工作树里面。

# 发布打包（唯一入口）

发布资源**只允许**用 `installer\build-innosetup.ps1`（Inno Setup 7 / `installer\daozai.iss`）生成，不再维护任何 NSIS 打包路径。脚本默认从零构建；`-SkipBuild` 复用 `target\<profile>` 下已有产物；`-CargoProfile dev-release` 用于自测，正式发版用默认 `release`。

产物固定落在 `dist\`，名字与历史发布一致：`Daozai-Setup-v<version>-x64.exe`、`Daozai-v<version>-windows-x64.zip`、`SHA256SUMS.txt`。zip 必须保持 `Daozai-v<version>-windows-x64/daozai.exe` 的结构（`updates.rs::release_info` 依赖它校验更新包）。上传 Release 时同时上传安装器和 zip，不能只传安装器；不要上传 `daozai-setup-custom-isolated.exe` 这类中间产物。

CI 的打包步骤也只调用 `installer\build-innosetup.ps1`；不要恢复 `scripts\package.ps1`、`scripts\get-nsis.ps1`、`installer\setup.nsi` 或 `tests\native_installer.ps1`。

# 本机测试版交付

桌面「岛仔－当前测试版」只运行 `test/acceptance` 的编译结果，快捷方式指向测试分支工作树 `D:\daozai-build\worktrees\acceptance` 里的 `target/x86_64-pc-windows-msvc/release/daozai.exe`。任何会话都不用自己的分支或 `main` 直接覆盖测试版。

交付步骤：把改动合进 `test/acceptance`，在该工作树跑测试，再以 `DAOZAI_TEST_BUILD=1` 编译 release。交付前核对快捷方式目标和文件哈希；如果岛仔正在运行，切换到新程序并核对进程路径。后续交付沿用同一入口，不另建临时交付目录。
