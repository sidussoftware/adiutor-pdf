//! Main window: toolbar, thumbnail grid and bottom bar.

use crate::engine::{Command, Engine, Event};
use crate::i18n::Tr;
use crate::thumb_cache::ThumbCache;
use crate::workspace::{PageRef, SourceDoc, Workspace};
use eframe::egui::{self, Color32, Sense, TextureHandle, TextureOptions, vec2};
use std::path::PathBuf;

const THUMB_CACHE_CAP: usize = 300;
/// Thumbnail width in points; the engine renders it at `THUMB_WIDTH * pixels_per_point`.
const THUMB_WIDTH: f32 = 150.0;
const LABEL_HEIGHT: f32 = 20.0;
const GAP: f32 = 12.0;

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
        });
    }

    fn grid(&mut self, ui: &mut egui::Ui) {
        if self.workspace.is_empty() {
            ui.centered_and_justified(|ui| ui.label(self.tr.t("grid.empty_hint")));
            return;
        }
        let mut visible: Vec<PageRef> = Vec::new();
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
                        let (rect, _) = ui.allocate_exact_size(
                            vec2(THUMB_WIDTH, thumb_h + LABEL_HEIGHT),
                            Sense::hover(),
                        );
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
                    }
                });
            });
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
