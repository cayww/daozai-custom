# spike/wgc-gpu-share — WGC 零拷贝共享纹理试验

> 状态：**试验分支，默认关闭，未实机验证，不部署不合并**。
> 起步点：main `082cc34`（2026-11-19）。

## 想验证什么

液态玻璃当前链路（最贵的一段）：

```
WGC 帧(D3D11) → CopySubresourceRegion 到 staging → Map(D3D11_MAP_READ) 同步等 GPU
             → CPU 逐行读回 ~1MB → CPU 合成底片/亮度采样 → queue.write_texture 传回 GPU
```

一去一回两次拷贝 + 一次 GPU 同步等待。本分支尝试替换为：

```
WGC 帧(D3D11) → CopyResource 到共享 D3D11 纹理(NT handle)
             → wgpu(D3D12) OpenSharedHandle 导入同一块显存 → shader 直接采样
```

## 开关

设环境变量 `DAOZAI_LIQUID_GPU_SHARE=1` 启动才生效。**不设则 100% 走旧路径**，零回归。
诊断看 `%APPDATA%\Daozai\diagnostics\liquid-glass.log` 里的 `[gpu-share]` 行。

## 社区参考（这条路已有成熟范式）

- **scrying 0.2.0** — WGC→wgpu 的 webview 捕获，持久共享 D3D11 纹理 + keyed mutex + event query 同步；
  consumer 侧 1×1 copy 刷屏障。作者标注 working empirically。
- **wgpu-external-frame**（water-rs）— `src/shared_handle.rs` 是本次导入代码的直接参照。
- 同型：Veyra-NRVideo（共享 fence + 租约池）、Servo/Slint、lumina-video、OBS shtex。

## 实现要点

| 文件 | 改动 |
|---|---|
| `dz-phone/src/wgc.rs` | `SharedFrame`(producer COM资源) + `SharedHandle`(consumer句柄)；`adapter_by_luid` 按 wgpu 同卡建 D3D11 设备；`shared_texture` 建 `SHARED_NTHANDLE\|KEYEDMUTEX` 纹理；`grab_shared` 用 keyed mutex + `D3D11_QUERY_EVENT` spin 替代 Map |
| `dz-phone/src/gpu_glass.rs` | `import_shared_handle`：`as_hal::<Dx12>` → `raw_d3d12_device().OpenSharedHandle` → `texture_from_raw` → `create_texture_from_hal`（wgpu 24 是 2 参数版）；`make_gpu_inner` 写 adapter LUID；`prepare()` 开头分流，成功则跳过整条 CPU 合成/上传 |

### 同步契约（来自 scrying 实证）

- producer：`AcquireSync(0,500ms)` → `CopySubresourceRegion` → `Begin/End(event query)` → `Flush` → spin `GetData` → `ReleaseSync(0)`。
- consumer：D3D12 从 keyed-mutex 句柄打开的资源无法 QI `IDXGIKeyedMutex`，故用一次性 1×1 `copy_texture_to_buffer` 触发状态转换屏障刷 shader cache。**这是驱动行为，不是契约**。

## 真机验证记录（2026-11-19，RTX 4070 Ti SUPER + 2560x1440）

- 编译 + 1372 测试通过；`DAOZAI_LIQUID_GPU_SHARE=1` 启动进程存活、无崩溃。
- 日志强证明零拷贝链路跑通：`D3D11 绑定到同卡 RTX 4070 Ti SUPER` → `GPU 共享纹理已创建`
  → `[gpu-share] frame N` 持续增长 → `[gpu-share] 导入共享纹理 WxH`（展开动画按尺寸重建）；
  **全程无 `capture: src=wgc` / `mean_r=`** = CPU 读回路径确实被绕过。
- **发现并修复真实 bug（提交 `9342a24`）**：初版共享路径在 `prepare()` 开头分流，绕过了 `capture()`，
  导致 `enable_capture_exclusion()`（WDA_EXCLUDEFROMCAPTURE）从未执行 → WGC 帧抓到岛仔自己 →
  展开态出现「玻璃里显示玻璃」的巨大错位文字重影。修复：共享路径入口主动调排除；冻结诊断模式
  (`DAOZAI_LIQUID_FREEZE_BG`) 整段跳过共享路径交回 `capture()`（冻结模式的排除/底片时序不适用共享帧）。
  修复后日志出现 `exclude: 检查 #0 窗口 1 个，重设 1 个`，重影消失。
- 视觉对照：冻结模式（旧路径）展开面板截图正常——壁纸折射自然、无重影/黑块、文字清楚。
  正常模式下 WDA 排除使岛仔本体从截屏消失（预期行为），故共享路径的画面无法直接截图，
  但它与旧路径共用同一 WGC 干净帧 + 同一 shader + 同一 uniform，仅数据进显存方式不同。

## v1.0.1 首轮真机反馈：背景放大/错位（`c37bc7f` 修复）

- **症状**：展开后背景里的鼠标被放大、位置对不上，左侧出现淡紫色不规则四边形。
- **根因**：shader `sample_px` 硬编码纹理分配尺寸 1280×512 算采样 `scale = tex/1280, tex/512`。
  旧路径纹理确实是 1280×512（帧写左上角），但共享纹理尺寸就是帧尺寸（854×422），
  scale 误算成 0.67 → 只采样左上角再铺满面板 → 背景/鼠标放大错位、越界取到未初始化区域。
- **修复**：WGSL 改用 `textureDimensions(bg)` 查纹理实际分配尺寸，`scale = tex / alloc`；
  旧路径不变、共享路径 scale=1。不改 Rust uniform / 测试。
- **教训**：从「固定纹理 + 左上角写帧」切到「纹理尺寸=帧尺寸」时，所有按 1280/512
  硬编码的采样 scale 都要改成动态查纹理尺寸。

## 显卡要求与配置说明（2026-11-19）

**结论：零拷贝的前提是「同卡」，不是「独显」。**
WGC 的 D3D11 设备和 wgpu 的 D3D12 设备必须在同一张显卡上，共享纹理才走显存直连；
跨卡（如 WGC 核显 / wgpu 独显）会经系统内存中转，收益打折。代码用
`adapter_by_luid`（wgpu adapter 的 LUID）创建 D3D11 设备来保证同卡，日志会打
`D3D11 设备绑定到 wgpu 同卡 adapter: "..."`。

三种情况：
1. 都在独显（混合显卡机器开 `liquid_prefer_dgpu=1`）：显存直连，收益最大。
2. 都在核显（无独显机器，或关掉 prefer_dgpu）：同卡共享系统内存，仍零拷贝，
   省掉 Map(READ) 的 CPU 同步等待，但没有 PCIe 传输可省，收益小一些。
3. 跨卡：代码用 LUID 匹配专门避免；匹配失败会记警告并继续（退化为系统内存中转）。

**非液态皮肤完全无影响**：两个开关都在「液态玻璃配置」弹窗里（`glassy()` 门禁），
非液态用户看不到也点不到；`gpu_share_enabled()` 只在 `GlassCallback::prepare` 里读，
而该回调只在液态皮肤 + 有 wgpu（render_compat=1）时注册。main.rs 无条件 `set_var`
只设环境变量，无消费者，零副作用。液态皮肤但 glow 后端同理不生效。

**文案**（设置页，已按此更新）：
- 优先独立显卡：「独显渲染玻璃，并让抓屏用同一张卡（零拷贝需要），重启生效」
- 零拷贝抓屏（实验）：「玻璃背景不走 CPU 读回，降低卡顿；异常时请关闭」
  + 提示「仅液态皮肤 + 兼容渲染(D3D12) 生效，重启后生效；非液态皮肤无影响」

## 已知限制 / 风险（未解决）

1. **未实机验证**：本机无 WGC + D3D12 真实画面，共享路径的正确性待真机确认。
2. **同卡前提**：WGC 的 D3D11 设备必须和 wgpu 的 D3D12 设备在同一 GPU；`adapter_by_luid` 匹配失败会记警告并继续，此时共享纹理经系统内存中转，收益打折。
3. **CPU 处理段仍在旧路径**：底片合成 / 主题亮度 / 黑岛帧判定靠 CPU 像素工作（防「玻璃里显示玻璃」）。纯 GPU 化要把它们搬进 WGSL compute，是更大的工程。本分支只去掉传输层的大头。
4. **1×1 屏障刷 cache 是驱动行为**：scrying 报告的更严谨替代是共享 fence（`CreateSharedHandle`→`OpenSharedFence`，约 150–250 行）。若真机出现陈旧帧，优先上 fence。
5. **回退细节**：共享导入成功后若某帧 `grab_shared` 返回 NoFrame，会落到旧 capture 路径，但 bind_group 仍指向共享纹理——可能出现一帧内容不同步。真机验证时留意。
6. **handle 生命周期**：`CreateSharedHandle` 返回的 NT handle 由 WGC 会话持有，未显式 `CloseHandle`（跟随会话 RAII 前需复查）。

## 验证记录

- `cargo check --locked --bin daozai`：通过。
- 全量串行测试：见本次工作日志。
