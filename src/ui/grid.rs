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

/// Corner radius of a page thumbnail.
const PAGE_RADIUS: u8 = 4;

/// What a thumbnail needs from its document, prepared once per frame.
#[derive(Clone)]
struct DocMark<'a> {
    doc: &'a DocSummary,
    color: Color32,
    letter: std::sync::Arc<egui::Galley>,
}

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

/// Thumbnail height in points for a page of `w` x `h` points.
fn thumb_height((w, h): (f32, f32)) -> f32 {
    if w > 0.0 && h > 0.0 {
        (THUMB_WIDTH * h / w).clamp(40.0, 400.0)
    } else {
        THUMB_WIDTH * 1.4
    }
}

/// Where the grid's cells go, computed without widgets so only visible rows need any.
struct GridLayout {
    cols: usize,
    /// (top, height) of each row, relative to the grid's top-left corner.
    rows: Vec<(f32, f32)>,
}

impl GridLayout {
    /// Same wrapping as a horizontal-wrapped row of `THUMB_WIDTH` cells with `GAP` spacing; each row
    /// is as tall as its tallest cell plus the label.
    fn new(heights: &[f32], width: f32) -> Self {
        let cols = (((width + GAP) / (THUMB_WIDTH + GAP)).floor() as usize).max(1);
        let mut top = 0.0;
        let rows = heights
            .chunks(cols)
            .map(|row| {
                let h = row.iter().copied().fold(0.0, f32::max) + LABEL_HEIGHT;
                let r = (top, h);
                top += h + GAP;
                r
            })
            .collect();
        GridLayout { cols, rows }
    }

    fn height(&self) -> f32 {
        self.rows.last().map_or(0.0, |(top, h)| top + h)
    }

    /// The cell of grid position `pos`, whose page is `thumb_h` tall, centred vertically in its row.
    fn cell(&self, origin: egui::Pos2, pos: usize, thumb_h: f32) -> egui::Rect {
        let (top, row_h) = self.rows[pos / self.cols];
        let size = vec2(THUMB_WIDTH, thumb_h + LABEL_HEIGHT);
        let x = (pos % self.cols) as f32 * (THUMB_WIDTH + GAP);
        egui::Rect::from_min_size(origin + vec2(x, top + (row_h - size.y) / 2.0), size)
    }
}

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
        let pal = palette(ui);
        // By DocId: each document's summary, colour and laid-out letter, built once per frame.
        let mut by_id: Vec<Option<DocMark>> = Vec::new();
        for d in docs {
            if by_id.len() <= d.id {
                by_id.resize(d.id + 1, None);
            }
            let color = pal.doc_colors[d.id % pal.doc_colors.len()];
            let letter = ui.painter().layout_no_wrap(
                d.letter.clone(),
                egui::FontId::new(12.0, semibold_family()),
                tag_text_color(color),
            );
            by_id[d.id] = Some(DocMark {
                doc: d,
                color,
                letter,
            });
        }
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
                let slots = self.workspace.slots();
                let heights: Vec<f32> = slots
                    .iter()
                    .map(|s| thumb_height(self.workspace.page_size(s)))
                    .collect();
                let layout = GridLayout::new(&heights, ui.available_width());
                let origin = ui.cursor().min;
                // Reserve the whole grid for scrolling; cells get widgets only when on screen.
                ui.allocate_rect(
                    egui::Rect::from_min_size(origin, vec2(ui.available_width(), layout.height())),
                    Sense::hover(),
                );
                let clip = ui.clip_rect();
                for (pos, slot) in slots.iter().enumerate() {
                    let thumb_h = heights[pos];
                    let rect = layout.cell(origin, pos, thumb_h);
                    if dragging {
                        cells.push(rect);
                    }
                    // Rows above or below the view: no widget, no drawing.
                    if !clip.intersects(rect.expand(GAP)) {
                        continue;
                    }
                    let resp = ui.interact(
                        rect,
                        egui::Id::NULL.with(("grid-cell", slot.id)),
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
                    let img_rect = egui::Rect::from_min_size(rect.min, vec2(THUMB_WIDTH, thumb_h));
                    // The page as drawn: letterboxed texture, or the placeholder.
                    let page_rect = match self.cache.get(&slot.page) {
                        Some(tex) => {
                            // Letterbox: the rendered bitmap can differ from the clamped cell.
                            let size = tex.size_vec2();
                            let scale = (img_rect.width() / size.x).min(img_rect.height() / size.y);
                            let r = egui::Rect::from_center_size(img_rect.center(), size * scale);
                            egui::Image::new(egui::load::SizedTexture::from_handle(tex))
                                .corner_radius(PAGE_RADIUS)
                                .paint_at(ui, r);
                            r
                        }
                        None => {
                            ui.painter().rect_filled(
                                img_rect,
                                PAGE_RADIUS,
                                Color32::from_gray(200),
                            );
                            visible.push(slot.page);
                            img_rect
                        }
                    };
                    let selected = self.workspace.is_selected(pos);
                    let mark = by_id.get(slot.page.doc).and_then(Option::as_ref);
                    // Thin border so white pages stand out; the selection frame is drawn outside it.
                    ui.painter().rect_stroke(
                        page_rect,
                        PAGE_RADIUS,
                        egui::Stroke::new(1.0, pal.border_strong),
                        egui::StrokeKind::Inside,
                    );
                    if let Some(m) = mark {
                        let top = egui::CornerRadius {
                            nw: PAGE_RADIUS,
                            ne: PAGE_RADIUS,
                            sw: 0,
                            se: 0,
                        };
                        let strip = egui::Rect::from_min_size(
                            page_rect.min,
                            vec2(page_rect.width(), STRIP_HEIGHT),
                        );
                        ui.painter().rect_filled(strip, top, m.color);
                        // The letter keeps the document identifiable without colour.
                        let size = m.letter.size();
                        let tag = egui::Rect::from_min_size(
                            page_rect.min,
                            vec2((size.x + 8.0).max(18.0), 18.0),
                        );
                        let corner = egui::CornerRadius { ne: 0, ..top };
                        ui.painter().rect_filled(tag, corner, m.color);
                        ui.painter().galley(
                            tag.center() - size / 2.0,
                            m.letter.clone(),
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
                            PAGE_RADIUS + 3,
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
                    if let Some(DocMark { doc: d, .. }) = mark {
                        // Built only while hovered.
                        resp.on_hover_ui(|ui| {
                            let page = (slot.page.index + 1).to_string();
                            ui.label(
                                self.tr
                                    .tf("grid.page_tooltip", &[("file", &d.name), ("page", &page)]),
                            );
                        });
                    }
                }
                // Over a cell or a gap of the grid, the drop goes next to the nearest cell.
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
                        egui::Stroke::new(3.0, pal.accent),
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

    #[test]
    fn layout_wraps_like_the_old_grid() {
        // Exactly two cells fit in 2 * 150 + 24; one point less fits one.
        assert_eq!(GridLayout::new(&[100.0; 4], 324.0).cols, 2);
        assert_eq!(GridLayout::new(&[100.0; 4], 323.0).cols, 1);
        // Never zero columns, even in a very narrow window.
        assert_eq!(GridLayout::new(&[100.0], 10.0).cols, 1);
    }

    #[test]
    fn layout_rows_take_the_tallest_cell_and_centre_the_others() {
        let l = GridLayout::new(&[100.0, 200.0, 50.0], 324.0);
        let row0 = 200.0 + LABEL_HEIGHT;
        assert_eq!(l.rows, vec![(0.0, row0), (row0 + GAP, 50.0 + LABEL_HEIGHT)]);
        assert_eq!(l.height(), row0 + GAP + 50.0 + LABEL_HEIGHT);
        let o = egui::pos2(10.0, 20.0);
        // Second cell: next column, top of its row.
        assert_eq!(
            l.cell(o, 1, 200.0).min,
            egui::pos2(10.0 + THUMB_WIDTH + GAP, 20.0)
        );
        // First cell is 100 pt shorter than its row: centred, 50 pt down.
        assert_eq!(l.cell(o, 0, 100.0).min, egui::pos2(10.0, 70.0));
        // Third cell starts the second row.
        assert_eq!(l.cell(o, 2, 50.0).min, egui::pos2(10.0, 20.0 + row0 + GAP));
    }

    #[test]
    fn empty_layout_has_no_height() {
        assert_eq!(GridLayout::new(&[], 800.0).height(), 0.0);
    }
}
