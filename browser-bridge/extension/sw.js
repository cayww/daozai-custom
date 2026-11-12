// 岛仔状态桥：把 Chrome 下载事件实时发给本机 Native Messaging 桥（com.daozai.island），
// 桥写 island_status.json，桌面灵动岛常驻区显示进度胶囊。
//
// 进度获取方式说明（关键设计约束）：
// Chrome 官方文档规定 downloads.onChanged 不上报 bytesReceived（每次网络包都变，
// 故意排除）——事件只覆盖状态切换（filename / state / error 等）。所以进度必须
// 主动轮询 chrome.downloads.search：下载活跃期间每 500 ms 查一次 in_progress 列表；
// 另加 30 秒 alarms 心跳兜底（SW 万一被回收，闹钟会唤醒并重启轮询）。
// 长连接懒建：首个下载事件到达时才 connectNative；原生消息端口 + 轮询活动
// 会维持 SW 存活，下载全部结束后轮询自动停止、SW 随之休眠，端口断开——
// 桥侧对断开是幂等的。
const HOST = "com.daozai.island";
let port = null;

function send(msg) {
  try {
    if (!port) {
      port = chrome.runtime.connectNative(HOST);
      port.onDisconnect.addListener(() => { port = null; });
    }
    port.postMessage(msg);
  } catch (e) {
    port = null;
  }
}

function baseName(p) {
  return p ? String(p).split(/[\\/]/).pop() : "下载";
}

// onChanged 每收到一段数据就触发一次，太密；每个下载至少间隔 400 ms 发一次，
// complete/cancel 无条件立即发。
const lastSent = {};
function throttled(id, msg) {
  const now = Date.now();
  const wait = (lastSent[id] || 0) + 400 - now;
  if (wait <= 0) {
    lastSent[id] = now;
    send(msg);
  } else {
    setTimeout(() => { lastSent[id] = 0; send(msg); }, wait);
  }
}

function push(item) {
  if (!item) return;
  if (item.state === "in_progress") {
    throttled(item.id, {
      t: "dl", id: item.id, name: baseName(item.filename),
      path: item.filename, state: "progress",
      received: item.bytesReceived, total: item.totalBytes || 0,
    });
    ensurePolling();
  } else if (item.state === "complete") {
    delete lastSent[item.id];
    send({
      t: "dl", id: item.id, name: baseName(item.filename),
      path: item.filename, state: "complete",
    });
  } else { // interrupted / cancelled
    delete lastSent[item.id];
    send({ t: "dl", id: item.id, state: "cancel" });
  }
}

// ==== 进度轮询引擎 ====
let poller = null;
function ensurePolling() {
  if (poller) return;
  poller = setInterval(() => {
    chrome.downloads.search({ state: "in_progress" }, (items) => {
      if (chrome.runtime.lastError) return;
      if (!items || !items.length) {
        clearInterval(poller);
        poller = null;
        return;
      }
      for (const item of items) push(item);
    });
  }, 500);
}

// 30 秒心跳兜底：SW 若被回收，闹钟唤醒后恢复轮询（长下载进度不断流）。
chrome.alarms.create("daozai-dl-heartbeat", { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name !== "daozai-dl-heartbeat") return;
  chrome.downloads.search({ state: "in_progress" }, (items) => {
    if (chrome.runtime.lastError) return;
    if (items && items.length) for (const item of items) push(item);
  });
});

chrome.downloads.onCreated.addListener(push);
chrome.downloads.onChanged.addListener((delta) => {
  chrome.downloads.search({ id: delta.id }, (items) => push(items && items[0]));
});
