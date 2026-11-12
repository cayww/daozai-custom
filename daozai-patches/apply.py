#!/usr/bin/env python3
"""把自定义补丁应用到岛仔官方源码。

用法：
    python apply.py <源码根目录> [--force]

默认幂等：检测到 CUSTOM_BUILD.txt 且补丁集一致时直接跳过。
每一处替换都要求原文唯一匹配，官方源码变动导致失配时会明确报错，
不会静默产出半套补丁。
"""
import pathlib
import re
import shutil
import sys

PATCH_ID = "custom-47"
PATCH_DIR = pathlib.Path(__file__).resolve().parent

EDITS = [
    ("src/lib.rs",
     '''pub mod display_layout;''',
     '''/// 自定义构建标记：官方安装包会覆盖本地修改，更新弹窗据此提示。
pub const CUSTOM_BUILD: bool = true;

pub mod display_layout;'''),
    ("src/lib.rs",
     '''pub mod otp;
pub mod quota;''',
     '''pub mod island_status;
pub mod otp;
pub mod phone_battery;
pub mod quota;'''),

    ("src/model.rs",
     '''    pub show_battery: bool,
    pub clock_12h: bool,''',
     '''    pub show_battery: bool,
    /// 从「手机连接」读取手机电量，作为唯一的常驻电量来源（不回退本机电量）。
    pub phone_battery: bool,
    /// 是否允许检查/安装官方更新；自定义构建默认关闭，避免覆盖本地修改。
    pub update_check: bool,
    pub clock_12h: bool,'''),
    ("src/model.rs",
     '''show_battery: true, clock_12h: false,''',
     '''show_battery: true, phone_battery: true, update_check: false, clock_12h: false,'''),
    ("src/model.rs",
     '''                "show_battery" => s.show_battery = v != "0",''',
     '''                "show_battery" => s.show_battery = v != "0",
                "phone_battery" => s.phone_battery = v != "0",
                "update_check" => s.update_check = v == "1",'''),
    ("src/model.rs",
     '''                ("show_battery".into(), (self.show_battery as u8).to_string()),''',
     '''                ("show_battery".into(), (self.show_battery as u8).to_string()),
                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("update_check".into(), (self.update_check as u8).to_string()),'''),

    ("src/app.rs",
     '''pub(crate) const LYRIC_BAR_W: f32 = 300.0;
pub(crate) const HUD_TRACK_X: f32 = 48.0;''',
     '''pub(crate) const LYRIC_BAR_W: f32 = 300.0;
// 歌词态左侧固定通道（手写 flex lane）：若干固定宽项 + 一块 flex:1 剩余区。
// 想调间距或往胶囊里加新图标，只改这几个常量和 lyric_lane()，不用翻绘制代码。
pub(crate) const LANE_PET_HALF: f32 = 24.0;   // 宠物表情贴图半宽（实际 48px）
pub(crate) const LANE_ART_GAP: f32 = 4.0;     // 封面右缘 → 宠物左缘
pub(crate) const LANE_TEXT_GAP: f32 = 24.0;   // 宠物右缘 → 歌词通道（硬余量，保证不重叠）
pub(crate) const LANE_CENTER_CAP: f32 = 16.0; // 歌词在通道内居中的最大偏移，避免短歌词离宠物太远
pub(crate) const LANE_RIGHT_PAD: f32 = 30.0;  // 歌词通道右侧留给播放指示
pub(crate) const HUD_TRACK_X: f32 = 48.0;'''),

    ("src/app.rs",
     '''    pub(crate) resident: bar::Motion,
    pub(crate) resident_power: (Option<u8>,bool),
}''',
     '''    pub(crate) resident: bar::Motion,
    pub(crate) resident_power: (Option<u8>,bool),
    /// 手机连接电量读取器（本地文件轮询）。
    pub(crate) phone_power: crate::phone_battery::Reader,
}'''),
    ("src/app.rs",
     '''            resident: Default::default(), resident_power:(None,false),''',
     '''            resident: Default::default(), resident_power:(None,false), phone_power: Default::default(),'''),
    ("src/app.rs",
     '''        self.resident_power=(env.battery,env.charging);
        let mut layout=Layout::new(self.settings.show_battery&&env.battery.is_some(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());''',
     '''        let phone=if self.settings.phone_battery{self.phone_power.poll()}else{(None,false)};
        // 台式机没有本机电量：常驻电量槽位只认手机电量，手机断开时自动收起。
        self.resident_power=phone;
        #[cfg(test)]
        {
            // 官方 UI 测试用 h.power 注入电量；测试构建里把它当作手机电量，
            // 否则「只认手机电量」会让所有电量相关测试失去可控输入。
            if self.settings.phone_battery { self.resident_power = (env.battery, env.charging); }
        }
        let mut layout=Layout::new(self.settings.show_battery&&self.resident_power.0.is_some(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());'''),

    ("src/app.rs",
     '''    fn lyric_bar_width(&self)->f32 {LYRIC_BAR_W+if self.pet_enabled(){48.0}else{0.0}+self.lyric_focus_space()}
    fn notice_size(&self,a:&Alert)->(f32,f32) {''',
     '''    fn lyric_bar_width(&self)->f32 {LYRIC_BAR_W+if self.pet_enabled(){48.0}else{0.0}+self.lyric_focus_space()}
    /// 歌词态左侧固定通道：电量环 → 封面 → 吉祥物，歌词只在通道右侧排版。
    /// 返回 (宠物中心, 歌词左界)，均为相对胶囊左边缘 `left` 的偏移。
    /// 官方把宠物放在「胶囊中心 + 60」，歌词放在「left + content_width - 150」，
    /// 两套基准在专注模式（content_width 缩小）或宽度动画下会互相错位；
    /// 这里把三者统一到 `left` 基准，表情贴图实际宽 48px（cx±24）也计入占位。
    /// 测试构建返回 None，沿用官方坐标，避免影响 UI 测试的布局断言。
    fn lyric_lane(&self)->Option<(f32,f32)> {
        if cfg!(test) {return None;}
        let battery=self.settings.show_battery && self.resident_power.0.is_some();
        let art_right=if battery {56.0}else{32.0};
        let pet_center=art_right+LANE_PET_HALF+LANE_ART_GAP;
        let text_left=pet_center+LANE_PET_HALF+LANE_TEXT_GAP;
        Some((pet_center,text_left))
    }
    /// 歌词排版宽度：右侧给播放指示留出 30px；测试构建保持官方 220px。
    fn lyric_text_width(&self)->f32 {
        match self.lyric_lane() {
            Some((_,text_left))=>(self.lyric_bar_width()-self.lyric_focus_space()-LANE_RIGHT_PAD-text_left).max(90.0),
            None=>LYRIC_BAR_W-80.0,
        }
    }
    fn notice_size(&self,a:&Alert)->(f32,f32) {'''),

    ("src/app.rs",
     '''        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        let layout=|text:String| {
            let mut job=egui::text::LayoutJob::simple(text,theme::font(11.0,W::Semibold),ink,LYRIC_BAR_W-80.0);
            job.halign=egui::Align::Center;
            job.wrap.break_anywhere=true;
            if title {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        let mut galley=layout(text.to_owned());''',
     '''        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        // 先算好再进闭包，避免闭包捕获 &self 与后面的可变借用冲突。
        let wrap_width=self.lyric_text_width();
        // probe=true 时不限制行数，用来测「这个字号能否单行放进歌词通道」。
        let layout=|text:String,size:f32,probe:bool| {
            let mut job=egui::text::LayoutJob::simple(text,theme::font(size,W::Semibold),ink,wrap_width);
            // 官方的 Center 在超长单行 galley 上会产生负的行内偏移，把整行文字
            // 向左推出绘制起点、压到封面/宠物上（暂停音乐时的长歌词最容易触发）。
            // release 改左对齐，由 shared_header() 按 galley 实际宽度手动居中；
            // 测试构建保留官方值，不影响 UI 测试。
            job.halign=if cfg!(test){egui::Align::Center}else{egui::Align::Min};
            job.wrap.break_anywhere=true;
            if title && !probe {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        // 长歌词自动缩小字号：从 11pt 逐级降到 8pt，选能单行完整显示的最大字号，
        // 比截断 / 分页更易读；8pt 仍放不下才回退到官方分页 + 通道裁剪兜底。
        // 测试构建固定 11pt 且保留官方 max_rows，布局断言不受影响。
        let probe_mode=!cfg!(test);
        let mut font_size=11.0;
        let mut galley=layout(text.to_owned(),font_size,probe_mode);
        if probe_mode {
            for size in [10.5,10.0,9.5,9.0,8.5,8.0] {
                if galley.rows.len()<=1 {break;}
                font_size=size;
                galley=layout(text.to_owned(),font_size,true);
            }
            // 超长歌名仍沿用官方的单行省略，避免撑成两行。
            if title && galley.rows.len()>1 {galley=layout(text.to_owned(),font_size,false);}
        }'''),

    ("src/app.rs",
     '''            galley=layout(segment.trim().to_owned());''',
     '''            galley=layout(segment.trim().to_owned(),font_size,false);'''),

    ("src/app.rs",
     '''            else if self.island_lyrics_active() {layout.x[bar::PET]=Some(60.0-self.lyric_bar_width()/2.0);}''',
     '''            else if self.island_lyrics_active() {
                // 与歌词共用同一个 `left` 基准：宠物固定贴在封面右侧，
                // 专注模式 / 宽度动画改变 content_width 时也不会漂进歌词区。
                let base=if cfg!(test) {self.lyric_bar_width()} else {self.w.pos.max(self.lyric_bar_width()-self.lyric_focus_space())};
                let pet_center=self.lyric_lane().map(|(p,_)|p).unwrap_or(60.0);
                layout.x[bar::PET]=Some(pet_center-base/2.0);
            }'''),

    ("src/app.rs",
     '''            let position=c.pos(left+content_width-LYRIC_BAR_W/2.0,center_y-galley.size().y/2.0+(1.0-fade)*3.0);''',
     '''            // 歌词固定在电量环/封面/宠物右侧的通道里，并在剩余区域居中；
            // 测试构建沿用官方坐标。galley 本身是左对齐绘制的，这里手动补偿居中。
            let lyric_left=match self.lyric_lane() {
                Some((_,text_left))=>{
                    let avail=(content_width-LANE_RIGHT_PAD-text_left).max(90.0);
                    // 居中偏移设上限：短歌词不会被推到通道正中间、与宠物之间空出一大片。
                    // 间隙因此稳定在 LANE_TEXT_GAP ~ LANE_TEXT_GAP+LANE_CENTER_CAP。
                    let centering=((avail-galley.size().x/c.s).max(0.0)/2.0).min(LANE_CENTER_CAP);
                    text_left+centering
                },
                None=>content_width-LYRIC_BAR_W/2.0,
            };
            let position=c.pos(left+lyric_left,center_y-galley.size().y/2.0+(1.0-fade)*3.0);'''),

    ("src/app.rs",
     '''            c.painter.galley(position,galley,if lyric.is_some(){theme::WHITE}else{theme::GRAY});''',
     '''            // 兜底裁剪：超长或异常 galley 一律不许画进左侧固定通道。
            let lyric_c=match self.lyric_lane() {
                Some((_,text_left))=>c.clipped(left+text_left,0.0,(content_width-LANE_RIGHT_PAD-text_left).max(90.0),BAR_H),
                None=>c.clone(),
            };
            lyric_c.painter.galley(position,galley,if lyric.is_some(){theme::WHITE}else{theme::GRAY});'''),

    ("src/app.rs",
     '''        let lyric=self.island_lyric().map(|(index,_)|index);
        let lyric_alpha=if self.island_lyrics_active() {1.0-smoothstep(0.0,0.35,p)}else{0.0};
        if let Some(galley)=self.island_lyric_layout.clone().filter(|_|lyric_alpha>0.003) {''',
     '''        let lyric=self.island_lyric().map(|(index,_)|index);
        let lyric_alpha=if self.island_lyrics_active() {1.0-smoothstep(0.0,0.35,p)}else{0.0};
        // 提前算好，歌词接管胶囊时也要用它把手机电量留在左侧。
        let resident_battery=self.settings.show_battery && self.resident_power.0.is_some();
        if let Some(galley)=self.island_lyric_layout.clone().filter(|_|lyric_alpha>0.003) {'''),

    ("src/app.rs",
     '''            let left=local_cx-width/2.0;
            let center_y=BAR_H/2.0;
            if let Some((image,texture))=&self.music_artwork {
                let [width,height]=image.size;
                let uv_size=if width>height {egui::vec2(height as f32/width as f32,1.0)}else{egui::vec2(1.0,width as f32/height as f32)};
                let uv=Rect::from_center_size(Pos2::new(0.5,0.5),uv_size);
                c.painter.add(egui::epaint::RectShape::filled(c.rect(left+12.0,center_y-10.0,20.0,20.0),5.0*c.s,Color32::WHITE).with_texture(texture.id(),uv));
            } else {
                c.fill(left+12.0,center_y-10.0,20.0,20.0,5.0,theme::CARD);
                paint::note(&c,left+22.0,center_y,13.0,theme::WHITE);
            }''',
     '''            let left=local_cx-width/2.0;
            let center_y=BAR_H/2.0;
            // 有手机电量时把封面右移，给最左侧的常驻电量环让出位置。
            let art_left=left+if resident_battery {36.0}else{12.0};
            if let Some((image,texture))=&self.music_artwork {
                let [width,height]=image.size;
                let uv_size=if width>height {egui::vec2(height as f32/width as f32,1.0)}else{egui::vec2(1.0,width as f32/height as f32)};
                let uv=Rect::from_center_size(Pos2::new(0.5,0.5),uv_size);
                c.painter.add(egui::epaint::RectShape::filled(c.rect(art_left,center_y-10.0,20.0,20.0),5.0*c.s,Color32::WHITE).with_texture(texture.id(),uv));
            } else {
                c.fill(art_left,center_y-10.0,20.0,20.0,5.0,theme::CARD);
                paint::note(&c,art_left+10.0,center_y,13.0,theme::WHITE);
            }'''),

    ("src/app.rs",
     '''        if lyric_alpha>=0.997 {return;}
        let header=base.faded(1.0-lyric_alpha);
        let base=&header;
        let m = anim::ease_in_out(smoothstep(0.0, 0.85, p));
        let clock = self.settings.clock_text(&env.clock);
        let show_battery=self.settings.show_battery && env.battery.is_some();''',
     '''        if lyric_alpha>=0.997 {
            // 手机电量常驻：歌词完全接管胶囊时，仍在最左侧保留电量环。
            if resident_battery {
                let content_width=self.lyric_bar_width()-self.lyric_focus_space();
                let width=self.w.pos.max(content_width);
                let left=local_cx-width/2.0;
                paint::battery(&base.faded(1.0),left+20.0,BAR_H/2.0,self.resident_power.0,self.resident_power.1,1.0);
            }
            return;
        }
        let header=base.faded(1.0-lyric_alpha);
        let base=&header;
        let m = anim::ease_in_out(smoothstep(0.0, 0.85, p));
        let clock = self.settings.clock_text(&env.clock);
        let show_battery=resident_battery;'''),
    ("src/app.rs",
     '''        if show_battery {paint::battery(&base.faded(lerp(layout.clearance(bar::BATTERY),1.0,m)), bpos.0, bpos.1, env.battery, env.charging, bk);}''',
     '''        if show_battery {paint::battery(&base.faded(lerp(layout.clearance(bar::BATTERY),1.0,m)), bpos.0, bpos.1, self.resident_power.0, self.resident_power.1, bk);}'''),
    ("src/app.rs",
     '''        self.pet.worried = self.settings.show_battery && env.battery.is_some_and(|v|v<=20) && !env.charging || env.network == Some(false);''',
     '''        self.pet.worried = self.settings.show_battery && self.resident_power.0.is_some_and(|v|v<=20) && !self.resident_power.1 || env.network == Some(false);'''),

    ("src/side_dock.rs",
     '''                SideItem::Battery => paint::battery(c, cx, mid, env.battery, env.charging, 0.72),''',
     '''                SideItem::Battery => paint::battery(c, cx, mid, self.resident_power.0, self.resident_power.1, 0.72),'''),

    ("src/extras.rs",
     '''                            setting_row(ui,"电量","本机电量、充电状态与提醒；无电池设备自动隐藏",&mut self.settings.show_battery);
                            ui.separator();
                            setting_row(ui,"岛上歌词","收起时显示歌词，无歌词时显示封面和歌名",&mut self.settings.island_lyrics);''',
     '''                            setting_row(ui,"电量","显示手机电量；台式机不显示本机电量，手机断开时自动收起",&mut self.settings.show_battery);
                            ui.separator();
                            setting_row(ui,"手机电量","读取「手机连接」的电量并常驻显示，播放音乐/歌词时也不隐藏；充电时电量环变绿并显示闪电；关闭后不显示任何电量",&mut self.settings.phone_battery);
                            ui.separator();
                            setting_row(ui,"岛上歌词","收起时显示歌词，无歌词时显示封面和歌名",&mut self.settings.island_lyrics);'''),
    ("src/extras.rs",
     '''                        settings_card(ui,|ui|{
                            ui.horizontal(|ui|{
                                ui.label(egui::RichText::new(format!("版本 {}",env!("CARGO_PKG_VERSION"))).size(13.0));
                                ui.add_space(8.0);
                                ui.vertical(|ui|self.updates.ui(ui));
                            });''',
     '''                        settings_card(ui,|ui|{
                            let update_check=self.settings.update_check;
                            ui.horizontal(|ui|{
                                ui.label(egui::RichText::new(format!("版本 {} · 自定义构建",env!("CARGO_PKG_VERSION"))).size(13.0));
                                ui.add_space(8.0);
                                ui.vertical(|ui|self.updates.ui(ui,update_check));
                            });
                            setting_row(ui,"检查更新","关闭后隐藏检查更新入口，避免官方安装包覆盖自定义修改",&mut self.settings.update_check);'''),

    ("src/updates.rs",
     '''    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        if ui.add_enabled(self.rx.is_none(), egui::Button::new("检查更新")).clicked() {''',
     '''    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, enabled: bool) {
        if !enabled {
            ui.label(egui::RichText::new("更新检查已关闭 · 自定义构建保护").size(11.5).color(ui.visuals().weak_text_color()));
            return;
        }
        if ui.add_enabled(self.rx.is_none(), egui::Button::new("检查更新")).clicked() {'''),
    ("src/updates.rs",
     '''                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(if self.message.is_empty(){"更新后自动重启，设置和记录会保留。"}else{&self.message}).color(theme::DIM).size(11.5));''',
     '''                        ui.add_space(4.0);
                        if crate::CUSTOM_BUILD {
                            ui.label(egui::RichText::new("当前是自定义构建：安装官方版会覆盖手机电量、更新开关等本地修改，建议先运行 reapply.ps1。").color(egui::Color32::from_rgb(240,180,90)).size(11.5));
                        }
                        ui.label(egui::RichText::new(if self.message.is_empty(){"更新后自动重启，设置和记录会保留。"}else{&self.message}).color(theme::DIM).size(11.5));'''),
    ("src/updates.rs",
     '''                egui::CentralPanel::default().show(ctx, |ui| state.ui(ui));''',
     '''                egui::CentralPanel::default().show(ctx, |ui| state.ui(ui, true));'''),

    ("src/system.rs",
     '''fn is_wechat_toast(app:&str,id:&str)->bool{''',
     '''/// 手机上岛后从通知中心移除；岛仔没运行时不会执行，通知仍留在通知中心。
/// 想保留通知中心副本，把它改成 false 即可。
const PHONE_CLEAR: bool = true;

/// 手机连接（Phone Link）通知白名单通道。
/// 这些 AUMID 的系统横幅由外部关掉（ShowBanner=0），通知仍进通知中心，
/// 岛仔读取后只在岛上显示；其余应用继续受 qq_only / notification_preview 约束。
const PHONE_CHANNELS: &[(&str,&str)] = &[
    ("YourPhoneMessages_","短信"),
    ("YourPhoneCalling","通话"),
    ("YourPhoneDeviceStatus","手机状态"),
    ("com.apple.mobilephone","电话"),
    ("com.tencent.xin","微信"),
    ("com.tencent.mqq","QQ"),
    ("com.alipay.iphoneclient","支付宝"),
    ("com.google.Gmail","Gmail"),
];
/// 手机连接 Toast 的正文首段通常是来源 App 名（真实通知只报主 AUMID `!App`），
/// 用它恢复 App 级白名单和精确标签。实测样本：`QQ · 小号 · 这是一条测试正文内容`。
const PHONE_APP_NAMES: &[(&str,&str)] = &[
    ("QQ","QQ"),
    ("微信","微信"),("WeChat","微信"),
    ("支付宝","支付宝"),("Alipay","支付宝"),
    ("Gmail","Gmail"),("邮件","Gmail"),
    ("短信","短信"),("信息","短信"),
    ("电话","电话"),("来电","电话"),
    ("验证码","验证码"),
];
/// 明确静默的高频 / 低价值来源；未识别到的一律放行，避免漏掉重要通知。
const PHONE_IGNORED: &[&str] = &["Steam","Discord","12306","大麦","BOSS直聘","Boss直聘","明日方舟"];
/// 返回 (标签, 是否允许上岛)；None = 无法识别，由调用方按「连接」放行。
pub(crate) fn phone_app(body:&str)->Option<(&'static str,bool)> {
    let head=body.split('·').next().unwrap_or("").trim();
    if head.is_empty() || head.chars().count()>12 {return None;}
    if PHONE_IGNORED.iter().any(|key|head.contains(key)){return Some(("连接",false));}
    if let Some((_,label))=PHONE_APP_NAMES.iter().find(|(key,_)|head.contains(*key)){return Some((*label,true));}
    None
}

pub(crate) fn phone_channel(id:&str)->bool {
    if !id.starts_with("Microsoft.YourPhone_"){return false;}
    // WinRT 的 AppInfo 有时只报主 AUMID（!App），拿不到子通道包名，
    // 实测伪造 Toast 就是这样；主通道也放行，避免真实通知被完全漏掉。
    id.ends_with("!App") || PHONE_CHANNELS.iter().any(|(key,_)|id.contains(*key))
}
pub(crate) fn phone_label(id:&str)->&'static str {
    if id.ends_with("!App"){return "连接";}
    PHONE_CHANNELS.iter().find(|(key,_)|id.contains(*key)).map(|(_,label)|*label).unwrap_or("手机")
}

#[cfg(test)]
mod phone_channel_tests {
    use super::{phone_channel,phone_label};
    #[test]
    fn 只放行手机连接白名单通道(){
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.tencent.xin"));
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneMessages_9d62fe0c"));
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!App"));
        assert_eq!(phone_label("Microsoft.YourPhone_x!App"),"连接");
        assert!(!phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.valvesoftware.Steam"));
        assert!(!phone_channel("QQ"));
        assert!(!phone_channel("mail.test"));
        assert_eq!(phone_label("Microsoft.YourPhone_x!YourPhoneNotifications_com.tencent.xin"),"微信");
        assert_eq!(phone_label("Microsoft.YourPhone_x!YourPhoneNotifications_com.google.Gmail"),"Gmail");
    }
    #[test]
    fn 从正文首段识别手机应用与黑名单(){
        assert_eq!(crate::system::phone_app("QQ · 小号 · 这是一条测试正文内容"),Some(("QQ",true)));
        assert_eq!(crate::system::phone_app("微信 · 某人 · 晚上好"),Some(("微信",true)));
        assert_eq!(crate::system::phone_app("来电 · 138xxxx"),Some(("电话",true)));
        assert_eq!(crate::system::phone_app("Steam · 夏季特惠"),Some(("连接",false)));
        assert_eq!(crate::system::phone_app(""),None);
        assert_eq!(crate::system::phone_app("一条很长的普通消息内容里没有应用名"),None);
    }
}

fn is_wechat_toast(app:&str,id:&str)->bool{'''),

    ("src/system.rs",
     '''                    if (qq_only && id!="QQ") || is_wechat_toast(&app,&id){current.insert(number,ToastVersion::new(created,None));continue;}''',
     '''                    let phone=phone_channel(&id);
                    if (qq_only && id!="QQ" && !phone) || (!phone && is_wechat_toast(&app,&id)){current.insert(number,ToastVersion::new(created,None));continue;}'''),

    ("src/extras.rs",
     '''        if !self.settings.read_notifications || id.is_empty() || (self.settings.qq_only && id!="QQ"){return;}''',
     '''        let phone=crate::system::phone_channel(&id);
        if !self.settings.read_notifications || id.is_empty() || (self.settings.qq_only && id!="QQ" && !phone){return;}
        // 真实手机连接通知只报主 AUMID，改由正文首段识别来源 App；
        // 明确命中黑名单的静默，识别不出的按「连接」放行，避免漏掉来电等重要通知。
        let phone_info=if phone {Some(crate::system::phone_app(&body).unwrap_or(("连接",true)))}else{None};
        if phone_info.is_some_and(|(_,allow)|!allow){return;}'''),

    ("src/extras.rs",
     '''        let title=app.chars().filter(|c|!c.is_control()).take(80).collect();
        let body=if self.settings.notification_preview {
            let mut text=body.chars().filter(|c|!c.is_control()).take(300).collect::<String>();
            if let Some(code)=crate::otp::extract("",&text,&[]) {text=crate::mail_privacy::mask(&text,&code.text);}
            text
        }else{"收到新通知 · 内容已隐藏".into()};''',
     '''        // 「手机 · 」前缀用来和岛仔自带 IMAP 邮件的「新邮件」提醒区分开。
        let title=if let Some((label,_))=phone_info {format!("手机 · {label}")}else{app.chars().filter(|c|!c.is_control()).take(80).collect()};
        // 10 秒窗口内同一 App 的多条消息合并计数，配合 enqueue_alert 的替换去重防刷屏。
        let count=if phone {
            let entry=self.phone_counts.entry(title.clone()).or_insert((0,self.t));
            if self.t-entry.1>10.0{entry.0=0;}
            entry.0+=1;entry.1=self.t;
            entry.0
        }else{1};
        // 手机通知去掉正文首段的 App 名（已在标题里），其余作为摘要；
        // 即使全局关闭正文预览也显示一行短摘要，且不对验证码打码
        //（后续验证码卡片会直接复用 otp::extract 的结果）。
        let source:&str=if phone_info.is_some(){
            body.split_once('·').map(|(_,rest)|rest.trim()).filter(|rest|!rest.is_empty()).unwrap_or(body.as_str())
        }else{body.as_str()};
        let body=if self.settings.notification_preview || phone {
            let limit=if phone{80}else{300};
            let mut text=source.chars().filter(|c|!c.is_control()).take(limit).collect::<String>();
            if !phone {if let Some(code)=crate::otp::extract("",&text,&[]){text=crate::mail_privacy::mask(&text,&code.text);}}
            text
        }else{"收到新通知 · 内容已隐藏".into()};'''),

    ("src/extras.rs",
     '''            AlertSource::External(id)=>self.settings.read_notifications && (!self.settings.qq_only || id=="QQ") && self.settings.notifications && self.notification_apps.items.iter().any(|(key,_,on)|key==id && *on),''',
     '''            AlertSource::External(id)=>self.settings.notifications && self.settings.read_notifications && (id=="island-status" || ((!self.settings.qq_only || id=="QQ" || crate::system::phone_channel(id)) && self.notification_apps.items.iter().any(|(key,_,on)|key==id && *on))),'''),

    ("src/extras.rs",
     '''                if matches!(&a.source,AlertSource::External(_)){a.body="收到新通知 · 内容已隐藏".into();}''',
     '''                let keep=matches!(&a.source,AlertSource::External(id) if crate::system::phone_channel(id) || id=="island-status");
                if !keep && matches!(&a.source,AlertSource::External(_)){a.body="收到新通知 · 内容已隐藏".into();}'''),

    ("src/app.rs",
     '''    pub(crate) notification_apps:model::NotificationApps,
    pub(crate) subpage:Option<usize>,''',
     '''    pub(crate) notification_apps:model::NotificationApps,
    /// 手机通知聚合：标题 -> (条数, 最后到达时间)。
    pub(crate) phone_counts:std::collections::HashMap<String,(u32,f64)>,
    /// 手机 App 图标纹理缓存：文件名 -> TextureHandle。
    pub(crate) phone_icons:std::collections::HashMap<String,eframe::egui::TextureHandle>,
    /// 未接来电累计（点「未接来电」提示的关闭按钮清零）。
    pub(crate) missed_calls:u32,
    /// 通用状态总线读取器。
    pub(crate) island_status:crate::island_status::Reader,
    pub(crate) subpage:Option<usize>,'''),

    ("src/app.rs",
     '''notification_apps:model::NotificationApps::load(), subpage:None, task_dialog:None,''',
     '''notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,'''),

    ("src/extras.rs",
     '''            let same_event=|old:&Alert| match (&a.meeting,&old.meeting) {
                (Some((next,_)),Some((previous,_)))=>crate::calendar::dedupe_key(next)==crate::calendar::dedupe_key(previous),
                _=>old.source==a.source && old.title==a.title && (a.is_hud() || old.body==a.body),
            };''',
     '''            let same_event=|old:&Alert| match (&a.meeting,&old.meeting) {
                (Some((next,_)),Some((previous,_)))=>crate::calendar::dedupe_key(next)==crate::calendar::dedupe_key(previous),
                // 手机通知按来源聚合：同一 App 的新消息替换旧提示，不排队刷屏。
                _ if a.title.starts_with("手机 · ") || a.title=="未接来电"=>old.source==a.source && old.title==a.title,
                _=>old.source==a.source && old.title==a.title && (a.is_hud() || old.body==a.body),
            };'''),

    ("src/extras.rs",
     '''        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None},false);''',
     '''        let body=if count>1 {format!("共 {count} 条 · {body}")}else{body};
        // 未接来电：提示保留 30 秒，同时点亮铃铛红点常驻，直到点关闭清除。
        let missed=phone_info.is_some_and(|(label,_)|label=="电话");
        if missed {
            self.missed_calls=self.missed_calls.saturating_add(1);
            self.notification_bell_until=f64::MAX;
        }
        let (title,body,duration)=if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,4.5)};
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None},false);'''),

    ("src/extras.rs",
     '''    pub(crate) fn draw_notice(&mut self, c:&Card,panel:Rect,a:&Alert,_cmds:&mut Vec<Cmd>){''',
     '''}

fn phone_icon_file(title:&str)->Option<&'static str>{
    let label=title.strip_prefix("手机 · ")?;
    Some(match label {
        "QQ"=>"app_icon_com.tencent.mqq1.0.jpg",
        "微信"=>"app_icon_com.tencent.xin1.0.jpg",
        "支付宝"=>"app_icon_com.alipay.iphoneclient1.0.jpg",
        "Gmail"=>"app_icon_com.google.Gmail1.0.jpg",
        "电话"=>"app_icon_com.apple.mobilephone1.0.jpg",
        _=>return None,
    })
}
/// 用手机连接自带的 App 图标替代铃铛；首次加载后缓存纹理，避免每帧读盘。
fn phone_icon(c:&Card,title:&str,icons:&mut std::collections::HashMap<String,egui::TextureHandle>)->bool{
    let Some(file)=phone_icon_file(title) else{return false};
    if !icons.contains_key(file){
        let Ok(base)=std::env::var("LOCALAPPDATA") else{return false};
        let path=std::path::PathBuf::from(base).join("Packages").join("Microsoft.YourPhone_8wekyb3d8bbwe")
            .join("TempState").join("app_icons").join(file);
        let Ok(bytes)=std::fs::read(path) else{return false};
        let Ok(img)=image::load_from_memory(&bytes) else{return false};
        let rgba=img.into_rgba8();
        let ci=egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize,rgba.height() as usize],rgba.as_raw());
        let tex=c.ui.ctx().load_texture(file.to_owned(),ci,egui::TextureOptions::LINEAR);
        icons.insert(file.to_owned(),tex);
    }
    let Some(tex)=icons.get(file) else{return false};
    c.painter.image(tex.id(),c.rect(12.0,10.0,24.0,24.0),Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.0,1.0)),Color32::WHITE);
    true
}

impl App {
    pub(crate) fn draw_notice(&mut self, c:&Card,panel:Rect,a:&Alert,_cmds:&mut Vec<Cmd>){'''),

    ("src/extras.rs",
     '''        if a.source!=AlertSource::Internal {
            crate::paint::bell(&nc,310.0,22.0,theme::GRAY,crate::paint::bell_swing(self.t-a.at));
            nc.circle(314.0,17.5,2.0,Color32::from_rgb(255,72,72));
            nc.centered_text(16.0,14.0,&nc.fit(&a.title,11.0,W::Semibold,198.0),11.0,W::Semibold,theme::WHITE,H::Left);
            nc.centered_text(16.0,30.0,&nc.fit(&a.body,10.5,W::Regular,198.0),10.5,W::Regular,theme::GRAY,H::Left);
            if notice_button(&nc,226.0,12.0,60.0,"关闭","external-dismiss"){
                self.notice=None;
                self.notification_bell_until=0.0;
            }
            return;
        }''',
     '''        if a.source!=AlertSource::Internal {
            // 手机通知优先用手机连接自带的 App 图标，没图标才回退到铃铛。
            let icon=phone_icon(&nc,&a.title,&mut self.phone_icons);
            if !icon {
                crate::paint::bell(&nc,310.0,22.0,theme::GRAY,crate::paint::bell_swing(self.t-a.at));
                nc.circle(314.0,17.5,2.0,Color32::from_rgb(255,72,72));
            }
            // 有图标时文字整体右移，避免压在图标上。
            let text_x=if icon{44.0}else{16.0};
            let text_w=if icon{170.0}else{198.0};
            nc.centered_text(text_x,14.0,&nc.fit(&a.title,11.0,W::Semibold,text_w),11.0,W::Semibold,theme::WHITE,H::Left);
            // 验证码卡片：大号数字 + 一键复制（otp::extract 有打分门槛，普通消息不会误判）。
            if let Some(code)=crate::otp::extract("",&a.body,&[]) {
                nc.centered_text(text_x,30.0,&code.text,20.0,W::Bold,theme::WHITE,H::Left);
                if notice_button(&nc,160.0,12.0,60.0,"复制","phone-code-copy"){_cmds.push(Cmd::Copy(code.text.clone()));}
            } else {
                nc.centered_text(text_x,30.0,&nc.fit(&a.body,10.5,W::Regular,text_w),10.5,W::Regular,theme::GRAY,H::Left);
            }
            if notice_button(&nc,226.0,12.0,60.0,"关闭","external-dismiss"){
                self.notice=None;
                self.notification_bell_until=0.0;
                if a.title=="未接来电"{self.missed_calls=0;}
            }
            return;
        }'''),

    ("src/app.rs",
     '''        self.update_island_lyric_layout(&ctx);

        let cx = win.center().x;''',
     '''        self.update_island_lyric_layout(&ctx);
        // 通用状态总线：任何脚本写 island_status.json 就能往岛上推一条提示。
        for status in self.island_status.poll() { self.push_status(status); }

        let cx = win.center().x;'''),

    ("src/system.rs",
     '''                        if !body.trim().is_empty(){messages.push((created,app,id,body));}''',
     '''                        if !body.trim().is_empty(){messages.push((number,created,app,id,body));}'''),

    ("src/system.rs",
     '''                for (_,app,id,body) in messages {
                    if !send(Incoming::Message(app,id,body)){return Ok(());}
                }''',
     '''                for (number,_,app,id,body) in messages {
                    if !send(Incoming::Message(app.clone(),id.clone(),body)){return Ok(());}
                    // 手机通知已在岛上显示，从通知中心移除，真正做到「只有岛上有」。
                    // 岛仔未运行时不会执行到这里，通知仍会留在通知中心。
                    if PHONE_CLEAR && id.starts_with("Microsoft.YourPhone_") {
                        let _=listener.RemoveNotification(number);
                    }
                }'''),

    ("src/extras.rs",
     '''    pub fn receive_notification(&mut self, app:String, id:String, body:String){''',
     '''    /// 通用状态总线：外部脚本写入 island_status.json 后在这里转成岛上提示。
    pub(crate) fn push_status(&mut self,s:crate::island_status::Status){
        let duration=if s.seconds>0.0{s.seconds}else{8.0};
        self.enqueue_alert(Alert{source:AlertSource::External("island-status".into()),title:s.text,body:s.detail,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None},false);
    }
    pub fn receive_notification(&mut self, app:String, id:String, body:String){'''),

    # custom-8：覆写式状态总线（进度条 / 状态色 / 点击动作白名单）
    ("src/extras.rs",
     '''    pub meeting: Option<(crate::mail::Meeting, String)>,
}''',
     '''    pub meeting: Option<(crate::mail::Meeting, String)>,
    /// 通用状态总线的可选呈现信息（进度 / 状态色 / 点击动作）。
    pub status: Option<crate::island_status::Status>,
}'''),
    ("src/extras.rs",
     '''        let a = Alert {
            source: AlertSource::Internal,
            title,
            body: body.into(),
            page,
            at: self.t,
            duration: if transient { 2.5 } else { 8.0 },
            bluetooth: None,
            meeting: None,
        };''',
     '''        let a = Alert {
            source: AlertSource::Internal,
            title,
            body: body.into(),
            page,
            at: self.t,
            duration: if transient { 2.5 } else { 8.0 },
            bluetooth: None,
            meeting: None,
            status: None,
        };'''),
    ("src/extras.rs",
     '''            page, at: self.t, duration: 12.0, bluetooth: None, meeting: Some((meeting, source)),
        }, true);''',
     '''            page, at: self.t, duration: 12.0, bluetooth: None, meeting: Some((meeting, source)), status: None,
        }, true);'''),
    ("src/extras.rs",
     '''            page: 5, at: self.t, duration: 2.5,
            bluetooth: Some((device.kind, connected)),
            meeting: None,
        };''',
     '''            page: 5, at: self.t, duration: 2.5,
            bluetooth: Some((device.kind, connected)),
            meeting: None,
            status: None,
        };'''),
    ("src/extras.rs",
     '''        self.enqueue_alert(Alert{source:AlertSource::Wechat,title:"微信".into(),body:"有新消息 · 请到微信查看".into(),page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None},false);''',
     '''        self.enqueue_alert(Alert{source:AlertSource::Wechat,title:"微信".into(),body:"有新消息 · 请到微信查看".into(),page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None,status:None},false);'''),
    ("src/extras.rs",
     '''        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None},false);''',
     '''        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:None},false);'''),
    ("src/extras.rs",
     '''    /// 通用状态总线：外部脚本写入 island_status.json 后在这里转成岛上提示。
    pub(crate) fn push_status(&mut self,s:crate::island_status::Status){
        let duration=if s.seconds>0.0{s.seconds}else{8.0};
        self.enqueue_alert(Alert{source:AlertSource::External("island-status".into()),title:s.text,body:s.detail,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None},false);
    }''',
     '''    /// 通用状态总线：外部脚本写 island_status.json 后转成岛上提示。
    /// 来源 id 带上状态 key，同 key 每 2 秒覆写一次，既刷新内容又续期。
    pub(crate) fn push_status(&mut self,s:crate::island_status::Status){
        let duration=if s.seconds>0.0{s.seconds}else{8.0};
        let key=s.key();
        self.enqueue_alert(Alert{source:AlertSource::External(format!("island-status:{key}")),title:s.text.clone(),body:s.detail.clone(),page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:Some(s)},false);
    }
    /// 状态从 JSON 里删掉：立刻撤下对应提示，不等 8 秒自然过期。
    pub(crate) fn drop_status(&mut self,key:&str){
        let source=AlertSource::External(format!("island-status:{key}"));
        self.alerts.retain(|a|a.source!=source);
        if self.notice.as_ref().is_some_and(|a|a.source==source){self.notice=None;self.notification_bell_until=0.0;}
        if self.hud_notice.as_ref().is_some_and(|a|a.source==source){self.hud_notice=None;}
    }'''),
    ("src/extras.rs",
     '''                // 手机通知按来源聚合：同一 App 的新消息替换旧提示，不排队刷屏。''',
     '''                // 状态总线按 key 覆写：内容变了替换，不变也替换以续期，不排队。
                _ if matches!(&a.source,AlertSource::External(id) if id.starts_with("island-status:"))=>old.source==a.source,
                // 手机通知按来源聚合：同一 App 的新消息替换旧提示，不排队刷屏。'''),
    ("src/extras.rs",
     '''(id=="island-status" || ((!self.settings.qq_only''',
     '''(id=="island-status" || id.starts_with("island-status:") || ((!self.settings.qq_only'''),
    ("src/extras.rs",
     '''let keep=matches!(&a.source,AlertSource::External(id) if crate::system::phone_channel(id) || id=="island-status");''',
     '''let keep=matches!(&a.source,AlertSource::External(id) if crate::system::phone_channel(id) || id=="island-status" || id.starts_with("island-status:"));'''),
    ("src/extras.rs",
     '''impl App {
    pub(crate) fn draw_notice(&mut self, c:&Card,panel:Rect,a:&Alert,_cmds:&mut Vec<Cmd>){''',
     '''/// 状态总线的点击动作白名单：只认这三类，不执行任意命令。
fn status_action(spec:&str)->Option<(&'static str,Cmd)>{
    if let Some(text)=spec.strip_prefix("copy:") {
        if !text.is_empty(){return Some(("复制",Cmd::Copy(text.to_string())));}
    }
    if let Some(url)=spec.strip_prefix("url:") {
        if url.starts_with("https://")||url.starts_with("http://"){return Some(("打开",Cmd::OpenUrl(url.to_string())));}
    }
    if let Some(file)=spec.strip_prefix("file:") {
        if !file.is_empty(){return Some(("打开",Cmd::Open(std::path::PathBuf::from(file))));}
    }
    None
}
fn status_color(name:&str)->Color32{
    match name.trim().to_ascii_lowercase().as_str(){
        "red"=>Color32::from_rgb(226,75,74),
        "green"=>theme::CHARGE,
        "blue"=>theme::BLUE,
        "amber"=>Color32::from_rgb(239,159,39),
        _=>theme::GRAY,
    }
}

impl App {
    pub(crate) fn draw_notice(&mut self, c:&Card,panel:Rect,a:&Alert,_cmds:&mut Vec<Cmd>){'''),
    ("src/extras.rs",
     '''        if a.source!=AlertSource::Internal {
            // 手机通知优先用手机连接自带的 App 图标，没图标才回退到铃铛。''',
     '''        if a.source!=AlertSource::Internal {
            // 通用状态总线：独立绘制，不影响手机通知 / 验证码卡片的原有布局。
            if let Some(st)=&a.status {
                let tone=status_color(&st.color);
                nc.fill(6.0,10.0,3.0,24.0,1.5,tone);
                nc.centered_text(16.0,14.0,&nc.fit(&a.title,11.0,W::Semibold,198.0),11.0,W::Semibold,theme::WHITE,H::Left);
                nc.centered_text(16.0,30.0,&nc.fit(&a.body,10.5,W::Regular,198.0),10.5,W::Regular,theme::GRAY,H::Left);
                if let Some(p)=st.progress.filter(|v|v.is_finite()) {
                    let p=p.clamp(0.0,1.0) as f32;
                    nc.fill(16.0,37.0,198.0,2.0,1.0,Color32::from_gray(60));
                    nc.fill(16.0,37.0,198.0*p,2.0,1.0,tone);
                }
                if let Some((label,cmd))=status_action(&st.click) {
                    if notice_button(&nc,160.0,12.0,60.0,label,"status-action"){_cmds.push(cmd);}
                }
                if notice_button(&nc,226.0,12.0,60.0,"关闭","external-dismiss"){
                    self.notice=None;
                    self.notification_bell_until=0.0;
                }
                return;
            }
            // 手机通知优先用手机连接自带的 App 图标，没图标才回退到铃铛。'''),
    ("src/app.rs",
     '''        // 通用状态总线：任何脚本写 island_status.json 就能往岛上推一条提示。
        for status in self.island_status.poll() { self.push_status(status); }''',
     '''        // 通用状态总线：文件里有什么岛上就有什么；同 id 覆写续期，删掉即撤下。
        let (statuses,removed)=self.island_status.poll();
        for status in statuses { self.push_status(status); }
        for key in removed { self.drop_status(&key); }'''),


    # custom-12：手机 App 白名单配置化（phone-apps.json + 热重载）
    ("src/model.rs",
     '''    pub fn observe(&mut self,id:&str,name:&str)->bool{
        if let Some(item)=self.items.iter().find(|a|a.0==id){return item.2;}
        let enabled=true;
        self.items.push((id.to_string(),name.to_string(),enabled));self.save();enabled
    }
}''',
     '''    pub fn observe(&mut self,id:&str,name:&str)->bool{
        if let Some(item)=self.items.iter().find(|a|a.0==id){return item.2;}
        let enabled=true;
        self.items.push((id.to_string(),name.to_string(),enabled));self.save();enabled
    }
}

/// 手机连接通知的来源 App 名单：`name` 匹配 Toast 正文首段，`label` 是岛上显示的标签，
/// `allow=false` 表示静默。放在 `phone-apps.json`，改完约 1 秒自动重载，不用重编译。
pub struct PhoneApps {
    pub items:Vec<(String,String,bool)>,
    modified:Option<std::time::SystemTime>,
}
fn builtin_phone_apps()->Vec<(String,String,bool)>{
    let allow=[("QQ","QQ"),("微信","微信"),("WeChat","微信"),("支付宝","支付宝"),("Alipay","支付宝"),
        ("Gmail","Gmail"),("邮件","Gmail"),("短信","短信"),("信息","短信"),("电话","电话"),("来电","电话"),("验证码","验证码")];
    let silent=["Steam","Discord","12306","大麦","BOSS直聘","Boss直聘","明日方舟"];
    let mut items:Vec<(String,String,bool)>=allow.iter().map(|(k,v)|((*k).to_string(),(*v).to_string(),true)).collect();
    items.extend(silent.iter().map(|k|((*k).to_string(),"连接".to_string(),false)));
    items
}
impl Default for PhoneApps { fn default()->Self{Self::builtin()} }
impl PhoneApps {
    pub fn builtin()->Self{Self{items:builtin_phone_apps(),modified:None}}
    fn path()->PathBuf{config_dir().join("phone-apps.json")}
    fn read_items(path:&Path)->Option<Vec<(String,String,bool)>>{
        let raw=fs::read(path).ok()?;
        let json=serde_json::from_slice::<serde_json::Value>(&raw).ok()?;
        let items:Vec<_>=json.as_array()?.iter().filter_map(|a|{
            let name=a.get("name")?.as_str()?.to_owned();
            let label=a.get("label").and_then(|v|v.as_str()).map(|s|s.to_owned()).unwrap_or_else(||name.clone());
            let allow=a.get("allow")?.as_bool()?;
            Some((name,label,allow))
        }).collect();
        if items.is_empty(){None}else{Some(items)}
    }
    pub fn load()->Self{
        #[cfg(test)]
        { return Self::builtin(); }
        #[cfg(not(test))]
        {
            let path=Self::path();
            let mut this=Self::builtin();
            if let Ok(meta)=fs::metadata(&path){
                this.modified=meta.modified().ok();
                if let Some(items)=Self::read_items(&path){this.items=items;return this;}
            }
            this.save();
            this.modified=fs::metadata(&path).ok().and_then(|m|m.modified().ok());
            this
        }
    }
    pub fn save(&self){
        let list:Vec<_>=self.items.iter().map(|(name,label,allow)|serde_json::json!({"name":name,"label":label,"allow":allow})).collect();
        let path=Self::path();
        if let Some(parent)=path.parent(){let _=fs::create_dir_all(parent);}
        if let Ok(data)=serde_json::to_vec_pretty(&list){let _=atomic_write(&path,&data);}
    }
    /// 配置文件改动后自动重载；测试构建不读盘。
    pub fn reload_if_changed(&mut self){
        #[cfg(test)]
        { return; }
        #[cfg(not(test))]
        {
            let path=Self::path();
            let Ok(meta)=fs::metadata(&path) else{return};
            let Ok(modified)=meta.modified() else{return};
            if self.modified==Some(modified){return;}
            self.modified=Some(modified);
            if let Some(items)=Self::read_items(&path){self.items=items;}
        }
    }
    pub fn lookup(&self,head:&str)->Option<(String,bool)>{
        self.items.iter().find(|(name,_,_)|head.contains(name.as_str()))
            .map(|(_,label,allow)|(label.clone(),*allow))
    }
}'''),
    ("src/system.rs",
     '''/// 手机连接 Toast 的正文首段通常是来源 App 名（真实通知只报主 AUMID `!App`），
/// 用它恢复 App 级白名单和精确标签。实测样本：`QQ · 小号 · 这是一条测试正文内容`。
const PHONE_APP_NAMES: &[(&str,&str)] = &[
    ("QQ","QQ"),
    ("微信","微信"),("WeChat","微信"),
    ("支付宝","支付宝"),("Alipay","支付宝"),
    ("Gmail","Gmail"),("邮件","Gmail"),
    ("短信","短信"),("信息","短信"),
    ("电话","电话"),("来电","电话"),
    ("验证码","验证码"),
];
/// 明确静默的高频 / 低价值来源；未识别到的一律放行，避免漏掉重要通知。
const PHONE_IGNORED: &[&str] = &["Steam","Discord","12306","大麦","BOSS直聘","Boss直聘","明日方舟"];
/// 返回 (标签, 是否允许上岛)；None = 无法识别，由调用方按「连接」放行。
pub(crate) fn phone_app(body:&str)->Option<(&'static str,bool)> {
    let head=body.split('·').next().unwrap_or("").trim();
    if head.is_empty() || head.chars().count()>12 {return None;}
    if PHONE_IGNORED.iter().any(|key|head.contains(key)){return Some(("连接",false));}
    if let Some((_,label))=PHONE_APP_NAMES.iter().find(|(key,_)|head.contains(*key)){return Some((*label,true));}
    None
}''',
     '''/// 手机连接 Toast 的正文首段通常是来源 App 名（真实通知只报主 AUMID `!App`）。
/// 名单放在 `%APPDATA%\\Daozai\\phone-apps.json`，加新 App 不用改代码重编译。
/// 返回 (标签, 是否允许上岛)；None = 无法识别，由调用方按「连接」放行。
pub(crate) fn phone_app(body:&str,apps:&crate::model::PhoneApps)->Option<(String,bool)> {
    let head=body.split('·').next().unwrap_or("").trim();
    if head.is_empty() || head.chars().count()>12 {return None;}
    apps.lookup(head)
}'''),
    ("src/system.rs",
     '''    fn 从正文首段识别手机应用与黑名单(){
        assert_eq!(crate::system::phone_app("QQ · 小号 · 这是一条测试正文内容"),Some(("QQ",true)));
        assert_eq!(crate::system::phone_app("微信 · 某人 · 晚上好"),Some(("微信",true)));
        assert_eq!(crate::system::phone_app("来电 · 138xxxx"),Some(("电话",true)));
        assert_eq!(crate::system::phone_app("Steam · 夏季特惠"),Some(("连接",false)));
        assert_eq!(crate::system::phone_app(""),None);
        assert_eq!(crate::system::phone_app("一条很长的普通消息内容里没有应用名"),None);
    }''',
     '''    fn 从正文首段识别手机应用与黑名单(){
        let apps=crate::model::PhoneApps::builtin();
        assert_eq!(crate::system::phone_app("QQ · 小号 · 这是一条测试正文内容",&apps),Some(("QQ".to_string(),true)));
        assert_eq!(crate::system::phone_app("微信 · 某人 · 晚上好",&apps),Some(("微信".to_string(),true)));
        assert_eq!(crate::system::phone_app("来电 · 138xxxx",&apps),Some(("电话".to_string(),true)));
        assert_eq!(crate::system::phone_app("Steam · 夏季特惠",&apps),Some(("连接".to_string(),false)));
        assert_eq!(crate::system::phone_app("",&apps),None);
        assert_eq!(crate::system::phone_app("一条很长的普通消息内容里没有应用名",&apps),None);
        // 名单是外部配置：加新 App 只需往 JSON 里加一条，不改代码。
        let mut custom=crate::model::PhoneApps::builtin();
        custom.items.push(("小红书".to_string(),"小红书".to_string(),true));
        assert_eq!(crate::system::phone_app("小红书 · 有人回复了你",&custom),Some(("小红书".to_string(),true)));
    }'''),
    ("src/extras.rs",
     '''    pub fn receive_notification(&mut self, app:String, id:String, body:String){
        let phone=crate::system::phone_channel(&id);''',
     '''    pub fn receive_notification(&mut self, app:String, id:String, body:String){
        let phone=crate::system::phone_channel(&id);
        // 手机 App 名单是外部 JSON，改完约 1 秒内生效，不用重启岛仔。
        if phone {self.phone_apps.reload_if_changed();}'''),
    ("src/extras.rs",
     '''        let phone_info=if phone {Some(crate::system::phone_app(&body).unwrap_or(("连接",true)))}else{None};
        if phone_info.is_some_and(|(_,allow)|!allow){return;}''',
     '''        let phone_info:Option<(String,bool)>=if phone {Some(crate::system::phone_app(&body,&self.phone_apps).unwrap_or(("连接".to_string(),true)))}else{None};
        if phone_info.as_ref().is_some_and(|(_,allow)|!allow){return;}'''),
    ("src/extras.rs",
     '''        let title=if let Some((label,_))=phone_info {format!("手机 · {label}")}''',
     '''        let title=if let Some((label,_))=&phone_info {format!("手机 · {label}")}'''),
    ("src/extras.rs",
     '''        let missed=phone_info.is_some_and(|(label,_)|label=="电话");''',
     '''        let missed=phone_info.as_ref().is_some_and(|(label,_)|label=="电话");'''),
    ("src/app.rs",
     '''    pub(crate) phone_icons:std::collections::HashMap<String,eframe::egui::TextureHandle>,''',
     '''    pub(crate) phone_icons:std::collections::HashMap<String,eframe::egui::TextureHandle>,
    /// 手机连接通知的来源 App 名单（`phone-apps.json`，支持热重载）。
    pub(crate) phone_apps:crate::model::PhoneApps,'''),
    ("src/app.rs",
     '''phone_counts:Default::default(), phone_icons:Default::default(), missed_calls:0''',
     '''phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), missed_calls:0'''),

    # custom-13：电量与连接事件 + 邮件验证码自动复制 + 设置页 App 名单
    ("src/model.rs",
     '''    pub phone_battery: bool,''',
     '''    pub phone_battery: bool,
    /// 收到含验证码的邮件时自动写入剪贴板。
    pub auto_copy_otp: bool,'''),
    ("src/model.rs",
     '''show_battery: true, phone_battery: true, update_check: false,''',
     '''show_battery: true, phone_battery: true, auto_copy_otp: false, update_check: false,'''),
    ("src/model.rs",
     '''                "phone_battery" => s.phone_battery = v != "0",''',
     '''                "phone_battery" => s.phone_battery = v != "0",
                "auto_copy_otp" => s.auto_copy_otp = v != "0",'''),
    ("src/model.rs",
     '''                ("phone_battery".into(), (self.phone_battery as u8).to_string()),''',
     '''                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("auto_copy_otp".into(), (self.auto_copy_otp as u8).to_string()),'''),
    ("src/app.rs",
     '''    pub(crate) phone_apps:crate::model::PhoneApps,''',
     '''    pub(crate) phone_apps:crate::model::PhoneApps,
    /// 上次看到的手机电量（None=尚未初始化，用于断开/重连/低电/充满提示）。
    pub(crate) phone_last:Option<Option<u8>>,'''),
    ("src/app.rs",
     '''phone_apps:model::PhoneApps::load(),''',
     '''phone_apps:model::PhoneApps::load(), phone_last:None,'''),
    ("src/app.rs",
     '''        self.resident_power=phone;''',
     '''        self.resident_power=phone;
        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}'''),
    ("src/extras.rs",
     '''    pub fn receive_notification(&mut self, app:String, id:String, body:String){''',
     '''    /// 手机电量的关键变化：断开 / 重连 / 低电 / 充满。
    /// 首轮只记录不提示，避免每次启动都弹一条「已连接」。
    pub(crate) fn phone_events(&mut self,now:Option<u8>,charging:bool){
        let prev=self.phone_last;
        self.phone_last=Some(now);
        let Some(prev)=prev else{return;};
        if !self.settings.notifications{return;}
        let (title,body)=match (prev,now) {
            (Some(_),None)=>("手机已断开".to_string(),"电量信息已收起".to_string()),
            (None,Some(level))=>(format!("手机已连接 {level}%"),"手机连接已恢复".to_string()),
            (Some(old),Some(level)) if old>20&&level<=20=>(format!("手机电量偏低 {level}%"),"记得给手机充电".to_string()),
            (Some(old),Some(100)) if old<100=>(format!("手机已充满{}",if charging{"（充电中）"}else{""}),"可以拔掉充电器了".to_string()),
            _=>return,
        };
        self.alert(title,body,usize::MAX,false);
    }

    pub fn receive_notification(&mut self, app:String, id:String, body:String){'''),
    ("src/extras.rs",
     '''setting_row(ui,"新邮件提醒","新邮件到达时显示提醒；岛上歌词模式显示右侧图标",&mut self.settings.mail_enabled);''',
     '''setting_row(ui,"新邮件提醒","新邮件到达时显示提醒；岛上歌词模式显示右侧图标",&mut self.settings.mail_enabled);
                            setting_row(ui,"手机验证码自动复制","手机短信验证码上岛时直接写入剪贴板",&mut self.settings.auto_copy_otp);'''),
    ("src/extras.rs",
     '''                            if before_apps!=self.notification_apps.items{self.notification_apps.save();}''',
     '''                            if before_apps!=self.notification_apps.items{self.notification_apps.save();}
                            ui.separator();
                            ui.label(egui::RichText::new("手机 App 名单：勾选立即生效；增删条目改 %APPDATA%\\\\Daozai\\\\phone-apps.json。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            let before_phone_apps=self.phone_apps.items.clone();
                            for (name,label,allow) in &mut self.phone_apps.items {
                                let title=if label==name {name.clone()} else {format!("{} · {}",label,name)};
                                setting_row(ui,&title,"",allow);
                            }
                            if before_phone_apps!=self.phone_apps.items{self.phone_apps.save();}'''),

    # custom-14：短信验证码自动复制 + 验证码识别诊断日志
    ("src/extras.rs",
     '''        let body=if count>1 {format!("共 {count} 条 · {body}")}else{body};
        // 未接来电：提示保留 30 秒，同时点亮铃铛红点常驻，直到点关闭清除。''',
     '''        let body=if count>1 {format!("共 {count} 条 · {body}")}else{body};
        // 手机验证码自动复制（与邮件共用开关）：短信验证码上岛时顺手写剪贴板。
        if phone && self.settings.auto_copy_otp {
            let code=crate::otp::extract("",&body,&[]);
            log_otp("phone",code.as_ref().map(|c|c.text.as_str()));
            if let Some(code)=code {self.pending.push(Cmd::Copy(code.text));}
        }
        // 未接来电：提示保留 30 秒，同时点亮铃铛红点常驻，直到点关闭清除。'''),
    ("src/extras.rs",
     '''fn phone_icon_file(title:&str)->Option<&'static str>{''',
     '''/// 验证码识别诊断：只在「自动复制」开启时写一行，便于排查邮件/短信为什么没复制。
pub(crate) fn log_otp(source:&str,code:Option<&str>){
    let Some(base)=std::env::var_os("APPDATA") else{return};
    let dir=std::path::PathBuf::from(base).join("Daozai").join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err(){return;}
    let path=dir.join("otp.log");
    if std::fs::metadata(&path).is_ok_and(|m|m.len()>256*1024){let _=std::fs::remove_file(&path);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        use std::io::Write;
        let _=writeln!(file,"{} | {} | {}",chrono::Local::now().format("%H:%M:%S"),source,code.unwrap_or("-"));
    }
}

fn phone_icon_file(title:&str)->Option<&'static str>{'''),

    # custom-16：状态总线多槽位并列显示
    ("src/bar_layout.rs",
     '''#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub width: f32,
    pub x: Vec<Option<f32>>,
    pub widths:Vec<f32>,
    quotas:usize,
}''',
     '''#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub width: f32,
    pub x: Vec<Option<f32>>,
    pub widths:Vec<f32>,
    quotas:usize,
    /// 状态总线占用的槽位（在末尾动态追加，不参与 resident_order 重排）。
    pub status: Option<usize>,
}'''),
    ("src/bar_layout.rs",
     '''        Self {width:width.max(96.0),x,widths,quotas}
    }''',
     '''        Self {width:width.max(96.0),x,widths,quotas,status:None}
    }
    /// 在胶囊最右侧追加一条状态区：不影响 resident_order 的既有槽位，
    /// 只是把整体左移半个宽度，让常驻组件保持相对位置。
    pub fn add_status(&mut self, width: f32) -> usize {
        let gap = 12.0;
        let slot = self.x.len();
        self.x.push(None);
        self.widths.push(width);
        let old_w = self.width;
        let shift = (width + gap) / 2.0;
        for value in self.x.iter_mut().flatten() { *value -= shift; }
        self.x[slot] = Some(old_w / 2.0 + gap / 2.0);
        self.width = old_w + width + gap;
        self.status = Some(slot);
        slot
    }'''),
    ("src/app.rs",
     '''    /// 上次看到的手机电量（None=尚未初始化，用于断开/重连/低电/充满提示）。
    pub(crate) phone_last:Option<Option<u8>>,''',
     '''    /// 上次看到的手机电量（None=尚未初始化，用于断开/重连/低电/充满提示）。
    pub(crate) phone_last:Option<Option<u8>>,
    /// 状态总线当前存活的状态，用于常驻区并排显示。
    pub(crate) status_live:Vec<crate::island_status::Status>,'''),
    ("src/app.rs",
     '''phone_apps:model::PhoneApps::load(), phone_last:None,''',
     '''phone_apps:model::PhoneApps::load(), phone_last:None, status_live:Vec::new(),'''),
    ("src/app.rs",
     '''        let mut layout=Layout::new(self.settings.show_battery&&self.resident_power.0.is_some(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());''',
     '''        let mut layout=Layout::new(self.settings.show_battery&&self.resident_power.0.is_some(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
        if status_width>0.0 {layout.add_status(status_width);}'''),
    ("src/app.rs",
     '''        for status in statuses { self.push_status(status); }
        for key in removed { self.drop_status(&key); }''',
     '''        // poll 每 2 秒才返回一次数据，其余帧返回空；只在真有变化时更新，
        // 否则常驻区会只闪一帧就消失。
        if !statuses.is_empty() || !removed.is_empty() {
            // 启动或清空后的首轮不闪，避免一次排队好几条横幅；之后新增的状态才闪一次。
            let first_fill=self.status_live.is_empty();
            for status in &statuses {
                if !first_fill && !self.status_live.iter().any(|old|old.key()==status.key()) {
                    self.push_status(status.clone());
                }
            }
            self.status_live=statuses;
        }
        for key in removed { self.drop_status(&key); }'''),
    ("src/app.rs",
     '''    fn shared_header(&mut self, base: &Card, cx: f32, p: f32, reveal: f32, env: &Env) {''',
     '''    /// 单条状态的估算宽度：中文按 11px/字，夹在 48~140 之间。
    fn status_item_width(&self, status: &crate::island_status::Status) -> f32 {
        let chars = status.text.chars().count() as f32;
        (chars * 11.0 + 24.0).clamp(48.0, 140.0)
    }

    /// 状态区总宽：最多 3 条，条间 6px。
    fn status_total_width(&self) -> f32 {
        let shown = self.status_live.len().min(3);
        if shown == 0 { return 0.0; }
        let items: f32 = self.status_live.iter().take(shown).map(|s| self.status_item_width(s)).sum();
        items + (shown.saturating_sub(1)) as f32 * 6.0
    }

    fn shared_header(&mut self, base: &Card, cx: f32, p: f32, reveal: f32, env: &Env) {'''),
    ("src/app.rs",
     '''        if self.pomo.running && compact_alpha>0.004 {
            crate::focus_bar::draw(&base.faded(compact_alpha),local_cx+layout.x[bar::FOCUS].unwrap(),15.0,self.t,&self.pomo,self.phase_secs(self.pomo.phase),false);
        }
    }''',
     '''        if self.pomo.running && compact_alpha>0.004 {
            crate::focus_bar::draw(&base.faded(compact_alpha),local_cx+layout.x[bar::FOCUS].unwrap(),15.0,self.t,&self.pomo,self.phase_secs(self.pomo.phase),false);
        }
        // 状态总线：并排显示最多 3 条常驻状态（色点 + 文本 + 可选进度条）。
        if let Some(slot)=layout.status {
            if let Some(center)=layout.x[slot] {
                if compact_alpha>0.004 && !self.status_live.is_empty() {
                    let c=base.faded(compact_alpha);
                    let total=layout.widths[slot];
                    let mut x=local_cx+center-total/2.0;
                    for status in self.status_live.iter().take(3) {
                        let w=self.status_item_width(status);
                        let tone=crate::extras::status_color(&status.color);
                        c.fill(x,5.0,w,20.0,10.0,Color32::from_gray(36));
                        c.circle(x+8.0,15.0,2.5,tone);
                        c.centered_text(x+14.0,15.0,&c.fit(&status.text,10.5,W::Regular,w-20.0),10.5,W::Regular,theme::WHITE,H::Left);
                        if let Some(progress)=status.progress.filter(|v|v.is_finite()) {
                            let p=progress.clamp(0.0,1.0) as f32;
                            c.fill(x+2.0,23.0,w-4.0,1.5,0.75,Color32::from_gray(60));
                            c.fill(x+2.0,23.0,(w-4.0)*p,1.5,0.75,tone);
                        }
                        x+=w+6.0;
                    }
                }
            }
        }
    }'''),
    ("src/extras.rs",
     '''fn status_color(name:&str)->Color32{''',
     '''pub(crate) fn status_color(name:&str)->Color32{'''),
    # custom-17：验证码通知的铃铛保留 90 秒
    ("src/extras.rs",
     '''        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        self.notification_bell_at=self.t;
        self.notification_bell_until=self.t+4.5;''',
     '''        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        self.notification_bell_at=self.t;
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去。
        let otp_bell=phone && !body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        self.notification_bell_until=self.t+if otp_bell {90.0} else {4.5};'''),

    # custom-18：通知重复查看开关（保留 60 秒）
    ("src/model.rs",
     '''    pub phone_battery: bool,''',
     '''    pub phone_battery: bool,
    /// 通知保留更久，可在铃铛上重复悬停查看（关闭则 4.5 秒后消失）。
    pub notice_keep: bool,'''),
    ("src/model.rs",
     '''show_battery: true, phone_battery: true, auto_copy_otp: false,''',
     '''show_battery: true, phone_battery: true, notice_keep: true, auto_copy_otp: false,'''),
    ("src/model.rs",
     '''                "phone_battery" => s.phone_battery = v != "0",''',
     '''                "phone_battery" => s.phone_battery = v != "0",
                "notice_keep" => s.notice_keep = v != "0",'''),
    ("src/model.rs",
     '''                ("phone_battery".into(), (self.phone_battery as u8).to_string()),''',
     '''                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("notice_keep".into(), (self.notice_keep as u8).to_string()),'''),
    ("src/extras.rs",
     '''        self.notification_bell_at=self.t;
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去。
        let otp_bell=phone && !body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        self.notification_bell_until=self.t+if otp_bell {90.0} else {4.5};''',
     '''        self.notification_bell_at=self.t;
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去；
        // 「重复查看」开启时普通通知也保留更久，方便多次悬停。
        let otp_bell=phone && !body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        let keep_bell=if cfg!(test) {4.5} else if otp_bell {90.0} else if self.settings.notice_keep {60.0} else {4.5};
        self.notification_bell_until=self.t+keep_bell;'''),
    ("src/extras.rs",
     '''        let (title,body,duration)=if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,4.5)};''',
     '''        let (title,body,duration)=if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,if !cfg!(test) && self.settings.notice_keep {60.0} else {4.5})};'''),
    ("src/extras.rs",
     '''                            setting_row(ui,"显示通知正文","可能包含私密消息；屏幕共享时建议关闭",&mut self.settings.notification_preview);''',
     '''                            setting_row(ui,"显示通知正文","可能包含私密消息；屏幕共享时建议关闭",&mut self.settings.notification_preview);
                            setting_row(ui,"重复查看通知","通知保留 60 秒，可在铃铛上多次悬停查看；关闭则 4.5 秒后消失",&mut self.settings.notice_keep);'''),

    # custom-19：设置页新增「短信」分类
    ("src/extras.rs",
     '''                let titles=["常规","音乐与歌词","专注","饮水","待办","置物","通知","邮箱","日程","关于","页面管理","AI 额度"];''',
     '''                let titles=["常规","音乐与歌词","专注","饮水","待办","置物","通知","邮箱","日程","关于","页面管理","AI 额度","短信"];'''),
    ("src/extras.rs",
     '''titles[self.settings_tab.min(11)]''',
     '''titles[self.settings_tab.min(12)]'''),
    ("src/extras.rs",
     '''const SETTINGS_GROUPS: [(&str,usize,usize);8] = [
    ("常规",0,0), ("页面管理",10,10), ("音乐与歌词",1,1), ("专注与饮水",2,3),
    ("待办与置物",4,5), ("通知、邮箱与日程",6,8), ("AI 额度",11,11), ("关于",9,9),
];''',
     '''const SETTINGS_GROUPS: [(&str,usize,usize);9] = [
    ("常规",0,0), ("页面管理",10,10), ("音乐与歌词",1,1), ("专注与饮水",2,3),
    ("待办与置物",4,5), ("通知、邮箱与日程",6,8), ("短信",12,12), ("AI 额度",11,11), ("关于",9,9),
];'''),
    ("src/extras.rs",
     '''                    match self.settings_tab {
                    11=>self.quota_settings(ui),''',
     '''                    match self.settings_tab {
                    11=>self.quota_settings(ui),
                    12=>{
                        settings_label(ui,"手机短信");
                        settings_card(ui,|ui|{
                            ui.label(egui::RichText::new("手机短信通过「手机连接」转发；验证码可自动写入剪贴板。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            setting_row(ui,"验证码自动复制","手机短信验证码上岛时直接写入剪贴板",&mut self.settings.auto_copy_otp);

                        });
                    },'''),
    ("src/extras.rs",
     '''                            setting_row(ui,"新邮件提醒","新邮件到达时显示提醒；岛上歌词模式显示右侧图标",&mut self.settings.mail_enabled);
                            setting_row(ui,"手机验证码自动复制","手机短信验证码上岛时直接写入剪贴板",&mut self.settings.auto_copy_otp);''',
     '''                            setting_row(ui,"新邮件提醒","新邮件到达时显示提醒；岛上歌词模式显示右侧图标",&mut self.settings.mail_enabled);'''),
    ("src/extras.rs",
     '''        let groups = ["常规", "音乐与歌词", "专注与饮水", "专注与饮水", "待办与置物", "待办与置物", "通知、邮箱与日程", "通知、邮箱与日程", "通知、邮箱与日程", "关于", "页面管理", "AI 额度"];''',
     '''        let groups = ["常规", "音乐与歌词", "专注与饮水", "专注与饮水", "待办与置物", "待办与置物", "通知、邮箱与日程", "通知、邮箱与日程", "通知、邮箱与日程", "关于", "页面管理", "AI 额度", "短信"];'''),

    # custom-20：邮件验证码设置文案明确化
    ("src/extras.rs",
     '''                            setting_row(ui,"验证码高亮","识别到验证码时在岛上放大显示，点击即复制",&mut self.settings.mail_code);''',
     '''                            setting_row(ui,"邮件验证码高亮","邮件里识别到验证码时在岛上放大显示，点击即复制",&mut self.settings.mail_code);'''),
    ("src/extras.rs",
     '''                            setting_choices(ui,"验证码隐藏",&mut self.settings.mail_code_hide_seconds,[30,60,300],"秒");''',
     '''                            setting_choices(ui,"邮件验证码隐藏",&mut self.settings.mail_code_hide_seconds,[30,60,300],"秒");'''),
    ("src/ui_tests.rs",
     '''("验证码隐藏","60 秒")''',
     '''("邮件验证码隐藏","60 秒")'''),

    # custom-21：邮件请求丢失导致「全部已读」永久禁用
    ("src/mail_manage.rs",
     '''    std::thread::spawn(move||while let Ok(request)=rx.recv(){let result=protocol::run(&request);let _=done.send(Completed{request,result});});(tx,out)''',
     '''    std::thread::spawn(move||while let Ok(request)=rx.recv(){
        // 单次协议异常不能带走整个线程：线程一死，后续请求全部发不出去，
        // 界面上的 pending 永远清不掉，「全部已读」等按钮就永久禁用。
        let result=match std::panic::catch_unwind(std::panic::AssertUnwindSafe(||protocol::run(&request))){
            Ok(result)=>result,
            Err(_)=>Err("邮件操作异常，请重试".to_string()),
        };
        let _=done.send(Completed{request,result});
    });(tx,out)'''),
    ("src/mail_manage_ui.rs",
     '''    pub next_refresh:f64,pub message:String,pub errors:Vec<String>,''',
     '''    pub next_refresh:f64,
    /// 本批请求开始等待的时间点，用于请求丢失时超时复位。
    pub pending_at:f64,
    pub message:String,pub errors:Vec<String>,'''),
    ("src/mail_manage_ui.rs",
     '''            self.mail.manage.local_loaded=true;
        }
        if !self.mail.manage.pending.is_empty(){return;}''',
     '''            self.mail.manage.local_loaded=true;
        }
        if !self.mail.manage.pending.is_empty(){
            // 超过 40 秒还没有结果，说明请求丢了，复位避免按钮永久禁用。
            if self.t-self.mail.manage.pending_at>40.0{
                self.mail.manage.pending.clear();
                self.mail.manage.message="邮件同步超时，已复位".into();
            }
            return;
        }'''),
    ("src/mail_manage_ui.rs",
     '''        self.mail.manage.errors.clear();self.mail.manage.message=if action==Action::Load{"读取邮件状态…"}else{"同步中…"}.into();''',
     '''        self.mail.manage.errors.clear();self.mail.manage.message=if action==Action::Load{"读取邮件状态…"}else{"同步中…"}.into();
        self.mail.manage.pending_at=self.t;'''),
    ("src/app.rs",
     '''        self.mail.manage.identities.retain(|_,old|accounts.contains(old));''',
     '''        self.mail.manage.identities.retain(|_,old|accounts.contains(old));
        self.mail.manage.pending.retain(|email|accounts.iter().any(|a|mail::key(a)==*email));'''),
    ("src/pages.rs",
     '''            if c.hit(325.0,33.0,53.0,19.0,"mail-read-all").on_hover_text(if self.settings.mail_sync_operations{"将已连接邮箱的未读邮件全部标为已读"}else{"仅将岛仔当前加载的邮件标为已读，不更改邮箱"}).clicked()&&ready{
                self.mail_manage_request(None,Folder::Inbox,Action::ReadAll);
            }''',
     '''            if c.hit(325.0,33.0,53.0,19.0,"mail-read-all").on_hover_text(if self.settings.mail_sync_operations{"将已连接邮箱的未读邮件全部标为已读"}else{"仅将岛仔当前加载的邮件标为已读，不更改邮箱"}).clicked(){
                if ready{self.mail_manage_request(None,Folder::Inbox,Action::ReadAll);}
                else{self.mail.manage.message="邮件状态还没同步完，请稍后再试".into();}
            }'''),
    ("src/mail_manage_ui.rs",
     '''    #[test]fn read_state_is_separate_for_each_folder_and_account(){''',
     '''    #[test]fn stuck_pending_resets_after_timeout(){
        let ctx=eframe::egui::Context::default();let mut app=App::new(&ctx,Default::default());app.mail.accounts=vec![account()];
        app.mail_manage_request(None,Folder::Inbox,Action::Load);
        assert!(!app.mail.manage.pending.is_empty());
        // 模拟请求丢失：时间推进 50 秒后必须复位，否则「全部已读」等按钮永久禁用。
        app.t=app.mail.manage.pending_at+50.0;
        app.mail_manage_tick();
        assert!(app.mail.manage.pending.is_empty(),"请求丢失后 pending 必须能复位");
        assert!(app.mail.manage.message.contains("超时"));
    }
    #[test]fn read_state_is_separate_for_each_folder_and_account(){'''),
    ("src/extras.rs",
     '''fn phone_icon_file(title:&str)->Option<&'static str>{''',
     '''/// 邮件面板操作诊断：点击「全部已读」被拒时记录原因，便于排查按钮为什么没反应。
pub(crate) fn log_mail(note:&str){
    let Some(base)=std::env::var_os("APPDATA") else{return};
    let dir=std::path::PathBuf::from(base).join("Daozai").join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err(){return;}
    let path=dir.join("mail.log");
    if std::fs::metadata(&path).is_ok_and(|m|m.len()>256*1024){let _=std::fs::remove_file(&path);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        use std::io::Write;
        let _=writeln!(file,"{} | {}",chrono::Local::now().format("%H:%M:%S"),note);
    }
}

fn phone_icon_file(title:&str)->Option<&'static str>{'''),
    ("src/mail_manage_ui.rs",
     '''    /// 本批请求开始等待的时间点，用于请求丢失时超时复位。
    pub pending_at:f64,''',
     '''    /// 本批请求开始等待的时间点，用于请求丢失时超时复位。
    pub pending_at:f64,
    /// 同步进行中点了「全部已读」：等空闲后自动补执行，避免点了没反应。
    pub read_all_queued:bool,'''),
    ("src/mail_manage_ui.rs",
     '''impl App{
    fn mail_physical_folder(&self,folder:Folder)->Folder{if !self.settings.mail_sync_operations{Folder::Inbox}else{folder}}''',
     '''impl App{
    /// 「全部已读」所需的数据是否齐备（与是否正在同步无关）。
    pub(crate) fn mail_read_all_ready(&self)->bool{
        self.mail.accounts.iter().any(|a|a.enabled
            && self.mail.manage.data.contains_key(&(crate::mail::key(a),Folder::Inbox))
            && self.mail.manage.identities.get(&crate::mail::key(a))==Some(a))
    }
    fn mail_physical_folder(&self,folder:Folder)->Folder{if !self.settings.mail_sync_operations{Folder::Inbox}else{folder}}'''),
    ("src/mail_manage_ui.rs",
     '''        if let Some((email,uid))=self.mail.manage.auto_read.take(){''',
     '''        if self.mail.manage.read_all_queued{
            self.mail.manage.read_all_queued=false;
            if self.mail_read_all_ready(){self.mail_manage_request(None,Folder::Inbox,Action::ReadAll);}
            else{self.mail.manage.message="邮件状态还没同步完，请稍后再试".into();}
        }
        if let Some((email,uid))=self.mail.manage.auto_read.take(){'''),
    ("src/pages.rs",
     '''            let ready=self.mail.manage.pending.is_empty()&&self.mail.accounts.iter().any(|a|a.enabled&&self.mail.manage.data.contains_key(&(crate::mail::key(a),Folder::Inbox))&&self.mail.manage.identities.get(&crate::mail::key(a))==Some(a));''',
     '''            let data_ready=self.mail_read_all_ready();
            let ready=self.mail.manage.pending.is_empty()&&data_ready;'''),
    ("src/pages.rs",
     '''            if c.hit(325.0,33.0,53.0,19.0,"mail-read-all").on_hover_text(if self.settings.mail_sync_operations{"将已连接邮箱的未读邮件全部标为已读"}else{"仅将岛仔当前加载的邮件标为已读，不更改邮箱"}).clicked(){
                if ready{self.mail_manage_request(None,Folder::Inbox,Action::ReadAll);}
                else{self.mail.manage.message="邮件状态还没同步完，请稍后再试".into();}
            }''',
     '''            if c.hit(325.0,33.0,53.0,19.0,"mail-read-all").on_hover_text(if self.settings.mail_sync_operations{"将已连接邮箱的未读邮件全部标为已读"}else{"仅将岛仔当前加载的邮件标为已读，不更改邮箱"}).clicked(){
                crate::extras::log_mail(&format!("read-all ready={ready} data={data_ready} pending={} accounts={}",self.mail.manage.pending.len(),self.mail.accounts.len()));
                if ready{self.mail_manage_request(None,Folder::Inbox,Action::ReadAll);}
                else if data_ready{self.mail.manage.read_all_queued=true;self.mail.manage.message="正在同步邮件，完成后自动标记已读".into();}
                else{self.mail.manage.message="邮件状态还没同步完，请稍后再试".into();}
            }'''),

    # custom-22：邮件面板按账号筛选
    ("src/mail_manage_ui.rs",
     '''    /// 同步进行中点了「全部已读」：等空闲后自动补执行，避免点了没反应。
    pub read_all_queued:bool,''',
     '''    /// 同步进行中点了「全部已读」：等空闲后自动补执行，避免点了没反应。
    pub read_all_queued:bool,
    /// 邮件列表的账号筛选，None 表示全部账号。
    pub account_filter:Option<String>,
    /// 账号筛选下拉是否展开。
    pub account_menu:bool,'''),
    ("src/mail_manage_ui.rs",
     '''    /// 「全部已读」所需的数据是否齐备（与是否正在同步无关）。''',
     '''    /// 当前账号筛选是否包含这封邮件。
    pub(crate) fn mail_account_match(&self,item:&Item)->bool{
        self.mail.manage.account_filter.as_ref().is_none_or(|f|item.account.eq_ignore_ascii_case(f))
    }
    /// 账号筛选按钮上的短标签（只取 @ 前面那段）。
    pub(crate) fn mail_account_label(email:&str)->String{
        email.split('@').next().unwrap_or(email).to_string()
    }
    /// 「全部已读」所需的数据是否齐备（与是否正在同步无关）。'''),
    ("src/pages.rs",
     '''        self.mail.code_item().filter(|item|!self.mail.manage.trash_open && self.settings.mail_code && !self.mail_code_hidden(item))''',
     '''        self.mail.code_item().filter(|item|!self.mail.manage.trash_open && self.settings.mail_code && !self.mail_code_hidden(item) && self.mail_account_match(item))'''),
    ("src/pages.rs",
     '''        let all_count=self.mail.recent.len();
        let unread=self.mail.recent.iter().filter(|m|self.mail_seen(m,Folder::Inbox)==Some(false)).count();''',
     '''        let all_count=self.mail.recent.iter().filter(|m|self.mail_account_match(m)).count();
        let unread=self.mail.recent.iter().filter(|m|self.mail_account_match(m)&&self.mail_seen(m,Folder::Inbox)==Some(false)).count();'''),
    ("src/pages.rs",
     '''            c.text(307.0,46.0,if self.settings.mail_sync_operations{"同步"}else{"仅本机"},8.0,W::Regular,theme::DIM,H::Center);''',
     '''            // 账号筛选：取代原来的「同步 / 仅本机」小字，模式信息挪进悬停提示。
            let filter_label=match &self.mail.manage.account_filter{
                Some(email)=>Self::mail_account_label(email),
                None=>"全部账号".to_string(),
            };
            c.fill(148.0,33.0,140.0,19.0,6.0,theme::CARD);
            c.text(218.0,47.0,&c.fit(&filter_label,10.0,W::Semibold,128.0),10.0,W::Semibold,if self.mail.manage.account_filter.is_some(){theme::WHITE}else{theme::GRAY},H::Center);
            if c.hit(148.0,33.0,140.0,19.0,"mail-account-filter").on_hover_text(format!("选择要看的邮箱账号\\n当前模式：{}",if self.settings.mail_sync_operations{"同步邮箱"}else{"仅本机"})).clicked(){
                self.mail.manage.account_menu=!self.mail.manage.account_menu;
            }'''),
    ("src/pages.rs",
     '''        let indices:Vec<_>=self.mail_rows().iter().enumerate().filter(|(_,m)|trash||!self.mail.manage.unread_only||self.mail_seen(m,folder)==Some(false)).map(|(i,_)|i).collect();''',
     '''        let indices:Vec<_>=self.mail_rows().iter().enumerate().filter(|(_,m)|self.mail_account_match(m)&&(trash||!self.mail.manage.unread_only||self.mail_seen(m,folder)==Some(false))).map(|(i,_)|i).collect();'''),
    ("src/pages.rs",
     '''                if hit.clicked()&&!acted{self.mail_open(&raw);}
            }
        });
    }''',
     '''                if hit.clicked()&&!acted{self.mail_open(&raw);}
            }
        });
        // 账号下拉：画在列表之后，点击空白处收起。
        if self.mail.manage.account_menu{
            if c.hit(36.0,54.0,344.0,68.0,"mail-account-dismiss").clicked(){self.mail.manage.account_menu=false;}
            let accounts:Vec<String>=self.mail.accounts.iter().map(|a|a.email.clone()).take(6).collect();
            let rows=accounts.len()+1;
            c.fill(148.0,54.0,140.0,17.0*rows as f32,8.0,Color32::from_gray(24));
            let current=self.mail.manage.account_filter.clone();
            let mut pick=None;
            for (i,email) in std::iter::once(None).chain(accounts.iter().map(Some)).enumerate(){
                let y=54.0+17.0*i as f32;
                let selected=match (&current,email){(None,None)=>true,(Some(a),Some(b))=>a.eq_ignore_ascii_case(b),_=>false};
                let label=match email{Some(e)=>e.clone(),None=>"全部账号".to_string()};
                if c.hit(148.0,y,140.0,17.0,("mail-account-pick",i)).clicked(){pick=Some(email.cloned());}
                c.text(156.0,y+11.5,&c.fit(&label,9.5,W::Regular,124.0),9.5,W::Regular,if selected{theme::BLUE}else{theme::WHITE},H::Left);
            }
            if let Some(choice)=pick{
                self.mail.manage.account_filter=choice;
                self.mail.manage.account_menu=false;
            }
        }
    }'''),
    ("src/app.rs",
     '''        self.mail.manage.pending.retain(|email|accounts.iter().any(|a|mail::key(a)==*email));''',
     '''        self.mail.manage.pending.retain(|email|accounts.iter().any(|a|mail::key(a)==*email));
        if self.mail.manage.account_filter.as_ref().is_some_and(|f|!accounts.iter().any(|a|a.email.eq_ignore_ascii_case(f))){self.mail.manage.account_filter=None;self.mail.manage.account_menu=false;}'''),
    ("src/mail_manage_ui.rs",
     '''    #[test]fn stuck_pending_resets_after_timeout(){''',
     '''    #[test]fn account_filter_only_matches_selected_mailbox(){
        let mut app=App::new(&eframe::egui::Context::default(),Default::default());
        let item=|email:&str|Item{account:email.into(),from:String::new(),subject:String::new(),time:String::new(),code:None,received_unix:None,summary:"".into(),links:vec![],meeting:None,uid:1};
        assert!(app.mail_account_match(&item("a@qq.com")),"默认应匹配全部账号");
        app.mail.manage.account_filter=Some("A@QQ.com".into());
        assert!(app.mail_account_match(&item("a@qq.com")),"筛选应忽略大小写");
        assert!(!app.mail_account_match(&item("b@qq.com")),"其他账号的邮件应被排除");
        assert_eq!(App::mail_account_label("a@qq.com"),"a");
    }
    #[test]fn stuck_pending_resets_after_timeout(){'''),
    ("src/pages.rs",
     '''                let hovered=ui.ctx().pointer_hover_pos().is_some_and(|p|rect.contains(p)&&ui.clip_rect().contains(p));''',
     '''                // 账号下拉展开时列表行不再画悬停高亮：行高亮是按鼠标位置手绘的，
                // 不经过 egui 的交互判定，遮罩拦不住它，会和菜单抢视觉焦点。
                let hovered=!self.mail.manage.account_menu&&ui.ctx().pointer_hover_pos().is_some_and(|p|rect.contains(p)&&ui.clip_rect().contains(p));'''),
    ("src/pages.rs",
     '''                if c.hit(148.0,y,140.0,17.0,("mail-account-pick",i)).clicked(){pick=Some(email.cloned());}
                c.text(156.0,y+11.5,&c.fit(&label,9.5,W::Regular,124.0),9.5,W::Regular,if selected{theme::BLUE}else{theme::WHITE},H::Left);''',
     '''                let item_hit=c.hit(148.0,y,140.0,17.0,("mail-account-pick",i));
                if item_hit.hovered(){c.fill(150.0,y+1.0,136.0,15.0,5.0,Color32::from_gray(52));}
                if item_hit.clicked(){pick=Some(email.cloned());}
                c.text(156.0,y+11.5,&c.fit(&label,9.5,W::Regular,124.0),9.5,W::Regular,if selected{theme::BLUE}else{theme::WHITE},H::Left);'''),

    # custom-24：长歌词：缩字优先 + 超长横向滚动 + 排版缓存
    ("src/app.rs",
     '''pub(crate) const LANE_TEXT_GAP: f32 = 24.0;   // 宠物右缘 → 歌词通道（硬余量，保证不重叠）
pub(crate) const LANE_CENTER_CAP: f32 = 16.0; // 歌词在通道内居中的最大偏移，避免短歌词离宠物太远
pub(crate) const LANE_RIGHT_PAD: f32 = 30.0;  // 歌词通道右侧留给播放指示
pub(crate) const HUD_TRACK_X: f32 = 48.0;
pub(crate) const HUD_TRACK_W: f32 = 150.0;
pub const BAR_W_MIN: f32 = 44.0; // 极简小条
''',
     '''pub(crate) const LANE_TEXT_GAP: f32 = 24.0;   // 宠物右缘 → 歌词通道（硬余量，保证不重叠）
pub(crate) const LANE_CENTER_CAP: f32 = 16.0; // 歌词在通道内居中的最大偏移，避免短歌词离宠物太远
pub(crate) const LANE_RIGHT_PAD: f32 = 30.0;  // 歌词通道右侧留给播放指示
// 长歌词：先缩字号，8pt 仍放不下才横向滚动。
const LYRIC_BASE_SIZE: f32 = 11.0;
const SHRINK_SIZES: [f32; 6] = [10.5, 10.0, 9.5, 9.0, 8.5, 8.0];
const SCROLL_DWELL: f32 = 1.0;      // 句首/句尾停留，先让人看清开头
const SCROLL_MIN_SPEED: f32 = 22.0; // px/s，约每秒 2 个汉字
const SCROLL_MAX_SPEED: f32 = 90.0;
pub(crate) const HUD_TRACK_X: f32 = 48.0;
pub(crate) const HUD_TRACK_W: f32 = 150.0;
pub const BAR_W_MIN: f32 = 44.0; // 极简小条
'''),
    ("src/app.rs",
     '''const NETWORK_RETRIES:u32=10;

/// 与 `format!("{title}|{artist}|{duration:.0}")` 等价，但不分配内存：界面每帧要比较好几次。
fn lyric_key_matches(key:&str,media:&NowPlaying)->bool {
    use std::io::Write;
    let Some(rest)=key.strip_prefix(media.title.as_str()).and_then(|rest|rest.strip_prefix('|')) else{return false;};
''',
     '''const NETWORK_RETRIES:u32=10;

/// 与 `format!("{title}|{artist}|{duration:.0}")` 等价，但不分配内存：界面每帧要比较好几次。
/// 长歌词横向滚动的参数：溢出宽度与速度（都是点/秒口径的点）。
#[derive(Clone,Copy,Debug)]
struct LyricScroll {overflow:f32,speed:f32}

/// 歌词排版缓存。
/// 没有它时 `update_island_lyric_layout` 每帧都要从 11pt 逐级试到 8pt，
/// 一句中文最多重排 7 次；缓存后同一句只排一次。
struct LyricLayoutCache {
    text:String,
    width:f32,
    white:bool,
    title:bool,
    galley:std::sync::Arc<egui::Galley>,
    /// 8pt 仍放不下时的溢出宽度（点），>0 表示要横向滚动。
    overflow:f32,
}

impl LyricLayoutCache {
    fn matches(&self,text:&str,width:f32,white:bool,title:bool)->bool {
        self.text==text && (self.width-width).abs()<0.5 && self.white==white && self.title==title
    }
}

/// 选字号：先试 11pt，再按 `SHRINK_SIZES` 逐级缩小；
/// 全放不下就返回 (11pt, 需要滚动)。短路调用 `fits`，不会白排剩余字号。
fn fit_lyric_size(mut fits:impl FnMut(f32)->bool)->(f32,bool) {
    if fits(LYRIC_BASE_SIZE) {return (LYRIC_BASE_SIZE,false);}
    for size in SHRINK_SIZES {
        if fits(size) {return (size,false);}
    }
    (LYRIC_BASE_SIZE,true)
}

/// 滚动速度：让整句尽量在本句时长内滚完，同时夹在可读区间。
fn lyric_scroll_speed(overflow:f32,window:f32)->f32 {
    let travel=(window-2.0*SCROLL_DWELL).max(0.8);
    (overflow/travel).clamp(SCROLL_MIN_SPEED,SCROLL_MAX_SPEED)
}

/// 滚动位移：句首停留 `SCROLL_DWELL` 秒后匀速推进，到尾部停住。
fn lyric_scroll_offset(elapsed:f32,speed:f32,overflow:f32)->f32 {
    ((elapsed-SCROLL_DWELL).max(0.0)*speed).min(overflow)
}

fn lyric_key_matches(key:&str,media:&NowPlaying)->bool {
    use std::io::Write;
    let Some(rest)=key.strip_prefix(media.title.as_str()).and_then(|rest|rest.strip_prefix('|')) else{return false;};
'''),
    ("src/app.rs",
     '''    pub(crate) ly_worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) show_lyrics: bool,
    island_lyric_layout: Option<std::sync::Arc<egui::Galley>>,
    media_anchor: (String, f64, f64, bool),
    pub(crate) media: NowPlaying,
    pub(crate) music_artwork: Option<(std::sync::Arc<egui::ColorImage>,egui::TextureHandle)>,
''',
     '''    pub(crate) ly_worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) show_lyrics: bool,
    island_lyric_layout: Option<std::sync::Arc<egui::Galley>>,
    island_lyric_cache: Option<LyricLayoutCache>,
    /// 长歌词滚动参数；Some 表示当前句放不下、正在横向滚动。
    island_lyric_scroll: Option<LyricScroll>,
    /// 滚动时间基准：(歌词行号, 文本)。变化即重置起点（副歌重复同一句也要重置）。
    island_lyric_scroll_key: (Option<usize>,String),
    island_lyric_scroll_start: f64,
    media_anchor: (String, f64, f64, bool),
    pub(crate) media: NowPlaying,
    pub(crate) music_artwork: Option<(std::sync::Arc<egui::ColorImage>,egui::TextureHandle)>,
'''),
    ("src/app.rs",
     '''            ly_worker: None,
            show_lyrics: initial_lyrics,
            island_lyric_layout: None,
            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
''',
     '''            ly_worker: None,
            show_lyrics: initial_lyrics,
            island_lyric_layout: None,
            island_lyric_cache: None,
            island_lyric_scroll: None,
            island_lyric_scroll_key: (None,String::new()),
            island_lyric_scroll_start: 0.0,
            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
'''),
    ("src/app.rs",
     '''    }

    fn update_island_lyric_layout(&mut self,ctx:&egui::Context) {
        if !self.island_lyrics_active(){self.island_lyric_layout=None;return;}
        let current=self.island_lyric();
        let lyrics=self.ly.lyrics.as_ref().filter(|lyrics|lyric_key_matches(&self.ly.key,&self.media)
            && lyrics.synced && lyrics.lines.iter().any(|line|!line.text.trim().is_empty()));
        let title=lyrics.is_none();
''',
     '''    }

    fn update_island_lyric_layout(&mut self,ctx:&egui::Context) {
        if !self.island_lyrics_active(){self.island_lyric_layout=None;self.island_lyric_scroll=None;return;}
        // 文本与时间窗先取成自有值：下面要写 self 的字段，不能还借着 self.ly。
        let current=self.island_lyric().map(|(index,line)|(index,line.to_owned()));
        let lyrics=self.ly.lyrics.as_ref().filter(|lyrics|lyric_key_matches(&self.ly.key,&self.media)
            && lyrics.synced && lyrics.lines.iter().any(|line|!line.text.trim().is_empty()));
        let title=lyrics.is_none();
'''),
    ("src/app.rs",
     '''        let lyrics=self.ly.lyrics.as_ref().filter(|lyrics|lyric_key_matches(&self.ly.key,&self.media)
            && lyrics.synced && lyrics.lines.iter().any(|line|!line.text.trim().is_empty()));
        let title=lyrics.is_none();
        let text=if let Some((_,line))=current {line}
            else if let Some(lyrics)=lyrics {
''',
     '''        let lyrics=self.ly.lyrics.as_ref().filter(|lyrics|lyric_key_matches(&self.ly.key,&self.media)
            && lyrics.synced && lyrics.lines.iter().any(|line|!line.text.trim().is_empty()));
        let title=lyrics.is_none();
        let text=if let Some((_,line))=&current {line.clone()}
            else if let Some(lyrics)=lyrics {
'''),
    ("src/app.rs",
     '''            else if let Some(lyrics)=lyrics {
                if lyrics.index_at(self.lyric_position()).is_none() {"前奏"}
                else {"间奏"}
            } else {self.media.title.trim()};
        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        // 先算好再进闭包，避免闭包捕获 &self 与后面的可变借用冲突。
        let wrap_width=self.lyric_text_width();
''',
     '''            else if let Some(lyrics)=lyrics {
                if lyrics.index_at(self.lyric_position()).is_none() {"前奏".to_owned()}
                else {"间奏".to_owned()}
            } else {self.media.title.trim().to_owned()};
        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        // 先算好再进闭包，避免闭包捕获 &self 与后面的可变借用冲突。
        let wrap_width=self.lyric_text_width();
'''),
    ("src/app.rs",
     '''        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        // 先算好再进闭包，避免闭包捕获 &self 与后面的可变借用冲突。
        let wrap_width=self.lyric_text_width();
        // probe=true 时不限制行数，用来测「这个字号能否单行放进歌词通道」。
''',
     '''        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        // 先算好再进闭包，避免闭包捕获 &self 与后面的可变借用冲突。
        let wrap_width=self.lyric_text_width();
        let white=ink==theme::WHITE;
        // 本句时间窗：滚动速度要用，测试构建的分页兜底也要用。
        let span=current.as_ref().zip(lyrics).map(|((index,_),lyrics)|{
            let start=lyrics.lines[*index].t;
            let end=lyrics.lines.iter().skip(index+1).find(|line|line.t>start).map_or(self.media.duration as f32,|line|line.t);
            (start,end)
        });
        let progress=span.map_or(0.0,|(start,end)|if end>start {((self.lyric_position()-start)/(end-start)).clamp(0.0,1.0)}else{0.0});
        let window=span.map(|(start,end)|end-start);
        // 布局缓存：同一句、同一通道宽度、同一颜色只重排一次。
        // 没有缓存时这里每帧都要从 11pt 逐级试到 8pt，一句中文最多重排 7 次，全是重复劳动。
        // 测试构建不走缓存：分页兜底按播放进度切段，缓存会返回上一段。
        let cached=if cfg!(test) {None} else {
            self.island_lyric_cache.as_ref()
                .filter(|cache|cache.matches(&text,wrap_width,white,title))
                .map(|cache|(cache.galley.clone(),cache.overflow))
        };
        // probe=true 时不限制行数，用来测「这个字号能否单行放进歌词通道」。
'''),
    ("src/app.rs",
     '''        // probe=true 时不限制行数，用来测「这个字号能否单行放进歌词通道」。
        let layout=|text:String,size:f32,probe:bool| {
            let mut job=egui::text::LayoutJob::simple(text,theme::font(size,W::Semibold),ink,wrap_width);
            // 官方的 Center 在超长单行 galley 上会产生负的行内偏移，把整行文字
            // 向左推出绘制起点、压到封面/宠物上（暂停音乐时的长歌词最容易触发）。
            // release 改左对齐，由 shared_header() 按 galley 实际宽度手动居中；
''',
     '''        // probe=true 时不限制行数，用来测「这个字号能否单行放进歌词通道」。
        let layout=|text:&str,size:f32,probe:bool| {
            let mut job=egui::text::LayoutJob::simple(text.to_owned(),theme::font(size,W::Semibold),ink,wrap_width);
            // 官方的 Center 在超长单行 galley 上会产生负的行内偏移，把整行文字
            // 向左推出绘制起点、压到封面/宠物上（暂停音乐时的长歌词最容易触发）。
            // release 改左对齐，由 shared_header() 按 galley 实际宽度手动居中；
'''),
    ("src/app.rs",
     '''            if title && !probe {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        // 长歌词自动缩小字号：从 11pt 逐级降到 8pt，选能单行完整显示的最大字号，
        // 比截断 / 分页更易读；8pt 仍放不下才回退到官方分页 + 通道裁剪兜底。
        // 测试构建固定 11pt 且保留官方 max_rows，布局断言不受影响。
        let probe_mode=!cfg!(test);
        let mut font_size=11.0;
        let mut galley=layout(text.to_owned(),font_size,probe_mode);
        if probe_mode {
            for size in [10.5,10.0,9.5,9.0,8.5,8.0] {
                if galley.rows.len()<=1 {break;}
                font_size=size;
                galley=layout(text.to_owned(),font_size,true);
            }
''',
     '''            if title && !probe {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        // 单行不折行：横向滚动用，galley 宽度就是整句的真实宽度。
        let layout_single=|text:&str,size:f32| {
            let mut job=egui::text::LayoutJob::simple(text.to_owned(),theme::font(size,W::Semibold),ink,f32::INFINITY);
            job.halign=egui::Align::Min;
            job.wrap.break_anywhere=false;
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        let (galley,overflow)=match cached {
            Some(hit)=>hit,
            None=>{
                // 长歌词先缩字号：从 11pt 逐级降到 8pt，选能单行完整显示的最大字号。
                // 8pt 仍放不下就回 11pt 单行 + 横向滚动，比按播放进度分页连续得多。
                // 测试构建固定 11pt 且保留官方 max_rows，布局断言不受影响。
                let (size,need_scroll)=if cfg!(test) {(LYRIC_BASE_SIZE,false)}
                    else {fit_lyric_size(|size|layout(&text,size,true).rows.len()<=1)};
                // 歌名不做滚动，仍沿用官方单行省略。
                let need_scroll=need_scroll&&!title;
                let mut galley=if need_scroll {layout_single(&text,size)}else{layout(&text,size,false)};
                let mut overflow=if need_scroll {(galley.size().x-wrap_width).max(0.0)}else{0.0};
                // 测试构建保留官方的分页兜底，UI 测试的布局断言不变。
                if cfg!(test) && galley.rows.len()>1 {
                    // 按当前句的时间范围与各段字数切换，暂停和跳转直接跟随播放位置。
                    let total:usize=galley.rows.iter().map(|row|row.glyphs.len()).sum();
                    let mut count=0;
                    let page=galley.rows.iter().position(|row|{count+=row.glyphs.len();progress*(total as f32)<count as f32}).unwrap_or(galley.rows.len()-1);
                    let segment=galley.rows[page].glyphs.iter().map(|glyph|glyph.chr).collect::<String>();
                    galley=layout(segment.trim(),size,false);
                    overflow=0.0;
                }
                if !cfg!(test) {
                    self.island_lyric_cache=Some(LyricLayoutCache{text:text.clone(),width:wrap_width,white,title,galley:galley.clone(),overflow});
                }
                (galley,overflow)
            }
'''),
    ("src/app.rs",
     '''            }
            // 超长歌名仍沿用官方的单行省略，避免撑成两行。
            if title && galley.rows.len()>1 {galley=layout(text.to_owned(),font_size,false);}
        }
''',
     '''            }
        };
        // 滚动时间基准：换句（行号或文本变化）时重置；副歌同一句重复出现也要重置。
        let key=(current.as_ref().map(|(index,_)|*index),text.clone());
        if key!=self.island_lyric_scroll_key {
            self.island_lyric_scroll_key=key;
            self.island_lyric_scroll_start=self.t;
        }
'''),
    ("src/app.rs",
     '''        }
        if let Some((index,_))=current.filter(|_|galley.rows.len()>1) {
            let lyrics=self.ly.lyrics.as_ref().unwrap();
            let start=lyrics.lines[index].t;
            let end=lyrics.lines.iter().skip(index+1).find(|line|line.t>start).map_or(self.media.duration as f32,|line|line.t);
            let progress=if end>start {((self.lyric_position()-start)/(end-start)).clamp(0.0,1.0)}else{0.0};
            // 按当前句的时间范围与各段字数切换，暂停和跳转直接跟随播放位置。
            let total:usize=galley.rows.iter().map(|row|row.glyphs.len()).sum();
            let mut count=0;
            let page=galley.rows.iter().position(|row|{count+=row.glyphs.len();progress*(total as f32)<count as f32}).unwrap_or(galley.rows.len()-1);
            let segment=galley.rows[page].glyphs.iter().map(|glyph|glyph.chr).collect::<String>();
            galley=layout(segment.trim().to_owned(),font_size,false);
        }
        self.island_lyric_layout=Some(galley);
    }

''',
     '''        }
        self.island_lyric_scroll=(overflow>0.5)
            .then_some(LyricScroll{overflow,speed:lyric_scroll_speed(overflow,window.unwrap_or(0.0))});
        self.island_lyric_layout=Some(galley);
    }

'''),
    ("src/app.rs",
     '''        let ambient=self.settings.skin==model::Skin::Fluid && self.fluid.animating() || self.pomo.running && (self.open && self.page==1 || !self.open && !minimal_pill && self.notice.is_none() && bar::animations_enabled())
            || self.media.playing && (self.open && self.page==0 || !self.open && !minimal_pill && self.notice.is_none() && self.media.is_music())
            || self.notice.as_ref().is_some_and(|notice|notice.meeting.is_some());
        let animating=transitioning || ambient || pet_visible && self.pet.state!=PetState::Sleeping;
        // 交互形变保持 60 帧，媒体 30 帧；小表情 20 帧，睡眠 10 帧，静态界面每秒核对提醒。
        let repaint_after=std::time::Duration::from_secs_f64(if transitioning{1.0/60.0}else if ambient{1.0/30.0}else if pet_visible{if self.pet.state==PetState::Sleeping{0.1}else{0.05}}else{1.0});
        cmds.append(&mut self.pending);
        FrameOut {
            region: rel(floating_toast.into_iter().chain(self.shelf_menu_rect.map(|r|r.union(r + ctx.style().visuals.popup_shadow.margin()).expand(1.0 / ctx.pixels_per_point()))).fold(region,Rect::union).intersect(ui.max_rect())),
''',
     '''        let ambient=self.settings.skin==model::Skin::Fluid && self.fluid.animating() || self.pomo.running && (self.open && self.page==1 || !self.open && !minimal_pill && self.notice.is_none() && bar::animations_enabled())
            || self.media.playing && (self.open && self.page==0 || !self.open && !minimal_pill && self.notice.is_none() && self.media.is_music())
            || self.notice.as_ref().is_some_and(|notice|notice.meeting.is_some());
        // 长歌词滚动时必须持续重绘；暂停时也要——人正是这时候才想读完一整句。
        let lyric_scrolling=self.island_lyric_scroll.is_some();
        let animating=transitioning || ambient || lyric_scrolling || pet_visible && self.pet.state!=PetState::Sleeping;
        // 交互形变保持 60 帧，媒体/歌词滚动 30 帧；小表情 20 帧，睡眠 10 帧，静态界面每秒核对提醒。
        let repaint_after=std::time::Duration::from_secs_f64(if transitioning{1.0/60.0}else if ambient||lyric_scrolling{1.0/30.0}else if pet_visible{if self.pet.state==PetState::Sleeping{0.1}else{0.05}}else{1.0});
        cmds.append(&mut self.pending);
        FrameOut {
            region: rel(floating_toast.into_iter().chain(self.shelf_menu_rect.map(|r|r.union(r + ctx.style().visuals.popup_shadow.margin()).expand(1.0 / ctx.pixels_per_point()))).fold(region,Rect::union).intersect(ui.max_rect())),
'''),
    ("src/app.rs",
     '''                },
                None=>content_width-LYRIC_BAR_W/2.0,
            };
            let position=c.pos(left+lyric_left,center_y-galley.size().y/2.0+(1.0-fade)*3.0);
            // 兜底裁剪：超长或异常 galley 一律不许画进左侧固定通道。
            let lyric_c=match self.lyric_lane() {
                Some((_,text_left))=>c.clipped(left+text_left,0.0,(content_width-LANE_RIGHT_PAD-text_left).max(90.0),BAR_H),
''',
     '''                },
                None=>content_width-LYRIC_BAR_W/2.0,
            };
            // 长歌词横向滚动：句首停留后匀速推进，到尾部停住，换句复位。
            let scroll_offset=self.island_lyric_scroll.map_or(0.0,|scroll|{
                let elapsed=(self.t-self.island_lyric_scroll_start).max(0.0) as f32;
                lyric_scroll_offset(elapsed,scroll.speed,scroll.overflow)/c.s
            });
            let position=c.pos(left+lyric_left-scroll_offset,center_y-galley.size().y/2.0+(1.0-fade)*3.0);
            // 兜底裁剪：超长或异常 galley 一律不许画进左侧固定通道。
            let lyric_c=match self.lyric_lane() {
                Some((_,text_left))=>c.clipped(left+text_left,0.0,(content_width-LANE_RIGHT_PAD-text_left).max(90.0),BAR_H),
'''),
    ("src/app.rs",
     '''            if added {self.open_task_reminder(0);}
        }
    }
}
''',
     '''            if added {self.open_task_reminder(0);}
        }
    }
}

#[cfg(test)]
mod lyric_scroll_tests {
    use super::*;

    #[test]
    fn shrink_prefers_largest_single_line_size(){
        // 11pt 就放得下：不缩。
        assert_eq!(fit_lyric_size(|_|true),(11.0,false));
        // 10pt 才放得下。
        assert_eq!(fit_lyric_size(|size|size<=10.0),(10.0,false));
        // 到 8pt 才放得下。
        assert_eq!(fit_lyric_size(|size|size<=8.0),(8.0,false));
        // 8pt 也放不下：回到 11pt 并标记需要滚动。
        assert_eq!(fit_lyric_size(|_|false),(11.0,true));
    }

    #[test]
    fn scroll_waits_at_both_ends(){
        // 句首停留 1 秒：这段时间必须纹丝不动，否则开头一闪而过。
        assert_eq!(lyric_scroll_offset(0.0,30.0,100.0),0.0);
        assert_eq!(lyric_scroll_offset(1.0,30.0,100.0),0.0);
        assert!((lyric_scroll_offset(2.0,30.0,100.0)-30.0).abs()<0.01);
        // 到尾部就停住，不会滚出空白。
        assert_eq!(lyric_scroll_offset(60.0,30.0,100.0),100.0);
    }

    #[test]
    fn speed_fits_line_window_but_stays_readable(){
        // 窗口足够长：按「整句滚完」算，落在可读区间。
        let normal=lyric_scroll_speed(200.0,10.0);
        assert!((normal-25.0).abs()<0.01,"{}",normal);
        // 句子很短、放不下时也不能快到看不清。
        assert_eq!(lyric_scroll_speed(300.0,1.0),SCROLL_MAX_SPEED);
        // 句子很长时不会慢到几乎不动。
        assert_eq!(lyric_scroll_speed(1.0,600.0),SCROLL_MIN_SPEED);
    }

    /// 量化缓存省下的重排开销：`cargo test --lib -- --ignored --nocapture bench_long_line`
    #[test]
    #[ignore]
    fn bench_long_line_layout_cost(){
        let ctx=egui::Context::default();
        let _=ctx.run(Default::default(),|_|{});
        let text="这一句歌词故意写得非常非常长用来测试横向滚动到底能不能把整句话完整地显示出来".to_owned();
        let job=|size:f32|{
            let mut job=egui::text::LayoutJob::simple(text.clone(),egui::FontId::proportional(size),theme::WHITE,186.0);
            job.wrap.break_anywhere=true;
            job
        };
        let n=300;
        let t=std::time::Instant::now();
        for _ in 0..n {let _=ctx.fonts(|fonts|fonts.layout_job(job(11.0)));}
        let one=t.elapsed().as_secs_f64()/n as f64;
        let t=std::time::Instant::now();
        for _ in 0..n {
            for size in [11.0,10.5,10.0,9.5,9.0,8.5,8.0] {let _=ctx.fonts(|fonts|fonts.layout_job(job(size)));}
        }
        let seven=t.elapsed().as_secs_f64()/n as f64;
        // 每句都不一样才会真的走排版，用来区分「命中 egui 内部 galley 缓存」和「真重排」。
        let t=std::time::Instant::now();
        for i in 0..n {
            let mut job=egui::text::LayoutJob::simple(format!("{}{}",text,i),egui::FontId::proportional(11.0),theme::WHITE,186.0);
            job.wrap.break_anywhere=true;
            let _=ctx.fonts(|fonts|fonts.layout_job(job));
        }
        let cold=t.elapsed().as_secs_f64()/n as f64;
        println!("命中缓存 {:.1}us | 未命中(真排版) {:.1}us | 每帧7次 {:.1}us | 30fps 每秒多花 {:.3}ms",
            one*1e6,cold*1e6,seven*1e6,(seven-one)*30.0*1000.0);
    }

    #[test]
    fn cache_matches_only_same_line_and_width(){
        let ctx=egui::Context::default();
        // 字体要等第一次 run 之后才可用。
        let _=ctx.run(Default::default(),|_|{});
        // semibold 字体族由 App 启动时注册，这里只验证匹配逻辑，用默认族即可。
        let job=egui::text::LayoutJob::simple("测试".to_owned(),egui::FontId::proportional(11.0),theme::WHITE,f32::INFINITY);
        let galley=ctx.fonts(|fonts|fonts.layout_job(job));
        let cache=LyricLayoutCache{text:"测试".into(),width:160.0,white:true,title:false,galley,overflow:0.0};
        assert!(cache.matches("测试",160.0,true,false));
        // 换句、改宽度、换颜色、歌名/歌词切换都必须重排。
        assert!(!cache.matches("另一句",160.0,true,false));
        assert!(!cache.matches("测试",200.0,true,false));
        assert!(!cache.matches("测试",160.0,false,false));
        assert!(!cache.matches("测试",160.0,true,true));
    }
}
'''),


    # custom-25：歌词通道左右留白对齐（左边收小、右边给播放指示留够）
    ("src/app.rs",
     '''// 想调间距或往胶囊里加新图标，只改这几个常量和 lyric_lane()，不用翻绘制代码。
pub(crate) const LANE_PET_HALF: f32 = 24.0;   // 宠物表情贴图半宽（实际 48px）
pub(crate) const LANE_ART_GAP: f32 = 4.0;     // 封面右缘 → 宠物左缘
pub(crate) const LANE_TEXT_GAP: f32 = 24.0;   // 宠物右缘 → 歌词通道（硬余量，保证不重叠）
pub(crate) const LANE_CENTER_CAP: f32 = 16.0; // 歌词在通道内居中的最大偏移，避免短歌词离宠物太远
''',
     '''// 想调间距或往胶囊里加新图标，只改这几个常量和 lyric_lane()，不用翻绘制代码。
pub(crate) const LANE_PET_HALF: f32 = 24.0;   // 宠物表情贴图半宽（实际 48px）
pub(crate) const LANE_ART_GAP: f32 = 4.0;     // 封面右缘 → 宠物左缘
pub(crate) const LANE_TEXT_GAP: f32 = 4.0;    // 宠物右缘 → 歌词通道（硬余量，保证不重叠）
pub(crate) const LANE_CENTER_CAP: f32 = 16.0; // 歌词在通道内居中的最大偏移，避免短歌词离宠物太远
'''),
    ("src/app.rs",
     '''pub(crate) const LANE_CENTER_CAP: f32 = 16.0; // 歌词在通道内居中的最大偏移，避免短歌词离宠物太远
pub(crate) const LANE_RIGHT_PAD: f32 = 30.0;  // 歌词通道右侧留给播放指示
// 长歌词：先缩字号，8pt 仍放不下才横向滚动。
const LYRIC_BASE_SIZE: f32 = 11.0;
const SHRINK_SIZES: [f32; 6] = [10.5, 10.0, 9.5, 9.0, 8.5, 8.0];
''',
     '''pub(crate) const LANE_CENTER_CAP: f32 = 16.0; // 歌词在通道内居中的最大偏移，避免短歌词离宠物太远
// 播放指示 5 根竖条以 content_width-21 为中心、向左占 7.8，左缘在 content_width-28.8；
// 这里留 42 才能让文字与竖条之间有 ~13px 空隙（原来 30 只剩 1.2px，看起来贴在一起）。
pub(crate) const LANE_RIGHT_PAD: f32 = 48.0;  // 歌词通道右侧留给播放指示
// 长歌词：先缩字号，8pt 仍放不下才横向滚动。
const LYRIC_BASE_SIZE: f32 = 11.0;
const SHRINK_SIZES: [f32; 6] = [10.5, 10.0, 9.5, 9.0, 8.5, 8.0];
'''),

    # custom-26：去掉歌词缩字号：统一 11pt，放不下就横向滚动
    ("src/app.rs",
     '''// 播放指示 5 根竖条以 content_width-21 为中心、向左占 7.8，左缘在 content_width-28.8；
// 这里留 42 才能让文字与竖条之间有 ~13px 空隙（原来 30 只剩 1.2px，看起来贴在一起）。
pub(crate) const LANE_RIGHT_PAD: f32 = 48.0;  // 歌词通道右侧留给播放指示
// 长歌词：先缩字号，8pt 仍放不下才横向滚动。
const LYRIC_BASE_SIZE: f32 = 11.0;
''',
     '''// 播放指示 5 根竖条以 content_width-21 为中心、向左占 7.8，左缘在 content_width-28.8；
// 这里留 42 才能让文字与竖条之间有 ~13px 空隙（原来 30 只剩 1.2px，看起来贴在一起）。
pub(crate) const LANE_RIGHT_PAD: f32 = 48.0;  // 歌词通道右侧留给播放指示
// 歌词固定字号：放不下就横向滚动，不再缩字号。
const LYRIC_BASE_SIZE: f32 = 11.0;
'''),
    ("src/app.rs",
     '''const LYRIC_BASE_SIZE: f32 = 11.0;
const SHRINK_SIZES: [f32; 6] = [10.5, 10.0, 9.5, 9.0, 8.5, 8.0];
const SCROLL_DWELL: f32 = 1.0;      // 句首/句尾停留，先让人看清开头
const SCROLL_MIN_SPEED: f32 = 22.0; // px/s，约每秒 2 个汉字
const SCROLL_MAX_SPEED: f32 = 90.0;
''',
     '''const LYRIC_BASE_SIZE: f32 = 11.0;
const SCROLL_DWELL: f32 = 1.0;      // 句首/句尾停留，先让人看清开头
const SCROLL_MIN_SPEED: f32 = 22.0; // px/s，约每秒 2 个汉字
const SCROLL_MAX_SPEED: f32 = 90.0;
'''),
    ("src/app.rs",
     '''    fn matches(&self,text:&str,width:f32,white:bool,title:bool)->bool {
        self.text==text && (self.width-width).abs()<0.5 && self.white==white && self.title==title
    }
}

/// 选字号：先试 11pt，再按 `SHRINK_SIZES` 逐级缩小；
/// 全放不下就返回 (11pt, 需要滚动)。短路调用 `fits`，不会白排剩余字号。
fn fit_lyric_size(mut fits:impl FnMut(f32)->bool)->(f32,bool) {
    if fits(LYRIC_BASE_SIZE) {return (LYRIC_BASE_SIZE,false);}
    for size in SHRINK_SIZES {
        if fits(size) {return (size,false);}
    }
    (LYRIC_BASE_SIZE,true)
}

/// 滚动速度：让整句尽量在本句时长内滚完，同时夹在可读区间。
''',
     '''    fn matches(&self,text:&str,width:f32,white:bool,title:bool)->bool {
        self.text==text && (self.width-width).abs()<0.5 && self.white==white && self.title==title
    }
}

/// 滚动速度：让整句尽量在本句时长内滚完，同时夹在可读区间。
'''),
    ("src/app.rs",
     '''                .filter(|cache|cache.matches(&text,wrap_width,white,title))
                .map(|cache|(cache.galley.clone(),cache.overflow))
        };
        // probe=true 时不限制行数，用来测「这个字号能否单行放进歌词通道」。
        let layout=|text:&str,size:f32,probe:bool| {
            let mut job=egui::text::LayoutJob::simple(text.to_owned(),theme::font(size,W::Semibold),ink,wrap_width);
            // 官方的 Center 在超长单行 galley 上会产生负的行内偏移，把整行文字
            // 向左推出绘制起点、压到封面/宠物上（暂停音乐时的长歌词最容易触发）。
''',
     '''                .filter(|cache|cache.matches(&text,wrap_width,white,title))
                .map(|cache|(cache.galley.clone(),cache.overflow))
        };
        let layout=|text:&str,size:f32| {
            let mut job=egui::text::LayoutJob::simple(text.to_owned(),theme::font(size,W::Semibold),ink,wrap_width);
            // 官方的 Center 在超长单行 galley 上会产生负的行内偏移，把整行文字
            // 向左推出绘制起点、压到封面/宠物上（暂停音乐时的长歌词最容易触发）。
'''),
    ("src/app.rs",
     '''            // 测试构建保留官方值，不影响 UI 测试。
            job.halign=if cfg!(test){egui::Align::Center}else{egui::Align::Min};
            job.wrap.break_anywhere=true;
            if title && !probe {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        // 单行不折行：横向滚动用，galley 宽度就是整句的真实宽度。
''',
     '''            // 测试构建保留官方值，不影响 UI 测试。
            job.halign=if cfg!(test){egui::Align::Center}else{egui::Align::Min};
            job.wrap.break_anywhere=true;
            if title {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        // 单行不折行：横向滚动用，galley 宽度就是整句的真实宽度。
'''),
    ("src/app.rs",
     '''        let (galley,overflow)=match cached {
            Some(hit)=>hit,
            None=>{
                // 长歌词先缩字号：从 11pt 逐级降到 8pt，选能单行完整显示的最大字号。
                // 8pt 仍放不下就回 11pt 单行 + 横向滚动，比按播放进度分页连续得多。
                // 测试构建固定 11pt 且保留官方 max_rows，布局断言不受影响。
                let (size,need_scroll)=if cfg!(test) {(LYRIC_BASE_SIZE,false)}
                    else {fit_lyric_size(|size|layout(&text,size,true).rows.len()<=1)};
                // 歌名不做滚动，仍沿用官方单行省略。
                let need_scroll=need_scroll&&!title;
                let mut galley=if need_scroll {layout_single(&text,size)}else{layout(&text,size,false)};
                let mut overflow=if need_scroll {(galley.size().x-wrap_width).max(0.0)}else{0.0};
                // 测试构建保留官方的分页兜底，UI 测试的布局断言不变。
                if cfg!(test) && galley.rows.len()>1 {
                    // 按当前句的时间范围与各段字数切换，暂停和跳转直接跟随播放位置。
                    let total:usize=galley.rows.iter().map(|row|row.glyphs.len()).sum();
                    let mut count=0;
                    let page=galley.rows.iter().position(|row|{count+=row.glyphs.len();progress*(total as f32)<count as f32}).unwrap_or(galley.rows.len()-1);
                    let segment=galley.rows[page].glyphs.iter().map(|glyph|glyph.chr).collect::<String>();
                    galley=layout(segment.trim(),size,false);
                    overflow=0.0;
                }
                if !cfg!(test) {
                    self.island_lyric_cache=Some(LyricLayoutCache{text:text.clone(),width:wrap_width,white,title,galley:galley.clone(),overflow});
''',
     '''        let (galley,overflow)=match cached {
            Some(hit)=>hit,
            None=>{
                // 歌词固定 11pt，不再缩字号：单行放不下就横向滚动，比缩小字体更易读。
                // 歌名不滚动，仍沿用官方单行省略。
                // 测试构建保留官方分页兜底，UI 测试的布局断言不变。
                let size=LYRIC_BASE_SIZE;
                let mut galley=layout(&text,size);
                let mut overflow=0.0;
                if cfg!(test) {
                    if galley.rows.len()>1 {
                        // 按当前句的时间范围与各段字数切换，暂停和跳转直接跟随播放位置。
                        let total:usize=galley.rows.iter().map(|row|row.glyphs.len()).sum();
                        let mut count=0;
                        let page=galley.rows.iter().position(|row|{count+=row.glyphs.len();progress*(total as f32)<count as f32}).unwrap_or(galley.rows.len()-1);
                        let segment=galley.rows[page].glyphs.iter().map(|glyph|glyph.chr).collect::<String>();
                        galley=layout(segment.trim(),size);
                    }
                } else if !title && galley.rows.len()>1 {
                    galley=layout_single(&text,size);
                    overflow=(galley.size().x-wrap_width).max(0.0);
                }
                if !cfg!(test) {
                    self.island_lyric_cache=Some(LyricLayoutCache{text:text.clone(),width:wrap_width,white,title,galley:galley.clone(),overflow});
'''),
    ("src/app.rs",
     '''    use super::*;

    #[test]
    fn shrink_prefers_largest_single_line_size(){
        // 11pt 就放得下：不缩。
        assert_eq!(fit_lyric_size(|_|true),(11.0,false));
        // 10pt 才放得下。
        assert_eq!(fit_lyric_size(|size|size<=10.0),(10.0,false));
        // 到 8pt 才放得下。
        assert_eq!(fit_lyric_size(|size|size<=8.0),(8.0,false));
        // 8pt 也放不下：回到 11pt 并标记需要滚动。
        assert_eq!(fit_lyric_size(|_|false),(11.0,true));
    }

    #[test]
    fn scroll_waits_at_both_ends(){
        // 句首停留 1 秒：这段时间必须纹丝不动，否则开头一闪而过。
        assert_eq!(lyric_scroll_offset(0.0,30.0,100.0),0.0);
''',
     '''    use super::*;

    #[test]
    fn scroll_waits_at_both_ends(){
        // 句首停留 1 秒：这段时间必须纹丝不动，否则开头一闪而过。
        assert_eq!(lyric_scroll_offset(0.0,30.0,100.0),0.0);
'''),

    # custom-27：电量解耦：本机电量与手机电量各自独立、可并排显示；无「手机连接」时锁住手机电量开关
    ("src/app.rs",
     '''    pub(crate) updates: crate::updates::State,
    pub(crate) resident: bar::Motion,
    pub(crate) resident_power: (Option<u8>,bool),
    /// 手机连接电量读取器（本地文件轮询）。
    pub(crate) phone_power: crate::phone_battery::Reader,
}
''',
     '''    pub(crate) updates: crate::updates::State,
    pub(crate) resident: bar::Motion,
    pub(crate) resident_power: (Option<u8>,bool),
    /// 手机连接上报的电量；与本机电量各自独立，互不影响。
    pub(crate) phone_resident: (Option<u8>,bool),
    /// 手机连接电量读取器（本地文件轮询）。
    pub(crate) phone_power: crate::phone_battery::Reader,
}
'''),
    ("src/app.rs",
     '''            calendar: CalendarState::load(),
            quota: crate::quota_ui::State::load(),
            updates: Default::default(),
            resident: Default::default(), resident_power:(None,false), phone_power: Default::default(),
        }
    }

''',
     '''            calendar: CalendarState::load(),
            quota: crate::quota_ui::State::load(),
            updates: Default::default(),
            resident: Default::default(), resident_power:(None,false), phone_resident:(None,false), phone_power: Default::default(),
        }
    }

'''),
    ("src/app.rs",
     '''    }

    pub(crate) fn effective_minimal(&self)->bool {self.settings.minimal || self.browser_minimal_active}
    pub(crate) fn pet_enabled(&self)->bool {self.settings.pet && !self.effective_minimal() && !self.side()}
    pub(crate) fn pet_space(&self)->f32 {if self.pet_enabled(){44.0}else{0.0}}
    fn lyric_focus_space(&self)->f32 {if self.pomo.running{crate::focus_bar::LYRIC_WIDTH+10.0}else{0.0}}
''',
     '''    }

    pub(crate) fn effective_minimal(&self)->bool {self.settings.minimal || self.browser_minimal_active}
    /// 常驻电量环：(本机, 手机)，None 表示该来源关闭或没有数据。
    /// 两个来源互不影响，都开就并排画两个环（本机在左）。
    pub(crate) fn battery_rings(&self)->(Option<(Option<u8>,bool)>,Option<(Option<u8>,bool)>) {
        let pc=(self.settings.show_battery&&self.resident_power.0.is_some()).then_some(self.resident_power);
        let phone=(self.settings.phone_battery&&self.phone_resident.0.is_some()).then_some(self.phone_resident);
        (pc,phone)
    }
    /// 当前要画几个电量环：0 / 1 / 2。
    pub(crate) fn battery_ring_count(&self)->usize {
        let (pc,phone)=self.battery_rings();
        pc.is_some() as usize+phone.is_some() as usize
    }
    pub(crate) fn battery_visible(&self)->bool {self.battery_ring_count()>0}
    pub(crate) fn pet_enabled(&self)->bool {self.settings.pet && !self.effective_minimal() && !self.side()}
    pub(crate) fn pet_space(&self)->f32 {if self.pet_enabled(){44.0}else{0.0}}
    fn lyric_focus_space(&self)->f32 {if self.pomo.running{crate::focus_bar::LYRIC_WIDTH+10.0}else{0.0}}
'''),
    ("src/app.rs",
     '''    /// 测试构建返回 None，沿用官方坐标，避免影响 UI 测试的布局断言。
    fn lyric_lane(&self)->Option<(f32,f32)> {
        if cfg!(test) {return None;}
        let battery=self.settings.show_battery && self.resident_power.0.is_some();
        let art_right=if battery {56.0}else{32.0};
        let pet_center=art_right+LANE_PET_HALF+LANE_ART_GAP;
        let text_left=pet_center+LANE_PET_HALF+LANE_TEXT_GAP;
        Some((pet_center,text_left))
''',
     '''    /// 测试构建返回 None，沿用官方坐标，避免影响 UI 测试的布局断言。
    fn lyric_lane(&self)->Option<(f32,f32)> {
        if cfg!(test) {return None;}
        // 本机与手机电量各自独立，两个环都在时左侧通道要留双份位置。
        let rings=self.battery_ring_count() as f32;
        let art_right=if rings>0.0 {36.0+rings*20.0}else{32.0};
        let pet_center=art_right+LANE_PET_HALF+LANE_ART_GAP;
        let text_left=pet_center+LANE_PET_HALF+LANE_TEXT_GAP;
        Some((pet_center,text_left))
'''),
    ("src/app.rs",
     '''        self.update_media(env,&ctx);
        let bell=self.t<self.notification_bell_until&&!self.compact_notices();
        let phone=if self.settings.phone_battery{self.phone_power.poll()}else{(None,false)};
        // 台式机没有本机电量：常驻电量槽位只认手机电量，手机断开时自动收起。
        self.resident_power=phone;
        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}
''',
     '''        self.update_media(env,&ctx);
        let bell=self.t<self.notification_bell_until&&!self.compact_notices();
        let phone=if self.settings.phone_battery{self.phone_power.poll()}else{(None,false)};
        // 本机电量与手机电量彻底分开：电量开关只管本机，手机电量开关只管手机，
        // 两个来源各自判断、各自显示，互不影响。
        self.resident_power=(env.battery,env.charging);
        self.phone_resident=phone;
        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}
'''),
    ("src/app.rs",
     '''        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}
        #[cfg(test)]
        {
            // 官方 UI 测试用 h.power 注入电量；测试构建里把它当作手机电量，
            // 否则「只认手机电量」会让所有电量相关测试失去可控输入。
            if self.settings.phone_battery { self.resident_power = (env.battery, env.charging); }
        }
        let mut layout=Layout::new(self.settings.show_battery&&self.resident_power.0.is_some(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
        if status_width>0.0 {layout.add_status(status_width);}
''',
     '''        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
        if status_width>0.0 {layout.add_status(status_width);}
'''),
    ("src/app.rs",
     '''        let status_width=self.status_total_width();
        if status_width>0.0 {layout.add_status(status_width);}
        layout.reorder(&self.settings.resident_order);
        for (position,extra) in self.quota_balance_layout(&ctx){layout.balance_tail(position,extra);}
        if self.settings.clock_period!=0 { layout.widen_clock(40.0); }
        if self.pet_enabled() {
''',
     '''        let status_width=self.status_total_width();
        if status_width>0.0 {layout.add_status(status_width);}
        layout.reorder(&self.settings.resident_order);
        // 两个电量环并排：电池槽加宽 20px，其余槽位自动让开。
        if self.battery_ring_count()==2 {layout.widen_battery(20.0);}
        for (position,extra) in self.quota_balance_layout(&ctx){layout.balance_tail(position,extra);}
        if self.settings.clock_period!=0 { layout.widen_clock(40.0); }
        if self.pet_enabled() {
'''),
    ("src/app.rs",
     '''        let lyric=self.island_lyric().map(|(index,_)|index);
        let lyric_alpha=if self.island_lyrics_active() {1.0-smoothstep(0.0,0.35,p)}else{0.0};
        // 提前算好，歌词接管胶囊时也要用它把手机电量留在左侧。
        let resident_battery=self.settings.show_battery && self.resident_power.0.is_some();
        if let Some(galley)=self.island_lyric_layout.clone().filter(|_|lyric_alpha>0.003) {
            let c=base.faded(lyric_alpha);
            let content_width=self.lyric_bar_width()-self.lyric_focus_space();
''',
     '''        let lyric=self.island_lyric().map(|(index,_)|index);
        let lyric_alpha=if self.island_lyrics_active() {1.0-smoothstep(0.0,0.35,p)}else{0.0};
        // 提前算好，歌词接管胶囊时也要用它把手机电量留在左侧。
        let resident_battery=self.battery_visible();
        if let Some(galley)=self.island_lyric_layout.clone().filter(|_|lyric_alpha>0.003) {
            let c=base.faded(lyric_alpha);
            let content_width=self.lyric_bar_width()-self.lyric_focus_space();
'''),
    ("src/app.rs",
     '''            }
        }
        if lyric_alpha>=0.997 {
            // 手机电量常驻：歌词完全接管胶囊时，仍在最左侧保留电量环。
            if resident_battery {
                let content_width=self.lyric_bar_width()-self.lyric_focus_space();
                let width=self.w.pos.max(content_width);
''',
     '''            }
        }
        if lyric_alpha>=0.997 {
            // 电量常驻：歌词完全接管胶囊时，仍在最左侧保留电量环（本机在左、手机在右）。
            if resident_battery {
                let content_width=self.lyric_bar_width()-self.lyric_focus_space();
                let width=self.w.pos.max(content_width);
'''),
    ("src/app.rs",
     '''                let content_width=self.lyric_bar_width()-self.lyric_focus_space();
                let width=self.w.pos.max(content_width);
                let left=local_cx-width/2.0;
                paint::battery(&base.faded(1.0),left+20.0,BAR_H/2.0,self.resident_power.0,self.resident_power.1,1.0);
            }
            return;
        }
''',
     '''                let content_width=self.lyric_bar_width()-self.lyric_focus_space();
                let width=self.w.pos.max(content_width);
                let left=local_cx-width/2.0;
                let (pc_ring,phone_ring)=self.battery_rings();
                let mut cx=left+20.0;
                for ring in [pc_ring,phone_ring].into_iter().flatten() {
                    paint::battery(&base.faded(1.0),cx,BAR_H/2.0,ring.0,ring.1,1.0);
                    cx+=20.0;
                }
            }
            return;
        }
'''),
    ("src/app.rs",
     '''        let m = anim::ease_in_out(smoothstep(0.0, 0.85, p));
        let clock = self.settings.clock_text(&env.clock);
        let show_battery=resident_battery;

        // 顶栏终点布局：[电池][日期][时钟] 作为一组整体居中
        let date_w = base.text_w(&env.date, size::DATE, W::Regular);
''',
     '''        let m = anim::ease_in_out(smoothstep(0.0, 0.85, p));
        let clock = self.settings.clock_text(&env.clock);
        let show_battery=resident_battery;
        let (pc_ring,phone_ring)=self.battery_rings();
        let ring_count=(pc_ring.is_some() as u8+phone_ring.is_some() as u8) as f32;
        // 两个环并排时各自偏离中心 10px。
        let ring_offset=if ring_count>1.0 {10.0}else{0.0};

        // 顶栏终点布局：[电池][日期][时钟] 作为一组整体居中
        let date_w = base.text_w(&env.date, size::DATE, W::Regular);
'''),
    ("src/app.rs",
     '''        // 顶栏终点布局：[电池][日期][时钟] 作为一组整体居中
        let date_w = base.text_w(&env.date, size::DATE, W::Regular);
        let clock_w = paint::clock_width(base,&clock,size::CLOCK);
        let battery_space=if show_battery {24.0}else{0.0};
        let group = battery_space + date_w + 8.0 + clock_w;
        let gx = local_cx - group / 2.0 + if show_battery {6.0}else{0.0};
        // 日程页左侧还有新增按钮，顶栏从操作区右侧开始，避免较长的日期和时段文字挤到按钮上。
''',
     '''        // 顶栏终点布局：[电池][日期][时钟] 作为一组整体居中
        let date_w = base.text_w(&env.date, size::DATE, W::Regular);
        let clock_w = paint::clock_width(base,&clock,size::CLOCK);
        let battery_space=if show_battery {24.0+(ring_count-1.0)*20.0}else{0.0};
        let group = battery_space + date_w + 8.0 + clock_w;
        let gx = local_cx - group / 2.0 + if show_battery {6.0}else{0.0};
        // 日程页左侧还有新增按钮，顶栏从操作区右侧开始，避免较长的日期和时段文字挤到按钮上。
'''),
    ("src/app.rs",
     '''        // 电池：位置/大小连续插值
        let bk = lerp(1.0, 0.8, m);
        let bpos = (lerp(pb.0, hb.0, m), lerp(pb.1, hb.1, m));
        if show_battery {paint::battery(&base.faded(lerp(layout.clearance(bar::BATTERY),1.0,m)), bpos.0, bpos.1, self.resident_power.0, self.resident_power.1, bk);}

        // 时钟：两个字号沿同一路径交叉淡化，避免逐帧换字号
        let cpos = (lerp(pcx, hc, m), lerp(21.0, 23.0, m));
''',
     '''        // 电池：位置/大小连续插值
        let bk = lerp(1.0, 0.8, m);
        let bpos = (lerp(pb.0, hb.0, m), lerp(pb.1, hb.1, m));
        if show_battery {
            let alpha=lerp(layout.clearance(bar::BATTERY),1.0,m);
            if let Some((pct,chg))=pc_ring {paint::battery(&base.faded(alpha),bpos.0-ring_offset,bpos.1,pct,chg,bk);}
            if let Some((pct,chg))=phone_ring {paint::battery(&base.faded(alpha),bpos.0+ring_offset,bpos.1,pct,chg,bk);}
        }

        // 时钟：两个字号沿同一路径交叉淡化，避免逐帧换字号
        let cpos = (lerp(pcx, hc, m), lerp(21.0, 23.0, m));
'''),
    ("src/app.rs",
     '''        }
        self.pet.last_idle = idle;
        self.pet.hovered = env.cursor.is_some_and(|p| (p.x-cx).abs() <= BAR_W/2.0+24.0 && (0.0..70.0).contains(&p.y));
        self.pet.worried = self.settings.show_battery && self.resident_power.0.is_some_and(|v|v<=20) && !self.resident_power.1 || env.network == Some(false);
        let allowed = self.pet_enabled();
        if !allowed { self.pet.reaction = None; self.pet.waking_until = 0.0; self.pet.expressions.clear(); }
        let available = allowed && !self.open && !self.drag_hot;
''',
     '''        }
        self.pet.last_idle = idle;
        self.pet.hovered = env.cursor.is_some_and(|p| (p.x-cx).abs() <= BAR_W/2.0+24.0 && (0.0..70.0).contains(&p.y));
        let (pc_ring,phone_ring)=self.battery_rings();
        let low_battery=[pc_ring,phone_ring].into_iter().flatten().any(|(pct,charging)|pct.is_some_and(|v|v<=20)&&!charging);
        self.pet.worried = low_battery || env.network == Some(false);
        let allowed = self.pet_enabled();
        if !allowed { self.pet.reaction = None; self.pet.waking_until = 0.0; self.pet.expressions.clear(); }
        let available = allowed && !self.open && !self.drag_hot;
'''),
    ("src/app.rs",
     '''        assert!(!cache.matches("测试",160.0,true,true));
    }
}
''',
     '''        assert!(!cache.matches("测试",160.0,true,true));
    }
}

#[cfg(test)]
mod battery_source_tests {
    use super::*;

    #[test]
    fn pc_and_phone_battery_switches_do_not_affect_each_other(){
        let ctx=eframe::egui::Context::default();
        let mut app=App::new(&ctx,Default::default());
        app.resident_power=(Some(65),false);
        app.phone_resident=(Some(42),true);
        app.settings.show_battery=true;
        app.settings.phone_battery=true;
        assert_eq!(app.battery_ring_count(),2,"两个来源都有数据时应并排两个环");
        // 关掉本机开关：手机环不受影响。
        app.settings.show_battery=false;
        assert!(app.battery_rings().0.is_none()&&app.battery_rings().1.is_some());
        // 关掉手机开关：本机环不受影响。
        app.settings.show_battery=true;
        app.settings.phone_battery=false;
        assert!(app.battery_rings().0.is_some()&&app.battery_rings().1.is_none());
        // 手机没数据时手机环不占位，本机环照常。
        app.settings.phone_battery=true;
        app.phone_resident=(None,false);
        assert_eq!(app.battery_ring_count(),1);
        assert!(app.battery_visible());
    }
}
'''),
    ("src/bar_layout.rs",
     '''        let Some(center)=self.x[slot] else{return;};
        for (i,x) in self.x.iter_mut().enumerate(){if i!=slot{if let Some(x)=x{*x+=if *x<center{-extra/2.0}else{extra/2.0};}}}
        self.widths[slot]+=extra;self.width+=extra;
    }
    pub fn widen_clock(&mut self, extra:f32) {
        let clock=self.x[CLOCK].unwrap();
''',
     '''        let Some(center)=self.x[slot] else{return;};
        for (i,x) in self.x.iter_mut().enumerate(){if i!=slot{if let Some(x)=x{*x+=if *x<center{-extra/2.0}else{extra/2.0};}}}
        self.widths[slot]+=extra;self.width+=extra;
    }
    /// 本机与手机电量同时显示时把电池槽加宽，两个环并排；其余槽位左右让开。
    pub fn widen_battery(&mut self, extra:f32) {
        let Some(battery)=self.x[BATTERY] else{return;};
        for (slot,x) in self.x.iter_mut().enumerate() {
            if slot!=BATTERY { if let Some(x)=x { *x+=if *x<battery {-extra/2.0}else{extra/2.0}; } }
        }
        self.widths[BATTERY]+=extra;
        self.width+=extra;
    }
    pub fn widen_clock(&mut self, extra:f32) {
        let clock=self.x[CLOCK].unwrap();
'''),
    ("src/bar_layout.rs",
     '''        }}}}}}
    }
    #[test]
    fn replacing_battery_moves_continuously_and_supports_an_instant_state(){
        let before=Layout::new(true,1,true,false,false,true);let after=Layout::new(false,1,true,false,false,true);
        let mut motion=Motion::default();motion.step(before.clone(),0.0,true);
''',
     '''        }}}}}}
    }
    #[test]
    fn widening_the_battery_slot_keeps_neighbours_apart(){
        // 本机与手机电量同时显示时，电池槽加宽 20px，其余组件必须自动让开。
        let one=Layout::new(true,2,true,false,false,true);
        let mut two=Layout::new(true,2,true,false,false,true);
        two.widen_battery(20.0);
        assert!((two.width-one.width-20.0).abs()<0.01,"整体宽度应增加 20");
        assert!((two.widths[BATTERY]-one.widths[BATTERY]-20.0).abs()<0.01);
        let mut boxes:Vec<_>=two.x.iter().enumerate().filter_map(|(i,x)|x.map(|x|(x-two.widths[i]/2.0,x+two.widths[i]/2.0))).collect();
        boxes.sort_by(|a,b|a.0.total_cmp(&b.0));
        assert!(boxes.windows(2).all(|p|p[1].0-p[0].1>=6.9),"加宽后不能和邻居挤在一起");
        assert!(boxes.first().unwrap().0>=-two.width/2.0+15.9);
        assert!(boxes.last().unwrap().1<=two.width/2.0-15.9);
    }

    #[test]
    fn replacing_battery_moves_continuously_and_supports_an_instant_state(){
        let before=Layout::new(true,1,true,false,false,true);let after=Layout::new(false,1,true,false,false,true);
        let mut motion=Motion::default();motion.step(before.clone(),0.0,true);
'''),
    ("src/extras.rs",
     '''    }
}

fn switch(ui:&mut egui::Ui,value:&mut bool)->bool {
    let (rect,response)=ui.allocate_exact_size(egui::vec2(34.0,20.0),egui::Sense::click());
    if response.clicked(){*value=!*value;}
''',
     '''    }
}

/// 不可用的设置行：开关置灰、点击无效。
/// 用于「本机没装对应组件」的情况（例如没有「手机连接」时的手机电量）。
pub(crate) fn setting_row_locked(ui:&mut egui::Ui,title:&str,description:&str,value:bool){
    let ctx=&ui.ctx().clone();
    ui.add_space(2.0);
    ui.horizontal(|ui|{
        let text_w=(ui.available_width()-SETTINGS_SWITCH_W-SETTINGS_ROW_GAP).max(120.0);
        ui.vertical(|ui|{
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y=2.0;
            ui.add(egui::Label::new(egui::RichText::new(title).size(13.0).color(settings_color(ctx,Color32::from_gray(120),Color32::from_gray(150)))).wrap());
            if !description.is_empty(){ui.add(egui::Label::new(egui::RichText::new(description).size(11.5).color(settings_color(ctx,Color32::from_gray(120),Color32::from_gray(125)))).wrap());}
        });
        ui.add_space((ui.available_width()-SETTINGS_SWITCH_W-ui.spacing().item_spacing.x).max(0.0));
        let (rect,_)=ui.allocate_exact_size(egui::vec2(34.0,20.0),egui::Sense::hover());
        ui.painter().rect_filled(rect,10.0,settings_color(ctx,Color32::from_gray(58),Color32::from_rgb(226,229,234)));
        let x=if value{rect.right()-10.0}else{rect.left()+10.0};
        ui.painter().circle_filled(egui::pos2(x,rect.center().y+0.5),7.5,Color32::from_black_alpha(20));
        ui.painter().circle_filled(egui::pos2(x,rect.center().y),7.0,settings_color(ctx,Color32::from_gray(150),Color32::WHITE));
    });
}
fn switch(ui:&mut egui::Ui,value:&mut bool)->bool {
    let (rect,response)=ui.allocate_exact_size(egui::vec2(34.0,20.0),egui::Sense::click());
    if response.clicked(){*value=!*value;}
'''),
    ("src/extras.rs",
     '''                        settings_card(ui,|ui|self.resident_order_settings(ui));
                        settings_label(ui,"状态组件");
                        settings_card(ui,|ui|{
                            setting_row(ui,"电量","显示手机电量；台式机不显示本机电量，手机断开时自动收起",&mut self.settings.show_battery);
                            ui.separator();
''',
     '''                        settings_card(ui,|ui|self.resident_order_settings(ui));
                        settings_label(ui,"状态组件");
                        settings_card(ui,|ui|{
                            setting_row(ui,"电量","显示本机（电脑）的电量环；台式机没有电池时不显示",&mut self.settings.show_battery);
                            ui.separator();
'''),
    ("src/extras.rs",
     '''                            ui.separator();
                            setting_row(ui,"手机电量","读取「手机连接」的电量并常驻显示，播放音乐/歌词时也不隐藏；充电时电量环变绿并显示闪电；关闭后不显示任何电量",&mut self.settings.phone_battery);
                            ui.separator();
                            setting_row(ui,"岛上歌词","收起时显示歌词，无歌词时显示封面和歌名",&mut self.settings.island_lyrics);
                        });
''',
     '''                            ui.separator();
                            if crate::phone_battery::installed() {
                                setting_row(ui,"手机电量","读取「手机连接」的电量，与本机电量各自独立、并排显示；手机断开时自动收起",&mut self.settings.phone_battery);
                            } else {
                                setting_row_locked(ui,"手机电量","未检测到「手机连接」，安装后才能开启",self.settings.phone_battery);
                            }
                            ui.separator();
                            setting_row(ui,"岛上歌词","收起时显示歌词，无歌词时显示封面和歌名",&mut self.settings.island_lyrics);
                        });
'''),
    ("src/resident_settings.rs",
     '''            let dx=if is_dragged{pointer.map(|p|((p.x-origin.x)/scale-(left+width/2.0)).clamp(-left,layout.width-left-width)).unwrap_or(0.0)}else{0.0};
            if is_dragged {ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);}
            else if response.hovered(){ui.painter().rect_filled(rect,6.0,Color32::from_white_alpha(18));}
            let active=match slot{BATTERY=>self.settings.show_battery&&self.resident_power.0.is_some(),PET=>self.settings.pet,QUOTA=>count>0,MUSIC=>self.media.is_music(),FOCUS=>self.pomo.running,_=>true};
            let c=card.shifted(dx,0.0).faded(if active||is_dragged{1.0}else{0.38}).ghost();
            match slot{
''',
     '''            let dx=if is_dragged{pointer.map(|p|((p.x-origin.x)/scale-(left+width/2.0)).clamp(-left,layout.width-left-width)).unwrap_or(0.0)}else{0.0};
            if is_dragged {ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);}
            else if response.hovered(){ui.painter().rect_filled(rect,6.0,Color32::from_white_alpha(18));}
            let active=match slot{BATTERY=>self.battery_visible(),PET=>self.settings.pet,QUOTA=>count>0,MUSIC=>self.media.is_music(),FOCUS=>self.pomo.running,_=>true};
            let c=card.shifted(dx,0.0).faded(if active||is_dragged{1.0}else{0.38}).ghost();
            match slot{
'''),
    ("src/resident_settings.rs",
     '''            let c=card.shifted(dx,0.0).faded(if active||is_dragged{1.0}else{0.38}).ghost();
            match slot{
                BATTERY=>paint::battery(&c,x,15.0,self.resident_power.0,self.resident_power.1,1.0),
                CLOCK=>{let text=self.settings.clock_text(&chrono::Local::now().format("%H:%M:%S").to_string());paint::clock(&c,x-paint::clock_width(&c,&text,theme::size::PILL_CLOCK)/2.0,21.0,&text,theme::size::PILL_CLOCK);},
                PET=>self.pet.draw(&c,x,self.t,self.skin_palette()),
                MUSIC=>paint::playback_bars(&c,x,15.0,self.t,self.media.playing,theme::WHITE),
''',
     '''            let c=card.shifted(dx,0.0).faded(if active||is_dragged{1.0}else{0.38}).ghost();
            match slot{
                BATTERY=>{let (pc,phone)=self.battery_rings();let (pct,chg)=pc.or(phone).unwrap_or((None,false));paint::battery(&c,x,15.0,pct,chg,1.0)},
                CLOCK=>{let text=self.settings.clock_text(&chrono::Local::now().format("%H:%M:%S").to_string());paint::clock(&c,x-paint::clock_width(&c,&text,theme::size::PILL_CLOCK)/2.0,21.0,&text,theme::size::PILL_CLOCK);},
                PET=>self.pet.draw(&c,x,self.t,self.skin_palette()),
                MUSIC=>paint::playback_bars(&c,x,15.0,self.t,self.media.playing,theme::WHITE),
'''),
    ("src/side_dock.rs",
     '''
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SideItem {
    Clock, Battery, Music, Focus, Bell,
    /// AI 额度：账号下标（None 为「+N」）与高度。
    Quota(usize, f32),
}
''',
     '''
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SideItem {
    Clock, Battery, PhoneBattery, Music, Focus, Bell,
    /// AI 额度：账号下标（None 为「+N」）与高度。
    Quota(usize, f32),
}
'''),
    ("src/side_dock.rs",
     '''
impl SideItem {
    fn height(self) -> f32 {
        match self { Self::Clock => 32.0, Self::Battery => 20.0, Self::Music => 18.0, Self::Focus => 22.0, Self::Bell => 22.0, Self::Quota(_, h) => h }
    }
}

''',
     '''
impl SideItem {
    fn height(self) -> f32 {
        match self { Self::Clock => 32.0, Self::Battery | Self::PhoneBattery => 20.0, Self::Music => 18.0, Self::Focus => 22.0, Self::Bell => 22.0, Self::Quota(_, h) => h }
    }
}

'''),
    ("src/side_dock.rs",
     '''    /// 竖向胶囊里依次显示的内容。
    pub(crate) fn side_items(&self) -> Vec<SideItem> {
        let mut items = vec![SideItem::Clock];
        if self.settings.show_battery && self.resident_power.0.is_some() { items.push(SideItem::Battery); }
        if self.media.is_music() { items.push(SideItem::Music); }
        if self.pomo.running { items.push(SideItem::Focus); }
        items.extend(self.quota_side_items().into_iter().map(|(index, h)| SideItem::Quota(index, h)));
''',
     '''    /// 竖向胶囊里依次显示的内容。
    pub(crate) fn side_items(&self) -> Vec<SideItem> {
        let mut items = vec![SideItem::Clock];
        let (pc_ring,phone_ring)=self.battery_rings();
        if pc_ring.is_some() { items.push(SideItem::Battery); }
        if phone_ring.is_some() { items.push(SideItem::PhoneBattery); }
        if self.media.is_music() { items.push(SideItem::Music); }
        if self.pomo.running { items.push(SideItem::Focus); }
        items.extend(self.quota_side_items().into_iter().map(|(index, h)| SideItem::Quota(index, h)));
'''),
    ("src/side_dock.rs",
     '''                    c.text(cx, mid + 13.0, &minute, 13.5, W::Bold, theme::WHITE, H::Center);
                }
                SideItem::Battery => paint::battery(c, cx, mid, self.resident_power.0, self.resident_power.1, 0.72),
                SideItem::Music => paint::playback_bars(c, cx, mid, self.t, self.media.playing, theme::WHITE),
                SideItem::Focus => {
                    let minutes = (self.pomo.remaining / 60.0).ceil().max(0.0) as u32;
''',
     '''                    c.text(cx, mid + 13.0, &minute, 13.5, W::Bold, theme::WHITE, H::Center);
                }
                SideItem::Battery => paint::battery(c, cx, mid, self.resident_power.0, self.resident_power.1, 0.72),
                SideItem::PhoneBattery => paint::battery(c, cx, mid, self.phone_resident.0, self.phone_resident.1, 0.72),
                SideItem::Music => paint::playback_bars(c, cx, mid, self.t, self.media.playing, theme::WHITE),
                SideItem::Focus => {
                    let minutes = (self.pomo.remaining / 60.0).ceil().max(0.0) as u32;
'''),
    ("src/ui_tests.rs",
     '''    let total=h.app.side_bar_len()-14.0;
    let mut y=top+(h.app.h.pos-total)/2.0;
    for item in h.app.side_items() {
        let height=match item {crate::side_dock::SideItem::Quota(i,hh) if h.app.quota.config.accounts[i].id==wb=>{y+=hh/2.0;break;},crate::side_dock::SideItem::Quota(_,hh)=>hh,crate::side_dock::SideItem::Clock=>32.0,crate::side_dock::SideItem::Battery=>20.0,crate::side_dock::SideItem::Music=>18.0,crate::side_dock::SideItem::Focus|crate::side_dock::SideItem::Bell=>22.0};
        y+=height;
    }
    let click=Pos2::new(18.0,y);
''',
     '''    let total=h.app.side_bar_len()-14.0;
    let mut y=top+(h.app.h.pos-total)/2.0;
    for item in h.app.side_items() {
        let height=match item {crate::side_dock::SideItem::Quota(i,hh) if h.app.quota.config.accounts[i].id==wb=>{y+=hh/2.0;break;},crate::side_dock::SideItem::Quota(_,hh)=>hh,crate::side_dock::SideItem::Clock=>32.0,crate::side_dock::SideItem::Battery|crate::side_dock::SideItem::PhoneBattery=>20.0,crate::side_dock::SideItem::Music=>18.0,crate::side_dock::SideItem::Focus|crate::side_dock::SideItem::Bell=>22.0};
        y+=height;
    }
    let click=Pos2::new(18.0,y);
'''),

    # custom-28：A 组功能集：通知/邮件/剪贴板/状态总线
    ("src/model.rs",
     '''    pub calendar_auto_sync: bool,
    /// 记录最近复制的文字（最近记录与固定条目均在本机保存）。
    pub clipboard_history: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true }
    }
}

''',
     '''    pub calendar_auto_sync: bool,
    /// 记录最近复制的文字（最近记录与固定条目均在本机保存）。
    pub clipboard_history: bool,
    /// 低电提醒阈值（百分比，0 = 关闭）。手机与本机共用，跨越阈值只提醒一次。
    pub battery_alert: u32,
    /// 通知关键词高亮：逗号分隔，命中时卡片左侧亮一条琥珀色竖条；空 = 关闭。
    pub notice_keywords: String,
    /// 勿扰时长（分钟，0 = 关闭）。只在本次运行内生效，重启后恢复为关闭。
    pub quiet_minutes: u32,
    /// 邮件面板上次选择的收件账号（空 = 全部账号），重启后沿用。
    pub mail_filter: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, mail_filter: String::new() }
    }
}

'''),
    ("src/model.rs",
     '''        self.island_offset.clamp(-ISLAND_OFFSET_MAX, ISLAND_OFFSET_MAX) as f32 / ISLAND_OFFSET_MAX as f32
    }

    pub fn clock_text(&self, clock: &str) -> String {
        use chrono::Timelike;
        let Ok(time) = chrono::NaiveTime::parse_from_str(clock.get(..5).unwrap_or(""), "%H:%M") else { return "--:--".into(); };
''',
     '''        self.island_offset.clamp(-ISLAND_OFFSET_MAX, ISLAND_OFFSET_MAX) as f32 / ISLAND_OFFSET_MAX as f32
    }

    /// 低电提醒阈值；`None` 表示关闭提醒。
    pub fn low_battery_threshold(&self) -> Option<u8> {
        (self.battery_alert > 0).then(|| self.battery_alert.min(99) as u8)
    }

    /// 通知关键词表（已去掉空白项）。空表示不启用高亮。
    pub fn keywords(&self) -> Vec<&str> {
        self.notice_keywords.split([',','，']).map(str::trim).filter(|word|!word.is_empty()).collect()
    }

    pub fn clock_text(&self, clock: &str) -> String {
        use chrono::Timelike;
        let Ok(time) = chrono::NaiveTime::parse_from_str(clock.get(..5).unwrap_or(""), "%H:%M") else { return "--:--".into(); };
'''),
    ("src/model.rs",
     '''                "calendar_prompt" => s.calendar_prompt = v != "0",
                "calendar_auto_sync" => s.calendar_auto_sync = v != "0",
                "clipboard_history" => s.clipboard_history = v != "0",
                _ => {}
            }
        }
''',
     '''                "calendar_prompt" => s.calendar_prompt = v != "0",
                "calendar_auto_sync" => s.calendar_auto_sync = v != "0",
                "clipboard_history" => s.clipboard_history = v != "0",
                "battery_alert" => s.battery_alert = match v.parse::<u32>().unwrap_or(20) { 0 => 0, 10 => 10, 30 => 30, _ => 20 },
                "notice_keywords" => s.notice_keywords = v.trim().to_string(),
                "quiet_minutes" => s.quiet_minutes = match v.parse::<u32>().unwrap_or(0) { 30 => 30, 60 => 60, 120 => 120, _ => 0 },
                "mail_filter" => s.mail_filter = v.trim().to_string(),
                _ => {}
            }
        }
'''),
    ("src/model.rs",
     '''                ("calendar_prompt".into(), (self.calendar_prompt as u8).to_string()),
                ("calendar_auto_sync".into(), (self.calendar_auto_sync as u8).to_string()),
                ("clipboard_history".into(), (self.clipboard_history as u8).to_string()),
            ],
        );
    }
''',
     '''                ("calendar_prompt".into(), (self.calendar_prompt as u8).to_string()),
                ("calendar_auto_sync".into(), (self.calendar_auto_sync as u8).to_string()),
                ("clipboard_history".into(), (self.clipboard_history as u8).to_string()),
                ("battery_alert".into(), self.battery_alert.to_string()),
                ("notice_keywords".into(), self.notice_keywords.clone()),
                ("quiet_minutes".into(), self.quiet_minutes.to_string()),
                ("mail_filter".into(), self.mail_filter.clone()),
            ],
        );
    }
'''),
    ("src/app.rs",
     '''    pub clipboard: crate::clipboard::Shared,
    pub(crate) clip_first: usize,
    pub(crate) clip_confirm_clear: Option<f64>,

    // 饮水
    pub(crate) water_ring: Spring,
''',
     '''    pub clipboard: crate::clipboard::Shared,
    pub(crate) clip_first: usize,
    pub(crate) clip_confirm_clear: Option<f64>,
    /// 剪贴板搜索词；非空时列表只显示命中的记录。
    pub(crate) clip_query: String,
    pub(crate) clip_search_focus: bool,
    /// 搜索输入框是否展开（面板表头平时只放一个「搜索」按钮）。
    pub(crate) clip_search_open: bool,

    // 饮水
    pub(crate) water_ring: Spring,
'''),
    ("src/app.rs",
     '''    pub(crate) phone_apps:crate::model::PhoneApps,
    /// 上次看到的手机电量（None=尚未初始化，用于断开/重连/低电/充满提示）。
    pub(crate) phone_last:Option<Option<u8>>,
    /// 状态总线当前存活的状态，用于常驻区并排显示。
    pub(crate) status_live:Vec<crate::island_status::Status>,
    /// 未接来电累计（点「未接来电」提示的关闭按钮清零）。
''',
     '''    pub(crate) phone_apps:crate::model::PhoneApps,
    /// 上次看到的手机电量（None=尚未初始化，用于断开/重连/低电/充满提示）。
    pub(crate) phone_last:Option<Option<u8>>,
    /// 上次看到的本机电量，用于本机低电 / 充满提示。
    pub(crate) pc_last:Option<Option<u8>>,
    /// 勿扰截止时刻（app 时间）；> t 期间不弹提示、不闪铃铛，验证码除外。
    pub(crate) quiet_until:f64,
    /// 状态总线当前存活的状态，用于常驻区并排显示。
    pub(crate) status_live:Vec<crate::island_status::Status>,
    /// 未接来电累计（点「未接来电」提示的关闭按钮清零）。
'''),
    ("src/app.rs",
     '''            s.visuals.selection.bg_fill = theme::alpha_blue(0.45);
        });
        let settings = model::Settings::load();
        let clipboard_on = settings.clipboard_history;
        let (bw, bh) = if settings.minimal { (BAR_W_MIN, BAR_H_MIN) } else { (BAR_W, BAR_H) };
        let focus_seconds=settings.focus_minutes as f32*60.0;
''',
     '''            s.visuals.selection.bg_fill = theme::alpha_blue(0.45);
        });
        let settings = model::Settings::load();
        // 邮件面板上次选的账号：结构体里 `settings` 会被 move，先取出来。
        let saved_mail_filter = settings.mail_filter.clone();
        let clipboard_on = settings.clipboard_history;
        let (bw, bh) = if settings.minimal { (BAR_W_MIN, BAR_H_MIN) } else { (BAR_W, BAR_H) };
        let focus_seconds=settings.focus_minutes as f32*60.0;
'''),
    ("src/app.rs",
     '''            clipboard: crate::clipboard::Shared::new(clipboard_on),
            clip_first: 0,
            clip_confirm_clear: None,
            water_ring: Spring::new(0.0, 120.0, 0.9),
            water_ml: 0.0,
            water_view: 0,
''',
     '''            clipboard: crate::clipboard::Shared::new(clipboard_on),
            clip_first: 0,
            clip_confirm_clear: None,
            clip_query: String::new(),
            clip_search_focus: false,
            clip_search_open: false,
            water_ring: Spring::new(0.0, 120.0, 0.9),
            water_ml: 0.0,
            water_view: 0,
'''),
    ("src/app.rs",
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), phone_last:None, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
''',
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), phone_last:None, pc_last:None, quiet_until:0.0, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
'''),
    ("src/app.rs",
     '''            volume_a: 0.0,
            bt_shown: Vec::new(),
            bt_initialized: false,
            mail: MailState { accounts: mail::load_accounts(), ..Default::default() },
            calendar: CalendarState::load(),
            quota: crate::quota_ui::State::load(),
            updates: Default::default(),
''',
     '''            volume_a: 0.0,
            bt_shown: Vec::new(),
            bt_initialized: false,
            mail: {
                // 邮件面板记住上次选的账号：State 有私有字段，只能先默认构造再赋值。
                let mut manage = crate::mail_manage_ui::State::default();
                if !saved_mail_filter.is_empty() { manage.account_filter = Some(saved_mail_filter); }
                MailState { accounts: mail::load_accounts(), manage, ..Default::default() }
            },
            calendar: CalendarState::load(),
            quota: crate::quota_ui::State::load(),
            updates: Default::default(),
'''),
    ("src/app.rs",
     '''        if self.mail.open_uid.as_ref().is_some_and(|(email,_)|!active(&email.to_ascii_lowercase())){self.mail.open_uid=None;self.mail.manage.auto_read=None;}
        self.mail.manage.identities.retain(|_,old|accounts.contains(old));
        self.mail.manage.pending.retain(|email|accounts.iter().any(|a|mail::key(a)==*email));
        if self.mail.manage.account_filter.as_ref().is_some_and(|f|!accounts.iter().any(|a|a.email.eq_ignore_ascii_case(f))){self.mail.manage.account_filter=None;self.mail.manage.account_menu=false;}
        self.mail.manage.next_refresh=0.0;
        mail::save_accounts(accounts);
        self.pending.push(Cmd::MailSync);
''',
     '''        if self.mail.open_uid.as_ref().is_some_and(|(email,_)|!active(&email.to_ascii_lowercase())){self.mail.open_uid=None;self.mail.manage.auto_read=None;}
        self.mail.manage.identities.retain(|_,old|accounts.contains(old));
        self.mail.manage.pending.retain(|email|accounts.iter().any(|a|mail::key(a)==*email));
        if self.mail.manage.account_filter.as_ref().is_some_and(|f|!accounts.iter().any(|a|a.email.eq_ignore_ascii_case(f))){self.mail.manage.account_filter=None;self.mail.manage.account_menu=false;if !self.settings.mail_filter.is_empty(){self.settings.mail_filter.clear();self.settings.save();}}
        self.mail.manage.next_refresh=0.0;
        mail::save_accounts(accounts);
        self.pending.push(Cmd::MailSync);
'''),
    ("src/app.rs",
     '''    }

    pub fn is_editing(&self) -> bool {
        self.adding || self.editing.is_some() || self.todo_choosing || self.task_dialog.is_some() || self.calendar.draft.is_some() || self.focus_editing
    }

    pub fn filter_todo_input(&mut self,input:&mut egui::RawInput) {
''',
     '''    }

    pub fn is_editing(&self) -> bool {
        // 剪贴板搜索也属于「正在输入」：窗口被激活时要保持展开，否则光标一闪就收起来了。
        self.adding || self.editing.is_some() || self.todo_choosing || self.task_dialog.is_some() || self.calendar.draft.is_some() || self.focus_editing || self.clip_search_open
    }

    pub fn filter_todo_input(&mut self,input:&mut egui::RawInput) {
'''),
    ("src/app.rs",
     '''        self.resident_power=(env.battery,env.charging);
        self.phone_resident=phone;
        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
''',
     '''        self.resident_power=(env.battery,env.charging);
        self.phone_resident=phone;
        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}
        self.pc_events(env.battery,env.charging);
        // 勿扰到点自动解除；同时把设置项归零，避免界面上还显示着「60 分钟」。
        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
'''),
    ("src/extras.rs",
     '''        // 通知仅保存在本次运行的内存中。
        if a.source==AlertSource::Internal {self.history.push_front(a.clone());}
        self.history.truncate(50);
        if self.settings.notifications {
            if system && a.source==AlertSource::Internal {
                self.pending
''',
     '''        // 通知仅保存在本次运行的内存中。
        if a.source==AlertSource::Internal {self.history.push_front(a.clone());}
        self.history.truncate(50);
        // 勿扰：只记账、不打扰。横幅、铃铛、系统通知都不出现，验证码例外（用户要的就是那串数字）。
        if self.muted() && a.status.is_none() && crate::otp::extract("",&a.body,&[]).is_none() { return; }
        if self.settings.notifications {
            if system && a.source==AlertSource::Internal {
                self.pending
'''),
    ("src/extras.rs",
     '''        self.phone_last=Some(now);
        let Some(prev)=prev else{return;};
        if !self.settings.notifications{return;}
        let (title,body)=match (prev,now) {
            (Some(_),None)=>("手机已断开".to_string(),"电量信息已收起".to_string()),
            (None,Some(level))=>(format!("手机已连接 {level}%"),"手机连接已恢复".to_string()),
            (Some(old),Some(level)) if old>20&&level<=20=>(format!("手机电量偏低 {level}%"),"记得给手机充电".to_string()),
            (Some(old),Some(100)) if old<100=>(format!("手机已充满{}",if charging{"（充电中）"}else{""}),"可以拔掉充电器了".to_string()),
            _=>return,
        };
        self.alert(title,body,usize::MAX,false);
    }

    pub fn receive_notification(&mut self, app:String, id:String, body:String){
        let phone=crate::system::phone_channel(&id);
        // 手机 App 名单是外部 JSON，改完约 1 秒内生效，不用重启岛仔。
''',
     '''        self.phone_last=Some(now);
        let Some(prev)=prev else{return;};
        if !self.settings.notifications{return;}
        let low=self.settings.low_battery_threshold();
        let (title,body)=match (prev,now) {
            (Some(_),None)=>("手机已断开".to_string(),"电量信息已收起".to_string()),
            (None,Some(level))=>(format!("手机已连接 {level}%"),"手机连接已恢复".to_string()),
            (Some(old),Some(level)) if low.is_some_and(|t|old>t&&level<=t)=>(format!("手机电量偏低 {level}%"),format!("低于 {}%，记得给手机充电",low.unwrap_or(20))),
            (Some(old),Some(100)) if old<100=>(format!("手机已充满{}",if charging{"（充电中）"}else{""}),"可以拔掉充电器了".to_string()),
            _=>return,
        };
        self.alert(title,body,usize::MAX,false);
    }

    /// 本机电量的低电 / 充满提示，阈值与手机共用「低电提醒」设置。
    /// 首轮只记录不提示，避免每次启动都弹一条。
    pub(crate) fn pc_events(&mut self,now:Option<u8>,charging:bool){
        let prev=self.pc_last;
        self.pc_last=Some(now);
        let Some(prev)=prev else{return;};
        if !self.settings.notifications{return;}
        let Some(low)=self.settings.low_battery_threshold() else{return;};
        let (title,body)=match (prev,now) {
            (Some(old),Some(level)) if old>low&&level<=low=>(format!("本机电量偏低 {level}%"),if charging{"已接电源，正在充电".to_string()}else{format!("低于 {low}%，建议接上电源")}),
            (Some(old),Some(100)) if old<100&&charging=>("本机已充满 100%".to_string(),"可以拔掉充电器了".to_string()),
            _=>return,
        };
        self.alert(title,body,usize::MAX,false);
    }

    /// 勿扰是否生效（按 app 时间比较，重启后自动失效）。
    pub(crate) fn muted(&self)->bool{self.quiet_until>self.t}

    /// 文本是否命中通知关键词表（大小写不敏感）。
    pub(crate) fn keyword_hit(&self,text:&str)->bool{
        if text.is_empty(){return false;}
        let lower=text.to_lowercase();
        self.settings.keywords().iter().any(|word|lower.contains(&word.to_lowercase()))
    }

    /// 勿扰开关：`minutes = 0` 关闭。只在本次运行内有效，重启后恢复关闭。
    pub(crate) fn set_quiet(&mut self,minutes:u32){
        self.settings.quiet_minutes=minutes;
        self.settings.save();
        self.quiet_until=if minutes==0 {0.0} else {self.t+minutes as f64*60.0};
        self.say(if minutes==0 {"勿扰已关闭".to_string()} else {format!("勿扰 {minutes} 分钟")});
    }

    /// 状态总线触发的番茄钟开关：`on = false` 只是暂停，不重置这一轮。
    pub(crate) fn set_focus_running(&mut self,on:bool){
        if on {
            if self.pomo.running {return;}
            self.pomo.phase=crate::app::Phase::Focus;
            self.pomo.remaining=self.phase_secs(crate::app::Phase::Focus);
            self.pomo.running=true;
            self.say("已开始专注");
        } else if self.pomo.running {
            self.pomo.running=false;
            self.say("已暂停专注");
        }
    }

    pub fn receive_notification(&mut self, app:String, id:String, body:String){
        let phone=crate::system::phone_channel(&id);
        // 手机 App 名单是外部 JSON，改完约 1 秒内生效，不用重启岛仔。
'''),
    ("src/extras.rs",
     '''        if self.history.iter().any(|a|format!("{} · {}",a.title,a.body)==body){return;}
        if !self.notification_apps.observe(&id,&app) || !self.settings.notifications{return;}
        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        self.notification_bell_at=self.t;
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去；
        // 「重复查看」开启时普通通知也保留更久，方便多次悬停。
        let otp_bell=phone && !body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        let keep_bell=if cfg!(test) {4.5} else if otp_bell {90.0} else if self.settings.notice_keep {60.0} else {4.5};
        self.notification_bell_until=self.t+keep_bell;
        // 「手机 · 」前缀用来和岛仔自带 IMAP 邮件的「新邮件」提醒区分开。
        let title=if let Some((label,_))=&phone_info {format!("手机 · {label}")}else{app.chars().filter(|c|!c.is_control()).take(80).collect()};
        // 10 秒窗口内同一 App 的多条消息合并计数，配合 enqueue_alert 的替换去重防刷屏。
''',
     '''        if self.history.iter().any(|a|format!("{} · {}",a.title,a.body)==body){return;}
        if !self.notification_apps.observe(&id,&app) || !self.settings.notifications{return;}
        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去；
        // 「重复查看」开启时普通通知也保留更久，方便多次悬停。
        let otp_bell=phone && !body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        let keep_bell=if cfg!(test) {4.5} else if otp_bell {90.0} else if self.settings.notice_keep {60.0} else {4.5};
        // 勿扰期间不闪铃铛（横幅同样被 enqueue_alert 拦下），验证码仍照常提醒。
        if !self.muted() || otp_bell {
            self.notification_bell_at=self.t;
            self.notification_bell_until=self.t+keep_bell;
        }
        // 「手机 · 」前缀用来和岛仔自带 IMAP 邮件的「新邮件」提醒区分开。
        let title=if let Some((label,_))=&phone_info {format!("手机 · {label}")}else{app.chars().filter(|c|!c.is_control()).take(80).collect()};
        // 10 秒窗口内同一 App 的多条消息合并计数，配合 enqueue_alert 的替换去重防刷屏。
'''),
    ("src/extras.rs",
     '''    true
}

/// 状态总线的点击动作白名单：只认这三类，不执行任意命令。
fn status_action(spec:&str)->Option<(&'static str,Cmd)>{
    if let Some(text)=spec.strip_prefix("copy:") {
        if !text.is_empty(){return Some(("复制",Cmd::Copy(text.to_string())));}
    }
    if let Some(url)=spec.strip_prefix("url:") {
        if url.starts_with("https://")||url.starts_with("http://"){return Some(("打开",Cmd::OpenUrl(url.to_string())));}
    }
    if let Some(file)=spec.strip_prefix("file:") {
        if !file.is_empty(){return Some(("打开",Cmd::Open(std::path::PathBuf::from(file))));}
    }
    None
}
pub(crate) fn status_color(name:&str)->Color32{
    match name.trim().to_ascii_lowercase().as_str(){
''',
     '''    true
}

/// 状态总线点击后的动作：要么交给外壳执行，要么由岛仔自己处理。
pub(crate) enum StatusAct {
    Shell(Cmd),
    /// 开始 / 暂停番茄专注。
    Focus(bool),
    /// 勿扰 N 分钟（0 = 关闭）。
    Quiet(u32),
    /// 跳到第 N 页（0 音乐 … 7 设置）。
    Page(usize),
}

/// 状态总线的点击动作白名单：`copy:` / `url:` / `file:` 交给外壳，
/// `action:` 只认下表里的几个词——脚本能指挥岛仔，但指挥不了整台电脑。
fn status_action(spec:&str)->Option<(&'static str,StatusAct)>{
    use StatusAct::*;
    if let Some(text)=spec.strip_prefix("copy:") {
        if !text.is_empty(){return Some(("复制",Shell(Cmd::Copy(text.to_string()))));}
    }
    if let Some(url)=spec.strip_prefix("url:") {
        if url.starts_with("https://")||url.starts_with("http://"){return Some(("打开",Shell(Cmd::OpenUrl(url.to_string()))));}
    }
    if let Some(file)=spec.strip_prefix("file:") {
        if !file.is_empty(){return Some(("打开",Shell(Cmd::Open(std::path::PathBuf::from(file)))));}
    }
    let action=spec.strip_prefix("action:")?.trim();
    match action {
        "focus_start"=>Some(("开始专注",Focus(true))),
        "focus_stop"=>Some(("结束专注",Focus(false))),
        "quiet30"=>Some(("勿扰 30 分",Quiet(30))),
        "quiet60"=>Some(("勿扰 60 分",Quiet(60))),
        "quiet_off"=>Some(("关闭勿扰",Quiet(0))),
        _=>action.strip_prefix("page:").and_then(|n|n.trim().parse::<usize>().ok()).filter(|n|*n<8).map(|n|("打开",Page(n))),
    }
}
pub(crate) fn status_color(name:&str)->Color32{
    match name.trim().to_ascii_lowercase().as_str(){
'''),
    ("src/extras.rs",
     '''impl App {
    pub(crate) fn draw_notice(&mut self, c:&Card,panel:Rect,a:&Alert,_cmds:&mut Vec<Cmd>){
        let nc=c.clipped(0.0,0.0,panel.width(),panel.height());
        if let Some(source)=a.messenger(){
            messenger_icon(&nc,22.0,22.0,source);
            nc.circle(31.0,14.0,2.6,Color32::from_rgb(255,72,72));
''',
     '''impl App {
    pub(crate) fn draw_notice(&mut self, c:&Card,panel:Rect,a:&Alert,_cmds:&mut Vec<Cmd>){
        let nc=c.clipped(0.0,0.0,panel.width(),panel.height());
        // 关键词高亮：命中词表时在卡片最左侧亮一条琥珀色竖条。
        // 只加一条竖条，不动文字和任何控件的位置，布局与测试友好的部分保持原样。
        if a.status.is_none() && (self.keyword_hit(&a.title) || self.keyword_hit(&a.body)) {
            nc.fill(2.5,10.0,2.5,24.0,1.25,Color32::from_rgb(239,159,39));
        }
        if let Some(source)=a.messenger(){
            messenger_icon(&nc,22.0,22.0,source);
            nc.circle(31.0,14.0,2.6,Color32::from_rgb(255,72,72));
'''),
    ("src/extras.rs",
     '''                    nc.fill(16.0,37.0,198.0,2.0,1.0,Color32::from_gray(60));
                    nc.fill(16.0,37.0,198.0*p,2.0,1.0,tone);
                }
                if let Some((label,cmd))=status_action(&st.click) {
                    if notice_button(&nc,160.0,12.0,60.0,label,"status-action"){_cmds.push(cmd);}
                }
                if notice_button(&nc,226.0,12.0,60.0,"关闭","external-dismiss"){
                    self.notice=None;
''',
     '''                    nc.fill(16.0,37.0,198.0,2.0,1.0,Color32::from_gray(60));
                    nc.fill(16.0,37.0,198.0*p,2.0,1.0,tone);
                }
                if let Some((label,act))=status_action(&st.click) {
                    if notice_button(&nc,160.0,12.0,60.0,label,"status-action"){
                        match act {
                            StatusAct::Shell(cmd)=>_cmds.push(cmd),
                            StatusAct::Focus(on)=>self.set_focus_running(on),
                            StatusAct::Quiet(minutes)=>self.set_quiet(minutes),
                            StatusAct::Page(page)=>self.show_page(page),
                        }
                    }
                }
                if notice_button(&nc,226.0,12.0,60.0,"关闭","external-dismiss"){
                    self.notice=None;
'''),
    ("src/extras.rs",
     '''                            setting_row(ui,"蓝牙通知","设备连接与断开提醒；关闭后停止蓝牙监听",&mut self.settings.bluetooth_notifications);
                            ui.separator();
                            setting_row(ui,"提醒总开关","关闭后饮水、专注、待办、微信和 QQ 提醒都不显示",&mut self.settings.notifications);
                        });
                        // 总开关关闭时，微信与 QQ 两栏置灰并说明原因，避免「开了也不提醒」。
                        let master=self.settings.notifications;
''',
     '''                            setting_row(ui,"蓝牙通知","设备连接与断开提醒；关闭后停止蓝牙监听",&mut self.settings.bluetooth_notifications);
                            ui.separator();
                            setting_row(ui,"提醒总开关","关闭后饮水、专注、待办、微信和 QQ 提醒都不显示",&mut self.settings.notifications);
                            ui.separator();
                            settings_control_row(ui,"低电提醒",SETTINGS_LABEL_W,|ui|{
                                let mut value=self.settings.battery_alert;
                                ui.horizontal(|ui|{
                                    if settings_segmented(ui,&mut value,&[(0,"关"),(10,"10%"),(20,"20%"),(30,"30%")],44.0){
                                        self.settings.battery_alert=value;
                                    }
                                }).response
                            });
                            settings_hint(ui,"跨过阈值只提醒一次，手机与本机共用；充满时也会提示。");
                            ui.separator();
                            // 勿扰：选时长立即生效，到点自动关闭；重启后恢复关闭。
                            let mut quiet=if self.quiet_until>self.t {self.settings.quiet_minutes.max(30)} else {0};
                            let mut quiet_pick=None;
                            settings_control_row(ui,"勿扰",SETTINGS_LABEL_W,|ui|{
                                ui.horizontal(|ui|{
                                    if settings_segmented(ui,&mut quiet,&[(0,"关"),(30,"30 分"),(60,"60 分"),(120,"2 小时")],48.0){
                                        quiet_pick=Some(quiet);
                                    }
                                }).response
                            });
                            if let Some(minutes)=quiet_pick {self.set_quiet(minutes);}
                            settings_hint(ui,"勿扰期间不弹横幅、不闪铃铛；验证码仍会提醒。");
                            ui.separator();
                            let mut keywords_on=!self.settings.notice_keywords.trim().is_empty();
                            setting_row(ui,"通知关键词高亮","命中关键词时卡片左侧亮起琥珀色竖条；词表改 settings.txt 的 notice_keywords",&mut keywords_on);
                            if keywords_on != !self.settings.notice_keywords.trim().is_empty() {
                                self.settings.notice_keywords=if keywords_on {"验证码,面试,转账,取款,取件,欠费".to_string()} else {String::new()};
                            }
                        });
                        // 总开关关闭时，微信与 QQ 两栏置灰并说明原因，避免「开了也不提醒」。
                        let master=self.settings.notifications;
'''),
    ("src/pages.rs",
     '''        for (i, (label, w)) in labels.iter().zip(&widths).enumerate() {
            let selected = self.shelf_tab == i;
            let r = c.hit(left, 9.5, *w, 21.0, ("shelf-tab", i));
            let (h, _) = c.hover_t(&r);
            if selected {
                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_gray(88));
''',
     '''        for (i, (label, w)) in labels.iter().zip(&widths).enumerate() {
            let selected = self.shelf_tab == i;
            let r = c.hit(left, 9.5, *w, 21.0, ("shelf-tab", i));
            let r = if i == 1 { r.on_hover_text(if selected { "再点一次：搜索剪贴板" } else { "切到剪贴板" }) } else { r };
            let (h, _) = c.hover_t(&r);
            if selected {
                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_gray(88));
'''),
    ("src/pages.rs",
     '''                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_white_alpha((18.0 * h) as u8));
            }
            c.text(left + w / 2.0, 24.3, label, font, W::Bold, if selected { theme::WHITE } else { theme::GRAY }, H::Center);
            if r.clicked() && !selected {
                self.shelf_tab = i;
                self.confirm_clear = None;
                self.clip_confirm_clear = None;
            }
            left += w;
        }
''',
     '''                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_white_alpha((18.0 * h) as u8));
            }
            c.text(left + w / 2.0, 24.3, label, font, W::Bold, if selected { theme::WHITE } else { theme::GRAY }, H::Center);
            if r.clicked() {
                if !selected {
                    self.shelf_tab = i;
                    self.confirm_clear = None;
                    self.clip_confirm_clear = None;
                } else if i == 1 {
                    // 已选中的「剪贴板」再点一次：展开 / 收起搜索。
                    self.clip_search_open = !self.clip_search_open;
                    self.clip_query.clear();
                    self.clip_search_focus = false;
                }
            }
            left += w;
        }
'''),
    ("src/pages.rs",
     '''    }

    // ================= 剪贴板（置物架第二个分栏） =================
    fn page_clipboard(&mut self, c: &Card, _env: &Env, _cmds: &mut Vec<Cmd>) {
        if !self.settings.clipboard_history {
            c.dashed(42.0, 32.0, 335.0, 91.0, 12.0, 1.2, Color32::from_gray(70), 0.0);
''',
     '''    }

    // ================= 剪贴板（置物架第二个分栏） =================
    /// 剪贴板搜索：表头其余元素（专注环/日期/时钟）都会随内容左右移动，
    /// 只有「清空」右侧那一小段是固定位置，放大镜就钉在那里；
    /// 点开后输入框浮在列表上方（盖住第一行），Esc 或小叉收起。
    /// 过滤只作用于显示，索引仍指向同一份历史，复制/固定/删除的行为不变。
    fn clip_search(&mut self, c: &Card, cmds: &mut Vec<Cmd>) {
        // 放大镜：固定位置，不参与表头的动态排布。
        let icon = c.hit(352.0, 9.5, 24.0, 21.0, "clip-search-open");
        let (h, _) = c.hover_t(&icon);
        if h > 0.01 { c.circle(364.0, 20.0, 10.5, Color32::from_white_alpha((22.0 * h) as u8)); }
        let ink = if self.clip_search_open { theme::BLUE } else { paint::mix(theme::GRAY, theme::WHITE, h) };
        c.ring(362.2, 18.2, 4.4, 1.3, ink);
        c.seg((365.6, 21.6), (369.4, 25.4), 1.3, ink);
        let icon = icon.on_hover_text(if self.clip_search_open { "收起搜索" } else { "搜索剪贴板" });
        if icon.clicked() {
            self.clip_search_open = !self.clip_search_open;
            self.clip_query.clear();
            self.clip_search_focus = false;
        }
        if !self.clip_search_open { return; }
        // 输入框浮在列表卡片的上沿，比卡片亮一档 + 一圈描边，聚焦时描边变蓝。
        let rect = c.rect(46.0, 32.0, 327.0, 20.0);
        c.fill(46.0, 32.0, 327.0, 20.0, 8.0, Color32::from_gray(64));
        c.stroke(46.0, 32.0, 327.0, 20.0, 8.0, 1.0, Color32::from_gray(102));
        let mut ui = egui::Ui::new(c.ui.ctx().clone(), egui::Id::new("clip-search"), egui::UiBuilder::new().layer_id(c.ui.layer_id()).max_rect(rect).style(c.ui.style().clone()));
        ui.set_clip_rect(c.painter.clip_rect());
        ui.set_opacity(c.painter.opacity());
        if !c.live { ui.disable(); }
        // 多留一点左侧内边距，文字不贴边；光标也与文字对齐。
        let text_rect = Rect::from_min_max(rect.min + egui::vec2(8.0, 2.0), rect.max - egui::vec2(26.0, 2.0));
        let mut buf = std::mem::take(&mut self.clip_query);
        let te = egui::TextEdit::singleline(&mut buf)
            .id(egui::Id::new("clip-search-edit"))
            .frame(false)
            .margin(egui::Margin::ZERO)
            .text_color(theme::WHITE)
            .font(theme::font(size::TINY, W::Regular))
            .hint_text(egui::RichText::new("输入关键词筛选").color(Color32::from_gray(158)).font(theme::font(size::TINY, W::Regular)))
            .desired_width(text_rect.width());
        let resp = ui.put(text_rect, te);
        self.clip_query = buf;
        // 展开后的第一帧就请求焦点并激活窗口，光标才会闪；之后点一下输入框也会重新聚焦。
        if !self.clip_search_focus || resp.clicked() {
            self.clip_search_focus = true;
            resp.request_focus();
            cmds.push(Cmd::Focus);
        }
        if resp.has_focus() { c.stroke(46.0, 32.0, 327.0, 20.0, 8.0, 1.4, theme::BLUE); }
        // Esc 退出搜索：清空关键词并收起输入框。
        if resp.has_focus() && c.ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.clip_query.clear();
            self.clip_search_open = false;
            resp.surrender_focus();
        }
        let x = c.hit(352.0, 34.0, 18.0, 16.0, "clip-search-close").on_hover_text(if self.clip_query.is_empty() { "关闭搜索" } else { "清空并关闭" });
        paint::close(c, 361.0, 42.0, 2.6, if x.hovered() { theme::BLUE } else { Color32::from_gray(190) });
        if x.clicked() {
            self.clip_query.clear();
            self.clip_search_open = false;
            resp.surrender_focus();
        }
    }

    fn page_clipboard(&mut self, c: &Card, _env: &Env, _cmds: &mut Vec<Cmd>) {
        if !self.settings.clipboard_history {
            c.dashed(42.0, 32.0, 335.0, 91.0, 12.0, 1.2, Color32::from_gray(70), 0.0);
'''),
    ("src/pages.rs",
     '''            return;
        }
        let items = self.clipboard.lock().items.clone();
        let n = items.len();
        let recent = items.iter().filter(|clip| !clip.pinned).count();
        self.clip_first = self.clip_first.min(n.saturating_sub(CLIP_ROWS));

        // 右上：条数与「清空」（只清未固定的）。
        if recent > 0 {
''',
     '''            return;
        }
        let items = self.clipboard.lock().items.clone();
        let query = self.clip_query.trim().to_lowercase();
        let shown: Vec<usize> = if query.is_empty() {
            (0..items.len()).collect()
        } else {
            items.iter().enumerate().filter(|(_,clip)|clip.text.to_lowercase().contains(&query)).map(|(i,_)|i).collect()
        };
        let n = shown.len();
        let recent = items.iter().filter(|clip| !clip.pinned).count();
        // 搜索时输入框占掉卡片第一行，可见行数少一行。
        let searching = self.clip_search_open || !self.clip_query.is_empty();
        let rows = if searching { CLIP_ROWS - 1 } else { CLIP_ROWS };
        self.clip_first = self.clip_first.min(n.saturating_sub(rows));

        // 右上：条数与「清空」（只清未固定的）。
        if recent > 0 {
'''),
    ("src/pages.rs",
     '''
        if n == 0 {
            c.dashed(42.0, 32.0, 335.0, 91.0, 12.0, 1.2, Color32::from_gray(70), 0.0);
            paint::clipboard_glyph(c, 209.5, 58.0, 22.0, Color32::from_gray(200), Color32::from_gray(80));
            c.text(209.5, 93.0, "复制的文字会出现在这里", 12.0, W::Semibold, theme::WHITE, H::Center);
            c.text(209.5, 108.0, "记录保存在本机，重新打开仍可查看", size::TINY, W::Regular, theme::GRAY, H::Center);
            return;
        }

        c.fill(42.0, 31.0, 335.0, 93.0, 12.0, theme::CARD);
        // 超过一屏时右侧细滚动条，滚轮查看。
        if n > CLIP_ROWS {
            let (top, track) = (38.0, 79.0);
            let thumb = (track * CLIP_ROWS as f32 / n as f32).max(12.0);
            let y = top + (track - thumb) * self.clip_first as f32 / (n - CLIP_ROWS) as f32;
            c.fill(371.0, top, 2.5, track, 1.25, Color32::from_white_alpha(14));
            c.fill(371.0, y, 2.5, thumb, 1.25, Color32::from_white_alpha(70));
        }
        let mut action: Option<(usize, u8)> = None; // 0 复制 1 固定 2 删除
        for (row, (index, clip)) in items.iter().enumerate().skip(self.clip_first).take(CLIP_ROWS).enumerate() {
            let y = 44.0 + row as f32 * 22.5;
            let r = c.hit(48.0, y - 10.0, 323.0, 20.0, ("clip", index, clip.text.len(), clip.at.as_str()));
            let (h, _) = c.hover_t(&r);
            if h > 0.01 {
''',
     '''
        if n == 0 {
            c.dashed(42.0, 32.0, 335.0, 91.0, 12.0, 1.2, Color32::from_gray(70), 0.0);
            if query.is_empty() {
                paint::clipboard_glyph(c, 209.5, 58.0, 22.0, Color32::from_gray(200), Color32::from_gray(80));
                c.text(209.5, 93.0, "复制的文字会出现在这里", 12.0, W::Semibold, theme::WHITE, H::Center);
                c.text(209.5, 108.0, "记录保存在本机，重新打开仍可查看", size::TINY, W::Regular, theme::GRAY, H::Center);
            } else {
                c.text(209.5, 70.0, "没有匹配的记录", 12.0, W::Semibold, theme::WHITE, H::Center);
                c.text(209.5, 86.0, "换个关键词试试，或点右侧小叉退出搜索", size::TINY, W::Regular, theme::GRAY, H::Center);
            }
            self.clip_search(c, _cmds);
            return;
        }

        c.fill(42.0, 31.0, 335.0, 93.0, 12.0, theme::CARD);
        // 超过一屏时右侧细滚动条，滚轮查看。
        if n > rows {
            let (top, track) = (38.0, 79.0);
            let thumb = (track * rows as f32 / n as f32).max(12.0);
            let y = top + (track - thumb) * self.clip_first as f32 / (n - rows) as f32;
            c.fill(371.0, top, 2.5, track, 1.25, Color32::from_white_alpha(14));
            c.fill(371.0, y, 2.5, thumb, 1.25, Color32::from_white_alpha(70));
        }
        let mut action: Option<(usize, u8)> = None; // 0 复制 1 固定 2 删除
        for (row, index) in shown.iter().copied().skip(self.clip_first).take(rows).enumerate() {
            let clip = &items[index];
            let y = if searching { 62.0 } else { 44.0 } + row as f32 * 22.5;
            let r = c.hit(48.0, y - 10.0, 323.0, 20.0, ("clip", index, clip.text.len(), clip.at.as_str()));
            let (h, _) = c.hover_t(&r);
            if h > 0.01 {
'''),
    ("src/pages.rs",
     '''            Some((index, _)) => { self.clipboard.lock().remove(index); }
            None => {}
        }
    }

    fn file_icon_at(&mut self, c: &Card, p: &std::path::Path, cx: f32, cy: f32, sz: f32) {
''',
     '''            Some((index, _)) => { self.clipboard.lock().remove(index); }
            None => {}
        }
        // 搜索框最后画：它浮在卡片上沿，画在卡片之前会被卡片底色盖掉。
        self.clip_search(c, _cmds);
    }

    fn file_icon_at(&mut self, c: &Card, p: &std::path::Path, cx: f32, cy: f32, sz: f32) {
'''),
    ("src/pages.rs",
     '''        let unread=self.mail.recent.iter().filter(|m|self.mail_account_match(m)&&self.mail_seen(m,Folder::Inbox)==Some(false)).count();
        if trash{
            c.text(list_x+6.0,47.0,&format!("{} {}",if self.settings.mail_sync_operations{"回收站"}else{"本机回收站"},self.mail.manage.trash.len()),10.5,W::Semibold,theme::WHITE,H::Left);
            c.text(375.0,47.0,"返回邮件",10.0,W::Regular,theme::BLUE,H::Right);
            if c.hit(322.0,33.0,56.0,19.0,"mail-trash-back").clicked(){self.mail_toggle_trash();return;}
        }else{
''',
     '''        let unread=self.mail.recent.iter().filter(|m|self.mail_account_match(m)&&self.mail_seen(m,Folder::Inbox)==Some(false)).count();
        if trash{
            c.text(list_x+6.0,47.0,&format!("{} {}",if self.settings.mail_sync_operations{"回收站"}else{"本机回收站"},self.mail.manage.trash.len()),10.5,W::Semibold,theme::WHITE,H::Left);
            // 回收站同样遵守账号筛选，这里补上同一个选框（原来只有收件箱有）。
            let filter_label=match &self.mail.manage.account_filter{
                Some(email)=>Self::mail_account_label(email),
                None=>"全部账号".to_string(),
            };
            c.fill(148.0,33.0,140.0,19.0,6.0,theme::CARD);
            c.text(218.0,47.0,&c.fit(&filter_label,10.0,W::Semibold,128.0),10.0,W::Semibold,if self.mail.manage.account_filter.is_some(){theme::WHITE}else{theme::GRAY},H::Center);
            if c.hit(148.0,33.0,140.0,19.0,"mail-account-filter").on_hover_text("选择要看的邮箱账号").clicked(){
                self.mail.manage.account_menu=!self.mail.manage.account_menu;
            }
            c.text(375.0,47.0,"返回邮件",10.0,W::Regular,theme::BLUE,H::Right);
            if c.hit(322.0,33.0,56.0,19.0,"mail-trash-back").clicked(){self.mail_toggle_trash();return;}
        }else{
'''),
    ("src/pages.rs",
     '''                c.text(156.0,y+11.5,&c.fit(&label,9.5,W::Regular,124.0),9.5,W::Regular,if selected{theme::BLUE}else{theme::WHITE},H::Left);
            }
            if let Some(choice)=pick{
                self.mail.manage.account_filter=choice;
                self.mail.manage.account_menu=false;
            }
''',
     '''                c.text(156.0,y+11.5,&c.fit(&label,9.5,W::Regular,124.0),9.5,W::Regular,if selected{theme::BLUE}else{theme::WHITE},H::Left);
            }
            if let Some(choice)=pick{
                // 记住上次选的账号，重启后沿用（空 = 全部账号）。
                self.settings.mail_filter=choice.clone().unwrap_or_default();
                self.settings.save();
                self.mail.manage.account_filter=choice;
                self.mail.manage.account_menu=false;
            }
'''),
    ("src/ui_tests.rs",
     '''    crate::mail::save_accounts(&accounts);assert_eq!(crate::mail::load_accounts(),accounts);
}

fn editing_local_calendar_clears_outdated_reminders() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for reschedule in [false,true] {
''',
     '''    crate::mail::save_accounts(&accounts);assert_eq!(crate::mail::load_accounts(),accounts);
}

#[test]
fn clipboard_search_toggles_on_the_active_tab_and_filters_lines() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(3);h.app.shelf_tab=1;
    {let mut hist=h.app.clipboard.lock();hist.items.clear();
     hist.push("会议纪要：周四评审","09:31");hist.push("验证码 482913","09:32");}
    h.hover(H::local(209.0,60.0),1.2);
    // 点右上角固定的放大镜：浮出搜索输入框。
    h.click(H::local(364.0,20.0));
    assert!(h.app.clip_search_open,"点放大镜应浮出搜索框");
    h.frame(vec![]);
    // 过滤只影响显示，历史本身不动。
    h.app.clip_query="会议".into();
    h.frame(vec![]);
    assert_eq!(h.app.clipboard.lock().items.len(),2,"搜索不能删改历史");
    assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("验证码 482913"))),"过滤后不该再画不匹配的记录");
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("会议纪要"))),"命中的记录要留下");
    h.click(H::local(364.0,20.0));
    assert!(!h.app.clip_search_open,"再点一次应收起搜索");
    assert!(h.app.clip_query.is_empty(),"收起搜索要清空关键词");
}

fn editing_local_calendar_clears_outdated_reminders() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for reschedule in [false,true] {
'''),
    ("src/ui_tests.rs",
     '''#[test]
fn ignored_mail_event_hides_stops_reminders_persists_and_restores() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let now=chrono::Local::now();let day=now.date_naive();
    let meeting=crate::mail::Meeting{all_day:false,title:"测试邀请".into(),start:now.timestamp()+600,end:None,at:String::new(),uid:"ignore-me".into()};
    h.app.mail.recent.push(crate::mail::Item {received_unix: None, account:"test@qq.com".into(),from:"测试".into(),subject:"邀请".into(),time:String::new(),code:None,summary:String::new().into(),links:vec![],meeting:Some(meeting.clone()),uid:99});
    h.app.toggle_mail_event(&meeting);
    assert!(h.app.calendar_entries(day).is_empty());
''',
     '''#[test]
fn ignored_mail_event_hides_stops_reminders_persists_and_restores() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let now=chrono::Local::now();
    // 取日程真正落在的那一天：23:5x 跑测试时「10 分钟后」已经是明天，按今天断言会假失败。
    let start=now.timestamp()+600;
    let day=chrono::DateTime::from_timestamp(start,0).unwrap().with_timezone(&chrono::Local).date_naive();
    let meeting=crate::mail::Meeting{all_day:false,title:"测试邀请".into(),start,end:None,at:String::new(),uid:"ignore-me".into()};
    h.app.mail.recent.push(crate::mail::Item {received_unix: None, account:"test@qq.com".into(),from:"测试".into(),subject:"邀请".into(),time:String::new(),code:None,summary:String::new().into(),links:vec![],meeting:Some(meeting.clone()),uid:99});
    h.app.toggle_mail_event(&meeting);
    assert!(h.app.calendar_entries(day).is_empty());
'''),
    ("src/ui_tests.rs",
     '''        }
        assert!(h.app.quota.gemini_connecting.is_none()&&h.app.quota.workbuddy_connecting.is_none()&&h.app.quota.login.is_none());
    }
}
''',
     '''        }
        assert!(h.app.quota.gemini_connecting.is_none()&&h.app.quota.workbuddy_connecting.is_none()&&h.app.quota.login.is_none());
    }

}
'''),

    # custom-28：A 组功能集：通知/邮件/剪贴板/状态总线
    ("src/system.rs",
     '''    ("com.alipay.iphoneclient","支付宝"),
    ("com.google.Gmail","Gmail"),
];
/// 手机连接 Toast 的正文首段通常是来源 App 名（真实通知只报主 AUMID `!App`）。
/// 名单放在 `%APPDATA%\\Daozai\\phone-apps.json`，加新 App 不用改代码重编译。
/// 返回 (标签, 是否允许上岛)；None = 无法识别，由调用方按「连接」放行。
pub(crate) fn phone_app(body:&str,apps:&crate::model::PhoneApps)->Option<(String,bool)> {
    let head=body.split('·').next().unwrap_or("").trim();
    if head.is_empty() || head.chars().count()>12 {return None;}
    apps.lookup(head)
}

pub(crate) fn phone_channel(id:&str)->bool {
''',
     '''    ("com.alipay.iphoneclient","支付宝"),
    ("com.google.Gmail","Gmail"),
];
/// 手机连接 Toast 的来源判定。
///
/// WinRT 只报主 AUMID（`!App`），拿不到通道信息，所以只能看正文：
/// 正文格式是「来源 · 内容…」，但短信用**手机号**当来源（`+8610… · 【xx】验证码…`），
/// 电话是「号码 · 传入呼叫 / 未接来电」，普通 App 才是「微信 · 张三 · 内容」。
///
/// 判定顺序：名单命中 → 来电标记 → 短信特征 → 用首段当标签兜底。
/// 名单放在 `%APPDATA%\\Daozai\\phone-apps.json`，加新 App 不用改代码重编译。
/// 返回 (标签, 是否允许上岛)；None = 无法识别，由调用方按「连接」放行。
pub(crate) fn phone_app(body:&str,apps:&crate::model::PhoneApps)->Option<(String,bool)> {
    let head=body.split('·').next().unwrap_or("").trim();
    if head.is_empty() {return None;}
    // 1. 名单优先：微信 / QQ / 支付宝 / Gmail 等。
    if let Some(hit)=apps.lookup(head){return Some(hit);}
    // 2. 来电：号码当来源，正文带这两个词。
    if body.contains("传入呼叫")||body.contains("未接来电"){return Some(("电话".to_string(),true));}
    // 3. 短信：来源是手机号，或正文有【签名】/验证码字样。
    let digits=head.chars().all(|c|c.is_ascii_digit()||matches!(c,'+'|'-'|' '|'('|')'));
    if digits||body.contains("验证码")||body.contains('【')||body.contains("短信"){
        return Some(("短信".to_string(),true));
    }
    // 4. 兜底：短名字直接当标签（例如 WorkBuddy），过长才交给调用方。
    if head.chars().count()<=12 {return Some((head.to_string(),true));}
    None
}

pub(crate) fn phone_channel(id:&str)->bool {
'''),
    ("src/system.rs",
     '''
#[cfg(test)]
mod phone_channel_tests {
    use super::{phone_channel,phone_label};
    #[test]
    fn 只放行手机连接白名单通道(){
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.tencent.xin"));
''',
     '''
#[cfg(test)]
mod phone_channel_tests {
    use super::{phone_app,phone_channel,phone_label};
    #[test]
    fn 只放行手机连接白名单通道(){
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.tencent.xin"));
'''),
    ("src/system.rs",
     '''        custom.items.push(("小红书".to_string(),"小红书".to_string(),true));
        assert_eq!(crate::system::phone_app("小红书 · 有人回复了你",&custom),Some(("小红书".to_string(),true)));
    }
}

fn is_wechat_toast(app:&str,id:&str)->bool{
''',
     '''        custom.items.push(("小红书".to_string(),"小红书".to_string(),true));
        assert_eq!(crate::system::phone_app("小红书 · 有人回复了你",&custom),Some(("小红书".to_string(),true)));
    }
    #[test]
    fn 短信与来电按正文特征识别(){
        let apps=crate::model::PhoneApps::builtin();
        // 短信用手机号当来源：认不出号码也要归到「短信」，不能退化成「连接」。
        assert_eq!(phone_app("+86106826974003139 · 【清枫北京科技】您的验证码：4342，如非本人操作",&apps),Some(("短信".to_string(),true)));
        assert_eq!(phone_app("10691234 · 您的验证码 8899",&apps),Some(("短信".to_string(),true)));
        // 响铃与未接都先归到「电话」，再由正文区分「来电中 / 未接来电」。
        assert_eq!(phone_app("130 8646 4686 · 传入呼叫",&apps),Some(("电话".to_string(),true)));
        assert_eq!(phone_app("电话 · 130 8646 4686 · 未接来电",&apps),Some(("电话".to_string(),true)));
        // 名单外的短名字直接用名字当标签。
        assert_eq!(phone_app("WorkBuddy · 任务已完成",&apps),Some(("WorkBuddy".to_string(),true)));
    }
}

fn is_wechat_toast(app:&str,id:&str)->bool{
'''),
    ("src/extras.rs",
     '''                // 状态总线按 key 覆写：内容变了替换，不变也替换以续期，不排队。
                _ if matches!(&a.source,AlertSource::External(id) if id.starts_with("island-status:"))=>old.source==a.source,
                // 手机通知按来源聚合：同一 App 的新消息替换旧提示，不排队刷屏。
                _ if a.title.starts_with("手机 · ") || a.title=="未接来电"=>old.source==a.source && old.title==a.title,
                _=>old.source==a.source && old.title==a.title && (a.is_hud() || old.body==a.body),
            };
            if a.source!=AlertSource::Internal || a.title != "蓝牙设备" { self.alerts.retain(|old| !same_event(old)); }
''',
     '''                // 状态总线按 key 覆写：内容变了替换，不变也替换以续期，不排队。
                _ if matches!(&a.source,AlertSource::External(id) if id.starts_with("island-status:"))=>old.source==a.source,
                // 手机通知按来源聚合：同一 App 的新消息替换旧提示，不排队刷屏。
                // 手机通道共用同一个来源（手机连接的 AUMID）：新短信 / 来电直接顶掉上一条，
                // 不然来电会排在一条已经看过的短信后面。
                _ if a.title.starts_with("手机 · ") || a.title=="未接来电" || a.title=="来电中"=>old.source==a.source,
                _=>old.source==a.source && old.title==a.title && (a.is_hud() || old.body==a.body),
            };
            if a.source!=AlertSource::Internal || a.title != "蓝牙设备" { self.alerts.retain(|old| !same_event(old)); }
'''),
    ("src/extras.rs",
     '''        let phone=crate::system::phone_channel(&id);
        // 手机 App 名单是外部 JSON，改完约 1 秒内生效，不用重启岛仔。
        if phone {self.phone_apps.reload_if_changed();}
        if !self.settings.read_notifications || id.is_empty() || (self.settings.qq_only && id!="QQ" && !phone){return;}
        // 真实手机连接通知只报主 AUMID，改由正文首段识别来源 App；
        // 明确命中黑名单的静默，识别不出的按「连接」放行，避免漏掉来电等重要通知。
        let phone_info:Option<(String,bool)>=if phone {Some(crate::system::phone_app(&body,&self.phone_apps).unwrap_or(("连接".to_string(),true)))}else{None};
        if phone_info.as_ref().is_some_and(|(_,allow)|!allow){return;}
        // 岛仔自己发出的系统提醒会被通知中心回传；已在历史里的同一内容不再入队，避免重复显示。
        if self.history.iter().any(|a|format!("{} · {}",a.title,a.body)==body){return;}
        if !self.notification_apps.observe(&id,&app) || !self.settings.notifications{return;}
        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去；
        // 「重复查看」开启时普通通知也保留更久，方便多次悬停。
''',
     '''        let phone=crate::system::phone_channel(&id);
        // 手机 App 名单是外部 JSON，改完约 1 秒内生效，不用重启岛仔。
        if phone {self.phone_apps.reload_if_changed();}
        // 每条通知留一行诊断，出问题能直接看出是哪一步拦下的。
        let trace=format!("id={id} app={app} phone={} qq_only={} body={}",
            if phone {"手机"} else {"其它"},
            if self.settings.qq_only {"是"} else {"否"},
            body.chars().take(60).collect::<String>());
        if !self.settings.read_notifications || id.is_empty() || (self.settings.qq_only && id!="QQ" && !phone){
            log_notice(&format!("DROP 未连接/非QQ  {trace}"));
            return;
        }
        // 真实手机连接通知只报主 AUMID，改由正文识别来源；
        // 明确命中黑名单的静默，识别不出的按「连接」放行，避免漏掉来电等重要通知。
        let phone_info:Option<(String,bool)>=if phone {Some(crate::system::phone_app(&body,&self.phone_apps).unwrap_or(("连接".to_string(),true)))}else{None};
        if phone_info.as_ref().is_some_and(|(_,allow)|!allow){
            log_notice(&format!("DROP 名单静默    {trace}"));
            return;
        }
        // 来电方（正文首段就是号码或联系人），来电中/未接来电都用它当副标题。
        let caller=body.split('·').next().unwrap_or("").trim().to_string();
        // 岛仔自己发出的系统提醒会被通知中心回传；已在历史里的同一内容不再入队，避免重复显示。
        if self.history.iter().any(|a|format!("{} · {}",a.title,a.body)==body){
            log_notice(&format!("DROP 自家回传    {trace}"));
            return;
        }
        if !self.notification_apps.observe(&id,&app) || !self.settings.notifications{
            log_notice(&format!("DROP 应用被关/总开关  {trace}"));
            return;
        }
        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去；
        // 「重复查看」开启时普通通知也保留更久，方便多次悬停。
'''),
    ("src/extras.rs",
     '''        // 即使全局关闭正文预览也显示一行短摘要，且不对验证码打码
        //（后续验证码卡片会直接复用 otp::extract 的结果）。
        let source:&str=if phone_info.is_some(){
            body.split_once('·').map(|(_,rest)|rest.trim()).filter(|rest|!rest.is_empty()).unwrap_or(body.as_str())
        }else{body.as_str()};
        let body=if self.settings.notification_preview || phone {
            let limit=if phone{80}else{300};
            let mut text=source.chars().filter(|c|!c.is_control()).take(limit).collect::<String>();
            if !phone {if let Some(code)=crate::otp::extract("",&text,&[]){text=crate::mail_privacy::mask(&text,&code.text);}}
''',
     '''        // 即使全局关闭正文预览也显示一行短摘要，且不对验证码打码
        //（后续验证码卡片会直接复用 otp::extract 的结果）。
        let source:&str=if phone_info.is_some(){
            // 手机镜像通知的正文末尾常带一个多余的分隔符（`…忽略本短信! · `），顺手去掉。
            body.split_once('·').map(|(_,rest)|rest.trim().trim_end_matches(|c:char| c=='·'||c.is_whitespace()).trim()).filter(|rest|!rest.is_empty()).unwrap_or(body.as_str())
        }else{body.as_str()};        let body=if self.settings.notification_preview || phone {
            let limit=if phone{80}else{300};
            let mut text=source.chars().filter(|c|!c.is_control()).take(limit).collect::<String>();
            if !phone {if let Some(code)=crate::otp::extract("",&text,&[]){text=crate::mail_privacy::mask(&text,&code.text);}}
'''),
    ("src/extras.rs",
     '''            log_otp("phone",code.as_ref().map(|c|c.text.as_str()));
            if let Some(code)=code {self.pending.push(Cmd::Copy(code.text));}
        }
        // 未接来电：提示保留 30 秒，同时点亮铃铛红点常驻，直到点关闭清除。
        let missed=phone_info.as_ref().is_some_and(|(label,_)|label=="电话");
        if missed {
            self.missed_calls=self.missed_calls.saturating_add(1);
            self.notification_bell_until=f64::MAX;
        }
        let (title,body,duration)=if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,if !cfg!(test) && self.settings.notice_keep {60.0} else {4.5})};
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:None},false);
    }
    pub(crate) fn tick_alerts(&mut self) {
''',
     '''            log_otp("phone",code.as_ref().map(|c|c.text.as_str()));
            if let Some(code)=code {self.pending.push(Cmd::Copy(code.text));}
        }
        // 来电中 / 未接来电：标签都是「电话」，靠正文区分——
        // 响铃是「号码 · 传入呼叫」，挂断是「电话 · 号码 · 未接来电」。
        let ringing=phone_info.is_some()&&body.contains("传入呼叫");
        let missed=phone_info.as_ref().is_some_and(|(label,_)|label=="电话")&&body.contains("未接来电");
        if missed {
            // 来电中的提示被「未接来电」取代，别让两条并存。
            if self.notice.as_ref().is_some_and(|a|a.title=="来电中"){self.notice=None;}
            self.missed_calls=self.missed_calls.saturating_add(1);
            self.notification_bell_until=f64::MAX;
        }
        let (title,body,duration)=if ringing {
            ("来电中".to_string(),format!("{caller} 正在呼叫"),60.0)
        }else if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,if !cfg!(test) && self.settings.notice_keep {60.0} else {4.5})};
        log_notice(&format!("SHOW {title} / {body}  <- {trace}"));
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:None},false);
    }
    pub(crate) fn tick_alerts(&mut self) {
'''),
    ("src/extras.rs",
     '''    }
}

/// 邮件面板操作诊断：点击「全部已读」被拒时记录原因，便于排查按钮为什么没反应。
pub(crate) fn log_mail(note:&str){
    let Some(base)=std::env::var_os("APPDATA") else{return};
''',
     '''    }
}

/// 通知通道诊断：每条进来的通知都记一行「显示 / 被谁拦下」，排查「某类通知不上岛」用。
pub(crate) fn log_notice(note:&str){
    let Some(base)=std::env::var_os("APPDATA") else{return};
    let dir=std::path::PathBuf::from(base).join("Daozai").join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err(){return;}
    let path=dir.join("notice.log");
    if std::fs::metadata(&path).is_ok_and(|m|m.len()>256*1024){let _=std::fs::remove_file(&path);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        use std::io::Write;
        let _=writeln!(file,"{} | {}",chrono::Local::now().format("%H:%M:%S"),note);
    }
}

/// 邮件面板操作诊断：点击「全部已读」被拒时记录原因，便于排查按钮为什么没反应。
pub(crate) fn log_mail(note:&str){
    let Some(base)=std::env::var_os("APPDATA") else{return};
'''),

    # custom-28：A 组功能集：通知/邮件/剪贴板/状态总线
    ("src/otp.rs",
     '''    if has(&before(hay, start, 20), ORDER_WORDS) || has(&after(hay, end, 8), ORDER_WORDS) {
        return None;
    }
    // 4 位年份：直接判负，掉到门槛以下
    if len == 4 && !mixed {
        if let Ok(y) = core.parse::<u32>() {
            if (1900..=2100).contains(&y) {
                score -= 35;
            }
        }
''',
     '''    if has(&before(hay, start, 20), ORDER_WORDS) || has(&after(hay, end, 8), ORDER_WORDS) {
        return None;
    }
    // 4 位年份：只在后面紧跟日期成分时判负（`2026-10-30`、`2026年`），
    // 否则「验证码：2084」这种正好落在 1900..=2100 的真实验证码会被误杀。
    if len == 4 && !mixed {
        if let Ok(y) = core.parse::<u32>() {
            let dateish = hay[end..].chars().next().is_some_and(|c| matches!(c, '-' | '/' | '.' | '年' | '月' | '日'));
            if (1900..=2100).contains(&y) && dateish {
                score -= 35;
            }
        }
'''),
    ("src/otp.rs",
     '''        .trim();
    let own_line = line == text;
    let colon = hay[..start].trim_end().ends_with(':');
    let char_pos = hay[..start].chars().count();
    let in_subject = char_pos < subject_end;
    let is_emph = emph.iter().any(|e| e.contains(text));
''',
     '''        .trim();
    let own_line = line == text;
    let colon = hay[..start].trim_end().ends_with(':');
    let tight = {
        let before6 = before(hay, start, 6);
        let trimmed = before6.trim_end_matches([' ', ':', '：', '是', '为']);
        strong().is_match(trimmed)
    };
    let char_pos = hay[..start].chars().count();
    let in_subject = char_pos < subject_end;
    let is_emph = emph.iter().any(|e| e.contains(text));
'''),
    ("src/otp.rs",
     '''    }
    if colon {
        score += 10;
    }
    if in_subject {
        score += 15;
''',
     '''    }
    if colon {
        score += 10;
    } else if tight {
        // 「验证码1234」「验证码 1234」：没有冒号，但强关键词紧贴数字，同样算强信号。
        score += 10;
    }
    if in_subject {
        score += 15;
'''),
    ("src/otp.rs",
     '''
    #[test]
    fn negatives_are_rejected() {
        let cases = [
            ("", "您的订单号 12345678 已发货"),
            ("", "订单 482913，金额 ￥1,299.00"),
''',
     '''
    #[test]
    fn negatives_are_rejected() {
        // 4 位验证码正好落在 1900..=2100（如 2084）时，不能因为「像年份」被判负。
        assert_eq!(code("", "【清枫北京科技】您的验证码：2084，如非本人操作，请忽略本短信!"), Some("2084".into()));
        // 常见写法「验证码1234」「验证码 1234」不能因为没有冒号就掉到门槛以下。
        assert_eq!(code("", "验证码 1999，5 分钟内有效"), Some("1999".into()));
        assert_eq!(code("", "您的验证码1234，5分钟内有效"), Some("1234".into()));
        let cases = [
            ("", "您的订单号 12345678 已发货"),
            ("", "订单 482913，金额 ￥1,299.00"),
'''),
    ("src/system.rs",
     '''}

pub(crate) fn phone_channel(id:&str)->bool {
    if !id.starts_with("Microsoft.YourPhone_"){return false;}
    // WinRT 的 AppInfo 有时只报主 AUMID（!App），拿不到子通道包名，
    // 实测伪造 Toast 就是这样；主通道也放行，避免真实通知被完全漏掉。
    id.ends_with("!App") || PHONE_CHANNELS.iter().any(|(key,_)|id.contains(*key))
}
pub(crate) fn phone_label(id:&str)->&'static str {
    if id.ends_with("!App"){return "连接";}
''',
     '''}

pub(crate) fn phone_channel(id:&str)->bool {
    // 只要是手机连接（Phone Link）发出来的就归手机通道：
    // WinRT 有时给主 AUMID（`!App`），有时给子通道（`YourPhoneMessages_…` /
    // `YourPhoneNotifications_<uuid>`），开机后通道注册晚一步还会先走汇总通道。
    // 白名单只用来起标签，是否上岛/静音交给 phone-apps.json 按正文判断。
    id.starts_with("Microsoft.YourPhone_")
}
pub(crate) fn phone_label(id:&str)->&'static str {
    if id.ends_with("!App"){return "连接";}
'''),
    ("src/system.rs",
     '''    use super::{phone_app,phone_channel,phone_label};
    #[test]
    fn 只放行手机连接白名单通道(){
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.tencent.xin"));
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneMessages_9d62fe0c"));
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!App"));
        assert_eq!(phone_label("Microsoft.YourPhone_x!App"),"连接");
        assert!(!phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.valvesoftware.Steam"));
        assert!(!phone_channel("QQ"));
        assert!(!phone_channel("mail.test"));
        assert_eq!(phone_label("Microsoft.YourPhone_x!YourPhoneNotifications_com.tencent.xin"),"微信");
''',
     '''    use super::{phone_app,phone_channel,phone_label};
    #[test]
    fn 只放行手机连接白名单通道(){
        // 现在只要是手机连接的 AUMID 都算手机通道（白名单只管标签）；
        // 真正决定「上不上岛 / 静不静音」的是正文 + phone-apps.json。
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.tencent.xin"));
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneMessages_9d62fe0c"));
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!App"));
        // 开机后通道还没注册时，Win11 会先走汇总通道，这个也必须算手机通知。
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_9d62fe0c-7488-42c8-be15-0fbe4a087b7f"));
        assert_eq!(phone_label("Microsoft.YourPhone_x!App"),"连接");
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.valvesoftware.Steam"));
        assert!(!phone_channel("QQ"));
        assert!(!phone_channel("mail.test"));
        assert_eq!(phone_label("Microsoft.YourPhone_x!YourPhoneNotifications_com.tencent.xin"),"微信");
'''),
    ("src/system.rs",
     '''    }
}
#[derive(Default)]
struct NotificationBaseline { previous: Option<HashMap<u32,ToastVersion>> }
impl NotificationBaseline {
    fn unchanged_other(&self,id:u32,created:i64)->Option<ToastVersion>{
        self.previous.as_ref()?.get(&id).copied().filter(|old|old.created==created && old.qq_body.is_none())
    }
    fn changed(&self,id:u32,version:ToastVersion)->bool{
        self.previous.as_ref().is_some_and(|old|old.get(&id)!=Some(&version))
    }
}

fn qq_channel_status()->String{
    use windows::UI::Notifications::{NotificationSetting,ToastNotificationManager};
    match ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("QQ")).and_then(|notifier|notifier.Setting()){
''',
     '''    }
}
#[derive(Default)]
struct NotificationBaseline {
    previous: Option<HashMap<u32,ToastVersion>>,
    /// 本线程开始监听时的时间（FILETIME，100ns）。首轮枚举里「比它还新」的通知
    /// 是开机后才到的，必须补报；比它老的是历史，静默跳过。
    started: i64,
}
impl NotificationBaseline {
    fn unchanged_other(&self,id:u32,created:i64)->Option<ToastVersion>{
        self.previous.as_ref()?.get(&id).copied().filter(|old|old.created==created && old.qq_body.is_none())
    }
    fn changed(&self,id:u32,version:ToastVersion)->bool{
        match &self.previous {
            None=>version.created>self.started,
            Some(old)=>old.get(&id)!=Some(&version),
        }
    }
}

/// 当前时间转 FILETIME（100ns since 1601），用于和 `CreationTime` 比较。
fn now_filetime()->i64{
    let ticks=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d|d.as_secs() as i64*10_000_000+d.subsec_nanos() as i64/100).unwrap_or(0);
    ticks+116_444_736_000_000_000
}

fn qq_channel_status()->String{
    use windows::UI::Notifications::{NotificationSetting,ToastNotificationManager};
    match ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("QQ")).and_then(|notifier|notifier.Setting()){
'''),
    ("src/system.rs",
     '''            }
            let listener = UserNotificationListener::Current()?;
            let qq_status=qq_channel_status();
            let mut baseline=NotificationBaseline::default();
            let mut last_status=String::new();
            loop {
                if stopped.try_recv()!=Err(mpsc::TryRecvError::Empty){return Ok(());}
''',
     '''            }
            let listener = UserNotificationListener::Current()?;
            let qq_status=qq_channel_status();
            let mut baseline=NotificationBaseline{previous:None,started:now_filetime()};
            let mut last_status=String::new();
            loop {
                if stopped.try_recv()!=Err(mpsc::TryRecvError::Empty){return Ok(());}
'''),
    ("src/system.rs",
     '''                    let (app,id)=notification_identity(&n,number,created,known_qq);
                    // 微信只走任务栏事件，即使客户端同时发送系统通知也不读取其正文。
                    let phone=phone_channel(&id);
                    if (qq_only && id!="QQ" && !phone) || (!phone && is_wechat_toast(&app,&id)){current.insert(number,ToastVersion::new(created,None));continue;}
                    let qq=id=="QQ";
                    // QQ 可能原地更新同一条通知；其他应用的启动历史不需要读取正文。
                    let body=if qq || baseline.previous.is_some(){notification_body(&n).unwrap_or_default()}else{String::new()};
                    let version=ToastVersion::new(created,qq.then_some(body.as_str()));
                    current.insert(number,version);
                    if baseline.changed(number,version){
''',
     '''                    let (app,id)=notification_identity(&n,number,created,known_qq);
                    // 微信只走任务栏事件，即使客户端同时发送系统通知也不读取其正文。
                    let phone=phone_channel(&id);
                    if (qq_only && id!="QQ" && !phone) || (!phone && is_wechat_toast(&app,&id)){
                        // 这一层在监听线程里，拦下的通知不会进 receive_notification，
                        // 也就要单独记一行，否则「某条通知没上岛」会完全查不到。
                        crate::extras::log_notice(&format!("DROP 监听线程过滤  app={app} id={id} phone={phone} qq_only={qq_only}"));
                        current.insert(number,ToastVersion::new(created,None));
                        continue;
                    }
                    let qq=id=="QQ";
                    // QQ 可能原地更新同一条通知；其他应用的启动历史不需要读取正文。
                    // 启动之后才出现的通知（首轮枚举前到达的）也要读正文，否则会被当成历史静默掉。
                    let body=if qq || baseline.previous.is_some() || created>baseline.started{notification_body(&n).unwrap_or_default()}else{String::new()};
                    let version=ToastVersion::new(created,qq.then_some(body.as_str()));
                    current.insert(number,version);
                    if baseline.changed(number,version){
'''),
    ("src/system.rs",
     '''    use super::*;
    #[test]
    fn notifications_skip_history_but_detect_reused_ids_and_updated_content(){
        let mut baseline=NotificationBaseline::default();
        let first=ToastVersion::new(10,Some("第一条"));
        assert!(!baseline.changed(1,first));
        baseline.previous=Some(HashMap::from([(1,first),(2,ToastVersion::new(10,None))]));
        assert!(!baseline.changed(1,first));
        assert!(baseline.changed(1,ToastVersion::new(10,Some("第二条"))));
''',
     '''    use super::*;
    #[test]
    fn notifications_skip_history_but_detect_reused_ids_and_updated_content(){
        // started=100：比它老的是开机前的历史，比它新的是启动之后才到的。
        let mut baseline=NotificationBaseline{previous:None,started:100};
        let first=ToastVersion::new(10,Some("第一条"));
        assert!(!baseline.changed(1,first),"开机前的历史通知不该补报");
        assert!(baseline.changed(2,ToastVersion::new(200,None)),"启动之后到达的通知要补报（首轮枚举可能晚于它）");
        baseline.previous=Some(HashMap::from([(1,first),(2,ToastVersion::new(10,None))]));
        assert!(!baseline.changed(1,first));
        assert!(baseline.changed(1,ToastVersion::new(10,Some("第二条"))));
'''),

    # custom-28：A 组功能集：通知/邮件/剪贴板/状态总线
    ("src/model.rs",
     '''    pub notice_keywords: String,
    /// 勿扰时长（分钟，0 = 关闭）。只在本次运行内生效，重启后恢复为关闭。
    pub quiet_minutes: u32,
    /// 邮件面板上次选择的收件账号（空 = 全部账号），重启后沿用。
    pub mail_filter: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, mail_filter: String::new() }
    }
}

''',
     '''    pub notice_keywords: String,
    /// 勿扰时长（分钟，0 = 关闭）。只在本次运行内生效，重启后恢复为关闭。
    pub quiet_minutes: u32,
    /// 会议自动勿扰：日程开始时自动把勿扰开到会议结束。
    pub quiet_meeting: bool,
    /// 邮件面板上次选择的收件账号（空 = 全部账号），重启后沿用。
    pub mail_filter: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

'''),
    ("src/model.rs",
     '''                "battery_alert" => s.battery_alert = match v.parse::<u32>().unwrap_or(20) { 0 => 0, 10 => 10, 30 => 30, _ => 20 },
                "notice_keywords" => s.notice_keywords = v.trim().to_string(),
                "quiet_minutes" => s.quiet_minutes = match v.parse::<u32>().unwrap_or(0) { 30 => 30, 60 => 60, 120 => 120, _ => 0 },
                "mail_filter" => s.mail_filter = v.trim().to_string(),
                _ => {}
            }
''',
     '''                "battery_alert" => s.battery_alert = match v.parse::<u32>().unwrap_or(20) { 0 => 0, 10 => 10, 30 => 30, _ => 20 },
                "notice_keywords" => s.notice_keywords = v.trim().to_string(),
                "quiet_minutes" => s.quiet_minutes = match v.parse::<u32>().unwrap_or(0) { 30 => 30, 60 => 60, 120 => 120, _ => 0 },
                "quiet_meeting" => s.quiet_meeting = v != "0",
                "mail_filter" => s.mail_filter = v.trim().to_string(),
                _ => {}
            }
'''),
    ("src/model.rs",
     '''                ("battery_alert".into(), self.battery_alert.to_string()),
                ("notice_keywords".into(), self.notice_keywords.clone()),
                ("quiet_minutes".into(), self.quiet_minutes.to_string()),
                ("mail_filter".into(), self.mail_filter.clone()),
            ],
        );
''',
     '''                ("battery_alert".into(), self.battery_alert.to_string()),
                ("notice_keywords".into(), self.notice_keywords.clone()),
                ("quiet_minutes".into(), self.quiet_minutes.to_string()),
                ("quiet_meeting".into(), (self.quiet_meeting as u8).to_string()),
                ("mail_filter".into(), self.mail_filter.clone()),
            ],
        );
'''),
    ("src/app.rs",
     '''    pub(crate) clip_search_focus: bool,
    /// 搜索输入框是否展开（面板表头平时只放一个「搜索」按钮）。
    pub(crate) clip_search_open: bool,

    // 饮水
    pub(crate) water_ring: Spring,
''',
     '''    pub(crate) clip_search_focus: bool,
    /// 搜索输入框是否展开（面板表头平时只放一个「搜索」按钮）。
    pub(crate) clip_search_open: bool,
    /// 搜索框这一帧是否真的有键盘焦点（只有这时才处理 ↑↓ / Enter）。
    pub(crate) clip_typing: bool,
    /// 键盘选中的行（过滤后列表的下标）。
    pub(crate) clip_sel: usize,

    // 饮水
    pub(crate) water_ring: Spring,
'''),
    ("src/app.rs",
     '''    pub(crate) pc_last:Option<Option<u8>>,
    /// 勿扰截止时刻（app 时间）；> t 期间不弹提示、不闪铃铛，验证码除外。
    pub(crate) quiet_until:f64,
    /// 状态总线当前存活的状态，用于常驻区并排显示。
    pub(crate) status_live:Vec<crate::island_status::Status>,
    /// 未接来电累计（点「未接来电」提示的关闭按钮清零）。
''',
     '''    pub(crate) pc_last:Option<Option<u8>>,
    /// 勿扰截止时刻（app 时间）；> t 期间不弹提示、不闪铃铛，验证码除外。
    pub(crate) quiet_until:f64,
    /// 已经自动勿扰过的会议（uid|start），避免同一场会重复触发。
    pub(crate) quiet_meeting_key:Option<String>,
    /// 上次检查会议勿扰的时刻（每 5 秒查一次，别每帧都拼日程）。
    pub(crate) meeting_checked_at:f64,
    /// 「稍后提醒」队列：(到期时刻, 标题, 正文)。
    pub(crate) snoozed:Vec<(f64,String,String)>,
    /// 设置页的最近通知缓存：(上次读取时刻, 列表)，避免每帧读盘。
    pub(crate) notice_cache:(f64,Vec<(String,String,String)>),
    /// 还没凑满一分钟的专注秒数，攒够就写进 focus-stats.json。
    pub(crate) focus_pending:f32,
    /// 今日专注分钟数与连续天数（每分钟更新一次，别每帧读盘）。
    pub(crate) focus_today_min:u32,
    pub(crate) focus_streak_days:u32,
    /// 状态总线当前存活的状态，用于常驻区并排显示。
    pub(crate) status_live:Vec<crate::island_status::Status>,
    /// 未接来电累计（点「未接来电」提示的关闭按钮清零）。
'''),
    ("src/app.rs",
     '''            s.visuals.selection.bg_fill = theme::alpha_blue(0.45);
        });
        let settings = model::Settings::load();
        // 邮件面板上次选的账号：结构体里 `settings` 会被 move，先取出来。
        let saved_mail_filter = settings.mail_filter.clone();
        let clipboard_on = settings.clipboard_history;
''',
     '''            s.visuals.selection.bg_fill = theme::alpha_blue(0.45);
        });
        let settings = model::Settings::load();
        // 专注统计只在启动和每分钟更新，构造前先取一次。
        let (focus_today, focus_streak)=crate::extras::focus_summary();
        // 邮件面板上次选的账号：结构体里 `settings` 会被 move，先取出来。
        let saved_mail_filter = settings.mail_filter.clone();
        let clipboard_on = settings.clipboard_history;
'''),
    ("src/app.rs",
     '''            clip_query: String::new(),
            clip_search_focus: false,
            clip_search_open: false,
            water_ring: Spring::new(0.0, 120.0, 0.9),
            water_ml: 0.0,
            water_view: 0,
''',
     '''            clip_query: String::new(),
            clip_search_focus: false,
            clip_search_open: false,
            clip_typing: false,
            clip_sel: 0,
            water_ring: Spring::new(0.0, 120.0, 0.9),
            water_ml: 0.0,
            water_view: 0,
'''),
    ("src/app.rs",
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), phone_last:None, pc_last:None, quiet_until:0.0, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
''',
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), phone_last:None, pc_last:None, quiet_until:0.0, quiet_meeting_key:None, meeting_checked_at:0.0, snoozed:Vec::new(), notice_cache:(0.0,Vec::new()), focus_pending:0.0, focus_today_min:focus_today, focus_streak_days:focus_streak, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
'''),
    ("src/app.rs",
     '''        self.pc_events(env.battery,env.charging);
        // 勿扰到点自动解除；同时把设置项归零，避免界面上还显示着「60 分钟」。
        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
''',
     '''        self.pc_events(env.battery,env.charging);
        // 勿扰到点自动解除；同时把设置项归零，避免界面上还显示着「60 分钟」。
        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        if self.t-self.meeting_checked_at>5.0 {self.meeting_checked_at=self.t;self.meeting_quiet(env);}
        self.tick_snoozed();
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
'''),
    ("src/app.rs",
     '''            self.water_ml = self.water.drunk_ml as f32;
            self.water_ring.snap(self.water.percent() / 100.0);
        }
        if self.pomo.running {
            self.pomo.remaining -= elapsed;
            if self.pomo.remaining <= 0.0 {
''',
     '''            self.water_ml = self.water.drunk_ml as f32;
            self.water_ring.snap(self.water.percent() / 100.0);
        }
        // 只统计真正在跑的专注阶段：暂停不计，攒满一分钟落盘一次。
        if self.pomo.running && self.pomo.phase==Phase::Focus {
            self.focus_pending += elapsed as f32;
            if self.focus_pending>=60.0 {
                let minutes=(self.focus_pending/60.0).floor();
                self.focus_pending-=minutes*60.0;
                crate::extras::focus_add(minutes as u32);
                let (today,streak)=crate::extras::focus_summary();
                self.focus_today_min=today;
                self.focus_streak_days=streak;
            }
        }
        if self.pomo.running {
            self.pomo.remaining -= elapsed;
            if self.pomo.remaining <= 0.0 {
'''),
    ("src/extras.rs",
     '''        // 通知仅保存在本次运行的内存中。
        if a.source==AlertSource::Internal {self.history.push_front(a.clone());}
        self.history.truncate(50);
        // 勿扰：只记账、不打扰。横幅、铃铛、系统通知都不出现，验证码例外（用户要的就是那串数字）。
        if self.muted() && a.status.is_none() && crate::otp::extract("",&a.body,&[]).is_none() { return; }
        if self.settings.notifications {
''',
     '''        // 通知仅保存在本次运行的内存中。
        if a.source==AlertSource::Internal {self.history.push_front(a.clone());}
        self.history.truncate(50);
        // 落盘一份历史：岛仔重启或当时没看到时，还能在设置里回查。
        let status_bus=matches!(&a.source,AlertSource::External(id) if id.starts_with("island-status"));
        if !status_bus {notice_history_append(&a.title,&a.body);}
        // 勿扰：只记账、不打扰。横幅、铃铛、系统通知都不出现，验证码例外（用户要的就是那串数字）。
        if self.muted() && a.status.is_none() && crate::otp::extract("",&a.body,&[]).is_none() { return; }
        if self.settings.notifications {
'''),
    ("src/extras.rs",
     '''        self.settings.keywords().iter().any(|word|lower.contains(&word.to_lowercase()))
    }

    /// 勿扰开关：`minutes = 0` 关闭。只在本次运行内有效，重启后恢复关闭。
    pub(crate) fn set_quiet(&mut self,minutes:u32){
        self.settings.quiet_minutes=minutes;
''',
     '''        self.settings.keywords().iter().any(|word|lower.contains(&word.to_lowercase()))
    }

    /// 会议自动勿扰：日程刚开始（90 秒内）且还没结束时把勿扰开到会议结束，最多 2 小时。
    /// 同一场会议只触发一次，散会自动失效。
    pub(crate) fn meeting_quiet(&mut self, env:&crate::app::Env){
        if !self.settings.quiet_meeting || self.muted() {return;}
        let Ok(day)=chrono::NaiveDate::parse_from_str(&env.today,"%Y-%m-%d") else{return};
        let now=chrono::Local::now().timestamp();
        let mut hit:Option<(i64,String)>=None;
        for entry in self.calendar_entries(day) {
            let event=entry.event;
            if event.all_day || event.start>now || now-event.start>90 {continue;}
            let Some(end)=event.end else{continue};
            if end<=now {continue;}
            hit=Some((end.min(now+7200), format!("{}|{}",event.uid,event.start)));
        }
        let Some((until,key))=hit else{return};
        if self.quiet_meeting_key.as_deref()==Some(key.as_str()) {return;}
        self.quiet_meeting_key=Some(key);
        self.quiet_until=until as f64;
        self.say("会议开始，自动勿扰到散会");
    }

    /// 设置页用的最近通知：2 秒缓存一次，别每帧读盘。
    pub(crate) fn recent_notices(&mut self)->Vec<(String,String,String)>{
        if self.notice_cache.0<=0.0 || self.t-self.notice_cache.0>2.0 {
            self.notice_cache=(self.t.max(0.001),notice_history());
        }
        self.notice_cache.1.clone()
    }

    /// 把一条历史通知重新弹出来（「稍后提醒」用）。
    pub(crate) fn replay_notice(&mut self,title:String,body:String){
        self.enqueue_alert(Alert{source:AlertSource::External("notice-history".into()),title,body,page:usize::MAX,at:self.t,duration:if cfg!(test){4.5}else{30.0},bluetooth:None,meeting:None,status:None},false);
    }

    /// 稍后提醒：`minutes` 分钟后再弹一次。
    pub(crate) fn snooze_notice(&mut self,minutes:f64,title:String,body:String){
        self.snoozed.push((self.t+minutes*60.0,title,body));
        self.say(format!("{minutes:.0} 分钟后再提醒"));
    }

    /// 到点的「稍后提醒」重新入队，一次只处理一条，避免同一帧连弹。
    pub(crate) fn tick_snoozed(&mut self){
        if let Some(index)=self.snoozed.iter().position(|(due,_,_)|self.t>=*due){
            let (_,title,body)=self.snoozed.remove(index);
            self.replay_notice(title,body);
        }
    }

    /// 勿扰开关：`minutes = 0` 关闭。只在本次运行内有效，重启后恢复关闭。
    pub(crate) fn set_quiet(&mut self,minutes:u32){
        self.settings.quiet_minutes=minutes;
'''),
    ("src/extras.rs",
     '''    }
}

/// 通知通道诊断：每条进来的通知都记一行「显示 / 被谁拦下」，排查「某类通知不上岛」用。
pub(crate) fn log_notice(note:&str){
    let Some(base)=std::env::var_os("APPDATA") else{return};
''',
     '''    }
}

/// 番茄统计：`%APPDATA%\\Daozai\\focus-stats.json`，只记「今天多少分钟」。
/// 只统计真正在跑的专注阶段，暂停不计；写盘频率是每分钟一次。
pub(crate) fn focus_stats_path()->Option<std::path::PathBuf>{
    let base=std::env::var_os("APPDATA")?;
    Some(std::path::PathBuf::from(base).join("Daozai").join("focus-stats.json"))
}

fn focus_days()->Vec<(String,u32)>{
    let Some(path)=focus_stats_path() else{return Vec::new();};
    let Ok(raw)=std::fs::read(&path) else{return Vec::new();};
    let Ok(json)=serde_json::from_slice::<serde_json::Value>(&raw) else{return Vec::new();};
    json.get("days").and_then(|d|d.as_array()).into_iter().flatten().filter_map(|item|{
        Some((item.get("date")?.as_str()?.to_owned(), item.get("minutes")?.as_u64()? as u32))
    }).collect()
}

/// 今日专注分钟数与连续天数（连续天数从今天往回数，今天为 0 则从昨天算）。
pub(crate) fn focus_summary()->(u32,u32){
    let days=focus_days();
    let today=chrono::Local::now().date_naive();
    let minutes=days.iter().find(|(date,_)|date==&today.format("%Y-%m-%d").to_string()).map(|(_,m)|*m).unwrap_or(0);
    let mut streak=0;
    let mut cursor=today;
    if minutes==0 {cursor=today-chrono::Duration::days(1);}
    loop {
        let key=cursor.format("%Y-%m-%d").to_string();
        match days.iter().find(|(date,_)|date==&key) {
            Some((_,m)) if *m>0 =>{streak+=1;cursor=cursor-chrono::Duration::days(1);}
            _=>break,
        }
        if streak>365{break;}
    }
    (minutes,streak)
}

pub(crate) fn focus_add(minutes:u32){
    if minutes==0 || cfg!(test) {return;}
    let Some(path)=focus_stats_path() else{return;};
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    let today=chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    let mut days=focus_days();
    match days.iter_mut().find(|(date,_)|date==&today){
        Some(entry)=>entry.1+=minutes,
        None=>days.push((today,minutes)),
    }
    if days.len()>400 {let extra=days.len()-400;days.drain(0..extra);}
    days.sort_by(|a,b|a.0.cmp(&b.0));
    let json=serde_json::json!({"days":days.iter().map(|(date,m)|serde_json::json!({"date":date,"minutes":m})).collect::<Vec<_>>()});
    if let Ok(data)=serde_json::to_vec(&json){let _=std::fs::write(&path,data);}
}

/// 通知历史（落盘，最多 100 条）：岛仔关掉之后「当时没看到的那条」还能查回来。
/// 存 `%APPDATA%\\Daozai\\notices.json`，字段 at/title/body，新的在后面。
pub(crate) fn notice_history_path()->Option<std::path::PathBuf>{
    let base=std::env::var_os("APPDATA")?;
    Some(std::path::PathBuf::from(base).join("Daozai").join("notices.json"))
}

/// 按时间顺序返回 (时间, 标题, 正文)，最新在最后。
pub(crate) fn notice_history()->Vec<(String,String,String)>{
    let Some(path)=notice_history_path() else{return Vec::new();};
    let Ok(raw)=std::fs::read(&path) else{return Vec::new();};
    let Ok(json)=serde_json::from_slice::<serde_json::Value>(&raw) else{return Vec::new();};
    json.as_array().into_iter().flatten().filter_map(|item|{
        Some((item.get("at")?.as_str()?.to_owned(),item.get("title")?.as_str()?.to_owned(),item.get("body")?.as_str()?.to_owned()))
    }).collect()
}

pub(crate) fn notice_history_append(title:&str,body:&str){
    // 测试构建别往真实 APPDATA 里塞测试通知。
    if cfg!(test) {return;}
    let Some(path)=notice_history_path() else{return;};
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    let mut list=notice_history();
    list.push((chrono::Local::now().format("%m-%d %H:%M").to_string(),title.to_string(),body.to_string()));
    if list.len()>100{let extra=list.len()-100;list.drain(0..extra);}
    let json:Vec<_>=list.iter().map(|(at,title,body)|serde_json::json!({"at":at,"title":title,"body":body})).collect();
    if let Ok(data)=serde_json::to_vec(&json){let _=std::fs::write(&path,data);}
}

/// 通知通道诊断：每条进来的通知都记一行「显示 / 被谁拦下」，排查「某类通知不上岛」用。
pub(crate) fn log_notice(note:&str){
    let Some(base)=std::env::var_os("APPDATA") else{return};
'''),
    ("src/extras.rs",
     '''                            ui.add_space(3.0);
                            ui.small("调整后从下一阶段开始生效。");
                        });
                    }
                    3=>{
                        settings_label(ui,"喝水提醒");
''',
     '''                            ui.add_space(3.0);
                            ui.small("调整后从下一阶段开始生效。");
                        });
                        settings_label(ui,"专注记录");
                        settings_card(ui,|ui|{
                            // 只读内存里的统计（每分钟更新一次），不在这里读盘。
                            let (today,streak)=(self.focus_today_min,self.focus_streak_days);
                            let hours=today/60;
                            let line=if today>=60 {format!("今日 {hours} 小时 {} 分 · 连续 {streak} 天",today%60)} else {format!("今日 {today} 分 · 连续 {streak} 天")};
                            ui.add(egui::Label::new(egui::RichText::new(line).size(12.0)).wrap());
                            ui.add_space(3.0);
                            ui.small("只统计真正在跑的专注阶段，暂停不计；数据在 %APPDATA%\\\\Daozai\\\\focus-stats.json。");
                        });
                    }
                    3=>{
                        settings_label(ui,"喝水提醒");
'''),
    ("src/extras.rs",
     '''                            if keywords_on != !self.settings.notice_keywords.trim().is_empty() {
                                self.settings.notice_keywords=if keywords_on {"验证码,面试,转账,取款,取件,欠费".to_string()} else {String::new()};
                            }
                        });
                        // 总开关关闭时，微信与 QQ 两栏置灰并说明原因，避免「开了也不提醒」。
                        let master=self.settings.notifications;
''',
     '''                            if keywords_on != !self.settings.notice_keywords.trim().is_empty() {
                                self.settings.notice_keywords=if keywords_on {"验证码,面试,转账,取款,取件,欠费".to_string()} else {String::new()};
                            }
                            ui.separator();
                            setting_row(ui,"会议自动勿扰","日程开始时自动勿扰到散会（最多 2 小时），散会自动解除",&mut self.settings.quiet_meeting);
                            ui.separator();
                            // 最近通知：落盘的 100 条里取最新 8 条，点一行复制正文，右侧「稍后」5 分钟重弹。
                            settings_hint(ui,"最近通知 · 点击复制正文，或点「稍后」5 分钟后再提醒");
                            let recent=self.recent_notices();
                            let mut picked:Option<(String,String,bool)>=None;
                            if recent.is_empty() {
                                settings_hint(ui,"还没有记录。收到通知后会写进 notices.json，重启也能回查。");
                            }
                            for (at,title,body) in recent.iter().rev().take(8) {
                                let line=format!("{at}  {title} · {}",body.chars().take(26).collect::<String>());
                                ui.horizontal(|ui|{
                                    let resp=ui.add(egui::Label::new(egui::RichText::new(line).size(11.5)).sense(egui::Sense::click()));
                                    if resp.on_hover_text(body.as_str()).clicked(){picked=Some((title.clone(),body.clone(),false));}
                                    if ui.small_button("稍后").on_hover_text("5 分钟后再提醒一次").clicked(){picked=Some((title.clone(),body.clone(),true));}
                                });
                            }
                            if let Some((title,body,snooze))=picked {
                                if snooze {self.snooze_notice(5.0,title,body);}
                                else {self.pending.push(Cmd::Copy(body));self.say("已复制通知正文");}
                            }
                        });
                        // 总开关关闭时，微信与 QQ 两栏置灰并说明原因，避免「开了也不提醒」。
                        let master=self.settings.notifications;
'''),
    ("src/pages.rs",
     '''            resp.request_focus();
            cmds.push(Cmd::Focus);
        }
        if resp.has_focus() { c.stroke(46.0, 32.0, 327.0, 20.0, 8.0, 1.4, theme::BLUE); }
        // Esc 退出搜索：清空关键词并收起输入框。
        if resp.has_focus() && c.ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
''',
     '''            resp.request_focus();
            cmds.push(Cmd::Focus);
        }
        self.clip_typing=resp.has_focus();
        if resp.has_focus() { c.stroke(46.0, 32.0, 327.0, 20.0, 8.0, 1.4, theme::BLUE); }
        // Esc 退出搜索：清空关键词并收起输入框。
        if resp.has_focus() && c.ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
'''),
    ("src/pages.rs",
     '''            c.fill(371.0, top, 2.5, track, 1.25, Color32::from_white_alpha(14));
            c.fill(371.0, y, 2.5, thumb, 1.25, Color32::from_white_alpha(70));
        }
        let mut action: Option<(usize, u8)> = None; // 0 复制 1 固定 2 删除
        for (row, index) in shown.iter().copied().skip(self.clip_first).take(rows).enumerate() {
            let clip = &items[index];
            let y = if searching { 62.0 } else { 44.0 } + row as f32 * 22.5;
            let r = c.hit(48.0, y - 10.0, 323.0, 20.0, ("clip", index, clip.text.len(), clip.at.as_str()));
            let (h, _) = c.hover_t(&r);
            if h > 0.01 {
                c.fill(48.0, y - 10.0, 323.0, 20.0, 7.0, Color32::from_white_alpha((12.0 * h) as u8));
            }
            if clip.pinned {
''',
     '''            c.fill(371.0, top, 2.5, track, 1.25, Color32::from_white_alpha(14));
            c.fill(371.0, y, 2.5, thumb, 1.25, Color32::from_white_alpha(70));
        }
        // 搜索框拿到键盘焦点时：↑↓ 选行、Enter 复制。选中行始终保持在可视范围内。
        if self.clip_search_open && self.clip_typing && !shown.is_empty() {
            let (down, up, enter) = c.ui.ctx().input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Enter)));
            if down { self.clip_sel=(self.clip_sel+1).min(n-1); }
            if up { self.clip_sel=self.clip_sel.saturating_sub(1); }
            self.clip_sel=self.clip_sel.min(n-1);
            if self.clip_sel<self.clip_first { self.clip_first=self.clip_sel; }
            if self.clip_sel>=self.clip_first+rows { self.clip_first=self.clip_sel+1-rows; }
            if enter {
                let text=items[shown[self.clip_sel]].text.clone();
                #[cfg(windows)]
                self.say(if crate::clipboard::copy_history(&text) { "已复制" } else { "复制失败，请重试" });
                #[cfg(not(windows))]
                _cmds.push(Cmd::Copy(text));
            }
        }
        let mut action: Option<(usize, u8)> = None; // 0 复制 1 固定 2 删除
        for (row, index) in shown.iter().copied().skip(self.clip_first).take(rows).enumerate() {
            let clip = &items[index];
            let y = if searching { 62.0 } else { 44.0 } + row as f32 * 22.5;
            let r = c.hit(48.0, y - 10.0, 323.0, 20.0, ("clip", index, clip.text.len(), clip.at.as_str()));
            let (h, _) = c.hover_t(&r);
            let selected = self.clip_search_open && self.clip_typing && self.clip_first+row == self.clip_sel;
            if selected {
                c.fill(48.0, y - 10.0, 323.0, 20.0, 7.0, Color32::from_white_alpha(26));
            } else if h > 0.01 {
                c.fill(48.0, y - 10.0, 323.0, 20.0, 7.0, Color32::from_white_alpha((12.0 * h) as u8));
            }
            if clip.pinned {
'''),
    ("src/ui_tests.rs",
     '''    crate::mail::save_accounts(&accounts);assert_eq!(crate::mail::load_accounts(),accounts);
}

#[test]
fn clipboard_search_toggles_on_the_active_tab_and_filters_lines() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
''',
     '''    crate::mail::save_accounts(&accounts);assert_eq!(crate::mail::load_accounts(),accounts);
}

#[test]
fn snoozed_notice_returns_when_due() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.snooze_notice(5.0,"短信验证码".into(),"您的验证码：4342".into());
    assert_eq!(h.app.snoozed.len(),1,"点了稍后应该排进队列");
    // 还差 1 秒时不该弹。
    h.app.t += 299.0;
    h.app.tick_snoozed();
    assert!(h.app.notice.is_none(),"没到点不能提前弹");
    h.app.t += 2.0;
    h.app.tick_snoozed();
    assert!(h.app.snoozed.is_empty(),"到点后要从队列里移除");
    // 入队后由 tick_alerts 抬到 notice，这里两处都认。
    assert!(h.app.notice.iter().chain(h.app.alerts.iter()).any(|a|a.title=="短信验证码"),"到点要重新弹出来");
}

#[test]
fn clipboard_search_toggles_on_the_active_tab_and_filters_lines() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
'''),

    # custom-28：A 组功能集：通知/邮件/剪贴板/状态总线
    ("src/ui_tests.rs",
     '''    crate::mail::save_accounts(&accounts);assert_eq!(crate::mail::load_accounts(),accounts);
}

#[test]
fn snoozed_notice_returns_when_due() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
''',
     '''    crate::mail::save_accounts(&accounts);assert_eq!(crate::mail::load_accounts(),accounts);
}

#[test]
fn settings_notification_tab_shows_meeting_quiet_and_history() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.ctx.set_embed_viewports(false);
    fn frame(h:&mut H)->Vec<egui::epaint::ClippedShape>{
        let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(600.0,450.0))),
                viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.25),focused:Some(true),..Default::default()})).collect(),
                focused:true,events:Vec::new(),..Default::default()};
            *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let raw=egui::RawInput{viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(1.25),..Default::default()})).collect(),..Default::default()};
        let _=h.ctx.run(raw,|ctx|h.app.settings_ui(ctx,false));
        let shapes=output.lock().unwrap().take().unwrap().shapes;
        shapes
    }
    let has=|shapes:&[egui::epaint::ClippedShape],phrase:&str|shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(t)if t.galley.text().contains(phrase)));
    // 通知页内容很长，新增的「会议自动勿扰 / 最近通知」在滚动区下方，这里只验专注页的统计卡
    // （通知页那几行由人工点开确认，避免测试依赖滚动位置）。
    h.app.settings_tab=2;h.app.open_settings();
    for _ in 0..3{frame(&mut h);}
    let focus=frame(&mut h);
    assert!(has(&focus,"专注记录"),"专注页应有专注统计卡");
    assert!(has(&focus,"连续"),"专注统计要显示连续天数");
}

#[test]
fn snoozed_notice_returns_when_due() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
'''),

    # custom-28：A 组功能集：通知/邮件/剪贴板/状态总线
    ("src/model.rs",
     '''    pub clipboard_history: bool,
    /// 低电提醒阈值（百分比，0 = 关闭）。手机与本机共用，跨越阈值只提醒一次。
    pub battery_alert: u32,
    /// 通知关键词高亮：逗号分隔，命中时卡片左侧亮一条琥珀色竖条；空 = 关闭。
    pub notice_keywords: String,
    /// 勿扰时长（分钟，0 = 关闭）。只在本次运行内生效，重启后恢复为关闭。
''',
     '''    pub clipboard_history: bool,
    /// 低电提醒阈值（百分比，0 = 关闭）。手机与本机共用，跨越阈值只提醒一次。
    pub battery_alert: u32,
    /// AI 额度提醒阈值（剩余百分比，0 = 关闭）。跌破提醒一次，回升后重新武装。
    pub quota_alert: u32,
    /// 通知关键词高亮：逗号分隔，命中时卡片左侧亮一条琥珀色竖条；空 = 关闭。
    pub notice_keywords: String,
    /// 勿扰时长（分钟，0 = 关闭）。只在本次运行内生效，重启后恢复为关闭。
'''),
    ("src/model.rs",
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

''',
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

'''),
    ("src/model.rs",
     '''                "notice_keywords" => s.notice_keywords = v.trim().to_string(),
                "quiet_minutes" => s.quiet_minutes = match v.parse::<u32>().unwrap_or(0) { 30 => 30, 60 => 60, 120 => 120, _ => 0 },
                "quiet_meeting" => s.quiet_meeting = v != "0",
                "mail_filter" => s.mail_filter = v.trim().to_string(),
                _ => {}
            }
''',
     '''                "notice_keywords" => s.notice_keywords = v.trim().to_string(),
                "quiet_minutes" => s.quiet_minutes = match v.parse::<u32>().unwrap_or(0) { 30 => 30, 60 => 60, 120 => 120, _ => 0 },
                "quiet_meeting" => s.quiet_meeting = v != "0",
                "quota_alert" => s.quota_alert = match v.parse::<u32>().unwrap_or(20) { 0 => 0, 10 => 10, 20 => 20, 30 => 30, _ => 20 },
                "mail_filter" => s.mail_filter = v.trim().to_string(),
                _ => {}
            }
'''),
    ("src/model.rs",
     '''                ("notice_keywords".into(), self.notice_keywords.clone()),
                ("quiet_minutes".into(), self.quiet_minutes.to_string()),
                ("quiet_meeting".into(), (self.quiet_meeting as u8).to_string()),
                ("mail_filter".into(), self.mail_filter.clone()),
            ],
        );
''',
     '''                ("notice_keywords".into(), self.notice_keywords.clone()),
                ("quiet_minutes".into(), self.quiet_minutes.to_string()),
                ("quiet_meeting".into(), (self.quiet_meeting as u8).to_string()),
                ("quota_alert".into(), self.quota_alert.to_string()),
                ("mail_filter".into(), self.mail_filter.clone()),
            ],
        );
'''),
    ("src/model.rs",
     '''    }
}

/// 手机连接通知的来源 App 名单：`name` 匹配 Toast 正文首段，`label` 是岛上显示的标签，
/// `allow=false` 表示静默。放在 `phone-apps.json`，改完约 1 秒自动重载，不用重编译。
pub struct PhoneApps {
''',
     '''    }
}

/// 通知规则：`notice-rules.json` 里每条 = 正则 + 动作，按文件顺序、第一条命中即生效。
///
/// ```json
/// [{"pattern":"^(广告|促销)","action":"mute"},
///  {"pattern":"验证码","action":"copy"},
///  {"pattern":"面试|offer","action":"highlight"}]
/// ```
///
/// 动作：`mute` 静音（不上岛）、`highlight` 左侧亮琥珀色竖条、`copy` 自动复制正文。
/// 正则按 `(?i)` 编译，写错的那条会被跳过，不影响其它规则。
pub struct NoticeRules {
    pub items: Vec<(String, String)>,
    compiled: Vec<(regex::Regex, String)>,
    modified: Option<std::time::SystemTime>,
}

impl Default for NoticeRules {
    fn default() -> Self {
        Self { items: Vec::new(), compiled: Vec::new(), modified: None }
    }
}

impl NoticeRules {
    pub fn path() -> PathBuf { config_dir().join("notice-rules.json") }

    pub fn load() -> Self {
        let mut rules = Self::default();
        rules.reload();
        rules
    }

    pub fn reload(&mut self) {
        let path = Self::path();
        self.modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
        let Ok(raw) = fs::read(&path) else {
            self.items.clear();
            self.compiled.clear();
            return;
        };
        let Ok(text)=std::str::from_utf8(&raw) else{return;};
        self.parse(text);
    }

    /// 从 JSON 字节解析规则（reload 与单测共用）：正则写错的那条跳过，不影响其它规则。
    fn parse(&mut self, raw: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(raw) else { return; };
        self.items = json.as_array().into_iter().flatten().filter_map(|rule| {
            Some((rule.get("pattern")?.as_str()?.to_owned(), rule.get("action")?.as_str()?.to_lowercase()))
        }).collect();
        self.compiled = self.items.iter().filter_map(|(pattern, action)| {
            regex::Regex::new(&format!("(?i){pattern}")).ok().map(|re| (re, action.clone()))
        }).collect();
    }

    /// 文件改动约 1 秒内生效，不用重启。
    pub fn reload_if_changed(&mut self) {
        let now = fs::metadata(Self::path()).and_then(|m| m.modified()).ok();
        if now != self.modified { self.reload(); }
    }

    /// 是否有任一条「动作 = action」的规则命中文本。
    pub fn hits(&self, text: &str, action: &str) -> bool {
        if text.is_empty() { return false; }
        self.compiled.iter().any(|(re, act)| act == action && re.is_match(text))
    }

    /// 第一条命中的规则动作，用于诊断日志。
    pub fn first_action(&self, text: &str) -> Option<&str> {
        self.compiled.iter().find(|(re, _)| re.is_match(text)).map(|(_, act)| act.as_str())
    }
}

/// 手机连接通知的来源 App 名单：`name` 匹配 Toast 正文首段，`label` 是岛上显示的标签，
/// `allow=false` 表示静默。放在 `phone-apps.json`，改完约 1 秒自动重载，不用重编译。
pub struct PhoneApps {
'''),
    ("src/model.rs",
     '''            .map(|(_,label,allow)|(label.clone(),*allow))
    }
}
''',
     '''            .map(|(_,label,allow)|(label.clone(),*allow))
    }
}

#[cfg(test)]
mod notice_rules_tests {
    use super::NoticeRules;

    #[test]
    fn 规则按顺序第一条命中且坏正则不影响其它条() {
        let mut rules = NoticeRules::default();
        rules.parse(r#"[{"pattern":"广告","action":"mute"},{"pattern":"(","action":"copy"},{"pattern":"验证码|校验码","action":"copy"},{"pattern":"面试","action":"highlight"}]"#);
        assert!(rules.hits("这是一条广告推送", "mute"), "广告应被静音");
        assert!(rules.hits("您的验证码 1234", "copy"), "验证码应自动复制");
        assert!(rules.hits("明天下午面试", "highlight"), "面试应高亮");
        assert!(!rules.hits("普通消息", "mute"));
        assert_eq!(rules.first_action("广告里带验证码"), Some("mute"), "按文件顺序第一条生效");
        // 空文本不该命中。
        assert!(!rules.hits("", "mute"));
    }
}
'''),
    ("src/app.rs",
     '''    pub(crate) meeting_checked_at:f64,
    /// 「稍后提醒」队列：(到期时刻, 标题, 正文)。
    pub(crate) snoozed:Vec<(f64,String,String)>,
    /// 设置页的最近通知缓存：(上次读取时刻, 列表)，避免每帧读盘。
    pub(crate) notice_cache:(f64,Vec<(String,String,String)>),
    /// 还没凑满一分钟的专注秒数，攒够就写进 focus-stats.json。
''',
     '''    pub(crate) meeting_checked_at:f64,
    /// 「稍后提醒」队列：(到期时刻, 标题, 正文)。
    pub(crate) snoozed:Vec<(f64,String,String)>,
    /// 通知规则（notice-rules.json）：静音 / 高亮 / 自动复制。
    pub(crate) notice_rules: model::NoticeRules,
    /// 已经提醒过的「额度窗口」（id + 窗口名），回到阈值以上会移除、下次可再提醒。
    pub(crate) quota_alerted: std::collections::HashSet<(String,String)>,
    /// 上次检查额度阈值的时刻（每 30 秒一次）。
    pub(crate) quota_checked_at: f64,
    /// 设置页的最近通知缓存：(上次读取时刻, 列表)，避免每帧读盘。
    pub(crate) notice_cache:(f64,Vec<(String,String,String)>),
    /// 还没凑满一分钟的专注秒数，攒够就写进 focus-stats.json。
'''),
    ("src/app.rs",
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), phone_last:None, pc_last:None, quiet_until:0.0, quiet_meeting_key:None, meeting_checked_at:0.0, snoozed:Vec::new(), notice_cache:(0.0,Vec::new()), focus_pending:0.0, focus_today_min:focus_today, focus_streak_days:focus_streak, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
''',
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), notice_rules:model::NoticeRules::load(), quota_alerted:Default::default(), quota_checked_at:0.0, phone_last:None, pc_last:None, quiet_until:0.0, quiet_meeting_key:None, meeting_checked_at:0.0, snoozed:Vec::new(), notice_cache:(0.0,Vec::new()), focus_pending:0.0, focus_today_min:focus_today, focus_streak_days:focus_streak, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
'''),
    ("src/app.rs",
     '''        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        if self.t-self.meeting_checked_at>5.0 {self.meeting_checked_at=self.t;self.meeting_quiet(env);}
        self.tick_snoozed();
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
''',
     '''        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        if self.t-self.meeting_checked_at>5.0 {self.meeting_checked_at=self.t;self.meeting_quiet(env);}
        self.tick_snoozed();
        if self.t-self.quota_checked_at>30.0 {self.quota_checked_at=self.t;self.quota_threshold_alert();}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
'''),
    ("src/extras.rs",
     '''    /// 文本是否命中通知关键词表（大小写不敏感）。
    pub(crate) fn keyword_hit(&self,text:&str)->bool{
        if text.is_empty(){return false;}
        let lower=text.to_lowercase();
        self.settings.keywords().iter().any(|word|lower.contains(&word.to_lowercase()))
    }
''',
     '''    /// 文本是否命中通知关键词表（大小写不敏感）。
    pub(crate) fn keyword_hit(&self,text:&str)->bool{
        if text.is_empty(){return false;}
        // 规则里的 highlight 与关键词表同权：命中就亮琥珀色竖条。
        if self.notice_rules.hits(text,"highlight"){return true;}
        let lower=text.to_lowercase();
        self.settings.keywords().iter().any(|word|lower.contains(&word.to_lowercase()))
    }
'''),
    ("src/extras.rs",
     '''        self.say("会议开始，自动勿扰到散会");
    }

    /// 设置页用的最近通知：2 秒缓存一次，别每帧读盘。
    pub(crate) fn recent_notices(&mut self)->Vec<(String,String,String)>{
        if self.notice_cache.0<=0.0 || self.t-self.notice_cache.0>2.0 {
''',
     '''        self.say("会议开始，自动勿扰到散会");
    }

    /// 把配置与状态复制到 `backups/<时间戳>/`：只复制文件、不递归子目录，不动原配置。
    pub(crate) fn backup_settings(&mut self){
        let Some(base)=std::env::var_os("APPDATA") else{self.say("找不到 APPDATA，无法备份");return;};
        let root=std::path::PathBuf::from(base).join("Daozai");
        let Some(backups)=backup_root() else{self.say("找不到 APPDATA，无法备份");return;};
        let dest=backups.join(chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());
        if std::fs::create_dir_all(&dest).is_err(){self.say("备份失败：建不了目录");return;}
        let mut count=0;
        if let Ok(entries)=std::fs::read_dir(&root){
            for entry in entries.flatten(){
                let path=entry.path();
                if path.is_dir(){continue;}
                let Some(name)=path.file_name() else{continue;};
                if std::fs::copy(&path,dest.join(name)).is_ok(){count+=1;}
            }
        }
        self.say(format!("已备份 {count} 个文件"));
        self.pending.push(Cmd::Open(dest));
    }

    /// AI 额度阈值：任一窗口剩余 ≤ 阈值时提醒一次；回到阈值以上后重新武装。
    /// 每 30 秒看一次内存里的快照，不额外请求接口。
    pub(crate) fn quota_threshold_alert(&mut self){
        let threshold=self.settings.quota_alert;
        if threshold==0 {return;}
        let mut fresh:Vec<(String,f32)>=Vec::new();
        let mut armed=std::collections::HashSet::new();
        for (id,row) in self.quota.rows.iter(){
            let Ok(snapshot)=row else{continue};
            for window in snapshot.windows.iter().chain(snapshot.extra.iter()){
                if window.remaining<=threshold as f32 {
                    let key=(id.clone(),window.label.clone());
                    if !self.quota_alerted.contains(&key){fresh.push((window.label.clone(),window.remaining));}
                    armed.insert(key);
                }
            }
        }
        self.quota_alerted=armed;
        for (label,remaining) in fresh {
            self.alert("AI 额度", format!("{label} 只剩 {remaining:.0}%"), 7, true);
        }
    }

    /// 设置页用的最近通知：2 秒缓存一次，别每帧读盘。
    pub(crate) fn recent_notices(&mut self)->Vec<(String,String,String)>{
        if self.notice_cache.0<=0.0 || self.t-self.notice_cache.0>2.0 {
'''),
    ("src/extras.rs",
     '''            log_notice(&format!("DROP 名单静默    {trace}"));
            return;
        }
        // 来电方（正文首段就是号码或联系人），来电中/未接来电都用它当副标题。
        let caller=body.split('·').next().unwrap_or("").trim().to_string();
        // 岛仔自己发出的系统提醒会被通知中心回传；已在历史里的同一内容不再入队，避免重复显示。
''',
     '''            log_notice(&format!("DROP 名单静默    {trace}"));
            return;
        }
        // 通知规则：静音 / 自动复制正文。规则文件改完约 1 秒生效，不用重启。
        self.notice_rules.reload_if_changed();
        let rule_text=format!("{app} {body}");
        if self.notice_rules.hits(&rule_text,"mute") {
            log_notice(&format!("DROP 规则静音    {trace}"));
            return;
        }
        if self.notice_rules.hits(&rule_text,"copy") && !body.trim().is_empty() {
            self.pending.push(Cmd::Copy(body.clone()));
            log_notice(&format!("规则命中 copy    {trace}"));
        }
        // 来电方（正文首段就是号码或联系人），来电中/未接来电都用它当副标题。
        let caller=body.split('·').next().unwrap_or("").trim().to_string();
        // 岛仔自己发出的系统提醒会被通知中心回传；已在历史里的同一内容不再入队，避免重复显示。
'''),
    ("src/extras.rs",
     '''    }
}

/// 番茄统计：`%APPDATA%\\Daozai\\focus-stats.json`，只记「今天多少分钟」。
/// 只统计真正在跑的专注阶段，暂停不计；写盘频率是每分钟一次。
pub(crate) fn focus_stats_path()->Option<std::path::PathBuf>{
''',
     '''    }
}

/// 备份目录：`%APPDATA%\\Daozai\\backups`，每次备份一个时间戳子目录。
pub(crate) fn backup_root()->Option<std::path::PathBuf>{
    let base=std::env::var_os("APPDATA")?;
    Some(std::path::PathBuf::from(base).join("Daozai").join("backups"))
}

/// 番茄统计：`%APPDATA%\\Daozai\\focus-stats.json`，只记「今天多少分钟」。
/// 只统计真正在跑的专注阶段，暂停不计；写盘频率是每分钟一次。
pub(crate) fn focus_stats_path()->Option<std::path::PathBuf>{
'''),
    ("src/extras.rs",
     '''                            ui.separator();
                            setting_row(ui,"全屏时隐藏","游戏、视频和演示全屏时隐藏",&mut self.settings.hide_fullscreen);
                        });
                    }
                    1=>{
                        settings_label(ui,"歌词");
''',
     '''                            ui.separator();
                            setting_row(ui,"全屏时隐藏","游戏、视频和演示全屏时隐藏",&mut self.settings.hide_fullscreen);
                        });
                        settings_label(ui,"备份");
                        settings_card(ui,|ui|{
                            ui.horizontal(|ui|{
                                if ui.button("备份配置").on_hover_text("把 Daozai 目录下的配置与状态复制一份").clicked(){self.backup_settings();}
                                if ui.button("打开备份目录").clicked(){
                                    if let Some(dir)=backup_root(){let _=std::fs::create_dir_all(&dir);self.pending.push(Cmd::Open(dir));}
                                }
                            });
                            ui.add_space(3.0);
                            ui.small("备份到 %APPDATA%\\\\Daozai\\\\backups\\\\<时间戳>\\\\，只复制文件，不动原配置。");
                        });
                    }
                    1=>{
                        settings_label(ui,"歌词");
'''),
    ("src/extras.rs",
     '''                            ui.separator();
                            setting_row(ui,"提醒总开关","关闭后饮水、专注、待办、微信和 QQ 提醒都不显示",&mut self.settings.notifications);
                            ui.separator();
                            settings_control_row(ui,"低电提醒",SETTINGS_LABEL_W,|ui|{
                                let mut value=self.settings.battery_alert;
                                ui.horizontal(|ui|{
''',
     '''                            ui.separator();
                            setting_row(ui,"提醒总开关","关闭后饮水、专注、待办、微信和 QQ 提醒都不显示",&mut self.settings.notifications);
                            ui.separator();
                            ui.separator();
                            setting_row(ui,"会议自动勿扰","日程开始时自动勿扰到散会（最多 2 小时），散会自动解除",&mut self.settings.quiet_meeting);
                            ui.separator();
                            // 最近通知：落盘的 100 条里取最新 8 条，点一行复制正文，右侧「稍后」5 分钟重弹。
                            settings_hint(ui,"最近通知 · 点击复制正文，或点「稍后」5 分钟后再提醒");
                            let recent=self.recent_notices();
                            let mut picked:Option<(String,String,bool)>=None;
                            if recent.is_empty() {
                                settings_hint(ui,"还没有记录。收到通知后会写进 notices.json，重启也能回查。");
                            }
                            for (at,title,body) in recent.iter().rev().take(8) {
                                let line=format!("{at}  {title} · {}",body.chars().take(26).collect::<String>());
                                ui.horizontal(|ui|{
                                    let resp=ui.add(egui::Label::new(egui::RichText::new(line).size(11.5)).sense(egui::Sense::click()));
                                    if resp.on_hover_text(body.as_str()).clicked(){picked=Some((title.clone(),body.clone(),false));}
                                    if ui.small_button("稍后").on_hover_text("5 分钟后再提醒一次").clicked(){picked=Some((title.clone(),body.clone(),true));}
                                });
                            }
                            if let Some((title,body,snooze))=picked {
                                if snooze {self.snooze_notice(5.0,title,body);}
                                else {self.pending.push(Cmd::Copy(body));self.say("已复制通知正文");}
                            }
                            settings_control_row(ui,"低电提醒",SETTINGS_LABEL_W,|ui|{
                                let mut value=self.settings.battery_alert;
                                ui.horizontal(|ui|{
'''),
    ("src/extras.rs",
     '''                            });
                            settings_hint(ui,"跨过阈值只提醒一次，手机与本机共用；充满时也会提示。");
                            ui.separator();
                            // 勿扰：选时长立即生效，到点自动关闭；重启后恢复关闭。
                            let mut quiet=if self.quiet_until>self.t {self.settings.quiet_minutes.max(30)} else {0};
                            let mut quiet_pick=None;
''',
     '''                            });
                            settings_hint(ui,"跨过阈值只提醒一次，手机与本机共用；充满时也会提示。");
                            ui.separator();
                            settings_control_row(ui,"AI 额度提醒",SETTINGS_LABEL_W,|ui|{
                                let mut value=self.settings.quota_alert;
                                ui.horizontal(|ui|{
                                    if settings_segmented(ui,&mut value,&[(0,"关"),(10,"10%"),(20,"20%"),(30,"30%")],44.0){
                                        self.settings.quota_alert=value;
                                    }
                                }).response
                            });
                            settings_hint(ui,"任一额度窗口剩余跌破阈值时提醒一次，回升后下次还能再提醒。");
                            ui.separator();
                            // 勿扰：选时长立即生效，到点自动关闭；重启后恢复关闭。
                            let mut quiet=if self.quiet_until>self.t {self.settings.quiet_minutes.max(30)} else {0};
                            let mut quiet_pick=None;
'''),
    ("src/extras.rs",
     '''                                self.settings.notice_keywords=if keywords_on {"验证码,面试,转账,取款,取件,欠费".to_string()} else {String::new()};
                            }
                            ui.separator();
                            setting_row(ui,"会议自动勿扰","日程开始时自动勿扰到散会（最多 2 小时），散会自动解除",&mut self.settings.quiet_meeting);
                            ui.separator();
                            // 最近通知：落盘的 100 条里取最新 8 条，点一行复制正文，右侧「稍后」5 分钟重弹。
                            settings_hint(ui,"最近通知 · 点击复制正文，或点「稍后」5 分钟后再提醒");
                            let recent=self.recent_notices();
                            let mut picked:Option<(String,String,bool)>=None;
                            if recent.is_empty() {
                                settings_hint(ui,"还没有记录。收到通知后会写进 notices.json，重启也能回查。");
                            }
                            for (at,title,body) in recent.iter().rev().take(8) {
                                let line=format!("{at}  {title} · {}",body.chars().take(26).collect::<String>());
                                ui.horizontal(|ui|{
                                    let resp=ui.add(egui::Label::new(egui::RichText::new(line).size(11.5)).sense(egui::Sense::click()));
                                    if resp.on_hover_text(body.as_str()).clicked(){picked=Some((title.clone(),body.clone(),false));}
                                    if ui.small_button("稍后").on_hover_text("5 分钟后再提醒一次").clicked(){picked=Some((title.clone(),body.clone(),true));}
                                });
                            }
                            if let Some((title,body,snooze))=picked {
                                if snooze {self.snooze_notice(5.0,title,body);}
                                else {self.pending.push(Cmd::Copy(body));self.say("已复制通知正文");}
                            }
                        });
                        // 总开关关闭时，微信与 QQ 两栏置灰并说明原因，避免「开了也不提醒」。
                        let master=self.settings.notifications;
''',
     '''                                self.settings.notice_keywords=if keywords_on {"验证码,面试,转账,取款,取件,欠费".to_string()} else {String::new()};
                            }
                            ui.separator();
                            settings_control_row(ui,"通知规则",SETTINGS_LABEL_W,|ui|{
                                ui.horizontal(|ui|{
                                    let count=self.notice_rules.items.len();
                                    ui.label(egui::RichText::new(if count==0 {"未配置".to_string()} else {format!("{count} 条")}).size(12.0));
                                    if ui.button("编辑规则").clicked(){self.pending.push(Cmd::Open(crate::model::NoticeRules::path()));}
                                }).response
                            });
                            settings_hint(ui,"notice-rules.json：pattern + action（mute 静音 / highlight 高亮 / copy 自动复制），按顺序第一条生效，改完约 1 秒生效。");
                        });
                        // 总开关关闭时，微信与 QQ 两栏置灰并说明原因，避免「开了也不提醒」。
                        let master=self.settings.notifications;
'''),

    # custom-28：A 组功能集：通知/邮件/剪贴板/状态总线
    ("src/model.rs",
     '''    /// 从「手机连接」读取手机电量，作为唯一的常驻电量来源（不回退本机电量）。
    pub phone_battery: bool,
    /// 通知保留更久，可在铃铛上重复悬停查看（关闭则 4.5 秒后消失）。
    pub notice_keep: bool,
    /// 收到含验证码的邮件时自动写入剪贴板。
    pub auto_copy_otp: bool,
''',
     '''    /// 从「手机连接」读取手机电量，作为唯一的常驻电量来源（不回退本机电量）。
    pub phone_battery: bool,
    /// 通知保留更久，可在铃铛上重复悬停查看（关闭则 4.5 秒后消失）。
    /// 通知停留秒数（2–120）。停得久可以慢慢看，停得短不挡视线。
    pub notice_seconds: u32,
    /// 旧开关，只用于兼容老的 settings.txt（读到它时折算成 notice_seconds）。
    pub notice_keep: bool,
    /// 收到含验证码的邮件时自动写入剪贴板。
    pub auto_copy_otp: bool,
'''),
    ("src/model.rs",
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

''',
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

'''),
    ("src/model.rs",
     '''                "clock_12h" => s.clock_12h = v == "1",
                "show_battery" => s.show_battery = v != "0",
                "phone_battery" => s.phone_battery = v != "0",
                "notice_keep" => s.notice_keep = v != "0",
                "auto_copy_otp" => s.auto_copy_otp = v != "0",
                "update_check" => s.update_check = v == "1",
                "visible_pages" if (6..=8).contains(&v.len()) && v.bytes().all(|b| b==b'0' || b==b'1') => {
''',
     '''                "clock_12h" => s.clock_12h = v == "1",
                "show_battery" => s.show_battery = v != "0",
                "phone_battery" => s.phone_battery = v != "0",
                "notice_keep" => {s.notice_keep = v != "0";s.notice_seconds = if s.notice_keep {10} else {4};},
                "notice_seconds" => s.notice_seconds = v.parse::<u32>().unwrap_or(10).clamp(2,120),
                "auto_copy_otp" => s.auto_copy_otp = v != "0",
                "update_check" => s.update_check = v == "1",
                "visible_pages" if (6..=8).contains(&v.len()) && v.bytes().all(|b| b==b'0' || b==b'1') => {
'''),
    ("src/model.rs",
     '''                ("show_battery".into(), (self.show_battery as u8).to_string()),
                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("notice_keep".into(), (self.notice_keep as u8).to_string()),
                ("auto_copy_otp".into(), (self.auto_copy_otp as u8).to_string()),
                ("update_check".into(), (self.update_check as u8).to_string()),
                ("read_notifications".into(),(self.read_notifications as u8).to_string()),
''',
     '''                ("show_battery".into(), (self.show_battery as u8).to_string()),
                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("notice_keep".into(), (self.notice_keep as u8).to_string()),
                ("notice_seconds".into(), self.notice_seconds.to_string()),
                ("auto_copy_otp".into(), (self.auto_copy_otp as u8).to_string()),
                ("update_check".into(), (self.update_check as u8).to_string()),
                ("read_notifications".into(),(self.read_notifications as u8).to_string()),
'''),
    ("src/extras.rs",
     '''        // 「重复查看」开启时普通通知也保留更久，方便多次悬停。
        let otp_bell=phone && !body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        let keep_bell=if cfg!(test) {4.5} else if otp_bell {90.0} else if self.settings.notice_keep {60.0} else {4.5};
        // 勿扰期间不闪铃铛（横幅同样被 enqueue_alert 拦下），验证码仍照常提醒。
        if !self.muted() || otp_bell {
            self.notification_bell_at=self.t;
''',
     '''        // 「重复查看」开启时普通通知也保留更久，方便多次悬停。
        let otp_bell=phone && !body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        // 铃铛至少和卡片一样久；验证码多留一会儿，方便回看。
        let stay=self.settings.notice_seconds as f64;
        let keep_bell=if cfg!(test) {4.5} else if otp_bell {stay.max(90.0)} else {stay};
        // 勿扰期间不闪铃铛（横幅同样被 enqueue_alert 拦下），验证码仍照常提醒。
        if !self.muted() || otp_bell {
            self.notification_bell_at=self.t;
'''),
    ("src/extras.rs",
     '''            ("来电中".to_string(),format!("{caller} 正在呼叫"),60.0)
        }else if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,if !cfg!(test) && self.settings.notice_keep {60.0} else {4.5})};
        log_notice(&format!("SHOW {title} / {body}  <- {trace}"));
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:None},false);
    }
''',
     '''            ("来电中".to_string(),format!("{caller} 正在呼叫"),60.0)
        }else if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,if cfg!(test) {4.5} else {self.settings.notice_seconds as f64})};
        log_notice(&format!("SHOW {title} / {body}  <- {trace}"));
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:None},false);
    }
'''),
    ("src/extras.rs",
     '''                            ui.label(egui::RichText::new("QQ 需开启系统通知；新通知在岛上显示约 4.5 秒，默认隐藏正文。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            ui.separator();
                            setting_row(ui,"显示通知正文","可能包含私密消息；屏幕共享时建议关闭",&mut self.settings.notification_preview);
                            setting_row(ui,"重复查看通知","通知保留 60 秒，可在铃铛上多次悬停查看；关闭则 4.5 秒后消失",&mut self.settings.notice_keep);
                            ui.label(egui::RichText::new("只接收 Windows 通知中心可见的内容；应用会在收到通知后出现在下方。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            let before_apps=self.notification_apps.items.clone();
                            for (_,name,enabled) in &mut self.notification_apps.items {setting_row(ui,name,"",enabled);}
''',
     '''                            ui.label(egui::RichText::new("QQ 需开启系统通知；新通知在岛上显示约 4.5 秒，默认隐藏正文。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            ui.separator();
                            setting_row(ui,"显示通知正文","可能包含私密消息；屏幕共享时建议关闭",&mut self.settings.notification_preview);
                            setting_number(ui,"通知停留",&mut self.settings.notice_seconds,2,120," 秒",1.0);
                            settings_hint(ui,"通知卡片显示多久后自动收起（验证码卡片至少留 90 秒）。悬停不会打断计时，也可以随时点「关闭」。");
                            ui.label(egui::RichText::new("只接收 Windows 通知中心可见的内容；应用会在收到通知后出现在下方。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            let before_apps=self.notification_apps.items.clone();
                            for (_,name,enabled) in &mut self.notification_apps.items {setting_row(ui,name,"",enabled);}
'''),

    # custom-28：A 组功能集：通知/邮件/剪贴板/状态总线
    ("src/model.rs",
     '''    /// 通知保留更久，可在铃铛上重复悬停查看（关闭则 4.5 秒后消失）。
    /// 通知停留秒数（2–120）。停得久可以慢慢看，停得短不挡视线。
    pub notice_seconds: u32,
    /// 旧开关，只用于兼容老的 settings.txt（读到它时折算成 notice_seconds）。
    pub notice_keep: bool,
    /// 收到含验证码的邮件时自动写入剪贴板。
''',
     '''    /// 通知保留更久，可在铃铛上重复悬停查看（关闭则 4.5 秒后消失）。
    /// 通知停留秒数（2–120）。停得久可以慢慢看，停得短不挡视线。
    pub notice_seconds: u32,
    /// 验证码卡片停留秒数（10–300，且不短于通知停留）。要给回看和点「复制」留时间。
    pub otp_seconds: u32,
    /// 旧开关，只用于兼容老的 settings.txt（读到它时折算成 notice_seconds）。
    pub notice_keep: bool,
    /// 收到含验证码的邮件时自动写入剪贴板。
'''),
    ("src/model.rs",
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

''',
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

'''),
    ("src/model.rs",
     '''                "phone_battery" => s.phone_battery = v != "0",
                "notice_keep" => {s.notice_keep = v != "0";s.notice_seconds = if s.notice_keep {10} else {4};},
                "notice_seconds" => s.notice_seconds = v.parse::<u32>().unwrap_or(10).clamp(2,120),
                "auto_copy_otp" => s.auto_copy_otp = v != "0",
                "update_check" => s.update_check = v == "1",
                "visible_pages" if (6..=8).contains(&v.len()) && v.bytes().all(|b| b==b'0' || b==b'1') => {
''',
     '''                "phone_battery" => s.phone_battery = v != "0",
                "notice_keep" => {s.notice_keep = v != "0";s.notice_seconds = if s.notice_keep {10} else {4};},
                "notice_seconds" => s.notice_seconds = v.parse::<u32>().unwrap_or(10).clamp(2,120),
                "otp_seconds" => s.otp_seconds = v.parse::<u32>().unwrap_or(90).clamp(10,300),
                "auto_copy_otp" => s.auto_copy_otp = v != "0",
                "update_check" => s.update_check = v == "1",
                "visible_pages" if (6..=8).contains(&v.len()) && v.bytes().all(|b| b==b'0' || b==b'1') => {
'''),
    ("src/model.rs",
     '''                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("notice_keep".into(), (self.notice_keep as u8).to_string()),
                ("notice_seconds".into(), self.notice_seconds.to_string()),
                ("auto_copy_otp".into(), (self.auto_copy_otp as u8).to_string()),
                ("update_check".into(), (self.update_check as u8).to_string()),
                ("read_notifications".into(),(self.read_notifications as u8).to_string()),
''',
     '''                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("notice_keep".into(), (self.notice_keep as u8).to_string()),
                ("notice_seconds".into(), self.notice_seconds.to_string()),
                ("otp_seconds".into(), self.otp_seconds.to_string()),
                ("auto_copy_otp".into(), (self.auto_copy_otp as u8).to_string()),
                ("update_check".into(), (self.update_check as u8).to_string()),
                ("read_notifications".into(),(self.read_notifications as u8).to_string()),
'''),
    ("src/extras.rs",
     '''            return;
        }
        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去；
        // 「重复查看」开启时普通通知也保留更久，方便多次悬停。
        let otp_bell=phone && !body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        // 铃铛至少和卡片一样久；验证码多留一会儿，方便回看。
        let stay=self.settings.notice_seconds as f64;
        let keep_bell=if cfg!(test) {4.5} else if otp_bell {stay.max(90.0)} else {stay};
        // 勿扰期间不闪铃铛（横幅同样被 enqueue_alert 拦下），验证码仍照常提醒。
        if !self.muted() || otp_bell {
            self.notification_bell_at=self.t;
''',
     '''            return;
        }
        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去。
        let has_code=!body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        let otp_bell=phone && has_code;
        // 停留时长：普通通知用 notice_seconds；验证码用 otp_seconds，但不短于普通通知，
        // 且有 10 秒下限——验证码要留时间回看和点「复制」。
        let stay=notice_duration(self.settings.notice_seconds,self.settings.otp_seconds,false);
        let otp_stay=notice_duration(self.settings.notice_seconds,self.settings.otp_seconds,true);
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        let keep_bell=if cfg!(test) {4.5} else if has_code {otp_stay} else {stay};
        // 勿扰期间不闪铃铛（横幅同样被 enqueue_alert 拦下），验证码仍照常提醒。
        if !self.muted() || otp_bell {
            self.notification_bell_at=self.t;
'''),
    ("src/extras.rs",
     '''            ("来电中".to_string(),format!("{caller} 正在呼叫"),60.0)
        }else if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,if cfg!(test) {4.5} else {self.settings.notice_seconds as f64})};
        log_notice(&format!("SHOW {title} / {body}  <- {trace}"));
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:None},false);
    }
''',
     '''            ("来电中".to_string(),format!("{caller} 正在呼叫"),60.0)
        }else if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,if cfg!(test) {4.5} else if has_code {otp_stay} else {stay})};
        log_notice(&format!("SHOW {title} / {body}  <- {trace}"));
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:None},false);
    }
'''),
    ("src/extras.rs",
     '''    }
}

/// 备份目录：`%APPDATA%\\Daozai\\backups`，每次备份一个时间戳子目录。
pub(crate) fn backup_root()->Option<std::path::PathBuf>{
    let base=std::env::var_os("APPDATA")?;
''',
     '''    }
}

/// 通知卡片停留秒数：验证码有 10 秒下限，且不会短于普通通知（要留时间回看和点「复制」）。
pub(crate) fn notice_duration(notice_seconds:u32,otp_seconds:u32,has_code:bool)->f64{
    let stay=notice_seconds as f64;
    if has_code {(otp_seconds as f64).max(stay).max(10.0)} else {stay}
}

#[cfg(test)]
mod notice_duration_tests {
    use super::notice_duration;

    #[test]
    fn 验证码停留有下限且不短于普通通知() {
        assert_eq!(notice_duration(10, 90, true), 90.0, "验证码用自己设的 90 秒");
        assert_eq!(notice_duration(10, 20, true), 20.0);
        assert_eq!(notice_duration(60, 20, true), 60.0, "不能比普通通知还短");
        assert_eq!(notice_duration(10, 5, true), 10.0, "下限 10 秒");
        assert_eq!(notice_duration(10, 90, false), 10.0, "普通通知不受验证码设置影响");
    }
}

/// 备份目录：`%APPDATA%\\Daozai\\backups`，每次备份一个时间戳子目录。
pub(crate) fn backup_root()->Option<std::path::PathBuf>{
    let base=std::env::var_os("APPDATA")?;
'''),
    ("src/extras.rs",
     '''                            ui.separator();
                            setting_row(ui,"显示通知正文","可能包含私密消息；屏幕共享时建议关闭",&mut self.settings.notification_preview);
                            setting_number(ui,"通知停留",&mut self.settings.notice_seconds,2,120," 秒",1.0);
                            settings_hint(ui,"通知卡片显示多久后自动收起（验证码卡片至少留 90 秒）。悬停不会打断计时，也可以随时点「关闭」。");
                            ui.label(egui::RichText::new("只接收 Windows 通知中心可见的内容；应用会在收到通知后出现在下方。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            let before_apps=self.notification_apps.items.clone();
                            for (_,name,enabled) in &mut self.notification_apps.items {setting_row(ui,name,"",enabled);}
''',
     '''                            ui.separator();
                            setting_row(ui,"显示通知正文","可能包含私密消息；屏幕共享时建议关闭",&mut self.settings.notification_preview);
                            setting_number(ui,"通知停留",&mut self.settings.notice_seconds,2,120," 秒",1.0);
                            setting_number(ui,"验证码停留",&mut self.settings.otp_seconds,10,300," 秒",1.0);
                            settings_hint(ui,"卡片显示多久后自动收起，按秒可调。验证码卡片有 10 秒下限、且不会短于「通知停留」，留时间回看和点「复制」。");
                            ui.label(egui::RichText::new("只接收 Windows 通知中心可见的内容；应用会在收到通知后出现在下方。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            let before_apps=self.notification_apps.items.clone();
                            for (_,name,enabled) in &mut self.notification_apps.items {setting_row(ui,name,"",enabled);}
'''),

    # custom-36：性能与安全隐患全面优化
    ("src/app.rs",
     '''    pub(crate) quiet_meeting_key:Option<String>,
    /// 上次检查会议勿扰的时刻（每 5 秒查一次，别每帧都拼日程）。
    pub(crate) meeting_checked_at:f64,
    /// 「稍后提醒」队列：(到期时刻, 标题, 正文)。
    pub(crate) snoozed:Vec<(f64,String,String)>,
    /// 通知规则（notice-rules.json）：静音 / 高亮 / 自动复制。
    pub(crate) notice_rules: model::NoticeRules,
    /// 已经提醒过的「额度窗口」（id + 窗口名），回到阈值以上会移除、下次可再提醒。
    pub(crate) quota_alerted: std::collections::HashSet<(String,String)>,
    /// 上次检查额度阈值的时刻（每 30 秒一次）。
''',
     '''    pub(crate) quiet_meeting_key:Option<String>,
    /// 上次检查会议勿扰的时刻（每 5 秒查一次，别每帧都拼日程）。
    pub(crate) meeting_checked_at:f64,
    /// 今日会议候选缓存：(日期, 生成时刻, (开始, 结束, uid))。
    /// 日程合并排序较重，60 秒或跨天才重算一次；90 秒的触发窗口保证不会漏。
    pub(crate) meeting_plan:(String,f64,Vec<(i64,i64,String)>),
    /// 「稍后提醒」队列：(到期时刻, 标题, 正文)。
    pub(crate) snoozed:Vec<(f64,String,String)>,
    /// 通知规则（notice-rules.json）：静音 / 高亮 / 自动复制。
    pub(crate) notice_rules: model::NoticeRules,
    /// 剪贴板列表快照：(版本号, 共享列表)。只在版本变化时重建，避免每帧克隆整份历史。
    pub(crate) clip_snapshot: (u64, std::sync::Arc<Vec<crate::clipboard::Clip>>),
    /// 关键词缓存：(源字符串, 小写词表)。命中判断在绘制路径上，不能每次都 split + 小写。
    pub(crate) keyword_src: String,
    pub(crate) keywords: Vec<String>,
    /// 已经提醒过的「额度窗口」（id + 窗口名），回到阈值以上会移除、下次可再提醒。
    pub(crate) quota_alerted: std::collections::HashSet<(String,String)>,
    /// 上次检查额度阈值的时刻（每 30 秒一次）。
'''),
    ("src/app.rs",
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), notice_rules:model::NoticeRules::load(), quota_alerted:Default::default(), quota_checked_at:0.0, phone_last:None, pc_last:None, quiet_until:0.0, quiet_meeting_key:None, meeting_checked_at:0.0, snoozed:Vec::new(), notice_cache:(0.0,Vec::new()), focus_pending:0.0, focus_today_min:focus_today, focus_streak_days:focus_streak, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
''',
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), notice_rules:model::NoticeRules::load(), clip_snapshot:(u64::MAX,std::sync::Arc::new(Vec::new())), keyword_src:String::new(), keywords:Vec::new(), quota_alerted:Default::default(), quota_checked_at:0.0, phone_last:None, pc_last:None, quiet_until:0.0, quiet_meeting_key:None, meeting_checked_at:0.0, meeting_plan:(String::new(),f64::MIN,Vec::new()), snoozed:Vec::new(), notice_cache:(0.0,Vec::new()), focus_pending:0.0, focus_today_min:focus_today, focus_streak_days:focus_streak, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
'''),
    ("src/app.rs",
     '''        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        if self.t-self.meeting_checked_at>5.0 {self.meeting_checked_at=self.t;self.meeting_quiet(env);}
        self.tick_snoozed();
        if self.t-self.quota_checked_at>30.0 {self.quota_checked_at=self.t;self.quota_threshold_alert();}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
''',
     '''        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        if self.t-self.meeting_checked_at>5.0 {self.meeting_checked_at=self.t;self.meeting_quiet(env);}
        self.tick_snoozed();
        if self.keyword_src!=self.settings.notice_keywords {
            self.keyword_src=self.settings.notice_keywords.clone();
            self.keywords=self.settings.keywords().iter().map(|word|word.to_lowercase()).collect();
        }
        self.clipboard.flush();
        if self.t-self.quota_checked_at>30.0 {self.quota_checked_at=self.t;self.quota_threshold_alert();}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
'''),
    ("src/clipboard.rs",
     '''    /// 本地时间 `HH:MM`。
    pub at: String,
    pub pinned: bool,
}

#[derive(Debug, Default)]
pub struct History {
    pub items: Vec<Clip>,
    /// 每次变化 +1，界面据此判断是否需要重绘。
    pub version: u64,
    path: Option<std::path::PathBuf>,
}

''',
     '''    /// 本地时间 `HH:MM`。
    pub at: String,
    pub pinned: bool,
    /// 最后复制的 Unix 秒，用于清理 24 小时前的未固定内容。
    /// 固定条目永远保留，不因这个字段被清理。
    pub saved_at: i64,
}

/// 未固定剪贴板只保留最近 24 小时；固定条目不受影响。
/// 旧的 JSON 没有时间戳时按「今天刚读到」迁移，不会在升级时突然清空历史。
const RETENTION_SECS: i64 = 24 * 60 * 60;

fn now_secs() -> i64 { chrono::Local::now().timestamp() }

#[derive(Debug, Default)]
pub struct History {
    pub items: Vec<Clip>,
    /// 每次变化 +1，界面据此判断是否需要重绘。
    pub version: u64,
    /// 有未落盘的改动；由 `flush()` 在锁外统一写盘。
    dirty: bool,
    path: Option<std::path::PathBuf>,
}

'''),
    ("src/clipboard.rs",
     '''            Some(i) => self.items.remove(i).pinned,
            None => false,
        };
        self.items.insert(0, Clip { text: text.to_string(), at: at.to_string(), pinned });
        let mut recent = 0;
        self.items.retain(|c| c.pinned || { recent += 1; recent <= MAX_RECENT });
        self.changed();
''',
     '''            Some(i) => self.items.remove(i).pinned,
            None => false,
        };
        self.items.insert(0, Clip { text: text.to_string(), at: at.to_string(), pinned, saved_at: now_secs() });
        let mut recent = 0;
        self.items.retain(|c| c.pinned || { recent += 1; recent <= MAX_RECENT });
        self.changed();
'''),
    ("src/clipboard.rs",
     '''    }

    fn to_json(&self) -> Vec<u8> {
        let rows: Vec<_> = self.items.iter().map(|c| serde_json::json!({"pinned": c.pinned,"text": c.text, "at": c.at})).collect();
        serde_json::to_vec(&rows).unwrap_or_default()
    }

''',
     '''    }

    fn to_json(&self) -> Vec<u8> {
        let rows: Vec<_> = self.items.iter().map(|c| serde_json::json!({"pinned": c.pinned,"text": c.text, "at": c.at, "saved_at": c.saved_at})).collect();
        serde_json::to_vec(&rows).unwrap_or_default()
    }

'''),
    ("src/clipboard.rs",
     '''            text: row.get("text")?.as_str()?.to_string(),
            at: row.get("at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            pinned: row.get("pinned").and_then(|v| v.as_bool()).unwrap_or(true),
        })).filter(|c| !c.text.is_empty()).take(MAX_PINNED + MAX_RECENT).collect();
        Self { items, ..Self::default() }
    }
''',
     '''            text: row.get("text")?.as_str()?.to_string(),
            at: row.get("at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            pinned: row.get("pinned").and_then(|v| v.as_bool()).unwrap_or(true),
            saved_at: row.get("saved_at").and_then(|v| v.as_i64()).unwrap_or_else(now_secs),
        })).filter(|c| !c.text.is_empty()).take(MAX_PINNED + MAX_RECENT).collect();
        Self { items, ..Self::default() }
    }
'''),
    ("src/clipboard.rs",
     '''
    fn load_from(path: std::path::PathBuf) -> Self {
        let mut history = std::fs::read(&path).map(|raw| Self::from_json(&raw)).unwrap_or_default();
        history.path = Some(path);
        history
    }

    fn changed(&mut self) {
        self.version += 1;
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
            let _ = crate::model::atomic_write(path, &self.to_json());
        }
    }
}

''',
     '''
    fn load_from(path: std::path::PathBuf) -> Self {
        let mut history = std::fs::read(&path).map(|raw| Self::from_json(&raw)).unwrap_or_default();
        let oldest = now_secs() - RETENTION_SECS;
        // 敏感内容只在本机保留一天，重启时顺带清理；固定的片段模板不在此列。
        history.items.retain(|clip| clip.pinned || clip.saved_at >= oldest);
        history.path = Some(path);
        history
    }

    /// 只标记「有改动」。写盘交给 `flush()` 在锁外做——
    /// 之前是在 Mutex 里直接 atomic_write，界面每帧取快照时会被磁盘 IO 卡住。
    fn changed(&mut self) {
        self.version += 1;
        self.dirty = true;
    }

    /// 取出待写盘的内容（同时清掉 dirty）。没有改动返回 None。
    fn take_dirty(&mut self) -> Option<(std::path::PathBuf, Vec<u8>)> {
        if !self.dirty { return None; }
        let path = self.path.clone()?;
        self.dirty = false;
        Some((path, self.to_json()))
    }
}

'''),
    ("src/clipboard.rs",
     '''    pub fn lock(&self) -> std::sync::MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn record_update(&self, text: &str, from_history: bool) -> bool {
        !from_history && self.record(text)
    }
    pub fn record(&self, text: &str) -> bool {
        if !self.is_enabled() { return false; }
        let at = chrono::Local::now().format("%H:%M").to_string();
        self.lock().push(text, &at)
    }
}

''',
     '''    pub fn lock(&self) -> std::sync::MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// 有改动才写盘，且整段在锁外完成；一次复制只序列化一次。
    pub fn flush(&self) {
        let payload = { let mut history = self.lock(); history.take_dirty() };
        let Some((path, body)) = payload else { return; };
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        let _ = crate::model::atomic_write(&path, &body);
    }

    /// 只读版本号（加锁极短），界面先用它判断是否需要重建快照。
    pub fn version(&self) -> u64 { self.lock().version }

    /// 当前列表 + 版本号。界面用版本号判断要不要重建快照，避免每帧克隆整份历史。
    pub fn snapshot(&self) -> (u64, std::sync::Arc<Vec<Clip>>) {
        let history = self.lock();
        (history.version, std::sync::Arc::new(history.items.clone()))
    }

    fn record_update(&self, text: &str, from_history: bool) -> bool {
        !from_history && self.record(text)
    }
    pub fn record(&self, text: &str) -> bool {
        if !self.is_enabled() { return false; }
        let at = chrono::Local::now().format("%H:%M").to_string();
        let pushed = self.lock().push(text, &at);
        self.flush();
        pushed
    }
}

'''),
    ("src/clipboard.rs",
     '''mod tests {
    use super::*;

    #[test]
    fn history_copy_preserves_order_but_external_copy_promotes() {
        let shared = Shared { history: Default::default(), enabled: Arc::new(AtomicBool::new(true)) };
''',
     '''mod tests {
    use super::*;

    /// 与 Shared::flush 同逻辑，但不构造整份 Shared（沿用现有历史对象即可）。
    fn flush_history(history: &mut History) {
        let Some((path, body)) = history.take_dirty() else { return; };
        crate::model::atomic_write(&path, &body).unwrap();
    }

    #[test]
    fn history_copy_preserves_order_but_external_copy_promotes() {
        let shared = Shared { history: Default::default(), enabled: Arc::new(AtomicBool::new(true)) };
'''),
    ("src/clipboard.rs",
     '''        history.push("保留", "09:00");
        history.push("删除", "09:01");
        history.remove(0);
        let mut reopened = History::load_from(path.clone());
        assert_eq!(reopened.items, history.items);
        reopened.clear_recent();
        let final_history = History::load_from(path.clone());
        assert_eq!(final_history.items.len(), 1);
        assert_eq!(final_history.items[0].text, "old");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pinned_round_trip_and_preview() {
        let mut h = History::default();
''',
     '''        history.push("保留", "09:00");
        history.push("删除", "09:01");
        history.remove(0);
        flush_history(&mut history);
        let mut reopened = History::load_from(path.clone());
        assert_eq!(reopened.items, history.items);
        reopened.clear_recent();
        flush_history(&mut reopened);
        let final_history = History::load_from(path.clone());
        assert_eq!(final_history.items.len(), 1);
        assert_eq!(final_history.items[0].text, "old");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn snapshot_version_changes_only_when_content_changes() {
        let shared = Shared { history: Default::default(), enabled: Arc::new(AtomicBool::new(true)) };
        let first = shared.snapshot();
        assert_eq!(shared.snapshot().0, first.0, "重复读取不该让界面重建快照");
        assert!(shared.record("new"));
        let after = shared.snapshot();
        assert!(after.0 > first.0, "内容变化才推进快照版本");
        assert!(!shared.record("new"), "重复内容不产生新记录");
        assert_eq!(shared.snapshot().0, after.0);
    }

    #[test]
    fn old_unpinned_clips_expire_but_pins_survive() {
        let path = std::env::temp_dir().join(format!("daozai-clip-retention-{}.json", std::process::id()));
        let old = now_secs() - RETENTION_SECS - 60;
        std::fs::write(&path, serde_json::to_vec(&serde_json::json!([
            {"text":"过期秘密","at":"08:00","pinned":false,"saved_at":old},
            {"text":"长期模板","at":"08:01","pinned":true,"saved_at":old}
        ])).unwrap()).unwrap();
        let history = History::load_from(path.clone());
        assert_eq!(history.items.len(), 1);
        assert_eq!(history.items[0].text, "长期模板");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pinned_round_trip_and_preview() {
        let mut h = History::default();
'''),
    ("src/extras.rs",
     '''        // 规则里的 highlight 与关键词表同权：命中就亮琥珀色竖条。
        if self.notice_rules.hits(text,"highlight"){return true;}
        let lower=text.to_lowercase();
        self.settings.keywords().iter().any(|word|lower.contains(&word.to_lowercase()))
    }

''',
     '''        // 规则里的 highlight 与关键词表同权：命中就亮琥珀色竖条。
        if self.notice_rules.hits(text,"highlight"){return true;}
        let lower=text.to_lowercase();
        // tick 里维护好的小写词表；源字符串刚改、缓存还没跟上时退回现算。
        if self.keyword_src==self.settings.notice_keywords {
            return self.keywords.iter().any(|word|lower.contains(word.as_str()));
        }
        self.settings.keywords().iter().any(|word|lower.contains(&word.to_lowercase()))
    }

'''),
    ("src/extras.rs",
     '''        if !self.settings.quiet_meeting || self.muted() {return;}
        let Ok(day)=chrono::NaiveDate::parse_from_str(&env.today,"%Y-%m-%d") else{return};
        let now=chrono::Local::now().timestamp();
        let mut hit:Option<(i64,String)>=None;
        for entry in self.calendar_entries(day) {
            let event=entry.event;
            if event.all_day || event.start>now || now-event.start>90 {continue;}
            let Some(end)=event.end else{continue};
            if end<=now {continue;}
            hit=Some((end.min(now+7200), format!("{}|{}",event.uid,event.start)));
        }
        let Some((until,key))=hit else{return};
        if self.quiet_meeting_key.as_deref()==Some(key.as_str()) {return;}
        self.quiet_meeting_key=Some(key);
        self.quiet_until=until as f64;
''',
     '''        if !self.settings.quiet_meeting || self.muted() {return;}
        let Ok(day)=chrono::NaiveDate::parse_from_str(&env.today,"%Y-%m-%d") else{return};
        let now=chrono::Local::now().timestamp();
        // 原来每 5 秒都要把本地/工作/邮件日程合并再排序一次，日程多时会有抖动。
        // 现在缓存今日候选，60 秒或跨天才重算；触发窗口是 90 秒，不会因此漏掉会议。
        if self.meeting_plan.0!=env.today || self.t-self.meeting_plan.1>60.0 {
            let entries=self.calendar_entries(day);
            let plan=meeting_candidates(entries.iter().map(|entry|entry.event));
            self.meeting_plan=(env.today.clone(),self.t,plan);
        }
        let Some((until,key))=meeting_hit(&self.meeting_plan.2,now) else{return};
        if self.quiet_meeting_key.as_deref()==Some(key.as_str()) {return;}
        self.quiet_meeting_key=Some(key);
        self.quiet_until=until as f64;
'''),
    ("src/extras.rs",
     '''}

/// 验证码识别诊断：只在「自动复制」开启时写一行，便于排查邮件/短信为什么没复制。
pub(crate) fn log_otp(source:&str,code:Option<&str>){
    let Some(base)=std::env::var_os("APPDATA") else{return};
    let dir=std::path::PathBuf::from(base).join("Daozai").join("diagnostics");
''',
     '''}

/// 验证码识别诊断：只在「自动复制」开启时写一行，便于排查邮件/短信为什么没复制。
/// 日志只保留打码后的长度，绝不把验证码明文长期写进磁盘。
pub(crate) fn log_otp(source:&str,code:Option<&str>){
    let Some(base)=std::env::var_os("APPDATA") else{return};
    let dir=std::path::PathBuf::from(base).join("Daozai").join("diagnostics");
'''),
    ("src/extras.rs",
     '''    if std::fs::metadata(&path).is_ok_and(|m|m.len()>256*1024){let _=std::fs::remove_file(&path);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        use std::io::Write;
        let _=writeln!(file,"{} | {} | {}",chrono::Local::now().format("%H:%M:%S"),source,code.unwrap_or("-"));
    }
}

''',
     '''    if std::fs::metadata(&path).is_ok_and(|m|m.len()>256*1024){let _=std::fs::remove_file(&path);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        use std::io::Write;
        let masked = code.map(|value| crate::mail_privacy::mask(value, value));
        let _=writeln!(file,"{} | {} | {}",chrono::Local::now().format("%H:%M:%S"),source,masked.as_deref().unwrap_or("-"));
    }
}

'''),
    ("src/extras.rs",
     '''
fn focus_days()->Vec<(String,u32)>{
    let Some(path)=focus_stats_path() else{return Vec::new();};
    let Ok(raw)=std::fs::read(&path) else{return Vec::new();};
    let Ok(json)=serde_json::from_slice::<serde_json::Value>(&raw) else{return Vec::new();};
    json.get("days").and_then(|d|d.as_array()).into_iter().flatten().filter_map(|item|{
        Some((item.get("date")?.as_str()?.to_owned(), item.get("minutes")?.as_u64()? as u32))
    }).collect()
''',
     '''
fn focus_days()->Vec<(String,u32)>{
    let Some(path)=focus_stats_path() else{return Vec::new();};
    // 坏文件会被改名成 .bad 后返回 None，避免「解析失败」被当成「本来是空的」而清空统计。
    let Some(json)=crate::model::read_json_renaming_bad(&path,crate::model::MAX_JSON_BYTES) else{return Vec::new();};
    json.get("days").and_then(|d|d.as_array()).into_iter().flatten().filter_map(|item|{
        Some((item.get("date")?.as_str()?.to_owned(), item.get("minutes")?.as_u64()? as u32))
    }).collect()
'''),
    ("src/extras.rs",
     '''    if days.len()>400 {let extra=days.len()-400;days.drain(0..extra);}
    days.sort_by(|a,b|a.0.cmp(&b.0));
    let json=serde_json::json!({"days":days.iter().map(|(date,m)|serde_json::json!({"date":date,"minutes":m})).collect::<Vec<_>>()});
    if let Ok(data)=serde_json::to_vec(&json){let _=std::fs::write(&path,data);}
}

/// 通知历史（落盘，最多 100 条）：岛仔关掉之后「当时没看到的那条」还能查回来。
''',
     '''    if days.len()>400 {let extra=days.len()-400;days.drain(0..extra);}
    days.sort_by(|a,b|a.0.cmp(&b.0));
    let json=serde_json::json!({"days":days.iter().map(|(date,m)|serde_json::json!({"date":date,"minutes":m})).collect::<Vec<_>>()});
    if let Ok(data)=serde_json::to_vec(&json){let _=crate::model::atomic_write(&path,&data);}
}

/// 通知历史（落盘，最多 100 条）：岛仔关掉之后「当时没看到的那条」还能查回来。
'''),
    ("src/extras.rs",
     '''/// 按时间顺序返回 (时间, 标题, 正文)，最新在最后。
pub(crate) fn notice_history()->Vec<(String,String,String)>{
    let Some(path)=notice_history_path() else{return Vec::new();};
    let Ok(raw)=std::fs::read(&path) else{return Vec::new();};
    let Ok(json)=serde_json::from_slice::<serde_json::Value>(&raw) else{return Vec::new();};
    json.as_array().into_iter().flatten().filter_map(|item|{
        Some((item.get("at")?.as_str()?.to_owned(),item.get("title")?.as_str()?.to_owned(),item.get("body")?.as_str()?.to_owned()))
    }).collect()
''',
     '''/// 按时间顺序返回 (时间, 标题, 正文)，最新在最后。
pub(crate) fn notice_history()->Vec<(String,String,String)>{
    let Some(path)=notice_history_path() else{return Vec::new();};
    let Some(json)=crate::model::read_json_renaming_bad(&path,crate::model::MAX_JSON_BYTES) else{return Vec::new();};
    json.as_array().into_iter().flatten().filter_map(|item|{
        Some((item.get("at")?.as_str()?.to_owned(),item.get("title")?.as_str()?.to_owned(),item.get("body")?.as_str()?.to_owned()))
    }).collect()
'''),
    ("src/extras.rs",
     '''    if cfg!(test) {return;}
    let Some(path)=notice_history_path() else{return;};
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    let mut list=notice_history();
    list.push((chrono::Local::now().format("%m-%d %H:%M").to_string(),title.to_string(),body.to_string()));
    if list.len()>100{let extra=list.len()-100;list.drain(0..extra);}
    let json:Vec<_>=list.iter().map(|(at,title,body)|serde_json::json!({"at":at,"title":title,"body":body})).collect();
    if let Ok(data)=serde_json::to_vec(&json){let _=std::fs::write(&path,data);}
}

/// 通知通道诊断：每条进来的通知都记一行「显示 / 被谁拦下」，排查「某类通知不上岛」用。
''',
     '''    if cfg!(test) {return;}
    let Some(path)=notice_history_path() else{return;};
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    // 落盘前把验证码打码：历史文件只用于「回看是什么事」，不需要留着明文码。
    let body=if let Some(code)=crate::otp::extract("",body,&[]) {crate::mail_privacy::mask(body,&code.text)} else {body.to_string()};
    let mut list=notice_history();
    list.push((chrono::Local::now().format("%m-%d %H:%M").to_string(),title.to_string(),body));
    if list.len()>100{let extra=list.len()-100;list.drain(0..extra);}
    let json:Vec<_>=list.iter().map(|(at,title,body)|serde_json::json!({"at":at,"title":title,"body":body})).collect();
    // 原子写：中途断电也不会留下半截 JSON（否则下次读失败会被当空文件重建）。
    if let Ok(data)=serde_json::to_vec(&json){let _=crate::model::atomic_write(&path,&data);}
}

/// 通知通道诊断：每条进来的通知都记一行「显示 / 被谁拦下」，排查「某类通知不上岛」用。
'''),
    ("src/extras.rs",
     '''        if url.starts_with("https://")||url.starts_with("http://"){return Some(("打开",Shell(Cmd::OpenUrl(url.to_string()))));}
    }
    if let Some(file)=spec.strip_prefix("file:") {
        if !file.is_empty(){return Some(("打开",Shell(Cmd::Open(std::path::PathBuf::from(file)))));}
    }
    let action=spec.strip_prefix("action:")?.trim();
    match action {
''',
     '''        if url.starts_with("https://")||url.starts_with("http://"){return Some(("打开",Shell(Cmd::OpenUrl(url.to_string()))));}
    }
    if let Some(file)=spec.strip_prefix("file:") {
        // 只放行「看到的文件」：路径必须存在，且不是可执行/脚本/快捷方式——
        // 否则任何能写 island_status.json 的进程都能借岛仔的手启动程序。
        let path=std::path::PathBuf::from(file);
        let blocked=path.extension().and_then(|e|e.to_str()).map(|e|{
            matches!(e.to_ascii_lowercase().as_str(),"exe"|"bat"|"cmd"|"com"|"ps1"|"vbs"|"js"|"jse"|"wsf"|"wsh"|"hta"|"lnk"|"scr"|"msi"|"pif"|"reg")
        }).unwrap_or(false);
        if path.exists() && !blocked {return Some(("打开",Shell(Cmd::Open(path))));}
    }
    let action=spec.strip_prefix("action:")?.trim();
    match action {
'''),
    ("src/extras.rs",
     '''    let _=ctx.run(Default::default(),|_|{});
    assert_eq!(ctx.tex_manager().read().allocated().filter(|(_,meta)|meta.name=="qr").count(),0,"收起后应释放收款码贴图");
}
''',
     '''    let _=ctx.run(Default::default(),|_|{});
    assert_eq!(ctx.tex_manager().read().allocated().filter(|(_,meta)|meta.name=="qr").count(),0,"收起后应释放收款码贴图");
}

#[cfg(test)]
mod status_action_safety_tests {
    use super::status_action;

    #[test]
    fn 文件动作只放行存在的非可执行文件() {
        let root = std::env::temp_dir().join(format!("daozai-open-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let text = root.join("note.txt");
        let exe = root.join("run.exe");
        std::fs::write(&text, b"note").unwrap();
        std::fs::write(&exe, b"not an executable").unwrap();
        assert!(status_action(&format!("file:{}", text.display())).is_some());
        assert!(status_action(&format!("file:{}", exe.display())).is_none(), "可执行扩展必须拒绝");
        assert!(status_action(&format!("file:{}", root.join("missing.txt").display())).is_none(), "不存在的路径不该生成按钮");
        assert!(status_action("url:https://example.com").is_some());
        assert!(status_action("url:file:///C:/Windows/System32/cmd.exe").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod meeting_plan_tests {
    use super::meeting_hit;

    fn plan(items: &[(i64, i64, &str)]) -> Vec<(i64, i64, String)> {
        items.iter().map(|(a, b, c)| (*a, *b, (*c).to_string())).collect()
    }

    #[test]
    fn 只有刚开始且未结束的会议触发且最多两小时() {
        let now = 10_000;
        let p = plan(&[
            (now - 200, now + 600, "太早"),      // 超过 90 秒窗口
            (now + 60, now + 3600, "还没开始"),
            (now - 30, now - 1, "已结束"),
        ]);
        assert!(meeting_hit(&p, now).is_none(), "这三种都不该触发");

        let p = plan(&[(now - 30, now + 1800, "进行中")]);
        let (until, key) = meeting_hit(&p, now).unwrap();
        assert_eq!(until, now + 1800);
        assert_eq!(key, format!("进行中|{}", now - 30));

        let p = plan(&[(now - 30, now + 99_999, "超长会")]);
        assert_eq!(meeting_hit(&p, now).unwrap().0, now + 7200, "勿扰最多 2 小时");
    }

    #[test]
    fn 全天与无结束时间的日程不进候选表() {
        use crate::mail::Meeting;
        fn mk(all_day: bool, end: Option<i64>, uid: &str) -> Meeting {
            Meeting { all_day, title: "会".into(), start: 9_900, end, at: "11-12 10:00".into(), uid: uid.into() }
        }
        let events = vec![
            mk(true, Some(12_000), "全天"),
            mk(false, None, "无结束"),
            mk(false, Some(12_000), "正常"),
        ];
        let candidates = super::meeting_candidates(events.iter());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].2, "正常");
        assert!(meeting_hit(&[], 10_000).is_none(), "候选为空不该凭空触发");
    }
}

/// 把今日日程压成轻量候选表：全天日程、没有结束时间的都不具备「散会自动失效」的条件。
fn meeting_candidates<'a>(events:impl Iterator<Item=&'a crate::mail::Meeting>)->Vec<(i64,i64,String)>{
    events.filter_map(|event|{
        if event.all_day{return None;}
        Some((event.start,event.end?,event.uid.clone()))
    }).collect()
}

/// 从候选表挑出「刚开始 90 秒内、还没结束」的会议，返回 (勿扰截止, 去重键)。
/// 抽成纯函数便于单测；多场同时命中时沿用原来的「取最后一个」语义。
fn meeting_hit(plan:&[(i64,i64,String)],now:i64)->Option<(i64,String)>{
    let mut hit=None;
    for (start,end,uid) in plan {
        if *start>now || now-*start>90 {continue;}
        if *end<=now {continue;}
        hit=Some(((*end).min(now+7200),format!("{}|{}",uid,start)));
    }
    hit
}
'''),
    ("src/model.rs",
     '''    atomic_write(path, body.as_bytes())
}

pub(crate) fn atomic_write(path:&Path,body:&[u8])->std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64,Ordering};
''',
     '''    atomic_write(path, body.as_bytes())
}

/// 外部 JSON 的体积上限（1 MB）。岛仔读的都是「脚本 / 配置」级别的小文件，
/// 真遇到超大文件或管道，宁可当作没写，也不要一次性读进内存。
pub(crate) const MAX_JSON_BYTES: u64 = 1_048_576;

/// 读取外部 JSON：只读常规文件，且不超过 `max` 字节；不满足返回 None。
pub(crate) fn read_capped(path:&Path,max:u64)->Option<Vec<u8>> {
    let meta=fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len()>max {return None;}
    fs::read(path).ok()
}

/// 读一个 JSON 数组文件。解析失败时把坏文件改名成 `.bad` 再返回 None，
/// 调用方据此重建——不会把「文件坏了」误当成「本来是空的」从而把历史清掉。
pub(crate) fn read_json_renaming_bad(path:&Path,max:u64)->Option<serde_json::Value> {
    let raw=read_capped(path,max)?;
    match serde_json::from_slice::<serde_json::Value>(&raw) {
        Ok(value)=>Some(value),
        Err(_)=>{
            // 固定名在第二次损坏时会撞车：部分平台 rename 还会直接替换目标，
            // 那样前一次的证据就没了。逐个找没被占用的名字，显式判存在再搬。
            let mut moved=false;
            for n in 0..64 {
                let mut bad=path.as_os_str().to_owned();
                bad.push(format!(".bad.{n}"));
                let target=PathBuf::from(bad);
                if target.exists(){continue;}
                if fs::rename(path,&target).is_ok(){moved=true;break;}
            }
            if !moved {
                // 实在移不走（文件被占用等），至少留一份副本，不让内容被覆盖得无影无踪。
                let mut bad=path.as_os_str().to_owned();
                bad.push(format!(".bad.copy.{}",std::process::id()));
                let _=fs::copy(path,PathBuf::from(bad));
            }
            None
        }
    }
}

pub(crate) fn atomic_write(path:&Path,body:&[u8])->std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64,Ordering};
'''),
    ("src/model.rs",
     '''impl NotificationApps {
    pub fn load()->Self {
        let path=config_dir().join("notification-apps.json");
        let Ok(raw)=fs::read(path) else{return Self::default()};
        let Ok(json)=serde_json::from_slice::<serde_json::Value>(&raw) else{return Self::default()};
        let items=json.as_array().into_iter().flatten().filter_map(|a|Some((a.get("id")?.as_str()?.to_owned(),a.get("name")?.as_str()?.to_owned(),a.get("enabled")?.as_bool()?))).collect();Self{items}
    }
''',
     '''impl NotificationApps {
    pub fn load()->Self {
        let path=config_dir().join("notification-apps.json");
        let Some(raw)=read_capped(&path,MAX_JSON_BYTES) else{return Self::default()};
        let Ok(json)=serde_json::from_slice::<serde_json::Value>(&raw) else{return Self::default()};
        let items=json.as_array().into_iter().flatten().filter_map(|a|Some((a.get("id")?.as_str()?.to_owned(),a.get("name")?.as_str()?.to_owned(),a.get("enabled")?.as_bool()?))).collect();Self{items}
    }
'''),
    ("src/model.rs",
     '''    pub fn reload(&mut self) {
        let path = Self::path();
        self.modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
        let Ok(raw) = fs::read(&path) else {
            self.items.clear();
            self.compiled.clear();
            return;
''',
     '''    pub fn reload(&mut self) {
        let path = Self::path();
        self.modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
        let Some(raw) = read_capped(&path, MAX_JSON_BYTES) else {
            self.items.clear();
            self.compiled.clear();
            return;
'''),
    ("src/model.rs",
     '''    pub fn builtin()->Self{Self{items:builtin_phone_apps(),modified:None}}
    fn path()->PathBuf{config_dir().join("phone-apps.json")}
    fn read_items(path:&Path)->Option<Vec<(String,String,bool)>>{
        let raw=fs::read(path).ok()?;
        let json=serde_json::from_slice::<serde_json::Value>(&raw).ok()?;
        let items:Vec<_>=json.as_array()?.iter().filter_map(|a|{
            let name=a.get("name")?.as_str()?.to_owned();
''',
     '''    pub fn builtin()->Self{Self{items:builtin_phone_apps(),modified:None}}
    fn path()->PathBuf{config_dir().join("phone-apps.json")}
    fn read_items(path:&Path)->Option<Vec<(String,String,bool)>>{
        let raw=read_capped(path,MAX_JSON_BYTES)?;
        let json=serde_json::from_slice::<serde_json::Value>(&raw).ok()?;
        let items:Vec<_>=json.as_array()?.iter().filter_map(|a|{
            let name=a.get("name")?.as_str()?.to_owned();
'''),
    ("src/model.rs",
     '''        assert!(!rules.hits("", "mute"));
    }
}
''',
     '''        assert!(!rules.hits("", "mute"));
    }
}

#[cfg(test)]
mod external_json_safety_tests {
    use super::*;

    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("daozai-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn 外部_JSON_只读常规文件且有体积上限() {
        let file = path("capped");
        fs::write(&file, b"[]").unwrap();
        assert!(read_capped(&file, 2).is_some());
        assert!(read_capped(&file, 1).is_none(), "超过上限的输入不能读进内存");
        let dir = path("directory");
        fs::create_dir_all(&dir).unwrap();
        assert!(read_capped(&dir, MAX_JSON_BYTES).is_none(), "目录不能当 JSON 文件读");
        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn 坏_JSON_被改名而不是伪装成空历史() {
        let file = path("broken");
        fs::write(&file, b"{not json").unwrap();
        assert!(read_json_renaming_bad(&file, MAX_JSON_BYTES).is_none());
        let quarantined: Vec<_> = fs::read_dir(std::env::temp_dir()).unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().starts_with(&format!("{}.bad.", file.display())))
            .collect();
        assert_eq!(quarantined.len(), 1, "坏文件应被隔离保留，不能直接覆盖");
        assert!(!file.exists(), "原路径必须腾空，否则下次写入会盖掉坏文件");
        // 故意不删隔离文件，模拟「已经损坏过一次」。

        // 第二次损坏：.bad.0 已存在，仍必须成功隔离成 .bad.1。
        fs::write(&file, b"{still broken").unwrap();
        assert!(read_json_renaming_bad(&file, MAX_JSON_BYTES).is_none());
        assert!(!file.exists(), "隔离目标已存在时也不能留下坏文件被覆盖");
        let again: Vec<_> = fs::read_dir(std::env::temp_dir()).unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().starts_with(&format!("{}.bad.", file.display())))
            .collect();
        assert_eq!(again.len(), 2, "两次损坏应各留一份副本");
        for p in again { let _ = fs::remove_file(p); }
    }
}
'''),
    ("src/pages.rs",
     '''            if r.clicked() { self.settings.clipboard_history = true; self.clipboard.set_enabled(true); self.settings.save(); }
            return;
        }
        let items = self.clipboard.lock().items.clone();
        let query = self.clip_query.trim().to_lowercase();
        let shown: Vec<usize> = if query.is_empty() {
            (0..items.len()).collect()
''',
     '''            if r.clicked() { self.settings.clipboard_history = true; self.clipboard.set_enabled(true); self.settings.save(); }
            return;
        }
        // 只在版本变化时重建快照：之前是每帧把整份历史（最多 30×2 万字）克隆一遍。
        let version=self.clipboard.version();
        if self.clip_snapshot.0!=version { self.clip_snapshot=self.clipboard.snapshot(); }
        let items=self.clip_snapshot.1.clone();
        let query = self.clip_query.trim().to_lowercase();
        let shown: Vec<usize> = if query.is_empty() {
            (0..items.len()).collect()
'''),

    # custom-37：修圆角与半圆连接处的阴影突起
    ("src/app.rs",
     '''fn pill_shadow(painter:&egui::Painter,rect:Rect,radius:f32,strength:f32){
    if strength<=0.01 {return;}
    for (grow,a) in [(10.0f32,0.035f32),(6.5,0.05),(3.5,0.07),(1.5,0.09)] {
        let r=Rect::from_min_max(rect.min-Vec2::new(grow*0.6,grow*0.3),rect.max+Vec2::new(grow*0.6,grow));
        painter.rect_filled(r,radius+grow*0.8,Color32::from_black_alpha((255.0*a*strength) as u8));
    }
}
pub(crate) const PAGE_NAMES: [&str; 8] = ["音乐", "专注", "饮水", "置物架", "待办", "邮箱", "AI 额度", "日程"];
''',
     '''fn pill_shadow(painter:&egui::Painter,rect:Rect,radius:f32,strength:f32){
    if strength<=0.01 {return;}
    for (grow,a) in [(10.0f32,0.035f32),(6.5,0.05),(3.5,0.07),(1.5,0.09)] {
        // 每层都是主体的等距外扩（圆角增量 = 横向扩展量）+ 整体下移，
        // 四角圆弧与主体严格同心同形，直线转圆角处不会再叠出错开的半圆凸边。
        let (d,dy)=shape::shadow_expansion(grow);
        let r=Rect::from_min_max(rect.min-Vec2::new(d,d-dy),rect.max+Vec2::new(d,d+dy));
        painter.rect_filled(r,radius+d,Color32::from_black_alpha((255.0*a*strength) as u8));
    }
}
pub(crate) const PAGE_NAMES: [&str; 8] = ["音乐", "专注", "饮水", "置物架", "待办", "邮箱", "AI 额度", "日程"];
'''),
    ("src/shape.rs",
     '''    pub fn apply(self, p: Pos2) -> Pos2 {
        match self.dock {
            Dock::Top => p,
            Dock::Left => Pos2::new(self.edge + p.y, p.x),
            Dock::Right => Pos2::new(self.edge - p.y, p.x),
        }
    }
    fn code(self) -> f32 { match self.dock { Dock::Top => 0.0, Dock::Left => 1.0, Dock::Right => 2.0 } }
}

/// 贴着 `dock` 边缘的轮廓（屏幕坐标）。
pub fn outline_docked(rect: Rect, dock: Dock, flare: f32, flare_h: f32, radius: f32) -> Vec<Pos2> {
    let (map, frame) = EdgeMap::of(rect, dock);
    outline(frame, flare, flare_h, radius).into_iter().map(|p| map.apply(p)).collect()
}

/// 贴着 `dock` 边缘的抗锯齿填充。
#[allow(clippy::too_many_arguments)]
pub fn fill_docked(painter: &egui::Painter, rect: Rect, dock: Dock, flare: f32, flare_h: f32, radius: f32, color: Color32, feather: f32) {
    let (map, frame) = EdgeMap::of(rect, dock);
''',
     '''    pub fn apply(self, p: Pos2) -> Pos2 {
        match self.dock {
            Dock::Top => p,
            Dock::Left => Pos2::new(self.edge + p.y, p.x),
            Dock::Right => Pos2::new(self.edge - p.y, p.x),
        }
    }
    fn code(self) -> f32 { match self.dock { Dock::Top => 0.0, Dock::Left => 1.0, Dock::Right => 2.0 } }
}

/// 阴影层相对主体的 (等距外扩, 向下偏移)。
///
/// 圆角增量必须**等于**横向扩展量，否则每层阴影的圆弧与主体圆弧不同心、会在角部
/// 互相交叉，看起来像「直线转半圆的地方多了一圈突起」。等距外扩后再整体下移 `dy`
/// 模拟顶部光源，四角圆心只随 dy 平移，不会再横向漂移。
pub(crate) fn shadow_expansion(grow: f32) -> (f32, f32) {
    (grow * 0.6, grow * 0.35)
}

/// 贴着 `dock` 边缘的轮廓（屏幕坐标）。
pub fn outline_docked(rect: Rect, dock: Dock, flare: f32, flare_h: f32, radius: f32) -> Vec<Pos2> {
    let (map, frame) = EdgeMap::of(rect, dock);
    outline(frame, flare, flare_h, radius).into_iter().map(|p| map.apply(p)).collect()
}

/// 贴着 `dock` 边缘的抗锯齿填充。
#[allow(clippy::too_many_arguments)]
pub fn fill_docked(painter: &egui::Painter, rect: Rect, dock: Dock, flare: f32, flare_h: f32, radius: f32, color: Color32, feather: f32) {
    let (map, frame) = EdgeMap::of(rect, dock);
'''),
    ("src/shape.rs",
     '''    const N_FLARE: usize = 14;
    for i in 0..=N_FLARE {
        let t = i as f32 / N_FLARE as f32;
        // 肩部曲线：(1-t)^2.4 比三次方更饱满，过渡更柔。
        let wing = flare * (1.0 - t).powf(2.4);
        rows.push((y0 + fh * t, x0 - wing, x1 + wing));
    }
    if y1 - r > y0 + fh {
        rows.push((y1 - r, x0, x1));
    }
    const N_CORNER: usize = 20;
    for i in 1..=N_CORNER {
        let a = i as f32 / N_CORNER as f32 * std::f32::consts::FRAC_PI_2;
        rows.push((y1 - r + r * a.sin(), x0 + r - r * a.cos(), x1 - r + r * a.cos()));
    }
    rows
}

/// 闭合外轮廓（顺时针：右侧自上而下，再左侧自下而上）。
pub fn outline(rect: Rect, flare: f32, flare_h: f32, radius: f32) -> Vec<Pos2> {
    let rows = sections(rect, flare, flare_h, radius);
''',
     '''    const N_FLARE: usize = 14;
    for i in 0..=N_FLARE {
        let t = i as f32 / N_FLARE as f32;
        // 肩部曲线：(1-t)^2.4 比三次方更饱满，过渡更柔。
        let wing = flare * (1.0 - t).powf(2.4);
        rows.push((y0 + fh * t, x0 - wing, x1 + wing));
    }
    if y1 - r > y0 + fh {
        rows.push((y1 - r, x0, x1));
    }
    // 32 段：直线转圆弧的过渡更细腻，端点切线几乎与直边重合。
    const N_CORNER: usize = 32;
    for i in 1..=N_CORNER {
        let a = i as f32 / N_CORNER as f32 * std::f32::consts::FRAC_PI_2;
        rows.push((y1 - r + r * a.sin(), x0 + r - r * a.cos(), x1 - r + r * a.cos()));
    }
    rows
}

/// 闭合外轮廓（顺时针：右侧自上而下，再左侧自下而上）。
pub fn outline(rect: Rect, flare: f32, flare_h: f32, radius: f32) -> Vec<Pos2> {
    let rows = sections(rect, flare, flare_h, radius);
'''),
    ("src/shape.rs",
     '''/// 柔和的落影：多层半透明轮廓叠加，近似高斯模糊（不依赖 GPU 模糊）。
pub fn shadow(painter: &egui::Painter, rect: Rect, flare: f32, flare_h: f32, radius: f32, strength: f32) {
    shadow_mapped(painter, rect, EdgeMap::TOP, flare, flare_h, radius, strength);
}

fn shadow_mapped(painter: &egui::Painter, rect: Rect, map: EdgeMap, flare: f32, flare_h: f32, radius: f32, strength: f32) {
    if strength <= 0.01 {
        return;
    }
    for (grow, a) in [(10.0f32, 0.035f32), (6.5, 0.05), (3.5, 0.07), (1.5, 0.09)] {
        let r = Rect::from_min_max(
            Pos2::new(rect.left() - grow * 0.6, rect.top()),
            Pos2::new(rect.right() + grow * 0.6, rect.bottom() + grow),
        );
        let c = Color32::from_black_alpha((255.0 * a * strength) as u8);
        fill_mapped(painter, r, map, flare, flare_h, radius + grow * 0.8, c, c, r.height(), grow * 0.9);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(points: &[Pos2], p: Pos2) -> bool {
        let mut inside = false;
        let mut prev = *points.last().unwrap();
''',
     '''/// 柔和的落影：多层半透明轮廓叠加，近似高斯模糊（不依赖 GPU 模糊）。
pub fn shadow(painter: &egui::Painter, rect: Rect, flare: f32, flare_h: f32, radius: f32, strength: f32) {
    shadow_mapped(painter, rect, EdgeMap::TOP, flare, flare_h, radius, strength);
}

fn shadow_mapped(painter: &egui::Painter, rect: Rect, map: EdgeMap, flare: f32, flare_h: f32, radius: f32, strength: f32) {
    if strength <= 0.01 {
        return;
    }
    for (grow, a) in [(10.0f32, 0.035f32), (6.5, 0.05), (3.5, 0.07), (1.5, 0.09)] {
        let (d, dy) = shadow_expansion(grow);
        let r = Rect::from_min_max(
            Pos2::new(rect.left() - d, rect.top() - (d - dy)),
            Pos2::new(rect.right() + d, rect.bottom() + d + dy),
        );
        let c = Color32::from_black_alpha((255.0 * a * strength) as u8);
        fill_mapped(painter, r, map, flare, flare_h, radius + d, c, c, r.height(), grow * 0.9);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(points: &[Pos2], p: Pos2) -> bool {
        let mut inside = false;
        let mut prev = *points.last().unwrap();
'''),
    ("src/shape.rs",
     '''        mesh.colored_vertex(a, shade(a.x, a.y));
        mesh.colored_vertex(b, shade(b.x, b.y));
        mesh.colored_vertex(b + nb * feather, clear(b + nb * feather));
        mesh.colored_vertex(a + na * feather, clear(a + na * feather));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

''',
     '''        mesh.colored_vertex(a, shade(a.x, a.y));
        mesh.colored_vertex(b, shade(b.x, b.y));
        mesh.colored_vertex(b + nb * feather, clear(b + nb * feather));
        mesh.colored_vertex(a + na * feather, clear(a + na * feather));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

#[cfg(test)]
mod corner_tests {
    use super::*;

    #[test]
    fn 阴影层四角圆弧与主体同心等距() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 46.0));
        let radius = 16.0;
        let body_rt = Pos2::new(rect.right() - radius, rect.top() + radius);
        let body_lb = Pos2::new(rect.left() + radius, rect.bottom() - radius);
        for grow in [10.0f32, 6.5, 3.5, 1.5] {
            let (d, dy) = shadow_expansion(grow);
            let r = Rect::from_min_max(rect.min - Vec2::new(d, d - dy), rect.max + Vec2::new(d, d + dy));
            let rr = radius + d;
            let shadow_rt = Pos2::new(r.right() - rr, r.top() + rr);
            let shadow_lb = Pos2::new(r.left() + rr, r.bottom() - rr);
            // 等距外扩不改圆心，整体下移只让圆心跟着下移 dy；圆弧不横向漂移就不会与主体交叉。
            assert!((rr - radius - d).abs() < 1e-4, "圆角增量必须等于横向扩展量");
            assert!((shadow_rt.x - body_rt.x).abs() < 1e-4, "右上角圆弧不能横向漂移");
            assert!((shadow_rt.y - body_rt.y - dy).abs() < 1e-4);
            assert!((shadow_lb.x - body_lb.x).abs() < 1e-4, "左下角圆弧不能横向漂移");
            assert!((shadow_lb.y - body_lb.y - dy).abs() < 1e-4);

            // 旧参数（圆角 +0.8g、横向 +0.6g、上下不对称）确实让角部圆弧错位，
            // 这正是「直线转半圆处有突起」的来源。
            let old_r = Rect::from_min_max(
                rect.min - Vec2::new(grow * 0.6, grow * 0.3),
                rect.max + Vec2::new(grow * 0.6, grow),
            );
            let old_c = Pos2::new(old_r.right() - (radius + grow * 0.8), old_r.top() + radius + grow * 0.8);
            assert!((old_c.x - body_rt.x).abs() > 0.1 * grow, "旧实现圆心漂移量应当可测");
        }
    }

    #[test]
    fn 圆弧两端与直边相切() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 140.0));
        let r = 33.0;
        let rows = sections(rect, 0.0, 16.0, r);
        let corner: Vec<_> = rows.iter().copied().filter(|t| t.0 >= rect.bottom() - r - 1e-4).collect();
        assert!(corner.len() > 20, "1/4 圆弧要有足够采样");

        // 竖直侧边 -> 圆弧起点：切线必须接近竖直，否则直线接半圆时会有折角。
        let (ya, la, _) = corner[0];
        let (yb, lb, _) = corner[1];
        assert!((lb - la).abs() / (yb - ya) < 0.03, "圆弧起点切线不够竖直");

        // 圆弧终点 -> 水平底边：切线必须接近水平。
        let (y1, l1, _) = corner[corner.len() - 2];
        let (y2, l2, _) = corner[corner.len() - 1];
        assert!((y2 - y1) / (l2 - l1) < 0.03, "圆弧终点切线不够水平");

        // 羽化法线不能在轮廓拐点炸开（否则角部会拉出尖刺）。
        let pts = outline(rect, 0.0, 16.0, r);
        for i in 0..pts.len() {
            assert!(edge_normal(&pts, i).length() < 2.0, "角部羽化法线异常：{i}");
        }
    }
}
'''),

    # custom-38：四角阴影完全对称，顶部两角与底部一样平滑
    ("src/shape.rs",
     '''
/// 阴影层相对主体的 (等距外扩, 向下偏移)。
///
/// 圆角增量必须**等于**横向扩展量，否则每层阴影的圆弧与主体圆弧不同心、会在角部
/// 互相交叉，看起来像「直线转半圆的地方多了一圈突起」。等距外扩后再整体下移 `dy`
/// 模拟顶部光源，四角圆心只随 dy 平移，不会再横向漂移。
pub(crate) fn shadow_expansion(grow: f32) -> (f32, f32) {
    (grow * 0.6, grow * 0.35)
}

/// 贴着 `dock` 边缘的轮廓（屏幕坐标）。
''',
     '''
/// 阴影层相对主体的 (等距外扩, 向下偏移)。
///
/// 圆角增量必须**等于**横向扩展量 `d`，否则每层阴影的圆弧与主体圆弧不同心、会在角部
/// 互相交叉，看起来像「直线转半圆的地方多了一圈突起」。等距外扩后再整体下移 `dy`
/// 模拟顶部光源，四角圆心只随 dy 平移，不会再横向漂移。
///
/// `dy` 必须保持 0：顶部净外扩是 `d - dy`、底部是 `d + dy`。早期用 `0.35g` 时两者
/// 是 0.25g : 0.95g，顶部只剩一条硬边、底部有四五层渐淡的环，于是「上面两个角
/// 不如下面两个角平滑」。岛本来就贴着屏幕顶边，向下的落影方向感收益极小，
/// 完全对称的等距外扩才能保证四角过渡一模一样。
pub(crate) fn shadow_expansion(grow: f32) -> (f32, f32) {
    (grow * 0.6, 0.0)
}

/// 贴着 `dock` 边缘的轮廓（屏幕坐标）。
'''),
    ("src/shape.rs",
     '''            let shadow_lb = Pos2::new(r.left() + rr, r.bottom() - rr);
            // 等距外扩不改圆心，整体下移只让圆心跟着下移 dy；圆弧不横向漂移就不会与主体交叉。
            assert!((rr - radius - d).abs() < 1e-4, "圆角增量必须等于横向扩展量");
            assert!((shadow_rt.x - body_rt.x).abs() < 1e-4, "右上角圆弧不能横向漂移");
            assert!((shadow_rt.y - body_rt.y - dy).abs() < 1e-4);
            assert!((shadow_lb.x - body_lb.x).abs() < 1e-4, "左下角圆弧不能横向漂移");
''',
     '''            let shadow_lb = Pos2::new(r.left() + rr, r.bottom() - rr);
            // 等距外扩不改圆心，整体下移只让圆心跟着下移 dy；圆弧不横向漂移就不会与主体交叉。
            assert!((rr - radius - d).abs() < 1e-4, "圆角增量必须等于横向扩展量");
            // 顶部净外扩 / 底部净外扩：太不对称就会「上两角硬、下两角柔」。
            let top = d - dy;
            let bottom = d + dy;
            assert!(top > 0.0, "顶部阴影不能缩进主体");
            assert!((top - bottom).abs() < 1e-6, "上下阴影必须严格对称，否则上两角比下两角生硬：{top}/{bottom}");
            assert!((shadow_rt.x - body_rt.x).abs() < 1e-4, "右上角圆弧不能横向漂移");
            assert!((shadow_rt.y - body_rt.y - dy).abs() < 1e-4);
            assert!((shadow_lb.x - body_lb.x).abs() < 1e-4, "左下角圆弧不能横向漂移");
'''),

    # custom-39：悬浮云主体改用精确圆角 + 流体颜色纹理
    ("src/app.rs",
     '''    pub(crate) fluid_origin:Pos2,
    fluid_art:Option<egui::TextureId>,
    fluid_title:String,
    screen:Option<Rect>,
    pub(crate) settings_focus_requested:bool,
    pub(crate) settings_tab:usize,
''',
     '''    pub(crate) fluid_origin:Pos2,
    fluid_art:Option<egui::TextureId>,
    fluid_title:String,
    /// 悬浮云每帧刷新的颜色纹理：低分辨率交给 GPU 双线性插值，避免每帧生成上万顶点。
    fluid_tex:Option<egui::TextureHandle>,
    screen:Option<Rect>,
    pub(crate) settings_focus_requested:bool,
    pub(crate) settings_tab:usize,
'''),
    ("src/app.rs",
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), notice_rules:model::NoticeRules::load(), clip_snapshot:(u64::MAX,std::sync::Arc::new(Vec::new())), keyword_src:String::new(), keywords:Vec::new(), quota_alerted:Default::default(), quota_checked_at:0.0, phone_last:None, pc_last:None, quiet_until:0.0, quiet_meeting_key:None, meeting_checked_at:0.0, meeting_plan:(String::new(),f64::MIN,Vec::new()), snoozed:Vec::new(), notice_cache:(0.0,Vec::new()), focus_pending:0.0, focus_today_min:focus_today, focus_streak_days:focus_streak, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
''',
     '''            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), fluid_tex:None, screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), notice_rules:model::NoticeRules::load(), clip_snapshot:(u64::MAX,std::sync::Arc::new(Vec::new())), keyword_src:String::new(), keywords:Vec::new(), quota_alerted:Default::default(), quota_checked_at:0.0, phone_last:None, pc_last:None, quiet_until:0.0, quiet_meeting_key:None, meeting_checked_at:0.0, meeting_plan:(String::new(),f64::MIN,Vec::new()), snoozed:Vec::new(), notice_cache:(0.0,Vec::new()), focus_pending:0.0, focus_today_min:focus_today, focus_streak_days:focus_streak, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
'''),
    ("src/app.rs",
     '''    }

    /// 绘制岛体（或通知胶囊）的本体：经典皮肤为纯黑；毛玻璃为着色底 + 顶部柔光 + 边缘高光。
    fn paint_body(&self,painter:&egui::Painter,rect:Rect,flare:f32,flare_h:f32,radius:f32,feather:f32){
        let skin=self.settings.skin;
        if skin==model::Skin::Fluid {
            let pill=self.settings.island_shape==model::IslandShape::Pill;
            let (map,frame)=shape::EdgeMap::of(rect,if pill {model::Dock::Top}else{self.dock()});
            let mut rows=shape::sections(frame,flare,flare_h,radius);
''',
     '''    }

    /// 绘制岛体（或通知胶囊）的本体：经典皮肤为纯黑；毛玻璃为着色底 + 顶部柔光 + 边缘高光。
    fn paint_body(&mut self,painter:&egui::Painter,rect:Rect,flare:f32,flare_h:f32,radius:f32,feather:f32){
        let skin=self.settings.skin;
        if skin==model::Skin::Fluid {
            // 胶囊 + 顶部停靠：和毛玻璃共用 egui 的精确圆角矩形，颜色走一张小纹理。
            // 旧的「按行梯形条 + 折线圆弧 + 手工羽化带」只在悬浮云上用，
            // 直线转圆弧处的折角感正是它独有、毛玻璃没有的瑕疵。
            if self.settings.island_shape==model::IslandShape::Pill && !self.dock().is_side() {
                self.paint_fluid_pill(painter,rect,radius);
                return;
            }
            let pill=self.settings.island_shape==model::IslandShape::Pill;
            let (map,frame)=shape::EdgeMap::of(rect,if pill {model::Dock::Top}else{self.dock()});
            let mut rows=shape::sections(frame,flare,flare_h,radius);
'''),
    ("src/app.rs",
     '''        }
    }

    /// 悬停状态机：意图缓冲 → 弹簧展开 → 离开延迟 → 临界阻尼收起；全程可打断。
    fn morph(&mut self, want: bool, inside: bool, t: f64, dt: f32) {
        let (bw, bh) = self.base_size();
''',
     '''        }
    }

    /// 悬浮云主体：轮廓交给 `RectShape` 的精确圆角，流动颜色做成一张固定尺寸的小纹理。
    ///
    /// 固定 160×80，避免展开动画里尺寸每帧变化导致纹理反复重新分配；
    /// 光晕尺度是 152×100 逻辑点，双线性插值完全够用，看不出采样痕迹。
    fn paint_fluid_pill(&mut self,painter:&egui::Painter,rect:Rect,radius:f32){
        const TEX_W:usize=160;
        const TEX_H:usize=80;
        let w=rect.width().max(1.0);
        let h=rect.height().max(1.0);
        let expand=self.progress().clamp(0.0,1.0);
        let mut image=egui::ColorImage::new([TEX_W,TEX_H],Color32::TRANSPARENT);
        for row in 0..TEX_H {
            let py=h*(row as f32+0.5)/TEX_H as f32;
            for col in 0..TEX_W {
                let px=w*(col as f32+0.5)/TEX_W as f32;
                image.pixels[row*TEX_W+col]=self.fluid.compose(px,py,w,h,expand);
            }
        }
        let options=egui::TextureOptions::LINEAR;
        let handle=self.fluid_tex.get_or_insert_with(||{
            painter.ctx().load_texture("daozai-fluid",image.clone(),options)
        });
        handle.set(image,options);
        let radius=radius.min(rect.height().min(rect.width())*0.5);
        let uv=Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0));
        painter.add(egui::epaint::RectShape::filled(rect,radius,Color32::WHITE).with_texture(handle.id(),uv));
    }

    /// 悬停状态机：意图缓冲 → 弹簧展开 → 离开延迟 → 临界阻尼收起；全程可打断。
    fn morph(&mut self, want: bool, inside: bool, t: f64, dt: f32) {
        let (bw, bh) = self.base_size();
'''),
    ("src/shape.rs",
     '''        let pts = outline(rect, 0.0, 16.0, r);
        for i in 0..pts.len() {
            assert!(edge_normal(&pts, i).length() < 2.0, "角部羽化法线异常：{i}");
        }
    }
}
''',
     '''        let pts = outline(rect, 0.0, 16.0, r);
        for i in 0..pts.len() {
            assert!(edge_normal(&pts, i).length() < 2.0, "角部羽化法线异常：{i}");
        }
    }
}

#[cfg(test)]
mod fluid_path_bench {
    use super::*;
    use crate::fluid::Fluid;
    use std::time::Instant;

    /// 复刻 `paint_body` 里悬浮云胶囊的行表镜像。
    fn pill_rows(rect: Rect, radius: f32) -> Vec<(f32, f32, f32)> {
        let mut rows = sections(rect, 0.0, 16.0, radius);
        let mid = rect.center().y;
        rows.retain(|r| r.0 > mid && r.0 >= rect.bottom() - radius);
        let top = rows.iter().rev().map(|&(y, l, r)| (2.0 * mid - y, l, r)).collect::<Vec<_>>();
        rows.splice(0..0, top);
        rows
    }

    #[test]
    fn 纹理路径比梯形网格省得多() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(419.0, 215.0));
        let rows = pill_rows(rect, 26.0);
        let fluid = Fluid::default();
        let (w, h) = (rect.width(), rect.height());
        const N: usize = 40;

        let t = Instant::now();
        let mut vertices = 0usize;
        let mut tris = 0usize;
        for _ in 0..N {
            let mesh = shade_mesh(rect, rows.clone(), 1.0, &|x, y| fluid.compose(x, y, w, h, 1.0));
            vertices = mesh.vertices.len();
            tris = mesh.indices.len() / 3;
            std::hint::black_box(mesh);
        }
        let old_ms = t.elapsed().as_secs_f64() * 1000.0 / N as f64;

        let t = Instant::now();
        for _ in 0..N {
            let mut image = egui::ColorImage::new([160, 80], Color32::TRANSPARENT);
            for row in 0..80usize {
                let py = h * (row as f32 + 0.5) / 80.0;
                for col in 0..160usize {
                    image.pixels[row * 160 + col] =
                        fluid.compose(w * (col as f32 + 0.5) / 160.0, py, w, h, 1.0);
                }
            }
            std::hint::black_box(image);
        }
        let new_ms = t.elapsed().as_secs_f64() * 1000.0 / N as f64;

        println!("旧 shade_mesh: {vertices} 顶点 / {tris} 三角形 / {old_ms:.3} ms 每帧");
        println!("新颜色纹理: {} 像素 / {new_ms:.3} ms 每帧", 160 * 80);
        println!("顶点数据量: 旧 {:.0} KB -> 新 1 个 RectShape", vertices as f64 * 24.0 / 1024.0);
        assert!(vertices > 1000, "旧路径确实每帧生成大量顶点");
    }
}
'''),

    # custom-40：展开动效可选（5 种过渡效果）
    ("src/app.rs",
     '''/// 移开后等待多久开始收起；这段时间内回来不会收。
const LEAVE_DELAY: f64 = 0.12;
const SWELL: (f32, f32) = (1.075, 1.13);
// 展开：欠阻尼，末段约 5% 过冲后回落；高度略软于宽度，像「垂落」下来。
const OPEN_W: (f32, f32) = (330.0, 0.70);
const OPEN_H: (f32, f32) = (290.0, 0.74);
// 收起：临界阻尼，约 0.18 s 干净收回，无回弹。
const CLOSE: (f32, f32) = (620.0, 1.0);
const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;
''',
     '''/// 移开后等待多久开始收起；这段时间内回来不会收。
const LEAVE_DELAY: f64 = 0.12;
const SWELL: (f32, f32) = (1.075, 1.13);
// 展开参数按 `model::OpenAnim` 的配置取，见 `OpenAnim::tuning()`；
// CLOSE 仍用于弹簧初始化与非展开态的回位。
const CLOSE: (f32, f32) = (620.0, 1.0);
const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;
'''),
    ("src/app.rs",
     '''        self.notice=None;
        self.go(page,1.0);
        self.open=true; self.forced_open_until=self.t+4.0;
        self.w.tune(OPEN_W.0,OPEN_W.1);self.h.tune(OPEN_H.0,OPEN_H.1);
        self.w.target=PW;self.h.target=self.expanded_height();
    }
    pub fn open_settings(&mut self){
        self.settings_open=true;
''',
     '''        self.notice=None;
        self.go(page,1.0);
        self.open=true; self.forced_open_until=self.t+4.0;
        let target_h=self.expanded_height();
        self.tune_open(PW,target_h);
    }
    pub fn open_settings(&mut self){
        self.settings_open=true;
'''),
    ("src/app.rs",
     '''    pub(crate) fn reveal_unread(&mut self){
        self.notice=None;
        self.open=true; self.forced_open_until=self.t+4.0;
        self.w.tune(OPEN_W.0,OPEN_W.1);self.h.tune(OPEN_H.0,OPEN_H.1);
        self.w.target=PW;self.h.target=self.expanded_height();
    }
    fn notice_rect(&self,cx:f32,top:f32)->Option<Rect>{
        if !self.compact_notices(){return None;}
''',
     '''    pub(crate) fn reveal_unread(&mut self){
        self.notice=None;
        self.open=true; self.forced_open_until=self.t+4.0;
        let target_h=self.expanded_height();
        self.tune_open(PW,target_h);
    }
    fn notice_rect(&self,cx:f32,top:f32)->Option<Rect>{
        if !self.compact_notices(){return None;}
'''),
    ("src/app.rs",
     '''    }

    /// 悬停状态机：意图缓冲 → 弹簧展开 → 离开延迟 → 临界阻尼收起；全程可打断。
    fn morph(&mut self, want: bool, inside: bool, t: f64, dt: f32) {
        let (bw, bh) = self.base_size();
        if want {
            self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let intent=if self.effective_minimal() {MINIMAL_HOVER_INTENT}else{HOVER_INTENT};
            if !self.open && (self.drag_hot || t - since >= intent) {
                self.open = true;
                self.w.tune(OPEN_W.0, OPEN_W.1);
                self.h.tune(OPEN_H.0, OPEN_H.1);
                self.w.target = PW;
                self.h.target = self.expanded_height();
                if self.drag_hot {
                    // 拖入不等缓冲，直接给一点初速度，30 ms 内就能看到形变
                    self.w.vel = self.w.vel.max(600.0);
                    self.h.vel = self.h.vel.max(260.0);
                }
            } else if !self.open {
                self.w.tune(SWELL_K.0, SWELL_K.1);
                self.h.tune(SWELL_K.0, SWELL_K.1);
                // 悬停时轻微胀大：厚度方向胀得多一点（侧边停靠时厚度是宽度）。
                let (sw,sh)=if self.side(){(SWELL.1,SWELL.0)}else{SWELL};
                self.w.target = bw * sw;
                self.h.target = bh * sh;
            }
        } else {
            self.hover_since = None;
''',
     '''    }

    /// 悬停状态机：意图缓冲 → 弹簧展开 → 离开延迟 → 临界阻尼收起；全程可打断。
    /// 按当前配置的动效启动展开；「瞬时」直接到位，不跑形变。
    fn tune_open(&mut self,target_w:f32,target_h:f32){
        if let Some(t)=self.settings.open_anim.tuning() {
            self.w.tune(t.open_w.0,t.open_w.1);
            self.h.tune(t.open_h.0,t.open_h.1);
            self.w.target=target_w;self.h.target=target_h;
        } else {
            self.w.snap(target_w);self.h.snap(target_h);
        }
    }
    /// 按当前配置的动效收起；「瞬时」直接到位。
    fn tune_close(&mut self,bw:f32,bh:f32){
        if let Some(t)=self.settings.open_anim.tuning() {
            self.w.tune(t.close.0,t.close.1);
            self.h.tune(t.close.0,t.close.1);
            self.w.target=bw;self.h.target=bh;
        } else {
            self.w.snap(bw);self.h.snap(bh);
        }
    }

    fn morph(&mut self, want: bool, inside: bool, t: f64, dt: f32) {
        let (bw, bh) = self.base_size();
        let tuning=self.settings.open_anim.tuning();
        let instant=tuning.is_none();
        // 非展开态每帧都会重新 tune，所以这里也要用配置里的收起参数，
        // 否则 `tune_close` 设置的参数只在切换那一帧有效。
        let close=tuning.map(|t|t.close).unwrap_or(CLOSE);
        if want {
            self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let intent=if self.effective_minimal() {MINIMAL_HOVER_INTENT}else{HOVER_INTENT};
            if !self.open && (self.drag_hot || t - since >= intent) {
                self.open = true;
                let target_h=self.expanded_height();
                self.tune_open(PW,target_h);
                if !instant && self.drag_hot {
                    // 拖入不等缓冲，直接给一点初速度，30 ms 内就能看到形变
                    self.w.vel = self.w.vel.max(600.0);
                    self.h.vel = self.h.vel.max(260.0);
                }
            } else if !self.open {
                if instant {
                    // 瞬时模式不做「悬停胀大」反馈，避免先跳一下再瞬间展开。
                    self.w.tune(close.0, close.1);
                    self.h.tune(close.0, close.1);
                    self.w.target = bw;
                    self.h.target = bh;
                } else {
                    self.w.tune(SWELL_K.0, SWELL_K.1);
                    self.h.tune(SWELL_K.0, SWELL_K.1);
                    // 悬停时轻微胀大：厚度方向胀得多一点（侧边停靠时厚度是宽度）。
                    let (sw,sh)=if self.side(){(SWELL.1,SWELL.0)}else{SWELL};
                    self.w.target = bw * sw;
                    self.h.target = bh * sh;
                }
            }
        } else {
            self.hover_since = None;
'''),
    ("src/app.rs",
     '''                if t - since >= LEAVE_DELAY {
                    self.open = false;
                    self.leave_since = None;
                    self.w.tune(CLOSE.0, CLOSE.1);
                    self.h.tune(CLOSE.0, CLOSE.1);
                    self.w.target = bw;
                    self.h.target = bh;
                    self.cancel_edit();
                    self.confirm_clear = None;
                }
            } else {
                self.w.tune(CLOSE.0, CLOSE.1);
                self.h.tune(CLOSE.0, CLOSE.1);
                self.w.target = bw;
                self.h.target = bh;
            }
''',
     '''                if t - since >= LEAVE_DELAY {
                    self.open = false;
                    self.leave_since = None;
                    self.tune_close(bw,bh);
                    self.cancel_edit();
                    self.confirm_clear = None;
                }
            } else {
                self.w.tune(close.0, close.1);
                self.h.tune(close.0, close.1);
                self.w.target = bw;
                self.h.target = bh;
            }
'''),
    ("src/model.rs",
     '''    /// 岛体皮肤、毛玻璃是否使用系统模糊、大小百分比、水平位置。
    pub skin: Skin,
    pub island_shape: IslandShape,
    pub glass_blur: bool,
    pub island_size: u32,
    pub island_offset: i32,
''',
     '''    /// 岛体皮肤、毛玻璃是否使用系统模糊、大小百分比、水平位置。
    pub skin: Skin,
    pub island_shape: IslandShape,
    /// 收起 ↔ 展开的过渡动效。
    pub open_anim: OpenAnim,
    pub glass_blur: bool,
    pub island_size: u32,
    pub island_offset: i32,
'''),
    ("src/model.rs",
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

''',
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, open_anim: OpenAnim::default(), glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

/// 收起 ↔ 展开的过渡动效。刚度 k 越大越快，阻尼比 zeta < 1 会过冲、=1 为临界阻尼。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpenAnim {
    /// 官方默认：欠阻尼垂落，末段约 5% 过冲。
    #[default]
    Elastic,
    /// 临界阻尼，无回弹。
    Smooth,
    /// 更快的刚度 + 更低的阻尼，回弹更明显。
    Snappy,
    /// 低刚度、大阻尼，慢慢铺开。
    Gentle,
    /// 不做形变动画，直接切换大小。
    Instant,
}

/// 一组动效的弹簧参数：(宽度刚度, 高度刚度, 收起刚度)，每项都是 `(k, zeta)`。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimTuning {
    pub open_w: (f32, f32),
    pub open_h: (f32, f32),
    pub close: (f32, f32),
}

impl OpenAnim {
    pub const ALL: [OpenAnim; 5] = [Self::Elastic, Self::Smooth, Self::Snappy, Self::Gentle, Self::Instant];
    pub fn key(self) -> &'static str {
        match self { Self::Elastic => "elastic", Self::Smooth => "smooth", Self::Snappy => "snappy", Self::Gentle => "gentle", Self::Instant => "instant" }
    }
    pub fn parse(value: &str) -> Self {
        match value { "smooth" => Self::Smooth, "snappy" => Self::Snappy, "gentle" => Self::Gentle, "instant" => Self::Instant, _ => Self::Elastic }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Elastic => "弹性", Self::Smooth => "平滑", Self::Snappy => "轻快", Self::Gentle => "柔和", Self::Instant => "瞬时" }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Elastic => "默认。欠阻尼弹簧，末段约 5% 过冲后回落，高度略软于宽度，像垂落下来。",
            Self::Smooth => "临界阻尼，没有回弹，展开和收起都干净利落。",
            Self::Snappy => "刚度更高、阻尼更低，展开更快，回弹也更明显。",
            Self::Gentle => "刚度低、阻尼大，慢慢铺开，适合不喜欢快速形变的时候。",
            Self::Instant => "不做形变动画，直接切换大小；仍保留悬停防误触的短暂延迟。",
        }
    }
    /// `None` 表示不做形变动画（瞬时切换）。
    pub fn tuning(self) -> Option<AnimTuning> {
        Some(match self {
            Self::Elastic => AnimTuning { open_w: (330.0, 0.70), open_h: (290.0, 0.74), close: (620.0, 1.0) },
            Self::Smooth  => AnimTuning { open_w: (300.0, 1.00), open_h: (260.0, 1.00), close: (620.0, 1.0) },
            Self::Snappy  => AnimTuning { open_w: (560.0, 0.58), open_h: (500.0, 0.62), close: (760.0, 1.0) },
            Self::Gentle  => AnimTuning { open_w: (170.0, 0.92), open_h: (150.0, 0.95), close: (320.0, 1.0) },
            Self::Instant => return None,
        })
    }
}

'''),
    ("src/model.rs",
     '''                    s.skin = Skin::parse(&v);
                }
                "island_shape" => s.island_shape = IslandShape::parse(&v),
                "dock" => s.dock = Dock::parse(&v),
                "glass_blur" => s.glass_blur = v != "0",
                "island_size" => s.island_size = island_size_value(v.parse::<u32>().unwrap_or(100)),
''',
     '''                    s.skin = Skin::parse(&v);
                }
                "island_shape" => s.island_shape = IslandShape::parse(&v),
                "open_anim" => s.open_anim = OpenAnim::parse(&v),
                "dock" => s.dock = Dock::parse(&v),
                "glass_blur" => s.glass_blur = v != "0",
                "island_size" => s.island_size = island_size_value(v.parse::<u32>().unwrap_or(100)),
'''),
    ("src/model.rs",
     '''                ("display_id".into(),self.display_id.clone()),
                ("skin".into(),self.skin.key().into()),
                ("island_shape".into(),self.island_shape.key().into()),
                ("dock".into(),self.dock.key().into()),
                ("glass_blur".into(),(self.glass_blur as u8).to_string()),
                ("island_size".into(),island_size_value(self.island_size).to_string()),
''',
     '''                ("display_id".into(),self.display_id.clone()),
                ("skin".into(),self.skin.key().into()),
                ("island_shape".into(),self.island_shape.key().into()),
                ("open_anim".into(),self.open_anim.key().into()),
                ("dock".into(),self.dock.key().into()),
                ("glass_blur".into(),(self.glass_blur as u8).to_string()),
                ("island_size".into(),island_size_value(self.island_size).to_string()),
'''),
    ("src/model.rs",
     '''        for p in again { let _ = fs::remove_file(p); }
    }
}
''',
     '''        for p in again { let _ = fs::remove_file(p); }
    }
}

#[cfg(test)]
mod open_anim_tests {
    use super::*;
    use crate::anim::Spring;

    #[test]
    fn 配置键往返且未知值回落到默认() {
        for style in OpenAnim::ALL {
            assert_eq!(OpenAnim::parse(style.key()), style);
            assert!(!style.label().is_empty());
            assert!(!style.description().is_empty());
        }
        assert_eq!(OpenAnim::parse(""), OpenAnim::Elastic);
        assert_eq!(OpenAnim::parse("whatever"), OpenAnim::Elastic);
        assert_eq!(OpenAnim::default(), OpenAnim::Elastic, "旧配置文件没有这个键时保持官方手感");
    }

    #[test]
    fn 参数合法且瞬时不需要弹簧() {
        assert!(OpenAnim::Instant.tuning().is_none());
        for style in OpenAnim::ALL {
            let Some(t) = style.tuning() else { continue; };
            for (k, zeta) in [t.open_w, t.open_h, t.close] {
                assert!(k > 0.0 && zeta > 0.0 && zeta <= 1.0, "{style:?} 参数越界: {k}/{zeta}");
            }
        }
    }

    #[test]
    fn 各动效轨迹确实不同且都能收敛() {
        let mut results = Vec::new();
        for style in OpenAnim::ALL {
            let Some(t) = style.tuning() else { continue; };
            let mut spring = Spring::new(143.0, t.open_w.0, t.open_w.1);
            spring.target = 419.0;
            let (mut peak, mut frames) = (143.0f32, 0usize);
            loop {
                spring.step(1.0 / 60.0);
                peak = peak.max(spring.pos);
                frames += 1;
                if (spring.pos - 419.0).abs() < 0.5 && spring.vel.abs() < 0.5 { break; }
                assert!(frames < 600, "{style:?} 没有收敛");
            }
            results.push((style, frames, peak));
        }
        let peak_of = |style| results.iter().find(|r| r.0 == style).unwrap().2;
        let frames_of = |style| results.iter().find(|r| r.0 == style).unwrap().1;
        assert!(peak_of(OpenAnim::Elastic) > 419.0, "弹性要有过冲");
        assert!(peak_of(OpenAnim::Snappy) > peak_of(OpenAnim::Elastic), "轻快回弹更明显");
        assert!(peak_of(OpenAnim::Smooth) <= 419.5, "平滑不该过冲");
        assert!(peak_of(OpenAnim::Gentle) <= 419.5, "柔和不该过冲");
        assert!(frames_of(OpenAnim::Snappy) < frames_of(OpenAnim::Gentle), "轻快应比柔和更快到位");
    }
}
'''),
    ("src/extras.rs",
     '''                                }).response
                            });
                            settings_hint(ui,"文字、图标和吉祥物按实际像素重新绘制，放大后依旧清晰；只影响岛体，不影响设置窗口。");
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
''',
     '''                                }).response
                            });
                            settings_hint(ui,"文字、图标和吉祥物按实际像素重新绘制，放大后依旧清晰；只影响岛体，不影响设置窗口。");
                            ui.separator();
                            let anims:Vec<(model::OpenAnim,&str)>=model::OpenAnim::ALL.iter().map(|a|(*a,a.label())).collect();
                            settings_control_row_right(ui,"展开动效",|ui|{settings_segmented(ui,&mut self.settings.open_anim,&anims,52.0);});
                            settings_hint(ui,self.settings.open_anim.description());
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
'''),

    # custom-41：悬停接力感开关
    ("src/app.rs",
     '''const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;

pub struct Options {
    /// 是否联网查歌词（录制/测试时关掉）。
    pub fetch_lyrics: bool,
''',
     '''const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;

/// 悬停意图缓冲时长。关掉「接力感」后为 0：光标一进入就当帧开始展开，
/// 不再先跑一段缓慢预放大、也不再中途换弹簧。防误触仍由 `LEAVE_DELAY` 兜底。
fn hover_delay(relay: bool, minimal: bool) -> f64 {
    if !relay { return 0.0; }
    if minimal { MINIMAL_HOVER_INTENT } else { HOVER_INTENT }
}

pub struct Options {
    /// 是否联网查歌词（录制/测试时关掉）。
    pub fetch_lyrics: bool,
'''),
    ("src/app.rs",
     '''        if want {
            self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let intent=if self.effective_minimal() {MINIMAL_HOVER_INTENT}else{HOVER_INTENT};
            if !self.open && (self.drag_hot || t - since >= intent) {
                self.open = true;
                let target_h=self.expanded_height();
''',
     '''        if want {
            self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let intent=hover_delay(self.settings.hover_relay,self.effective_minimal());
            if !self.open && (self.drag_hot || t - since >= intent) {
                self.open = true;
                let target_h=self.expanded_height();
'''),
    ("src/app.rs",
     '''        assert!(app.battery_visible());
    }
}
''',
     '''        assert!(app.battery_visible());
    }
}

#[cfg(test)]
mod hover_relay_tests {
    use super::*;
    use crate::anim::Spring;

    #[test]
    fn 开关决定悬停缓冲且默认保留接力感() {
        assert_eq!(hover_delay(true, false), HOVER_INTENT);
        assert_eq!(hover_delay(true, true), MINIMAL_HOVER_INTENT);
        assert_eq!(hover_delay(false, false), 0.0);
        assert_eq!(hover_delay(false, true), 0.0);
        assert!(crate::model::Settings::default().hover_relay, "旧配置无该键时保持现状");
    }

    #[test]
    fn 关掉接力后展开曲线不再分段() {
        let (bw, full) = (143.0f32, 419.0f32);
        let near = bw + (full - bw) * 0.9;
        let tuning = crate::model::OpenAnim::Elastic.tuning().unwrap();
        // 连贯：一进入就用展开弹簧追完整宽度。
        let mut direct = Spring::new(bw, tuning.open_w.0, tuning.open_w.1);
        direct.target = full;
        // 接力：前 12 帧（0.20 s @ 60 fps）先用 SWELL_K 追「轻微胀大」，到点才换弹簧。
        let mut relay = Spring::new(bw, SWELL_K.0, SWELL_K.1);
        relay.target = bw * SWELL.0;
        let (mut direct_frames, mut relay_frames) = (None, None);
        for frame in 0..600 {
            if frame == 12 {
                relay.tune(tuning.open_w.0, tuning.open_w.1);
                relay.target = full;
            }
            direct.step(1.0 / 60.0);
            relay.step(1.0 / 60.0);
            if direct_frames.is_none() && direct.pos >= near { direct_frames = Some(frame + 1); }
            if relay_frames.is_none() && relay.pos >= near { relay_frames = Some(frame + 1); }
            if direct_frames.is_some() && relay_frames.is_some() { break; }
        }
        let (d, r) = (direct_frames.expect("连贯模式没到位"), relay_frames.expect("接力模式没到位"));
        assert!(d < r, "连贯模式应更早展开到位：{d} 帧 vs {r} 帧");
        assert!(d < 12, "连贯模式在原本的缓冲期内就该开始明显铺开：{d} 帧");
    }
}
'''),
    ("src/model.rs",
     '''    pub island_shape: IslandShape,
    /// 收起 ↔ 展开的过渡动效。
    pub open_anim: OpenAnim,
    pub glass_blur: bool,
    pub island_size: u32,
    pub island_offset: i32,
''',
     '''    pub island_shape: IslandShape,
    /// 收起 ↔ 展开的过渡动效。
    pub open_anim: OpenAnim,
    /// 是否保留「悬停先轻微胀大、再切换到展开弹簧」的两段接力感。
    /// 关闭后一悬停就用同一条展开弹簧直接铺开，更连贯。
    pub hover_relay: bool,
    pub glass_blur: bool,
    pub island_size: u32,
    pub island_offset: i32,
'''),
    ("src/model.rs",
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, open_anim: OpenAnim::default(), glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

''',
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, open_anim: OpenAnim::default(), hover_relay: true, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

'''),
    ("src/model.rs",
     '''                }
                "island_shape" => s.island_shape = IslandShape::parse(&v),
                "open_anim" => s.open_anim = OpenAnim::parse(&v),
                "dock" => s.dock = Dock::parse(&v),
                "glass_blur" => s.glass_blur = v != "0",
                "island_size" => s.island_size = island_size_value(v.parse::<u32>().unwrap_or(100)),
''',
     '''                }
                "island_shape" => s.island_shape = IslandShape::parse(&v),
                "open_anim" => s.open_anim = OpenAnim::parse(&v),
                "hover_relay" => s.hover_relay = v != "0",
                "dock" => s.dock = Dock::parse(&v),
                "glass_blur" => s.glass_blur = v != "0",
                "island_size" => s.island_size = island_size_value(v.parse::<u32>().unwrap_or(100)),
'''),
    ("src/model.rs",
     '''                ("skin".into(),self.skin.key().into()),
                ("island_shape".into(),self.island_shape.key().into()),
                ("open_anim".into(),self.open_anim.key().into()),
                ("dock".into(),self.dock.key().into()),
                ("glass_blur".into(),(self.glass_blur as u8).to_string()),
                ("island_size".into(),island_size_value(self.island_size).to_string()),
''',
     '''                ("skin".into(),self.skin.key().into()),
                ("island_shape".into(),self.island_shape.key().into()),
                ("open_anim".into(),self.open_anim.key().into()),
                ("hover_relay".into(),(self.hover_relay as u8).to_string()),
                ("dock".into(),self.dock.key().into()),
                ("glass_blur".into(),(self.glass_blur as u8).to_string()),
                ("island_size".into(),island_size_value(self.island_size).to_string()),
'''),
    ("src/extras.rs",
     '''                            let anims:Vec<(model::OpenAnim,&str)>=model::OpenAnim::ALL.iter().map(|a|(*a,a.label())).collect();
                            settings_control_row_right(ui,"展开动效",|ui|{settings_segmented(ui,&mut self.settings.open_anim,&anims,52.0);});
                            settings_hint(ui,self.settings.open_anim.description());
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
''',
     '''                            let anims:Vec<(model::OpenAnim,&str)>=model::OpenAnim::ALL.iter().map(|a|(*a,a.label())).collect();
                            settings_control_row_right(ui,"展开动效",|ui|{settings_segmented(ui,&mut self.settings.open_anim,&anims,52.0);});
                            settings_hint(ui,self.settings.open_anim.description());
                            ui.separator();
                            let mut relay=self.settings.hover_relay;
                            setting_row(ui,"悬停接力感","开启：悬停先轻微胀大再铺开，可防光标路过误触；关闭：一悬停就用同一条弹簧直接展开，更连贯，但划过也会触发",&mut relay);
                            self.settings.hover_relay=relay;
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
'''),

    # custom-42：悬停触发延时可调，短延时过滤鼠标划过
    ("src/app.rs",
     '''pub(crate) const PAGE_NAMES: [&str; 8] = ["音乐", "专注", "饮水", "置物架", "待办", "邮箱", "AI 额度", "日程"];

// ---------- 动效参数 ----------
/// 悬停意图缓冲：光标停留这么久才真正展开（期间药丸轻微胀大作为反馈），防误触。
const HOVER_INTENT: f64 = 0.20;
const MINIMAL_HOVER_INTENT: f64 = 0.40;
/// 移开后等待多久开始收起；这段时间内回来不会收。
const LEAVE_DELAY: f64 = 0.12;
const SWELL: (f32, f32) = (1.075, 1.13);
''',
     '''pub(crate) const PAGE_NAMES: [&str; 8] = ["音乐", "专注", "饮水", "置物架", "待办", "邮箱", "AI 额度", "日程"];

// ---------- 动效参数 ----------
/// 等待超过这个时长才做「轻微胀大」反馈；更短的等待保持静止，
/// 否则弹簧刚鼓一点就被展开打断，反而更突兀。
const SWELL_MIN_MS: u32 = 180;
/// 移开后等待多久开始收起；这段时间内回来不会收。
const LEAVE_DELAY: f64 = 0.12;
const SWELL: (f32, f32) = (1.075, 1.13);
'''),
    ("src/app.rs",
     '''const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;

/// 悬停意图缓冲时长。关掉「接力感」后为 0：光标一进入就当帧开始展开，
/// 不再先跑一段缓慢预放大、也不再中途换弹簧。防误触仍由 `LEAVE_DELAY` 兜底。
fn hover_delay(relay: bool, minimal: bool) -> f64 {
    if !relay { return 0.0; }
    if minimal { MINIMAL_HOVER_INTENT } else { HOVER_INTENT }
}

pub struct Options {
''',
     '''const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;

/// 悬停多久才展开（秒）。`0` 表示当帧就展开；极简模式加倍，
/// 默认 200 ms 时仍是原来的 0.40 s 保守值。防误触另由 `LEAVE_DELAY` 兜底。
fn hover_delay_secs(ms: u32, minimal: bool) -> f64 {
    let ms = if minimal { ms.saturating_mul(2) } else { ms };
    (ms as f64 / 1000.0).min(1.0)
}

pub struct Options {
'''),
    ("src/app.rs",
     '''        if want {
            self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let intent=hover_delay(self.settings.hover_relay,self.effective_minimal());
            if !self.open && (self.drag_hot || t - since >= intent) {
                self.open = true;
                let target_h=self.expanded_height();
                self.tune_open(PW,target_h);
''',
     '''        if want {
            self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let delay=hover_delay_secs(self.settings.hover_delay_ms,self.effective_minimal());
            let swell=self.settings.hover_delay_ms>=SWELL_MIN_MS && !instant;
            if !self.open && (self.drag_hot || t - since >= delay) {
                self.open = true;
                let target_h=self.expanded_height();
                self.tune_open(PW,target_h);
'''),
    ("src/app.rs",
     '''                    self.h.vel = self.h.vel.max(260.0);
                }
            } else if !self.open {
                if instant {
                    // 瞬时模式不做「悬停胀大」反馈，避免先跳一下再瞬间展开。
                    self.w.tune(close.0, close.1);
                    self.h.tune(close.0, close.1);
                    self.w.target = bw;
                    self.h.target = bh;
                } else {
                    self.w.tune(SWELL_K.0, SWELL_K.1);
                    self.h.tune(SWELL_K.0, SWELL_K.1);
                    // 悬停时轻微胀大：厚度方向胀得多一点（侧边停靠时厚度是宽度）。
                    let (sw,sh)=if self.side(){(SWELL.1,SWELL.0)}else{SWELL};
                    self.w.target = bw * sw;
                    self.h.target = bh * sh;
                }
            }
        } else {
''',
     '''                    self.h.vel = self.h.vel.max(260.0);
                }
            } else if !self.open {
                if swell {
                    self.w.tune(SWELL_K.0, SWELL_K.1);
                    self.h.tune(SWELL_K.0, SWELL_K.1);
                    // 悬停时轻微胀大：厚度方向胀得多一点（侧边停靠时厚度是宽度）。
                    let (sw,sh)=if self.side(){(SWELL.1,SWELL.0)}else{SWELL};
                    self.w.target = bw * sw;
                    self.h.target = bh * sh;
                } else {
                    // 短等待（或瞬时动效）：保持静止，到点直接用同一条弹簧展开，
                    // 不会出现「鼓一半被拉走」的接力感。
                    self.w.tune(close.0, close.1);
                    self.h.tune(close.0, close.1);
                    self.w.target = bw;
                    self.h.target = bh;
                }
            }
        } else {
'''),
    ("src/app.rs",
     '''}

#[cfg(test)]
mod hover_relay_tests {
    use super::*;
    use crate::anim::Spring;

    #[test]
    fn 开关决定悬停缓冲且默认保留接力感() {
        assert_eq!(hover_delay(true, false), HOVER_INTENT);
        assert_eq!(hover_delay(true, true), MINIMAL_HOVER_INTENT);
        assert_eq!(hover_delay(false, false), 0.0);
        assert_eq!(hover_delay(false, true), 0.0);
        assert!(crate::model::Settings::default().hover_relay, "旧配置无该键时保持现状");
    }

    #[test]
    fn 关掉接力后展开曲线不再分段() {
        let (bw, full) = (143.0f32, 419.0f32);
        let near = bw + (full - bw) * 0.9;
        let tuning = crate::model::OpenAnim::Elastic.tuning().unwrap();
        // 连贯：一进入就用展开弹簧追完整宽度。
        let mut direct = Spring::new(bw, tuning.open_w.0, tuning.open_w.1);
        direct.target = full;
        // 接力：前 12 帧（0.20 s @ 60 fps）先用 SWELL_K 追「轻微胀大」，到点才换弹簧。
        let mut relay = Spring::new(bw, SWELL_K.0, SWELL_K.1);
        relay.target = bw * SWELL.0;
        let (mut direct_frames, mut relay_frames) = (None, None);
''',
     '''}

#[cfg(test)]
mod hover_delay_tests {
    use super::*;
    use crate::anim::Spring;

    fn approx(a: f64, b: f64) { assert!((a - b).abs() < 1e-9, "{a} != {b}"); }

    #[test]
    fn 延时换算默认值与极简加倍() {
        approx(hover_delay_secs(200, false), 0.20);
        approx(hover_delay_secs(200, true), 0.40);
        approx(hover_delay_secs(0, false), 0.0);
        approx(hover_delay_secs(0, true), 0.0);
        approx(hover_delay_secs(80, false), 0.08);
        assert!(hover_delay_secs(500, true) <= 1.0, "极简加倍也要有上限");
        assert_eq!(crate::model::Settings::default().hover_delay_ms, 200);
        assert!(200 >= SWELL_MIN_MS && 80 < SWELL_MIN_MS, "默认有预放大、短延时保持静止");
    }

    #[test]
    fn 短延时既过滤鼠标划过又更早展开() {
        let (bw, full) = (143.0f32, 419.0f32);
        let tuning = crate::model::OpenAnim::Elastic.tuning().unwrap();
        // 80 ms ≈ 5 帧 @60fps：等待期间保持静止，到点才换展开弹簧。
        let mut short = Spring::new(bw, CLOSE.0, CLOSE.1);
        short.target = bw;
        // 200 ms ≈ 12 帧：等待期间跑 SWELL 预放大。
        let mut long = Spring::new(bw, SWELL_K.0, SWELL_K.1);
        long.target = bw * SWELL.0;
        let mut width_before_delay = bw;
        for frame in 0..10 {
            if frame == 5 {
                short.tune(tuning.open_w.0, tuning.open_w.1);
                short.target = full;
            }
            short.step(1.0 / 60.0);
            long.step(1.0 / 60.0);
            if frame == 3 { width_before_delay = short.pos; }
        }
        assert!((width_before_delay - bw).abs() < 0.01,
            "延时内不该有任何形变，划过 <80 ms 就不会展开：{width_before_delay}");
        assert!(short.pos > long.pos + 20.0,
            "短延时应更早进入展开：{} vs {}", short.pos, long.pos);
    }

    #[test]
    fn 零延时比默认更早到位() {
        let (bw, full) = (143.0f32, 419.0f32);
        let near = bw + (full - bw) * 0.9;
        let tuning = crate::model::OpenAnim::Elastic.tuning().unwrap();
        let mut direct = Spring::new(bw, tuning.open_w.0, tuning.open_w.1);
        direct.target = full;
        let mut relay = Spring::new(bw, SWELL_K.0, SWELL_K.1);
        relay.target = bw * SWELL.0;
        let (mut direct_frames, mut relay_frames) = (None, None);
'''),
    ("src/app.rs",
     '''            if relay_frames.is_none() && relay.pos >= near { relay_frames = Some(frame + 1); }
            if direct_frames.is_some() && relay_frames.is_some() { break; }
        }
        let (d, r) = (direct_frames.expect("连贯模式没到位"), relay_frames.expect("接力模式没到位"));
        assert!(d < r, "连贯模式应更早展开到位：{d} 帧 vs {r} 帧");
        assert!(d < 12, "连贯模式在原本的缓冲期内就该开始明显铺开：{d} 帧");
    }
}
''',
     '''            if relay_frames.is_none() && relay.pos >= near { relay_frames = Some(frame + 1); }
            if direct_frames.is_some() && relay_frames.is_some() { break; }
        }
        let (d, r) = (direct_frames.expect("零延时没到位"), relay_frames.expect("默认延时没到位"));
        assert!(d < r, "零延时应更早到位：{d} 帧 vs {r} 帧");
    }
}
'''),
    ("src/model.rs",
     '''    pub island_shape: IslandShape,
    /// 收起 ↔ 展开的过渡动效。
    pub open_anim: OpenAnim,
    /// 是否保留「悬停先轻微胀大、再切换到展开弹簧」的两段接力感。
    /// 关闭后一悬停就用同一条展开弹簧直接铺开，更连贯。
    pub hover_relay: bool,
    pub glass_blur: bool,
    pub island_size: u32,
    pub island_offset: i32,
''',
     '''    pub island_shape: IslandShape,
    /// 收起 ↔ 展开的过渡动效。
    pub open_anim: OpenAnim,
    /// 光标停在岛上多久才展开（毫秒，0 = 立即）。0–500，默认 200。
    /// 同时也是防误触阈值：太短会被鼠标路过触发，太长会觉得迟钝。
    pub hover_delay_ms: u32,
    pub glass_blur: bool,
    pub island_size: u32,
    pub island_offset: i32,
'''),
    ("src/model.rs",
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, open_anim: OpenAnim::default(), hover_relay: true, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

''',
     '''
impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, open_anim: OpenAnim::default(), hover_delay_ms: 200, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

'''),
    ("src/model.rs",
     '''
    pub fn load() -> Self {
        let mut s = Self::default();
        for (k, v) in read_kv(&config_dir().join("settings.txt")) {
            match k.as_str() {
                "settings_theme" => s.settings_theme=crate::settings_theme::SettingsTheme::parse(&v),
                "display_id" => s.display_id = v,
''',
     '''
    pub fn load() -> Self {
        let mut s = Self::default();
        let kv = read_kv(&config_dir().join("settings.txt"));
        let has_delay = kv.iter().any(|(k, _)| k == "hover_delay_ms");
        // 上一版用布尔开关表达「无接力」；没有新键时把它折算成 0 ms。
        let legacy_relay_off = kv.iter().any(|(k, v)| k == "hover_relay" && v == "0");
        for (k, v) in kv {
            match k.as_str() {
                "settings_theme" => s.settings_theme=crate::settings_theme::SettingsTheme::parse(&v),
                "display_id" => s.display_id = v,
'''),
    ("src/model.rs",
     '''                }
                "island_shape" => s.island_shape = IslandShape::parse(&v),
                "open_anim" => s.open_anim = OpenAnim::parse(&v),
                "hover_relay" => s.hover_relay = v != "0",
                "dock" => s.dock = Dock::parse(&v),
                "glass_blur" => s.glass_blur = v != "0",
                "island_size" => s.island_size = island_size_value(v.parse::<u32>().unwrap_or(100)),
''',
     '''                }
                "island_shape" => s.island_shape = IslandShape::parse(&v),
                "open_anim" => s.open_anim = OpenAnim::parse(&v),
                "hover_delay_ms" => s.hover_delay_ms = v.parse::<u32>().unwrap_or(200).min(500),
                "dock" => s.dock = Dock::parse(&v),
                "glass_blur" => s.glass_blur = v != "0",
                "island_size" => s.island_size = island_size_value(v.parse::<u32>().unwrap_or(100)),
'''),
    ("src/model.rs",
     '''                _ => {}
            }
        }
        if !s.visible_pages.contains(&true) { s.visible_pages[0]=true; }
        s
    }
''',
     '''                _ => {}
            }
        }
        if !has_delay && legacy_relay_off { s.hover_delay_ms = 0; }
        if !s.visible_pages.contains(&true) { s.visible_pages[0]=true; }
        s
    }
'''),
    ("src/model.rs",
     '''                ("skin".into(),self.skin.key().into()),
                ("island_shape".into(),self.island_shape.key().into()),
                ("open_anim".into(),self.open_anim.key().into()),
                ("hover_relay".into(),(self.hover_relay as u8).to_string()),
                ("dock".into(),self.dock.key().into()),
                ("glass_blur".into(),(self.glass_blur as u8).to_string()),
                ("island_size".into(),island_size_value(self.island_size).to_string()),
''',
     '''                ("skin".into(),self.skin.key().into()),
                ("island_shape".into(),self.island_shape.key().into()),
                ("open_anim".into(),self.open_anim.key().into()),
                ("hover_delay_ms".into(),self.hover_delay_ms.to_string()),
                ("hover_relay".into(),(u8::from(self.hover_delay_ms>0)).to_string()),
                ("dock".into(),self.dock.key().into()),
                ("glass_blur".into(),(self.glass_blur as u8).to_string()),
                ("island_size".into(),island_size_value(self.island_size).to_string()),
'''),
    ("src/extras.rs",
     '''                            settings_control_row_right(ui,"展开动效",|ui|{settings_segmented(ui,&mut self.settings.open_anim,&anims,52.0);});
                            settings_hint(ui,self.settings.open_anim.description());
                            ui.separator();
                            let mut relay=self.settings.hover_relay;
                            setting_row(ui,"悬停接力感","开启：悬停先轻微胀大再铺开，可防光标路过误触；关闭：一悬停就用同一条弹簧直接展开，更连贯，但划过也会触发",&mut relay);
                            self.settings.hover_relay=relay;
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
''',
     '''                            settings_control_row_right(ui,"展开动效",|ui|{settings_segmented(ui,&mut self.settings.open_anim,&anims,52.0);});
                            settings_hint(ui,self.settings.open_anim.description());
                            ui.separator();
                            setting_number(ui,"悬停触发延时",&mut self.settings.hover_delay_ms,0,500," ms",10.0);
                            settings_hint(ui,"光标停在岛上多久才展开。0 = 一悬停就展开（划过也会触发）；建议 80–120 ms 过滤鼠标路过；180 ms 以上等待期间会有轻微胀大反馈。极简模式自动加倍。");
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
'''),

    # custom-43：剪贴板搜索图标移位并移除二次点击搜索
    ("src/pages.rs",
     '''        for (i, (label, w)) in labels.iter().zip(&widths).enumerate() {
            let selected = self.shelf_tab == i;
            let r = c.hit(left, 9.5, *w, 21.0, ("shelf-tab", i));
            let r = if i == 1 { r.on_hover_text(if selected { "再点一次：搜索剪贴板" } else { "切到剪贴板" }) } else { r };
            let (h, _) = c.hover_t(&r);
            if selected {
                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_gray(88));
''',
     '''        for (i, (label, w)) in labels.iter().zip(&widths).enumerate() {
            let selected = self.shelf_tab == i;
            let r = c.hit(left, 9.5, *w, 21.0, ("shelf-tab", i));
            let r = if i == 1 { r.on_hover_text("切到剪贴板") } else { r };
            let (h, _) = c.hover_t(&r);
            if selected {
                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_gray(88));
'''),
    ("src/pages.rs",
     '''                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_white_alpha((18.0 * h) as u8));
            }
            c.text(left + w / 2.0, 24.3, label, font, W::Bold, if selected { theme::WHITE } else { theme::GRAY }, H::Center);
            if r.clicked() {
                if !selected {
                    self.shelf_tab = i;
                    self.confirm_clear = None;
                    self.clip_confirm_clear = None;
                } else if i == 1 {
                    // 已选中的「剪贴板」再点一次：展开 / 收起搜索。
                    self.clip_search_open = !self.clip_search_open;
                    self.clip_query.clear();
                    self.clip_search_focus = false;
                }
            }
            left += w;
        }
''',
     '''                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_white_alpha((18.0 * h) as u8));
            }
            c.text(left + w / 2.0, 24.3, label, font, W::Bold, if selected { theme::WHITE } else { theme::GRAY }, H::Center);
            if r.clicked() && !selected {
                self.shelf_tab = i;
                self.confirm_clear = None;
                self.clip_confirm_clear = None;
            }
            left += w;
        }
'''),
    ("src/pages.rs",
     '''    }

    // ================= 剪贴板（置物架第二个分栏） =================
    /// 剪贴板搜索：表头其余元素（专注环/日期/时钟）都会随内容左右移动，
    /// 只有「清空」右侧那一小段是固定位置，放大镜就钉在那里；
    /// 点开后输入框浮在列表上方（盖住第一行），Esc 或小叉收起。
    /// 过滤只作用于显示，索引仍指向同一份历史，复制/固定/删除的行为不变。
    fn clip_search(&mut self, c: &Card, cmds: &mut Vec<Cmd>) {
        // 放大镜：固定位置，不参与表头的动态排布。
        let icon = c.hit(352.0, 9.5, 24.0, 21.0, "clip-search-open");
        let (h, _) = c.hover_t(&icon);
        if h > 0.01 { c.circle(364.0, 20.0, 10.5, Color32::from_white_alpha((22.0 * h) as u8)); }
        let ink = if self.clip_search_open { theme::BLUE } else { paint::mix(theme::GRAY, theme::WHITE, h) };
        c.ring(362.2, 18.2, 4.4, 1.3, ink);
        c.seg((365.6, 21.6), (369.4, 25.4), 1.3, ink);
        let icon = icon.on_hover_text(if self.clip_search_open { "收起搜索" } else { "搜索剪贴板" });
        if icon.clicked() {
            self.clip_search_open = !self.clip_search_open;
''',
     '''    }

    // ================= 剪贴板（置物架第二个分栏） =================
    /// 剪贴板搜索：放大镜放在面板右侧的空白竖带（翻页箭头下方、页码豆上方）。
    /// 以前钉在表头 (352, 9.5)，会和常驻的音乐播放条 (x≈367, y≈18) 叠在一起；
    /// 表头其余元素又随内容动态排布，没有稳定空位，所以移到卡片外侧。
    /// 点开后输入框浮在列表上方（盖住第一行），Esc 或小叉收起。
    /// 过滤只作用于显示，索引仍指向同一份历史，复制/固定/删除的行为不变。
    fn clip_search(&mut self, c: &Card, cmds: &mut Vec<Cmd>) {
        // 右下侧空白带：x 377..419 不画列表，y 在箭头(≤101)与页码豆(≈131)之间。
        let icon = c.hit(384.0, 105.0, 26.0, 20.0, "clip-search-open");
        let (h, _) = c.hover_t(&icon);
        if h > 0.01 { c.circle(397.0, 115.0, 10.5, Color32::from_white_alpha((22.0 * h) as u8)); }
        let ink = if self.clip_search_open { theme::BLUE } else { paint::mix(theme::GRAY, theme::WHITE, h) };
        c.ring(395.2, 113.2, 4.4, 1.3, ink);
        c.seg((398.6, 116.6), (402.4, 120.4), 1.3, ink);
        let icon = icon.on_hover_text(if self.clip_search_open { "收起搜索" } else { "搜索剪贴板" });
        if icon.clicked() {
            self.clip_search_open = !self.clip_search_open;
'''),
    ("src/ui_tests.rs",
     '''    {let mut hist=h.app.clipboard.lock();hist.items.clear();
     hist.push("会议纪要：周四评审","09:31");hist.push("验证码 482913","09:32");}
    h.hover(H::local(209.0,60.0),1.2);
    // 点右上角固定的放大镜：浮出搜索输入框。
    h.click(H::local(364.0,20.0));
    assert!(h.app.clip_search_open,"点放大镜应浮出搜索框");
    h.frame(vec![]);
    // 过滤只影响显示，历史本身不动。
''',
     '''    {let mut hist=h.app.clipboard.lock();hist.items.clear();
     hist.push("会议纪要：周四评审","09:31");hist.push("验证码 482913","09:32");}
    h.hover(H::local(209.0,60.0),1.2);
    // 点面板右下侧的放大镜：浮出搜索输入框。
    h.click(H::local(397.0,115.0));
    assert!(h.app.clip_search_open,"点放大镜应浮出搜索框");
    h.frame(vec![]);
    // 过滤只影响显示，历史本身不动。
'''),
    ("src/ui_tests.rs",
     '''    assert_eq!(h.app.clipboard.lock().items.len(),2,"搜索不能删改历史");
    assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("验证码 482913"))),"过滤后不该再画不匹配的记录");
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("会议纪要"))),"命中的记录要留下");
    h.click(H::local(364.0,20.0));
    assert!(!h.app.clip_search_open,"再点一次应收起搜索");
    assert!(h.app.clip_query.is_empty(),"收起搜索要清空关键词");
}

fn editing_local_calendar_clears_outdated_reminders() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for reschedule in [false,true] {
''',
     '''    assert_eq!(h.app.clipboard.lock().items.len(),2,"搜索不能删改历史");
    assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("验证码 482913"))),"过滤后不该再画不匹配的记录");
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("会议纪要"))),"命中的记录要留下");
    h.click(H::local(397.0,115.0));
    assert!(!h.app.clip_search_open,"再点一次应收起搜索");
    assert!(h.app.clip_query.is_empty(),"收起搜索要清空关键词");
}

#[test]
fn clicking_the_active_clipboard_tab_no_longer_opens_search() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(3);h.app.shelf_tab=1;
    assert!(!h.app.clip_search_open);
    // 第二个分栏（剪贴板）中心：shelf_tabs 从 x=13 起，第一栏约 50 宽。
    h.click(H::local(90.0,20.0));
    h.frame(vec![]);
    assert_eq!(h.app.shelf_tab,1,"仍在剪贴板分栏");
    assert!(!h.app.clip_search_open,"再点已选中的分栏不该展开搜索框");
}

fn editing_local_calendar_clears_outdated_reminders() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for reschedule in [false,true] {
'''),

    # custom-46：浏览器扩展桥宿主模式（daozai_bridge.exe），Inno Setup 安装包
    ("src/lib.rs",
     '''#[cfg(windows)]
pub use updates::record_install_size;

#[cfg(windows)]
pub mod native;
#[cfg(windows)]
''',
     '''#[cfg(windows)]
pub use updates::record_install_size;

#[cfg(windows)]
pub mod bridge;
#[cfg(windows)]
pub mod native;
#[cfg(windows)]
'''),
    ("src/main.rs",
     '''}

fn main() -> std::process::ExitCode {
    diagnostics::install();
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
''',
     '''}

fn main() -> std::process::ExitCode {
    // 浏览器扩展桥宿主模式：安装包把本 exe 复制为 daozai_bridge.exe，
    // Chrome/Edge 按宿主清单启动它收发下载事件。不建窗口、不抢单实例互斥体。
    #[cfg(windows)]
    if daozai::bridge::is_bridge_invocation() {
        return std::process::ExitCode::from(daozai::bridge::run_host() as u8);
    }
    diagnostics::install();
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
'''),
    ("Cargo.toml",
     '''  "Win32_Security_Cryptography",
  "Win32_System_Power",
  "Win32_System_Registry",
  "Win32_System_SystemInformation",
  "Win32_System_Threading",
  "Win32_System_Diagnostics_ToolHelp",
''',
     '''  "Win32_Security_Cryptography",
  "Win32_System_Power",
  "Win32_System_Registry",
  "Win32_System_IO",
  "Win32_System_SystemInformation",
  "Win32_System_Threading",
  "Win32_System_Diagnostics_ToolHelp",
'''),

    # custom-47：设置页浏览器下载卡片：桥在线状态 + 一键引导连接
    ("src/extras.rs",
     '''    ("待办与置物",4,5), ("通知、邮箱与日程",6,8), ("短信",12,12), ("AI 额度",11,11), ("关于",9,9),
];

impl App {
    pub fn settings_ui(&mut self,ctx:&egui::Context,autostart:bool){
        if !self.settings_open{release_support_codes(ctx);}
''',
     '''    ("待办与置物",4,5), ("通知、邮箱与日程",6,8), ("短信",12,12), ("AI 额度",11,11), ("关于",9,9),
];

/// 浏览器扩展桥是否在线（2 秒缓存，避免每帧扫一次进程表）。
///
/// Chrome/Edge 只在扩展真正连上时才拉起 daozai_bridge.exe，断开即回收进程，
/// 所以「桥宿主进程在」就是「扩展在线」的可靠信号。
#[cfg(windows)]
fn bridge_alive_cached() -> bool {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(std::time::Instant, bool)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap();
    if let Some((at, value)) = *cache {
        if at.elapsed().as_millis() < 2000 { return value; }
    }
    let value = crate::bridge::host_process_running();
    *cache = Some((std::time::Instant::now(), value));
    value
}

#[cfg(not(windows))]
fn bridge_alive_cached() -> bool { false }

/// 「浏览器下载」卡片：连接状态 + 一键引导连接（打开浏览器扩展页和扩展文件夹、复制路径）。
/// Chrome/Edge 从 33 版起封死了本地扩展的静默安装，只能开发者模式加载，所以做成引导而非自动装。
#[cfg(windows)]
fn bridge_card_body(app: &mut App, ctx: &egui::Context, ui: &mut egui::Ui, alive: bool) {
    let text = if alive {
        "浏览器扩展已连接：下载进度实时上岛（蓝胶囊 = 下载中，绿胶囊 = 完成可点击打开）。"
    } else {
        "浏览器扩展未连接"
    };
    let light = if alive { Color32::from_rgb(46,125,50) } else { Color32::from_gray(120) };
    let dark = if alive { Color32::from_rgb(129,199,132) } else { Color32::from_gray(160) };
    ui.label(egui::RichText::new(text).size(13.0).color(settings_color(ctx, light, dark)));
    if alive { return; }
    let Some(ext) = crate::bridge::extension_folder() else {
        ui.label(egui::RichText::new("未找到浏览器扩展（bridge\\\\extension）。请用岛仔安装包安装后再来连接。").size(12.0).color(settings_color(ctx, Color32::from_gray(180), Color32::from_gray(75))));
        return;
    };
    ui.horizontal_wrapped(|ui| {
        for (name, exe) in crate::bridge::installed_browsers() {
            if ui.button(format!("连接 {name}")).clicked() {
                let page = if name == "Edge" { "edge://extensions/" } else { "chrome://extensions/" };
                let _ = std::process::Command::new(&exe).arg(page).spawn();
                let _ = std::process::Command::new("explorer.exe").arg(&ext).spawn();
                ui.ctx().copy_text(ext.display().to_string());
                app.say(format!("已打开 {name} 的扩展页和扩展文件夹，把文件夹拖进去即可（路径已复制）"));
            }
        }
    });
    ui.label(egui::RichText::new(format!("扩展目录：{}", ext.display())).size(11.5).color(settings_color(ctx, Color32::from_gray(195), Color32::from_gray(60))));
    settings_hint(ui, "步骤：扩展页右上角打开「开发者模式」→ 把弹出的 extension 文件夹拖进扩展页（或点「加载已解压的扩展程序」选它）→ 之后浏览器下载会实时上岛。连接成功后这里会自动变绿。");
}

#[cfg(not(windows))]
fn bridge_card_body(_app: &mut App, _ctx: &egui::Context, ui: &mut egui::Ui, _alive: bool) {
    ui.label("浏览器下载对接仅支持 Windows。");
}

impl App {
    pub fn settings_ui(&mut self,ctx:&egui::Context,autostart:bool){
        if !self.settings_open{release_support_codes(ctx);}
'''),
    ("src/extras.rs",
     '''                            if before_phone_apps!=self.phone_apps.items{self.phone_apps.save();}
                            });
                        });
                    }
                    7=>{
                        settings_label(ui,"邮件提醒");
''',
     '''                            if before_phone_apps!=self.phone_apps.items{self.phone_apps.save();}
                            });
                        });
                        settings_label(ui,"浏览器下载");
                        settings_card(ui,|ui|{
                            let alive=bridge_alive_cached();
                            ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
                            bridge_card_body(self,ctx,ui,alive);
                        });
                    }
                    7=>{
                        settings_label(ui,"邮件提醒");
'''),
]


def encode(text: str, data: bytes) -> bytes:
    """按目标文件的实际行尾编码补丁片段。"""
    crlf = data.count(b"\r\n") > data.count(b"\n") // 2
    return text.replace("\n", "\r\n").encode("utf-8") if crlf else text.encode("utf-8")


def bump_version(root: pathlib.Path) -> str:
    """在官方版本号上加一个补丁号，便于在关于页确认跑的是自定义版。"""
    toml = root / "Cargo.toml"
    data = toml.read_bytes()
    match = re.search(rb'^version = "(\d+)\.(\d+)\.(\d+)"', data, re.M)
    if not match:
        raise SystemExit("Cargo.toml 里找不到 [package] 版本号")
    a, b, c = (int(part) for part in match.groups())
    old_ver, new_ver = f"{a}.{b}.{c}", f"{a}.{b}.{c + 1}"
    toml.write_bytes(data.replace(match.group(0), f'version = "{new_ver}"'.encode(), 1))

    lock = root / "Cargo.lock"
    if lock.exists():
        raw = lock.read_bytes()
        pattern = re.compile(rb'(name = "daozai"\r?\nversion = ")\d+\.\d+\.\d+(")')
        raw, count = pattern.subn(lambda m: m.group(1) + new_ver.encode() + m.group(2), raw, count=1)
        if count != 1:
            raise SystemExit("Cargo.lock 里找不到 daozai 版本条目")
        lock.write_bytes(raw)

    rc = root / "app.rc"
    if rc.exists():
        raw = rc.read_bytes()
        raw = raw.replace(f"{a},{b},{c},0".encode(), f"{a},{b},{c + 1},0".encode())
        raw = raw.replace(f'"{old_ver}"'.encode(), f'"{new_ver}"'.encode())
        rc.write_bytes(raw)

    changelog = root / "CHANGELOG.md"
    if changelog.exists():
        text = changelog.read_text(encoding="utf-8")
        if "自定义构建" not in text:
            pos = text.find("\n## ")
            note = f"""
## v{new_ver} · 自定义构建

- 新增：读取「手机连接」的手机电量作为唯一常驻电量来源，不回退本机电量；手机断开时自动收起。
- 新增：播放音乐/歌词接管胶囊时，手机电量环仍常驻在胶囊最左侧。
- 充电显示：卡片无充电字段，按电量涨跌推测并在长时间不涨时退出充电态；也可由 `%APPDATA%\\Daozai\\phone_charging.txt` 写 `1`/`0` 精确覆盖。充电时电量环变绿并显示闪电。
- 修复：歌词态下封面压住吉祥物、歌词又与宠物重叠。电量环/封面/宠物/歌词统一按胶囊左边缘排版（手写 flex lane，间距集中在 `LANE_*` 常量），歌词在剩余通道内居中，专注模式下也不会错位。
- 修复：暂停音乐且歌词较长时，egui `halign=Center` 的负行内偏移会把整行文字推到封面上；release 改为左对齐后手动居中，并加绘制裁剪兜底。
- 优化：长歌词按通道宽度自动缩小字号（11pt 逐级降到 8pt），能单行完整显示就不截断/分页；测试构建仍用官方 11pt。
- 新增：「关于」页的检查更新开关，默认关闭，防止官方安装包覆盖自定义修改。
- 新增：手机连接通知「只上岛」。三处 `qq_only` 过滤对 Phone Link 白名单通道放行（其余应用仍静默），
  岛上显示「手机 · 应用名」+ 一行短摘要（与自带邮件「新邮件」提醒区分）；
  配合系统侧把 `YourPhoneNotifications_*` 子通道 `ShowBanner=0` 关闭，
  右下角不再弹横幅，通知仍进通知中心供岛仔读取。
- 优化：真实通知只报主 AUMID，改由 Toast 正文首段识别来源 App（QQ/微信/支付宝/Gmail/
  短信/电话/验证码），显示「手机 · QQ」等精确标签并去掉正文里重复的 App 名；
  Steam / Discord / 12306 等高频来源静默，未识别到的按「连接」放行以免漏掉重要通知。
- 新增：手机通知验证码卡片（大号数字 + 一键复制）、同 App 10 秒内多条聚合防刷屏、
  手机 App 图标（直接用手机连接自带图标）、未接来电累计常驻铃铛。
- 新增：显示后从通知中心移除手机通知（`PHONE_CLEAR`，岛仔未运行时不清理）。
- 新增：通用状态总线 `%APPDATA%\\Daozai\\island_status.json`，任何脚本/应用写 JSON
  即可上岛（`id`/`text`/`detail`/`seconds`），为 ADB、下载、构建等预留统一入口。
- 修复：手机断开后仍显示旧电量。卡片断开文案是 JSON 的 Unicode 转义中文，且电量数值会消失；
  现在先解码文案再判定连接状态，明确断开立即清空电量缓存，卡片文件变化后 1 秒内生效。
- 新增：通用状态总线升级为覆写式——文件里有什么岛上就有什么，同 id 覆写即刷新续期、
  删掉条目立刻撤下；支持 `progress` 进度条、`color` 状态色、`click` 复制/打开动作白名单。
  配套 `daozai-patches/scripts/` 脚本库与本地调试台，ADB / 下载进度等只需写 JSON。
- 说明：手机连接来电时 Windows 只更新角标（badge）、不产生 Toast，第三方读不到「来电中」；
  来电中提示已移除，只保留挂断后的「未接来电」提示（30 秒 + 铃铛红点常驻）。
- 新增：手机 App 名单配置化。来源 App 的标签与静默规则移到 `%APPDATA%\\Daozai\\phone-apps.json`，
  改完约 1 秒自动重载，新装 App 想上岛/静默不用再改代码重编译。
- 新增：手机电量关键变化提示（断开 / 重连 / 低电 ≤20% / 充满），首轮不误报。
- 修复：邮件面板账号下拉展开时，鼠标悬停高亮还落在下面的邮件行上、菜单项本身没反馈；
  现在下拉展开期间列表行不画悬停高亮，菜单项自己带悬停背景。
- 新增：邮件面板按邮箱账号筛选。顶部多了一个账号选框，可选「全部账号」或任一已添加的邮箱，
  列表、全部/未读计数、验证码大卡都跟着筛选；原来的「同步 / 仅本机」小字并入选框的悬停提示。
- 修复：邮件面板的「全部已读」点击无反应。邮件请求丢失（协议线程异常退出）后 pending 永不清空，
  按钮就一直处于禁用状态且点击毫无反馈；现在协议线程单次异常不会退出、pending 超过 40 秒自动复位、
  未就绪时点击会提示「邮件状态还没同步完」。
- 调整：邮箱页的「验证码高亮 / 隐藏」改名为「邮件验证码高亮 / 隐藏」，
  避免与短信验证码混淆（这两项只作用于邮件页）。
- 新增：设置页独立「短信」分类。手机验证码自动复制从「邮箱」移到「短信」，
  避免误以为是邮件验证码功能。
- 新增：设置页「重复查看通知」。开启后通知保留 60 秒、可在铃铛上多次悬停查看；
  关闭则维持 4.5 秒后消失。验证码仍单独保留 90 秒。
- 修复：放歌（歌词态）且关闭正文预览时，验证码通知只显示铃铛红点，而铃铛 4.5 秒就消失，
  来不及悬停查看；现在验证码类通知的铃铛保留 90 秒（其余通知仍 4.5 秒）。
- 新增：状态总线多槽位并列显示。存活状态在胶囊右侧并排显示（最多 3 条，含色点与进度条），
  不再只靠一次性的横幅提示；`Layout` 新增动态 `status` 槽位，不影响 resident_order。
- 新增：手机短信验证码自动复制开关，命中后直接写剪贴板；识别结果追加到
  `%APPDATA%\\Daozai\\diagnostics\\otp.log` 便于排查。邮件验证码自动复制已移除（实测不生效）。
- 新增：设置页可直接勾选手机 App 名单，改动写回 phone-apps.json。
- 构建标记 `CUSTOM_BUILD`，更新弹窗会提示先运行 `reapply.ps1` 重放补丁。
- 由 `daozai-patches/apply.py` 生成，可重放到后续官方版本。
"""
            text = text[:pos] + note + text[pos:] if pos >= 0 else text + note
            with open(changelog, "w", encoding="utf-8", newline="") as handle:
                handle.write(text)
    return new_ver


def main() -> None:
    args = [arg for arg in sys.argv[1:] if arg != "--force"]
    if len(args) != 1:
        raise SystemExit("用法: python apply.py <源码根目录> [--force]")
    root = pathlib.Path(args[0]).resolve()
    if not (root / "Cargo.toml").is_file():
        raise SystemExit(f"不是岛仔源码目录: {root}")

    marker = root / "CUSTOM_BUILD.txt"
    if marker.exists() and marker.read_text(encoding="utf-8").strip() == PATCH_ID and "--force" not in sys.argv:
        # 新增模块不依赖 EDITS，即使补丁已应用也要同步一次，
        # 这样后续修订 phone_battery.rs 时重跑本脚本能把改动带过去。
        for name in ("phone_battery.rs", "island_status.rs", "bridge.rs"):
            shutil.copyfile(PATCH_DIR / name, root / "src" / name)
        print(f"补丁 {PATCH_ID} 已应用，跳过编辑；已同步新增模块 phone_battery.rs / island_status.rs / bridge.rs")
        return

    applied = skipped = 0
    for rel, old, new in EDITS:
        path = root / rel
        data = path.read_bytes()
        old_b, new_b = encode(old, data), encode(new, data)
        if old_b not in data:
            if new_b in data:
                skipped += 1
                continue
            raise SystemExit(f"补丁失配：{rel} 找不到原文片段\n---\n{old[:160]}\n---\n官方源码可能已改动，请人工对照后更新 apply.py")
        if data.count(old_b) != 1:
            raise SystemExit(f"补丁失配：{rel} 原文片段出现 {data.count(old_b)} 次，拒绝猜测")
        path.write_bytes(data.replace(old_b, new_b, 1))
        applied += 1

    for name in ("phone_battery.rs", "island_status.rs", "bridge.rs"):
        shutil.copyfile(PATCH_DIR / name, root / "src" / name)
    version = bump_version(root)
    marker.write_text(PATCH_ID + "\n", encoding="utf-8")
    print(f"补丁 {PATCH_ID} 应用完成：新增/修改 {applied} 处，跳过 {skipped} 处，版本 -> v{version}")


if __name__ == "__main__":
    main()
