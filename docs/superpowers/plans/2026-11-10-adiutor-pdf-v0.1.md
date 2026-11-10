# adiutor-pdf v0.1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A portable, offline desktop app (Windows, macOS, Linux) to merge, split/extract, reorder and delete PDF pages
from one workspace window.

**Architecture:** One Rust crate (lib + thin bin). A pure-Rust `workspace` holds the ordered page list and selection.
A dedicated worker thread (`engine`) is the only owner of PDFium and talks to the egui UI through channels. The UI
draws state and sends commands.

**Tech Stack:** Rust (edition 2024, MSRV 1.95), eframe/egui 0.37 (glow backend), pdfium-render 0.9 + PDFium binaries
`chromium/8125` from `bblanchon/pdfium-binaries`, rfd 0.18, sys-locale 0.3, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-10-adiutor-pdf-design.md`

## Global Constraints

- License: `MIT OR Apache-2.0`; every dependency must be MIT/Apache-2.0/BSD-compatible.
- Docs and code comments in English. UI strings only through `i18n` (English and Italian).
- The original file is never modified or overwritten.
- Page numbers typed by the user are **1-based positions in the current grid** across all loaded files.
- Fully offline: no network calls, no telemetry. Errors go to stderr only.
- Release profile: `opt-level = 3`, `lto = true`, `codegen-units = 1`, `strip = true`; panics unwind; no
  `target-cpu=native`.
- eframe with `default-features = false` and the `glow` renderer (OpenGL 2+ on ~2015 integrated GPUs). Keep
  `accesskit` for accessibility.
- pdfium-render with `default-features = false, features = ["pdfium_8125"]`. PDFium binaries pinned to `chromium/8125`.
- Performance targets (manual, ~2015 laptop, 300-page ~50 MB PDF): startup < 1 s, first thumbnails < 1 s,
  memory < 200 MB, idle CPU ~0%.
- Thumbnails: ~150 logical px wide × display scale factor; cache cap 300 thumbnails.
- Commits end with the attribution lines required by the session.

## Decisions made in this plan (flagged for owner review)

The spec is silent on these. The plan picks the option below; the owner can change any of them before execution.

1. **Reversed range `3-1`**: rejected as an error ("invalid range 3-1"), not read as 3, 2, 1.
2. **Empty items `1,,2` and trailing commas**: ignored. Whitespace anywhere is ignored.
3. **Duplicates `1, 1`**: allowed (the page is used twice), consistent with adding a file twice.
4. **Split never overwrites an existing file in the chosen folder**: a clashing name gets ` (2)`, ` (3)`, … before
   `.pdf`. Duplicate names within one split are made unique the same way.
5. **Language choice is not remembered** between runs in v0.1: it is auto-detected at every start and can be
   switched for the session.
6. **Split naming stem**: the file stem of the source document of the **first page in the grid**.

## Review Focus

1. **Split producing the same name twice** (`1, 1`, or a file already in the folder): must never overwrite. Test
   in Task 3 (`split_names_are_unique_and_never_clash`).
2. **Save destination equal to a loaded source, spelled differently** (other case on Windows/macOS, `./`, symlink):
   must be blocked. Test in Task 3 (`source_path_detection_uses_canonical_paths`).
3. **Save failure mid-way** (unwritable folder): no `.tmp` file and no partial PDF left. Test in Task 5
   (`failed_save_leaves_no_files`).
4. **Wrong password then correct password** for the same file: the second attempt must open it. Test in Task 5
   (`encrypted_pdf_wrong_then_right_password`).
5. **Fast scrolling**: stale thumbnail requests must be dropped, not rendered. Test in Task 5
   (`visible_set_replaces_pending_renders`).

---

## File Structure

```
Cargo.toml                 crate metadata, deps, release profile
LICENSE-MIT, LICENSE-APACHE
README.md                  usage, build, unsigned-build warnings
scripts/fetch-pdfium.sh    download pinned PDFium for a platform into a folder (CI + local dev)
src/main.rs                bin: calls adiutor_pdf::run()
src/lib.rs                 module declarations + run()
src/ranges.rs              page range parser
src/workspace.rs           page list, selection, dirty flag, split names, source-path check
src/i18n.rs                language enum, string tables, lookup
src/engine.rs              worker thread owning PDFium; Command / Event types
src/pdfium_lib.rs          locating and binding the PDFium library
src/thumb_cache.rs         bounded LRU of thumbnail keys (pure logic, used by ui)
src/ui.rs                  eframe App
locales/en.txt, locales/it.txt
tests/engine.rs            integration tests with real PDFium
tests/fixtures/encrypted.pdf, tests/fixtures/make_encrypted.py
docs/manual-test-checklist.md
.github/workflows/ci.yml, .github/workflows/release.yml
packaging/macos/Info.plist, packaging/linux/adiutor-pdf.desktop, packaging/linux/adiutor-pdf.png
```

---

### Task 1: Project skeleton, licenses, PDFium fetch script, CI

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/lib.rs`, `LICENSE-MIT`, `LICENSE-APACHE`, `scripts/fetch-pdfium.sh`,
  `.github/workflows/ci.yml`, `.gitignore`
- Modify: `README.md`

**Interfaces:**
- Produces: crate `adiutor_pdf` with `pub fn run() -> eframe::Result<()>` (stub opening an empty window titled
  `adiutor-pdf`). `scripts/fetch-pdfium.sh <platform> <dest-dir>` where `<platform>` is one of `linux-x64`,
  `win-x64`, `mac-x64`, `mac-arm64`; afterwards the library is at `<dest-dir>/lib/libpdfium.so`,
  `<dest-dir>/bin/pdfium.dll` or `<dest-dir>/lib/libpdfium.dylib` respectively, and its license files are in
  `<dest-dir>/` (as in the upstream archive).

- [ ] **Step 1: Write `Cargo.toml`**

  `name = "adiutor-pdf"`, `version = "0.1.0"`, `edition = "2024"`, `rust-version = "1.95"`,
  `license = "MIT OR Apache-2.0"`, `repository = "https://github.com/sidussoftware/adiutor-pdf"`.
  Dependencies:
  - `eframe = { version = "0.37", default-features = false, features = ["glow", "accesskit", "default_fonts", "wayland", "x11"] }`
  - `pdfium-render = { version = "0.9", default-features = false, features = ["pdfium_8125"] }`
  - `rfd = "0.18"`
  - `sys-locale = "0.3"`

  Dev-dependency: `tempfile = "3"`. Release profile as in Global Constraints.

- [ ] **Step 2: Write `src/main.rs`, `src/lib.rs` stub, licenses, `.gitignore` (`/target`, `/pdfium`)**

  `main.rs` calls `adiutor_pdf::run()`; on Windows add `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`.
  Licenses: standard MIT text (copyright "2026 adiutor-pdf contributors") and the Apache-2.0 text.

- [ ] **Step 3: Write `scripts/fetch-pdfium.sh`**

  Downloads `https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8125/pdfium-<platform>.tgz`
  and extracts it into `<dest-dir>`. `set -euo pipefail`; fails with a usage message on a wrong platform.

- [ ] **Step 4: Verify locally**

  Run: `scripts/fetch-pdfium.sh linux-x64 pdfium && ls pdfium/lib/libpdfium.so && cargo build && cargo clippy -- -D warnings && cargo fmt --check`
  Expected: library listed; build, clippy and fmt pass with no warnings.

- [ ] **Step 5: Write `.github/workflows/ci.yml`**

  On `push` and `pull_request`. Matrix: `ubuntu-22.04` (platform `linux-x64`), `windows-latest` (`win-x64`),
  `macos-latest` (`mac-arm64`). Steps: checkout, `dtolnay/rust-toolchain@stable` with `rustfmt, clippy`,
  `Swatinem/rust-cache@v2`, on Linux `apt-get install` the GTK/X11/Wayland dev packages rfd and winit need
  (`libgtk-3-dev libxkbcommon-dev libwayland-dev`), `bash scripts/fetch-pdfium.sh <platform> pdfium`, set
  `ADIUTOR_PDFIUM_DIR` to the folder holding the library (`pdfium/lib` or `pdfium/bin` on Windows), then
  `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

- [ ] **Step 6: README skeleton**

  Title, one-line description, "Status: in development", License section (dual MIT/Apache-2.0), Development section
  with the fetch script and `ADIUTOR_PDFIUM_DIR`.

- [ ] **Step 7: Commit** — `chore: project skeleton, licenses and CI`

---

### Task 2: Page range parser (`ranges`)

**Files:**
- Create: `src/ranges.rs` (tests in the same file, `#[cfg(test)] mod tests`)
- Modify: `src/lib.rs` (`pub mod ranges;`)

**Interfaces:**
- Produces:
  ```rust
  pub enum RangeError {
      InvalidFormat { item: String },          // "a", "1-", "1-2-3"
      ZeroPage,                                // "0"
      OutOfRange { page: usize, total: usize },// "12" with total 10
      Reversed { start: usize, end: usize },   // "3-1"
  }
  /// Returns one group per comma-separated item, as 0-based grid positions.
  /// Empty or whitespace-only input returns Ok(vec![]).
  pub fn parse(input: &str, total: usize) -> Result<Vec<Vec<usize>>, RangeError>;
  ```
  Derives `Debug, Clone, PartialEq, Eq`.

- [ ] **Step 1: Write the failing tests**

  ```rust
  assert_eq!(parse("1-3, 5", 10), Ok(vec![vec![0,1,2], vec![4]]));
  assert_eq!(parse("  7 ", 10), Ok(vec![vec![6]]));
  assert_eq!(parse("1,,2,", 10), Ok(vec![vec![0], vec![1]]));
  assert_eq!(parse("1, 1", 10), Ok(vec![vec![0], vec![0]]));
  assert_eq!(parse("", 10), Ok(vec![]));
  assert_eq!(parse(" 1 - 3 ", 10), Ok(vec![vec![0,1,2]]));
  assert_eq!(parse("3-1", 10), Err(RangeError::Reversed { start: 3, end: 1 }));
  assert_eq!(parse("0", 10), Err(RangeError::ZeroPage));
  assert_eq!(parse("12", 10), Err(RangeError::OutOfRange { page: 12, total: 10 }));
  assert_eq!(parse("5-12", 10), Err(RangeError::OutOfRange { page: 12, total: 10 }));
  assert_eq!(parse("a", 10), Err(RangeError::InvalidFormat { item: "a".into() }));
  assert_eq!(parse("1-2-3", 10), Err(RangeError::InvalidFormat { item: "1-2-3".into() }));
  assert_eq!(parse("1", 0), Err(RangeError::OutOfRange { page: 1, total: 0 }));
  ```
  Test names: `parses_groups`, `ignores_whitespace_and_empty_items`, `allows_duplicates`, `empty_input`,
  `rejects_reversed`, `rejects_zero`, `rejects_out_of_range`, `rejects_bad_format`.

- [ ] **Step 2: Run** `cargo test ranges` — Expected: FAIL (not implemented).
- [ ] **Step 3: Implement `parse`** — split on `,`, trim, skip empty; an item is `N` or `N-M` (trim around `-`); parse
  with `usize::from_str`; the first error found (left to right) is returned.
- [ ] **Step 4: Run** `cargo test ranges` — Expected: PASS.
- [ ] **Step 5: Commit** — `feat: page range parser`

---

### Task 3: Workspace model (`workspace`)

**Files:**
- Create: `src/workspace.rs` (tests in the same file)
- Modify: `src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  pub type DocId = usize;
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub struct PageRef { pub doc: DocId, pub index: u32 }       // index: 0-based page in its source
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub struct SlotId(u64);                                      // stable id of one grid cell
  #[derive(Clone, Debug)]
  pub struct SourceDoc { pub id: DocId, pub path: PathBuf, pub page_sizes: Vec<(f32, f32)> } // points (w, h)
  pub struct Slot { pub id: SlotId, pub page: PageRef }

  impl Workspace {
      pub fn new() -> Self;
      pub fn add_document(&mut self, doc: SourceDoc);          // appends all its pages; marks dirty
      pub fn doc(&self, id: DocId) -> Option<&SourceDoc>;
      pub fn slots(&self) -> &[Slot];
      pub fn len(&self) -> usize;
      pub fn is_empty(&self) -> bool;
      pub fn page_size(&self, slot: &Slot) -> (f32, f32);
      // selection (by grid position)
      pub fn is_selected(&self, pos: usize) -> bool;
      pub fn select_only(&mut self, pos: usize);               // plain click
      pub fn toggle(&mut self, pos: usize);                    // Ctrl/Cmd+click
      pub fn select_range_to(&mut self, pos: usize);           // Shift+click, from last clicked position
      pub fn set_selection(&mut self, positions: &[usize]);    // from the Pages field
      pub fn clear_selection(&mut self);
      pub fn selected_positions(&self) -> Vec<usize>;          // ascending
      // edits
      pub fn move_selected_to(&mut self, target: usize);       // insert before grid position `target` (len = end); keeps relative order; marks dirty
      pub fn delete_selected(&mut self);                       // marks dirty, clears selection
      // saving helpers
      pub fn pages_at(&self, positions: &[usize]) -> Vec<PageRef>;
      pub fn all_pages(&self) -> Vec<PageRef>;
      pub fn is_dirty(&self) -> bool;
      pub fn mark_saved(&mut self);
      pub fn is_source_path(&self, dest: &Path) -> bool;
  }
  /// File names for Split, one per group (groups are 0-based positions from `ranges::parse`),
  /// unique among themselves and against `existing` (names already in the target folder).
  pub fn split_names(stem: &str, groups: &[Vec<usize>], existing: &HashSet<String>) -> Vec<String>;
  pub fn split_stem(ws: &Workspace) -> Option<String>;          // file stem of the first grid page's source
  ```
  Selection is stored as a set of `SlotId`, so it survives reordering.

- [ ] **Step 1: Write the failing tests**

  Helper `ws_with(&[3, 2])` builds a workspace with doc 0 of 3 pages and doc 1 of 2 pages.
  - `add_document_appends_pages`: slots' pages are `[(0,0),(0,1),(0,2),(1,0),(1,1)]`; `is_dirty()`.
  - `move_selected_to_keeps_relative_order`: select positions 0 and 2, `move_selected_to(5)` → pages
    `[(0,1),(1,0),(1,1),(0,0),(0,2)]`; selected positions now `[3,4]`.
  - `move_selected_to_front`: select 4, `move_selected_to(0)` → first page `(1,1)`.
  - `delete_selected`: select 1 and 3, delete → pages `[(0,0),(0,2),(1,1)]`, selection empty.
  - `shift_click_selects_range`: `select_only(1)`, `select_range_to(3)` → `[1,2,3]`.
  - `toggle`: `select_only(0)`, `toggle(2)`, `toggle(0)` → `[2]`.
  - `mark_saved_clears_dirty`: after `mark_saved()`, `!is_dirty()`; a move makes it dirty again.
  - `split_names_basic`: stem `"doc"`, groups `[[0],[1,2,3]]`, empty existing → `["doc_p1.pdf", "doc_p2-4.pdf"]`.
  - `split_names_are_unique_and_never_clash`: groups `[[0],[0]]`, existing `{"doc_p1.pdf"}` →
    `["doc_p1 (2).pdf", "doc_p1 (3).pdf"]`.
  - `source_path_detection_uses_canonical_paths`: with a real temp file `a.pdf` as a source,
    `is_source_path(dir.join(".").join("a.pdf"))` is `true`; `dir.join("b.pdf")` (not existing) is `false`.

- [ ] **Step 2: Run** `cargo test workspace` — Expected: FAIL.
- [ ] **Step 3: Implement** — `is_source_path` compares `std::fs::canonicalize` of both sides when `dest` exists;
  when it does not exist, canonicalize its parent and join the file name; on Windows and macOS compare
  case-insensitively. `split_names` uses 1-based numbers: `{stem}_p{a}.pdf` or `{stem}_p{a}-{b}.pdf`
  (`a`, `b` = first and last position of the group + 1).
- [ ] **Step 4: Run** `cargo test workspace` — Expected: PASS.
- [ ] **Step 5: Commit** — `feat: workspace page list, selection and split naming`

---

### Task 4: Translations (`i18n`)

**Files:**
- Create: `src/i18n.rs` (tests in the same file), `locales/en.txt`, `locales/it.txt`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Lang { En, It }
  impl Lang {
      pub const ALL: [Lang; 2];
      pub fn native_name(self) -> &'static str;               // "English", "Italiano"
      pub fn from_locale(locale: Option<&str>) -> Lang;       // "it", "it-IT", "it_IT.UTF-8" → It; else En
      pub fn detect() -> Lang;                                // from sys_locale::get_locale()
  }
  pub struct Tr { /* current lang table + English fallback */ }
  impl Tr {
      pub fn new(lang: Lang) -> Self;
      pub fn lang(&self) -> Lang;
      pub fn t(&self, key: &str) -> &str;                     // missing in lang → English; missing everywhere → key
      pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String; // replaces {name} placeholders
  }
  ```
  File format: one `key = value` per line, `#` starts a comment line, blank lines ignored, `\n` in a value means a
  newline. Files are embedded with `include_str!`.
  Keys (all UI copy lives here; Tasks 6–8 use only these keys, adding new ones to both files when needed):
  `app.title`, `toolbar.add_pdf`, `toolbar.pages`, `toolbar.select`, `toolbar.language`,
  `action.save_all`, `action.save_selection`, `action.split`, `grid.empty_hint`,
  `range.invalid_format` ({item}), `range.zero_page`, `range.out_of_range` ({page}, {total}),
  `range.reversed` ({start}, {end}),
  `open.failed` ({file}, {reason}), `password.title`, `password.prompt` ({file}), `password.wrong`,
  `password.ok`, `common.cancel`, `common.close`,
  `save.blocked_original`, `save.failed` ({file}, {reason}), `save.done` ({file}), `save.progress` ({done}, {total}),
  `split.done` ({count}, {folder}),
  `close.unsaved_title`, `close.unsaved_body`, `close.confirm`,
  `error.internal`.

- [ ] **Step 1: Write the failing tests**
  - `from_locale_detects_italian`: `"it"`, `"it-IT"`, `"it_IT.UTF-8"` → `It`; `"en-US"`, `"fr"`, `None` → `En`.
  - `every_english_key_exists_in_italian`: parse both files; the key sets are equal.
  - `missing_key_falls_back`: `Tr::new(Lang::It).t("does.not.exist") == "does.not.exist"`.
  - `tf_replaces_placeholders`: `Tr::new(Lang::En).tf("range.out_of_range", &[("page","12"),("total","10")])`
    equals `"Page 12 does not exist (total 10)"`.
- [ ] **Step 2: Run** `cargo test i18n` — Expected: FAIL.
- [ ] **Step 3: Implement** `i18n.rs` and write both locale files with every key above.
  English copy for the spec-fixed messages: `range.out_of_range = Page {page} does not exist (total {total})`,
  `save.blocked_original = Choose another name: the original file is never overwritten`,
  `close.unsaved_body = There are unsaved changes. Close anyway?`,
  `error.internal = Internal error, please restart the app`,
  `open.failed = Cannot open {file}: {reason}`. Italian: natural translations of the same.
- [ ] **Step 4: Run** `cargo test i18n` — Expected: PASS.
- [ ] **Step 5: Commit** — `feat: English and Italian translations`

---

### Task 5: PDF engine worker (`pdfium_lib`, `engine`)

**Files:**
- Create: `src/pdfium_lib.rs`, `src/engine.rs` (unit tests for `runs` in the same file), `tests/engine.rs`,
  `tests/fixtures/make_encrypted.py`, `tests/fixtures/encrypted.pdf`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `workspace::{DocId, PageRef}`.
- Produces:
  ```rust
  // pdfium_lib.rs
  /// Search order: $ADIUTOR_PDFIUM_DIR, exe dir, exe_dir/../Frameworks (macOS bundle),
  /// exe_dir/../lib (AppImage), then the system library.
  pub fn bind() -> Result<pdfium_render::prelude::Pdfium, String>;

  // engine.rs
  pub enum Command {
      Open { path: PathBuf, password: Option<String> },
      /// Replaces all pending thumbnail requests (newest first wins).
      Visible { pages: Vec<PageRef>, width_px: u32 },
      Save { job: u64, pages: Vec<PageRef>, dest: PathBuf },
  }
  pub enum Event {
      Opened { path: PathBuf, doc: DocId, page_sizes: Vec<(f32, f32)> },
      PasswordRequired { path: PathBuf, wrong: bool },        // wrong = a password was given and rejected
      OpenFailed { path: PathBuf, reason: String },
      Thumbnail { page: PageRef, width: u32, height: u32, rgba: Vec<u8> },
      SaveProgress { job: u64, done: usize, total: usize },
      Saved { job: u64, dest: PathBuf },
      SaveFailed { job: u64, dest: PathBuf, reason: String },
  }
  pub struct EngineDead;
  pub struct Engine { /* Sender<Command>, Receiver<Event>, JoinHandle */ }
  impl Engine {
      /// Binds PDFium on the worker thread; returns Err(reason) if the library cannot be loaded.
      pub fn start(wake: impl Fn() + Send + 'static) -> Result<Engine, String>;
      pub fn send(&self, cmd: Command) -> Result<(), EngineDead>;
      pub fn try_recv(&self) -> Result<Option<Event>, EngineDead>; // Disconnected → EngineDead
  }
  /// Groups consecutive pages of the same source into (doc, first..=last) runs.
  pub fn runs(pages: &[PageRef]) -> Vec<(DocId, std::ops::RangeInclusive<u32>)>;
  ```
  `DocId`s are assigned by the engine in open order (0, 1, 2, …), only for successfully opened documents.

- [ ] **Step 1: Write the failing unit test for `runs`**

  `runs_groups_consecutive_pages`: pages `[(0,0),(0,1),(0,2),(1,0),(0,3),(0,5)]` →
  `[(0,0..=2),(1,0..=0),(0,3..=3),(0,5..=5)]`. `runs_empty`: `[]` → `[]`.

- [ ] **Step 2: Generate the encrypted fixture**

  `tests/fixtures/make_encrypted.py` (pypdf): writes a 2-page PDF (pages 200×300 and 300×200 points) encrypted with
  user password `secret` to `tests/fixtures/encrypted.pdf`. Run: `python3 tests/fixtures/make_encrypted.py`.
  Commit both files.

- [ ] **Step 3: Write the failing integration tests in `tests/engine.rs`**

  Helpers: `make_pdf(path, sizes: &[(f32,f32)])` creates a fixture with PDFium (via `pdfium_lib::bind()`,
  `create_new_pdf`, one blank page per size, `save_to_file`); `wait_for(engine, pred, timeout 10s) -> Event`.
  Tests are skipped with a printed notice if `pdfium_lib::bind()` fails and `CI` is unset; under `CI` they must run.
  - `open_reports_page_sizes`: a 3-page file of sizes (100,100),(200,100),(300,100) → `Opened` with those sizes.
  - `merge_preserves_order`: open A (100,100),(200,100) and B (300,100); save `[(B,0),(A,1),(A,0)]`; reopen output →
    sizes `[(300,100),(200,100),(100,100)]`; `SaveProgress` with `total == 3` was seen before `Saved`.
  - `extract_subset`: save `[(A,1)]` → one page of size (200,100).
  - `corrupted_file_fails`: a file containing `b"not a pdf"` → `OpenFailed`.
  - `encrypted_pdf_wrong_then_right_password`: open without password → `PasswordRequired { wrong: false }`; with
    `"nope"` → `PasswordRequired { wrong: true }`; with `"secret"` → `Opened` with 2 pages.
  - `failed_save_leaves_no_files`: save to `dir/missing_subdir/out.pdf` → `SaveFailed`; `dir` contains no
    `.tmp` file and no `out.pdf`.
  - `visible_set_replaces_pending_renders`: open a 50-page file; send `Visible` with all 50 pages, then
    immediately `Visible` with only page 49; within 10 s a `Thumbnail` for page 49 arrives, and fewer than 50
    `Thumbnail` events arrive in total.
  - `thumbnail_has_requested_width`: `Visible { [page 0 of a (200,100) page], width_px: 150 }` → `Thumbnail` with
    `width == 150`, `height == 75`, `rgba.len() == 150*75*4`.

- [ ] **Step 4: Run** `cargo test` — Expected: FAIL (engine not implemented).

- [ ] **Step 5: Implement `pdfium_lib::bind` and `Engine`**

  Worker loop: block on `recv` when idle; otherwise drain all pending commands with `try_recv` before each
  thumbnail render, so a new `Visible` replaces the queue. Commands other than `Visible` are handled in order.
  - Open: `load_pdf_from_file(path, password)`; map
    `PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError)` to `PasswordRequired`; other
    errors to `OpenFailed` with the error's `Display`/`Debug` text.
  - Render: `render_with_config(&PdfRenderConfig::new().set_target_width(width_px))`, `as_rgba_bytes()`.
  - Save: new document; for each run, `pages_mut().copy_page_range_from_document(src, run, dest_len)` then emit
    `SaveProgress`; `save_to_file` to `<dest_dir>/.<file_name>.adiutor-tmp`, then `std::fs::rename` to `dest`;
    on any error remove the temp file and emit `SaveFailed`.
  - After sending each event, call `wake()`.
  The worker owns `Pdfium` and `Vec<PdfDocument>` locally inside the thread function (documents borrow `Pdfium`).

- [ ] **Step 6: Run** `ADIUTOR_PDFIUM_DIR=$PWD/pdfium/lib cargo test` — Expected: all PASS.
- [ ] **Step 7: Commit** — `feat: PDF engine worker thread`

---

### Task 6: UI — window, opening files, thumbnail grid

**Files:**
- Create: `src/thumb_cache.rs` (tests in the same file), `src/ui.rs`, `docs/manual-test-checklist.md`
- Modify: `src/lib.rs` (`run()` builds the app)

**Interfaces:**
- Consumes: `Engine`, `Command`, `Event` (Task 5); `Workspace`, `SourceDoc`, `PageRef` (Task 3); `Tr`, `Lang` (Task 4).
- Produces:
  ```rust
  // thumb_cache.rs — pure LRU of keys; the UI keeps egui::TextureHandle values in it
  pub struct ThumbCache<V> { /* cap, map, recency */ }
  impl<V> ThumbCache<V> {
      pub fn new(cap: usize) -> Self;
      pub fn get(&mut self, key: &PageRef) -> Option<&V>;     // marks as recently used
      pub fn insert(&mut self, key: PageRef, value: V);       // evicts least recently used beyond cap
      pub fn contains(&self, key: &PageRef) -> bool;
      pub fn len(&self) -> usize;
  }
  // ui.rs
  pub struct AdiutorApp { /* engine, workspace, tr, cache, state */ }
  impl eframe::App for AdiutorApp { /* … */ }
  ```
  `run()`: `eframe::run_native` with `NativeOptions { renderer: Renderer::Glow, viewport: title from i18n,
  drag-and-drop enabled, inner size 1000×700 }`. If `Engine::start` fails, the window shows the reason and the
  location the app searched, instead of the grid.

- [ ] **Step 1: Write the failing tests for `ThumbCache`**
  `evicts_least_recently_used`: cap 2; insert a, b; `get(a)`; insert c → contains a and c, not b.
  `insert_existing_updates_without_growing`: cap 2; insert a twice → `len() == 1`.
- [ ] **Step 2: Run** `cargo test thumb_cache` — Expected: FAIL.
- [ ] **Step 3: Implement `ThumbCache`** (cap used by the UI: 300).
- [ ] **Step 4: Run** `cargo test thumb_cache` — Expected: PASS.
- [ ] **Step 5: Implement the window**
  - Top bar: `toolbar.add_pdf` (rfd multi-file picker, filter `*.pdf`) and files dropped on the window
    (`ctx.input(|i| i.raw.dropped_files)`) send `Command::Open`.
  - Event handling each frame: drain `try_recv`; `Opened` → `workspace.add_document`; `Thumbnail` → load as
    `egui::ColorImage::from_rgba_unmultiplied` texture into the cache; `EngineDead` → show `error.internal`.
  - Grid: wrapping layout inside a `ScrollArea`; each cell is a placeholder sized by the page aspect ratio until its
    texture is cached, with the 1-based position under it. Collect cells whose rect intersects the visible clip
    rect and that are not cached; if that set changed since the last frame, send `Command::Visible` with
    `width_px = (150.0 * ctx.pixels_per_point()).round()`.
  - Empty workspace shows `grid.empty_hint`.
  - Engine `wake` closure calls `ctx.request_repaint()`; no continuous repaint anywhere.
- [ ] **Step 6: Write `docs/manual-test-checklist.md`** with sections: Open, Thumbnails, Selection, Reorder/Delete,
  Save, Split, Password, Errors, Close, Language, Performance (the four targets from Global Constraints and how to
  measure them: stopwatch for startup and first thumbnails, Task Manager / Activity Monitor / `top` for memory and
  idle CPU, a generated 300-page PDF). Tasks 7 and 8 add their checks to it.
- [ ] **Step 7: Verify manually**: `cargo run --release`; drop 2 PDFs → all pages appear in order, thumbnails fill
  in while scrolling, idle CPU ~0% in `top`. Tick the Open and Thumbnails sections.
- [ ] **Step 8: Commit** — `feat: workspace window with thumbnail grid`

---

### Task 7: UI — selection, reorder, delete, Pages field

**Files:**
- Modify: `src/ui.rs`, `docs/manual-test-checklist.md`

**Interfaces:**
- Consumes: `Workspace` selection and edit methods (Task 3), `ranges::parse` and `RangeError` (Task 2),
  i18n keys `range.*`, `toolbar.pages`, `toolbar.select`.

- [ ] **Step 1: Implement**
  - Click → `select_only`; Ctrl (Cmd on macOS, i.e. `modifiers.command`) + click → `toggle`; Shift + click →
    `select_range_to`. Selected cells get a highlighted frame.
  - Drag a cell with `dnd_drag_source` / `dnd_drop_zone`: if the dragged cell is not selected, `select_only` it
    first, then `move_selected_to(drop position)`. Show an insertion marker between cells while dragging.
  - Delete or Backspace key (when the Pages field has no focus) → `delete_selected`. Evict nothing from the
    cache: `PageRef` keys stay valid.
  - Pages field + `toolbar.select` button (Enter in the field does the same): `ranges::parse(text, len)`; on
    success `set_selection(flattened groups)`; on error the field gets a red frame and the translated
    `range.*` message appears under it. The message clears when the text changes.
- [ ] **Step 2: Add checks** to the Selection and Reorder/Delete sections of the checklist (including: select
  pages 2 and 4, drag them to the start; type `11-12` with two 10-page files selects the first two pages of the
  second file; type `3-1` shows the reversed-range message).
- [ ] **Step 3: Verify manually** with `cargo run --release` and tick those sections.
- [ ] **Step 4: Run** `cargo clippy --all-targets -- -D warnings && cargo test` — Expected: PASS.
- [ ] **Step 5: Commit** — `feat: page selection, reordering and deletion`

---

### Task 8: UI — save, split, password, errors, close confirmation, language switch

**Files:**
- Modify: `src/ui.rs`, `docs/manual-test-checklist.md`

**Interfaces:**
- Consumes: `Command::Save`, `Event::{Saved, SaveFailed, SaveProgress, PasswordRequired, OpenFailed}` (Task 5);
  `Workspace::{all_pages, pages_at, selected_positions, is_source_path, mark_saved, is_dirty}`,
  `split_names`, `split_stem` (Task 3); `ranges::parse` (Task 2); `Tr`, `Lang` (Task 4).

- [ ] **Step 1: Implement**
  - `action.save_all` (disabled when empty) and `action.save_selection` (disabled when the selection is empty):
    rfd save dialog with default name `<split_stem>_edited.pdf`, filter `*.pdf`, appending `.pdf` if missing. If
    `is_source_path(dest)` → show `save.blocked_original` and do not send. Otherwise send `Save`.
  - `action.split` (disabled when empty): parse the Pages field; if it is empty, groups = one per page; on a parse
    error show it as in Task 7. Pick a folder with rfd; read the existing file names in it; `split_names`; send
    one `Save` per group. When all jobs of the split finish, show `split.done`.
  - While any job runs: progress bar from `SaveProgress` (`save.progress`), and the save/split buttons are
    disabled. `Saved` of a Save-all or Save-selection → `mark_saved()` and `save.done`. A split also calls
    `mark_saved()` once all its jobs succeed.
  - Messages (`open.failed`, `save.failed`, `save.done`, `split.done`, `save.blocked_original`) show in a
    dismissible bar at the bottom; several can stack. Also `eprintln!` errors.
  - `PasswordRequired` → modal with a password field (masked), `password.prompt`, `password.wrong` when
    `wrong`, OK → `Open { path, password }`, Cancel → skip the file. Several files waiting are queued, one modal
    at a time.
  - Close request (`ctx.input(|i| i.viewport().close_requested())`) with `is_dirty()` → send
    `ViewportCommand::CancelClose` and show a modal (`close.unsaved_title`, `close.unsaved_body`, `close.confirm`,
    `common.cancel`); confirm → `ViewportCommand::Close`.
  - Language combo box in the top bar (`toolbar.language`, items `Lang::native_name`) replaces `Tr` for the
    session. Startup language is `Lang::detect()`.
- [ ] **Step 2: Add checks** to the Save, Split, Password, Errors, Close and Language sections, including: save over
  an open source file is blocked; split `1, 1` twice into the same folder produces `_p1.pdf`, `_p1 (2).pdf`,
  `_p1 (3).pdf`, `_p1 (4).pdf`; saving into a read-only folder shows an error and leaves no `.adiutor-tmp` file;
  `tests/fixtures/encrypted.pdf` with a wrong then the right password (`secret`).
- [ ] **Step 3: Verify manually** with `cargo run --release` and tick those sections.
- [ ] **Step 4: Run** `cargo clippy --all-targets -- -D warnings && cargo test` — Expected: PASS.
- [ ] **Step 5: Commit** — `feat: saving, splitting, passwords and close confirmation`

---

### Task 9: Packaging, draft-release workflow, README

**Files:**
- Create: `.github/workflows/release.yml`, `packaging/macos/Info.plist`, `packaging/linux/adiutor-pdf.desktop`,
  `packaging/linux/adiutor-pdf.png` (256×256 placeholder icon: a plain document glyph, no third-party art)
- Modify: `README.md`

**Interfaces:**
- Consumes: `scripts/fetch-pdfium.sh` (Task 1), the PDFium search order of `pdfium_lib::bind` (Task 5).

- [ ] **Step 1: Write `release.yml`**
  - Trigger: `push` to `main`. `permissions: contents: write`.
  - Job `check`: read `version` from `Cargo.toml` (`cargo metadata --no-deps --format-version 1 | jq`); output
    `create=true` only if `gh release view v$VERSION` fails **and** `git ls-remote --tags origin v$VERSION` is
    empty.
  - Build jobs (only if `create`), all `cargo build --release`:
    - `windows-latest`: zip `adiutor-pdf-v$VERSION-windows-x64.zip` = `adiutor-pdf.exe`, `pdfium.dll`,
      `LICENSE-MIT`, `LICENSE-APACHE`, PDFium license files in `licenses/pdfium/`.
    - `macos-latest`: build targets `x86_64-apple-darwin` and `aarch64-apple-darwin`; `lipo -create` the two
      binaries; fetch `mac-x64` and `mac-arm64` PDFium and `lipo -create` the two `libpdfium.dylib`; bundle
      `adiutor-pdf.app/Contents/{MacOS/adiutor-pdf, Frameworks/libpdfium.dylib, Info.plist}`; verify with
      `lipo -info` that both have `x86_64 arm64`; zip `adiutor-pdf-v$VERSION-macos-universal.zip` with the
      `.app` and licenses. Ad-hoc sign (`codesign --force --deep -s -`) so Apple Silicon allows launching.
    - `ubuntu-22.04`: AppDir with `usr/bin/adiutor-pdf`, `usr/lib/libpdfium.so`, `.desktop`, icon, licenses;
      build with `appimagetool` (pinned release) → `adiutor-pdf-v$VERSION-linux-x86_64.AppImage`.
    - Each build job uploads its package as an artifact.
  - Job `release`: download the artifacts and run
    `gh release create v$VERSION --draft --target $GITHUB_SHA --title "adiutor-pdf v$VERSION" --generate-notes <files>`.
- [ ] **Step 2: Write `Info.plist`** (`CFBundleIdentifier` `io.github.sidussoftware.adiutor-pdf`,
  `CFBundleExecutable` `adiutor-pdf`, `CFBundleShortVersionString` set from `Cargo.toml` by the workflow with
  `plutil -replace`, `LSMinimumSystemVersion` `10.13`, `NSHighResolutionCapable` true) and the `.desktop` file.
- [ ] **Step 3: Finish `README.md`**: what it does, download from Releases, first launch of unsigned builds
  (Windows: SmartScreen "More info → Run anyway"; macOS: right-click → Open, or System Settings → Privacy &
  Security → Open Anyway; Linux: `chmod +x` the AppImage), usage summary, page-number syntax, build from source,
  how to make a release (bump `version` in `Cargo.toml`, merge to `main`, review the draft, Publish), license.
- [ ] **Step 4: Verify**: `actionlint` on both workflows (download the pinned binary into the scratchpad if not
  installed). Then, with the owner's approval, push the branch and check that `ci.yml` is green on all three
  OSes. The release workflow is verified on the first merge to `main` (draft only; nothing is public until the
  owner publishes it). Download the three packages from the draft and launch each where possible; record the
  results in the checklist.
- [ ] **Step 5: Commit** — `ci: draft release workflow and packaging`

---

## Self-review notes

- Spec coverage: §2 scope → Tasks 2–8; §3 UX → Tasks 6–8; §4 modules → Tasks 2–6; §5 data flow → Tasks 5–6;
  §6 errors → Tasks 5, 7, 8; §7 performance → Global Constraints + Task 6 checklist; §8 testing → Tasks 2–6;
  §9 CI/release → Tasks 1 and 9; §10 layout → File Structure (adds `pdfium_lib.rs`, `thumb_cache.rs`,
  `scripts/`, `packaging/`, which the spec did not list).
- Known platform limit: on Linux and macOS a source file deleted after opening can often still be read through the
  open handle, so saving may succeed instead of failing. The output is still correct; no partial file is possible.
