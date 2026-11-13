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
                        close = stack(ui, &self.tr, &pal, progress, &self.toasts);
                    });
            });
        if let Some(id) = close {
            self.toasts.close(id);
        }
    }
}

/// The progress card and the toasts, newest first; returns the id of the toast whose close button was clicked.
fn stack(
    ui: &mut egui::Ui,
    tr: &Tr,
    pal: &Palette,
    progress: Option<(usize, usize)>,
    toasts: &crate::toasts::Toasts,
) -> Option<u64> {
    let mut close = None;
    if let Some((done, total)) = progress {
        card(ui, pal.card, pal.border, |ui| {
            let text = tr.tf(
                "progress.saving",
                &[("done", &done.to_string()), ("total", &total.to_string())],
            );
            ui.colored_label(pal.text, text);
            let frac = done as f32 / total.max(1) as f32;
            ui.add(egui::ProgressBar::new(frac).fill(pal.accent));
        });
    }
    for t in toasts.items() {
        let (bg, fg) = match t.kind {
            ToastKind::Success => (pal.success_bg, pal.success_text),
            ToastKind::Error => (pal.error_bg, pal.error_text),
        };
        card(ui, bg, fg, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().override_text_color = Some(fg);
                let w = ui.available_width() - 40.0 - ui.spacing().item_spacing.x;
                // Not add_sized: its justified layout stretches the letters of wrapped text.
                let top_down = egui::Layout::top_down(egui::Align::Min);
                ui.allocate_ui_with_layout(egui::vec2(w, 0.0), top_down, |ui| {
                    ui.add(egui::Label::new(message_text(tr, &t.msg)).wrap());
                });
                if icons::icon_button(ui, icons::close, tr.t("common.close")).clicked() {
                    close = Some(t.id);
                }
            });
        });
    }
    close
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use egui_kittest::{Harness, kittest::Queryable};
    use std::time::Instant;

    #[test]
    fn messages_are_translated_when_drawn() {
        let m: Message = ("save.done", vec![("file", "a.pdf".into())]);
        assert_eq!(message_text(&Tr::new(Lang::En), &m), "Saved a.pdf");
        assert_eq!(message_text(&Tr::new(Lang::It), &m), "Salvato a.pdf");
    }

    #[test]
    fn many_toasts_render_and_close_works() {
        let mut toasts = crate::toasts::Toasts::default();
        let now = Instant::now();
        for _ in 0..30 {
            let m: Message = ("save.done", vec![("file", "a.pdf".into())]);
            toasts.push(ToastKind::Error, m, now);
        }
        let tr = Tr::new(Lang::En);
        let pal = Palette::light();
        let mut closed = None;
        let mut h = Harness::builder()
            .with_size(egui::vec2(400.0, 600.0))
            .with_context_setup(crate::install_fonts)
            .build_ui(|ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    closed = closed.or(stack(ui, &tr, &pal, Some((3, 10)), &toasts));
                });
            });
        h.run();
        h.get_by_label("Saving 3 / 10");
        h.get_all_by_label("Close message").next().unwrap().click();
        h.run();
        drop(h);
        assert!(closed.is_some());
    }
}
