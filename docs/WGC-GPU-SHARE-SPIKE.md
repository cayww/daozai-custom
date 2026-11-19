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
