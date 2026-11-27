"""验证 A7 的真实输入门控、休眠前的最后发送边界和共用接收器分流。"""

import sys
import threading
import unittest
from types import ModuleType, SimpleNamespace
from unittest.mock import Mock, patch

with patch.dict(sys.modules, {"hid": ModuleType("hid")}):
    from worker import AUXILIARY_FIELDS, Engine, Gateway, Receiver, ScopedProvider
    import mchose_a7
    from halo.hid_transport import HidDevice
    from halo.polling import PollingRuntime
    from provider_registry import MchoseReceiverProvider
    from providers.base import Provider


CONTROL = {"path": b"control", "interface_number": 2, "usage_page": 0xFF01, "usage": 1,
           "product_string": "MCHOSE A7 V3 Ultra+", "mc_report_verified": True}
MOUSE = {"path": b"mouse", "usage_page": 1, "usage": 2}


def response(level, charging=False):
    # 以公开实测的帧结构构造，1018 的完整原始回包没有公开。
    payload = bytes.fromhex("37 38 26 40") + bytes(7) + bytes((charging, level))
    packet = bytes.fromhex("4d 01 01 0d 00 09 00 00") + payload
    return (packet + bytes((mchose_a7.mchose._mc_xor(packet[2:]),))).ljust(64, b"\0")


class A7ActivityGateTests(unittest.TestCase):
    def setUp(self):
        self.now = 100.0
        self.rows = [CONTROL, MOUSE]
        self.input = Mock()
        self.input.latest.return_value = None
        self.activity = Mock()
        self.activity.subscribe.return_value = self.input
        self.device = Mock()
        self.device.get_report_lengths.return_value = {"input": 64, "output": 64}
        self.device.write.return_value = 64
        self.device.read.side_effect = lambda _, timeout: response(84) if timeout else []
        self.hid = SimpleNamespace(enumerate=lambda *_: self.rows, device=Mock(return_value=self.device))
        self.enterContext(patch.object(mchose_a7, "ACTIVITY", self.activity))
        self.enterContext(patch.object(mchose_a7, "time", SimpleNamespace(monotonic=lambda: self.now)))
        self.enterContext(patch.object(mchose_a7.mchose, "hid", self.hid))

    def test_heartbeat_does_not_query_until_mouse_activity_and_idle_stays_quiet(self):
        provider = mchose_a7.MchoseA7Provider()
        provider.wake = Mock()
        self.assertEqual(provider.poll(), [])
        self.hid.device.assert_not_called()
        paths, activity_callback = self.activity.subscribe.call_args.args
        self.assertEqual(paths, frozenset({b"mouse"}))

        self.input.latest.return_value = self.now
        activity_callback()
        provider.wake.assert_called_once_with()
        first, = provider.poll()
        self.assertEqual(first.level, 84)
        self.assertEqual(first.ui_params[AUXILIARY_FIELDS[2]], 100.0)
        self.device.write.assert_called_once_with(mchose_a7.mchose.make_mc_request(), deadline=100.25)

        for heartbeat in range(160, 1200, 60):
            self.now = float(heartbeat)
            reading, = provider.poll()
            self.assertEqual(reading.ui_params[AUXILIARY_FIELDS[2]], 100.0)
        self.assertEqual(self.device.open_path.call_count, 1)
        self.assertEqual(self.device.write.call_count, 1)

        self.input.latest.return_value = self.now
        self.device.read.side_effect = lambda _, timeout: response(72, True) if timeout else []
        newer, = provider.poll()
        self.assertEqual((newer.level, newer.charging), (72, True))
        self.assertEqual(self.device.write.call_count, 2)

    def test_expired_event_and_missing_report_proof_produce_no_write(self):
        provider = mchose_a7.MchoseA7Provider()
        self.rows = [{**CONTROL, "mc_report_verified": False}, MOUSE]
        self.input.latest.return_value = self.now
        self.assertEqual(provider.poll(), [])
        self.activity.subscribe.assert_not_called()
        self.rows = [CONTROL, MOUSE]
        self.now += 0.251
        self.assertEqual(provider.poll(), [])
        self.hid.device.assert_not_called()

    def test_packet_is_not_dispatched_when_native_queue_passes_deadline(self):
        device = HidDevice.__new__(HidDevice)
        device._require_open = Mock()
        device._caps = SimpleNamespace(output=64)
        device._invoke = lambda operation: operation()
        device._api = Mock()
        with patch("halo.hid_transport.time.monotonic", return_value=100.26):
            self.assertEqual(device.write(mchose_a7.mchose.make_mc_request(), deadline=100.25), 0)
        device.api.create_write.assert_not_called()

    def test_ambiguous_or_other_1018_products_cannot_use_old_periodic_reader(self):
        self.hid.enumerate = lambda _, pid: self.rows if pid == 0x1018 else []
        provider = MchoseReceiverProvider()
        with patch.object(mchose_a7.mchose.MchoseProvider, "poll", return_value=["K7"]) as legacy:
            for model in ("MCHOSE A7 V3 Ultra+", "MCHOSE A7 V3 Pro", "", "通用接收器"):
                self.rows = [{**CONTROL, "product_string": model}, MOUSE]
                self.assertEqual(provider.poll(), [])
            legacy.assert_not_called()
            self.rows = [{**CONTROL, "product_string": "MCHOSE K7 V2 Ultra+"}, MOUSE]
            self.assertEqual(provider.poll(), ["K7"])
            legacy.assert_called_once_with()

    def test_activity_sample_keeps_its_real_age_across_later_host_poll(self):
        receiver = Receiver(0x3837, 0x1018, r"USB\VID_3837&PID_1018\RADIO")
        windows = SimpleNamespace(paths_by_receiver=lambda _: {receiver.identity: ("mouse", "control")})
        gate = Gateway(None, windows)
        gate.update([receiver])
        # 模拟宿主 60 秒后才取样，后一次定时检查没有发生新的设备读取。
        self.input.latest.return_value = 100.0
        job = ScopedProvider(mchose_a7.MchoseA7Provider, gate)
        job.wake = lambda: None
        job.poll()
        self.now = 160.0
        engine = Engine.__new__(Engine)
        engine.gate, engine.observed = gate, {}
        statuses = job.poll()
        with patch("worker.time.monotonic", return_value=160.0):
            result, = engine.project(statuses, {status.key for status in statuses})
        self.assertEqual((result["level"], result["age_ms"]), (84, 60000))


class ActivitySchedulingTests(unittest.TestCase):
    def test_activity_poll_queues_only_one_provider(self):
        class Poll(Provider):
            def __init__(self, name):
                self.name = name
                self.called = threading.Event()
                self.poll = Mock(side_effect=self._sample)

            def _sample(self):
                self.called.set()
                return []

        a7, other = Poll("sample"), Poll("unrelated")
        runtime = PollingRuntime((a7, other), lambda _: None)
        try:
            runtime.configure(())
            runtime.request_poll(provider=a7)
            self.assertTrue(a7.called.wait(2))
            self.assertTrue(runtime.close(timeout=2))
            a7.poll.assert_called_once_with()
            other.poll.assert_not_called()
        finally:
            runtime.close()
