"""鼠标原始输入的路径归属、时间和订阅生命周期测试。"""

import ctypes
from ctypes import wintypes
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import worker  # 载入与组件入口一致的上游模块路径。
import mouse_activity
from mouse_activity import MouseActivity, _Header, _MouseInput, _RawMouseWindow, _event_time, _identity


HID_SUFFIX = "#{4d1e55b2-f16f-11cf-88cb-001111000030}"
MOUSE_SUFFIX = "#{378de44c-56ef-11d1-bc8c-00a0c91405dd}"
FIRST = r"\\?\HID#VID_3837&PID_1018&MI_00&COL01#7&1234abcd&0&0000"
SECOND = r"\\?\HID#VID_3837&PID_1018&MI_00&COL01#7&1234fade&0&0000"


class MouseActivityTests(unittest.TestCase):
    def test_matches_two_interface_guids_but_keeps_collection_and_instance(self):
        self.assertEqual(_identity((FIRST + HID_SUFFIX).encode()), _identity(FIRST.lower() + MOUSE_SUFFIX))
        self.assertNotEqual(_identity(FIRST + HID_SUFFIX), _identity(SECOND + HID_SUFFIX))
        self.assertNotEqual(_identity(FIRST + HID_SUFFIX),
                            _identity(FIRST.replace("COL01", "COL02") + HID_SUFFIX))
        for other in (FIRST, FIRST + "#{00000000-0000-0000-0000-000000000000}",
                      r"\\?\ROOT#RDP_MOU#0000" + MOUSE_SUFFIX,
                      r"\\?\HID#VID&00020337_PID&1018#0" + MOUSE_SUFFIX, None):
            with self.subTest(other=other):
                self.assertIsNone(_identity(other))

    def test_message_age_wraps_and_rejects_stale_or_future_ticks(self):
        self.assertAlmostEqual(_event_time(4.0, 0x004, -2), 3.994)
        self.assertAlmostEqual(_event_time(4.0, 300, 50), 3.75)
        self.assertIsNone(_event_time(4.0, 301, 50))
        self.assertIsNone(_event_time(4.0, 300, 301))

    def test_bindings_are_independent_and_new_window_waits_for_old_window_to_finish(self):
        with (patch.object(mouse_activity.sys, "platform", "win32"),
              patch.object(mouse_activity._RawMouseWindow, "_run"),
              patch.object(mouse_activity.time, "monotonic", return_value=10.0)):
            activity = MouseActivity()
            wake_first, wake_second = Mock(), Mock()
            first = activity.subscribe([FIRST + HID_SUFFIX], wake_first)
            second = activity.subscribe([SECOND + HID_SUFFIX], wake_second)
            window = activity._window
            activity.subscribe([None, "无效路径"]).close()
            first_identity = _identity(FIRST + MOUSE_SUFFIX)
            second_identity = _identity(SECOND + MOUSE_SUFFIX)
            activity._record(window, first_identity, 9.99)
            self.assertIsNone(first.latest())
            activity._record(window, first_identity, 10.01)
            activity._record(window, first_identity, 10.02)
            self.assertEqual(first.latest(), 10.02)
            self.assertIsNone(second.latest())
            wake_first.assert_called_once()
            wake_second.assert_not_called()
            first.close()
            self.assertFalse(window.stopped.is_set())
            self.assertIsNone(first.latest())
            activity._record(window, second_identity, 10.21)
            self.assertEqual(second.latest(), 10.21)
            activity._record(window, second_identity, None)
            self.assertIsNone(second.latest())
            second.close()
            self.assertTrue(window.stopped.is_set())
            new = activity.subscribe([SECOND + HID_SUFFIX])
            self.assertIs(activity._window, window)
            activity._record(window, second_identity, 10.22)
            self.assertIsNone(new.latest())
            activity._finished(window)
            self.assertIsNot(activity._window, window)
            activity._record(activity._window, second_identity, 10.23)
            self.assertEqual(new.latest(), 10.23)
            activity.close()
            self.assertIsNone(new.latest())

    def test_rejects_unbound_zero_movement_wrong_handle_and_expired_messages(self):
        class User:
            kind, handle, raw_valid = 0, 1, True
            dx, buttons = 0, 0

            def GetRawInputData(self, hwnd, command, buffer, size, header_size):
                self.command = command
                if command == 0x10000003:
                    packet = ctypes.cast(buffer, ctypes.POINTER(_MouseInput)).contents
                    packet.header.kind, packet.header.device = self.kind, self.handle
                    packet.mouse.x, packet.mouse.buttons = self.dx, self.buttons
                    return ctypes.sizeof(packet)
                header = ctypes.cast(buffer, ctypes.POINTER(_Header)).contents
                header.kind, header.device = self.kind, self.handle
                return header_size if self.raw_valid else 0xFFFFFFFF

            def GetRawInputDeviceInfoW(self, handle, command, buffer, length):
                lengths = ctypes.cast(length, ctypes.POINTER(wintypes.UINT))
                path = (FIRST if handle == 1 else SECOND) + MOUSE_SUFFIX
                if buffer is None:
                    lengths[0] = len(path) + 1
                    return 0
                buffer.value = path
                return len(path) + 1

        owner = SimpleNamespace(_record=Mock(), _listens_to=lambda identity: identity == _identity(FIRST + HID_SUFFIX))
        window = _RawMouseWindow(owner)
        user = User()
        kernel = SimpleNamespace(GetTickCount=lambda: 400)
        message = SimpleNamespace(lParam=11, time=370)
        with patch.object(mouse_activity.time, "monotonic", return_value=50.0):
            for kind, handle, valid, queued_at in ((1, 1, True, 370), (2, 1, True, 370),
                                                   (0, 0, True, 370), (0, 1, False, 370),
                                                   (0, 1, True, 100)):
                user.kind, user.handle, user.raw_valid = kind, handle, valid
                message.time = queued_at
                window._input(user, kernel, message)
            owner._record.assert_not_called()
            message.time = 370
            user.handle = 2
            window._input(user, kernel, message)
            self.assertEqual(user.command, 0x10000005)
            user.handle = 1
            window._input(user, kernel, message)
            owner._record.assert_not_called()
            user.dx = 1
            window._input(user, kernel, message)
        self.assertEqual(user.command, 0x10000003)
        owner._record.assert_called_once_with(window, _identity(FIRST + HID_SUFFIX), 49.97)


if __name__ == "__main__":
    unittest.main()
