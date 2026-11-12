#!/usr/bin/env python3
"""岛仔 × 浏览器扩展的 Native Messaging 桥。

Chrome/Edge 扩展通过原生消息协议（stdin/stdout：4 字节小端长度 + UTF-8 JSON）
把下载进度等事件发进来，桥翻译成 island_status.json 的状态条目，
岛仔常驻区实时显示（岛每 2 秒读一次文件）。

协议（扩展 → 桥）：
    {"t":"dl","id":17,"name":"file.zip","path":"C:\\...\\file.zip",
     "state":"progress","received":12345,"total":54321}
    state ∈ progress | complete | cancel；complete 时 path 用于「点一下打开」。
也支持通用透传（扩展以后想挂任何状态都走这个）：
    {"t":"status","id":"tab","text":"...","detail":"...","progress":0.5,
     "color":"green","click":"copy:...","seconds":8}
    字段白名单同 send_status.py，多余的键一律丢弃。

桥 → 扩展：每条回 {"ok":true}（调试用，扩展可忽略）。
出错只写 stderr——stdout 是协议通道，绝不能被日志污染。
"""
import json
import os
import pathlib
import struct
import sys
import time

# send_status.py 是零依赖单文件，随桥一起分发（browser-bridge/scripts）。
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent / "scripts"))
from send_status import FIELDS, StatusBus  # noqa: E402

bus = StatusBus()
SPEED = {}  # dl id -> (received, monotonic t)


def _out(obj):
    raw = json.dumps(obj, ensure_ascii=False).encode("utf-8")
    sys.stdout.buffer.write(struct.pack("<I", len(raw)) + raw)
    sys.stdout.buffer.flush()


def _fmt(n):
    n = float(n or 0)
    for unit, lim in (("GB", 1 << 30), ("MB", 1 << 20), ("KB", 1 << 10)):
        if n >= lim:
            return f"{n / lim:.1f} {unit}"
    return f"{int(n)} B"


def _short(name, n=24):
    name = os.path.basename(str(name))
    return name if len(name) <= n else name[: n - 1] + "…"


def handle(msg):
    t = msg.get("t")
    if t == "dl":
        _download(msg)
    elif t == "status":
        clean = {k: v for k, v in msg.items() if k in FIELDS and v is not None}
        if clean.get("text"):
            bus.set(clean)
    elif t == "ping":
        pass
    else:
        raise ValueError(f"unknown t={t!r}")


def _download(msg):
    dl_id = msg.get("id")
    key = "dl-" + str(dl_id if dl_id is not None else msg.get("name"))
    state = msg.get("state", "progress")
    if state == "cancel":
        bus.remove(key)
        SPEED.pop(dl_id, None)
        return
    name, total = msg.get("name") or "下载", msg.get("total") or 0
    if state == "complete":
        SPEED.pop(dl_id, None)
        item = {"id": key, "text": "下载完成 · " + _short(name), "seconds": 12, "color": "green"}
        path = msg.get("path") or ""
        # 只放行真实存在的普通文件；可执行扩展岛仔侧还会再拦一道。
        if path and os.path.isfile(path):
            item["click"] = "file:" + path
        bus.set(item)
        return
    received = msg.get("received") or 0
    speed_txt = ""
    now = time.monotonic()
    prev = SPEED.get(dl_id)
    if prev and now > prev[1]:
        rate = max(0.0, (received - prev[0]) / (now - prev[1]))
        if rate > 0:
            speed_txt = f" · {_fmt(rate)}/s"
    SPEED[dl_id] = (received, now)
    item = {
        "id": key,
        "text": "下载 · " + _short(name),
        "detail": (f"{_fmt(received)} / {_fmt(total)}" if total > 0 else _fmt(received)) + speed_txt,
        "seconds": 15,  # 桥进程挂了也能自动消失；活着就靠覆写续期
        "color": "blue",
    }
    if total > 0:
        item["progress"] = max(0.0, min(1.0, received / total))
    bus.set(item)


def main():
    while True:
        head = sys.stdin.buffer.read(4)
        if not head or len(head) < 4:
            return  # 扩展端口断开（浏览器空闲回收 SW），正常退出
        (size,) = struct.unpack("<I", head)
        raw = sys.stdin.buffer.read(size)
        try:
            handle(json.loads(raw.decode("utf-8")))
            _out({"ok": True})
        except Exception as exc:  # noqa: BLE001
            print(f"bridge error: {exc}", file=sys.stderr)
            _out({"ok": False, "error": str(exc)})


if __name__ == "__main__":
    main()
