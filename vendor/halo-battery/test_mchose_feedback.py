"""验证迈从反馈型号的接口门槛、读数和未知状态。"""

import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

from mchose_feedback import G3_ONLINE, G3_QUERY, V9_QUERY, MchoseG3V2Provider, MchoseV9Pro2Provider, hidlist


class MchoseFeedbackTests(unittest.TestCase):
    def poll(self, provider, rows, replies=(), lengths=(64, 64)):
        device = Mock()
        device.write.side_effect = len
        device.get_report_lengths.return_value = dict(zip(("input", "output"), lengths))
        frames = iter(replies)
        device.read.side_effect = lambda size, timeout: next(frames, []) if timeout else []
        discovery = Mock(return_value=rows)
        with patch.object(hidlist, "hid", SimpleNamespace(device=lambda: device, enumerate=discovery)):
            result = provider.poll()
        return result, device, discovery

    def test_v9_pro_2_returns_level_only(self):
        row = {"path": b"wireless", "interface_number": 3, "usage_page": 0xFF07,
               "usage": 0x0222, "product_string": "MCHOSE V9 Pro 2", "vendor_id": 0x3837, "product_id": 0x6011}
        online = bytes.fromhex("55 AA 01 00 03 00 01 0B 01 D0").ljust(64, b"\0")
        result, device, discovery = self.poll(MchoseV9Pro2Provider(), [row], [V9_QUERY, online])
        discovery.assert_called_once_with(0x3837, 0x6011)
        device.open_path.assert_called_once_with(b"wireless")
        device.write.assert_called_once_with(V9_QUERY)
        self.assertEqual(result[0].level, 80)
        self.assertFalse(result[0].charging_known)

    def test_v9_ignores_offline_and_wrong_interface(self):
        row = {"path": b"wireless", "interface_number": 3, "usage_page": 0xFF07,
               "usage": 0x0222, "product_string": "MCHOSE V9 Pro 2", "vendor_id": 0x3837, "product_id": 0x6011}
        offline = bytes.fromhex("55 AA 01 00 03 00 01 0B 00 50").ljust(64, b"\0")
        result, _, _ = self.poll(MchoseV9Pro2Provider(), [row], [offline])
        self.assertEqual(result, [])
        result, device, _ = self.poll(MchoseV9Pro2Provider(), [{**row, "interface_number": 1}])
        self.assertEqual(result, [])
        device.open_path.assert_not_called()

    def test_g3_v2_reads_only_when_receiver_reports_online(self):
        row = {"path": b"wireless", "interface_number": 1, "usage_page": 0xFF01,
               "usage": 0x0010, "product_string": "MCHOSE G3 V2", "vendor_id": 0x3837, "product_id": 0x4245}
        online = bytes.fromhex("AA ED 00 00 00 00 00 00 02").ljust(64, b"\0")
        battery = bytes.fromhex("AA 30 00 00 00 00 00 00 43 01").ljust(64, b"\0")
        result, device, discovery = self.poll(MchoseG3V2Provider(), [row], [online, battery], (65, 65))
        discovery.assert_called_once_with(0x3837, 0x4245)
        self.assertEqual([call.args[0] for call in device.write.call_args_list], [G3_ONLINE, G3_QUERY])
        self.assertEqual(result[0].level, 67)
        self.assertTrue(result[0].charging and result[0].charging_known)

        offline = online[:8] + b"\x01" + online[9:]
        result, device, _ = self.poll(MchoseG3V2Provider(), [row], [offline], (65, 65))
        self.assertEqual(result, [])
        device.write.assert_called_once_with(G3_ONLINE)
