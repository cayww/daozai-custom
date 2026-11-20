# 岛仔：fence 与 device-lost 社区方案研究

> 2026-11-21 00:35，研究分支 `research/community-fence`（从 main 分出，不动 main）。
> 背景：零拷贝模式换分辨率触发 D3D11 device-lost（0x887A0005）重建风暴。
> 当前 main 的解法：设置项「切换分辨率/显示器模式后自动重启」（workaround）。
> 本文调研社区是否有更优雅的根治方案，重点是 fence。

---

## 一、先厘清一个关键误解：fence 解决什么问题

**fence 不是 device-lost 的解法**。fence 解决的是 **CPU/GPU 同步的正确性与效率**：
- 我们的现状：producer 用 `D3D11_QUERY_EVENT` + CPU spin（`GetData`）等 GPU 写完 Copy，再 ReleaseSync。CPU 空转、占主线程。
- fence 方案：producer 在 GPU 命令队列里 Signal(fence, n+1)，consumer 在 D3D12 submit 前 Queue.Wait(fence, n+1)。**CPU 不等待**，顺序由 GPU 保证。

device-lost 则是完全不同层面：DWM 在换分辨率时重置了 D3D11 device，旧 device 上的所有资源（纹理/fence/query/session）全部失效。fence 也一样会失效——见下面 Rive 的 crash 案例。

---

## 二、社区最相关项目：wgpu-scry（几乎和我们做的一样）

`wgpu-scry` / `scrying` crate（merely-made）：WebView2 + WGC + D3D11 共享纹理 → wgpu D3D12 导入，**跟我们的零拷贝链路几乎一模一样**。它的演进路线对我们极有参考价值：

### 2.1 fence：从 "future work" 升级到 "shipped"（已交付）

scrying 0.2.0 文档还把 explicit fence 列为 "future work"；到 0.4.x/0.5 设计文档，已经变成 **shipped contractual path**。标准做法（微软官方也这么写）：

1. **在 wgpu D3D12 device 上创建 fence**：`ID3D12Device::CreateFence(0, D3D12_FENCE_FLAG_SHARED)`
2. **导出 NT handle**：`ID3D12Device::CreateSharedHandle(&fence, ..., GENERIC_ALL)`
3. **producer 的 D3D11 打开**：`ID3D11Device5::OpenSharedFence(handle)`（需 Device5 + Context4）
4. **每帧**：producer `CopyResource` 到**新分配的**共享目标纹理后，在 D3D11 immediate context 上 `Signal(fence, n+1)`
5. **consumer**：render submit 前 `ID3D12CommandQueue::Wait(fence, n+1)`
6. fence value `n` 每帧单调递增

**关键设计点**（比我们当前实现强的地方）：
- **每帧用新的 destination texture**，frame 携带 `SyncMechanism::ExplicitFence` + monotonic `fence_value`。producer 不会覆盖 consumer 正在用的 in-flight sample——这正是 keyed-mutex 方案的隐患。
- host 创建并长期持有 `Dx12FenceSynchronizer`，通过 `WebView2CompositionConfig::with_dx12_fence_synchronizer` 传入，importer 必须用同一个 synchronizer；缺 synchronizer 时**拒绝构造 imported texture**（fail fast，不静默降级）。
- 保留旧的 barrier/cache fallback + "fallback invalidation escape hatch"（驱动行为不可靠时的兜底）。

### 2.2 但对 device-lost：scrying 文档也没有答案

深读它的 Windows companion doc（2026-05-11）——**明确没有** device-loss handling、frame pool recreation、stall watchdog、`force_restart_capture` 的实现细节。它只提到：
- `capture_metrics()`：统计收到的 WGC 帧、host 消费的帧、resize/restart churn 期间**丢弃的 stale dimension-mismatch 帧**
- "Add stale-frame / dim-match guards around resize and capture restart"
- `ScaleFactorChanged` 走同一个 renderer resize 路径；`--scale-test` 模拟缩放导致的物理捕获尺寸变化，每个尺寸重新 acquire/import 新 WGC 帧
- 固定 sRGB/Bgra8Unorm，P3/HDR 不支持

**结论**：连这个专门的库也没有公开的 device-lost 恢复秘方。它的健壮性来自"每帧新资源 + 尺寸校验 + 丢弃 stale 帧 + 可诊断 metrics"，不是某种神奇 API。

### 2.3 对我们最有价值的借鉴

| scrying 做法 | 我们现状 | 可借鉴? |
|---|---|---|
| explicit D3D12 fence（shipped） | event query + CPU spin，或污染链里试过 fence 但放弃 | ✅ 值得重做：能显著降低主线程占用 |
| 每帧新 destination texture + fence_value | 复用一张共享纹理 + keyed mutex | ⚠️ 大改，但能消除 in-flight 竞态 |
| stale/dim-match guard + drop | 有 PLATE 的 inner/ring 亮度判定，但无 dim-match | ✅ 可加：换分辨率期丢弃尺寸不符帧 |
| capture_metrics 可诊断 | 有 log，但无结构化 metrics | ✅ 可加 |

---

## 三、fence 的代码含义（微软官方 API）

- `ID3D11Fence::CreateSharedHandle`：fence 必须用 `D3D11_FENCE_FLAG_SHARED` / `D3D11_FENCE_FLAG_SHARED_CROSS_ADAPTER` 创建；`dwAccess` 只接受 `GENERIC_ALL`。
- `ID3D11Device5::OpenSharedFence(handle, REFIID)`：D3D11 侧打开共享 fence（只用于 fence；共享纹理走 `OpenSharedResource1`）。
- 跨适配器：如果 D3D11 producer 和 D3D12 consumer 不在同一块 GPU，需要 `SHARED_CROSS_ADAPTER` flag；我们绑定了 wgpu 同卡 LUID，同卡无需。

---

## 四、fence 在 device-lost 时的真实行为（反面案例）

Rive Flutter runtime（GitHub issue，2026-03）：D3D11 fence 路径在 sleep/wake、TDR、驱动重置后，GPU device removed，`ID3D11Fence::SetEventOnCompletion` / `ID3D11DeviceContext4::Signal` 返回 0x887A0005，代码里的 `VERIFY_OK`（失败即 abort）直接崩溃。

**教训**：
1. fence 调用在 device-lost 时照样失败——fence 不是 device-lost 的避风港。
2. 所有 fence 调用必须检查 HRESULT，失败时标记 deviceLost、退出 fence 线程、通知上层——不能像 Rive 那样 abort。
3. 触发场景很广：sleep/wake、TDR、驱动更新、远程桌面 attach/detach——我们的换分辨率只是其中一种。

---

## 五、综合结论与建议

### 关于 fence
- **值得做**，但定位是"把 event query CPU spin 换成 GPU fence 同步"，解决主线程占用/同步正确性，**不是解决 device-lost**。
- 污染链里的 `8b6c0e3` 已实现过一版（consumer 创建 fence → producer 打开 → Signal/Wait），当时因整体混乱未采纳；其代码可作参考，但要按 scrying 的 shipped 版本加固：每帧新纹理 + fence_value + fail-fast + fallback。
- 代价：~150-250 行（wgpu-hal escape hatch 拿 queue、Device5/Context4 plumbing、fence 追踪）。

### 关于 device-lost（换分辨率卡死）
- 社区（微软 + scrying + 多方）**没有"会话内完美恢复"的银弹**。微软开发者 robmikh 承认 WGC 内部有 device-lost 时机 bug；恢复的标准动作就是"新建 device + 重建资源 + frame pool.Recreate"，但我们实测重建后立刻又 lost。
- **scrying 的策略是"承认会断，快速重建 + 丢弃坏帧 + 可诊断"**，不是"永不中断"。
- 我们 main 上的"换分辨率自动重启"正是这个思路的极端形式（进程级重建），**实际是最可靠的**——因为它连 wgpu D3D12 device 一起重建了，这是会话内做不到的事。

### 建议优先级
1. **保持当前 main 的自动重启 workaround**（已验证方向正确），不要为了"优雅"退回会话内重建。
2. **可选增强 A（低风险）**：加 stale/dim-match guard——换分辨率期尺寸不符的帧直接丢弃，避免坏帧进 PLATE 造成花屏。
3. **可选增强 B（中风险，收益明确）**：把 event query CPU spin 换成显式 D3D12 fence，消除主线程空转；按 scrying shipped 版做（每帧新纹理 + fence_value + fail-fast + fallback）。这是独立的优化，与 device-lost 解耦。
4. **不建议**：继续在会话内 device-lost 恢复上堆叠机制（Recreate/冷却/退避），实测反复失败，且社区也没有更好的会话内方案。

---

## 参考来源
- 微软 Learn：ID3D11Fence::CreateSharedHandle / ID3D11Device5::OpenSharedFence
- 微软 GitHub issue `microsoft/Windows.UI.Composition-Win32-Samples#139`（robmikh：device removed 需新建 device + Recreate pool；承认内部 bug）
- scrying 0.2.0 docs.rs（future work: explicit fence sync；keyed-mutex + event query + 1x1 copy barrier 的旧路径）
- wgpu-scry design_docs/2026-05-07_platform_ceilings.md（explicit D3D12 fence shipped；每帧 fresh resource + fence_value；with_dx12_fence_synchronizer）
- wgpu-scry design_docs/2026-05-11_windows_webview2_target.md（capture_metrics、stale/dim-match guard、ScaleFactorChanged、--scale-test；无 device-loss 细节）
- Rive Flutter runtime issue #86（fence 在 device-lost 时返回 0x887A0005，VERIFY_OK abort 的教训）

---

## 六、追加调研：社区成功案例（回答"有没有人用新建 device + 重建成功过"）

**答案：有，而且是生产级——OBS Studio（数百万用户的直播软件）。** 我上一节"没有银弹"的结论过于悲观，需要更正：确实有成熟项目用这套方案稳定运行多年。

### 6.1 OBS Studio 的 device-lost 恢复（已验证成功，生产环境）

OBS 的 `libobs-winrt/winrt-capture.cpp` + `libobs-d3d11/d3d11-subsystem.cpp` 就是"新建 device + 重建所有资源 + 重建 frame pool"的完整实现，而且是**官方主线代码、用了多年**（最早 2016 年为 NVIDIA Win10 Anniversary Update 的 device removed 加的，见 git commit "libobs-d3d11: Rebuild device and assets if device removed/reset"）。

**关键架构**（这是它成功、我们失败的根本原因）：

1. **它有一个统一的 D3D device 所有者**：OBS 的 graphics subsystem（`gs_get_device_obj()`）持有唯一的 D3D11 device。device lost 时，**子系统统一重建 device**，然后遍历所有注册了 `gs_register_loss_callbacks` 的对象（包括 WGC capture），逐个回调重建。
   - 我们的项目：**wgpu(D3D12) 和 WGC(D3D11) 各有各的 device，没有统一所有者**。这正是进程内恢复失败的核心——device lost 时只重建了 WGC 的 D3D11，wgpu 的 D3D12 那侧没同步重建，共享纹理/fence 句柄两边对不上。

2. **回调两段式**：`device_loss_release`（撤销事件、Close frame_pool/session、清空所有 COM 引用）→ 子系统重建 device → `device_loss_rebuild`（用新 device 重建整条链）。

3. **rebuild 时重新创建 `GraphicsCaptureItem`**（不是复用）：通过保存的 `window`/`monitor` 调 `CreateForWindow`/`CreateForMonitor` 重新创建 item → 新 device 创建 IDirect3DDevice → `Direct3D11CaptureFramePool::Create` → `CreateCaptureSession` → 重新注册 FrameArrived → `StartCapture()`。
   - 这正是我之前推测但没确认的点：**item 在 device lost 后要重建**。我们的 Recreate 方案保留了旧 item，可能是帧不到达的原因之一。

4. **失败处理**：`StartCapture()` 用 try/catch 包住，失败则 `active` 保持 FALSE，靠下一轮重试；重建里有 `do...while(_canvasDevice == null)` 式重试（微软 ScreenCapturePlayer 示例），直到拿到可用 device。

5. **每帧 size 变化时调 `frame_pool.Recreate(device, format, 2, frame_content_size)`**（注意用的是 `frame.ContentSize`，且代码注释 `need GetDesc because ContentSize is not reliable`——实际用 `GetDesc` 拿的 desc.Width/Height 更可靠）。

### 6.2 微软官方 ScreenCapturePlayer 示例（也是这套）

微软 `windows-uwp` 仓库的 ScreenCapturePlayer：
```
ProcessFrame → catch IsDeviceLost → needsReset=true, recreateDevice=true
  → ResetFramePool(size, recreateDevice):
      do {
        if recreateDevice: _canvasDevice = new CanvasDevice()
        _framePool.Recreate(_canvasDevice, B8G8R8A8UIntNormalized, 2, size)
      } catch IsDeviceLost: _canvasDevice = null; recreateDevice = true
      while (_canvasDevice == null)
```
`do...while` **循环重试直到拿到可用 device**——不是重建一次就放弃。

### 6.3 为什么我们的实现失败了（逐条对照）

| OBS/微软成功要素 | 我们的实现 | 结果 |
|---|---|---|
| 统一 device 所有者，两侧一起重建 | wgpu D3D12 与 WGC D3D11 各自独立 | D3D11 重建了、D3D12 没跟上 → 共享纹理句柄失效（我一直怀疑的双 device 问题，被 OBS 架构佐证） |
| device lost 时**重建 GraphicsCaptureItem** | Recreate 保留旧 item | 旧 item 在 device lost 后可能失效 → 帧不到达 |
| `Release` 彻底 Close 旧 pool/session + 撤销事件 | 直接 drop，无显式 Close | COM 引用可能残留 |
| `do...while` 重试直到拿到可用 device | 重建一次失败就冷却/放弃 | 撞在 device 仍 removed 的窗口就永久卡住 |
| 新 device 用 try/catch 验证可用才 `active=TRUE` | 无验证 | 用着坏 device 继续失败 |

### 6.4 对项目的启示

1. **进程重启 workaround 之所以最可靠，本质就是"强制统一重建两侧 device"**——OBS 用架构做到了，我们用重启做到了。方向是对的。
2. **如果要在进程内做对**，需要模仿 OBS：让 wgpu 侧在检测到 D3D11 device lost 时也重建自己的 D3D12 device + 重新导入共享纹理 + 重建 item。这是很大的架构改动（wgpu device/queue/pipeline 全部要重建），风险高。
3. **低风险改进**（可考虑）：在现有自动重启前，先尝试一次 OBS 式的完整会话重建（Close 旧 pool/session → 重建 item → 新 device → Create pool → StartCapture），带 `do...while` 重试；失败再重启进程。这样多数情况下不用重启，只在真失败时重启。
4. 单纯 `frame_pool.Recreate(pool, 新device)` 而**保留旧 item/session**——社区成功案例里都不是这么用的（OBS 重建 item + 重建整个 pool/session；微软示例 Recreate 但配合 do-while 重试 + CanvasDevice 重建）。我们最初的 Recreate 方案只做了半套。

### 新增参考
- OBS Studio `libobs-winrt/winrt-capture.cpp`：`winrt_capture_device_loss_release` / `winrt_capture_device_loss_rebuild` / `gs_register_loss_callbacks` / `gs_get_device_obj`
- OBS Studio `libobs-d3d11/d3d11-subsystem.cpp`：2016  commit "Rebuild device and assets if device removed/reset"（生产验证多年）
- 微软 `windows-uwp` ScreenCapturePlayer：`ResetFramePool` 的 `do...while(_canvasDevice == null)` 重试
- gist `tor4kichi/cd76923d4b58bd4105d31aca0bf2d4d6`：同一模式的可运行示例

---

## 十七、逐行读 OBS 抓屏实现——结构性差异（新分支 research/obs-capture-study）

用户要求回退到 OBS 研究起点(4f764e8)、逐行翻阅 OBS 如何抓取。用 GitHub API 读完整
`libobs-winrt/winrt-capture.cpp`。**发现此前完全忽略的结构性差异**：

### OBS 的抓屏根本不用跨 API 共享机制
`on_frame_arrived` 全部逻辑：
```
frame = TryGetNextFrame()
frame_surface = GetDXGIInterfaceFromObject(frame.Surface())
frame_surface->GetDesc(&desc)   // 注释: ContentSize 不可靠，用 GetDesc
obs_enter_graphics()
if desc.Format == 期望:
  建/复用 OBS 自己的 gs_texture (gs_texture_create)
  context->CopyResource(gs_texture的obj, frame_surface)   // 直接 Copy，无同步原语
  texture_written = true
  if 尺寸变了: frame_pool.Recreate(device, format, 2, frame_content_size)
obs_leave_graphics()
```
`winrt_capture_render`: 直接用这个 gs_texture 画 (gs_effect + gs_draw_sprite)。

**OBS 没有：event query、keyed mutex、CPU spin、NT handle 跨进程共享、
D3D12 OpenSharedHandle、1x1 copy barrier。这些全是我们自己引入的复杂度。**

### OBS 用单一 graphics device
`d3d_device = gs_get_device_obj()` —— WGC 抓屏的 D3D11 device **就是 OBS 渲染器自己的
device**。所以：
- 没有"两个 device 同步"问题
- device-lost 时 graphics subsystem 统一重建 device + loss callback 通知 capture 重建
- 抓屏纹理和渲染纹理在同一个 device 上，CopyResource 直接可用，无需跨 API

### 对比我们的架构
我们：单独建 D3D11 device 给 WGC → 共享纹理(NT handle) → keyed mutex 同步 →
event query 等 GPU → wgpu D3D12 OpenSharedHandle → 1x1 copy barrier 刷 cache。
**这整套机制是 device-lost 的根源**——每个环节都可能在不同渲染后端/驱动下失配。

### 意义（待验证，不下结论）
我们一直在修这套共享机制(query/keyed mutex/poll/fence)，但 OBS 证明存在一条更简单的路：
让液态玻璃的抓屏也走"渲染器自己的 device"，消除跨 API 共享。但我们的渲染器是
egui-wgpu(D3D12)，WGC 只给 D3D11 texture——要统一到 D3D12 需要 WGC→D3D12 直接
路径(微软较新 API)或让 egui 走 D3D11 后端(影响大)。这是架构级决策。

另一个更轻的可能：OBS 的 CopyResource 后**完全不等 GPU**——它靠 gs_texture 的
渲染器内部同步(下一帧 render 时 GPU 自然串行)。我们可否也去掉所有显式同步，
靠 wgpu 的 queue 串行？这值得作为一个实验。

---

## 十八、OBS 方案能否移植——技术死结分析

逐行读完 OBS 后，核心问题：OBS 用**单一 D3D11 device**（gs_get_device_obj），
CopyResource 和 render 在同一 context、靠命令按序执行，无需任何跨 API 同步。
我们是 wgpu D3D12 渲染 + WGC D3D11 抓屏（两个 device），必须跨 API 共享。

### 三种跨 API 同步方案的可行性核查
1. **explicit D3D12 fence**（scrying shipped）：需要 wgpu 的 ID3D12CommandQueue 来
   `Queue::Wait(fence, n)`。**死结**：wgpu 24 的 `Queue.inner` 是 pub(crate)，
   `Queue` 无 `as_hal()`，wgpu-core 无 `queue_as_hal`，`Device::as_hal()` 只能拿 device
   不能拿 queue。污染链 fence 版失败的真正原因在此。
2. **keyed mutex**：D3D12 通过 OpenSharedHandle 打开的纹理**无法 QI IDXGIKeyedMutex**
   （scrying 明确说明）。D3D11 侧 Acquire/Release 对 D3D12 侧无效。
3. **CPU 等 GPU**（我们现状 event query spin）：等价方案是 fence + SetEventOnCompletion +
   WaitForSingleObject，但仍是 CPU 阻塞，且我们已证明 query 方式在此机器上失败。

### 结论性判断
OBS 的简单性来自"同 device"，而我们是"跨 device + wgpu 拿不到 queue"。
在这套架构下，跨 API 同步没有可靠的标准途径——这是 wgpu 24 抽象层的限制，
不是我们的代码 bug。这解释了为什么十几种改法（Recreate/冷却/OBS恢复/poll/query修复/
fence）都失败：都在试图用错误层的 API 解决。

### 可能的架构级出路（需用户决策）
A. 让液态玻璃渲染改用 D3D11 后端（与 WGC 同 device）——需换掉 egui-wgpu 的 backend，影响极大。
B. WGC 之后先 Copy 到 CPU staging，再 upload 给 wgpu（放弃零拷贝，回退 staging）——稳定但慢。
C. 升级/定制 wgpu（vendor wgpu-hal，暴露 queue）实现真 fence——工程量大。
D. 接受"换分辨率=进程自动重启"——main 现有方案，最可靠。

---

## 十九、【重要】A 方案（液态玻璃走 D3D11 后端）技术上不可行

用户选 A：让液态玻璃走 D3D11 后端与 WGC 同 device。动手前核查渲染栈，发现硬障碍：

1. **eframe 0.31 只支持 glow(OpenGL) 和 wgpu 两个后端**（Cargo.toml features 确认），
   没有 D3D11 渲染后端。
2. **wgpu 24 不支持 D3D11 后端**——它的后端是 DX12/Vulkan/Metal/GL（D3D12 是 Windows
   上的 DX12 实现）。`wgpu` crate 里没有也没法加 D3D11 backend。
3. 液态玻璃 `GlassCallback` 是 `egui_wgpu::CallbackTrait`，在 wgpu render pass 里画，
   依赖 wgpu 管线/纹理/bind_group。要改走 D3D11 等于重写整个玻璃渲染器 + 替换 eframe
   后端——后者 eframe 根本不提供。
4. 退一步用 glow(OpenGL) 后端与 D3D11 WGC 共享纹理需 WGL_NV_DX_interop（NVIDIA 专有），
   不通用且同样跨 API 同步问题。

**结论：A 方案在当前 eframe/wgpu 技术栈下不存在，无法实施。** 这不是工作量问题，
是后端根本不支持。已向用户说明并请求重新选择方向。

---

## 二十、社区 D3D11 渲染实现调研（用户要求，02:49）

用户要求先看社区有没有实现 D3D11 渲染的 egui 后端。**有，而且是活跃的**：

### 候选库
1. **egui-directx11**（nekomaru-pku/egui-directx11，活跃维护）
   - 最新版 0.13.0+egui-0.35（2026-07），MIT/Apache
   - **版本 0.7.0（2025-03）正好对应 egui 0.31.0**（我们是 egui 0.31 ✓）+ windows 0.60（我们 0.58，差一个小版本）
   - 0.5.0 对应 windows 0.58 + egui 0.28.1
   - 从 0.10.0 起 GA、无已知问题；支持 user texture（把 D3D11 纹理作为 egui 图片显示）
   - **但它是纯渲染器**：只渲染 egui 输出，window/device/swapchain/事件循环全部由调用方负责
2. egui-d3d11（jac3km4，已停止维护，egui 0.17/0.19，WIP）——不推荐

### 关键约束
- egui-directx11 只替换"渲染器"这一层，**不含 eframe 的 app hosting**。
  我们的项目用 eframe（窗口/事件循环/输入/渲染全包），要改走 D3D11 渲染器必须：
  1. 弃用 eframe，改用 winit + 自己搭事件循环（或保留 eframe 但替换其内部渲染——eframe 不开放这个）
  2. 把 GlassCallback 从 egui_wgpu::CallbackTrait 改写成 D3D11 渲染路径
  3. 把液态玻璃的 WGSL shader 全部改写成 HLSL（工程量巨大）
  4. windows crate 0.58→0.60 可能引发连锁 API 变更
- **WGSL→HLSL 重写**是最大障碍：整个液态玻璃的光学效果（Snell 折射、色散、Fresnel）都要重写。

### 结论
社区有 egui 0.31 的 D3D11 渲染器（egui-directx11 0.7.0），技术上"存在"，但要真正
替换 eframe + 重写玻璃 shader，工程量和风险都极大（相当于重写渲染栈）。
这不是"救活零拷贝"的轻量方案，是架构级重写。需用户权衡。
