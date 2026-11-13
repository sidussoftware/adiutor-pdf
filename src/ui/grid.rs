//! Thumbnail grid.

use super::*;
use eframe::egui::{Color32, Sense, vec2};
use style::palette;

/// Thumbnail width in points; the engine renders it at `THUMB_WIDTH * pixels_per_point`.
const THUMB_WIDTH: f32 = 150.0;
const LABEL_HEIGHT: f32 = 28.0;
/// Room for a selected cell's frame (6 pt) and badge (15 pt) without touching a neighbour.
const GAP: f32 = 24.0;

type GridAction = Box<dyn FnOnce(&mut Workspace)>;

impl AdiutorApp {
    pub(super) fn grid(&mut self, ui: &mut egui::Ui) {
        if self.workspace.is_empty() {
            ui.centered_and_justified(|ui| ui.label(self.tr.t("grid.empty_hint")));
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
