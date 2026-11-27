"""按实测 USB 身份读取 Wave 75、G3 A 和前行者 X87。"""

import time

try:
    import hid
except ImportError:
    hid = None

from providers.base import DeviceStatus, Provider


def _request(payload, size):
    return (b"\0" + payload).ljust(size, b"\0")


def _wait_reply(device, size, prefix, timeout_ms):
    deadline = time.monotonic() + timeout_ms / 1000
    for _ in range(8):
        remaining = int((deadline - time.monotonic()) * 1000)
        if remaining < 1:
            break
        reply = bytes(device.read(size, remaining))
        if not reply:
            break
        # 组件的 Windows HID 传输层已经移除了 0 号报告的前缀字节。
        if len(reply) == size - 1 and reply.startswith(prefix):
            return reply
    return None


def _drain(device, size):
    device.set_nonblocking(True)
    for _ in range(8):
        if not device.read(size, 0):
            return True
    return False


class _ReceiverProvider(Provider):
    kind = "keyboard"
    interface = None
    size = 33

    @classmethod
    def matches_product(cls, row):
        return (
            (row.get("vendor_id"), row.get("product_id")) == cls.identity
            and (row.get("product_string") or "").strip().casefold() == cls.product
        )

    @classmethod
    def matches(cls, row):
        return (
            cls.matches_product(row) and (row.get("usage_page"), row.get("usage")) == cls.usage
            and (cls.interface is None or row.get("interface_number") == cls.interface)
        )

    def poll(self):
        if hid is None:
            return []
        paths = [row["path"] for row in hid.enumerate(*self.identity) if self.matches(row)]
        if len(paths) != 1:
            return []
        device = hid.device()
        try:
            device.open_path(paths[0])
            caps = device.get_report_lengths()
            if (caps["input"], caps["output"]) != (self.size, self.size):
                return []
            if not _drain(device, self.size):
                return []
            reading = self.read(device)
        except (OSError, ValueError):
            return []
        finally:
            device.close()
        if reading is None:
            return []
        level, charging = reading
        return [DeviceStatus(self.name, self.device_name, level, charging is True,
                             source=self.name, kind=self.kind, charging_known=charging is not None)]


class Wave75Provider(_ReceiverProvider):
    # Wave 75 RGB 原机、接收器产品名及配对键盘 ID。
    # https://github.com/Renly1994/xiaotang-daozai/issues/267#issuecomment-7206609792
    # 会话命令及回包字段：https://github.com/ferloom/Wireless-Device-Battery-Monitor/blob/181b371c4c10301c6810acd18601a177085de95b/src/drivers/via_dongle.rs
    name = "pmo_wave75"
    device_name = "PMO Wave 75"
    identity = 0x36B0, 0x3002
    product = "wave75-2.4g"
    usage = 0xFF60, 0x0061
    interface = 1

    def read(self, device):
        start = b"\xaa\x10\0\0"
        if device.write(_request(start + b"\x18", self.size)) != self.size:
            return None
        try:
            paired = _wait_reply(device, self.size, start, 450)
            if paired is None or paired[7:13] != b"\x55\x00\xb0\x36\x09\x30":
                return None
            if not _drain(device, self.size):
                return None
            query = b"\xaa\x30\0\0"
            if device.write(_request(query + b"\x18", self.size)) != self.size:
                return None
            reply = _wait_reply(device, self.size, query, 450)
            if reply is not None and reply[7] == 0x55 and reply[8] <= 100:
                return reply[8], None
        finally:
            device.write(_request(b"\xaa\x11\0\0\x18", self.size))
        return None


class MchoseG3AProvider(_ReceiverProvider):
    # 仅该产品走 YJX 在线门控。开启接收器但关闭鼠标时，电量命令可能返回旧值。
    # https://github.com/Renly1994/xiaotang-daozai/issues/267#issuecomment-7206609792
    # 帧事实：https://github.com/battery-hub/battery-hub/blob/aff030007f6cd217803e552cc0a0b60d17bd7bc9/src/main/hid/drivers/yjx.js
    name = "mchose_g3a"
    device_name = "MCHOSE G3 A"
    identity = 0xA8A5, 0x2255
    product = "mchose g3 a"
    usage = 0xFF01, 0x0010
    size = 65
    kind = "mouse"

    def read(self, device):
        if device.write(_request(b"\x55\xed", self.size)) != self.size:
            return None
        online = _wait_reply(device, self.size, b"\xaa\xed", 250)
        if online is None or online[8] != 2 or not _drain(device, self.size):
            return None
        query = bytes.fromhex("55 30 a5 0b 2e 01 01 01")
        if device.write(_request(query, self.size)) != self.size:
            return None
        reply = _wait_reply(device, self.size, b"\xaa\x30", 350)
        if reply is None or not 1 <= reply[8] <= 100:
            return None
        charging = {0: False, 1: True}.get(reply[9])
        return reply[8], charging


class EweadnX87Provider(_ReceiverProvider):
    # 发帖者记录了该 E HUB 白名单产品、33 字节 HID 报告和内层设备信息。
    # https://github.com/Renly1994/xiaotang-daozai/issues/267#issuecomment-7181336570
    # https://github.com/chongchong2501/2.4G_hub/tree/main/docs
    name = "eweadn_x87"
    device_name = "前行者 X87"
    identity = 0x0C45, 0xFEFE
    product = "2.4g dongle"
    usage = 0xFF60, 0x0061

    def read(self, device):
        # 共用 Sonix 接收器只查设备信息，配对设备的 VID/PID 与无线模式均须回包证明。
        if device.write(_request(b"\xaa\x10\x18", self.size)) != self.size:
            return None
        reply = _wait_reply(device, self.size, b"\x55\x10\x18", 450)
        if (reply is None or reply[12:16] != b"\x45\x0c\x06\x80"
                or reply[24] != 2 or reply[25] > 100):
            return None
        charging = {0: False, 1: True, 2: True}.get(reply[26])
        return reply[25], charging
