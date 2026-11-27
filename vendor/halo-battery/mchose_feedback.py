"""按实测无线接收器读取迈从 V9 Pro 2 和 G3 V2。"""

import time

from providers import hidlist
from providers.base import DeviceStatus, Provider


VID = 0x3837
V9_PID = 0x6011
G3_PID = 0x4245
V9_QUERY = bytes.fromhex("55 AA 01 00 01 00 01 0B").ljust(64, b"\0")
G3_ONLINE = bytes.fromhex("00 55 ED").ljust(65, b"\0")
G3_QUERY = bytes.fromhex("00 55 30 A5 0B 2E 01 01 01").ljust(65, b"\0")


def select_path(pid, page, usage, products, *, interface=None):
    if hidlist.hid is None:
        return None
    rows = [
        row for row in hidlist.hid.enumerate(VID, pid)
        if (row.get("vendor_id"), row.get("product_id")) == (VID, pid)
        and (row.get("usage_page"), row.get("usage")) == (page, usage)
        and (interface is None or row.get("interface_number") == interface)
    ]
    if len(rows) != 1:
        return None
    product = (rows[0].get("product_string") or "").strip().casefold()
    return rows[0]["path"] if product in products else None


def read_reply(device, request, size, deadline, matches):
    device.set_nonblocking(True)
    for _ in range(4):
        if not device.read(size, 0):
            break
    else:
        return None
    if device.write(request) != len(request):
        return None
    for _ in range(5):
        remaining = int((deadline - time.monotonic()) * 1000)
        if remaining < 1:
            return None
        reply = bytes(device.read(size, remaining))
        if not reply:
            return None
        if len(reply) == size and matches(reply):
            return reply
    return None


class MchoseV9Pro2Provider(Provider):
    """只接受 MC 电量回应，睡眠值不作有效读数。"""

    name = "mchose_v9pro2"

    def poll(self):
        # M HUB 的 MC 电量查询，在 fuyue521 项目由 V9 Pro 2 实机验证。
        # https://github.com/fuyue521/Wireless-Device-Battery-Monitor-CN/blob/main/src/drivers/mchose_headset.cpp
        path = select_path(V9_PID, 0xFF07, 0x0222,
                           {"mchose v9 pro 2", "bestechnic mchose v9 pro 2"}, interface=3)
        if path is None:
            return []
        device = hidlist.hid.device()
        try:
            device.open_path(path)
            lengths = device.get_report_lengths()
            if (lengths["input"], lengths["output"]) != (64, 64):
                return []
            reply = read_reply(device, V9_QUERY, 64, time.monotonic() + 0.4,
                               lambda frame: frame[:3] == b"\x55\xaa\x01"
                               and frame[5:8] in (b"\x00\x01\x0b", b"\x00\x04\x22")
                               and 3 <= frame[4] <= 59)
            if reply is None or reply[8] == 0:
                return []
            level = reply[9] & 0x7F
            if level > 100:
                return []
            return [DeviceStatus(self.name, "MCHOSE V9 Pro 2", level, source=self.name,
                                 kind="headset", charging_known=False)]
        except (OSError, ValueError):
            return []
        finally:
            device.close()


class MchoseG3V2Provider(Provider):
    name = "mchose_g3v2"

    def poll(self):
        # 原机的查询、响应布局与无线身份见 MCHOSE Monitor 的 HID 记录。
        # https://github.com/Jstdi001/MCHOSE-Monitor/blob/main/docs/HID_PROTOCOL.md
        path = select_path(G3_PID, 0xFF01, 0x0010, {"mchose g3 v2"})
        if path is None:
            return []
        device = hidlist.hid.device()
        try:
            device.open_path(path)
            lengths = device.get_report_lengths()
            if (lengths["input"], lengths["output"]) != (65, 65):
                return []
            deadline = time.monotonic() + 0.55
            online = read_reply(device, G3_ONLINE, 64, deadline,
                                lambda frame: frame.startswith(b"\xaa\xed"))
            if online is None or online[8] not in (0, 2):
                return []
            reply = read_reply(device, G3_QUERY, 64, deadline,
                               lambda frame: frame.startswith(b"\xaa\x30"))
            if reply is None or not 1 <= reply[8] <= 100:
                return []
            return [DeviceStatus(self.name, "MCHOSE G3 V2", reply[8], charging=reply[9] == 1,
                                 source=self.name, kind="mouse", charging_known=reply[9] in (0, 1))]
        except (OSError, ValueError):
            return []
        finally:
            device.close()
