//! 插件开发者工具中的模板、校验与指南。

use super::{market, option_card, settings_control_row, settings_hint};
use eframe::egui;
use std::path::PathBuf;

pub(super) const GUIDE: &str =
    "https://github.com/Renly1994/xiaotang-daozai-plugins/blob/main/docs/author-submission.md";

#[derive(Clone, Default)]
pub(super) struct AuthorTools {
    draft: Option<TemplateDraft>,
    notice: Option<Notice>,
}

#[derive(Clone, Default)]
struct TemplateDraft {
    id: String,
    name: String,
    author: String,
}

#[derive(Clone)]
enum Notice {
    Created(PathBuf),
    Valid(String),
    Error(String),
}

pub(super) enum Action {
    Create,
    Validate,
}

fn template_id_error(id: &str) -> Option<&'static str> {
    if id.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Some("请将插件 ID 中的大写字母改成小写。")
    } else {
        (!super::super::valid_id(id)).then_some("请按上方格式填写插件 ID。")
    }
}

impl AuthorTools {
    /// 直接进入「从模板创建」表单，教程里的按钮用。
    pub(super) fn template() -> Self {
        Self {
            draft: Some(TemplateDraft::default()),
            notice: None,
        }
    }

    /// 刚创建好的插件文件夹。
    pub(super) fn created_dir(&self) -> Option<&std::path::Path> {
        match self.notice.as_ref() {
            Some(Notice::Created(manifest)) => manifest.parent(),
            _ => None,
        }
    }

    pub(super) fn creating(&self) -> bool {
        self.draft.is_some()
    }

    pub(super) fn menu(
        &mut self,
        ui: &mut egui::Ui,
        watching: &mut bool,
        logs: &[String],
    ) -> Option<Action> {
        if option_card(
            ui,
            "从模板创建",
            "生成可编辑的发薪倒计时插件、封面和许可证",
            false,
        ) {
            self.draft = Some(TemplateDraft::default());
            self.notice = None;
            ui.ctx().request_repaint();
        }
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("本地校验")
                .on_hover_text("选择 plugin.json，检查清单、入口和封面并试跑卡片")
                .clicked()
            {
                action = Some(Action::Validate);
            }
            if ui.link("投稿指南").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(GUIDE));
            }
        });
        ui.checkbox(watching, "监视文件变化");
        ui.collapsing("脚本日志", |ui| {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
            if logs.is_empty() {
                settings_hint(ui, "本地插件运行后，日志显示在这里。");
            }
            egui::ScrollArea::vertical()
                .max_height(145.0)
                .show(ui, |ui| {
                    for line in logs {
                        ui.label(line);
                    }
                });
        });
        self.show_notice(ui);
        action
    }

    pub(super) fn form(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let draft = self.draft.as_mut()?;
        let input = |ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str, limit| {
            settings_control_row(ui, label, 90.0, |ui| {
                ui.add_sized(
                    [ui.available_width(), 26.0],
                    egui::TextEdit::singleline(value)
                        .hint_text(hint)
                        .char_limit(limit),
                )
            });
        };
        input(ui, "插件 ID", &mut draft.id, "例如 xiaoming.payday", 64);
        settings_hint(
            ui,
            "用英文句点分隔，每段以小写英文字母开头，只能用小写字母、数字和连字符，最多 64 个字符。",
        );
        let id = draft.id.trim();
        let id_error = template_id_error(id);
        if let Some(message) = id_error.filter(|_| !id.is_empty()) {
            ui.colored_label(ui.visuals().error_fg_color, message);
        }
        input(ui, "插件名称", &mut draft.name, "例如 我的发薪倒计时", 80);
        input(ui, "作者名称", &mut draft.author, "你的名字或昵称", 80);
        let ready =
            id_error.is_none() && !draft.name.trim().is_empty() && !draft.author.trim().is_empty();
        self.show_notice(ui);
        let (back, create) = ui
            .horizontal_wrapped(|ui| {
                let back = ui.button("返回").clicked();
                let create = ui
                    .add_enabled(ready, egui::Button::new("选择位置并创建…"))
                    .clicked();
                (back, create)
            })
            .inner;
        if back {
            self.draft = None;
            self.notice = None;
            ui.ctx().request_repaint();
        }
        create.then_some(Action::Create)
    }

    pub(super) fn run(&mut self, action: Action) {
        match action {
            Action::Create => {
                let Some(draft) = self.draft.as_ref() else {
                    return;
                };
                if let Some(message) = template_id_error(draft.id.trim()) {
                    self.notice = Some(Notice::Error(message.into()));
                    return;
                }
                match crate::system::pick_folder_for("选择插件模板的保存位置") {
                    Ok(Some(parent)) => match market::create_template(
                        &parent,
                        draft.id.trim(),
                        draft.name.trim(),
                        draft.author.trim(),
                    ) {
                        Ok(manifest) => {
                            self.draft = None;
                            self.notice = Some(Notice::Created(manifest));
                        }
                        Err(error) => self.notice = Some(Notice::Error(error.to_string())),
                    },
                    Ok(None) => {}
                    Err(error) => self.notice = Some(Notice::Error(error.to_string())),
                }
            }
            Action::Validate => {
                match crate::system::pick_files_for("选择要校验的 plugin.json", false) {
                    Ok(files) => {
                        if let Some(path) = files.first() {
                            self.notice = Some(match market::validate_project(path) {
                                Ok(message) => Notice::Valid(message),
                                Err(error) => Notice::Error(format!("校验失败：{error}")),
                            });
                        }
                    }
                    Err(error) => self.notice = Some(Notice::Error(error.to_string())),
                }
            }
        }
    }

    fn show_notice(&mut self, ui: &mut egui::Ui) {
        match self.notice.as_ref() {
            Some(Notice::Created(manifest)) => {
                ui.label("已创建插件模板");
                settings_hint(ui, &manifest.display().to_string());
                if let Some(directory) = manifest.parent() {
                    if ui.button("打开目录").clicked() {
                        let mut explorer = std::process::Command::new("explorer.exe");
                        explorer.arg(directory);
                        if let Err(error) = crate::child_process::spawn(&mut explorer) {
                            self.notice = Some(Notice::Error(format!("无法打开目录：{error}")));
                        }
                    }
                }
            }
            Some(Notice::Valid(message)) => {
                ui.label(message);
            }
            Some(Notice::Error(message)) => {
                ui.colored_label(ui.visuals().error_fg_color, message);
            }
            None => {}
        }
    }
}
