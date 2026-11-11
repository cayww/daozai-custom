#!/usr/bin/env python3
"""岛仔通用状态总线写入器（零依赖，Python 3.8+）。

文件里的每一条就是「当前状态」：同 id 覆写会刷新并续期，删掉条目立刻从岛上撤下。
用法：
    py send_status.py set --id build --text "构建中 55%" --detail "cargo build --release" --progress 0.55 --color blue
    py send_status.py set --id build --text "构建完成" --color green --click "copy:cargo build --release"
    py send_status.py remove --id build
    py send_status.py list
    py send_status.py clear
    py send_status.py demo
其他脚本可直接 from send_status import StatusBus 复用（带文件锁，多进程并发写不互相覆盖）。
"""
import argparse
import json
import os
import pathlib
import tempfile

try:
    import msvcrt
except ImportError:
    msvcrt = None

FIELDS = ("id", "text", "detail", "seconds", "progress", "color", "click")
COLORS = ("", "red", "green", "blue", "amber", "gray")


def status_path():
    base = os.environ.get("APPDATA") or str(pathlib.Path.home() / "AppData" / "Roaming")
    folder = pathlib.Path(base) / "Daozai"
    folder.mkdir(parents=True, exist_ok=True)
    return folder / "island_status.json"


class StatusBus:
    """读改写用文件锁保护，写入用临时文件 + 原子替换，岛仔不会读到半截 JSON。"""

    def __init__(self, path=None):
        self.path = pathlib.Path(path) if path else status_path()
        self.lock = self.path.with_suffix(".lock")

    @staticmethod
    def key(item):
        return str(item.get("id") or item.get("text") or "")

    def _read(self):
        try:
            data = json.loads(self.path.read_text(encoding="utf-8"))
            return data if isinstance(data, list) else []
        except (OSError, ValueError):
            return []

    def _write(self, items):
        fd, tmp = tempfile.mkstemp(dir=str(self.path.parent), prefix=".island_status-", suffix=".tmp")
        try:
            with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as handle:
                json.dump(items, handle, ensure_ascii=False, indent=2)
                handle.write("\n")
            os.replace(tmp, self.path)
        finally:
            if os.path.exists(tmp):
                os.unlink(tmp)

    def _locked(self, op):
        self.lock.touch(exist_ok=True)
        with open(self.lock, "a+") as handle:
            if msvcrt:
                msvcrt.locking(handle.fileno(), msvcrt.LK_LOCK, 1)
            try:
                return op()
            finally:
                if msvcrt:
                    try:
                        msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
                    except OSError:
                        pass

    def list(self):
        return self._read()

    def set(self, item):
        clean = {k: v for k, v in item.items() if k in FIELDS and v is not None}
        if not clean.get("text"):
            raise ValueError("text 不能为空")
        key = self.key(clean)

        def op():
            items = [x for x in self._read() if self.key(x) != key]
            items.append(clean)
            self._write(items)
            return clean

        return self._locked(op)

    def remove(self, key):
        def op():
            items = self._read()
            kept = [x for x in items if self.key(x) != key]
            self._write(kept)
            return len(items) - len(kept)

        return self._locked(op)

    def clear(self):
        def op():
            self._write([])

        return self._locked(op)


def demo():
    import time
    bus = StatusBus()
    print("演示：进度条 0→100%，完成后自动撤下（约 12 秒）")
    for pct in range(0, 101, 5):
        bus.set({
            "id": "demo",
            "text": "构建中 {}%".format(pct),
            "detail": "cargo build --release",
            "progress": pct / 100.0,
            "color": "green" if pct == 100 else "blue",
        })
        time.sleep(0.6)
    bus.remove("demo")
    print("已撤下 demo 状态")


def main():
    ap = argparse.ArgumentParser(description="岛仔通用状态总线写入器")
    sub = ap.add_subparsers(dest="cmd", required=True)

    setter = sub.add_parser("set", help="新增或覆写一条状态")
    setter.add_argument("--id", required=True, help="同一 id 反复写就是覆写/续期")
    setter.add_argument("--text", required=True, help="标题")
    setter.add_argument("--detail", default="", help="副标题")
    setter.add_argument("--seconds", type=float, default=0, help="0 = 默认 8 秒（靠覆写续期）")
    setter.add_argument("--progress", type=float, default=None, help="0~1 进度条")
    setter.add_argument("--color", default="", choices=COLORS, help="状态色")
    setter.add_argument("--click", default="", help="copy:文本 / url:链接 / file:路径")

    remover = sub.add_parser("remove", help="撤下指定 id")
    remover.add_argument("--id", required=True)
    sub.add_parser("list", help="打印当前 JSON")
    sub.add_parser("clear", help="清空全部状态")
    sub.add_parser("demo", help="进度条动画演示")

    args = ap.parse_args()
    bus = StatusBus()
    if args.cmd == "set":
        item = {"id": args.id, "text": args.text, "detail": args.detail, "seconds": args.seconds}
        if args.progress is not None:
            item["progress"] = max(0.0, min(1.0, args.progress))
        if args.color:
            item["color"] = args.color
        if args.click:
            item["click"] = args.click
        print(json.dumps(bus.set(item), ensure_ascii=False))
    elif args.cmd == "remove":
        print("removed", bus.remove(args.id))
    elif args.cmd == "list":
        print(json.dumps(bus.list(), ensure_ascii=False, indent=2))
    elif args.cmd == "clear":
        bus.clear()
        print("cleared")
    elif args.cmd == "demo":
        demo()


if __name__ == "__main__":
    main()
