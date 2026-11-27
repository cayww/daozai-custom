"""离线验证 CfgMgr 限定、WinUSB 绑定和 Mic Mini 2 的双路电量。"""

from contextlib import contextmanager
import ctypes as c
from queue import Empty, Queue
import threading
import time
import unittest
from unittest.mock import Mock, patch
from uuid import UUID

from worker import Receiver
from dji_winusb import DjiWinUsbProvider, _CfgMgr, _PipeInformation, _open_pipe, dji, w


# HaloBattery v1.18.0 tests/test_dji.py 的真机状态帧，覆盖两只发射器及 TX2 关闭。
STATUS = bytes.fromhex(
    "557604a65a020000005b03036600030000000020002100000000000000000000000000000000"
    "000000000000030000001e00000002010000001a982404010078000000000000000000000000"
    "000000000000000002020000001a982404410078000000000000000000000000000000000000"
    "0000c3f2"
)
TX2_OFF = bytes.fromhex(
    "555604675a020000005b03034600030000000020003100000000000000000000000000000000"
    "000000000000010000001e00000002010000001a982404010078000000000000000000000000"
    "0000000000000000755c"
)
A = Receiver(dji.VID, dji.PID, r"USB\VID_2CA3&PID_4011\A")
B = Receiver(dji.VID, dji.PID, r"USB\VID_2CA3&PID_4011\B")
MI_A = r"USB\VID_2CA3&PID_4011&MI_06\A_CHILD"
MI_B = r"USB\VID_2CA3&PID_4011&MI_06\B_CHILD"
GUID = UUID("87654321-4321-4321-b456-123456789abc")


class CfgMgrTests(unittest.TestCase):
    def test_只对精确请求物理设备的第六接口枚举已安装的路径(self):
        cfg = _CfgMgr.__new__(_CfgMgr)
        cfg.children = Mock(side_effect={
            A.instance: [(4, MI_A.replace("MI_06", "MI_03")), (6, MI_A)],
            B.instance: [(9, MI_B)],
        }.__getitem__)
        cfg.is_winusb = Mock(side_effect=lambda node: node in (6, 9))
        cfg.interface_classes = Mock(return_value={GUID})
        cfg.paths = Mock(side_effect=lambda instance, _: {instance})
        self.assertEqual(MI_A, cfg.find(A.instance))
        self.assertEqual(MI_B, cfg.find(B.instance))
        self.assertEqual([((MI_A, GUID),), ((MI_B, GUID),)], cfg.paths.call_args_list)
        cfg.is_winusb.assert_any_call(6)
        self.assertEqual(2, cfg.is_winusb.call_count)

    def test_其他驱动及多个路径不打开设备(self):
        cfg = _CfgMgr.__new__(_CfgMgr)
        cfg.children = Mock(return_value=[(6, MI_A)])
        cfg.is_winusb = Mock(return_value=False)
        cfg.interface_classes = Mock(return_value={GUID})
        cfg.paths = Mock(return_value={"first", "second"})
        self.assertIsNone(cfg.find(A.instance))
        cfg.paths.assert_not_called()
        cfg.is_winusb.return_value = True
        self.assertIsNone(cfg.find(A.instance))
        self.assertIsNone(cfg.find(MI_A))
        self.assertIsNone(cfg.find(A.instance.replace("4011", "4010")))


class WinUsbTests(unittest.TestCase):
    def setUp(self):
        self.kernel, self.usb = Mock(), Mock()
        self.kernel.CreateFileW.return_value = 1234
        modules = {"kernel32": self.kernel, "winusb": self.usb}
        patcher = patch("dji_winusb.c.WinDLL", side_effect=lambda name, **_: modules[name])
        patcher.start()
        self.addCleanup(patcher.stop)
        self.usb.WinUsb_QueryInterfaceSettings.side_effect = self.describe
        self.usb.WinUsb_QueryPipe.side_effect = self.describe_pipe
        self.usb.WinUsb_ReadPipe.side_effect = self.read
        self.interface = 6

    def describe(self, _, alternate, output):
        descriptor = c.cast(output, c.POINTER(c.c_ubyte * 9)).contents
        descriptor[:] = (9, 4, self.interface, alternate, 1, 0xFF, 0xF0, 0, 0)
        return True

    @staticmethod
    def describe_pipe(_, alternate, index, output):
        pipe = c.cast(output, c.POINTER(_PipeInformation)).contents
        pipe.pipe_id, pipe.pipe_type = dji.EP_IN, 2
        return True

    @staticmethod
    def read(_, pipe, buffer, length, count, overlapped):
        buffer[:3] = (0x55, 0x76, 0x04)
        c.cast(count, c.POINTER(w.ULONG)).contents.value = 3
        return True

    def test_只读_0x86_并设置超时_退出后释放(self):
        with _open_pipe("exact-device-path") as pipe:
            self.assertEqual(b"\x55\x76\x04", pipe.read())
        self.assertEqual(dji.EP_IN, self.usb.WinUsb_ReadPipe.call_args.args[1])
        self.assertEqual((dji.EP_IN, 3, c.sizeof(w.ULONG)),
                         self.usb.WinUsb_SetPipePolicy.call_args.args[1:4])
        self.usb.WinUsb_WritePipe.assert_not_called()
        self.usb.WinUsb_ControlTransfer.assert_not_called()
        self.usb.WinUsb_Free.assert_called_once()
        self.kernel.CloseHandle.assert_called_once_with(1234)

    def test_意外接口读取前回退并释放(self):
        self.interface = 3
        with self.assertRaises(OSError), _open_pipe("wrong-interface"):
            self.fail("描述符不匹配的接口进入读取")
        self.usb.WinUsb_ReadPipe.assert_not_called()
        self.usb.WinUsb_Free.assert_called_once()
        self.kernel.CloseHandle.assert_called_once_with(1234)


class FakeBulk:
    def __init__(self):
        self.events = Queue()
        self.closed = threading.Event()

    def read(self):
        try:
            item = self.events.get(timeout=0.025)
        except Empty:
            return b""
        if isinstance(item, OSError):
            raise item
        return item


class ProviderTests(unittest.TestCase):
    def setUp(self):
        self.streams = {x.instance: FakeBulk() for x in (A, B)}
        self.running = {}

    @contextmanager
    def open(self, path):
        stream = self.streams[path]
        self.running[path] = stream
        try:
            yield stream
        finally:
            stream.closed.set()

    def provider(self, receiver, **kwargs):
        provider = DjiWinUsbProvider(receiver, find=lambda instance: instance,
                                     open_pipe=self.open, **kwargs)
        self.addCleanup(provider.close)
        return provider

    def test_同PID物理隔离_分帧_每路电量_重复轮询不更新时间(self):
        self.streams[A.instance].events.put(STATUS[:67])
        self.streams[A.instance].events.put(STATUS[67:])
        # 用相同结构构造另一只接收器的 4 档、充电中，并补正确的 CRC。
        charging = bytearray(TX2_OFF)
        charging[52 + 7] = (4 << 2) | 0x02
        charging[-2:] = dji.crc16(charging[:-2], 0x3692).to_bytes(2, "little")
        self.streams[B.instance].events.put(bytes(charging))
        one, two = self.provider(A), self.provider(B)
        a, b = one.poll(), two.poll()
        self.assertEqual(["tx1", "tx2"], [entry.key for entry in a])
        self.assertEqual(["tx1"], [entry.key for entry in b])
        self.assertEqual([(100, False), (100, False)], [(s.level, s.charging) for s in a])
        self.assertEqual([(60, True)], [(s.level, s.charging) for s in b])
        self.assertTrue(all(entry.approx.startswith("about ") for entry in a + b))
        time.sleep(0.04)
        self.assertEqual(a[1].ui_params, one.poll()[1].ui_params)
        self.assertEqual({A.instance, B.instance}, set(self.running))

    def test_坏CRC和关机宽限_以及过期都不会生成新电量(self):
        clock = Mock(return_value=5.0)
        provider = self.provider(A, clock=clock)
        provider._accept(STATUS)
        clock.return_value = 6.0
        bad = bytearray(STATUS)
        bad[60] ^= 1
        provider._accept(bytes(bad))
        self.assertEqual(5.0, provider._states["tx2"][0])
        provider._accept(TX2_OFF)
        self.assertIn("tx2", provider._states)
        self.assertEqual(5.0, provider._states["tx2"][0])
        provider._accept(TX2_OFF)
        provider._accept(TX2_OFF)
        self.assertEqual({"tx1"}, set(provider._states))
        self.assertEqual(6.0, provider._states["tx1"][0])

        self.streams[A.instance].events.put(TX2_OFF)
        self.assertEqual(1, len(provider.poll()))
        clock.return_value = 6.0 + dji.STALE_SECONDS + 0.01
        self.assertEqual([], provider.poll())

    def test_驱动路径不可用不打开_拔出立即清空并释放(self):
        unavailable = DjiWinUsbProvider(A, find=lambda _: None, open_pipe=self.open)
        self.addCleanup(unavailable.close)
        self.assertEqual([], unavailable.poll())
        self.assertEqual({}, self.running)
        unavailable.close()

        stream = self.streams[A.instance]
        stream.events.put(STATUS)
        provider = self.provider(A)
        self.assertEqual(2, len(provider.poll()))
        stream.events.put(OSError("device detached"))
        self.assertTrue(stream.closed.wait(0.3))
        # 退出 read 后，后台线程可能还在清理缓存。
        for _ in range(10):
            if not provider.poll():
                break
            time.sleep(0.005)
        self.assertEqual([], provider.poll())
        provider.close()
        self.assertFalse(provider._reader.is_alive())


if __name__ == "__main__":
    unittest.main()
