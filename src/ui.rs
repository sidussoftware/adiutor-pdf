//! Main window: toolbar, thumbnail grid and bottom bar.

use crate::batch::Batch;
use crate::engine::{Command, Engine, Event};
use crate::i18n::{Lang, Tr};
use crate::ranges::{self, RangeError};
use crate::thumb_cache::ThumbCache;
use crate::workspace::{PageRef, SourceDoc, Workspace, free_split_names, split_stem};
use eframe::egui::{self, Color32, Sense, TextureHandle, TextureOptions, vec2};
use std::collections::{HashSet, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Bytes of thumbnail textures kept (RGBA, 4 bytes per pixel).
const THUMB_CACHE_BYTES: usize = 48 << 20;
/// Thumbnail width in points; the engine renders it at `THUMB_WIDTH * pixels_per_point`.
const THUMB_WIDTH: f32 = 150.0;
const LABEL_HEIGHT: f32 = 20.0;
const GAP: f32 = 12.0;

type GridAction = Box<dyn FnOnce(&mut Workspace)>;

/// An i18n key and its placeholder values, translated when drawn so a language switch applies.
type Message = (&'static str, Vec<(&'static str, String)>);

fn message_text(tr: &Tr, (key, args): &Message) -> String {
    let args: Vec<(&str, &str)> = args.iter().map(|(n, v)| (*n, v.as_str())).collect();
    tr.tf(key, &args)
}

pub struct AdiutorApp {
    /// Err = why the engine could not start; shown instead of the grid.
    engine: Result<Engine, String>,
    workspace: Workspace,
    tr: Tr,
    cache: ThumbCache<TextureHandle>,
    /// Last set sent with `Command::Visible`, to avoid resending an unchanged set.
    last_visible: Vec<PageRef>,
    /// Scale the cached thumbnails were rendered for.
    pixels_per_point: f32,
    /// Dismissible messages at the bottom (failures, results).
    messages: Vec<Message>,
    /// Contents of the Pages field and the parse error shown under it.
    pages_text: String,
    pages_error: Option<RangeError>,
    next_job: u64,
    /// The running save or split, if any.
    batch: Option<Batch>,
    /// Encrypted files waiting for a password (path, previous attempt was wrong).
    passwords: VecDeque<(PathBuf, bool)>,
    password_text: String,
    close_prompt: bool,
    /// A save to an existing file (name had no ".pdf") waiting for the user's confirmation.
    pending_overwrite: Option<(Vec<PageRef>, PathBuf)>,
    /// The user confirmed closing; stops the cancel loop.
    closing: bool,
    /// The engine thread ended; its error was already shown.
    engine_dead: bool,
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

/// Modal with a title, a text, a confirm button and Cancel (keys are i18n keys).
/// Returns Some(true) on confirm, Some(false) on Cancel, Esc or a click outside, None while
/// unanswered.
fn confirm_modal(
    ctx: &egui::Context,
    tr: &Tr,
    id: &str,
    title_key: &str,
    body: &str,
    confirm_key: &str,
) -> Option<bool> {
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::NULL.with(id)).show(ctx, |ui| {
        ui.heading(tr.t(title_key));
        ui.label(body);
        ui.horizontal(|ui| {
            if ui.button(tr.t(confirm_key)).clicked() {
                answer = Some(true);
            }
            if ui.button(tr.t("common.cancel")).clicked() {
                answer = Some(false);
            }
        });
    });
    answer.or_else(|| modal.should_close().then_some(false))
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
            cache: ThumbCache::new(THUMB_CACHE_BYTES),
            last_visible: Vec::new(),
            pixels_per_point: ctx.pixels_per_point(),
            messages: Vec::new(),
            pages_text: String::new(),
            pages_error: None,
            next_job: 0,
            batch: None,
            passwords: VecDeque::new(),
            password_text: String::new(),
            close_prompt: false,
            pending_overwrite: None,
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
        // No more events will come: unblock Save and Split.
        self.batch = None;
        if !std::mem::replace(&mut self.engine_dead, true) {
            self.error("error.internal", vec![]);
        }
    }

    fn error(&mut self, key: &'static str, args: Vec<(&'static str, String)>) {
        let message = (key, args);
        eprintln!("{}", message_text(&self.tr, &message));
        self.messages.push(message);
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
                    let bytes = width as usize * height as usize * 4;
                    self.cache.insert(page, tex, bytes);
                }
                Event::OpenFailed { path, reason } => {
                    self.error(
                        "open.failed",
                        vec![("file", file_name(&path)), ("reason", reason)],
                    );
                }
                Event::PasswordRequired { path, wrong } => self.passwords.push_back((path, wrong)),
                Event::SaveProgress { job, done, .. } => {
                    if let Some(b) = &mut self.batch {
                        b.progress(job, done);
                    }
                }
                Event::Saved { job, dest } => {
                    if self.batch.as_ref().is_some_and(|b| !b.is_split()) {
                        self.messages
                            .push(("save.done", vec![("file", file_name(&dest))]));
                    }
                    self.finish_job(job, true);
                }
                Event::SaveFailed { job, dest, reason } => {
                    self.error(
                        "save.failed",
                        vec![("file", file_name(&dest)), ("reason", reason)],
                    );
                    self.finish_job(job, false);
                }
            }
        }
    }

    fn finish_job(&mut self, job: u64, ok: bool) {
        let current = self.workspace.all_pages();
        let Some(outcome) = self
            .batch
            .as_mut()
            .and_then(|b| b.finish(job, ok, &current))
        else {
            return;
        };
        self.batch = None;
        if let Some((count, folder)) = outcome.split_done {
            let key = if count == 1 {
                "split.done_one"
            } else {
                "split.done"
            };
            let folder = folder.to_string_lossy().into_owned();
            self.messages
                .push((key, vec![("count", count.to_string()), ("folder", folder)]));
        }
        if outcome.mark_saved {
            self.workspace.mark_saved();
        }
    }

    /// Sends one `Save` per (pages, dest) and starts tracking them as a batch.
    fn start_batch(&mut self, jobs: Vec<(Vec<PageRef>, PathBuf)>, split_folder: Option<PathBuf>) {
        if jobs.iter().any(|(_, d)| self.workspace.is_source_path(d)) {
            self.messages.push(("save.blocked_original", vec![]));
            return;
        }
        let all = self.workspace.all_pages();
        let (batch, send) = Batch::new(jobs, all, split_folder, &mut self.next_job);
        self.batch = Some(batch);
        for (job, pages, dest) in send {
            self.send(Command::Save { job, pages, dest });
        }
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
        let appended = !dest
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        if appended {
            let mut name = OsString::from(dest.file_name().unwrap_or_default());
            name.push(".pdf");
            dest.set_file_name(name);
        }
        let pages = if selection_only {
            self.workspace
                .pages_at(&self.workspace.selected_positions())
        } else {
            self.workspace.all_pages()
        };
        // The dialog checked the name without ".pdf", so ask before replacing this file.
        if appended && dest.exists() && !self.workspace.is_source_path(&dest) {
            self.pending_overwrite = Some((pages, dest));
            return;
        }
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
                let file = folder.to_string_lossy().into_owned();
                self.error(
                    "save.failed",
                    vec![("file", file), ("reason", e.to_string())],
                );
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
            let sel = self.workspace.has_selection();
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
        let ppp = ui.ctx().pixels_per_point();
        if ppp != self.pixels_per_point {
            // Re-render at the new scale so thumbnails stay sharp.
            self.pixels_per_point = ppp;
            self.cache.clear();
            self.last_visible.clear();
        }
        let mut visible: Vec<PageRef> = Vec::new();
        let mut action: Option<GridAction> = None;
        // Cell rects by grid position, collected while dragging to find the drop target.
        let mut cells: Vec<egui::Rect> = Vec::new();
        let dragging = egui::DragAndDrop::has_payload_of_type::<usize>(ui.ctx());
        let mods = ui.input(|i| i.modifiers);
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
                        match self.cache.get(&slot.page) {
                            Some(tex) => {
                                // Letterbox: the rendered bitmap can differ from the clamped cell.
                                let size = tex.size_vec2();
                                let scale =
                                    (img_rect.width() / size.x).min(img_rect.height() / size.y);
                                ui.painter().image(
                                    tex.id(),
                                    egui::Rect::from_center_size(img_rect.center(), size * scale),
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
                    ui.painter().line_segment(
                        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                        egui::Stroke::new(3.0, ui.visuals().selection.bg_fill),
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

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        if let Some(b) = &self.batch {
            let (done, total) = b.total_progress();
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
                ui.label(message_text(&self.tr, message));
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
        let modal = egui::Modal::new(egui::Id::NULL.with("password")).show(ctx, |ui| {
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
        cancel |= !ok && modal.should_close();
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

    fn overwrite_modal(&mut self, ctx: &egui::Context) {
        let Some((_, dest)) = &self.pending_overwrite else {
            return;
        };
        let body = self
            .tr
            .tf("save.overwrite_body", &[("file", &file_name(dest))]);
        let answer = confirm_modal(
            ctx,
            &self.tr,
            "overwrite",
            "save.overwrite_title",
            &body,
            "save.overwrite_confirm",
        );
        match answer {
            Some(true) => {
                if let Some(job) = self.pending_overwrite.take() {
                    self.start_batch(vec![job], None);
                }
            }
            Some(false) => self.pending_overwrite = None,
            None => {}
        }
    }

    fn close_modal(&mut self, ctx: &egui::Context) {
        if !self.close_prompt {
            return;
        }
        let answer = confirm_modal(
            ctx,
            &self.tr,
            "close",
            "close.unsaved_title",
            self.tr.t("close.unsaved_body"),
            "close.confirm",
        );
        match answer {
            Some(true) => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(false) => self.close_prompt = false,
            None => {}
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

        let modal_open =
            self.close_prompt || !self.passwords.is_empty() || self.pending_overwrite.is_some();
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
        self.overwrite_modal(&ctx);
        self.close_modal(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn messages_are_translated_when_drawn() {
        let m: Message = ("save.done", vec![("file", "a.pdf".into())]);
        assert_eq!(message_text(&Tr::new(Lang::En), &m), "Saved a.pdf");
        assert_eq!(message_text(&Tr::new(Lang::It), &m), "Salvato a.pdf");
    }

    /// Shows the overwrite modal in a harness; the state keeps its first answer.
    fn overwrite_harness() -> Harness<'static, Option<bool>> {
        let tr = Tr::new(Lang::En);
        let mut harness = Harness::new_ui_state(
            move |ui, answer: &mut Option<bool>| {
                let a = confirm_modal(
                    ui.ctx(),
                    &tr,
                    "overwrite",
                    "save.overwrite_title",
                    "out.pdf already exists. Replace it?",
                    "save.overwrite_confirm",
                );
                *answer = answer.or(a);
            },
            None,
        );
        harness.run();
        assert_eq!(*harness.state(), None, "no answer before input");
        harness
    }

    #[test]
    fn overwrite_modal_replace() {
        let mut harness = overwrite_harness();
        harness.get_by_label("Replace").click();
        harness.run();
        assert_eq!(*harness.state(), Some(true));
    }

    #[test]
    fn overwrite_modal_cancel() {
        let mut harness = overwrite_harness();
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(*harness.state(), Some(false));
    }

    #[test]
    fn overwrite_modal_click_outside_cancels() {
        let mut harness = overwrite_harness();
        let corner = egui::pos2(2.0, 2.0);
        harness.hover_at(corner);
        harness.drag_at(corner);
        harness.drop_at(corner);
        harness.run();
        assert_eq!(*harness.state(), Some(false));
    }

    #[test]
    fn overwrite_modal_escape_cancels() {
        let mut harness = overwrite_harness();
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert_eq!(*harness.state(), Some(false));
    }
}
