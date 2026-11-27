"""逐型号核对过 HID 电量查询的无线鼠标接收器。"""

from time import sleep

import hid

from providers.base import DeviceStatus, Provider
from providers import compx


_RAPOO_REPORT_ID = 0xBA
_RAPOO_REPORT_LEN = 32
_RAPOO_QUERY = bytes((_RAPOO_REPORT_ID, 0x00, 0xAA, 0x00)) + bytes(_RAPOO_REPORT_LEN - 4)


def _rapoo_battery(report):
    data = bytes(report)
    # Windows 可能返回去掉报告 ID 的载荷，保留 ID 的驱动也用同一布局。
    if data[:1] == bytes([_RAPOO_REPORT_ID]):
        data = data[1:]
    if len(data) < 3 or data[0] != 1 or data[1] not in (1, 2) or data[2] > 100:
        return None
    return data[2], data[1] == 2


def _read_rapoo(device, caps):
    # #267 的 VT9 Air 在 MI_01 / FF00:000E 以 BA 00 AA 00 查询，控制输入返回 01 01 5E。
    # https://github.com/Renly1994/xiaotang-daozai/issues/267#issuecomment-7209848666
    # https://github.com/OpenMouse-Project/mouse-protocol/blob/main/docs/rapoo-vt9-pro-testing.md
    if (caps["input"], caps["output"]) != (_RAPOO_REPORT_LEN, _RAPOO_REPORT_LEN):
        return None
    for _ in range(4):
        if device.write(_RAPOO_QUERY) != _RAPOO_REPORT_LEN:
            return None
        for _ in range(4):
            sleep(0.03)
            try:
                report = device.get_input_report(_RAPOO_REPORT_ID, _RAPOO_REPORT_LEN)
            except TimeoutError:
                return None
            except OSError:
                # 控制输入暂时未缓冲时仍可能在下一次读到。
                continue
            if (reading := _rapoo_battery(report)) is not None:
                return reading
    return None


def _read_vgn(device, caps):
    # #592 记录了 F503 的 MI_01 / FF02:0002 及 17 字节 CompX 输出和回包。
    # https://github.com/wushuai0802/myfinder-device-battery/issues/592
    size = compx.PAYLOAD_LEN
    if caps["output"] != size or caps["input"] < size:
        return None
    for _ in range(compx.READ_ATTEMPTS):
        if not device.read(size, 15):
            break
    query = bytes(compx.make_request())
    if device.write(query) != size:
        return None
    for _ in range(compx.READ_ATTEMPTS):
        report = bytes(device.read(size, compx.READ_TIMEOUT_MS))
        if not report:
            break
        if len(report) != size or report == query:
            continue
        if (reading := compx.parse_power(report)) is not None:
            level, charging = reading
            return level, charging if report[compx.POWER_INDEX] in (0, 1) else None
    return None


def _poll_mouse(vid, pid, usage, product, source, label, read):
    # 现有 Gateway 负责核对 USB 物理父实例。这里只接受实测的接口和用途。
    paths = [row["path"] for row in hid.enumerate(vid, pid)
             if (row["vendor_id"], row["product_id"], row["interface_number"]) == (vid, pid, 1)
             and (row["usage_page"], row["usage"]) == usage
             and (row.get("product_string") or "").strip().casefold() == product]
    if len(paths) != 1:
        return []
    device = hid.device()
    try:
        device.open_path(paths[0])
        reading = read(device, device.get_report_lengths())
    except (OSError, ValueError):
        return []
    finally:
        try:
            device.close()
        except OSError:
            pass
    if reading is None:
        return []
    level, charging = reading
    return [DeviceStatus(source, label, level, bool(charging), source=source, kind="mouse",
                         charging_known=charging is not None)]


class RapooVt9AirProvider(Provider):
    name = "rapoo_vt9_air"

    def poll(self):
        return _poll_mouse(0x24AE, 0x1200, (0xFF00, 0x000E), "rapoo gaming device",
                           self.name, "雷柏 VT9 Air", _read_rapoo)


class VgnF1MobaProvider(Provider):
    name = "vgn_f1_moba"

    def poll(self):
        return _poll_mouse(0x3554, 0xF503, compx.CONTROL_USAGE, "vgn mouse 2.4g receiver",
                           self.name, "VGN 蜻蜓 F1 MOBA", _read_vgn)
