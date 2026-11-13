//! Main window: sidebar, top bar, thumbnail grid and toasts.

mod grid;
mod icons;
mod modals;
mod sidebar;
mod style;
mod toasts;
mod toolbar;

use crate::batch::Batch;
use crate::engine::{Command, Engine, Event};
use crate::i18n::Tr;
use crate::palette::{self, Palette};
use crate::ranges::{self, RangeError};
use crate::settings::{LanguagePref, Settings, ThemePref};
use crate::thumb_cache::ThumbCache;
use crate::toasts::{ToastKind, Toasts};
use crate::workspace::{PageRef, SourceDoc, Workspace, free_split_names, split_stem};
use eframe::egui::{self, TextureHandle, TextureOptions};
use std::collections::{HashSet, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;
use toasts::message_text;

/// Bytes of thumbnail textures kept (RGBA, 4 bytes per pixel).
const THUMB_CACHE_BYTES: usize = 48 << 20;

pub struct AdiutorApp {
    /// Err = why the engine could not start; shown instead of the grid.
    engine: Result<Engine, String>,
    workspace: Workspace,
    tr: Tr,
    settings: Settings,
    settings_path: Option<PathBuf>,
    /// The settings file was unreadable; the first save keeps a `.bak` copy.
    settings_corrupt: bool,
    cache: ThumbCache<TextureHandle>,
    /// Last set sent with `Command::Visible`, to avoid resending an unchanged set.
    last_visible: Vec<PageRef>,
    /// Scale the cached thumbnails were rendered for.
    pixels_per_point: f32,
    /// Failures and results, drawn as toasts.
    toasts: Toasts,
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
    /// `selection_only` of a save the user aimed at an open original.
    blocked_save: Option<bool>,
    /// File name shown in the blocked-save modal.
    blocked_file: String,
    /// The user confirmed closing; stops the cancel loop.
    closing: bool,
    /// The engine thread ended; its error was already shown.
    engine_dead: bool,
    #[cfg(target_os = "macos")]
    quit_events: crate::macos::QuitEvents,
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

fn theme_preference(t: ThemePref) -> egui::ThemePreference {
    match t {
        ThemePref::System => egui::ThemePreference::System,
        ThemePref::Light => egui::ThemePreference::Light,
        ThemePref::Dark => egui::ThemePreference::Dark,
    }
}

/// Installs both themes (so System follows the OS live), the theme preference and the control height.
fn apply_style(ctx: &egui::Context, pref: ThemePref) {
    ctx.set_visuals_of(
        egui::Theme::Light,
        palette::visuals(&Palette::light(), false),
    );
    ctx.set_visuals_of(egui::Theme::Dark, palette::visuals(&Palette::dark(), true));
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |s| s.spacing.interact_size.y = 40.0);
    }
    ctx.set_theme(theme_preference(pref));
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

impl AdiutorApp {
    pub fn new(
        ctx: &egui::Context,
        tr: Tr,
        loaded: crate::settings::Loaded,
        settings_path: Option<PathBuf>,
        startup_paths: Vec<PathBuf>,
    ) -> Self {
        apply_style(ctx, loaded.settings.theme);
        let wake_ctx = ctx.clone();
        let engine = Engine::start(move || wake_ctx.request_repaint());
        let mut app = Self {
            engine,
            workspace: Workspace::new(),
            tr,
            settings: loaded.settings,
            settings_path,
            settings_corrupt: loaded.was_corrupt,
            cache: ThumbCache::new(THUMB_CACHE_BYTES),
            last_visible: Vec::new(),
            pixels_per_point: ctx.pixels_per_point(),
            toasts: Toasts::default(),
            pages_text: String::new(),
            pages_error: None,
            next_job: 0,
            batch: None,
            passwords: VecDeque::new(),
            password_text: String::new(),
            close_prompt: false,
            pending_overwrite: None,
            blocked_save: None,
            blocked_file: String::new(),
            closing: false,
            engine_dead: false,
            #[cfg(target_os = "macos")]
            quit_events: crate::macos::QuitEvents::install(ctx),
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

    /// Applies and saves new settings; a failed save is reported but the change stays for this run.
    fn set_settings(&mut self, ctx: &egui::Context, s: Settings) {
        if s.theme != self.settings.theme {
            ctx.set_theme(theme_preference(s.theme));
        }
        self.settings = s;
        let Some(path) = &self.settings_path else {
            return;
        };
        match crate::settings::save_to(path, &self.settings, self.settings_corrupt) {
            Ok(()) => self.settings_corrupt = false,
            Err(e) => self.error("settings.save_failed", vec![("reason", e.to_string())]),
        }
    }

    fn error(&mut self, key: &'static str, args: Vec<(&'static str, String)>) {
        let message = (key, args);
        eprintln!("{}", message_text(&self.tr, &message));
        self.toasts.push(ToastKind::Error, message, Instant::now());
    }

    fn lang_label(&self, p: LanguagePref) -> &str {
        match p {
            LanguagePref::System => self.tr.t("lang.system"),
            LanguagePref::English => self.tr.t("lang.en"),
            LanguagePref::Italian => self.tr.t("lang.it"),
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
                        let msg = ("save.done", vec![("file", file_name(&dest))]);
                        self.toasts.push(ToastKind::Success, msg, Instant::now());
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
            let msg = (key, vec![("count", count.to_string()), ("folder", folder)]);
            self.toasts.push(ToastKind::Success, msg, Instant::now());
        }
        if outcome.mark_saved {
            self.workspace.mark_saved();
        }
    }

    /// Sends one `Save` per (pages, dest) and starts tracking them as a batch.
    fn start_batch(&mut self, jobs: Vec<(Vec<PageRef>, PathBuf)>, split_folder: Option<PathBuf>) {
        if jobs.iter().any(|(_, d)| self.workspace.is_source_path(d)) {
            let file = jobs
                .iter()
                .map(|(_, d)| d)
                .find(|d| self.workspace.is_source_path(d))
                .map(|d| file_name(d))
                .unwrap_or_default();
            self.error("save.blocked_body", vec![("file", file)]);
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
        if self.workspace.is_source_path(&dest) {
            self.blocked_save = Some(selection_only);
            self.blocked_file = file_name(&dest);
            return;
        }
        let pages = if selection_only {
            self.workspace
                .pages_at(&self.workspace.selected_positions())
        } else {
            self.workspace.all_pages()
        };
        // The dialog checked the name without ".pdf", so ask before replacing this file.
        if appended && dest.exists() {
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
}

impl AdiutorApp {
    /// True if closing now would lose something, so the close must be confirmed first.
    fn needs_close_confirm(&self) -> bool {
        self.workspace.is_dirty() || self.batch.is_some()
    }
}

impl eframe::App for AdiutorApp {
    // eframe calls `logic` every pass, but `ui` only while the window is visible (not minimized,
    // hidden or covered), so anything that must work then lives here.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events(ctx);
        let now = Instant::now();
        self.toasts.expire(now);
        if let Some(d) = self.toasts.next_expiry(now) {
            ctx.request_repaint_after(d);
        }

        // The default macOS menu is off (see lib.rs), so Cmd+Q arrives here as a key press.
        #[cfg(target_os = "macos")]
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Quit from the Dock or logout: same path as closing the window, so unsaved changes ask.
        #[cfg(target_os = "macos")]
        if self.quit_events.quit_requested() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            if self.needs_close_confirm() {
                crate::macos::bring_app_forward();
            }
        }

        if ctx.input(|i| i.viewport().close_requested())
            && !self.closing
            && self.needs_close_confirm()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_prompt = true;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
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

        let modal_open = self.close_prompt
            || !self.passwords.is_empty()
            || self.pending_overwrite.is_some()
            || self.blocked_save.is_some();
        let delete = !modal_open
            && !ctx.egui_wants_keyboard_input()
            && ctx
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete {
            self.workspace.delete_selected();
        }
        if !modal_open
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::A))
        {
            self.workspace.select_all();
        }

        if let Err(reason) = &self.engine {
            let text = self.tr.tf("error.engine_start", &[("reason", reason)]);
            egui::CentralPanel::default().show(ui, |ui| ui.label(text));
            return;
        }
        self.sidebar(ui);
        self.toolbar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(e) = &self.pages_error {
                let text = range_error_text(&self.tr, e);
                ui.colored_label(ui.visuals().error_fg_color, text);
            }
            self.grid(ui);
            self.toasts_ui(ui);
        });
        self.password_modal(&ctx);
        self.overwrite_modal(&ctx);
        self.blocked_modal(&ctx);
        self.close_modal(&ctx);
    }
}
