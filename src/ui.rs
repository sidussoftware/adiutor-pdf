//! Main window: toolbar, thumbnail grid and bottom bar.

use crate::engine::{Command, Engine, Event};
use crate::i18n::{Lang, Tr};
use crate::ranges::{self, RangeError};
use crate::thumb_cache::ThumbCache;
use crate::workspace::{PageRef, SourceDoc, Workspace, free_split_names, split_stem};
use eframe::egui::{self, Color32, Sense, TextureHandle, TextureOptions, vec2};
use std::collections::{HashSet, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
    /// Dismissible messages at the bottom (failures, results).
    messages: Vec<String>,
    /// Contents of the Pages field and the parse error shown under it.
    pages_text: String,
    pages_error: Option<RangeError>,
    next_job: u64,
    /// The running save or split, if any.
    batch: Option<Batch>,
    /// Latest `SaveProgress` of the running batch.
    progress: Option<(usize, usize)>,
    /// Encrypted files waiting for a password (path, previous attempt was wrong).
    passwords: VecDeque<(PathBuf, bool)>,
    password_text: String,
    close_prompt: bool,
    /// The user confirmed closing; stops the cancel loop.
    closing: bool,
    /// The engine thread ended; its error was already shown.
    engine_dead: bool,
}

/// Jobs of one Save or Split that have not finished yet.
struct Batch {
    pending: HashSet<u64>,
    ok: usize,
    failed: bool,
    /// Some for a split: the destination folder.
    split_folder: Option<PathBuf>,
    /// The page list when the batch started; `mark_saved` only if it is unchanged at the end.
    pages: Vec<PageRef>,
    /// The jobs write every page of `pages`, in order (Save all, or a full split).
    covers_all: bool,
}

/// True if the jobs' pages, concatenated in job order, are exactly `all`.
fn covers_all(jobs: &[(Vec<PageRef>, PathBuf)], all: &[PageRef]) -> bool {
    jobs.iter().flat_map(|(pages, _)| pages).eq(all)
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

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
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
            messages: Vec::new(),
            pages_text: String::new(),
            pages_error: None,
            next_job: 0,
            batch: None,
            progress: None,
            passwords: VecDeque::new(),
            password_text: String::new(),
            close_prompt: false,
            closing: false,
            engine_dead: false,
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
            self.engine_died();
        }
    }

    /// Shows the internal error once, however often the dead engine is used.
    fn engine_died(&mut self) {
        if !std::mem::replace(&mut self.engine_dead, true) {
            self.error(self.tr.t("error.internal").to_string());
        }
    }

    fn error(&mut self, text: String) {
        eprintln!("{text}");
        self.messages.push(text);
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
                        self.engine_died();
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
                    let text = self.tr.tf(
                        "open.failed",
                        &[("file", &file_name(&path)), ("reason", &reason)],
                    );
                    self.error(text);
                }
                Event::PasswordRequired { path, wrong } => self.passwords.push_back((path, wrong)),
                Event::SaveProgress { done, total, .. } => self.progress = Some((done, total)),
                Event::Saved { job, dest } => {
                    if self
                        .batch
                        .as_ref()
                        .is_some_and(|b| b.split_folder.is_none())
                    {
                        let text = self.tr.tf("save.done", &[("file", &file_name(&dest))]);
                        self.messages.push(text);
                    }
                    self.finish_job(job, true);
                }
                Event::SaveFailed { job, dest, reason } => {
                    let text = self.tr.tf(
                        "save.failed",
                        &[("file", &file_name(&dest)), ("reason", &reason)],
                    );
                    self.error(text);
                    self.finish_job(job, false);
                }
            }
        }
    }

    fn finish_job(&mut self, job: u64, ok: bool) {
        let Some(b) = &mut self.batch else { return };
        if !b.pending.remove(&job) {
            return;
        }
        b.ok += usize::from(ok);
        b.failed |= !ok;
        if !b.pending.is_empty() {
            return;
        }
        let b = self.batch.take().expect("checked above");
        self.progress = None;
        if let Some(folder) = &b.split_folder
            && b.ok > 0
        {
            let text = self.tr.tf(
                "split.done",
                &[
                    ("count", &b.ok.to_string()),
                    ("folder", &folder.to_string_lossy()),
                ],
            );
            self.messages.push(text);
        }
        if !b.failed && b.covers_all && self.workspace.all_pages() == b.pages {
            self.workspace.mark_saved();
        }
    }

    /// Sends one `Save` per (pages, dest) and starts tracking them as a batch.
    fn start_batch(&mut self, jobs: Vec<(Vec<PageRef>, PathBuf)>, split_folder: Option<PathBuf>) {
        if jobs.iter().any(|(_, d)| self.workspace.is_source_path(d)) {
            let text = self.tr.t("save.blocked_original").to_string();
            self.messages.push(text);
            return;
        }
        let pages = self.workspace.all_pages();
        let covers_all = covers_all(&jobs, &pages);
        let mut pending = HashSet::new();
        for (pages, dest) in jobs {
            let job = self.next_job;
            self.next_job += 1;
            pending.insert(job);
            self.send(Command::Save { job, pages, dest });
        }
        self.progress = None;
        self.batch = Some(Batch {
            pending,
            ok: 0,
            failed: false,
            split_folder,
            pages,
            covers_all,
        });
    }

    fn save(&mut self, selection_only: bool) {
        let Some(stem) = split_stem(&self.workspace) else {
            return;
        };
        let Some(mut dest) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name(format!("{stem}_edited.pdf"))
            .save_file()
        else {
            return;
        };
        if !dest
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            let mut name = OsString::from(dest.file_name().unwrap_or_default());
            name.push(".pdf");
            dest.set_file_name(name);
            // The dialog checked the name without ".pdf", so ask before replacing this file.
            if dest.exists() && !self.workspace.is_source_path(&dest) {
                let answer = rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title(self.tr.t("save.overwrite_title"))
                    .set_description(
                        self.tr
                            .tf("save.overwrite_body", &[("file", &file_name(&dest))]),
                    )
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if answer != rfd::MessageDialogResult::Yes {
                    return;
                }
            }
        }
        let pages = if selection_only {
            self.workspace
                .pages_at(&self.workspace.selected_positions())
        } else {
            self.workspace.all_pages()
        };
        self.start_batch(vec![(pages, dest)], None);
    }

    fn split(&mut self) {
        let Some(stem) = split_stem(&self.workspace) else {
            return;
        };
        let mut groups = match ranges::parse(&self.pages_text, self.workspace.len()) {
            Ok(g) => g,
            Err(e) => {
                self.pages_error = Some(e);
                return;
            }
        };
        if groups.is_empty() {
            groups = (0..self.workspace.len()).map(|p| vec![p]).collect();
        }
        let Some(folder) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        // An unreadable folder aborts: with an incomplete list a split could overwrite files.
        let listing: Result<Vec<_>, _> =
            std::fs::read_dir(&folder).and_then(|rd| rd.collect::<Result<Vec<_>, _>>());
        let existing: HashSet<String> = match listing {
            Ok(entries) => entries
                .into_iter()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect(),
            Err(e) => {
                let text = self.tr.tf(
                    "save.failed",
                    &[
                        ("file", &folder.to_string_lossy()),
                        ("reason", &e.to_string()),
                    ],
                );
                self.error(text);
                return;
            }
        };
        let names = free_split_names(&stem, &groups, &existing, |n| folder.join(n).exists());
        let jobs = groups
            .iter()
            .zip(names)
            .map(|(g, name)| (self.workspace.pages_at(g), folder.join(name)))
            .collect();
        self.start_batch(jobs, Some(folder));
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
                    Err(e) => self.pages_error = Some(e),
                }
            }
            let idle = self.batch.is_none();
            let any = !self.workspace.is_empty();
            let sel = !self.workspace.selected_positions().is_empty();
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
                let mut lang = self.tr.lang();
                egui::ComboBox::from_id_salt("language")
                    .selected_text(lang.native_name())
                    .show_ui(ui, |ui| {
                        for l in Lang::ALL {
                            ui.selectable_value(&mut lang, l, l.native_name());
                        }
                    });
                ui.label(self.tr.t("toolbar.language"));
                if lang != self.tr.lang() {
                    self.tr = Tr::new(lang);
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

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        if self.batch.is_some() {
            let (done, total) = self.progress.unwrap_or((0, 1));
            let text = self.tr.tf(
                "save.progress",
                &[("done", &done.to_string()), ("total", &total.to_string())],
            );
            let frac = done as f32 / total.max(1) as f32;
            ui.add(egui::ProgressBar::new(frac).text(text));
        }
        let mut dismiss = None;
        for (i, message) in self.messages.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.button(self.tr.t("common.close")).clicked() {
                    dismiss = Some(i);
                }
                ui.label(message);
            });
        }
        if let Some(i) = dismiss {
            self.messages.remove(i);
        }
    }

    fn password_modal(&mut self, ctx: &egui::Context) {
        let Some((path, wrong)) = self.passwords.front().cloned() else {
            return;
        };
        let (mut ok, mut cancel) = (false, false);
        egui::Modal::new(egui::Id::NULL.with("password")).show(ctx, |ui| {
            ui.heading(self.tr.t("password.title"));
            ui.label(
                self.tr
                    .tf("password.prompt", &[("file", &file_name(&path))]),
            );
            if wrong {
                ui.colored_label(ui.visuals().error_fg_color, self.tr.t("password.wrong"));
            }
            let edit = ui.add(egui::TextEdit::singleline(&mut self.password_text).password(true));
            ok = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            edit.request_focus();
            ui.horizontal(|ui| {
                ok |= ui.button(self.tr.t("password.ok")).clicked();
                cancel = ui.button(self.tr.t("common.cancel")).clicked();
            });
        });
        if ok || cancel {
            self.passwords.pop_front();
            let password = std::mem::take(&mut self.password_text);
            if ok {
                self.send(Command::Open {
                    path,
                    password: Some(password),
                });
            }
        }
    }

    fn close_modal(&mut self, ctx: &egui::Context) {
        if !self.close_prompt {
            return;
        }
        egui::Modal::new(egui::Id::NULL.with("close")).show(ctx, |ui| {
            ui.heading(self.tr.t("close.unsaved_title"));
            ui.label(self.tr.t("close.unsaved_body"));
            ui.horizontal(|ui| {
                if ui.button(self.tr.t("close.confirm")).clicked() {
                    self.closing = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button(self.tr.t("common.cancel")).clicked() {
                    self.close_prompt = false;
                }
            });
        });
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

        // The default macOS menu is off (see lib.rs), so Cmd+Q arrives here as a key press.
        #[cfg(target_os = "macos")]
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if ctx.input(|i| i.viewport().close_requested())
            && !self.closing
            && (self.workspace.is_dirty() || self.batch.is_some())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_prompt = true;
        }

        let modal_open = self.close_prompt || !self.passwords.is_empty();
        let delete = !modal_open
            && !ctx.egui_wants_keyboard_input()
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
        self.password_modal(&ctx);
        self.close_modal(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(index: u32) -> PageRef {
        PageRef { doc: 0, index }
    }

    #[test]
    fn covers_all_only_for_every_page_in_order() {
        let all = [p(0), p(1), p(2)];
        let job = |pages: &[PageRef]| (pages.to_vec(), PathBuf::new());
        assert!(covers_all(&[job(&all)], &all));
        assert!(covers_all(&[job(&[p(0)]), job(&[p(1), p(2)])], &all));
        assert!(!covers_all(&[job(&[p(0), p(1)])], &all), "selection");
        assert!(!covers_all(&[job(&[p(0)]), job(&[p(2)])], &all), "gap");
        assert!(
            !covers_all(&[job(&[p(1), p(2)]), job(&[p(0)])], &all),
            "order"
        );
        assert!(!covers_all(&[job(&all), job(&[p(0)])], &all), "extra");
    }
}
