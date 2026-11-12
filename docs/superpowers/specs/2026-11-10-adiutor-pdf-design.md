# adiutor-pdf — Design (v0.1)

Date: 2026-11-10
Status: approved by owner on 2026-11-10

## 1. Goal

A lightweight, offline, open-source desktop app for common PDF page operations.
Primary user: the owner; published publicly for anyone to download and run locally.
No web/hosted version.

### Decisions summary

| Topic | Decision |
|---|---|
| Form | Desktop GUI app |
| Platforms | Windows, macOS, Linux |
| Minimum hardware | ~2015 laptop: 64-bit, Windows 10 era, integrated GPU (OpenGL 2+) |
| Language | Rust |
| Priorities | Lightweight, offline, fast; optimization is essential |
| License | MIT OR Apache-2.0 (dual) |
| UI languages | English and Italian; architecture allows adding more |
| GUI toolkit | egui (eframe) |
| PDF engine | PDFium via `pdfium-render` (rendering and editing) |
| Distribution | Portable, unsigned builds on GitHub Releases; installers and code signing later |
| Docs and code comments | English |

## 2. Scope

### In scope (v0.1)

- **Merge**: add several PDFs, save all pages as one PDF.
- **Split / extract**: save a selection as one PDF, or split into one file per page or per range.
- **Reorder / delete** pages.
- Page selection both with **thumbnails** (mouse, drag & drop) and by **typing page numbers** (`1-3, 5, 8`).
- Opening **password-protected PDFs** (the app asks for the password; output is saved unprotected).
- UI language: auto-detected from the system, switchable in the app (English, Italian).

### Out of scope (v0.1)

- Preserving bookmarks / outline when merging or extracting.
- Rotation, compression, conversion (images ↔ PDF), watermarks, encryption, page numbers, OCR.
- Overwriting the original file.
- Installers, package managers (winget, Homebrew, Flathub), code signing.
- ARM builds for Windows and Linux.
- Automated GUI tests.

## 3. User experience

A single workspace window:

```
┌───────────────────────────────────────────────────┐
│ [+ Add PDF]   Pages: [1-3, 5      ] [Select]       │
├───────────────────────────────────────────────────┤
│ ▢1  ▢2  ▢3  ▢4  ▢5  ▢6  ...        (thumbnails)    │
│ drag to reorder, Delete key to remove              │
├───────────────────────────────────────────────────┤
│ [Save all as one PDF] [Save selection] [Split]     │
└───────────────────────────────────────────────────┘
```

- PDFs are added by dragging them onto the window or with **Add PDF**. All pages of all added files appear in one grid, in order.
- Every operation is the same flow: build the page list in the grid, then save.
  - Merge = add several files, **Save all as one PDF**.
  - Extract = select pages, **Save selection**.
  - Split = **Split** uses the Pages field: each comma-separated group becomes one file (`1-3, 4-6, 7` gives
    3 files). If the field is empty, every page becomes its own file. Output goes to a chosen folder.
  - Reorder / delete = edit the grid.
- **Page numbers refer to the current position in the grid** (1..N across all loaded files), not to the original page numbers in each file. Reordering the grid renumbers it.
- Adding the same file twice is allowed; its pages appear twice.

### Saving

- Always to a **new file** chosen with a native save dialog. The original file is never modified.
- Split writes to a chosen folder with automatic names based on the first source file name, for example
  `document_p1.pdf`, `document_p2-4.pdf`.

## 4. Architecture

One Rust crate (binary), split into modules with one job each.

```
┌──────────── UI thread (egui) ─────────────────────┐
│  ui        window, thumbnail grid, drag & drop,   │
│            command bar, dialogs                   │
│  workspace state: ordered page list + selection   │
│            (pure Rust, no GUI or PDFium deps)     │
│  ranges    parses "1-3, 5, 8"                     │
│  i18n      English / Italian strings              │
└──────────────┬───────────────▲────────────────────┘
      commands │               │ results
               ▼               │
┌──────────── PDF worker thread ────────────────────┐
│  engine    sole owner of PDFium: open (with       │
│            password), render thumbnails, save /   │
│            merge / split                          │
└───────────────────────────────────────────────────┘
```

### Modules

- **`workspace`**: the core logic. Holds a list of page references (`source document id`, `page index`) in the
  current order, plus the selection. Reorder, delete and select are operations on this list only: instant, no file
  I/O. Also generates the automatic file names for Split. No dependency on egui or PDFium.
- **`ranges`**: parses and validates page range input against the current page count, returning the page
  positions grouped per comma-separated item (one group = one output file for Split), or an error describing the
  problem.
- **`engine`**: runs on a dedicated worker thread and is the only code that touches PDFium (PDFium is not
  thread-safe). Receives commands over a channel, sends results back, and wakes the UI with `request_repaint`.
- **`ui`**: draws state and sends commands. Holds no business logic.
- **`i18n`**: minimal in-house system. One key/value text file per language (`locales/en.txt`, `locales/it.txt`)
  embedded in the binary with `include_str!`. Adding a language = adding a file and registering it.
  English is the fallback for missing keys.

### Dependencies

All MIT/Apache-2.0 or compatible:

- `eframe` / `egui`: GUI (glow / OpenGL backend for older integrated GPUs).
- `pdfium-render`: PDFium bindings. PDFium itself is BSD/Apache licensed and shipped as a native library.
- `rfd`: native open / save / folder dialogs.
- `sys-locale`: system language detection.

## 5. Data flow

### Opening a PDF

1. A file is dropped or picked, and the UI sends `Open { path }` to the worker.
2. The worker loads the document. PDFium reads the structure, not every page. It returns the page count and page
   sizes. If the file is encrypted, it returns `PasswordRequired` and the UI shows a password dialog, then retries
   with `Open { path, password }`.
3. The workspace appends page references, and the grid immediately shows placeholders with the correct aspect
   ratio.

### Thumbnails

- Only **visible** pages are requested. The UI sends the currently visible set. The worker drops queued
  requests that are no longer visible, so fast scrolling does not build a backlog.
- Rendered at a fixed width (about 150 logical px, scaled by the display scale factor) and uploaded as egui
  textures (about 130 KB each).
- **Bounded cache** of about 300 thumbnails (about 40 MB). Thumbnails far from the viewport are evicted and
  re-rendered on demand.
- The UI repaints only on input or when a thumbnail or result arrives, so idle CPU is about 0%.

### Saving

1. The UI sends `Save { pages, destination }`. Split sends one `Save` job per output file.
2. The worker creates a new document and imports pages in order, grouping consecutive pages from the same source
   into a single import call for speed.
3. It writes to a **temporary file in the destination folder**, then renames it to the final name, so no partial
   or corrupt PDF is left behind on failure.
4. Progress is reported to the UI and shown as a progress bar.

## 6. Error handling

| Situation | Behavior |
|---|---|
| File is not a PDF or is corrupted | Message "Cannot open `x.pdf`: <reason>"; other files in the same drop still load |
| Encrypted PDF | Password dialog; wrong password asks again; Cancel skips that file |
| Invalid page input | Field turns red with the reason (for example "page 12 does not exist (total 10)", "invalid format") |
| Empty selection | **Save selection** is disabled (**Split** with an empty Pages field splits every page) |
| Save destination is one of the open source files | Save is blocked: "Choose another name; the original is never overwritten" |
| Disk full / no permission | Message with the cause; temp file removed; workspace unchanged so the user can retry |
| Source file moved or deleted after opening | Save fails with a clear message; no partial output |
| Closing with unsaved changes | Confirmation "There are unsaved changes, close anyway?" |
| Worker thread crashes | UI shows "Internal error, please restart the app" instead of hanging |

Errors are written to stderr only. The app is fully offline and sends no telemetry.

"Unsaved changes" means the page list was changed (pages added, reordered or deleted) since the last successful
save.

## 7. Performance targets

Measured manually on a ~2015 laptop with a 300-page, ~50 MB PDF. Typical expected documents have up to a few
hundred pages and tens of MB.

| Metric | Target |
|---|---|
| App startup | < 1 s |
| First visible thumbnails after opening | < 1 s |
| Memory use | < 200 MB |
| Idle CPU | ~0% |

Release profile: `opt-level = 3`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic` left as `unwind`
(needed to detect a crashed worker thread). No `target-cpu=native`: binaries target generic x86-64 so they run on
any CPU of that era.

## 8. Testing

- **`ranges`**: unit tests for valid and invalid input (`"1-3, 5"`, `"3-1"`, `"0"`, out-of-range pages,
  whitespace, repeated commas).
- **`workspace`**: unit tests for reorder, delete, selection, multi-page moves, and Split file naming.
- **`engine`**: integration tests with the real PDFium library on small fixture PDFs committed under
  `tests/fixtures/` (a few KB each): merge, extract, split, page order (fixtures use pages of distinct sizes so
  order can be verified), a password-protected PDF, and a corrupted file.
- **GUI**: manual checklist in `docs/manual-test-checklist.md`, including the performance targets above.

## 9. Build, CI and release

### Continuous integration (`.github/workflows/ci.yml`)

On every push and pull request, on Windows, macOS and Linux:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`, with PDFium downloaded from `bblanchon/pdfium-binaries` at a **pinned** version.
- `cargo about generate` (Linux only): fails when a dependency's license is not accepted in `about.toml`.

### Release (`.github/workflows/release.yml`)

- Triggered on push to `main`.
- Reads `version` from `Cargo.toml`. If no release or tag `v<version>` exists yet, it builds all packages and
  creates a **draft GitHub Release** `v<version>` with the packages attached. If it already exists, it does nothing.
- The owner reviews the draft and publishes it manually. The tag `v<version>` is created on publish.
- Packages, each including the project licenses and the third-party notices (Rust crates via cargo-about, PDFium,
  fonts) in `licenses/`:
  - Windows x86-64: `.zip` with the `.exe` and `pdfium.dll`.
  - macOS **universal** (x86-64 + Apple Silicon): `.zip` with the `.app` bundle containing the universal PDFium
    library.
  - Linux x86-64: `.tar.gz` with one folder holding the executable and `libpdfium.so`, built on the oldest
    available Ubuntu runner for broad glibc compatibility.
    Changed in v0.1.2: Linux ships as .tar.gz (the AppImage runtime embeds LGPL libfuse).
    It also carries `licenses/lgpl/` (LGPL-2.1 text, NOTICE.txt, the protocol XML files) for the
    LGPL-2.1-or-later KDE protocol definitions that winit's Wayland backend compiles in via `wayland-protocols-plasma`.
- Builds are unsigned. The README explains how to get past the Windows SmartScreen and macOS Gatekeeper warnings.

## 10. Repository layout

```
Cargo.toml
about.toml, about.hbs   (cargo-about: third-party notices)
src/
  main.rs
  ui.rs
  workspace.rs
  ranges.rs
  engine.rs
  i18n.rs
locales/
  en.txt
  it.txt
tests/
  fixtures/
  engine.rs
docs/
  manual-test-checklist.md
  superpowers/specs/
.github/workflows/
  ci.yml
  release.yml
LICENSE-MIT
LICENSE-APACHE
README.md
```

## 11. Future work (not v0.1)

Bookmarks preservation, rotation, compression, image ↔ PDF conversion, installers and package managers, code
signing, ARM builds for Windows and Linux, more UI languages.
