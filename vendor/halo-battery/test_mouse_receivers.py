"""雷柏和 VGN 实测接收器回包及 HID 路由测试。"""

from collections import deque
import sys
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import patch

from worker import Engine, Gateway, Receiver, ScopedProvider

with patch.dict(sys.modules, {"hid": ModuleType("hid")}):
    import mouse_receivers


RAPOO_REPLY = bytes.fromhex("01 01 5E") + bytes(28)
VGN_REPLY = bytes.fromhex("08 04 00 00 00 02 5F 00 10 05 00 00 00 00 00 00 D3")
VGN_QUERY = bytes.fromhex("08 04") + bytes(14) + b"\x49"


class FakeDevice:
    def __init__(self, output_size, input_size, replies):
        self.caps = {"input": input_size, "output": output_size}
        self.replies = deque(replies)
        self.opened = []
        self.writes = []
        self.reads = []
        self.closed = 0

    def open_path(self, path):
        self.opened.append(path)

    def close(self):
        self.closed += 1

    def get_report_lengths(self):
        return self.caps

    def write(self, data):
        self.writes.append(bytes(data))
        return len(data)

    def read(self, size, timeout_ms):
        self.reads.append(("interrupt", size, timeout_ms))
        return self.replies.popleft() if self.writes and self.replies else b""

    def get_input_report(self, report_id, size):
        self.reads.append(("input_report", report_id, size))
        reply = self.replies.popleft() if self.replies else b""
        if isinstance(reply, OSError):
            raise reply
        return reply


def metadata(vid, pid, page, usage, path="control", interface=1):
    return {"path": path.encode(), "vendor_id": vid, "product_id": pid, "interface_number": interface,
            "usage_page": page, "usage": usage, "product_string": (
                "Rapoo Gaming Device" if vid == 0x24AE else "VGN Mouse 2.4G Receiver"
            )}


class MouseReceiversTests(unittest.TestCase):
    def test_rapoo_retries_control_input_and_accepts_windows_and_prefixed_reports(self):
        provider = mouse_receivers.RapooVt9AirProvider
        rows = [metadata(0x24AE, 0x1200, 0xFF00, 0x000E),
                metadata(0x24AE, 0x1200, 0xFF00, 0x000E, "wrong-interface", 0)]
        for frame in (RAPOO_REPLY, b"\xBA" + RAPOO_REPLY):
            with self.subTest(frame=frame):
                device = FakeDevice(32, 32, [OSError("无缓冲"), b"\x01\x00\x30", b"", b"", frame])
                hid = SimpleNamespace(enumerate=lambda *_: rows, device=lambda: device)
                with patch.object(mouse_receivers, "hid", hid), patch.object(mouse_receivers, "sleep"):
                    result, = provider().poll()
                self.assertEqual((result.level, result.charging), (94, False))
                self.assertEqual(device.opened, [b"control"])
                self.assertEqual(device.writes, [b"\xBA\x00\xAA\x00" + bytes(28)] * 2)
                self.assertEqual(device.reads, [("input_report", 0xBA, 32)] * 5)
                self.assertEqual(device.closed, 1)

    def test_rapoo_accepts_charging_but_refuses_other_status_or_oversized_level(self):
        rows = [metadata(0x24AE, 0x1200, 0xFF00, 0x000E)]
        for reply, expected in ((b"\xBA\x01\x02\x00", [(0, True)]),
                                (b"\x01\x01\x65", []), (b"\x00\x01\x5E", []),
                                (b"\x01\x00\x5E", []), (b"\x01\x01", [])):
            with self.subTest(reply=reply):
                device = FakeDevice(32, 32, [reply])
                hid = SimpleNamespace(enumerate=lambda *_: rows, device=lambda: device)
                with patch.object(mouse_receivers, "hid", hid), patch.object(mouse_receivers, "sleep"):
                    results = mouse_receivers.RapooVt9AirProvider().poll()
                self.assertEqual([(r.level, r.charging) for r in results], expected)
                self.assertLessEqual(len(device.writes), 4)

    def test_vgn_validates_reply_and_ignores_events_echo_bad_checksum(self):
        rows = [metadata(0x3554, 0xF503, 0xFF02, 0x0002),
                metadata(0x3554, 0xF505, 0xFF02, 0x0002, "adjacent-pid"),
                metadata(0x3554, 0xF503, 0xFF02, 0x0002, "wrong-interface", 2)]
        device = FakeDevice(17, 17, [b"\x08\x03" + bytes(15), VGN_QUERY,
                                    VGN_REPLY[:-1] + b"\xD4", VGN_REPLY])
        hid = SimpleNamespace(enumerate=lambda *_: rows, device=lambda: device)
        with patch.object(mouse_receivers, "hid", hid):
            result, = mouse_receivers.VgnF1MobaProvider().poll()
        self.assertEqual((result.level, result.charging), (95, False))
        self.assertEqual(device.opened, [b"control"])
        self.assertEqual(device.writes, [VGN_QUERY])
        self.assertTrue(all(read[0] == "interrupt" for read in device.reads))

    def test_rejects_multiple_candidates_and_incompatible_lengths_before_writing(self):
        for provider, vid, pid, page, usage, size in (
            (mouse_receivers.RapooVt9AirProvider, 0x24AE, 0x1200, 0xFF00, 0x000E, 32),
            (mouse_receivers.VgnF1MobaProvider, 0x3554, 0xF503, 0xFF02, 0x0002, 17),
        ):
            with self.subTest(provider=provider.name):
                row = metadata(vid, pid, page, usage)
                duplicate = metadata(vid, pid, page, usage, "second-control")
                device = FakeDevice(size + 1, size, [])
                hid = SimpleNamespace(enumerate=lambda *_: [row, duplicate], device=lambda: device)
                with patch.object(mouse_receivers, "hid", hid):
                    self.assertEqual(provider().poll(), [])
                    self.assertEqual(device.opened, [])
                    hid.enumerate = lambda *_: [{**row, "product_string": "普通接收器"}]
                    self.assertEqual(provider().poll(), [])
                    self.assertEqual(device.opened, [])
                    hid.enumerate = lambda *_: [row]
                    self.assertEqual(provider().poll(), [])
                self.assertEqual(device.opened, [b"control"])
                self.assertEqual(device.writes, [])
                self.assertEqual(device.closed, 1)

    def test_gateway_keeps_two_same_pid_physical_receivers_separate(self):
        first = Receiver(0x3554, 0xF503, r"USB\VID_3554&PID_F503\FIRST")
        second = Receiver(0x3554, 0xF503, r"USB\VID_3554&PID_F503\SECOND")
        nearby = Receiver(0x3554, 0xF502, r"USB\VID_3554&PID_F502\NEARBY")
        devices = {root.identity: FakeDevice(17, 17, [VGN_REPLY]) for root in (first, second, nearby)}
        windows = SimpleNamespace(
            paths_by_receiver=lambda receivers: {r.identity: (r.identity,) for r in receivers},
            parent_for=lambda path: path,
            metadata=lambda path, root: metadata(root.vid, root.pid, 0xFF02, 0x0002, path),
        )
        transport = SimpleNamespace(invoke=lambda action, _: action(),
                                    device=lambda: devices[gate.local.scope.root.identity])
        gate = Gateway(transport, windows)
        hid = ModuleType("hid")
        gate.install(hid)
        gate.update([first, second, nearby])
        with patch.object(mouse_receivers, "hid", hid):
            statuses = ScopedProvider(mouse_receivers.VgnF1MobaProvider, gate).poll()
        engine = Engine.__new__(Engine)
        engine.gate, engine.observed = gate, {}
        replies = engine.project(statuses, {status.key for status in statuses})
        self.assertEqual([r["usb_instance"] for r in replies], [first.instance, second.instance])
        self.assertEqual([r["level"] for r in replies], [95, 95])
        self.assertNotEqual(replies[0]["key"], replies[1]["key"])
        self.assertTrue(all(r["fresh"] for r in replies))
        self.assertEqual(devices[nearby.identity].opened, [])


if __name__ == "__main__":
    unittest.main()
