//! Toolbar.

use super::*;

impl AdiutorApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button(self.tr.t("toolbar.add_pdf")).clicked()
                && let Some(files) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .pick_files()
            {
                for path in files {
                    self.open(path);
                }
            }
            ui.label(self.tr.t("toolbar.pages"));
            let red = ui.visuals().error_fg_color;
            let stroke = if self.pages_error.is_some() {
                egui::Stroke::new(1.5, red)
            } else {
                egui::Stroke::NONE
            };
            let edit = egui::Frame::new()
                .stroke(stroke)
                .corner_radius(3.0)
                .show(ui, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.pages_text).desired_width(160.0))
                })
                .inner;
            if edit.changed() {
                self.pages_error = None;
            }
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button(self.tr.t("toolbar.select")).clicked() || enter {
                match ranges::parse(&self.pages_text, self.workspace.len()) {
                    Ok(groups) => {
                        self.pages_error = None;
                        self.workspace.set_selection(&groups.concat());
                    }
                    Err(e) => self.pages_error = Some(e),
                }
            }
            let idle = self.batch.is_none();
            let any = !self.workspace.is_empty();
            let sel = self.workspace.has_selection();
            let enabled = |on: bool| idle && on;
            let btn = |ui: &mut egui::Ui, on: bool, text: &str| {
                ui.add_enabled(enabled(on), egui::Button::new(text))
                    .clicked()
            };
            if btn(ui, any, self.tr.t("action.save_all")) {
                self.save(false);
            }
            if btn(ui, sel, self.tr.t("action.save_selection")) {
                self.save(true);
            }
            if btn(ui, any, self.tr.t("action.split")) {
                self.split();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut pref = self.settings.language;
                egui::ComboBox::from_id_salt("language")
                    .selected_text(self.lang_label(pref))
                    .show_ui(ui, |ui| {
                        for p in LanguagePref::ALL {
                            ui.selectable_value(&mut pref, p, self.lang_label(p));
                        }
                    });
                ui.label(self.tr.t("toolbar.language"));
                if pref != self.settings.language {
                    self.set_settings(Settings {
                        language: pref,
                        ..self.settings.clone()
                    });
                    self.tr = Tr::new(pref.resolve());
                    let title = self.tr.t("app.title").to_string();
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Title(title));
                }
            });
        });
        if let Some(e) = &self.pages_error {
            let text = range_error_text(&self.tr, e);
            ui.colored_label(ui.visuals().error_fg_color, text);
        }
    }
}
