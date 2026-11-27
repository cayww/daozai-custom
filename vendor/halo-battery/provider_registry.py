"""上游 v1.18.0 的 HID 提供器注册顺序，协议由上游源码实现。"""

import time

from providers.razer import RazerProvider
from providers.blackshark_v3x import BlackSharkV3XProvider
from providers.audeze import AudezeProvider
from providers.wlmouse import WLmouseProvider
from providers import mchose
from providers.hyperx_alpha2 import HyperXAlpha2Provider
from providers.hyperx_cloud3 import HyperXCloud3Provider
from providers.hyperx_cloud_core import HyperXCloudCoreProvider
from providers.hyperx import HyperXProvider
from providers.keychron import KeychronProvider
from providers.pulsar import PulsarProvider
from providers.areson import AresonProvider
from providers.arbiter import ArbiterProvider
from providers.fantech import FantechProvider
from providers.msi import MsiProvider
from providers.wraith import WraithProvider
from providers.machenike import MachenikeProvider
from providers.moondrop import MoondropProvider
from providers.ninjutso_sprime import NinjutsoSprimeProvider
from providers.jbl import JblProvider
from providers.jbl_hidv3 import JblHidV3Provider
from providers.cherry import CherryProvider
from providers.coolermaster import CoolerMasterProvider
from providers.royal_kludge import RoyalKludgeProvider
from providers.zowie import ZowieProvider
from providers.fractal import FractalProvider
from providers.logitech import LogitechProvider
from providers.steelseries import SteelSeriesProvider
from providers.eightbitdo import EightBitDoProvider
from providers.barracuda import BarracudaProvider
from providers.barracuda_x import BarracudaXProvider
from providers.asus import AsusProvider
from providers.gwolves import GWolvesProvider
from providers.aula import AulaProvider
from providers.aula_f108 import AulaF108Provider
from providers.lofree import LofreeProvider
from providers.astro import AstroProvider
from providers.corsair import CorsairProvider
from providers.lamzu import LamzuProvider
from providers.io_aurora import IoAuroraProvider
from providers.vaxee import VaxeeProvider
from providers.am_infinity import AmInfinityProvider
from providers.steelseries_elite import SteelSeriesEliteProvider
from providers.logitech_centurion import LogitechCenturionProvider
from providers.logitech_g435_rtk import LogitechG435RtkProvider
from providers.hyperx_cloud3s import HyperXCloud3SProvider
from providers.nzxt import NzxtProvider
from providers.steam import SteamProvider
from providers.endgame_we import EndgameWEProvider
from providers.mchose_v9 import MchoseV9Provider
from providers.dareu import DareuProvider
from providers.lamzu import LamzuAtlantisProvider
from providers.inphic import InphicProvider
from providers.attackshark_r5 import AttackSharkR5Provider
from providers.turtlebeach import TurtleBeachProvider
from providers.skullcandy import SkullcandyProvider
from providers.rapoo import RapooProvider
from providers.ajazz import AjazzProvider
from providers.blackshark_mtk import BlackSharkMtkProvider
from providers.irocks import IRocksProvider
from providers.sony_inzone import SonyInzoneProvider
from providers.sony_h9ii import SonyH9IIProvider
from providers.atk_nearlink import AtkNearlinkProvider
from providers.finalmouse import FinalmouseProvider
from providers.atk_keyboard import AtkKeyboardProvider
from providers.attackshark_x66 import AttackSharkX66Provider
from providers.attackshark_r11 import AttackSharkR11Provider
from providers.hyperx_alpha_wireless import HyperXAlphaWirelessProvider
from providers.jabra import JabraProvider
from providers.hyperx_cloud_flight import HyperXCloudFlightProvider
from providers.monsgeek import MonsGeekProvider
from providers.royuan import RoyuanProvider
from providers.eweadn import EweadnProvider
from providers.eweadn_ydx import EweadnYdxProvider
from providers.mchose_kb import MchoseKbProvider
from providers.plantronics import PlantronicsProvider
from providers.hyperx_ngenuity import HyperXNgenuityProvider
from providers.alienware import AlienwareProvider
from providers.darmoshark import DarmosharkProvider
from providers.glorious import GloriousProvider
from providers.gs3075t import Gs3075tProvider
from providers.urx import UrxProvider
from providers.mchose_k99 import MchoseK99Provider
from providers.redsquare import RedSquareProvider
from providers.kraken_v4 import KrakenV4Provider
from providers.ninjutso_sora_v3 import NinjutsoSoraV3Provider
from providers.akko_s9 import AkkoS9Provider
from providers.logitech_g325 import LogitechG325Provider
from providers.jbl_quantum350 import JblQuantum350Provider
from providers import beitong, eightbitdo_ultimate2, flydigi, gamesir, raikiri
from providers.base import DeviceStatus, Provider

from feedback_receivers import DareuEk87ProProvider, MachenikeL8MaxProvider, VgnDragonfly3ProProvider
from mchose_a7 import PID as MC_PID, MchoseA7Provider, control_collections, product_name
from mchose_feedback import MchoseG3V2Provider, MchoseV9Pro2Provider
from mouse_receivers import RapooVt9AirProvider, VgnF1MobaProvider
from regional_receivers import EweadnX87Provider, MchoseG3AProvider, Wave75Provider


class MchoseReceiverProvider(mchose.MchoseProvider):
    """V9 Turbo+ 只使用实测应答的物理 USB 接收器端点。"""

    def poll(self):
        # 枚举和打开均由组件限制在本次请求指定的同一 USB 物理实例内。
        hid = mchose.hid
        turbo = hid.enumerate(mchose.NEW_VID, mchose.HEADSET_BODY_PID)
        if turbo:
            return self._poll_turbo_plus(turbo)
        if hid.enumerate(mchose.NEW_VID, mchose.HEADSET_DONGLE_PID):
            return []
        shared = hid.enumerate(mchose.NEW_VID, MC_PID)
        if shared:
            controls = control_collections(shared)
            if len(controls) != 1 or product_name(controls[0]) != "mchose k7 v2 ultra+":
                return []
        if any(MchoseG3AProvider.matches_product(row)
               for row in hid.enumerate(mchose.G7_VID, mchose.G7_PID)):
            return []
        return super().poll()

    def _poll_turbo_plus(self, rows):
        candidates = [row["path"] for row in rows
                      if (row.get("interface_number"), row.get("usage_page"), row.get("usage"))
                      == (0, 0xFF90, 0x0001)]
        if len(candidates) != 1:
            return []
        # #193 无线探针只有此集合应答 55 65 01，回包为 55 65 64 02。
        # https://github.com/user-attachments/files/33004819/probe-output-20261003-225111.txt
        query = bytes([mchose.AUDIO_REPORT]) + mchose.STATUS_REQUEST
        size = len(query)
        device = mchose.hid.device()
        try:
            device.open_path(candidates[0])
            caps = device.get_report_lengths()
            if (caps["input"], caps["output"]) != (size, size) or device.write(query) != size:
                return []
            deadline = time.monotonic() + mchose.HEADSET_READ_MS / 1000
            for _ in range(16):
                remaining = int((deadline - time.monotonic()) * 1000)
                if remaining < 1:
                    break
                response = device.read(size, remaining)
                if not response:
                    break
                if len(response) != size:
                    continue
                status = mchose.parse_status_55(response)
                if status is not None and status[0] is not None:
                    # 实机放电和底座充电返回相同状态，充电信息保持未知。
                    key = mchose.device_key(mchose.NEW_VID, mchose.HEADSET_BODY_PID)
                    return [DeviceStatus(key, "MCHOSE V9 Turbo+", status[0], source=self.name,
                                         kind="headset", charging_known=False)]
        except (OSError, ValueError):
            pass
        finally:
            device.close()
        return []


class SonyInzoneHidProvider(SonyInzoneProvider):
    """沿用 H5 的 HID 路线。H9 串口由按物理接收器绑定的提供器读取。"""

    def poll(self):
        self._diag = []
        return self._poll_hid()


class GamepadReceivers(Provider):
    """按已限定的接收器复用厂商 HID 查询。"""

    name = "gamepad_receivers"

    def poll(self):
        result = []
        for reader in (beitong, eightbitdo_ultimate2, flydigi, gamesir, raikiri):
            reading = reader.read_connected([])
            if reading is not None:
                result.append(DeviceStatus(
                    reader.__name__, reading.name, reading.level, reading.charging,
                    source=self.name, approx=reading.approx, kind="gamepad",
                    charging_known=getattr(reading, "charging_known", True),
                ))
        return result


PROVIDERS = (
    RazerProvider,
    BlackSharkV3XProvider,
    AudezeProvider,
    WLmouseProvider,
    MchoseReceiverProvider,
    MchoseA7Provider,
    HyperXAlpha2Provider,
    HyperXCloud3Provider,
    HyperXCloudCoreProvider,
    HyperXProvider,
    KeychronProvider,
    PulsarProvider,
    AresonProvider,
    ArbiterProvider,
    FantechProvider,
    MsiProvider,
    WraithProvider,
    MachenikeProvider,
    MoondropProvider,
    NinjutsoSprimeProvider,
    JblProvider,
    JblHidV3Provider,
    CherryProvider,
    CoolerMasterProvider,
    RoyalKludgeProvider,
    ZowieProvider,
    FractalProvider,
    LogitechProvider,
    SteelSeriesProvider,
    GamepadReceivers,
    EightBitDoProvider,
    BarracudaProvider,
    BarracudaXProvider,
    AsusProvider,
    GWolvesProvider,
    AulaProvider,
    AulaF108Provider,
    LofreeProvider,
    AstroProvider,
    CorsairProvider,
    LamzuProvider,
    IoAuroraProvider,
    VaxeeProvider,
    AmInfinityProvider,
    SteelSeriesEliteProvider,
    LogitechCenturionProvider,
    LogitechG435RtkProvider,
    HyperXCloud3SProvider,
    NzxtProvider,
    SteamProvider,
    EndgameWEProvider,
    MchoseV9Provider,
    DareuProvider,
    LamzuAtlantisProvider,
    InphicProvider,
    AttackSharkR5Provider,
    TurtleBeachProvider,
    SkullcandyProvider,
    RapooProvider,
    AjazzProvider,
    BlackSharkMtkProvider,
    IRocksProvider,
    SonyInzoneHidProvider,
    SonyH9IIProvider,
    AtkNearlinkProvider,
    FinalmouseProvider,
    AtkKeyboardProvider,
    AttackSharkX66Provider,
    AttackSharkR11Provider,
    HyperXAlphaWirelessProvider,
    JabraProvider,
    HyperXCloudFlightProvider,
    MonsGeekProvider,
    RoyuanProvider,
    EweadnProvider,
    EweadnYdxProvider,
    MchoseKbProvider,
    PlantronicsProvider,
    HyperXNgenuityProvider,
    AlienwareProvider,
    DarmosharkProvider,
    GloriousProvider,
    Gs3075tProvider,
    UrxProvider,
    MchoseK99Provider,
    RedSquareProvider,
    KrakenV4Provider,
    NinjutsoSoraV3Provider,
    AkkoS9Provider,
    LogitechG325Provider,
    JblQuantum350Provider,
    RapooVt9AirProvider,
    VgnF1MobaProvider,
    Wave75Provider,
    MchoseG3AProvider,
    EweadnX87Provider,
    MchoseG3V2Provider,
    MchoseV9Pro2Provider,
    MachenikeL8MaxProvider,
    VgnDragonfly3ProProvider,
    DareuEk87ProProvider,
)
