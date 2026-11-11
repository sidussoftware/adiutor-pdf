//! Main window: toolbar, thumbnail grid and bottom bar.

use crate::engine::{Command, Engine, Event};
use crate::i18n::Tr;
use crate::ranges::{self, RangeError};
use crate::thumb_cache::ThumbCache;
use crate::workspace::{PageRef, SourceDoc, Workspace};
use eframe::egui::{self, Color32, Sense, TextureHandle, TextureOptions, vec2};
use std::path::PathBuf;

const THUMB_CACHE_CAP: usize = 300;
/// Thumbnail width in points; the engine renders it at `THUMB_WIDTH * pixels_per_point`.
const THUMB_WIDTH: f32 = 150.0;
const LABEL_HEIGHT: f32 = 20.0;
const GAP: f32 = 12.0;

type GridAction = Box<dyn FnOnce(&mut Workspace)>;

pub struct AdiutorApp {
    /// Err = why the engine could not start; shown instead of the grid.
    engine: Result<Engine, String>,
    workspace: Workspace,
    tr: Tr,
    cache: ThumbCache<TextureHandle>,
    /// Last set sent with `Command::Visible`, to avoid resending an unchanged set.
    last_visible: Vec<PageRef>,
    /// One-line message for the user (open failures, internal error).
    message: Option<String>,
    /// Contents of the Pages field and the translated parse error shown under it.
    pages_text: String,
    pages_error: Option<String>,
}

fn range_error_text(tr: &Tr, e: &RangeError) -> String {
    match e {
        RangeError::InvalidFormat { item } => tr.tf("range.invalid_format", &[("item", item)]),
        RangeError::ZeroPage => tr.t("range.zero_page").to_string(),
        RangeError::OutOfRange { page, total } => tr.tf(
            "range.out_of_range",
            &[("page", &page.to_string()), ("total", &total.to_string())],
        ),
        RangeError::Reversed { start, end } => tr.tf(
            "range.reversed",
            &[("start", &start.to_string()), ("end", &end.to_string())],
        ),
    }
}

impl AdiutorApp {
    pub fn new(ctx: &egui::Context, tr: Tr, startup_paths: Vec<PathBuf>) -> Self {
        let wake_ctx = ctx.clone();
        let engine = Engine::start(move || wake_ctx.request_repaint());
        let mut app = Self {
            engine,
            workspace: Workspace::new(),
            tr,
            cache: ThumbCache::new(THUMB_CACHE_CAP),
            last_visible: Vec::new(),
            message: None,
            pages_text: String::new(),
            pages_error: None,
        };
        for path in startup_paths {
            app.open(path);
        }
        app
    }

    fn send(&mut self, cmd: Command) {
        if let Ok(engine) = &self.engine
            && engine.send(cmd).is_err()
        {
            self.message = Some(self.tr.t("error.internal").to_string());
        }
    }

    fn open(&mut self, path: PathBuf) {
        self.send(Command::Open {
            path,
            password: None,
        });
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        loop {
            let event = match &self.engine {
                Ok(engine) => match engine.try_recv() {
                    Ok(Some(event)) => event,
                    Ok(None) => return,
                    Err(_) => {
                        self.message = Some(self.tr.t("error.internal").to_string());
                        return;
                    }
                },
                Err(_) => return,
            };
            match event {
                Event::Opened {
                    path,
                    doc,
                    page_sizes,
                } => self.workspace.add_document(SourceDoc {
                    id: doc,
                    path,
                    page_sizes,
                }),
                Event::Thumbnail {
                    page,
                    width,
                    height,
                    rgba,
                } => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        &rgba,
                    );
                    let name = format!("thumb-{}-{}", page.doc, page.index);
                    let tex = ctx.load_texture(name, image, TextureOptions::LINEAR);
                    self.cache.insert(page, tex);
                }
                Event::OpenFailed { path, reason } => {
                    let file = path
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    self.message = Some(
                        self.tr
                            .tf("open.failed", &[("file", &file), ("reason", &reason)]),
                    );
                }
                // Task 8 builds the password dialog.
                Event::PasswordRequired { path, .. } => {
                    eprintln!("password required for {}: not handled yet", path.display());
                }
                // Task 8 handles saving.
                Event::SaveProgress { .. } | Event::Saved { .. } | Event::SaveFailed { .. } => {}
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
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
                    Err(e) => self.pages_error = Some(range_error_text(&self.tr, &e)),
                }
            }
        });
        if let Some(err) = &self.pages_error {
            ui.colored_label(ui.visuals().error_fg_color, err);
        }
    }

    fn grid(&mut self, ui: &mut egui::Ui) {
        if self.workspace.is_empty() {
            ui.centered_and_justified(|ui| ui.label(self.tr.t("grid.empty_hint")));
            return;
        }
        let mut visible: Vec<PageRef> = Vec::new();
        let mut action: Option<GridAction> = None;
        let mut marker: Option<(f32, f32, f32)> = None; // x, top, bottom
        let dragging = egui::DragAndDrop::has_payload_of_type::<usize>(ui.ctx());
        egui::ScrollArea::vertical()
            .auto_shrink(false)
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
                        let mods = ui.input(|i| i.modifiers);
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
                        if dragging && resp.contains_pointer() {
                            let after = ui
                                .input(|i| i.pointer.interact_pos())
                                .is_some_and(|p| p.x >= rect.center().x);
                            let target = pos + usize::from(after);
                            let x = if after { rect.right() } else { rect.left() };
                            marker = Some((
                                x + if after { GAP / 2.0 } else { -GAP / 2.0 },
                                rect.top(),
                                rect.bottom(),
                            ));
                            if resp.dnd_release_payload::<usize>().is_some() {
                                action = Some(Box::new(move |w| w.move_selected_to(target)));
                            }
                        }
                        if !ui.is_rect_visible(rect) {
                            continue;
                        }
                        let img_rect =
                            egui::Rect::from_min_size(rect.min, vec2(THUMB_WIDTH, thumb_h));
                        match self.cache.get(&slot.page) {
                            Some(tex) => {
                                ui.painter().image(
                                    tex.id(),
                                    img_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    Color32::WHITE,
                                );
                            }
                            None => {
                                ui.painter()
                                    .rect_filled(img_rect, 2.0, Color32::from_gray(200));
                                visible.push(slot.page);
                            }
                        }
                        ui.painter().text(
                            egui::pos2(rect.center().x, img_rect.max.y + 2.0),
                            egui::Align2::CENTER_TOP,
                            (pos + 1).to_string(),
                            egui::FontId::proportional(13.0),
                            ui.visuals().text_color(),
                        );
                        if self.workspace.is_selected(pos) {
                            // Thick frame plus a check badge, so selection is not color-only.
                            let accent = ui.visuals().selection.bg_fill;
                            ui.painter().rect_stroke(
                                rect.expand(3.0),
                                3.0,
                                egui::Stroke::new(3.0, accent),
                                egui::StrokeKind::Inside,
                            );
                            let c = img_rect.left_top() + vec2(14.0, 14.0);
                            ui.painter().circle_filled(c, 10.0, accent);
                            ui.painter().line(
                                vec![
                                    c + vec2(-4.5, 0.0),
                                    c + vec2(-1.0, 3.5),
                                    c + vec2(5.0, -3.5),
                                ],
                                egui::Stroke::new(2.0, Color32::WHITE),
                            );
                        }
                    }
                });
                if let Some((x, top, bottom)) = marker {
                    ui.painter().line_segment(
                        [egui::pos2(x, top), egui::pos2(x, bottom)],
                        egui::Stroke::new(3.0, ui.visuals().selection.bg_fill),
                    );
                }
            });
        if let Some(f) = action {
            f(&mut self.workspace);
        }
        if visible != self.last_visible {
            let width_px = (THUMB_WIDTH * ui.ctx().pixels_per_point()).round() as u32;
            self.last_visible = visible.clone();
            self.send(Command::Visible {
                pages: visible,
                width_px,
            });
        }
    }

    fn bottom_bar(&self, ui: &mut egui::Ui) {
        if let Some(message) = &self.message {
            ui.label(message);
        }
    }
}

impl eframe::App for AdiutorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_events(&ctx);
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        for path in dropped {
            self.open(path);
        }

        let delete = !ctx.egui_wants_keyboard_input()
            && ctx
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete {
            self.workspace.delete_selected();
        }

        if let Err(reason) = &self.engine {
            let text = self.tr.tf("error.engine_start", &[("reason", reason)]);
            egui::CentralPanel::default().show(ui, |ui| ui.label(text));
            return;
        }
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("bottom_bar").show(ui, |ui| self.bottom_bar(ui));
        egui::CentralPanel::default().show(ui, |ui| self.grid(ui));
    }
}
