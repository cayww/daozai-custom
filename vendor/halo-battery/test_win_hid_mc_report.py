"""验证 Ultra+ 的报告 ID 准入只依赖零访问权限的 HID 元数据。"""

from types import SimpleNamespace
import unittest
from unittest.mock import Mock

from worker import WindowsHid
from win_hid import HIDP_STATUS_SUCCESS


class FakeHidMetadata:
    def __init__(self, *, vid=0x3837, pid=0x1018, page=0xFF01, usage=1,
                 input_length=64, output_length=64, report_results=None):
        self.attributes = vid, pid
        self.caps = page, usage, input_length, output_length
        self.report_results = report_results or {}
        self.report_calls = []
        self.preparsed = None

    def HidD_GetAttributes(self, handle, target):
        target._obj.vendor, target._obj.product = self.attributes
        return True

    def HidD_GetPreparsedData(self, handle, target):
        self.preparsed = 0x1234
        target._obj.value = self.preparsed
        return True

    def HidP_GetCaps(self, preparsed, target):
        self.require_preparsed(preparsed)
        caps = target._obj
        (caps.UsagePage, caps.Usage, caps.InputReportByteLength,
         caps.OutputReportByteLength) = self.caps
        return HIDP_STATUS_SUCCESS

    def HidD_FreePreparsedData(self, preparsed):
        self.require_preparsed(preparsed)
        self.preparsed = None

    def HidP_InitializeReportForID(self, kind, report_id, preparsed, buffer, length):
        self.require_preparsed(preparsed)
        assert bytes(buffer) == bytes(length)
        self.report_calls.append((kind, report_id, length))
        status, initialized_id = self.report_results.get(kind, (HIDP_STATUS_SUCCESS, report_id))
        buffer[0] = bytes((initialized_id,))
        return status

    def require_preparsed(self, pointer):
        assert self.preparsed is not None and pointer.value == self.preparsed


class WinHidMcReportTest(unittest.TestCase):
    def metadata(self, dll, *, interface=2, product="MCHOSE A7 V3 Ultra+", same_parent=True):
        vid, pid = dll.attributes
        path = f"hid&mi_{interface:02x}#test"
        root = SimpleNamespace(vid=vid, pid=pid, identity=f"usb\\vid_{vid:04x}&pid_{pid:04x}\\test")
        handle = SimpleNamespace(handle=1, close=Mock())
        windows = WindowsHid.__new__(WindowsHid)
        windows._file = SimpleNamespace(open_path=Mock(return_value=handle))
        windows._metadata = SimpleNamespace(
            dll=dll, string=lambda _handle, kind: product if kind == "Product" else None,
        )
        windows.parent_for = lambda _path: root.identity if same_parent else None

        metadata = windows.metadata(path, root)
        if same_parent:
            windows._file.open_path.assert_called_once_with(path, access=0)
            handle.close.assert_called_once_with()
        else:
            windows._file.open_path.assert_not_called()
        self.assertIsNone(dll.preparsed)
        return metadata

    def test_verifies_input_and_output_on_the_same_preparsed_data(self):
        dll = FakeHidMetadata()
        metadata = self.metadata(dll)
        self.assertIs(metadata["mc_report_verified"], True)
        self.assertEqual(dll.report_calls, [(0, 0x4D, 64), (1, 0x4D, 64)])
        self.assertEqual(metadata["product_string"], "MCHOSE A7 V3 Ultra+")

    def test_both_status_and_report_id_must_match(self):
        for report_type, result in (
            (0, (0xC011000A, 0x4D)),
            (1, (0xC011000A, 0x4D)),
            (0, (HIDP_STATUS_SUCCESS, 0)),
            (1, (HIDP_STATUS_SUCCESS, 0)),
        ):
            with self.subTest(report_type=report_type, result=result):
                dll = FakeHidMetadata(report_results={report_type: result})
                self.assertIs(self.metadata(dll)["mc_report_verified"], False)

    def test_other_models_interfaces_and_lengths_leave_report_unverified(self):
        for caps, metadata in (
            ({"pid": 0x100A}, {}),
            ({"vid": 0x5253}, {}),
            ({}, {"product": "MCHOSE A7 V3 Pro"}),
            ({}, {"product": "MCHOSE K7 V2 Ultra+"}),
            ({}, {"product": "MCHOSE A7 V3 Ultra+ unverified variant"}),
            ({}, {"interface": 1}),
            ({"page": 0xFF00}, {}),
            ({"usage": 2}, {}),
            ({"input_length": 65}, {}),
            ({"output_length": 63}, {}),
        ):
            with self.subTest(caps=caps, metadata=metadata):
                dll = FakeHidMetadata(**caps)
                self.assertIs(self.metadata(dll, **metadata)["mc_report_verified"], False)
                self.assertEqual(dll.report_calls, [])

    def test_does_not_open_path_under_another_usb_receiver(self):
        dll = FakeHidMetadata()
        self.assertIsNone(self.metadata(dll, same_parent=False))
        self.assertEqual(dll.report_calls, [])


if __name__ == "__main__":
    unittest.main()
