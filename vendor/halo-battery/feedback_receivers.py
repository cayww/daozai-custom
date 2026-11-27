"""根据实机采样读取三个已确认的 USB 接收器。"""

import time

import hid

from providers.base import DeviceStatus, Provider


class _VerifiedReceiverProvider(Provider):
    def poll(self):
        vid, pid = self.receiver
        string_key, expected_string = self.identity_string
        paths = [row["path"] for row in hid.enumerate(vid, pid)
                 if (row.get("vendor_id"), row.get("product_id")) == self.receiver
                 and (row.get("usage_page"), row.get("usage")) == self.usage
                 and row.get("interface_number") in self.interfaces
                 and (row.get(string_key) or "").strip().casefold() == expected_string]
        if len(paths) != 1:
            return []

        device = hid.device()
        try:
            device.open_path(paths[0])
            caps = device.get_report_lengths()
            lengths = zip(("input", "output", "feature"), self.report_lengths)
            if any(expected is not None and caps[key] != expected for key, expected in lengths):
                return []
            sample = self.read_battery(device)
        except OSError:
            return []
        finally:
            device.close()

        if sample is None:
            return []
        level, charging = sample
        return [DeviceStatus(self.name, self.device_name, level=level, charging=charging is True,
                             source=self.name, kind=self.kind, charging_known=charging is not None)]


def _read_interrupt_battery(device, request, decode):
    # 先清掉已经排队的旧报告，不等待后续异步报告。
    device.set_nonblocking(True)
    for _ in range(4):
        if not device.read(len(request), 0):
            break
    else:
        return None
    device.set_nonblocking(False)
    if device.write(request) != len(request):
        return None
    deadline = time.monotonic() + 0.30
    for _ in range(8):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None
        reply = bytes(device.read(len(request), max(1, int(remaining * 1000))))
        if not reply:
            return None
        sample = decode(reply)
        if sample is not None:
            return sample
    return None


def _decode_l8_max(reply):
    if len(reply) != 64 or not reply.startswith(b"\xA1\x02\x04\x01\x00\x38"):
        return None

    # 按本体 ID、ASCII 序列号和固定长度记录的边界解析，避免在值中搜索电量标记。
    payload = reply[6:]
    offset = 0
    identified = False
    level = None
    while offset < len(payload):
        marker = payload[offset:offset + 2]
        offset += 2
        if marker == b"\x02\x11":
            if identified or payload[offset:offset + 4] != b"\x43\x00\x9B\x36":
                return None
            identified = True
            offset += 4
        elif marker == b"\x02\x13":
            start = offset
            while offset < len(payload) and 0x20 <= payload[offset] <= 0x7E:
                offset += 1
            if start == offset:
                return None
            if offset < len(payload) and payload[offset] == 0:
                offset += 1
        elif marker in (b"\x02\x25", b"\x02\x26"):
            if offset == len(payload):
                return None
            if marker == b"\x02\x25":
                if level is not None or payload[offset] > 100:
                    return None
                level = payload[offset]
            offset += 1
        else:
            # 未知记录无法安全跳过。此前已解析的字段仍有明确边界。
            break
    return (level, None) if identified and level is not None else None


class MachenikeL8MaxProvider(_VerifiedReceiverProvider):
    # 设备持有者实测命令和本体标识。
    # https://github.com/Renly1994/xiaotang-daozai/issues/267#issuecomment-7179894023
    name = "machenike_l8_max"
    device_name = "机械师 L8 Max"
    kind = "mouse"
    receiver = (0x369B, 0x0044)
    identity_string = ("product_string", "l8 max")
    usage = (0xFFA0, 0x0002)
    # Windows 单接口设备的 HID 路径可能省略 MI_00。
    interfaces = (0, -1)
    report_lengths = (64, 64, None)

    def read_battery(self, device):
        return _read_interrupt_battery(device, b"\xA0\x02" + bytes(62), _decode_l8_max)


def _decode_dragonfly_3_pro(reply):
    # HID 传输层已去掉无编号输入报告的 Windows 前导零。
    if (len(reply) == 64 and reply.startswith(b"\xAA\x30\xA5\x2E")
            and 1 <= reply[8] <= 100 and reply[9] == 0):
        return reply[8], False
    return None


class VgnDragonfly3ProProvider(_VerifiedReceiverProvider):
    # Windows 11 真机的单条电量查询及 74% 回包。
    # https://github.com/ferloom/Wireless-Device-Battery-Monitor/issues/272
    name = "vgn_dragonfly_3_pro"
    device_name = "VGN 蜻蜓 3 Pro"
    kind = "mouse"
    receiver = (0x391D, 0x1A03)
    identity_string = ("manufacturer_string", "yjx-chip")
    usage = (0xFF01, 0x0010)
    interfaces = (2,)
    report_lengths = (65, 65, 0)

    def read_battery(self, device):
        request = bytes.fromhex("00 55 30 A5 2E 2E") + bytes(59)
        return _read_interrupt_battery(device, request, _decode_dragonfly_3_pro)


class DareuEk87ProProvider(_VerifiedReceiverProvider):
    # 官方网页驱动的实机抓包，电量寄存器在键盘休眠时仍可能保留旧值。
    # https://github.com/Renly1994/xiaotang-daozai/issues/267#issuecomment-7181199203
    name = "dareu_ek87_pro"
    device_name = "达尔优 EK87 Pro"
    kind = "keyboard"
    receiver = (0x260D, 0x0037)
    identity_string = ("product_string", "usb 2.4g receiver")
    usage = (0x000C, 0x0001)
    interfaces = (3,)
    report_lengths = (0, 0, 65)

    def read_battery(self, device):
        request = bytes.fromhex("00 10 03 07 80") + bytes(60)
        for _ in range(3):
            if device.send_feature_report(request) != len(request):
                return None
            reply = bytes(device.get_feature_report(0, len(request)))
            if (len(reply) == 65 and reply.startswith(b"\x00\x12\x03\x07\x80\x00\x00")
                    and reply[8] <= 100 and reply[9] == 100):
                # 实机已确认标志 0 表示未充电，其他标志保留未知。
                return reply[8], False if reply[7] == 0 else None
        return None
