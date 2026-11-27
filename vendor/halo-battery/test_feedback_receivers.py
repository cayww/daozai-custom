"""实机报告的电量回放及相邻接收器拒绝测试。"""

from collections import deque
import sys
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import patch

from worker import Engine, Gateway, Receiver, ScopedProvider

with patch.dict(sys.modules, {"hid": ModuleType("hid")}):
    import feedback_receivers as readers


L8 = readers.MachenikeL8MaxProvider
DRAGONFLY = readers.VgnDragonfly3ProProvider
EK87 = readers.DareuEk87ProProvider


def report(hex_prefix, size):
    return bytes.fromhex(hex_prefix).ljust(size, b"\0")


def row(provider, path=b"control", **changes):
    string_key, expected_string = provider.identity_string
    return {
        "path": path, "vendor_id": provider.receiver[0], "product_id": provider.receiver[1],
        "interface_number": provider.interfaces[0], "usage_page": provider.usage[0],
        "usage": provider.usage[1], string_key: expected_string.upper(), **changes,
    }


class FakeDevice:
    def __init__(self, caps, replies=(), pending=()):
        self.caps = dict(zip(("input", "output", "feature"), caps))
        self.replies = deque(replies)
        self.pending = deque(pending)
        self.opened = []
        self.writes = []
        self.reads = []
        self.closed = 0
        self.nonblocking = False

    def open_path(self, path):
        self.opened.append(path)

    def close(self):
        self.closed += 1

    def get_report_lengths(self):
        return self.caps

    def set_nonblocking(self, enabled):
        self.nonblocking = enabled

    def write(self, data):
        self.writes.append(("output", bytes(data)))
        return len(data)

    def send_feature_report(self, data):
        self.writes.append(("feature", bytes(data)))
        return len(data)

    def read(self, size, timeout_ms):
        self.reads.append(("interrupt", size, timeout_ms))
        if self.nonblocking:
            return self.pending.popleft() if self.pending else b""
        return self.replies.popleft() if self.replies else b""

    def get_feature_report(self, report_id, size):
        self.reads.append(("feature", report_id, size))
        return self.replies.popleft() if self.replies else b""


class FeedbackReceiversTests(unittest.TestCase):
    def poll(self, provider, device, *rows):
        hid = SimpleNamespace(enumerate=lambda *_: rows, device=lambda: device)
        with patch.object(readers, "hid", hid):
            return provider().poll()

    def test_l8_tlv_uses_paired_mouse_identity_and_skips_unsolicited_packets(self):
        # 原始反馈提供记录结构和 100% 读数，用可核对的结构组合回放。
        reply = report("A1 02 04 01 00 38 02 11 43 00 9B 36 02 13 41 42 43 00 02 25 64 02 26 01", 64)
        device = FakeDevice((64, 64, 0), [
            report("A1 50 04 01 00 38 02 25 64", 64),
            reply[:8] + b"\x44" + reply[9:],
            reply,
        ])
        status, = self.poll(L8, device, row(L8, interface_number=-1))
        self.assertEqual((status.level, status.charging_known), (100, False))
        self.assertEqual(device.writes, [("output", report("A0 02", 64))])
        self.assertEqual(device.closed, 1)

    def test_l8_rejects_unrecognized_record_and_invalid_battery(self):
        for reply in (
            "A1 02 04 01 00 38 02 11 43 00 9B 36 02 43 02 25 12",
            "A1 02 04 01 00 38 02 11 43 00 9B 36 02 25 65",
            "A1 02 04 01 00 38 02 25 01 02 25 64",
            "A1 02 04 01 00 38 02 25 15",
        ):
            with self.subTest(reply=reply):
                device = FakeDevice((64, 64, 0), [report(reply, 64)])
                self.assertEqual(self.poll(L8, device, row(L8)), [])

    def test_l8_valid_prefix_survives_unknown_trailing_record_and_feature_caps(self):
        reply = report("A1 02 04 01 00 38 02 11 43 00 9B 36 02 25 64 02 27 01", 64)
        device = FakeDevice((64, 64, 64), [reply])
        status, = self.poll(L8, device, row(L8))
        self.assertEqual(status.level, 100)

    def test_dragonfly_ignores_wrong_protocol_and_wired_sentinel(self):
        device = FakeDevice((65, 65, 0), [
            report("55 30 A5 2E 0A 00 00 00 4A 00", 64),
            report("AA 30 A5 2E 0A 00 00 00 4A 01", 64),
            report("AA 30 A5 2E 0A 00 00 00 00 00", 64),
            report("AA 30 A5 2E 0A 00 00 00 4A 00", 64),
        ])
        status, = self.poll(DRAGONFLY, device, row(DRAGONFLY),
                            row(DRAGONFLY, path=b"3-se", product_id=0x1A01))
        self.assertEqual((status.level, status.charging, status.charging_known), (74, False, True))
        self.assertEqual(device.writes, [("output", report("00 55 30 A5 2E 2E", 65))])
        self.assertEqual(device.opened, [b"control"])

    def test_interrupt_read_drains_old_valid_reply_and_bounds_unrelated_frames(self):
        stale = report("AA 30 A5 2E 0A 00 00 00 11 00", 64)
        fresh = report("AA 30 A5 2E 0A 00 00 00 4A 00", 64)
        other = report("AA 31 A5 2E 0A 00 00 00 4A 00", 64)
        device = FakeDevice((65, 65, 0), [fresh], pending=[stale])
        status, = self.poll(DRAGONFLY, device, row(DRAGONFLY))
        self.assertEqual(status.level, 74)
        self.assertEqual([r[2] for r in device.reads[:2]], [0, 0])
        self.assertGreater(device.reads[2][2], 0)
        device = FakeDevice((65, 65, 0), [fresh], pending=[other] * 4 + [stale])
        self.assertEqual(self.poll(DRAGONFLY, device, row(DRAGONFLY)), [])
        self.assertEqual(sum(r[2] == 0 for r in device.reads), 4)
        self.assertEqual(device.writes, [])
        device = FakeDevice((65, 65, 0), [other] * 8 + [fresh])
        self.assertEqual(self.poll(DRAGONFLY, device, row(DRAGONFLY)), [])
        self.assertEqual(sum(r[2] > 0 for r in device.reads), 8)

    def test_dareu_feature_retries_wrong_address_and_checks_battery_fields(self):
        invalid_address = report("00 12 03 07 81 00 00 00 5F 64", 65)
        invalid_full_scale = report("00 12 03 07 80 00 00 00 5F FF", 65)
        valid = report("00 12 03 07 80 00 00 00 5F 64", 65)
        device = FakeDevice((0, 0, 65), [invalid_address, invalid_full_scale, valid])
        status, = self.poll(EK87, device, row(EK87), row(EK87, b"input-only", interface_number=2))
        self.assertEqual((status.level, status.charging, status.charging_known), (95, False, True))
        self.assertEqual(device.writes, [("feature", report("00 10 03 07 80", 65))] * 3)
        self.assertEqual(device.reads, [("feature", 0, 65)] * 3)
        self.assertEqual(device.closed, 1)

    def test_dareu_preserves_unknown_charging_and_limits_retries(self):
        valid = report("00 12 03 07 80 00 00 02 5F 64", 65)
        device = FakeDevice((0, 0, 65), [valid])
        status, = self.poll(EK87, device, row(EK87))
        self.assertEqual((status.level, status.charging_known), (95, False))
        device = FakeDevice((0, 0, 65), [b""] * 3 + [valid])
        self.assertEqual(self.poll(EK87, device, row(EK87)), [])
        self.assertEqual(len(device.writes), 3)

    def test_all_reject_ambiguous_collections_and_wrong_report_lengths_without_writes(self):
        for provider in (L8, DRAGONFLY, EK87):
            with self.subTest(provider=provider.name):
                device = FakeDevice((1, 1, 1))
                self.assertEqual(self.poll(provider, device, row(provider), row(provider, b"duplicate")), [])
                self.assertEqual(device.opened, [])
                self.assertEqual(self.poll(provider, device, row(provider)), [])
                self.assertEqual(device.opened, [b"control"])
                self.assertEqual(device.writes, [])
                self.assertEqual(device.closed, 1)

    def test_shared_ids_with_different_or_missing_identity_are_never_opened(self):
        for provider in (L8, DRAGONFLY, EK87):
            for wrong_string in (None, "other USB device"):
                with self.subTest(provider=provider.name, value=wrong_string):
                    device = FakeDevice(provider.report_lengths)
                    wrong = row(provider, **{provider.identity_string[0]: wrong_string})
                    self.assertEqual(self.poll(provider, device, wrong), [])
                    self.assertEqual(device.opened, [])
                    self.assertEqual(device.writes, [])

    def test_only_exact_usb_parent_is_opened_when_two_identical_receivers_present(self):
        a, b = [Receiver(*DRAGONFLY.receiver, rf"USB\VID_391D&PID_1A03\{suffix}")
                for suffix in ("FIRST", "SECOND")]
        devices = {
            receiver.identity: FakeDevice((65, 65, 0), [
                report(f"AA 30 A5 2E 0A 00 00 00 {percentage:02X} 00", 64),
            ])
            for receiver, percentage in ((a, 74), (b, 89))
        }
        windows = SimpleNamespace(
            paths_by_receiver=lambda items: {r.identity: (r.identity,) for r in items},
            parent_for=lambda path: path,
            metadata=lambda path, root: row(DRAGONFLY, path.encode()),
        )
        transport = SimpleNamespace(invoke=lambda action, _: action(),
                                    device=lambda: devices[gate.local.scope.root.identity])
        gate = Gateway(transport, windows)
        hid = ModuleType("hid")
        gate.install(hid)
        gate.update([a, b])
        with patch.object(readers, "hid", hid):
            statuses = ScopedProvider(DRAGONFLY, gate).poll()
        engine = Engine.__new__(Engine)
        engine.gate, engine.observed = gate, {}
        projected = engine.project(statuses, {s.key for s in statuses})
        self.assertEqual([(item["usb_instance"], item["level"]) for item in projected],
                         [(a.instance, 74), (b.instance, 89)])
        self.assertNotEqual(projected[0]["key"], projected[1]["key"])


if __name__ == "__main__":
    unittest.main()
