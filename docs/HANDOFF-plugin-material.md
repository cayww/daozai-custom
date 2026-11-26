# 交接：插件生态融合 + D3D11 原生材质包接入（2026-11-26 15:44）

> 新 agent 接手先读这份。主交接文档是 `docs/HANDOFF.md`（2056 行），本文件是本次专项的增量交接。
> 当前分支：`merge-upstream-v0670`。工作树有**未提交改动**（见 §2），先看清楚再动手。

---

## 0. 30 秒速览

- 目标：把上游 v0.6.70 的插件生态完整接入定制版（D3D11 + 液态玻璃栈），并**只屏蔽液态玻璃这一个插件**，其他材质包要能正常用。
- 已完成：插件市场/安装/设置注入/JS 运行时/状态页/额度页/导航图标/外观图标/无线接收器/pinned 等绝大多数注入点（详见 §2）。
- **用户已确认走方案 B：D3D11 原生材质包**。理由：用户会提 PR 让材质插件作者改走 D3D11 架构，所以不做 WGSL→HLSL 转译层（方案 A），直接建 D3D11 材质 pass + HLSL/DXBC 材质包加载。**不要去做 Naga 转译**。
- 未完成：材质渲染接入（§4，5 个阶段）、appearance_picker + 底栏快捷切换（§4.4）、pinned 改动的验证提交、全量测试与打包（§5）。
- 现状包：`installer/daozai-setup-custom-isolated.exe`（12M，sha 前 16 `9ed7a3dfd3dc13c1`，commit `668e528`）——**这个包不含工作树里未提交的 pinned 改动**。

---

## 1. 红线与硬约束（务必先读）

1. **不要启用 `sdk-material` 的 wgpu 路径**。本地 Cargo.toml:53 明确写"The legacy wgpu/D3D12/WGSL stack is gone; rendering is D3D11 + winit only"。上游 `material/` 那 2459 行是死代码，`lib.rs` 里没有 `mod material` 声明——开启 `sdk-material` feature 会直接编译失败（3 个错误，见 §4.1）。
2. **不要恢复 wgpu/eframe 后端**。项目历史调研 `087b3d4` 和 `b440fee`（`docs/research-fence-device-lost.md` §19/§20）已确认：wgpu 24 无 D3D11 后端，eframe 无 D3D11 后端，此路不通。这是历史定论，别重复验证。
3. **液态玻璃是内置的，走内置 D3D11 HLSL 管线**（`shaders/glass_vs.bin` / `glass_ps.bin`，`d3d11/runtime.rs:create_glass_pipeline`）。只按 id `cayww.liquid-glass` 屏蔽那个插件，**不要屏蔽其他材质插件**——用户明确要求其他材质包能正常用。
4. **设置页 tab 编号已对齐上游**：13=外观、14=位置、15=系统性能、16=无线设备、17=插件、18=插件详情、**19=自定义与集成（定制版新增，排在关于之后）**。改 tab 编号会引发 `ui_tests.rs::every_module_settings_tab_maps_to_its_page` 失败，动前先看该测试。
5. **数据/安装目录隔离**：安装到 DaozaiCustom 隔离目录，数据在 `%APPDATA%\DaozaiCustom`（commit `8e4204a`）。别改回 Daozai。
6. **egui 用的是 git pin 0.35**（不是上游 0.31），windows crate 0.58，不要动依赖版本。

---

## 2. 当前状态：已完成 / 未提交 / 未验证

### 2.1 已提交（commit 序列，从旧到新）

| commit | 内容 |
|---|---|
| `c1751c7` | 修复手机电量 UTF-8 崩溃与设置页 tab 冲突 |
| `d190524` | 修复插件市场封面队列卡死与无线设备图标 |
| `1212205` | 封面优先读本地已安装记录，绕开 Gitee 限流 |
| `1bb7832` | 安装按钮不再依赖封面加载状态 |
| `8e4204a` | 数据目录与安装目录隔离为 DaozaiCustom |
| `f8bc05c` | 修 `PAGE_SHORTCUT_KEYS`（单字母→完整页面名，否则页面快捷切换装不上）+ 集市隐藏液态玻璃 |
| `89090e4` | 补 `header_tabs` 集市入口 + 插件独立设置页渲染 + manage 返回值导航 |
| `98a92ad` | 补设置页插件注入 + `SETTINGS_TAB_PAGES` 跳转 1→10 |
| `f92218c` | JS 运行时驱动（tick_features/take_script_notices/focus_finished/watch_files_enabled/pause_owner）+ quota/tasks/calendar 设置注入 + tab 编号对齐上游 |
| `668e528` | JS 状态页注入（status_pages/sync_status_pages/render_page/page_title/导航豆解耦）+ 额度页插件来源 |

### 2.2 ⚠️ 工作树未提交改动（3 文件，+119/-5）

`git status` 显示 `M dz-phone/src/app.rs` / `extras.rs` / `main.rs`。这些是本次会话最后阶段加的，**编译通过但没跑全量测试、没打包、没提交**：

- `app.rs`：pinned 字段（`pinned_displays`/`pinned_horizontal_width`）、`wireless_compat_store()`、`resident_fit()`、`appearance_pet`/`sync_canvas_fonts` 调用、pinned 同步与绘制、导航图标（`navigation_icon`）、状态页设置入口调用
- `extras.rs`：插件外观图标两处（`appearance_icon`）、`status_page_settings` 入口
- `main.rs`：`receiver_rx`/`receivers` 字段与 2.4G 接收器服务启停（`wireless_store`）

**接手第一步**：跑一次全量测试（命令见附录）确认这批改动没问题，通过就提交（建议 commit message：`merge: 补齐 pinned/外观图标/无线接收器注入`）。失败就修——最可能出问题的是 pinned 相关（`bar_layout.rs` 早就有 `append_pinned`/`pinned_slots`，但 App 层是这次才接的）。

### 2.3 最后一次全量测试结果

`668e528` 之后、**未提交改动之前**：`1521 passed / 0 failed / 138 ignored`。
未提交改动之后的全量测试被用户中断，未跑完。

---

## 3. 插件生态接入对照（已完成度）

对上游 0615f19 的 40 个插件方法调用，**本地已调用 34 个**。仍未调用的 6 个：

| 方法 | 状态 / 原因 |
|---|---|
| `activate_material` / `active_material` | 材质渲染未接（§4）——本次方案 B 要接 |
| `material_settings` | 同上 |
| `appearance_picker` | **未接**，用户已确认要接（§4.4） |
| `active_page_shortcut_colors` | **未接**，依赖底栏快捷切换整行（§4.4） |
| `pinned_reserved_width` | 架构差异，不需要接——本地用固定 960px 画布（`CANVAS_W = WIN_W + 320`），不动态扩交换链 |

其他 noteworthy：
- `wireless_store` 已接（`main.rs` 接收器服务）
- `pinned_displays` 已接但**未验证**（§2.2）
- `appearance_icon` / `appearance_pet` / `active_skin` 已接但未验证
- 底栏快捷切换整行**缺失**：`paint/mod.rs:1345` 的 `page_shortcut_icon` 是空实现，没有 `page-shortcut` 命中区、没有 `PAGE_SHORTCUT_ROW_H`；`show_page_shortcuts` 字段存在但无人读

---

## 4. 未完成的任务（按执行顺序）

### 4.1 阶段 0：先验证并提交 §2.2 的未提交改动
- 跑全量测试 → 通过则提交 → 打包一版保底
- 这一步不做，后面出问题无法定位是新改动还是旧改动

### 4.2 阶段 1：材质编译链（方案 B 的核心）
**目标**：能把材质插件的 shader 编译成 D3D11 可用的 DXBC（或加载预编译的）。

技术事实（已调研，别重复查）：
- 材质插件 entry 固定是 `material.wgsl`（`spike/local.rs:668`，测试见 `spike/local_tests.rs:957`）
- 示例材质 shader：`spike/examples/liquid-glass/material.wgsl`
- 材质 WGSL 用 prelude 提供的入口 `daozai_vs` / `daozai_fs`，插件只提供 `material(MaterialIn) -> vec4<f32>`（`material/prelude.wgsl:88-112`）
- **本地 D3D11 用预编译 `.bin`**（`d3d11/runtime.rs:459-465` 从 `include_bytes!("../../shaders/glass_ps.bin")` 创建 PixelShader），**没有运行时 HLSL 编译器**
- 系统有 `C:\Windows\System32\d3dcompiler_47.dll`，但 windows crate 当前 feature 没有 d3dcompiler 绑定（Cargo.toml:154 只有 `Win32_Graphics_Direct3D11` 等）
- 社区：Naga 支持 WGSL→HLSL（SM5.0+），但**用户已决定不做转译层**——等作者 PR 把材质包改成 HLSL/DXBC

要做的：
1. 加 `windows` crate 的 `Win32_Graphics_Direct3DCompiler`/`D3DCompileFromFile` 相关 feature，或用 `libloading` 动态加载 `d3dcompiler_47.dll`
2. 定义 D3D11 材质包格式：建议 manifest `entry` 指向 `.hlsl` 源码或 `.dxbc` 预编译文件，两者都支持最好
3. 写 shader 校验/编译测试：用一节测试 HLSL 验证能编出有效 DXBC
4. 修 `spike/market.rs:655` 的 `crate::material::validate_plugin_shader` 引用——开 feature 时会编译失败（当前在 `sdk-material` 关闭下走另一个分支所以没事）；方案 B 下应改成走新的 D3D11 校验函数

### 4.3 阶段 2：D3D11 材质 pass
**目标**：能把编译好的 DXBC 画到岛体上。

参考范式：`d3d11/runtime.rs` 的 `create_glass_pipeline`（459-480）+ `render`（617+）里玻璃 pass 的写法——同一个 device、WGC 抓屏纹理作输入、写 cbuffer、Draw(3)。

关键差异（必须做适配层）：
- 材质 WGSL 的 `Frame` uniform 有 13 个 vec4 字段（capture/body/shape/notch/env/tint/tex/count/ctl_rect×16/ctl_shape×16/ctl_spring×16/params×8/motion）——见 `material/prelude.wgsl:1-17`
- 本地 HLSL 玻璃用 `cbuffer U`（16 个 float4 + 控件数组）——见 `shaders/glass.hlsl:7-24`
- 两套接口**完全不同**，要写填充逻辑把材质 Frame 数据填进 D3D11 cbuffer
- 材质的 `sample_backdrop` 等辅助函数要在 HLSL 侧提供对应实现

建议：先用一个固定的测试 HLSL 材质（比如纯色/简单模糊）跑通 pass，再接插件数据。

### 4.4 阶段 3：插件激活/设置/Appearance + 快捷切换
1. 接 `activate_material` / `active_material` / `material_settings`（app.rs 的 `material_spec()` / `deactivate_material()`）
2. 接 `appearance_picker` 三处（设置 → 外观页的皮肤/图标/吉祥物行）：
   - 上游位置 `extras.rs:2191`（Skin）、`2217`（Icon）、`2246`（Pet，注意本地行号不同）
   - 本地外观页在 `extras.rs` 搜 `settings_label(ui,"岛体外观")`（约 5420 行）
   - 逻辑：装了外观包插件时，用 ComboBox 替换内置控件；没装时保持原样
3. 接底栏快捷切换整行（让 `active_page_shortcut_colors` 生效）：
   - 实现 `paint::page_shortcut_icon`（当前是空函数，`paint/mod.rs:1345`）
   - 加 `page-shortcut` 命中区、图标绘制、配色（上游 `app.rs:4090-4130`）
   - 加 `PAGE_SHORTCUT_ROW_H` 高度预留
   - 让 `show_page_shortcuts` 字段真正被读取
4. 与液态玻璃共存：材质包和内置玻璃**不能同时叠加**，要有互斥/优先级规则（建议：装了并激活材质包时停用内置玻璃 pass，反之亦然）

### 4.5 阶段 4：验证
- 全量测试（`1521 passed` 是基线，新增测试会增加）
- 实机验证：内置液态玻璃不受影响（截图对比）、材质包能显示、外观包能在设置里选
- 打包：`installer/daozai.iss` + Inno Setup 7

---

## 5. 常用命令

```bash
cd D:\code\daozai-custom
# 编译检查（默认 feature，sdk-material 关闭）
MSYS_NO_PATHCONV=1 cargo check --locked --manifest-path dz-phone/Cargo.toml --lib --bins

# 全量测试（约 100 秒，用隔离数据目录）
MSYS_NO_PATHCONV=1 DAOZAI_DATA_DIR=D:/code/daozai-custom/merge-work/test-data \
  cargo test --manifest-path dz-phone/Cargo.toml --locked --release --lib -- --test-threads=1

# release 构建
MSYS_NO_PATHCONV=1 cargo build --locked --release --manifest-path dz-phone/Cargo.toml \
  --bin daozai --bin daozai-bridge

# 打包（Inno Setup 7）
cd installer && MSYS_NO_PATHCONV=1 "/d/software/Inno Setup 7/ISCC.exe" daozai.iss
```

注意：Git Bash 下路径用 `/d/...` 或 `D:/...`，`MSYS_NO_PATHCONV=1` 防止路径被转换。
测试若失败先看 `ui_tests.rs::every_module_settings_tab_maps_to_its_page`（对 tab 编号敏感）。

---

## 6. 关键文件索引

| 文件 | 作用 |
|---|---|
| `dz-phone/src/d3d11/runtime.rs` | D3D11 渲染运行时，玻璃 pass 范式（459-480 创建、617+ 渲染、595 resize） |
| `dz-phone/shaders/glass.hlsl` / `.bin` | 内置液态玻璃 shader（已预编译，运行时不编译 HLSL） |
| `dz-phone/src/material/` | 上游材质模块死代码（2459 行，未接入 lib.rs，与上游逐字节一致） |
| `spike/examples/liquid-glass/material.wgsl` | 材质插件 shader 示例 |
| `spike/local.rs:455-457` | `is_material()` 判定（runtime == "material"） |
| `spike/market.rs:651-660` | `validate_material`，当前 sdk-material 关闭时拒绝所有材质插件 |
| `spike/local_ui.rs:888-892` | `visible_ids` 过滤材质插件（当前按 `is_material()` 全过滤，需改成只过滤液态玻璃 id——见 §7） |
| `dz-phone/src/app.rs` | App 主体，字段/frame/render_page/nav/pinned 都在这 |
| `dz-phone/src/bar_layout.rs:114-131` | `append_pinned` / `pinned_slots`（布局层早就就绪） |
| `docs/research-fence-device-lost.md` §19/§20 | wgpu/D3D11 后端调研历史（定论：此路不通） |

---

## 7. 一个待澄清的点（接手时注意）

`spike/local_ui.rs:888-892` 的 `visible_ids` 现在按 `!p.manifest.is_material()` 过滤——这会把**所有材质插件**从已安装列表隐藏，而用户要的是"只屏蔽液态玻璃 `cayww.liquid-glass`"。

但 §2.1 的 `f8bc05c` 又加了 `market_browse_ui.rs` 的集市列表过滤（按 id 精确过滤液态玻璃）。所以当前状态是：
- 集市列表：只隐藏液态玻璃 ✓（符合要求）
- 已安装列表：隐藏所有材质插件 ✗（需要改成只隐藏液态玻璃 id）

方案 B 接入材质包时必须改这里，否则材质包装上了也看不见。

另外 `spike/market.rs:651-660` 的 `validate_material` 在 sdk-material 关闭时**拒绝所有材质插件安装**——方案 B 下要改成 D3D11 校验（接受 HLSL/DXBC），否则材质包根本装不上。

---

## 8. 用户偏好（本次会话体现）

- 不要"一个个截图指出问题再补"，要系统对照上游排查（本次已做，见 §3）
- 不会的实现**先查社区**，社区方案优先
- 改动后**全量测试**（但本次用户中途多次要求"先不全量测试"，最终收尾要跑）
- 沟通要如实：不确定的、舍弃的、风险都要明说，不要含糊带过
- 液态玻璃是内置的、要保留；屏蔽的是 `cayww.liquid-glass` 这个插件，不是整个材质类

---

## 9. 本次续作进度（2026-11-26 16:xx）

### 9.1 已落地
| commit / 状态 | 内容 |
|---|---|
| `3cdeb76` | 提交 §2.2 的 pinned/外观图标/无线接收器注入；提交前全量测试 **1521 passed / 0 failed / 138 ignored** |
| 工作树（待提交） | D3D11 方案 B 主体 + 外观/快捷切换接入 |

### 9.2 方案 B 的实现形态（重要，接手先看这里）
- **没有启用 `sdk-material`**，没有 WGSL→HLSL 转译。新增 `spike/material_d3d11.rs`：
  - 材质包入口支持 `material.hlsl`（HLSL 源码，现场用 `d3dcompiler_47.dll` 编成 `ps_5_0` DXBC）或 `material.dxbc`（DXBC 二进制的 **base64 文本**，因为 `Package.source` 是 String）。
  - 宿主 prelude 提供 `cbuffer U(b0)`（与 `GlassParams` 完全一致）、`cbuffer P(b1)`（8×float4 = 32 个设置值）、`bg/samp`、全屏三角形 VS、遮罩 fs。**插件只实现 `float4 material(MaterialIn i)`**，不要重复声明 `U/P/bg/samp/vs/fs/MaterialIn`。
  - `MaterialIn` 字段：`uv`、`pixel`（窗口内物理像素，底原点）、`bounds`（xy=左下，zw=宽高）、`radius`、`values[8]`（设置值）。
  - helper：`dz_backdrop(px)` 采样岛体背后画面；`dz_rr_sdf` / `dz_island_sdf` 做岛体边界遮罩。
  - 入口 `.wgsl` 会给出明确错误"请改 HLSL/DXBC"，不再静默拒绝整个材质类。
- `Cargo.toml` 给 windows 依赖加了 `Win32_Graphics_Direct3D_Fxc`（只加 feature，不动 Cargo.lock）。
- `dz-phone/src/d3d11/runtime.rs`：`GlassPipeline` 增加 b1；`D3d11Runtime` 持有 `material_ps`，DXBC generation 变化时重建 PS，失败回退内置玻璃；材质与内置玻璃**互斥**，同一 pass 二选一。
- `spike/local.rs` / `local_ui.rs`：材质激活/权限/设置相关的 `cfg(feature="sdk-material")` 改成 `cfg(windows)`；`MaterialSpec` 增加 `entry`。
- `spike/market.rs`：`validate_material` 改走新校验；官方集市材质准入不再只放行 `cayww.liquid-glass@1.0.2`，改为按渠道信任（目录条目的 size/sha256 仍校验）。
- `spike/local_ui.rs`：`visible_ids` 只隐藏 `cayww.liquid-glass`，其他材质包在"已安装"里可见。
- `spike/local_ui.rs` 的 `manage()` 因材质 UI 解封新增第 5 个参数 `active_material`，所有调用点已补。

### 9.3 外观与底栏快捷切换
- 外观设置页接了 `appearance_picker`（皮肤/图标/吉祥物）和 `material_settings`（选择材质、权限确认、参数滑杆）。
- 底栏快捷切换整行补齐：`app.rs` 新增 `PAGE_SHORTCUT_ROW_H=24`、`page_shortcuts_enabled()`，`expanded_height()` 预留行高；`nav()` 绘制图标+页码点同排；`paint::page_shortcut_icon` 从空实现改成完整矢量图标（复用 `paint::icons` 的 `note/droplet/check`）。
- 与液态玻璃共存规则：激活材质包时 D3D11 pass 用材质 PS，否则用内置玻璃 PS；不会叠加。

### 9.4 验证记录
- `3cdeb76` 提交前：1521 passed / 0 failed / 138 ignored。
- 方案 B 改动后首次全量：**1523 passed / 1 failed**（新加的 HLSL 编译测试失败：HLSL 不允许在定义前调用 `material`）。
- 已修：把 prelude 拆成 `MATERIAL_PRELUDE` + `MATERIAL_EPILOGUE`，插件源码夹在中间。
- 修复后全量：**1524 passed / 0 failed / 138 ignored**（新增 3 个材质编译测试，`expanded_height` +24px 未打破布局断言）。

### 9.5 材质包最小示例（给插件作者/PR 用）
`plugin.json` 关键字段：
```json
{
  "id": "example.tint-glass", "runtime": "material", "entry": "material.hlsl",
  "owner": "appearance", "capabilities": ["appearance.material", "settings"],
  "permissions": {"screen.backdrop": true},
  "material": {"controls": true, "spring": {"stiffness": 1.0, "damping": 0.5}},
  "settings": [{"key":"tint","label":"着色","type":"number","min":0,"max":1,"step":0.05,"default":"0.5"}]
}
```
`material.hlsl`：
```hlsl
float4 material(MaterialIn i) {
    float3 bg = dz_backdrop(i.pixel);
    return float4(lerp(bg, float3(0.05,0.35,0.75), saturate(i.values[0].x)), 1.0);
}
```

### 9.6 待办（接手继续）
1. ~~跑完整全量测试~~ 已完成：1524 passed / 0 failed / 138 ignored。
2. 提交当前工作树。
3. 实机验证：内置玻璃不受影响；装一个 HLSL 材质包并激活，截图确认材质生效；外观包在设置里可选择；快捷切换行能显示和点击。
4. 打包并同步 `_archive/README.md` 指纹表、桌面 `HANDOFF.md` 与仓库副本。
