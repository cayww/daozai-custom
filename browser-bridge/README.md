# 岛仔 × 浏览器扩展桥（Native Messaging）

Chrome 扩展通过官方 Native Messaging 通道把**精确的下载进度**（以及未来的任意状态）
发给本机桥进程，桥写入 `island_status.json`，岛仔常驻区实时显示。
岛仔本体零改动——扩展只是状态总线的又一个生产者。

## 结构

```
extension/          Chrome 扩展（MV3，manifest 里带固定 key → 扩展 ID 恒定）
  manifest.json       权限仅 downloads；ID = dcmheeclfeihicoaakoffjbbepfdfcin
  sw.js               downloads.onCreated/onChanged → 节流 400ms → connectNative
host/
  daozai_bridge.py    桥：原生消息协议(4字节小端长度+JSON) → StatusBus 写入
  key.pem / pub.der   扩展签名密钥（固定 ID 的来源；勿外传）
  com.daozai.island.json  Native Messaging 宿主清单（install_host.py 生成）
  daozai_bridge.cmd       宿主入口（install_host.py 生成）
scripts/send_status.py  零依赖 StatusBus（与 daozai-patches 同源，自包含分发）
install_host.py     安装/卸载/自测
```

## 安装（一次性，两步）

1. **注册宿主**（写 HKCU，无需管理员）：
   ```
   python install_host.py          # Chrome
   python install_host.py --edge   # 同时支持 Edge
   ```
2. **加载扩展**：Chrome 打开 `chrome://extensions` → 右上角开「开发者模式」
   → 「加载已解压的扩展程序」→ 选本目录的 `extension/` 文件夹。

之后浏览器里任何下载都会实时上岛：蓝色胶囊「下载 · 文件名」+ 进度条 + 速度，
完成后变绿色「下载完成」，点一下直接打开文件，12 秒自动消失。

## 安全设计

- 扩展 ID 由 `manifest.json` 的 `key` 固定，宿主清单只放行这一个 ID；
- 桥只写状态文件，字段白名单同 send_status.py，多余键丢弃；
- `click` 动作沿用岛仔侧白名单（copy:/url:/file:，可执行文件拒绝）——
  扩展能「显示状态」，不能「指挥电脑」；
- 桥异常退出时，进行中条目 15 秒内自动消失（seconds 续期机制），不会留僵尸胶囊。

## 验证

```
python install_host.py --test   # 用假消息驱动桥，看岛上的胶囊
```

## 已知限制

- MV3 Service Worker 空闲 30 秒被浏览器回收，端口随之断开——下次下载事件自动重连，桥侧幂等；
- `--load-extension` 命令行加载在近版 Google Chrome 已被禁用，须手动「加载已解压」；
- 沙箱/安全策略若拦截 `reg.exe`，把安装命令放到自己的终端里跑即可。
