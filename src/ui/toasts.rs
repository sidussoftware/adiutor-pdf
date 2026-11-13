//! Toast stack and progress card, top-right of the central area.

use super::*;
use crate::toasts::{Message, ToastKind};

const WIDTH: f32 = 360.0;

pub(super) fn message_text(tr: &Tr, (key, args): &Message) -> String {
    let args: Vec<(&str, &str)> = args.iter().map(|(n, v)| (*n, v.as_str())).collect();
    tr.tf(key, &args)
}

impl AdiutorApp {
    /// Draws the stack inside the central panel; `ui` is that panel's ui.
    pub(super) fn toasts_ui(&mut self, ui: &mut egui::Ui) {
        let progress = self.batch.as_ref().map(|b| b.total_progress());
        if progress.is_none() && self.toasts.items().next().is_none() {
            return;
        }
        let pal = Palette::for_dark_mode(ui.visuals().dark_mode);
        let area = ui.max_rect();
        let width = WIDTH.min(area.width() - 32.0).max(0.0);
        let mut close = None;
        egui::Area::new(egui::Id::NULL.with("toasts"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(area.right_top() + egui::vec2(-16.0, 16.0))
            .show(ui.ctx(), |ui| {
                ui.set_width(width);
                egui::ScrollArea::vertical()
                    .max_height(area.height() / 2.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if let Some((done, total)) = progress {
                            card(ui, pal.card, pal.border, |ui| {
                                let text = self.tr.tf(
                                    "progress.saving",
                                    &[("done", &done.to_string()), ("total", &total.to_string())],
                                );
                                ui.colored_label(pal.text, text);
                                let frac = done as f32 / total.max(1) as f32;
                                ui.add(egui::ProgressBar::new(frac).fill(pal.accent));
                            });
                        }
                        for t in self.toasts.items() {
                            let (bg, fg) = match t.kind {
                                ToastKind::Success => (pal.success_bg, pal.success_text),
                                ToastKind::Error => (pal.error_bg, pal.error_text),
                            };
                            card(ui, bg, fg, |ui| {
                                ui.colored_label(fg, message_text(&self.tr, &t.msg));
                                ui.scope(|ui| {
                                    ui.visuals_mut().override_text_color = Some(fg);
                                    if icons::button(
                                        ui,
                                        true,
                                        icons::close,
                                        self.tr.t("common.close"),
                                    )
                                    .clicked()
                                    {
                                        close = Some(t.id);
                                    }
                                });
                            });
                        }
                    });
            });
        if let Some(id) = close {
            self.toasts.close(id);
        }
    }
}

fn card(
    ui: &mut egui::Ui,
    fill: egui::Color32,
    stroke: egui::Color32,
    add: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    ui.add_space(8.0);
}
