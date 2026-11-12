// 岛仔状态桥：把 Chrome 下载事件实时发给本机 Native Messaging 桥（com.daozai.island），
// 桥写 island_status.json，桌面灵动岛常驻区显示进度胶囊。
// 长连接懒建：下载事件到达时才 connectNative；下载空闲 30 秒后 Service Worker
// 被浏览器回收、端口随之中断，下次事件再重连——桥侧对断开是幂等的。
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

chrome.downloads.onCreated.addListener(push);
chrome.downloads.onChanged.addListener((delta) => {
  chrome.downloads.search({ id: delta.id }, (items) => push(items && items[0]));
});
