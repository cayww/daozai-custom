"""验证同 ID 接收器的准入条件与三种型号的回包。"""

from collections import deque
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import regional_receivers
from regional_receivers import EweadnX87Provider, MchoseG3AProvider, Wave75Provider


def report(size, prefix, fields):
    data = bytearray(size - 1)
    data[:len(prefix)] = prefix
    for offset, value in fields.items():
        data[offset:offset + len(value)] = value
    return bytes(data)


class FakeDevice:
    def __init__(self, size, exchanges):
        self.size = size
        self.exchanges = deque(exchanges)
        self.input = deque()
        self.writes = []
        self.opened = []

    def open_path(self, path):
        self.opened.append(path)

    def close(self):
        pass

    def get_report_lengths(self):
        return {"input": self.size, "output": self.size}

    def set_nonblocking(self, enabled):
        assert enabled is True

    def read(self, size, timeout_ms):
        return list(self.input.popleft()) if self.input else []

    def write(self, frame):
        self.writes.append(frame)
        if self.exchanges:
            self.input.extend(self.exchanges.popleft())
        return len(frame)


WAVE_START = report(33, b"\xaa\x10\0\0", {7: b"\x55\0\xb0\x36\x09\x30"})
WAVE_BATTERY = report(33, b"\xaa\x30\0\0", {7: bytes((0x55, 93))})
G3_ONLINE = report(65, b"\xaa\xed", {8: b"\x02"})
G3_OFFLINE = report(65, b"\xaa\xed", {8: b"\x01"})
G3_BATTERY = report(65, b"\xaa\x30", {8: bytes((85, 0))})
X87_BATTERY = report(33, b"\x55\x10\x18", {
    12: b"\x45\x0c\x06\x80", 24: bytes((2, 84, 0)),
})


class RegionalReceiversTest(unittest.TestCase):
    def run_poll(self, provider, device, **changes):
        vid, pid = provider.identity
        page, usage = provider.usage
        row = dict(vendor_id=vid, product_id=pid, product_string=provider.product,
                   usage_page=page, usage=usage, interface_number=provider.interface, path=b"p")
        row.update(changes)
        hid = SimpleNamespace(enumerate=lambda *_: [row], device=lambda: device)
        with patch.object(regional_receivers, "hid", hid):
            return provider().poll()

    def test_wave75_checks_pair_and_closes_session(self):
        device = FakeDevice(33, [[WAVE_START], [WAVE_BATTERY], []])
        reading, = self.run_poll(Wave75Provider, device, product_string="WAVE75-2.4G")
        self.assertEqual(reading.level, 93)
        self.assertFalse(reading.charging_known)
        self.assertEqual([frame[1:6] for frame in device.writes],
                         [b"\xaa\x10\0\0\x18", b"\xaa\x30\0\0\x18", b"\xaa\x11\0\0\x18"])

    def test_wave75_rejects_shared_identity_and_other_paired_keyboard(self):
        device = FakeDevice(33, [])
        self.assertEqual(self.run_poll(Wave75Provider, device, product_string="URX Core75 Wireless"), [])
        self.assertEqual(self.run_poll(Wave75Provider, device, interface_number=3), [])
        self.assertEqual(device.opened, [])
        wrong_pair = report(33, b"\xaa\x10\0\0", {7: b"\x55\0\xb0\x36\x04\x30"})
        device = FakeDevice(33, [[wrong_pair], []])
        self.assertEqual(self.run_poll(Wave75Provider, device), [])
        self.assertEqual([frame[2] for frame in device.writes], [0x10, 0x11])

    def test_g3a_requires_online_reply_before_fresh_battery(self):
        device = FakeDevice(65, [[G3_ONLINE], [G3_BATTERY]])
        device.input.append(report(65, b"\xaa\x30", {8: bytes((96, 1))}))
        reading, = self.run_poll(MchoseG3AProvider, device, product_string="MCHOSE G3 A")
        self.assertEqual((reading.level, reading.charging, reading.charging_known), (85, False, True))
        self.assertEqual([frame[:3] for frame in device.writes], [b"\0\x55\xed", b"\0\x55\x30"])
        for online in ([], [G3_OFFLINE], [G3_BATTERY]):
            with self.subTest(online=online):
                device = FakeDevice(65, [online])
                self.assertEqual(self.run_poll(MchoseG3AProvider, device), [])
                self.assertEqual(len(device.writes), 1)

    def test_g3a_does_not_probe_g7_or_other_collections(self):
        device = FakeDevice(65, [])
        self.assertEqual(self.run_poll(MchoseG3AProvider, device, product_string="YJX-CHIP"), [])
        self.assertEqual(self.run_poll(MchoseG3AProvider, device, usage=1), [])
        self.assertEqual(device.opened, [])
        device = FakeDevice(33, [])
        self.assertEqual(self.run_poll(MchoseG3AProvider, device), [])
        self.assertEqual(device.writes, [])

    def test_x87_requires_the_paired_keyboard_and_wireless_mode(self):
        device = FakeDevice(33, [[X87_BATTERY]])
        reading, = self.run_poll(EweadnX87Provider, device, product_string="2.4G Dongle")
        self.assertEqual((reading.level, reading.charging, reading.charging_known), (84, False, True))
        self.assertEqual(device.writes[0][:5], b"\0\xaa\x10\x18\0")
        for offset, invalid in ((12, 0), (14, 7), (24, 1), (25, 101)):
            with self.subTest(offset=offset):
                wrong = bytearray(X87_BATTERY)
                wrong[offset] = invalid
                self.assertEqual(self.run_poll(EweadnX87Provider, FakeDevice(33, [[wrong]])), [])
        device = FakeDevice(33, [])
        self.assertEqual(self.run_poll(EweadnX87Provider, device, product_string="GS3075T 2.4G"), [])
        self.assertEqual(device.opened, [])


if __name__ == "__main__":
    unittest.main()
