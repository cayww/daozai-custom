"""Sony INZONE H9 串口的归属、回复和失败处理测试。"""

from collections import deque
from contextlib import contextmanager
import ctypes
import os
import unittest

from worker import Receiver
from sony_serial import SonySerialProvider, _Dcb, _PropertyKey, _Timeouts, _WindowsSerial
from providers.sony_inzone import INIT_1, INIT_2, QUERY, SIGNATURE


FIRST = Receiver(0x054C, 0x0E53, r"USB\VID_054C&PID_0E53\FIRST")
SECOND = Receiver(0x054C, 0x0E53, r"USB\VID_054C&PID_0E53\SECOND")


def battery(level, charging):
    return SIGNATURE + bytes((0x10, 1, 0, charging, level, 0))


class FakeClock:
    def __init__(self):
        self.now = 0.0

    def sleep(self, amount):
        self.now += amount


class FakePort:
    def __init__(self, clock, *chunks, fail=False):
        self.clock, self.chunks, self.fail = clock, deque(chunks), fail
        self.commands, self.reads, self.purged, self.closed = [], 0, False, False

    def purge(self):
        self.purged = True

    def write(self, data):
        self.commands.append(data)
        if self.fail:
            raise TimeoutError("串口写入超时")

    def read(self, size):
        self.reads += 1
        self.clock.sleep(0.08)
        return self.chunks.popleft() if self.chunks else b""


class FakeWindows(_WindowsSerial):
    def __init__(self, ports):
        self.opened, self.ports = [], ports
        self.endpoints = {"Path-A": (1, "COM3"), "Path-B": (4, "COM3")}
        self.nodes = {
            1: (r"USB\VID_054C&PID_0E53&MI_02\IFACE_A", 2),
            2: (FIRST.instance, None),
            4: (r"USB\VID_054C&PID_0E53&MI_02\IFACE_B", 5),
            5: (SECOND.instance, None),
        }

    def _present_interfaces(self):
        return tuple(self.endpoints)

    def _node_for(self, path):
        return self.endpoints[path][0]

    def _device_id(self, node):
        return self.nodes[node][0]

    def _parent(self, node):
        return self.nodes[node][1]

    def _port_name(self, path, node):
        return self.endpoints[path][1]

    @contextmanager
    def open(self, path):
        self.opened.append(path)
        port = self.ports[path]
        if port is None:
            raise PermissionError("串口已占用")
        try:
            yield port
        finally:
            port.closed = True


def provider(receiver, windows, clock):
    return SonySerialProvider(receiver, windows=windows, clock=lambda: clock.now, sleep=clock.sleep)


class SonySerialTests(unittest.TestCase):
    def test_selects_exact_usb_parent_with_identical_vid_pid_and_com_names(self):
        clock = FakeClock()
        ports = {"Path-A": FakePort(clock, battery(60, 0)), "Path-B": FakePort(clock, battery(85, 1))}
        windows = FakeWindows(ports)
        self.assertEqual([(s.level, s.charging) for s in provider(SECOND, windows, clock).poll()], [(85, True)])
        self.assertEqual(windows.opened, ["Path-B"])
        self.assertEqual(ports["Path-A"].commands, [])
        self.assertEqual([(s.level, s.charging) for s in provider(FIRST, windows, clock).poll()], [(60, False)])
        self.assertEqual(windows.opened, ["Path-B", "Path-A"])

    def test_rejects_ambiguous_missing_or_unverified_receiver_before_opening(self):
        clock = FakeClock()
        windows = FakeWindows({})
        windows.endpoints["Other-A"] = (1, "COM8")
        self.assertEqual(provider(FIRST, windows, clock).poll(), [])
        self.assertEqual(provider(Receiver(FIRST.vid, FIRST.pid, r"USB\VID_054C&PID_0E53\GONE"), windows, clock).poll(), [])
        self.assertEqual(provider(Receiver(FIRST.vid, 0x0FA8, FIRST.instance), windows, clock).poll(), [])
        self.assertEqual(provider(Receiver(FIRST.vid, FIRST.pid, r"USB\VID_054C&PID_0FA8\MISLABELLED"), windows, clock).poll(), [])
        self.assertEqual(windows.opened, [])

    def test_fragmented_new_frame_keeps_zero_and_sends_only_known_commands(self):
        clock = FakeClock()
        frame = battery(0, 1)
        port = FakePort(clock, b"noise" + battery(101, 0) + frame[:6], frame[6:])
        results = provider(FIRST, FakeWindows({"Path-A": port}), clock).poll()
        self.assertEqual([(s.level, s.charging, s.online) for s in results], [(0, True, True)])
        self.assertEqual(port.commands, [INIT_1, INIT_2, QUERY])
        self.assertTrue(port.purged and port.closed)

    def test_timeout_and_sharing_conflict_keep_the_reading_unknown(self):
        clock = FakeClock()
        unplugged = FakePort(clock, battery(50, 2))
        windows = FakeWindows({"Path-A": unplugged, "Path-B": None})
        self.assertEqual(provider(FIRST, windows, clock).poll(), [])
        self.assertLessEqual(unplugged.reads, 11)
        self.assertTrue(unplugged.closed)
        self.assertEqual(provider(SECOND, windows, clock).poll(), [])
        self.assertEqual(windows.opened, ["Path-A", "Path-B"])

    def test_failed_write_never_sends_another_frame(self):
        clock = FakeClock()
        port = FakePort(clock, fail=True)
        self.assertEqual(provider(FIRST, FakeWindows({"Path-A": port}), clock).poll(), [])
        self.assertEqual(port.commands, [INIT_1])
        self.assertEqual(port.reads, 0)
        self.assertTrue(port.closed)

    @unittest.skipUnless(os.name == "nt", "需要 Windows ctypes ABI")
    def test_windows_api_layout_and_interface_discovery(self):
        self.assertEqual((ctypes.sizeof(_Dcb), ctypes.sizeof(_Timeouts), ctypes.sizeof(_PropertyKey)), (28, 20, 20))
        self.assertIsInstance(_WindowsSerial()._present_interfaces(), tuple)
