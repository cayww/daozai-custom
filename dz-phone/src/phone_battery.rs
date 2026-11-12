//! 手机连接（Phone Link）电量：读取 Windows 开始菜单小组件的卡片数据。
//!
//! 手机连接会把已配对手机的电量写进 StartMenuCompanion.json，
//! 本模块只读本地文件，不联网、不依赖手机连接的私有接口。
//! 卡片里只有电量百分比和连接状态，没有充电标志：默认按电量涨跌推测，
//! 也可由外部自动化写 phone_charging.txt 精确覆盖。
//! 卡片里的中文是 JSON 的 \uXXXX 转义，判断文案前统一解码。

use regex::Regex;
use std::{path::{Path, PathBuf}, time::{Duration, Instant, SystemTime}};

const RELATIVE: &str = r"Packages\Microsoft.YourPhone_8wekyb3d8bbwe\LocalState\StartMenu\StartMenuCompanion.json";
/// 卡片刷新很频繁，10 秒读一次足够；文件一有变化则最快 1 秒内重读，
/// 这样手机断开后电量环能及时收起，而不是等满一个轮询周期。
const INTERVAL: Duration = Duration::from_secs(10);
const MIN_RECHECK: Duration = Duration::from_secs(1);
/// 卡片没有明确连接状态时，超过 2 小时没更新才视为手机已断开（兜底）。
const STALE: Duration = Duration::from_secs(2 * 60 * 60);
/// 明确仍连接、但卡片这一帧没写电量数值时的宽限，避免刷新瞬间闪烁。
const MISSING_GRACE: Duration = Duration::from_secs(3 * 60);
/// 充电中若电量长时间不再上涨，退出充电态，避免拔电后一直显示绿色。
const CHARGE_IDLE: Duration = Duration::from_secs(20 * 60);

#[derive(Default)]
pub struct Reader {
    checked: Option<Instant>,
    last_modified: Option<SystemTime>,
    /// 卡片读不到电量数值的开始时刻（用于宽限）。
    missing_since: Option<Instant>,
    level: Option<u8>,
    charging: bool,
    charged_at: Option<Instant>,
    device: String,
}

impl Reader {
    /// 返回 (电量百分比, 是否充电)；手机断开或数据不可用时返回 (None, false)。
    /// 卡片没有充电标志时按电量变化推测，或由外部文件精确覆盖。
    pub fn poll(&mut self) -> (Option<u8>, bool) {
        let path = card_path();
        let modified = path.as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|meta| meta.modified().ok());
        let changed = modified != self.last_modified;
        // 先限制最高重读频率，再看是否需要提前重读：
        // 文件改动（连接/断开、卡片刷新）后 1 秒内就能反映，稳定时每 10 秒一次。
        if self.checked.is_some_and(|at| at.elapsed() < MIN_RECHECK) {
            return (self.level, self.charging);
        }
        if !changed && self.checked.is_some_and(|at| at.elapsed() < INTERVAL) {
            return (self.level, self.charging);
        }
        self.checked = Some(Instant::now());
        self.last_modified = modified;

        let Some(path) = path else { self.clear(); return (None, false); };
        let Ok(text) = std::fs::read_to_string(&path) else { self.clear(); return (None, false); };
        // 中文文案在 JSON 里是 \uXXXX，解码后再判连接状态与电量。
        let plain = decode_escapes(&text);
        self.evaluate(&plain, fresh(&path, STALE))
    }

    /// 按卡片文本更新状态，与文件读取 / 节流解耦，
    /// 这样「已连接 → 断开」的状态切换能用单测直接覆盖。
    fn evaluate(&mut self, plain: &str, is_fresh: bool) -> (Option<u8>, bool) {
        let level = parse_level(plain);
        // 连接态必须先于「沿用缓存」判断：断开时移除电量数值，避免断开后仍显示旧电量。
        if !card_usable(plain, level, is_fresh) {
            self.clear();
            return (None, false);
        }
        let Some(level) = level else {
            // 明确仍连接但这一帧没写电量：给 3 分钟宽限，之后收起。
            let since = *self.missing_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= MISSING_GRACE {
                self.clear();
                return (None, false);
            }
            return (self.level, self.charging);
        };
        self.missing_since = None;
        // 卡片没有充电字段：优先用外部精确状态，否则按电量涨跌推测；
        // 电量长期不涨时自动退出充电态，避免拔电后一直保持绿色。
        self.charging = match override_charging() {
            // 精确覆盖由外部负责，不做超时回落；清掉计时器，删文件后立刻回到推测。
            Some(state) => { self.charged_at = None; state },
            None => {
                let rising = self.level.is_some_and(|old| level > old);
                if rising { self.charged_at = Some(Instant::now()); }
                let next = next_charging(self.level, level, self.charging);
                // 没有上涨记录也视为超时，避免覆盖文件撤销后一直保持绿色。
                let idle = self.charged_at.map_or(true, |at| at.elapsed() > CHARGE_IDLE);
                if next && level < 100 && idle { false } else { next }
            },
        };
        self.level = Some(level);
        self.device = parse_device(plain).unwrap_or_default();
        (self.level, self.charging)
    }

    /// 断开 / 数据失效时清空所有缓存，避免 UI 继续显示上一次的电量。
    fn clear(&mut self) {
        self.level = None;
        self.charging = false;
        self.charged_at = None;
        self.missing_since = None;
        self.device.clear();
    }

    pub fn device(&self) -> &str { &self.device }
}

/// 可选精确充电状态：外部自动化把 `1` / `0` 写进
/// `%APPDATA%\Daozai\phone_charging.txt` 即可覆盖推测值
/// （例如手机端 Tasker 在插拔充电器时同步该文件）。
/// 文件不存在、写着 auto 或其它内容时返回 None，沿用推测逻辑。
fn override_charging() -> Option<bool> {
    let base = std::env::var_os("APPDATA")?;
    let path = PathBuf::from(base).join("Daozai").join("phone_charging.txt");
    parse_override(&std::fs::read_to_string(path).ok()?)
}

fn parse_override(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "charging" | "充电" => Some(true),
        "0" | "false" | "discharging" | "未充电" => Some(false),
        _ => None,
    }
}

/// 「手机连接」是否装在本机：只看包目录在不在，不依赖卡片是否已生成。
/// 没装的话设置页不允许开启手机电量。
pub fn installed() -> bool {
    #[cfg(test)]
    return false;
    #[cfg(not(test))]
    {
        let Some(base) = std::env::var_os("LOCALAPPDATA") else { return false; };
        std::path::Path::new(&base).join(r"Packages\Microsoft.YourPhone_8wekyb3d8bbwe").is_dir()
    }
}

fn card_path() -> Option<PathBuf> {
    // 官方 UI 测试依赖「无电池设备」的确定性布局；测试构建不读真实手机连接数据，
    // 否则本机配对手机的电量会污染 resident_power，让布局/吉祥物断言失效。
    #[cfg(test)]
    return None;
    #[cfg(not(test))]
    {
        let base = std::env::var_os("LOCALAPPDATA")?;
        Some(PathBuf::from(base).join(RELATIVE))
    }
}

/// 文件在 max_age 内更新过才算新鲜。
fn fresh(path: &Path, max_age: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified()).ok()
        .and_then(|time| time.elapsed().ok())
        .is_some_and(|age| age <= max_age)
}

/// 把 JSON 里的 \uXXXX 还原成真实字符（中文文案全靠这个才能匹配）。
/// 非法转义、代理项等原样保留，不影响后续判断。
fn decode_escapes(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'u') && i + 6 <= bytes.len() {
            // get() 而不是直接切片：碰到非 UTF-8 边界的坏数据时安全跳过。
            if let Some(hex) = text.get(i + 2..i + 6) {
                if let Ok(code) = u32::from_str_radix(hex, 16) {
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                        i += 6;
                        continue;
                    }
                }
            }
        }
        let ch = text[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// 卡片是否可用：明确的连接文案优先；没有连接文案时，
/// 必须还能读到电量数值、且文件没过期，才允许兜底显示。
fn card_usable(plain: &str, level: Option<u8>, fresh: bool) -> bool {
    match parse_linked(plain) {
        Some(linked) => linked,
        None => level.is_some() && fresh,
    }
}

/// 卡片 speak 形如 `Xiaomi 17 Pro Max - 已连接到你的电脑`（原文是 \u 转义），
/// 断开后变成 `Xiaomi 17 Pro Max - 已从电脑断开连接`，且电量数值一并消失。
fn parse_linked(text: &str) -> Option<bool> {
    let lower = text.to_ascii_lowercase();
    let disconnected = [
        "未连接", "断开连接", "已断开", "连接已断开", "已从电脑断开",
        "disconnected", "not connected", "no phone connected", "offline",
    ].iter().any(|key| lower.contains(key));
    // "disconnected" / "not connected" 已被上面捕获；这里再排除一次更稳妥。
    let connected = lower.contains("已连接")
        || (lower.contains("connected") && !lower.contains("not connected") && !lower.contains("disconnected"));
    if disconnected { Some(false) }
    else if connected { Some(true) }
    else { None }
}

fn next_charging(previous: Option<u8>, level: u8, current: bool) -> bool {
    match previous {
        Some(old) if level > old => true,
        Some(old) if level < old => false,
        _ => current,
    }
}

/// 卡片里电量写成 `"电池电量剩余 45%"` 或 JSON 转义的
/// `"\u7535\u6C60\u7535\u91CF\u5269\u4F59 45%"`，两种都要认。
/// 只有紧跟在电量关键字后面的百分比才算数，避免把布局里的百分数误当电量。
fn parse_level(text: &str) -> Option<u8> {
    let re = Regex::new(r"(\d{1,3})%").ok()?;
    for caps in re.captures_iter(text) {
        let whole = caps.get(0)?;
        let before = &text[whole.start().saturating_sub(80)..whole.start()];
        let battery = before.contains(r"\u7535\u6C60") || before.contains("电池")
            || before.contains(r"\u5269\u4F59") || before.contains("剩余")
            || before.contains(r"\u4E3A") || before.contains("电量");
        if !battery { continue; }
        if let Ok(value) = caps[1].parse::<u8>() {
            if value <= 100 { return Some(value); }
        }
    }
    None
}

fn parse_device(text: &str) -> Option<String> {
    let re = Regex::new(r#""text"\s*:\s*"([^"\\]{1,64})""#).ok()?;
    for caps in re.captures_iter(text) {
        let value = caps[1].to_owned();
        if !value.contains('%') && !value.trim().is_empty() { return Some(value); }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解析转义与明文卡片() {
        let escaped = r#"{"tooltip":"\u7535\u6C60\u7535\u91CF\u5269\u4F59 45%","altText":"\u5F53\u524D\u7535\u6C60\u7535\u91CF\u4E3A 45%"}"#;
        assert_eq!(parse_level(escaped), Some(45));
        assert_eq!(parse_level(r#"{"text":"49%","tooltip":"电池电量剩余 49%"}"#), Some(49));
    }

    #[test]
    fn 无关百分比不被误判() {
        assert_eq!(parse_level(r#"{"width":"100%","clipMargin":"2px"}"#), None);
        assert_eq!(parse_level(r#"{"tooltip":"电池电量剩余 101%"}"#), None);
    }

    #[test]
    fn 电量上升视为充电() {
        assert!(next_charging(Some(45), 49, false));
        assert!(!next_charging(Some(80), 79, true));
        assert!(next_charging(Some(80), 80, true));
    }

    #[test]
    fn 能取出设备名() {
        let text = r#"{"speak":"Xiaomi 17 Pro Max - connected","body":[{"text":"Xiaomi 17 Pro Max"},{"text":"49%"}]}"#;
        assert_eq!(parse_device(text).as_deref(), Some("Xiaomi 17 Pro Max"));
    }

    #[test]
    fn 精确充电状态覆盖值解析() {
        assert_eq!(parse_override("1"), Some(true));
        assert_eq!(parse_override(" charging\n"), Some(true));
        assert_eq!(parse_override("充电"), Some(true));
        assert_eq!(parse_override("0"), Some(false));
        assert_eq!(parse_override("未充电"), Some(false));
        assert_eq!(parse_override("auto"), None);
        assert_eq!(parse_override(""), None);
    }

    #[test]
    fn 连接状态优先判断() {
        // 转义文案先解码，再交给 parse_linked（poll 里就是这么做的）。
        let linked = decode_escapes(r#"{"speak":"Xiaomi - \u5DF2\u8FDE\u63A5\u5230\u4F60\u7684\u7535\u8111"}"#);
        let unlinked = decode_escapes(r#"{"speak":"Xiaomi - \u672A\u8FDE\u63A5"}"#);
        assert_eq!(parse_linked(&linked), Some(true));
        assert_eq!(parse_linked(&unlinked), Some(false));
        assert_eq!(parse_linked(r#"{"speak":"Phone - connected"}"#), Some(true));
        assert_eq!(parse_linked(r#"{"speak":"Phone - disconnected"}"#), Some(false));
        assert_eq!(parse_linked(r#"{"speak":"Phone - not connected"}"#), Some(false));
        assert_eq!(parse_linked(r#"{"text":"49%"}"#), None);
    }

    #[test]
    fn 解码中文转义文案() {
        // 真实断开卡片的 speak：\u5DF2\u4ECE\u7535\u8111\u65AD\u5F00\u8FDE\u63A5
        let raw = r#"{"speak":"Xiaomi 17 Pro Max - \u5DF2\u4ECE\u7535\u8111\u65AD\u5F00\u8FDE\u63A5"}"#;
        let plain = decode_escapes(raw);
        assert!(plain.contains("已从电脑断开连接"));
        assert_eq!(parse_linked(&plain), Some(false));
        // 断开卡片里没有电量数值
        assert_eq!(parse_level(&plain), None);
        assert!(!card_usable(&plain, None, true));
        // 即使残留旧百分比，明确断开也要收起
        assert!(!card_usable(&plain, Some(80), true));
    }

    #[test]
    fn 已连接卡片仍显示电量() {
        let plain = decode_escapes(r#"{"speak":"Xiaomi - \u5DF2\u8FDE\u63A5\u5230\u4F60\u7684\u7535\u8111","tooltip":"\u7535\u6C60\u7535\u91CF\u5269\u4F59 80%"}"#);
        assert_eq!(parse_linked(&plain), Some(true));
        assert_eq!(parse_level(&plain), Some(80));
        assert!(card_usable(&plain, Some(80), false));
    }

    #[test]
    fn 无连接文案时的新鲜度兜底() {
        // 手机息屏等场景：卡片没写连接状态但仍有电量，靠文件新鲜度兜底。
        assert!(card_usable(r#"{"tooltip":"电池电量剩余 49%"}"#, Some(49), true));
        assert!(!card_usable(r#"{"tooltip":"电池电量剩余 49%"}"#, Some(49), false));
        // 断开后的典型形态：既无连接文案也无电量数值，即便文件刚更新也不能沿用旧值。
        assert!(!card_usable(r#"{"text":"电池处于未知状态"}"#, None, true));
    }

    #[test]
    fn 运行中手机断开会清空已缓存电量() {
        // 用户遇到的场景：先连着手机（缓存 80%），再断开，卡片改为断开文案且不再写电量数值。
        let mut reader = Reader::default();
        let linked = decode_escapes(r#"{"speak":"Xiaomi - \u5DF2\u8FDE\u63A5\u5230\u4F60\u7684\u7535\u8111","text":"Xiaomi 17 Pro Max","tooltip":"\u7535\u6C60\u7535\u91CF\u5269\u4F59 80%"}"#);
        assert_eq!(reader.evaluate(&linked, true).0, Some(80));
        assert_eq!(reader.level, Some(80));

        let gone = decode_escapes(r#"{"speak":"Xiaomi 17 Pro Max - \u5DF2\u4ECE\u7535\u8111\u65AD\u5F00\u8FDE\u63A5","tooltip":"\u7535\u6C60\u5904\u4E8E\u672A\u77E5\u72B6\u6001"}"#);
        assert_eq!(reader.evaluate(&gone, true), (None, false));
        assert_eq!(reader.level, None);
        assert!(reader.device().is_empty());

        // 重新连上要能立刻恢复，不需要重启岛仔。
        assert_eq!(reader.evaluate(&linked, true).0, Some(80));
    }

    #[test]
    fn 连接正常但卡片暂无电量数值时不会误收起() {
        let mut reader = Reader::default();
        let linked = decode_escapes(r#"{"speak":"Xiaomi - \u5DF2\u8FDE\u63A5\u5230\u4F60\u7684\u7535\u8111","tooltip":"\u7535\u6C60\u7535\u91CF\u5269\u4F59 80%"}"#);
        assert_eq!(reader.evaluate(&linked, true).0, Some(80));
        let no_level = decode_escapes(r#"{"speak":"Xiaomi - \u5DF2\u8FDE\u63A5\u5230\u4F60\u7684\u7535\u8111"}"#);
        // 明确仍连接、只是这一帧没写电量：宽限期内沿用旧值，避免刷新闪烁。
        assert_eq!(reader.evaluate(&no_level, true).0, Some(80));
    }
}
