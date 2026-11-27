"""DJI 无线麦克风接收器的 WinUSB 只读采集。"""

from __future__ import annotations

from contextlib import contextmanager
import ctypes as c
from ctypes import wintypes as w
import re
import threading
import time
from uuid import UUID
import winreg

from providers import dji
from providers.base import DeviceStatus


_RECV = re.compile(r"USB\\VID_2CA3&PID_4011\\[^\\\x00]+", re.IGNORECASE)
_MI_06 = re.compile(r"USB\\VID_2CA3&PID_4011&MI_06\\[^\\\x00]+", re.IGNORECASE)
_UCHAR = c.c_ubyte
_PULONG = c.POINTER(w.ULONG)
_INVALID_HANDLE = c.c_void_p(-1).value


class _Guid(c.Structure):
    _fields_ = [
        ("data1", w.ULONG), ("data2", w.USHORT), ("data3", w.USHORT),
        ("data4", _UCHAR * 8),
    ]


class _PipeInformation(c.Structure):
    _fields_ = [
        ("pipe_type", c.c_int), ("pipe_id", _UCHAR),
        ("max_packet_size", w.USHORT), ("interval", _UCHAR),
    ]


def _bind(dll, signatures, result):
    for name, args in signatures.items():
        function = getattr(dll, name)
        function.argtypes, function.restype = args, result


class _CfgMgr:
    def __init__(self):
        self.dll = c.WinDLL("cfgmgr32")
        _bind(self.dll, {
            "CM_Locate_DevNodeW": [_PULONG, w.LPCWSTR, w.ULONG],
            "CM_Get_Child": [_PULONG, w.ULONG, w.ULONG],
            "CM_Get_Sibling": [_PULONG, w.ULONG, w.ULONG],
            "CM_Get_Device_IDW": [w.ULONG, w.LPWSTR, w.ULONG, w.ULONG],
            "CM_Get_DevNode_Registry_PropertyW": [w.ULONG, w.ULONG, _PULONG, c.c_void_p, _PULONG, w.ULONG],
            "CM_Open_DevNode_Key": [w.ULONG, w.ULONG, w.ULONG, w.ULONG, c.POINTER(w.HKEY), w.ULONG],
            "CM_Get_Device_Interface_List_SizeW": [_PULONG, c.POINTER(_Guid), w.LPCWSTR, w.ULONG],
            "CM_Get_Device_Interface_ListW": [c.POINTER(_Guid), w.LPCWSTR, w.LPWSTR, w.ULONG, w.ULONG],
        }, w.ULONG)

    def children(self, instance):
        root, node = w.ULONG(), w.ULONG()
        if self.dll.CM_Locate_DevNodeW(c.byref(root), instance, 0):
            return
        if self.dll.CM_Get_Child(c.byref(node), root.value, 0):
            return
        name = c.create_unicode_buffer(512)
        while True:
            if self.dll.CM_Get_Device_IDW(node.value, name, len(name), 0) == 0:
                yield node.value, name.value
            if self.dll.CM_Get_Sibling(c.byref(node), node.value, 0):
                return

    def is_winusb(self, node):
        buffer = c.create_unicode_buffer(32)
        size = w.ULONG(c.sizeof(buffer))
        # CM_DRP_SERVICE，读出 MI_06 对应子节点目前绑定的驱动。
        rc = self.dll.CM_Get_DevNode_Registry_PropertyW(node, 5, None, buffer, c.byref(size), 0)
        return rc == 0 and buffer.value.casefold() == "winusb"

    def interface_classes(self, node):
        key = w.HKEY()
        # KEY_QUERY_VALUE、RegDisposition_OpenExisting、CM_REGISTRY_HARDWARE。
        if self.dll.CM_Open_DevNode_Key(node, winreg.KEY_QUERY_VALUE, 0, 1, c.byref(key), 0):
            return set()
        classes = set()
        try:
            for name in ("DeviceInterfaceGUIDs", "DeviceInterfaceGUID"):
                try:
                    data, value_type = winreg.QueryValueEx(key.value, name)
                except OSError:
                    continue
                if value_type == winreg.REG_SZ:
                    data = [data]
                elif value_type != winreg.REG_MULTI_SZ:
                    continue
                for value in data:
                    try:
                        classes.add(UUID(value))
                    except (AttributeError, ValueError):
                        continue
        finally:
            winreg.CloseKey(key.value)
        return classes

    def paths(self, instance, interface_class):
        guid = _Guid.from_buffer_copy(interface_class.bytes_le)
        for _ in range(3):
            length = w.ULONG()
            if self.dll.CM_Get_Device_Interface_List_SizeW(c.byref(length), c.byref(guid), instance, 0):
                break
            if not 1 < length.value <= 32768:
                break
            buffer = c.create_unicode_buffer(length.value)
            status = self.dll.CM_Get_Device_Interface_ListW(c.byref(guid), instance, buffer, length.value, 0)
            if status == 0:
                raw = c.wstring_at(c.addressof(buffer), length.value)
                return {path for path in raw.split("\0") if path}
            if status != 0x1A:   # CR_BUFFER_SMALL，PnP 列表在两次调用间变动。
                break
        return set()

    def find(self, instance):
        if not _RECV.fullmatch(instance):
            return None
        candidates = [(node, name) for node, name in self.children(instance) if _MI_06.fullmatch(name)]
        if len(candidates) != 1:
            return None
        node, name = candidates[0]
        if not self.is_winusb(node):
            return None
        paths = set()
        for guid in self.interface_classes(node):
            paths.update(self.paths(name, guid))
        return next(iter(paths)) if len(paths) == 1 else None


class _WinUsbPipe:
    def __init__(self, api, handle):
        self.api, self.handle = api, handle
        self.buffer = (_UCHAR * 4096)()

    def read(self):
        count = w.ULONG()
        if self.api.WinUsb_ReadPipe(self.handle, dji.EP_IN, self.buffer, len(self.buffer), c.byref(count), None):
            return bytes(self.buffer[:count.value])
        error = c.get_last_error()
        if error in (121, 1460):      # ERROR_SEM_TIMEOUT / ERROR_TIMEOUT
            return b""
        raise c.WinError(error)


@contextmanager
def _open_pipe(path):
    kernel = c.WinDLL("kernel32", use_last_error=True)
    _bind(kernel, {"CreateFileW": [
        w.LPCWSTR, w.DWORD, w.DWORD, c.c_void_p, w.DWORD, w.DWORD, w.HANDLE,
    ]}, w.HANDLE)
    _bind(kernel, {"CloseHandle": [w.HANDLE]}, w.BOOL)
    api = c.WinDLL("winusb", use_last_error=True)
    _bind(api, {
        "WinUsb_Initialize": [w.HANDLE, c.POINTER(w.HANDLE)],
        "WinUsb_Free": [w.HANDLE],
        "WinUsb_GetCurrentAlternateSetting": [w.HANDLE, c.POINTER(_UCHAR)],
        "WinUsb_QueryInterfaceSettings": [w.HANDLE, _UCHAR, c.c_void_p],
        "WinUsb_QueryPipe": [w.HANDLE, _UCHAR, _UCHAR, c.POINTER(_PipeInformation)],
        "WinUsb_SetPipePolicy": [w.HANDLE, _UCHAR, w.ULONG, w.ULONG, c.c_void_p],
        "WinUsb_ReadPipe": [w.HANDLE, _UCHAR, c.c_void_p, w.ULONG, _PULONG, c.c_void_p],
    }, w.BOOL)
    # WinUSB 要求 FILE_FLAG_OVERLAPPED 与读写权限；下面仅访问读管道。
    handle = kernel.CreateFileW(path, 0xC0000000, 3, None, 3, 0x40000000, None)
    if handle in (None, _INVALID_HANDLE):
        raise c.WinError(c.get_last_error())
    interface = w.HANDLE()
    try:
        if not api.WinUsb_Initialize(handle, c.byref(interface)):
            raise c.WinError(c.get_last_error())
        try:
            alternate = _UCHAR()
            descriptor = (_UCHAR * 9)()
            if (not api.WinUsb_GetCurrentAlternateSetting(interface, c.byref(alternate))
                    or not api.WinUsb_QueryInterfaceSettings(interface, alternate.value, c.byref(descriptor))
                    or (descriptor[2], descriptor[5], descriptor[6]) != (dji.IFACE, 0xFF, 0xF0)):
                raise OSError("WinUSB 接口描述符与 DJI 状态接口不符")
            pipes = []
            for index in range(descriptor[4]):
                info = _PipeInformation()
                if not api.WinUsb_QueryPipe(interface, alternate.value, index, c.byref(info)):
                    raise c.WinError(c.get_last_error())
                if (info.pipe_id, info.pipe_type) == (dji.EP_IN, 2):  # UsbdPipeTypeBulk
                    pipes.append(info)
            if len(pipes) != 1:
                raise OSError("未找到唯一的 DJI bulk IN 管道")
            timeout = w.ULONG(dji.READ_TIMEOUT_MS)
            if not api.WinUsb_SetPipePolicy(interface, dji.EP_IN, 3, c.sizeof(timeout), c.byref(timeout)):
                raise c.WinError(c.get_last_error())
            yield _WinUsbPipe(api, interface)
        finally:
            api.WinUsb_Free(interface)
    finally:
        kernel.CloseHandle(handle)


class DjiWinUsbProvider:
    """与一个已登记的物理接收器关联，监听两个发射器的电量。"""

    target = dji.VID, dji.PID
    name = "dji_winusb"
    _leases_lock = threading.Lock()
    _leases = set()

    def __init__(self, receiver, *, find=None, open_pipe=_open_pipe, clock=time.monotonic):
        if (receiver.vid, receiver.pid) != self.target or not _RECV.fullmatch(receiver.instance):
            raise ValueError("无效的 DJI 接收器实例")
        self.instance = receiver.instance
        self.identity = receiver.instance.casefold()
        self._find, self._open_pipe, self._clock = find, open_pipe, clock
        self._done = threading.Event()
        self._settled = threading.Event()
        self._lock = threading.Lock()
        self._reader = None
        self._states = {}
        self._products = {}

    def _listen(self, path):
        remaining = b""
        with self._open_pipe(path) as pipe:
            while not self._done.is_set():
                chunk = pipe.read()
                if self._done.is_set():
                    break
                frames, remaining = dji.split_frames(remaining + chunk)
                for frame in frames:
                    self._accept(frame)

    def _run(self):
        while not self._done.is_set():
            with self._leases_lock:
                acquired = self.identity not in self._leases
                if acquired:
                    self._leases.add(self.identity)
            if acquired:
                try:
                    if self._find is None:
                        self._find = _CfgMgr().find
                    path = self._find(self.instance)
                    if path is not None and not self._done.is_set():
                        self._listen(path)
                except OSError:
                    # 接收器拔出、WinUSB 不可访问或被占用时，清空已失效的读数。
                    pass
                finally:
                    with self._lock:
                        self._states.clear()
                        self._products.clear()
                    with self._leases_lock:
                        self._leases.remove(self.identity)
            self._settled.set()
            self._done.wait(dji.RETRY_SECONDS)

    def _accept(self, frame):
        if not dji.crc_ok(frame):
            return
        status = dji.decode_status(frame)
        if status is None:
            if dji.record_tags(frame) & {0x01, 0x06}:
                with self._lock:
                    for slot, identity in dji.identity_records(frame).items():
                        product = identity.get("product")
                        if slot in ("tx1", "tx2") and product in ("DJI Mic Mini", "DJI Mic Mini 2"):
                            self._products[slot] = product
            return
        observed = self._clock()
        with self._lock:
            for slot, bit in (("tx1", 1), ("tx2", 2)):
                tx = status.get(slot)
                if frame[44] & bit:
                    if tx is not None:
                        self._states[slot] = (observed, tx["gauge"], tx["charging"], 0)
                elif slot in self._states:
                    when, gauge, charging, misses = self._states[slot]
                    if misses + 1 >= dji.GRACE_PUSHES:
                        del self._states[slot]
                    else:
                        self._states[slot] = (when, gauge, charging, misses + 1)
        self._settled.set()

    def poll(self):
        if self._done.is_set():
            return []
        if self._reader is None:
            self._reader = threading.Thread(target=self._run, name="daozai-dji-winusb", daemon=True)
            self._reader.start()
            self._settled.wait(dji.FIRST_PUSH_WAIT_SECONDS)
        result = []
        with self._lock:
            now = self._clock()
            for slot, (when, gauge, charging, _) in sorted(self._states.items()):
                if now - when > dji.STALE_SECONDS:
                    continue
                level = dji.GAUGE_TO_PERCENT.get(gauge)
                product = self._products.get(slot, "DJI Mic")
                result.append(DeviceStatus(
                    slot, f"{product} {slot.upper()}", level, charging,
                    source=self.name, kind="microphone",
                    approx=f"about {level}% (gauge {gauge} of 7)" if level is not None else "",
                    ui_params={"daozai_observed": when},
                ))
        return result

    def close(self):
        self._done.set()
        self._settled.set()
        if self._reader is not None:
            self._reader.join(dji.READ_TIMEOUT_MS / 1000 + 0.2)
