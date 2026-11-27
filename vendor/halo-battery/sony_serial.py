"""仅从已匹配的 Sony INZONE H9 接收器读取电量。"""

from __future__ import annotations

from contextlib import contextmanager
import ctypes
from ctypes import wintypes
import re
import time
from uuid import UUID

from providers.base import DeviceStatus
from providers.sony_inzone import FRAME_LENGTH, INIT_1, INIT_2, KEYS, NAME, QUERY, find_frame
from win_hid import USB_INSTANCE


_GUID = ctypes.c_ubyte * 16
_COMPORT = _GUID.from_buffer_copy(UUID("86e0d1e0-8089-11d0-9ce4-08003e301f73").bytes_le)
_COM_NAME = re.compile(r"COM[1-9][0-9]{0,4}", re.IGNORECASE | re.ASCII)
_CR_SUCCESS = 0
_CR_BUFFER_SMALL = 0x1A
_DEVPROP_TYPE_STRING = 0x12
_INVALID_HANDLE = ctypes.c_void_p(-1).value


class _PropertyKey(ctypes.Structure):
    _fields_ = [("guid", _GUID), ("pid", wintypes.DWORD)]


_INSTANCE_ID = _PropertyKey(_GUID.from_buffer_copy(UUID("78c34fc8-104a-4aca-9ea4-524d52996e57").bytes_le), 256)
_PORT_NAME = _PropertyKey(_GUID.from_buffer_copy(UUID("4c6bf15c-4c03-4aac-91f5-64c0f852bcf4").bytes_le), 4)


class _Dcb(ctypes.Structure):
    _fields_ = [
        ("length", wintypes.DWORD), ("baud", wintypes.DWORD), ("flags", wintypes.DWORD),
        ("reserved", wintypes.WORD), ("xon_limit", wintypes.WORD), ("xoff_limit", wintypes.WORD),
        ("byte_size", wintypes.BYTE), ("parity", wintypes.BYTE), ("stop_bits", wintypes.BYTE),
        ("xon", ctypes.c_char), ("xoff", ctypes.c_char), ("error", ctypes.c_char),
        ("eof", ctypes.c_char), ("event", ctypes.c_char), ("reserved1", wintypes.WORD),
    ]


class _Timeouts(ctypes.Structure):
    _fields_ = [(name, wintypes.DWORD) for name in (
        "read_interval", "read_multiplier", "read_constant", "write_multiplier", "write_constant",
    )]


def _bind(dll, declarations):
    for name, result, arguments in declarations:
        function = getattr(dll, name)
        function.argtypes, function.restype = arguments, result


class _WindowsSerial:
    def __init__(self):
        self.cfg = ctypes.WinDLL("cfgmgr32", use_last_error=True)
        u32, string, buffer, pointer = wintypes.ULONG, wintypes.LPCWSTR, wintypes.LPWSTR, ctypes.POINTER
        _bind(self.cfg, (
            ("CM_Get_Device_Interface_List_SizeW", u32, [pointer(u32), pointer(_GUID), string, u32]),
            ("CM_Get_Device_Interface_ListW", u32, [pointer(_GUID), string, buffer, u32, u32]),
            ("CM_Get_Device_Interface_PropertyW", u32, [string, pointer(_PropertyKey), pointer(u32), ctypes.c_void_p, pointer(u32), u32]),
            ("CM_Locate_DevNodeW", u32, [pointer(u32), string, u32]),
            ("CM_Get_Device_IDW", u32, [u32, buffer, u32, u32]),
            ("CM_Get_Parent", u32, [pointer(u32), u32, u32]),
            ("CM_Open_DevNode_Key", u32, [u32, u32, u32, u32, pointer(wintypes.HKEY), u32]),
        ))
        self.file = ctypes.WinDLL("kernel32", use_last_error=True)
        handle, boolean = wintypes.HANDLE, wintypes.BOOL
        _bind(self.file, (
            ("CreateFileW", handle, [string, u32, u32, ctypes.c_void_p, u32, u32, handle]),
            ("CloseHandle", boolean, [handle]),
            ("GetCommState", boolean, [handle, pointer(_Dcb)]),
            ("SetCommState", boolean, [handle, pointer(_Dcb)]),
            ("SetCommTimeouts", boolean, [handle, pointer(_Timeouts)]),
            ("PurgeComm", boolean, [handle, u32]),
            ("ReadFile", boolean, [handle, ctypes.c_void_p, u32, pointer(u32), ctypes.c_void_p]),
            ("WriteFile", boolean, [handle, ctypes.c_void_p, u32, pointer(u32), ctypes.c_void_p]),
        ))

    def _present_interfaces(self):
        for _ in range(3):
            size = wintypes.ULONG()
            if self.cfg.CM_Get_Device_Interface_List_SizeW(ctypes.byref(size), ctypes.byref(_COMPORT), None, 0):
                break
            if not 1 <= size.value <= 65536:
                break
            buffer = ctypes.create_unicode_buffer(size.value)
            result = self.cfg.CM_Get_Device_Interface_ListW(ctypes.byref(_COMPORT), None, buffer, size.value, 0)
            if result == _CR_SUCCESS:
                return tuple(filter(None, buffer[:].split("\0")))
            if result != _CR_BUFFER_SMALL:
                break
        return ()

    def _property(self, path, key):
        value = ctypes.create_unicode_buffer(512)
        kind = wintypes.ULONG()
        size = wintypes.ULONG(ctypes.sizeof(value))
        result = self.cfg.CM_Get_Device_Interface_PropertyW(
            path, ctypes.byref(key), ctypes.byref(kind), value, ctypes.byref(size), 0,
        )
        if result == _CR_SUCCESS and kind.value == _DEVPROP_TYPE_STRING:
            return value.value or None
        return None

    def _node_for(self, path):
        instance = self._property(path, _INSTANCE_ID)
        if instance:
            node = wintypes.ULONG()
            if self.cfg.CM_Locate_DevNodeW(ctypes.byref(node), instance, 0) == _CR_SUCCESS:
                return node.value
        return None

    def _device_id(self, node):
        name = ctypes.create_unicode_buffer(512)
        if self.cfg.CM_Get_Device_IDW(node, name, len(name), 0) == _CR_SUCCESS:
            return name.value
        return None

    def _parent(self, node):
        parent = wintypes.ULONG()
        if self.cfg.CM_Get_Parent(ctypes.byref(parent), node, 0) == _CR_SUCCESS:
            return parent.value
        return None

    def _usb_instance(self, node):
        for _ in range(16):
            instance = self._device_id(node)
            if instance is None:
                break
            if USB_INSTANCE.fullmatch(instance):
                return instance.casefold()
            node = self._parent(node)
            if node is None:
                break
        return None

    def _port_name(self, path, node):
        name = self._property(path, _PORT_NAME)
        if name:
            return name
        # 某些 USB 串口只在设备硬件键中公布 PortName。
        import winreg

        key = wintypes.HKEY()
        if self.cfg.CM_Open_DevNode_Key(node, winreg.KEY_QUERY_VALUE, 0, 1, ctypes.byref(key), 0) != _CR_SUCCESS:
            return None
        try:
            value, kind = winreg.QueryValueEx(key.value, "PortName")
            return value if kind == winreg.REG_SZ and isinstance(value, str) else None
        except OSError:
            return None
        finally:
            winreg.CloseKey(key.value)

    def port_for(self, instance):
        expected = instance.casefold()
        matching = {}
        for path in self._present_interfaces():
            node = self._node_for(path)
            if node is None or self._usb_instance(node) != expected:
                continue
            if _COM_NAME.fullmatch(self._port_name(path, node) or ""):
                matching[path.casefold()] = path
        return next(iter(matching.values())) if len(matching) == 1 else None

    @contextmanager
    def open(self, path):
        # 使用设备接口路径打开已验证的端点，避免 COM 名称碰撞或重用了同一序号。
        handle = self.file.CreateFileW(path, 0xC0000000, 0, None, 3, 0x80, None)
        if handle is None or handle == _INVALID_HANDLE:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            port = _SerialPort(self.file, handle)
            port.configure()
            yield port
        finally:
            self.file.CloseHandle(handle)


class _SerialPort:
    def __init__(self, file, handle):
        self.file, self.handle = file, handle

    def configure(self):
        dcb = _Dcb(length=ctypes.sizeof(_Dcb))
        if not self.file.GetCommState(self.handle, ctypes.byref(dcb)):
            raise ctypes.WinError(ctypes.get_last_error())
        dcb.baud, dcb.byte_size, dcb.parity, dcb.stop_bits = 115200, 8, 0, 0
        dcb.xon, dcb.xoff = b"\x11", b"\x13"
        # 二进制模式，DTR 和 RTS 保持启用，不启用流控。
        dcb.flags = 1 | (1 << 4) | (1 << 12)
        timeouts = _Timeouts(0xFFFFFFFF, 0xFFFFFFFF, 80, 0, 250)
        if (not self.file.SetCommState(self.handle, ctypes.byref(dcb))
                or not self.file.SetCommTimeouts(self.handle, ctypes.byref(timeouts))):
            raise ctypes.WinError(ctypes.get_last_error())

    def purge(self):
        if not self.file.PurgeComm(self.handle, 0x04 | 0x08):
            raise ctypes.WinError(ctypes.get_last_error())

    def write(self, data):
        count = wintypes.DWORD()
        if not self.file.WriteFile(self.handle, data, len(data), ctypes.byref(count), None):
            raise ctypes.WinError(ctypes.get_last_error())
        if count.value != len(data):
            raise TimeoutError("串口查询发送超时")

    def read(self, size):
        data = ctypes.create_string_buffer(size)
        count = wintypes.DWORD()
        if not self.file.ReadFile(self.handle, data, size, ctypes.byref(count), None):
            raise ctypes.WinError(ctypes.get_last_error())
        return data.raw[:count.value]


def _read_frame(port, clock, sleep):
    sleep(0.5)
    port.purge()
    for command in (INIT_1, INIT_2, QUERY):
        port.write(command)
        sleep(0.25)
    deadline = clock() + 0.8
    tail = b""
    while clock() < deadline:
        data = tail + port.read(128)
        if clock() < deadline and (frame := find_frame(data)) is not None:
            return frame
        tail = data[-(FRAME_LENGTH - 1):]
    return None


class SonySerialProvider:
    name = "sony_inzone_serial"
    target = (0x054C, 0x0E53)

    def __init__(self, receiver, *, windows=None, clock=time.monotonic, sleep=time.sleep):
        self.receiver, self.windows, self.clock, self.sleep = receiver, windows, clock, sleep

    def poll(self) -> list[DeviceStatus]:
        if (self.receiver.vid, self.receiver.pid) != self.target:
            return []
        instance = self.receiver.instance
        match = USB_INSTANCE.fullmatch(instance)
        if match is None or tuple(int(value, 16) for value in match.groups()) != self.target:
            return []
        try:
            if self.windows is None:
                self.windows = _WindowsSerial()
            path = self.windows.port_for(instance)
            if path is None:
                return []
            with self.windows.open(path) as port:
                frame = _read_frame(port, self.clock, self.sleep)
        except OSError:
            return []
        if frame is None:
            return []
        return [DeviceStatus(KEYS[self.receiver.pid], NAME, frame[12], frame[11] == 1, True,
                             self.name, kind="headset")]
