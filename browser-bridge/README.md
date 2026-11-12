# 岛仔 × 浏览器扩展桥（Native Messaging）

Chrome 扩展通过官方 Native Messaging 通道把**精确的下载进度**发给本机桥进程，
桥写入 `island_status.json`，岛仔常驻区实时显示。
岛仔本体通过状态总线消费——扩展只是状态总线的又一个生产者。

## 结构

```
extension/          Chrome 扩展（MV3，manifest 里带固定 key → 扩展 ID 恒定）
  manifest.json       权限 downloads + nativeMessaging；ID = dcmheeclfeihicoaakoffjbbepfdfcin
  sw.js               downloads 事件 + 500ms 轮询进度 + 30s 闹钟心跳 → connectNative
```

桥宿主（接收消息、写状态文件）已拆成独立 Rust 二进制 `daozai-bridge`，源码在
`dz-phone/src/bin/daozai-bridge/`，由安装包（`installer/daozai.iss`）打包为
`{app}\bridge\daozai_bridge.exe` 并写入 Chrome/Edge 的 NativeMessagingHosts 注册表。

## 安装（由岛仔安装包自动完成）

安装包会：
1. 把 `daozai_bridge.exe`（约 460KB 的独立桥）装到安装目录 `bridge\`；
2. 生成宿主清单 `com.daozai.island.json` 并注册到 Chrome + Edge（HKCU，无需管理员）；
3. 把 `extension/` 里的扩展文件放到 `bridge\extension\`。

用户只需在岛仔「设置 → 浏览器下载」点「安装到 Chrome / Edge」一键引导加载扩展
（或手动 `chrome://extensions` 开「开发者模式」→「加载已解压的扩展程序」→
选 `bridge\extension\` 文件夹）。

## 安全设计

- 扩展 ID 由 `manifest.json` 的 `key` 固定，宿主清单只放行这一个 ID；
- 桥只写状态文件，字段白名单（id/text/detail/seconds/progress/color/click/ttl/priority），多余键丢弃；
- `click` 动作沿用岛仔侧白名单（copy:/url:/file:，可执行文件拒绝）——
  扩展能「显示状态」，不能「指挥电脑」；
- 桥异常退出时，进行中条目 15 秒内自动消失（ttl 续期机制），不会留僵尸胶囊。

## 已知限制

- MV3 Service Worker 空闲 30 秒被浏览器回收，端口随之断开——下次下载事件自动重连，桥侧幂等；
- `--load-extension` 命令行加载在近版 Google Chrome 已被禁用，须手动「加载已解压」；
- Chrome 的 `downloads.onChanged` 不推送 `bytesReceived`，进度靠扩展轮询
  `downloads.search`（500ms）+ 30 秒闹钟心跳兜底。
