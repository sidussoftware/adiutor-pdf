# UI Redesign (direction B) and Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the default egui look with prototype B: a document sidebar with letters and colours, a top bar,
toasts, an empty state, Inter, our own icons, and Light/Dark/System themes. Also add a persistent settings module for
language and theme.

**Architecture:** Pure, unit-tested modules carry the logic:
- `settings.rs`: settings model, file I/O and config path;
- `palette.rs`: colours, contrast and document colours/letters;
- `workspace.rs` additions: per-document counts and display names;
- `toasts.rs`: toast model and expiry.

`ui.rs` is first split into `ui/` files with no behaviour change. Each later task then changes one area.

**Tech Stack:** Rust 1.95, eframe/egui 0.37 (glow), serde (derive) and toml for settings, egui_kittest (dev), Inter 4.1
(OFL).

**Spec:** `docs/superpowers/specs/2026-11-13-ui-redesign-design.md`

## Global Constraints
- No continuous repaint; idle CPU stays about 0%. Toast expiry uses one `request_repaint_after`.
- Contrast:
  - text ≥ 4.5:1, and UI elements (borders, accent, document colours) ≥ 3:1, in both themes, tested;
  - selection, document identity and button state are never shown by colour alone.
- Controls are at least 40 px tall. They are real egui widgets with text labels; icons are decorative.
- All UI text goes through i18n with English/Italian key and placeholder parity.
- New runtime crates: only `serde` (with `derive`) and `toml`. No other new runtime crates.
- The v0.1.x guarantees are unchanged: originals are never overwritten, split never overwrites, no partial output,
  and the unsaved-changes prompt stays.
- The sidebar is view-only (actions are #6).
- Minimum window size 1024×600.
- fmt, `clippy --all-targets -D warnings` and tests are clean on Linux. Clippy is also clean for
  `x86_64-apple-darwin` and `x86_64-pc-windows-msvc`. `cargo about generate --locked --fail` passes
  (`bash scripts/third-party-licenses.sh <out>`).

## Review Focus
1. A missing, read-only or corrupt config folder or file: the app starts with defaults and never crashes. A save
   failure shows one toast. (Task 1 tests corrupt and missing files; Task 1 Step 6 runs a read-only folder by hand.)
2. More than 26 documents: letters go to AA, AB…, and colours repeat while letters stay unique. (Task 6 test
   `letters_spreadsheet_style`.)
3. Many errors at once (a split of 30 files that all fail): no toast is dropped, and the stack scrolls. (Task 4 test
   `errors_never_expire_and_are_kept`.)
4. Switching the theme to System while the OS is in dark mode: it follows the OS without a restart. (Task 3 manual
   check.)
5. Two files with the same name from different folders: the sidebar shows the parent folder for both. (Task 6 test
   `display_names_disambiguate_same_file_name`.)

---

### Task 1: Settings module and persistence of the language

**Files:**
- Create: `src/settings.rs`
- Modify: `Cargo.toml`, `src/lib.rs` (startup), `src/ui.rs` (language combo), `src/i18n.rs` (optional
  `Lang::from_pref`), `locales/en.txt`, `locales/it.txt`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
  #[serde(default)]
  pub struct Settings { pub language: LanguagePref, pub theme: ThemePref }
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
  #[serde(rename_all = "lowercase")]
  pub enum LanguagePref { #[default] System, #[serde(rename = "en")] English, #[serde(rename = "it")] Italian }
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
  #[serde(rename_all = "lowercase")]
  pub enum ThemePref { #[default] System, Light, Dark }
  pub struct Loaded { pub settings: Settings, pub was_corrupt: bool }
  pub fn load_from(path: &Path) -> Loaded;
  pub fn save_to(path: &Path, s: &Settings, backup_corrupt: bool) -> std::io::Result<()>;
  pub fn config_path() -> Option<PathBuf>;                       // uses config_path_from(env)
  fn config_path_from(get: impl Fn(&str) -> Option<String>) -> Option<PathBuf>;
  impl LanguagePref { pub fn resolve(self) -> Lang }             // System -> Lang::detect()
  ```
  `AdiutorApp` holds `settings: Settings`, `settings_path: Option<PathBuf>` and `settings_corrupt: bool`. Any later
  task changes a setting through `fn set_settings(&mut self, s: Settings)`. It saves right away and shows the
  error toast key `settings.save_failed` with `{reason}` on failure; Task 4 turns it into a toast, until then use
  `self.error`.

- [ ] **Step 1: Failing tests** in `settings.rs`:
  - `round_trip`: save, then load, gives equal settings;
  - `missing_fields_default`: an empty file gives `Settings::default()`;
  - `unknown_fields_ignored`: `language = "it"\nfuture = 3` gives `Italian`;
  - `corrupt_file_defaults_and_untouched`: `language = [` gives the defaults with `was_corrupt == true`, and the
    file bytes are unchanged;
  - `missing_file_defaults`;
  - `save_leaves_no_temp_file`: the folder contains only `settings.toml`;
  - `corrupt_file_backed_up_on_first_save`: `save_to(.., true)` leaves `settings.toml.bak` with the old bytes;
  - `config_path_per_os`, with an injected env (cfg-gated per target_os; test the current OS branch):
    - Linux: `XDG_CONFIG_HOME=/x` gives `/x/adiutor-pdf/settings.toml`;
    - no XDG and `HOME=/h` gives `/h/.config/adiutor-pdf/settings.toml`;
    - neither gives `None`.
- [ ] **Step 2:** Run `cargo test --lib settings`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - Add `serde = { version = "1", features = ["derive"] }` and `toml = "<the version in Cargo.lock>"` to
    `[dependencies]`.
  - Write with `toml::to_string_pretty`.
  - Temp file `.settings.toml.tmp`, then rename. Create the parent dirs.
  - macOS path: `$HOME/Library/Application Support/adiutor-pdf/settings.toml`. Windows: `%APPDATA%\adiutor-pdf\settings.toml`.
- [ ] **Step 4: Wire it up.**
  - In `lib.rs`, load before creating `Tr` and the viewport: `Tr::new(settings.language.resolve())`.
  - The language combo offers three entries, System / English / Italiano. Keys `lang.system`, `lang.en`, `lang.it`;
    the native names stay as they are. Changing it calls `set_settings`.
- [ ] **Step 5:** Run `cargo test`, fmt and clippy (three targets). Expected: clean. Then
  `bash scripts/third-party-licenses.sh /tmp/claude-0/-home-user-adiutor-pdf/b3d0a552-6ee3-599e-81fd-644f27e79979/scratchpad/n.html`
  must pass, because `toml` adds new crates.
- [ ] **Step 6: Manual check.** Run under Xvfb with `XDG_CONFIG_HOME` set to a temp dir. Choose Italiano, restart,
  and the UI is Italian. Make the dir read-only, change the language: an error message appears and there is no crash.
- [ ] **Step 7: Commit** `feat: persistent settings module; language choice with System option`.

### Task 2: Split `ui.rs` into `ui/` modules (no behaviour change)

**Files:**
- Create: `src/ui/mod.rs` (AdiutorApp, App impl, `logic`/`ui`), `src/ui/toolbar.rs`, `src/ui/grid.rs`,
  `src/ui/modals.rs` (`confirm_modal` and the four modal methods), `src/ui/messages.rs` (`Message`, `message_text`,
  bottom bar), `src/ui/style.rs` (`selection_accent`, `on_accent` and their test)
- Delete: `src/ui.rs`

**Interfaces:**
- Produces: the same public API (`ui::AdiutorApp::new`). Internal methods stay `impl AdiutorApp` blocks spread over
  the files, with `pub(super)` where needed.

- [ ] **Step 1:** Move the code. Only module wiring changes; no logic edits. Existing tests move with their code.
- [ ] **Step 2:** Run `cargo test`, fmt and clippy (three targets). Expected: the same test count passes (record it).
- [ ] **Step 3:** Run under Xvfb with a PDF and take one screenshot. It must look identical to before.
- [ ] **Step 4: Commit** `refactor: split ui.rs into ui/ modules`.

### Task 3: Palette, themes, Inter and icons

**Files:**
- Create: `src/palette.rs`, `src/ui/icons.rs`, `assets/fonts/inter/Inter-Regular.ttf`,
  `assets/fonts/inter/Inter-SemiBold.ttf`, `assets/fonts/inter/OFL.txt`, `assets/fonts/inter/SHA256SUMS`
- Modify: `src/ui/style.rs`, used everywhere `selection_accent`/`on_accent` were used; `src/ui/mod.rs`, which applies
  the theme; `src/lib.rs` (fonts, `with_min_inner_size([1024.0, 600.0])`); locales

**Interfaces:**
- Produces:
  ```rust
  pub struct Palette { pub bg: Color32, pub sidebar: Color32, pub topbar: Color32, pub card: Color32,
      pub border: Color32, pub border_strong: Color32, pub text: Color32, pub text_secondary: Color32,
      pub accent: Color32, pub on_accent: Color32, pub error_bg: Color32, pub error_text: Color32,
      pub success_bg: Color32, pub success_text: Color32, pub doc_colors: [Color32; 8] }
  impl Palette { pub fn light() -> Self; pub fn dark() -> Self; pub fn for_dark_mode(dark: bool) -> Self }
  pub fn visuals(p: &Palette, dark: bool) -> egui::Visuals;      // radius 8, 40px interact size via Style
  pub fn contrast(a: Color32, b: Color32) -> f32;                // WCAG, sRGB linearised
  // ui/icons.rs — each draws a stroke icon fitted in `rect`
  pub fn plus(p: &egui::Painter, rect: Rect, color: Color32);  // likewise: split, save, close, check, file_add
  ```
  The current palette for drawing is `Palette::for_dark_mode(ui.visuals().dark_mode)`.

- [ ] **Step 1: Failing test** `palette_contrast_both_themes`. For `light()` and `dark()`:
  - `text` and `text_secondary` against `bg`, `sidebar`, `topbar` and `card` ≥ 4.5;
  - `on_accent` against `accent` ≥ 4.5;
  - `accent` against `bg` ≥ 3;
  - `border_strong` against `card` ≥ 3;
  - `error_text` against `error_bg` ≥ 4.5, and `success_text` against `success_bg` ≥ 4.5;
  - each `doc_colors[i]` against `sidebar` ≥ 3 and against `Color32::WHITE` ≥ 3.

  Move the existing selection-contrast test onto `accent` and `on_accent`.
- [ ] **Step 2:** Run the test. Expected: FAIL.
- [ ] **Step 3: Implement the palette.** Start from the spec's table. The first two document colours per theme are
  the spec's values. Pick the other six so they stay apart for colour-vision deficiencies and differ in lightness,
  then adjust any value the test rejects. The page thumbnail background stays white.
- [ ] **Step 4: Apply the theme.**
  - At start, and whenever the setting changes: `ctx.set_theme(match pref {System => ThemePreference::System,
    Light => ThemePreference::Light, Dark => ThemePreference::Dark})`.
  - Set both `ctx.set_visuals_of(Theme::Light, visuals(&light(), false))` and the dark equivalent, so System follows
    the OS live.
  - The selection frame, pill and badge use `accent`/`on_accent`.
- [ ] **Step 5: Inter.**
  - Download `Inter-4.1.zip` from the rsms/inter GitHub release.
  - Commit its static `Inter-Regular.ttf`, `Inter-SemiBold.ttf` and `OFL.txt` (the zip's `extras/ttf/` folder or
    equivalent), and write `SHA256SUMS`.
  - In `lib.rs`, insert `Inter-Regular` as the first proportional font, keeping egui's defaults as fallbacks. Add a
    named family `"semibold"` for Inter-SemiBold.
  - Record the release binary size before and after in the report.
- [ ] **Step 6: Icons.** Plus, split (scissors), save (down-arrow into tray), close (×), check and file_add (file with
  an up-arrow). Use 1.75 px strokes and round caps. Show them in the existing buttons next to their text.
- [ ] **Step 7:** Run tests, fmt and clippy (three targets). Take Xvfb screenshots in Light and in Dark, forcing the
  theme with the setting (settings file).
- [ ] **Step 8: Commit** `feat: palette, Light/Dark/System themes, Inter font and vector icons`.

### Task 4: Toasts and the progress card (remove the bottom bar)

**Files:**
- Create: `src/toasts.rs` (pure), `src/ui/toasts.rs` (drawing)
- Modify: `src/ui/messages.rs` (removed or reduced), `src/ui/mod.rs`, all `messages.push`/`error` call sites; locales
  (`progress.saving` with `{done}` / `{total}`)

**Interfaces:**
- Produces:
  ```rust
  pub enum ToastKind { Success, Error }
  pub struct Toast { pub id: u64, pub kind: ToastKind, pub msg: Message, pub shown_at: Instant }
  pub struct Toasts { /* items, next id */ }
  impl Toasts {
      pub const SUCCESS_TTL: Duration = Duration::from_secs(5);
      pub fn push(&mut self, kind: ToastKind, msg: Message, now: Instant);
      pub fn close(&mut self, id: u64);
      pub fn expire(&mut self, now: Instant);                   // removes expired Success only
      pub fn next_expiry(&self, now: Instant) -> Option<Duration>;
      pub fn items(&self) -> impl Iterator<Item = &Toast>;      // newest first
  }
  ```
  `Message` moves to `toasts.rs`: `(&'static str, Vec<(&'static str, String)>)`. `AdiutorApp::error` pushes Error,
  and success messages push Success.

- [ ] **Step 1: Failing tests:**
  - `success_expires_after_ttl`;
  - `errors_never_expire_and_are_kept`: push 30 errors, expire 1 h later, and all 30 remain;
  - `next_expiry_is_earliest_success`;
  - `close_removes_by_id`;
  - `items_newest_first`.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3: Implement** `toasts.rs`.
- [ ] **Step 4: Draw the toasts.**
  - Use an `egui::Area` anchored top-right of the central panel, at most 360 px wide, below the top bar.
  - Use a `ScrollArea` when the stack is taller than half the central height.
  - Each toast shows its text and a close button (`aria` label key `common.close`, accessible name "Chiudi messaggio").
  - Colours: Error uses `error_bg`/`error_text`; Success uses `success_bg`/`success_text`.
  - In `logic()`: `toasts.expire(now)`, and if `next_expiry` is `Some(d)`, call `ctx.request_repaint_after(d)`.
- [ ] **Step 5: Progress card.** While a batch runs, draw a card at the top of the stack with
  `progress.saving` X / Y and an `egui::ProgressBar`. Remove the bottom bar.
- [ ] **Step 6:** Run tests, fmt and clippy (three targets). Under Xvfb:
  - a save shows a success toast that disappears after about 5 s;
  - a corrupt PDF shows an error toast that stays;
  - `top` at rest shows about 0% after the toast is gone.
- [ ] **Step 7: Commit** `feat: toasts and progress card replace the bottom bar`.

### Task 5: Sidebar and top bar layout

**Files:**
- Create: `src/ui/sidebar.rs`
- Modify: `src/ui/toolbar.rs` (becomes the top bar), `src/ui/mod.rs` (panel order), `src/workspace.rs`, locales

**Interfaces:**
- Consumes: `Palette`, icons (Task 3); `set_settings` (Task 1).
- Produces in `workspace.rs`:
  ```rust
  pub struct DocSummary { pub id: DocId, pub letter: String, pub name: String, pub pages_in_grid: usize }
  pub fn doc_letter(index: usize) -> String;                     // 0->"A", 25->"Z", 26->"AA", 27->"AB", 701->"ZZ", 702->"AAA"
  pub fn display_names(paths: &[PathBuf]) -> Vec<String>;       // file name; "name — parent/folder" when names collide
  impl Workspace { pub fn doc_summaries(&self) -> Vec<DocSummary>; }  // open order; index = position in docs
  ```

- [ ] **Step 1: Failing tests:**
  - `letters_spreadsheet_style`: the values above;
  - `display_names_disambiguate_same_file_name`:
    - `["/a/x/c.pdf", "/b/y/c.pdf", "/d.pdf"]` gives `["c.pdf — a/x", "c.pdf — b/y", "d.pdf"]`, using the last two
      path components of the parent (or fewer if not available);
  - `doc_summaries_count_pages_in_grid`: after deleting a page of doc 0, its count drops, and a doc with 0 pages stays
    listed.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3: Implement** the workspace functions.
- [ ] **Step 4: Layout.**
  - `egui::Panel::left("sidebar")`, exact width 264, not resizable, `sidebar` fill. It contains:
    - "adiutor-pdf" in SemiBold;
    - the **Aggiungi PDF** button (plus icon, dashed border);
    - the heading `sidebar.documents`;
    - the rows: letter badge, a swatch in `doc_colors[id % 8]`, the name (truncate + hover full) and the count via
      `sidebar.pages_one` / `sidebar.pages_many` with `{count}`. If empty: `sidebar.empty`. Rows scroll.
    - pinned at the bottom: the Lingua and Tema combos (`settings.language`, `settings.theme`; theme options
      `theme.system`, `theme.light`, `theme.dark`), both calling `set_settings`.
  - The top bar (`Panel::top` inside the central area, `topbar` fill, height 68) holds:
    - left: Pagine, the field and Seleziona;
    - right: Dividi, Salva selezione and **Salva PDF**, the primary button filled with `accent`.
  - Rename `action.save_all` to "Save PDF" / "Salva PDF". Keep the existing enable rules.
- [ ] **Step 5: kittest** `sidebar_lists_documents_with_letters_and_counts`. Build a sidebar UI from two
  `DocSummary` values and query the labels "A", "contratto.pdf" and "8 pages".
- [ ] **Step 6:** Run tests, fmt and clippy (three targets). Take an Xvfb screenshot with two PDFs and compare it with
  prototype B.
- [ ] **Step 7: Commit** `feat: document sidebar and top bar (direction B)`.

### Task 6: Thumbnail letters and colours, tooltip, empty state

**Files:**
- Modify: `src/ui/grid.rs`, locales

**Interfaces:**
- Consumes: `doc_summaries`, `doc_letter` and `Palette::doc_colors` (Tasks 3 and 5).

- [ ] **Step 1: Thumbnails.**
  - Draw a 4 px strip in the document's colour along the top edge of the drawn page.
  - Put a small tag with the document letter at its left end (`on`-contrast text: white or black, whichever passes
    4.5:1 against the colour, using `palette::contrast`).
  - The hover tooltip reads "<display name>, page N" (key `grid.page_tooltip` with `{file}` and `{page}`), where N is
    the 1-based page in its source document.
  - The v0.1.3 selection drawing is unchanged except for its colours.
- [ ] **Step 2: Empty state.** When the workspace is empty, the central area shows a dashed rounded panel with:
  - the `file_add` icon;
  - `empty.title` ("Drop PDFs here" / "Trascina qui i PDF") and `empty.or`;
  - a primary **Aggiungi PDF** button that opens the picker;
  - `empty.hint` (the spec's exact sentence in Italian, and an English equivalent).
- [ ] **Step 3: Test** `tag_text_contrast_on_every_doc_color`: in both themes, the tag text chosen for each of the 8
  colours is ≥ 4.5:1. Add a kittest `empty_state_shows_drop_panel`.
- [ ] **Step 4:** Run tests, fmt and clippy (three targets). Take Xvfb screenshots: two documents (light and dark)
  and empty. Compare them with the three B boards.
- [ ] **Step 5: Commit** `feat: document strip and letter on thumbnails; empty state`.

### Task 7: Packaging, docs, version 0.2.0

**Files:**
- Modify: `.github/workflows/release.yml` (copy `assets/fonts/inter/OFL.txt` into each package's
  `licenses/fonts/Inter-OFL.txt`), `README.md` (Licenses section names Inter; Usage describes the sidebar, themes and
  remembered settings, and where the settings file lives), `docs/manual-test-checklist.md` (sidebar, letters,
  themes, persistence, toasts, empty state, minimum window size), `Cargo.toml` / `Cargo.lock` (0.2.0)

- [ ] **Step 1:** Make the edits above. Run actionlint with the binary already at
  `/tmp/claude-0/-home-user-adiutor-pdf/b3d0a552-6ee3-599e-81fd-644f27e79979/scratchpad/actionlint`.
- [ ] **Step 2:** Run the release job's Linux packaging commands locally and confirm
  `licenses/fonts/Inter-OFL.txt` is present.
- [ ] **Step 3:** Run tests, fmt, clippy (three targets) and the licenses script.
- [ ] **Step 4: Commit** `chore: package Inter licence, docs and checklist; version 0.2.0`.
