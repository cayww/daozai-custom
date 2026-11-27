"""按 USB HID 实例记录鼠标实际移动或按键的时间。"""

from __future__ import annotations

import ctypes as C
from ctypes import wintypes as W
import logging
import re
import sys
import threading
import time

from win_hid import HID_PATH


_LOG = logging.getLogger("daozai-halo")
_HID_GUID = "{4d1e55b2-f16f-11cf-88cb-001111000030}"
_MOUSE_GUID = "{378de44c-56ef-11d1-bc8c-00a0c91405dd}"
_USB_HARDWARE = re.compile(r"vid_[0-9a-f]{4}&pid_[0-9a-f]{4}(?:&[^#\\]+)?")
_INPUT_AGE_MS = 250
_WAKE_INTERVAL = 0.1
_WM_INPUT = 0x00FF
_WM_INPUT_DEVICE_CHANGE = 0x00FE
_RID_INPUT = 0x10000003
_RID_HEADER = 0x10000005
_RIDI_DEVICENAME = 0x20000007
_UINT_ERROR = 0xFFFFFFFF


def _identity(path):
    if isinstance(path, bytes):
        path = path.decode("ascii", errors="replace")
    if not isinstance(path, str) or not path.isascii() or len(path) > 1024:
        return None
    path = path.casefold()
    match = HID_PATH.fullmatch(path)
    if (match is None or path[match.end(3) + 1:] not in (_HID_GUID, _MOUSE_GUID)
            or _USB_HARDWARE.fullmatch(match[2]) is None):
        return None
    # 鼠标接口与 HID 接口的 GUID 可以不同，前面的完整 HID 设备实例相同。
    return path[:match.end(3)]


def _event_time(now, current_tick, message_tick):
    age = (current_tick - message_tick) & 0xFFFFFFFF
    return now - age / 1000 if age <= _INPUT_AGE_MS else None


class _Subscription:
    def __init__(self, owner, paths, wake):
        self.owner, self.paths, self.wake = owner, paths, wake
        self._bound_at = time.monotonic()
        self._observed = None
        self._last_wake = float("-inf")

    def latest(self) -> float | None:
        with self.owner._lock:
            return self._observed if self in self.owner._subscriptions else None

    def close(self):
        self.owner._unsubscribe(self)


class MouseActivity:
    """同一组件进程共享一个鼠标监听窗口，每个订阅绑定自己的 HID 实例。"""

    def __init__(self):
        self._lock = threading.Lock()
        self._subscriptions = set()
        self._window = None
        self._closed = False

    def subscribe(self, paths, wake=None):
        subscription = _Subscription(self, frozenset(filter(None, map(_identity, paths))), wake)
        with self._lock:
            if not self._closed and subscription.paths and sys.platform == "win32":
                self._subscriptions.add(subscription)
                self._start()
        return subscription

    def _start(self):
        if self._window is None:
            self._window = _RawMouseWindow(self)
            self._window.thread.start()

    def _finished(self, window):
        with self._lock:
            self._window = None
            for subscription in self._subscriptions:
                subscription._observed = None
            # 上一窗口完成注销后才启动下一窗口，避免 Windows 覆盖鼠标注册。
            if window.stopped.is_set() and self._subscriptions and not self._closed:
                self._start()

    def _unsubscribe(self, subscription):
        with self._lock:
            self._subscriptions.discard(subscription)
            if not self._subscriptions and self._window is not None:
                self._window.stop()

    def _record(self, window, identity, observed):
        callbacks = []
        with self._lock:
            if self._window is not window or window.stopped.is_set():
                return
            for subscription in self._subscriptions:
                if identity not in subscription.paths:
                    continue
                if observed is None:
                    subscription._bound_at = time.monotonic()
                    subscription._observed = None
                    subscription._last_wake = float("-inf")
                    continue
                previous = subscription._observed
                if observed < subscription._bound_at or (previous is not None and observed <= previous):
                    continue
                subscription._observed = observed
                if subscription.wake is not None and observed - subscription._last_wake >= _WAKE_INTERVAL:
                    subscription._last_wake = observed
                    callbacks.append(subscription.wake)
        for wake in callbacks:
            try:
                wake()
            except Exception:
                _LOG.exception("鼠标活动唤醒回调失败")

    def _listens_to(self, identity):
        with self._lock:
            return any(identity in subscription.paths for subscription in self._subscriptions)

    def close(self):
        with self._lock:
            self._closed = True
            self._subscriptions.clear()
            window = self._window
            if window is not None:
                window.stop()
        if window is not None and window.thread is not threading.current_thread():
            window.thread.join(1)


class _Registration(C.Structure):
    _fields_ = [("page", W.USHORT), ("usage", W.USHORT), ("flags", W.DWORD), ("window", W.HWND)]


class _Header(C.Structure):
    _fields_ = [("kind", W.DWORD), ("size", W.DWORD), ("device", W.HANDLE), ("param", W.WPARAM)]


class _Mouse(C.Structure):
    _fields_ = [("flags", W.USHORT), ("buttons", W.ULONG), ("raw_buttons", W.ULONG),
                ("x", W.LONG), ("y", W.LONG), ("extra", W.ULONG)]


class _MouseInput(C.Structure):
    _fields_ = [("header", _Header), ("mouse", _Mouse)]


def _bind(dll, declarations):
    for name, result, arguments in declarations:
        fn = getattr(dll, name)
        fn.restype, fn.argtypes = result, arguments


class _RawMouseWindow:
    def __init__(self, owner):
        self.owner = owner
        self.stopped = threading.Event()
        self.thread = threading.Thread(target=self._run, name="daozai-raw-mouse", daemon=True)
        self.window = None
        self.post = None
        self.paths = {}

    def stop(self):
        self.stopped.set()
        if self.window is not None:
            self.post(self.window, 0, 0, 0)

    def _run(self):
        try:
            self._listen()
        except OSError:
            _LOG.exception("无线鼠标输入监听不可用")
        finally:
            self.owner._finished(self)

    def _listen(self):
        user = C.WinDLL("user32", use_last_error=True)
        kernel = C.WinDLL("kernel32", use_last_error=True)
        proc = C.WINFUNCTYPE(C.c_ssize_t, W.HWND, W.UINT, W.WPARAM, W.LPARAM)

        class WindowClass(C.Structure):
            _fields_ = [("style", W.UINT), ("proc", proc), ("class_extra", C.c_int), ("window_extra", C.c_int),
                        ("instance", W.HINSTANCE), ("icon", W.HICON), ("cursor", W.HANDLE),
                        ("brush", W.HBRUSH), ("menu", W.LPCWSTR), ("name", W.LPCWSTR)]

        _bind(kernel, (("GetModuleHandleW", W.HMODULE, [W.LPCWSTR]), ("GetTickCount", W.DWORD, [])))
        _bind(user, (
            ("RegisterClassW", W.ATOM, [C.POINTER(WindowClass)]),
            ("UnregisterClassW", W.BOOL, [W.LPCWSTR, W.HINSTANCE]),
            ("CreateWindowExW", W.HWND, [W.DWORD, W.LPCWSTR, W.LPCWSTR, W.DWORD, C.c_int, C.c_int,
                                       C.c_int, C.c_int, W.HWND, W.HMENU, W.HINSTANCE, C.c_void_p]),
            ("DefWindowProcW", C.c_ssize_t, [W.HWND, W.UINT, W.WPARAM, W.LPARAM]),
            ("DestroyWindow", W.BOOL, [W.HWND]),
            ("GetMessageW", W.BOOL, [C.POINTER(W.MSG), W.HWND, W.UINT, W.UINT]),
            ("DispatchMessageW", C.c_ssize_t, [C.POINTER(W.MSG)]),
            ("PostMessageW", W.BOOL, [W.HWND, W.UINT, W.WPARAM, W.LPARAM]),
            ("RegisterRawInputDevices", W.BOOL, [C.POINTER(_Registration), W.UINT, W.UINT]),
            ("GetRawInputData", W.UINT, [W.HANDLE, W.UINT, C.c_void_p, C.POINTER(W.UINT), W.UINT]),
            ("GetRawInputDeviceInfoW", W.UINT, [W.HANDLE, W.UINT, C.c_void_p, C.POINTER(W.UINT)]),
        ))
        self.post = user.PostMessageW
        instance = kernel.GetModuleHandleW(None)
        name = f"DaozaiRawMouse_{id(self)}"
        callback = proc(user.DefWindowProcW)
        window_class = WindowClass(0, callback, 0, 0, instance, None, None, None, None, name)
        if not user.RegisterClassW(C.byref(window_class)):
            raise C.WinError(C.get_last_error())
        listening = False
        try:
            self.window = user.CreateWindowExW(0, name, name, 0, 0, 0, 0, 0, W.HWND(-3), None, instance, None)
            if not self.window:
                raise C.WinError(C.get_last_error())
            if self.stopped.is_set():
                return
            # Generic Desktop Mouse，后台输入和设备拔插。键盘使用另外的注册。
            reg = _Registration(1, 2, 0x100 | 0x2000, self.window)
            if not user.RegisterRawInputDevices(C.byref(reg), 1, C.sizeof(reg)):
                raise C.WinError(C.get_last_error())
            listening = True
            message = W.MSG()
            while not self.stopped.is_set() and user.GetMessageW(C.byref(message), self.window, 0, 0) > 0:
                if message.message == _WM_INPUT and not self.stopped.is_set():
                    self._input(user, kernel, message)
                elif message.message == _WM_INPUT_DEVICE_CHANGE:
                    identity = self.paths.pop(message.lParam, None)
                    if identity is not None:
                        self.owner._record(self, identity, None)
                # 由 DefWindowProc 清理系统提交的原始输入消息。
                user.DispatchMessageW(C.byref(message))
        finally:
            if listening:
                reg = _Registration(1, 2, 1, None)
                user.RegisterRawInputDevices(C.byref(reg), 1, C.sizeof(reg))
            window, self.window = self.window, None
            if window:
                user.DestroyWindow(window)
            user.UnregisterClassW(name, instance)

    def _input(self, user, kernel, message):
        if not message.lParam:
            return
        observed = _event_time(time.monotonic(), kernel.GetTickCount(), message.time)
        if observed is None:
            return
        header = _Header()
        size = W.UINT(C.sizeof(header))
        if (user.GetRawInputData(message.lParam, _RID_HEADER, C.byref(header), C.byref(size), C.sizeof(header))
                != C.sizeof(header) or header.kind != 0 or not header.device):
            return
        if header.device not in self.paths:
            if len(self.paths) >= 128:
                self.paths.clear()
            self.paths[header.device] = self._device_identity(user, header.device)
        identity = self.paths[header.device]
        if (identity is not None and self.owner._listens_to(identity)
                and self._mouse_moved(user, message.lParam, header.device)):
            self.owner._record(self, identity, observed)

    @staticmethod
    def _mouse_moved(user, raw, device):
        # 只为已绑定的鼠标检查是否有按键或相对位移，坐标和按键值不留存。
        packet = _MouseInput()
        size = W.UINT(C.sizeof(packet))
        count = user.GetRawInputData(raw, _RID_INPUT, C.byref(packet), C.byref(size), C.sizeof(_Header))
        if count != C.sizeof(packet) or packet.header.kind != 0 or packet.header.device != device:
            return False
        mouse = packet.mouse
        return bool(mouse.buttons & 0xFFFF or (not (mouse.flags & 1) and (mouse.x != 0 or mouse.y != 0)))

    @staticmethod
    def _device_identity(user, handle):
        length = W.UINT()
        if (user.GetRawInputDeviceInfoW(handle, _RIDI_DEVICENAME, None, C.byref(length)) == _UINT_ERROR
                or not 1 <= length.value <= 1024):
            return None
        value = C.create_unicode_buffer(length.value + 1)
        length.value += 1
        if user.GetRawInputDeviceInfoW(handle, _RIDI_DEVICENAME, value, C.byref(length)) == _UINT_ERROR:
            return None
        return _identity(value.value)
