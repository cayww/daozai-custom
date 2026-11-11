#!/usr/bin/env python3
"""零依赖数据源示例：把本机状态写成 island_status.json，岛上 2 秒内可见。

    py examples.py demo                     # 进度条动画演示
    py examples.py battery --once            # 手机连接电量 / 断开状态
    py examples.py adb --once               # ADB / 模拟器在线状态
    py examples.py download --dir D:\\Downloads --once   # 下载中文件数
    py examples.py battery                  # 持续轮询（默认 5 秒一次）

要点：每个示例只用 StatusBus.set/remove，不需要改岛仔源码；
不同来源用不同 id，互不覆盖。Ctrl+C 退出时会撤下自己那条状态。
"""
import argparse
import json
import os
import pathlib
import re
import subprocess
import sys
import time
from shutil import which

from send_status import StatusBus

CARD = r"Packages\Microsoft.YourPhone_8wekyb3d8bbwe\LocalState\StartMenu\StartMenuCompanion.json"
PART_SUFFIXES = (".part", ".crdownload", ".downloading", ".tmp")


def decode_escapes(text):
    out, i = [], 0
    while i < len(text):
        if text[i] == "\\" and i + 6 <= len(text) and text[i + 1] == "u":
            try:
                out.append(chr(int(text[i + 2:i + 6], 16)))
                i += 6
                continue
            except ValueError:
                pass
        out.append(text[i])
        i += 1
    return "".join(out)


def card_path():
    base = os.environ.get("LOCALAPPDATA") or str(pathlib.Path.home() / "AppData" / "Local")
    return pathlib.Path(base) / CARD


def battery(bus):
    path = card_path()
    if not path.exists():
        bus.remove("phone")
        return "手机连接卡片不存在"
    try:
        text = decode_escapes(path.read_text(encoding="utf-8", errors="replace"))
    except OSError as exc:
        return "读取失败: {}".format(exc)
    if "断开连接" in text or "未连接" in text or "disconnected" in text.lower():
        bus.set({"id": "phone", "text": "手机已断开", "detail": "Phone Link 未连接", "color": "amber"})
        return "已断开"
    match = re.search(r"电池电量剩余\s*(\d{1,3})\s*%", text)
    if not match:
        bus.remove("phone")
        return "卡片里没有电量数值"
    level = max(0, min(100, int(match.group(1))))
    color = "red" if level <= 20 else ("green" if level >= 95 else "blue")
    bus.set({
        "id": "phone",
        "text": "手机电量 {}%".format(level),
        "detail": "低电提醒" if level <= 20 else "来自手机连接",
        "progress": level / 100.0,
        "color": color,
    })
    return "{}%".format(level)


def find_adb():
    exe = which("adb")
    if exe:
        return exe
    candidates = [
        pathlib.Path(os.environ.get("LOCALAPPDATA", "")) / "Android/Sdk/platform-tools/adb.exe",
        pathlib.Path("D:/software/MuMu Player 12/shell/adb.exe"),
        pathlib.Path("D:/software/platform-tools/adb.exe"),
        pathlib.Path("D:/software/adb/adb.exe"),
    ]
    for item in candidates:
        if item.is_file():
            return str(item)
    return None


def adb(bus):
    exe = find_adb()
    if not exe:
        bus.remove("adb")
        return "找不到 adb.exe（把它加进 PATH，或改 examples.py 的候选路径）"
    try:
        out = subprocess.run([exe, "devices"], capture_output=True, text=True, timeout=8).stdout
    except (OSError, subprocess.SubprocessError) as exc:
        return "adb 执行失败: {}".format(exc)
    devices = [ln.split("\t")[0] for ln in out.splitlines()[1:]
               if "\t" in ln and ln.strip().endswith("device")]
    if devices:
        bus.set({
            "id": "adb",
            "text": "ADB 在线 {} 台".format(len(devices)),
            "detail": " · ".join(devices[:2]),
            "color": "green",
        })
    else:
        bus.remove("adb")
    return "{} 台设备".format(len(devices))


def download(bus, folder):
    root = pathlib.Path(folder)
    if not root.is_dir():
        bus.remove("download")
        return "目录不存在: {}".format(folder)
    parts = [p for p in root.rglob("*") if p.is_file() and p.suffix.lower() in PART_SUFFIXES]
    if parts:
        bus.set({
            "id": "download",
            "text": "下载中 {} 个".format(len(parts)),
            "detail": parts[0].name[:40],
            "color": "blue",
        })
    else:
        bus.remove("download")
    return "{} 个进行中".format(len(parts))


def demo(bus):
    for pct in range(0, 101, 10):
        bus.set({
            "id": "demo",
            "text": "构建中 {}%".format(pct),
            "detail": "cargo build --release",
            "progress": pct / 100.0,
            "color": "green" if pct == 100 else "blue",
            "click": "copy:cargo build --release",
        })
        time.sleep(0.5)
    bus.remove("demo")
    return "演示完成，已撤下"


def main():
    ap = argparse.ArgumentParser(description="岛仔状态总线数据源示例")
    ap.add_argument("mode", choices=["demo", "battery", "adb", "download"])
    ap.add_argument("--dir", default=str(pathlib.Path.home() / "Downloads"), help="download 模式监听的目录")
    ap.add_argument("--once", action="store_true", help="只跑一次就退出")
    ap.add_argument("--interval", type=float, default=5.0, help="持续模式轮询间隔（秒）")
    args = ap.parse_args()

    bus = StatusBus()
    state_id = {"demo": "demo", "battery": "phone", "adb": "adb", "download": "download"}[args.mode]
    try:
        while True:
            if args.mode == "demo":
                result = demo(bus)
            elif args.mode == "battery":
                result = battery(bus)
            elif args.mode == "adb":
                result = adb(bus)
            else:
                result = download(bus, args.dir)
            print("[{}] {}".format(time.strftime("%H:%M:%S"), result), flush=True)
            if args.once or args.mode == "demo":
                break
            time.sleep(max(1.0, args.interval))
    except KeyboardInterrupt:
        pass
    finally:
        if not args.once and args.mode != "demo":
            bus.remove(state_id)
            print("已撤下 {} 状态".format(state_id))


if __name__ == "__main__":
    main()
