//! Top bar: page selection on the left, save and split on the right.

use super::*;
use style::palette;

type IconFn = fn(&egui::Painter, egui::Rect, egui::Color32);

/// The icon-only Undo and Redo buttons, in right-to-left order; returns (undo, redo) clicked.
fn undo_redo_buttons(ui: &mut egui::Ui, tr: &Tr, can_undo: bool, can_redo: bool) -> (bool, bool) {
    let name = |key: &str, action: &str| tr.tf(key, &[("shortcut", shortcut(action))]);
    let redo = ui
        .add_enabled_ui(can_redo, |ui| {
            icons::icon_button(ui, icons::redo, &name("action.redo", "redo")).clicked()
        })
        .inner;
    let undo = ui
        .add_enabled_ui(can_undo, |ui| {
            icons::icon_button(ui, icons::undo, &name("action.undo", "undo")).clicked()
        })
        .inner;
    (undo, redo)
}

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
            let label = ui.label(self.tr.t("toolbar.pages"));
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
                    ui.add(
                        egui::TextEdit::singleline(&mut self.pages_text)
                            .desired_width(140.0)
                            .align(egui::Align2::LEFT_CENTER),
                    )
                })
                .inner
                .labelled_by(label.id);
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
            let btn = |ui: &mut egui::Ui, on: bool, icon: IconFn, text: egui::RichText| {
                icons::button(ui, enabled(on), icon, text).clicked()
            };
            // Right to left: the primary action is added first, so it sits at the far right.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let primary = ui
                    .scope(|ui| {
                        style::primary(ui, &pal);
                        btn(
                            ui,
                            any,
                            icons::save,
                            style::primary_text(self.tr.t("action.save_all")),
                        )
                    })
                    .inner;
                if primary {
                    self.save(false);
                }
                if btn(
                    ui,
                    sel,
                    icons::save,
                    egui::RichText::new(self.tr.t("action.save_selection")),
                ) {
                    self.save(true);
                }
                if btn(
                    ui,
                    any,
                    icons::split,
                    egui::RichText::new(self.tr.t("action.split")),
                ) {
                    self.split();
                }
                let (undo, redo) = undo_redo_buttons(
                    ui,
                    &self.tr,
                    self.workspace.can_undo(),
                    self.workspace.can_redo(),
                );
                if undo {
                    self.workspace.undo();
                }
                if redo {
                    self.workspace.redo();
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};

    #[test]
    fn undo_redo_disabled_without_history() {
        let tr = Tr::new(Lang::It);
        let mut h = Harness::builder()
            .with_context_setup(crate::install_fonts)
            .build_ui(|ui| {
                undo_redo_buttons(ui, &tr, false, true);
            });
        h.run();
        let undo = format!("Annulla ({})", shortcut("undo"));
        let redo = format!("Ripristina ({})", shortcut("redo"));
        assert!(h.get_by_label(&undo).accesskit_node().is_disabled());
        assert!(!h.get_by_label(&redo).accesskit_node().is_disabled());
    }
}
