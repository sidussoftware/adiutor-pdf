//! Thumbnail grid.

use super::*;
use crate::palette::contrast;
use crate::workspace::DocSummary;
use eframe::egui::{Color32, Sense, vec2};
use style::{palette, semibold, semibold_family};

/// Thumbnail width in points; the engine renders it at `THUMB_WIDTH * pixels_per_point`.
const THUMB_WIDTH: f32 = 150.0;
const LABEL_HEIGHT: f32 = 28.0;
/// Room for a selected cell's frame (6 pt) and badge (15 pt) without touching a neighbour.
const GAP: f32 = 24.0;

/// Height of the document colour strip along the top of a page.
const STRIP_HEIGHT: f32 = 4.0;

/// White or black, whichever reads better on a document colour.
fn tag_text_color(bg: Color32) -> Color32 {
    if contrast(Color32::WHITE, bg) >= contrast(Color32::BLACK, bg) {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

/// The dashed drop panel shown with no documents; true when its Add PDF button was clicked.
fn empty_state(ui: &mut egui::Ui, tr: &Tr) -> bool {
    let pal = palette(ui);
    let rect = ui.available_rect_before_wrap().shrink(40.0);
    ui.painter().rect_filled(rect, 16.0, pal.card);
    style::dashed_rect(
        ui,
        rect,
        16.0,
        egui::Stroke::new(2.0, pal.border_strong),
        8.0,
        6.0,
    );
    let mut clicked = false;
    // Content is about 250 pt tall; centre it vertically.
    let top = ((rect.height() - 250.0) / 2.0).max(0.0);
    let inner = rect.shrink2(vec2(16.0, 0.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.y = 16.0;
            ui.add_space(top);
            let (icon, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
            icons::file_add(ui.painter(), icon, pal.text_secondary);
            ui.label(semibold(tr.t("empty.title"), 20.0).color(pal.text));
            ui.label(egui::RichText::new(tr.t("empty.or")).color(pal.text_secondary));
            ui.scope(|ui| {
                style::primary(ui, &pal);
                clicked = icons::button(
                    ui,
                    true,
                    icons::plus,
                    style::primary_text(tr.t("toolbar.add_pdf")),
                )
                .clicked();
            });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(tr.t("empty.hint"))
                    .size(13.0)
                    .color(pal.text_secondary),
            );
        },
    );
    clicked
}

type GridAction = Box<dyn FnOnce(&mut Workspace)>;

impl AdiutorApp {
    pub(super) fn grid(&mut self, ui: &mut egui::Ui, docs: &[DocSummary]) {
        if self.workspace.is_empty() {
            if empty_state(ui, &self.tr) {
                self.pick_pdfs();
            }
            return;
        }
        let ppp = ui.ctx().pixels_per_point();
        if ppp != self.pixels_per_point {
            // Re-render at the new scale so thumbnails stay sharp.
            self.pixels_per_point = ppp;
            self.cache.clear();
            self.last_visible.clear();
        }
        // Thumbnails drawn from here on are on screen; inserts until the next draw keep them.
        self.cache.begin_frame();
        let mut visible: Vec<PageRef> = Vec::new();
        let mut action: Option<GridAction> = None;
        // Cell rects by grid position, collected while dragging to find the drop target.
        let mut cells: Vec<egui::Rect> = Vec::new();
        let dragging = egui::DragAndDrop::has_payload_of_type::<usize>(ui.ctx());
        let mods = ui.input(|i| i.modifiers);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            // Room for the selection frame and badge at the edges; inside the clip rect.
            .content_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 16,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(GAP, GAP);
                ui.horizontal_wrapped(|ui| {
                    for (pos, slot) in self.workspace.slots().iter().enumerate() {
                        let (w, h) = self.workspace.page_size(slot);
                        let thumb_h = if w > 0.0 && h > 0.0 {
                            (THUMB_WIDTH * h / w).clamp(40.0, 400.0)
                        } else {
                            THUMB_WIDTH * 1.4
                        };
                        let (rect, resp) = ui.allocate_exact_size(
                            vec2(THUMB_WIDTH, thumb_h + LABEL_HEIGHT),
                            Sense::click_and_drag(),
                        );
                        resp.dnd_set_drag_payload(pos);
                        if resp.drag_started() && !self.workspace.is_selected(pos) {
                            action = Some(Box::new(move |w| w.select_only(pos)));
                        } else if resp.clicked() {
                            action = Some(Box::new(move |w| {
                                if mods.command {
                                    w.toggle(pos)
                                } else if mods.shift {
                                    w.select_range_to(pos)
                                } else {
                                    w.select_only(pos)
                                }
                            }));
                        }
                        if dragging {
                            cells.push(rect);
                        }
                        if !ui.is_rect_visible(rect) {
                            continue;
                        }
                        let img_rect =
                            egui::Rect::from_min_size(rect.min, vec2(THUMB_WIDTH, thumb_h));
                        // The page as drawn: letterboxed texture, or the placeholder.
                        let page_rect = match self.cache.get(&slot.page) {
                            Some(tex) => {
                                // Letterbox: the rendered bitmap can differ from the clamped cell.
                                let size = tex.size_vec2();
                                let scale =
                                    (img_rect.width() / size.x).min(img_rect.height() / size.y);
                                let r =
                                    egui::Rect::from_center_size(img_rect.center(), size * scale);
                                ui.painter().image(
                                    tex.id(),
                                    r,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    Color32::WHITE,
                                );
                                r
                            }
                            None => {
                                ui.painter()
                                    .rect_filled(img_rect, 2.0, Color32::from_gray(200));
                                visible.push(slot.page);
                                img_rect
                            }
                        };
                        let selected = self.workspace.is_selected(pos);
                        let pal = palette(ui);
                        let doc = docs.iter().find(|d| d.id == slot.page.doc);
                        // Thin border so white pages stand out; the selection frame is drawn outside it.
                        ui.painter().rect_stroke(
                            page_rect,
                            0.0,
                            egui::Stroke::new(1.0, pal.border_strong),
                            egui::StrokeKind::Inside,
                        );
                        if let Some(d) = doc {
                            let color = pal.doc_colors[d.id % pal.doc_colors.len()];
                            let strip = egui::Rect::from_min_size(
                                page_rect.min,
                                vec2(page_rect.width(), STRIP_HEIGHT),
                            );
                            ui.painter().rect_filled(strip, 0.0, color);
                            // The letter keeps the document identifiable without colour.
                            let letter = ui.painter().layout_no_wrap(
                                d.letter.clone(),
                                egui::FontId::new(12.0, semibold_family()),
                                tag_text_color(color),
                            );
                            let tag = egui::Rect::from_min_size(
                                page_rect.min,
                                vec2((letter.size().x + 8.0).max(18.0), 18.0),
                            );
                            ui.painter().rect_filled(tag, 0.0, color);
                            ui.painter().galley(
                                tag.center() - letter.size() / 2.0,
                                letter,
                                Color32::PLACEHOLDER,
                            );
                        }
                        let accent = pal.accent;
                        let number = ui.painter().layout_no_wrap(
                            (pos + 1).to_string(),
                            egui::FontId::proportional(13.0),
                            if selected {
                                pal.on_accent
                            } else {
                                ui.visuals().text_color()
                            },
                        );
                        // Below the frame, which reaches 6 pt under the page.
                        let number_rect = egui::Align2::CENTER_TOP.anchor_size(
                            egui::pos2(rect.center().x, img_rect.max.y + 10.0),
                            number.size(),
                        );
                        if selected {
                            // Frame, pill and check badge, so selection is not color-only.
                            let frame = page_rect.expand(3.0);
                            ui.painter().rect_stroke(
                                frame,
                                6.0,
                                egui::Stroke::new(3.0, accent),
                                egui::StrokeKind::Outside,
                            );
                            let pill = number_rect.expand2(vec2(7.0, 1.0));
                            ui.painter().rect_filled(pill, pill.height() / 2.0, accent);
                            let c = frame.right_top();
                            // Stroke is centred on the radius: accent disc of 10, ring out to 12.
                            ui.painter().circle(
                                c,
                                11.0,
                                accent,
                                egui::Stroke::new(2.0, ui.visuals().panel_fill),
                            );
                            icons::check(
                                ui.painter(),
                                egui::Rect::from_center_size(c, vec2(12.0, 12.0)),
                                pal.on_accent,
                            );
                        }
                        ui.painter()
                            .galley(number_rect.min, number, Color32::PLACEHOLDER);
                        if let Some(d) = doc {
                            // Built only while hovered.
                            resp.on_hover_ui(|ui| {
                                let page = (slot.page.index + 1).to_string();
                                ui.label(self.tr.tf(
                                    "grid.page_tooltip",
                                    &[("file", &d.name), ("page", &page)],
                                ));
                            });
                        }
                    }
                });
                // Over a cell or a gap of the grid, the drop goes next to the nearest cell.
                let clip = ui.clip_rect();
                let pointer = ui.input(|i| i.pointer.interact_pos());
                let nearest = pointer
                    .filter(|_| ui.rect_contains_pointer(clip))
                    .and_then(|p| {
                        let dist = |r: &egui::Rect| r.distance_sq_to_pos(p);
                        let cell = cells
                            .iter()
                            .enumerate()
                            .min_by(|(_, a), (_, b)| dist(a).total_cmp(&dist(b)))?;
                        Some((p, cell))
                    });
                if let Some((p, (pos, rect))) = nearest {
                    let after = p.x >= rect.center().x;
                    let target = pos + usize::from(after);
                    let x = if after {
                        rect.right() + GAP / 2.0
                    } else {
                        rect.left() - GAP / 2.0
                    };
                    // Keep the 3 pt line inside the clip, e.g. before a row's first cell.
                    let x = x.clamp(clip.left() + 1.5, clip.right() - 1.5);
                    // Start below the badge, which reaches 15 pt above and right of a selected page.
                    ui.painter().line_segment(
                        [
                            egui::pos2(x, rect.top() + 16.0),
                            egui::pos2(x, rect.bottom()),
                        ],
                        egui::Stroke::new(3.0, palette(ui).accent),
                    );
                    if ui.input(|i| i.pointer.any_released())
                        && egui::DragAndDrop::take_payload::<usize>(ui.ctx()).is_some()
                    {
                        action = Some(Box::new(move |w| w.move_selected_to(target)));
                    }
                }
            });
        if let Some(f) = action {
            f(&mut self.workspace);
        }
        if visible != self.last_visible {
            let width_px = (THUMB_WIDTH * ppp).round() as u32;
            self.last_visible = visible.clone();
            self.send(Command::Visible {
                pages: visible,
                width_px,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Lang, Tr};
    use crate::palette::{Palette, contrast};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn tag_text_contrast_on_every_doc_color() {
        for p in [Palette::light(), Palette::dark()] {
            for c in p.doc_colors {
                let t = tag_text_color(c);
                assert!(contrast(t, c) >= 4.5, "{c:?}: {:.2}", contrast(t, c));
            }
        }
    }

    #[test]
    fn empty_state_shows_drop_panel() {
        let tr = Tr::new(Lang::It);
        let mut h = Harness::builder()
            .with_size(egui::vec2(900.0, 600.0))
            .with_context_setup(crate::install_fonts)
            .build_ui(move |ui| {
                empty_state(ui, &tr);
            });
        h.run();
        h.get_by_label("Trascina qui i PDF");
        h.get_by_label("oppure");
        h.get_by_label("Aggiungi PDF");
        h.get_by_label_contains("I file originali non vengono mai modificati");
    }
}
