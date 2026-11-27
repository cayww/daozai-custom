"""仅在 A7 V3 Ultra+ 自身刚产生鼠标输入时采集电量。"""

import threading
import time

from mouse_activity import MouseActivity
from providers import mchose
from providers.base import DeviceStatus, Provider


PID = 0x1018
PRODUCT = "mchose a7 v3 ultra+"
CONTROL = (2, 0xFF01, 0x0001)
ACTIVE_WINDOW = 0.25
REFRESH_INTERVAL = 60.0
ACTIVITY = MouseActivity()


def control_collections(rows):
    return [row for row in rows if (row.get("interface_number"), row.get("usage_page"),
                                    row.get("usage")) == CONTROL]


def product_name(row):
    return (row.get("product_string") or "").strip().casefold()


class MchoseA7Provider(Provider):
    name = "mchose_a7_v3_ultra_plus"
    poll_on_activity = True

    def __init__(self):
        self.activity = None
        self.wake = None
        self._paths = frozenset()
        self._lock = threading.Lock()
        self._next_request = self._next_wake = 0.0
        self._last_event = None
        self._sample = None

    def _on_input(self):
        now = time.monotonic()
        with self._lock:
            if now < max(self._next_request, self._next_wake):
                return
            self._next_wake = now + 0.5
        if self.wake is not None:
            self.wake()

    def _bind(self, paths):
        if paths == self._paths:
            return
        if self.activity is not None:
            self.activity.close()
        self._paths = paths
        self.activity = ACTIVITY.subscribe(paths, self._on_input) if paths else None

    def _deadline(self):
        at = self.activity.latest() if self.activity is not None else None
        now = time.monotonic()
        with self._lock:
            if (at is None or at == self._last_event or now < self._next_request
                    or not 0 <= now - at < ACTIVE_WINDOW):
                return None
            self._last_event = at
            self._next_request = now + REFRESH_INTERVAL
            return at + ACTIVE_WINDOW

    def poll(self):
        hid = mchose.hid
        rows = hid.enumerate(mchose.NEW_VID, PID)
        controls = control_collections(rows)
        if (len(controls) != 1 or product_name(controls[0]) != PRODUCT
                or controls[0].get("mc_report_verified") is not True):
            self._bind(frozenset())
            self._sample = None
            return []

        # 所有路径已由组件的 Gateway 验证属于同一 USB 物理实例。
        self._bind(frozenset(row["path"] for row in rows
                            if (row.get("usage_page"), row.get("usage")) == (0x0001, 0x0002)))
        deadline = self._deadline()
        if deadline is not None:
            self._read(controls[0]["path"], deadline)

        if self._sample is None:
            return []
        level, charging, measured = self._sample
        return [DeviceStatus(self.name, "迈从 A7 V3 Ultra+", level, charging,
                             source=self.name, kind="mouse", ui_params={"daozai_observed": measured})]

    def _read(self, path, deadline):
        # #536 通过 Win32 证明 MI_02 支持双向 4D/64，使用唯一 0900 电量命令。
        # https://github.com/wushuai0802/myfinder-device-battery/pull/536
        if time.monotonic() >= deadline:
            return
        device = mchose.hid.device()
        try:
            device.open_path(path)
            lengths = device.get_report_lengths()
            if (lengths["input"], lengths["output"]) != (mchose.MC_LEN, mchose.MC_LEN):
                return
            device.set_nonblocking(True)
            for _ in range(4):
                if time.monotonic() >= deadline:
                    return
                if not device.read(mchose.MC_LEN, 0):
                    break
            else:
                return
            if device.write(mchose.make_mc_request(), deadline=deadline) != mchose.MC_LEN:
                return
            until = time.monotonic() + 0.35
            for _ in range(6):
                remaining = int((until - time.monotonic()) * 1000)
                if remaining < 1:
                    return
                reply = device.read(mchose.MC_LEN, remaining)
                if not reply:
                    return
                if (reading := mchose.parse_mc_info(reply)) is not None:
                    self._sample = (*reading, time.monotonic())
                    return
        except (OSError, ValueError):
            return
        finally:
            device.close()
