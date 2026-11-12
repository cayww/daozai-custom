#!/usr/bin/env python3
"""安装/卸载「岛仔状态桥」的 Native Messaging 宿主。

做三件事（全部可逆，只写 HKCU，不需要管理员）：
  1. 生成 host/daozai_bridge.cmd（定位当前 python.exe）
  2. 生成 host/com.daozai.island.json（allowed_origins 用扩展固定 ID）
  3. HKCU\\Software\\Google\\Chrome\\NativeMessagingHosts\\com.daozai.island 指向上面的 json

用法：
    python install_host.py            # 安装（Chrome；--edge 加装 Edge）
    python install_host.py --uninstall
    python install_host.py --test     # 不注册，直接用假消息自测桥
"""
import argparse
import json
import os
import pathlib
import subprocess
import sys

HOST_NAME = "com.daozai.island"
EXT_ID = pathlib.Path(__file__).parent / "host" / "ext_meta.txt"
HERE = pathlib.Path(__file__).resolve().parent
HOST_DIR = HERE / "host"


def ext_id():
    return EXT_ID.read_text(encoding="ascii").splitlines()[0].strip()


def write_files():
    py = sys.executable
    cmd = HOST_DIR / "daozai_bridge.cmd"
    cmd.write_text('@echo off\r\n"{}" "{}" %*\r\n'.format(py, HOST_DIR / "daozai_bridge.py"), encoding="ascii")
    manifest = {
        "name": HOST_NAME,
        "description": "岛仔状态总线桥：把扩展消息写进 island_status.json 显示到灵动岛",
        "path": str(cmd),
        "type": "stdio",
        "allowed_origins": ["chrome-extension://" + ext_id() + "/"],
    }
    out = HOST_DIR / (HOST_NAME + ".json")
    out.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    return out


def reg_root(browser):
    return r"Software\{}\NativeMessagingHosts\{}".format(
        "Google\\Chrome" if browser == "chrome" else "Microsoft\\Edge", HOST_NAME)


def install(browsers):
    manifest = write_files()
    for b in browsers:
        key = reg_root(b)
        subprocess.run(["reg", "add", key, "/ve", "/d", str(manifest), "/f"], check=True)
        print(f"[{b}] {key} -> {manifest}")
    print("完成。扩展目录：", HERE / "extension")


def uninstall(browsers):
    for b in browsers:
        key = reg_root(b)
        subprocess.run(["reg", "delete", key, "/f"], check=False)
        print(f"[{b}] 已删除 {key}")


def selftest():
    """模拟扩展发消息（原生消息 = 4 字节小端长度 + JSON），验证桥的翻译。"""
    import struct
    msgs = [
        {"t": "dl", "id": 1, "name": "演示文件.zip", "state": "progress", "received": 20 * 1024 * 1024, "total": 100 * 1024 * 1024},
        {"t": "dl", "id": 1, "name": "演示文件.zip", "state": "progress", "received": 60 * 1024 * 1024, "total": 100 * 1024 * 1024},
        {"t": "status", "id": "tab", "text": "扩展通道在线", "color": "green", "seconds": 6},
    ]
    payload = b"".join(
        (lambda raw: struct.pack("<I", len(raw)) + raw)(json.dumps(m, ensure_ascii=False).encode("utf-8"))
        for m in msgs
    )
    proc = subprocess.run([sys.executable, str(HOST_DIR / "daozai_bridge.py")],
                           input=payload, capture_output=True)
    sys.stdout.write(proc.stdout.decode("utf-8", "replace"))
    if proc.stderr:
        print(proc.stderr.decode("utf-8", "replace"), file=sys.stderr)
    bus = HERE / "scripts" / "send_status.py"
    subprocess.run([sys.executable, str(bus), "list"], check=False)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--edge", action="store_true", help="同时注册到 Edge")
    ap.add_argument("--uninstall", action="store_true")
    ap.add_argument("--test", action="store_true")
    args = ap.parse_args()
    browsers = ["chrome"] + (["edge"] if args.edge else [])
    if args.test:
        selftest()
    elif args.uninstall:
        uninstall(browsers)
    else:
        install(browsers)


if __name__ == "__main__":
    main()
