"""可选采集进程的实例归属和读数语义测试。"""
import io
import json
import sys
import time
import unittest
from types import ModuleType, SimpleNamespace
from unittest.mock import patch

from worker import Engine, Gateway, Receiver, ScopedDevice, ScopedProvider, read_request, write_reply
from providers.base import DeviceStatus


FIRST = Receiver(0x0B05, 0x1ACE, r"USB\VID_0B05&PID_1ACE\FIRST")
SECOND = Receiver(0x0B05, 0x1ACE, r"USB\VID_0B05&PID_1ACE\SECOND")


class FakeWindows:
    def parent_for(self, path):
        return {"first": FIRST.identity, "second": SECOND.identity}.get(path)

    def paths_by_receiver(self, receivers):
        return {receiver.identity: frozenset([path]) for receiver, path in
                ((FIRST, "first"), (SECOND, "second")) if receiver in receivers}

    def metadata(self, path, receiver):
        return {"path": path, "vendor_id": receiver.vid, "product_id": receiver.pid}


class FakeTransport:
    def device(self):
        return SimpleNamespace(open_path=lambda path: None, read=lambda: [1], close=lambda: None)

    def invoke(self, action, _description):
        return action()


class WorkerTests(unittest.TestCase):
    def test_requires_explicit_parent_identity(self):
        request = {"version": 1, "id": 42, "command": "poll", "receivers": [
            {"vid": FIRST.vid, "pid": FIRST.pid, "usb_instance": FIRST.instance}]}
        self.assertEqual(read_request(json.dumps(request)), (42, [FIRST]))
        request["receivers"][0]["pid"] = 1
        with self.assertRaises(ValueError):
            read_request(json.dumps(request))

    def test_limits_discovery_and_open_to_the_same_receiver(self):
        gate = Gateway(FakeTransport(), FakeWindows())
        gate.update([FIRST, SECOND])
        with gate.scope(gate.inventory[0][FIRST.identity]):
            self.assertEqual([row["path"] for row in gate.enumerate()], ["first"])
            handle = ScopedDevice(gate)
            with self.assertRaises(OSError):
                handle.open_path("second")
            handle.open_path("first")
            self.assertEqual(handle.read(), [1])
            gate.update([SECOND])
            with self.assertRaises(OSError):
                handle.read()

    def test_keeps_charge_only_coarse_and_actual_age(self):
        gate = Gateway(FakeTransport(), FakeWindows())
        gate.update([FIRST, SECOND])
        engine = Engine.__new__(Engine)
        engine.gate, engine.observed = gate, {}
        now = time.monotonic()
        first_generation = gate.inventory[0][FIRST.identity].generation
        second_generation = gate.inventory[0][SECOND.identity].generation

        charge = DeviceStatus("charge-only", "headset", 80, True, approx="on cable, charging",
                              ui_params={"daozai_instance": FIRST.identity, "daozai_generation": first_generation,
                                         "daozai_observed": now - 30})
        coarse = DeviceStatus("coarse", "mouse", 55, False, approx="about 55% (medium)",
                              ui_params={"daozai_instance": SECOND.identity, "daozai_generation": second_generation,
                                         "daozai_observed": now - 20})

        first = engine.project([charge, coarse], {"charge-only", "coarse"})
        self.assertEqual([device["usb_instance"] for device in first], [FIRST.instance, SECOND.instance])
        self.assertIsNone(first[0]["level"])
        self.assertTrue(first[0]["charging"])
        self.assertTrue(first[0]["fresh"])
        self.assertEqual(first[1]["level"], 55)
        self.assertTrue(first[1]["approximate"])
        again = engine.project([charge, coarse], set())
        self.assertGreaterEqual(again[0]["age_ms"], 30_000)
        self.assertGreaterEqual(again[1]["age_ms"], 20_000)

        gate.update([SECOND])
        gate.update([FIRST, SECOND])
        self.assertEqual([device["key"] for device in engine.project([charge, coarse], set())], ["coarse"])

    def test_non_hid_receivers_are_scoped_even_without_hid_interfaces(self):
        sony = Receiver(0x054C, 0x0E53, r"USB\VID_054C&PID_0E53\SONY")
        dji = Receiver(0x2CA3, 0x4011, r"USB\VID_2CA3&PID_4011\DJI")
        dji_second = Receiver(0x2CA3, 0x4011, r"USB\VID_2CA3&PID_4011\DJI-SECOND")
        closed = []
        sampled_at = time.monotonic() - 15

        class Sony:
            name = "sony"
            target = (0x054C, 0x0E53)

            def __init__(self, receiver):
                self.receiver = receiver

            def poll(self):
                return [DeviceStatus("sony", self.receiver.instance, 35)]

        class Dji(Sony):
            name = "dji"
            target = (0x2CA3, 0x4011)

            def poll(self):
                return [DeviceStatus("mic", self.receiver.instance, 80,
                                     ui_params={"daozai_observed": sampled_at})]

            def close(self):
                closed.append(self.receiver)

        gate = Gateway(FakeTransport(), FakeWindows(), (Sony.target, Dji.target))
        gate.update([FIRST, sony, dji, dji_second])
        engine = Engine.__new__(Engine)
        engine.gate, engine.observed = gate, {}
        sony_provider = ScopedProvider(Sony, gate)
        dji_provider = ScopedProvider(Dji, gate)
        rows = sony_provider.poll() + dji_provider.poll()
        projected = engine.project(rows, {row.key for row in rows})
        self.assertEqual([device["usb_instance"] for device in projected],
                         [sony.instance, dji.instance, dji_second.instance])
        self.assertEqual([device["level"] for device in projected], [35, 80, 80])
        self.assertNotEqual(projected[1]["key"], projected[2]["key"])
        self.assertGreaterEqual(projected[1]["age_ms"], 15_000)

        gate.update([FIRST, sony, dji_second])
        self.assertEqual(len(dji_provider.poll()), 1)
        self.assertEqual(closed, [dji])
        self.assertFalse(any(value["usb_instance"] == dji.instance for value in
                             engine.project(rows, set())))

    def test_turbo_plus_limits_hid_to_the_verified_physical_interface(self):
        radio = Receiver(0x3837, 0x600A, r"USB\VID_3837&PID_600A\RADIO")
        second = Receiver(0x3837, 0x600A, r"USB\VID_3837&PID_600A\SECOND")
        sibling = Receiver(0x3837, 0x6008, r"USB\VID_3837&PID_6008\SIBLING")
        x9 = Receiver(0x3837, 0x6045, r"USB\VID_3837&PID_6045\X9")
        devices = {
            "radio-correct": (radio, 0, 0xFF90, 1),
            "radio-wrong-mi": (radio, 1, 0xFF90, 1),
            "radio-wrong-page": (radio, 0, 0xFF82, 1),
            "radio-wrong-usage": (radio, 0, 0xFF90, 2),
            "second-correct": (second, 0, 0xFF90, 1),
            "sibling": (sibling, 0, 0xFF90, 1),
            "x9": (x9, 0, 0xFF90, 1),
        }
        opened, sent = [], []
        replies = {"radio-correct": bytes.fromhex("55 65 64 02").ljust(64, b"\0"),
                   "second-correct": bytes.fromhex("55 65 25 03").ljust(64, b"\0"),
                   "x9": bytes.fromhex("55 65 28 03").ljust(64, b"\0")}

        class Native:
            def open_path(self, path):
                self.path = path.decode()
                opened.append(self.path)

            def get_report_lengths(self):
                return {"input": 64, "output": 64, "feature": 0}

            def write(self, data):
                sent.append((self.path, bytes(data)))
                return len(data)

            def read(self, size, timeout):
                return replies.pop(self.path, b"")

            def close(self):
                pass

        class Windows:
            def parent_for(self, path):
                return devices[path][0].identity

            def paths_by_receiver(self, receivers):
                return {receiver.identity: tuple(path for path, info in devices.items()
                                                if receiver == info[0]) for receiver in receivers}

            def metadata(self, path, receiver):
                _, interface, page, usage = devices[path]
                return {"path": path.encode(), "vendor_id": receiver.vid, "product_id": receiver.pid,
                        "interface_number": interface, "usage_page": page, "usage": usage}

        class Transport(FakeTransport):
            def device(self):
                return Native()

        gate = Gateway(Transport(), Windows())
        hid = ModuleType("hid")
        gate.install(hid)
        with patch.dict(sys.modules, {"hid": hid}):
            from provider_registry import MchoseReceiverProvider, PROVIDERS
            from providers import mchose

        self.assertIn(MchoseReceiverProvider, PROVIDERS)
        self.assertNotIn(mchose.MchoseProvider, PROVIDERS)
        provider = ScopedProvider(MchoseReceiverProvider, gate)
        engine = Engine.__new__(Engine)
        engine.gate, engine.observed = gate, {}
        request = {"version": 1, "id": 42, "command": "poll", "receivers": [
            {"vid": root.vid, "pid": root.pid, "usb_instance": root.instance}
            for root in (radio, second, sibling, x9)]}
        request_id, targets = read_request(json.dumps(request))

        with patch.object(mchose, "hid", hid), patch.object(mchose.hidlist, "enumerate", gate.enumerate):
            gate.update(targets)
            statuses = provider.poll()
            reply = io.BytesIO()
            write_reply(reply, {"version": 1, "id": request_id, "devices":
                                engine.project(statuses, {status.key for status in statuses})})
            rows = json.loads(reply.getvalue())["devices"]
            self.assertEqual([(row["usb_instance"], row["level"], row["charging"]) for row in rows],
                             [(radio.instance, 100, None), (second.instance, 37, None), (x9.instance, 40, True)])
            self.assertTrue(all(row["fresh"] for row in rows))
            self.assertNotEqual(rows[0]["key"], rows[1]["key"])
            self.assertEqual(opened, ["radio-correct", "second-correct", "x9"])
            self.assertEqual(sent, [(path, b"\x55\x65\x01".ljust(64, b"\0")) for path in opened])

            # 拔出一只同型号接收器，旧回复不得进入另一只接收器的快照。
            gate.update([second])
            self.assertEqual([row["usb_instance"] for row in engine.project(statuses, set())], [second.instance])

            # 已确认用途的集合出现歧义或消失时，不能选择近似集合。
            opened.clear()
            devices["second-duplicate"] = devices["second-correct"]
            gate.update([second])
            self.assertEqual(provider.poll(), [])
            del devices["second-correct"], devices["second-duplicate"]
            gate.update([second])
            self.assertEqual(provider.poll(), [])
            self.assertEqual(opened, [])


    def test_g3_a_does_not_fall_through_to_the_shared_g7_reader(self):
        hid = ModuleType("hid")
        with patch.dict(sys.modules, {"hid": hid}):
            from provider_registry import MchoseReceiverProvider
            from providers import mchose

        row = {"vendor_id": 0xA8A5, "product_id": 0x2255, "product_string": "MCHOSE G3 A"}
        hid.enumerate = lambda vid, pid: [row] if (vid, pid) == (0xA8A5, 0x2255) else []
        with patch.object(mchose, "hid", hid), patch.object(mchose.MchoseProvider, "poll") as legacy:
            provider = MchoseReceiverProvider()
            self.assertEqual(provider.poll(), [])
            legacy.assert_not_called()
            row["product_string"] = "MCHOSE G7"
            provider.poll()
            legacy.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()
