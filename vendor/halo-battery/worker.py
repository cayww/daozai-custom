"""岛仔可选 2.4 GHz 电量组件的标准输入输出入口。"""

from __future__ import annotations

import hashlib
import json
import logging
from contextlib import contextmanager
from dataclasses import dataclass, replace
from pathlib import Path
import sys
import threading
import time
from types import ModuleType


if not getattr(sys, "frozen", False):
    sys.path.insert(0, str(Path(__file__).resolve().parent / "upstream"))

from halo.diagnostic_guard import InstanceGuard
from halo.hid_transport import TransportRuntime
from halo.polling import PollingRuntime
from halo.provider_policy import HELD_PROVIDER_REASONS
from providers.base import is_coarse_reading
from win_hid import USB_INSTANCE, WindowsHid


VERSION = 1
UPSTREAM = "1.18.0"
MAX_RECEIVERS = 256
MAX_LINE = 1024 * 1024
REPLY_WAIT = 3.0
AUXILIARY_FIELDS = ("daozai_instance", "daozai_generation", "daozai_observed")
log = logging.getLogger("daozai-halo")


@dataclass(frozen=True)
class Receiver:
    vid: int
    pid: int
    instance: str

    @property
    def identity(self):
        return self.instance.casefold()


def read_request(line):
    def unique(pairs):
        value = dict(pairs)
        if len(value) != len(pairs):
            raise ValueError("请求字段重复")
        return value

    req = json.loads(line, object_pairs_hook=unique)
    if (not isinstance(req, dict) or set(req) != {"version", "id", "command", "receivers"}
            or type(req["version"]) is not int or req["version"] != VERSION
            or type(req["id"]) is not int or not 0 <= req["id"] < 2**64
            or req["command"] != "poll"):
        raise ValueError("无效的轮询请求")
    if not isinstance(req["receivers"], list) or len(req["receivers"]) > MAX_RECEIVERS:
        raise ValueError("设备列表超出限制")
    receivers = {}
    for raw in req["receivers"]:
        if not isinstance(raw, dict) or set(raw) != {"vid", "pid", "usb_instance"}:
            raise ValueError("无效的 USB 设备信息")
        vid, pid, path = raw["vid"], raw["pid"], raw["usb_instance"]
        match = USB_INSTANCE.fullmatch(path) if isinstance(path, str) and len(path) <= 512 else None
        if (match is None or any(ord(c) < 32 or ord(c) == 127 for c in path)
                or type(vid) is not int or type(pid) is not int
                or (vid, pid) != (int(match[1], 16), int(match[2], 16))):
            raise ValueError("USB 接收器实例与 VID/PID 不一致")
        root = Receiver(vid, pid, path)
        if root.identity in receivers:
            raise ValueError("USB 实例重复")
        receivers[root.identity] = root
    return req["id"], list(receivers.values())


class ReceiverScope:
    def __init__(self, root, paths, generation):
        self.root, self.paths, self.generation = root, paths, generation
        self.metadata = {}
        self.lock = threading.Lock()


class ScopedDevice:
    def __init__(self, gate):
        self.gate, self.device = gate, gate.transport.device()
        self.path = None
        self.scope = None

    def open_path(self, path):
        clean = path.decode("utf-8") if isinstance(path, bytes) else path
        if not isinstance(clean, str):
            raise OSError("无效的 HID 路径")
        clean = clean.casefold()
        scope = self.gate.inventory[1].get(clean)
        local = getattr(self.gate.local, "scope", None)
        if (scope is None or (local is not None and scope is not local)
                or self.gate.windows.parent_for(clean) != scope.root.identity):
            raise OSError("HID 路径不在当前接收器范围")
        self.device.open_path(path)
        self.path, self.scope = clean, scope

    def close(self):
        self.device.close()
        self.path = self.scope = None

    def __getattr__(self, name):
        method = getattr(self.device, name)

        def checked(*args, **kwargs):
            if self.path is None or self.gate.inventory[1].get(self.path) is not self.scope:
                raise OSError("接收器已离开当前采集范围")
            result = method(*args, **kwargs)
            if (result and name in ("read", "get_input_report", "get_feature_report")
                    and getattr(self.gate.local, "scope", None) is self.scope):
                self.gate.local.observed = time.monotonic()
            return result

        return checked


class Gateway:
    def __init__(self, transport, windows, direct_targets=()):
        self.transport, self.windows = transport, windows
        self.direct_targets = frozenset(direct_targets)
        self.inventory = {}, {}
        self.local = threading.local()
        self.generation = 0

    def update(self, receivers):
        current = self.windows.paths_by_receiver(receivers)
        previous = self.inventory[0]
        receivers_by_id, paths = {}, {}
        for root in receivers:
            paths_for_root = current.get(root.identity, ())
            if not paths_for_root and (root.vid, root.pid) not in self.direct_targets:
                continue
            scope = previous.get(root.identity)
            if scope is None or scope.paths != paths_for_root:
                self.generation += 1
                scope = ReceiverScope(root, paths_for_root, self.generation)
            receivers_by_id[root.identity] = scope
            paths.update((p, scope) for p in paths_for_root)
        self.inventory = receivers_by_id, paths

    @contextmanager
    def scope(self, receiver):
        self.local.scope = receiver
        self.local.observed = None
        try:
            yield
        finally:
            self.local.scope = None
            self.local.observed = None

    def paths(self):
        scope = getattr(self.local, "scope", None)
        return frozenset(scope.paths if scope else ())

    def enumerate(self, vendor_id=0, product_id=0):
        scope = getattr(self.local, "scope", None)
        if (scope is None or (vendor_id and vendor_id != scope.root.vid)
                or (product_id and product_id != scope.root.pid)):
            return []
        rows = []
        for path in scope.paths:
            with scope.lock:
                hit = scope.metadata.get(path)
            if hit is None or (hit[0] is None and time.monotonic() - hit[1] >= 30):
                row = self.transport.invoke(
                    lambda: self.windows.metadata(path, scope.root),
                    "metadata:" + hashlib.sha256(path.encode("utf-8")).hexdigest(),
                )
                with scope.lock:
                    scope.metadata[path] = row, time.monotonic()
            else:
                row = hit[0]
            if row is not None:
                rows.append(dict(row))
        return rows

    def install(self, module):
        module.enumerate = self.enumerate
        module.device = lambda: ScopedDevice(self)


class ScopedProvider:
    """每个物理接收器使用独立的上游实例，隔离相同 PID 与上游缓存。"""

    def __init__(self, factory, gate):
        self.factory, self.gate = factory, gate
        self.name = factory.name
        self.target = getattr(factory, "target", None)
        self.instances = {}
        self.pending = False
        self.wake = None

    def poll(self):
        current = self.gate.inventory[0]
        self.pending = False
        for receiver in list(self.instances):
            if receiver not in current or self.instances[receiver][0] is not current[receiver]:
                _, provider = self.instances.pop(receiver)
                self.close_provider(provider)
        results = []
        for root, scope in current.items():
            receiver = scope.root
            if self.target is not None:
                if (receiver.vid, receiver.pid) != self.target:
                    continue
            elif not scope.paths:
                continue
            if root not in self.instances:
                provider = self.factory(receiver) if self.target is not None else self.factory()
                if self.wake is not None:
                    provider.wake = self.wake
                self.instances[root] = scope, provider
            _, provider = self.instances[root]
            with self.gate.scope(scope):
                statuses = provider.poll()
                report_at = self.gate.local.observed
            self.pending |= getattr(provider, "pending", False) is True
            limit = 256 - len(results)
            eligible = [s for s in statuses if s.via.casefold() not in {"bluetooth", "usb", "usb cable", "wired"}]
            for status in eligible[:limit]:
                key = hashlib.sha256(f"{self.name}\0{root}\0{status.key}".encode("utf-8")).hexdigest()
                # JBL 的 upstream online 状态只在本次消费到新电量报告时为真。
                if self.wake is not None:
                    # 活动触发的结果可能到下一次宿主轮询才被消费，沿用实际收到时刻。
                    at = status.ui_params.get(AUXILIARY_FIELDS[2])
                elif self.target is not None:
                    at = status.ui_params.get(AUXILIARY_FIELDS[2], time.monotonic())
                else:
                    at = time.monotonic() if self.name == "jbl" and status.online else report_at
                observed = (at if status.online and not status.polling_paused and not status.polling_stale
                            else None)
                results.append(replace(status, key=key, ui_params={
                    **status.ui_params, **dict(zip(AUXILIARY_FIELDS, (root, scope.generation, observed))),
                }))
            if len(results) >= 256:
                break
        return results

    def diagnostics(self):
        return []

    def close_provider(self, provider):
        if self.target is not None:
            close = getattr(provider, "close", None)
            if close is not None:
                close()
            return
        for reader in getattr(provider, "_readers", {}).values():
            reader.stop()
        activity = getattr(provider, "activity", None)
        if activity is not None:
            activity.close()

    def close(self):
        for _, provider in self.instances.values():
            self.close_provider(provider)


class Engine:
    def __init__(self, gate, factories):
        self.gate = gate
        self.observed = {}
        ready = self.changed = threading.Event()
        self.jobs = [ScopedProvider(f, gate) for f in factories if f.name not in HELD_PROVIDER_REASONS]
        transport = gate.transport
        self.polling = PollingRuntime(
            self.jobs, lambda _full: ready.set(),
            provider_scope=transport.provider_scope, transport_idle=transport.transport_idle,
            cancel_owner=transport.cancel_owner,
        )
        for job in self.jobs:
            if getattr(job.factory, "poll_on_activity", False):
                job.wake = lambda job=job: self.polling.request_poll(provider=job)
        self.polling.configure(HELD_PROVIDER_REASONS)

    def poll(self, receivers):
        if not receivers and not self.gate.inventory[0]:
            return []
        self.gate.update(receivers)
        self.polling.request_poll()
        completed, fresh = set(), set()
        wait_until = time.monotonic() + REPLY_WAIT
        while True:
            self.changed.clear()
            batch = self.polling.consume()
            fresh.update(batch.fresh_keys)
            completed.update(batch.completed)
            if (not self.gate.inventory[0] or len(completed) == len(self.jobs)
                    or time.monotonic() >= wait_until):
                break
            self.changed.wait(max(0, wait_until - time.monotonic()))
        return self.project(batch.hid, fresh)

    def project(self, statuses, fresh):
        output = {}
        now = time.monotonic()
        current = self.gate.inventory[0]
        seen = {}
        for st in statuses:
            root, generation, at = (st.ui_params.get(field) for field in AUXILIARY_FIELDS)
            scope = current.get(root)
            if scope is None or generation != scope.generation:
                continue
            previous = self.observed.get(st.key)
            coarse = is_coarse_reading(st)
            level = (st.level if type(st.level) is int and 0 <= st.level <= 100
                     and (not st.approx or coarse) else None)
            charging = st.charging if st.charging_known and type(st.charging) is bool else None
            current_state = level, charging, coarse
            if st.key in fresh and at is not None and (level is not None or charging is not None):
                measurement = at, current_state
            else:
                measurement = previous
            if measurement is not None and measurement[1] == current_state:
                seen[st.key] = measurement
                age = max(0, int((now - measurement[0]) * 1000))
            else:
                age = None
            device = {
                "key": st.key, "name": st.name[:256], "kind": st.kind,
                "level": level, "charging": charging, "online": st.online,
                "approximate": coarse, "fresh": age is not None and not st.polling_stale,
                "age_ms": age, "vid": scope.root.vid,
                "pid": scope.root.pid, "usb_instance": scope.root.instance,
            }
            output[st.key] = device
        self.observed = seen
        return list(output.values())[:MAX_RECEIVERS]

    def close(self):
        self.gate.inventory = {}, {}
        clean = self.polling.close()
        if clean:
            for job in self.jobs:
                job.close()
        return self.gate.transport.close() and clean


def write_reply(stream, value):
    data = json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
    if len(data) >= MAX_LINE:
        raise ValueError("回复超出尺寸限制")
    stream.write(data + b"\n")
    stream.flush()


def run_stdio(engine, stdin, stdout):
    write_reply(stdout, {"version": VERSION, "event": "ready", "upstream": UPSTREAM})
    while line := stdin.readline(MAX_LINE + 1):
        if len(line) > MAX_LINE:
            raise ValueError("请求超出尺寸限制")
        request_id, receivers = read_request(line)
        devices = engine.poll(receivers)
        write_reply(stdout, {"version": VERSION, "id": request_id, "devices": devices})


def main():
    if sys.argv[1:] == ["--licenses"]:
        if getattr(sys, "frozen", False):
            files = sorted((Path(sys._MEIPASS) / "licenses").rglob("*"))
        else:
            root = Path(__file__).resolve().parent / "upstream"
            files = [root / "LICENSE", root / "docs" / "credits.md"]
        for file in files:
            if file.is_file():
                sys.stdout.buffer.write((file.name + "\n").encode("utf-8") + file.read_bytes() + b"\n")
        return 0
    if sys.argv[1:] != ["--stdio"] or sys.platform != "win32":
        return 2
    # 沿用 HaloBattery 的单实例保护，避免重复采集同一设备。
    guard = InstanceGuard()
    engine = None
    try:
        if not guard.acquire():
            return 3
        hid = sys.modules["hid"] = ModuleType("hid")
        from providers import hidlist
        from provider_registry import PROVIDERS
        from sony_serial import SonySerialProvider
        from dji_winusb import DjiWinUsbProvider

        transport = TransportRuntime()
        direct = (SonySerialProvider, DjiWinUsbProvider)
        gate = Gateway(transport, WindowsHid(hidlist.interface_paths), (p.target for p in direct))
        gate.install(hid)
        hidlist.interface_paths = gate.paths
        engine = Engine(gate, (*PROVIDERS, *direct))
        run_stdio(engine, sys.stdin.buffer, sys.stdout.buffer)
        return 0
    except (OSError, ValueError) as exc:
        log.error("组件终止：%s", type(exc).__name__)
        return 1
    finally:
        if engine is not None:
            engine.close()
        guard.close()


if __name__ == "__main__":
    raise SystemExit(main())
