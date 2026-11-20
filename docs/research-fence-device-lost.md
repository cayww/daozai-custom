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
