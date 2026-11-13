//! Top bar: page selection on the left, save and split on the right.

use super::*;
use style::palette;

type IconFn = fn(&egui::Painter, egui::Rect, egui::Color32);

impl AdiutorApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let pal = palette(ui);
        let frame = egui::Frame::new()
            .fill(pal.topbar)
            .inner_margin(egui::Margin::symmetric(24, 14));
        egui::Panel::top("toolbar")
            .exact_size(68.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| self.toolbar_contents(ui));
    }

    fn toolbar_contents(&mut self, ui: &mut egui::Ui) {
        let pal = palette(ui);
        ui.horizontal(|ui| {
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
                    ui.add(egui::TextEdit::singleline(&mut self.pages_text).desired_width(140.0))
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
            let btn = |ui: &mut egui::Ui, on: bool, icon: IconFn, text: &str| {
                icons::button(ui, enabled(on), icon, text).clicked()
            };
            // Right to left: the primary action is added first, so it sits at the far right.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let primary = ui
                    .scope(|ui| {
                        let v = ui.visuals_mut();
                        v.override_text_color = Some(pal.on_accent);
                        for w in [
                            &mut v.widgets.inactive,
                            &mut v.widgets.hovered,
                            &mut v.widgets.active,
                        ] {
                            w.weak_bg_fill = pal.accent;
                            w.fg_stroke.color = pal.on_accent;
                        }
                        btn(ui, any, icons::save, self.tr.t("action.save_all"))
                    })
                    .inner;
                if primary {
                    self.save(false);
                }
                if btn(ui, sel, icons::save, self.tr.t("action.save_selection")) {
                    self.save(true);
                }
                if btn(ui, any, icons::split, self.tr.t("action.split")) {
                    self.split();
                }
            });
        });
    }
}
