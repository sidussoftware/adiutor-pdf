# UI redesign (direction B) and persistent settings — design

Date: 2026-11-13. Status: awaiting owner approval (builder check done). Size: project.
Prototypes: the owner's Design canvas "adiutor-pdf UI prototypes", boards "B · Barra laterale documenti",
"B · tema scuro" and "B · nessun documento aperto".

## Intent
After the v0.1.3 fixes the owner found the overall look "to be revisited". It reads as egui's default developer look.
This change gives adiutor-pdf a calmer, more finished interface that also makes clear which file each page comes
from, which helps when merging. It must still run on ~2015 hardware.

## Goals (owner decisions)
1. **Layout B.**
   - A fixed left sidebar with the documents.
   - A top bar over the grid with the page field and the save actions.
   - The thumbnail grid.
   - Notifications in the top-right corner.
2. **The sidebar is view-only in this version.** Actions on documents are tracked in
   sidussoftware/adiutor-pdf#6.
3. **Theme choice:** Sistema / Chiaro / Scuro (System / Light / Dark), in the sidebar next to the language.
4. **Language choice:** Sistema / Italiano / English. "System" follows the OS language, as today at start-up.
5. **Language and theme are remembered between runs**, in a settings structure designed so that more settings can be
   added later.
6. **Inter as the UI font**, embedded. Icons are our own vector drawings in code, with no icon library.

## Non-goals
- Sidebar actions: select a document's pages, remove a document, reorder documents (#6).
- A resizable or collapsible sidebar, a remembered window size, true fullscreen, undo.
- An icon font or icon library.

## Anti-goals (this would be a failure even if it "works")
- Selection, document identity or button state shown by colour alone (documents carry letters, see below).
- Continuous repainting: idle CPU must stay about 0%.
- A settings file problem that blocks the app or loses the user's work.
- Breaking any v0.1.x guarantee: originals never overwritten, split never overwrites, no partial output, the
  unsaved-changes prompt.

## Constraints
- egui/eframe 0.37 with the glow renderer, Rust 1.95, offline.
- New runtime crates are allowed only where stated below. cargo-about must still pass, which it does automatically
  for MIT/Apache crates.
- All UI text goes through i18n with English and Italian key/placeholder parity.
- Accessibility:
  - real egui widgets, so AccessKit exposes them;
  - text contrast ≥ 4.5:1, and UI elements (borders, accents, document swatches) ≥ 3:1, in both themes, enforced by
    unit tests;
  - controls at least 40 px tall.
- Licences: Inter (SIL OFL 1.1) ships with its licence text in every package under `licenses/fonts/`, next to
  egui's fonts, and the README Licenses section names it.

## Approaches considered
- **Direction**:
  - A: a clean toolbar with a status bar;
  - **B: a document sidebar (chosen)**;
  - C: dark and compact, with a floating action bar.
  - B was picked because it shows where each page comes from.
- **Font and icons**:
  - **Inter plus our own vector icons (chosen)**: no extra dependency and the closest match to the prototype;
  - Inter plus an icon font such as Lucide: one more dependency and licence;
  - egui's defaults: nothing added, but the look stays technical.
- **Settings storage**:
  - **our own `settings.rs` with a TOML file (chosen)**: GUI-independent, testable, saves at once, easy to extend;
  - eframe's built-in persistence: two new crates, tied to the GUI, saves only at exit or on an interval;
  - a hand-written key=value file: no crates, but every new setting needs new parsing code.

## Design

### Window layout
```
┌ sidebar 264 ┬ top bar 68 ───────────────────────────────────────────────┐
│ adiutor-pdf │ Pagine [es. 1-3, 5] [Seleziona]   [Dividi][Salva selezione][Salva PDF] │
│ [+ Aggiungi]├────────────────────────────────────────────────────────────┤
│ DOCUMENTI   │  thumbnail grid (v0.1.3 selection look + document colour strip)  ┌toasts┐ │
│ ■ a.pdf 8 p │                                                                    └──────┘ │
│ ■ b.pdf 4 p │                                                                            │
│   …         │                                                                            │
│ Lingua [ ▾] │                                                                            │
│ Tema   [ ▾] │                                                                            │
└─────────────┴────────────────────────────────────────────────────────────┘
```
- **Sidebar:** `egui::Panel::left`, fixed 264 px, not resizable. From the top:
  - the app name, "adiutor-pdf";
  - the **Aggiungi PDF** button (opens the file picker, as now; files can still be dropped anywhere in the window);
  - the "DOCUMENTI" heading;
  - one row per open document: a colour swatch, the file name and "N pagine";
  - "Nessun documento" when there are none;
  - at the bottom, the **Lingua** and **Tema** combo boxes.
  - Long file names are truncated with an ellipsis, and the full name shows on hover.
  - The list scrolls if it gets long.
- **Top bar** (over the central area):
  - left: "Pagine" label, field and Seleziona;
  - right: Dividi, Salva selezione and **Salva PDF** (the primary action, filled with the accent colour);
  - the existing enable/disable rules are kept.
  - "Save all as one PDF" is renamed **Save PDF / Salva PDF**.
- **The bottom bar is removed.** Messages become toasts, and progress moves into the toast area (below).

### Documents in the sidebar
- Documents are listed in the order they were opened.
- **Every document gets a letter (owner decision)** in opening order: A, B, … Z, then AA, AB, … (spreadsheet
  style). The letter appears in a small badge in front of the colour swatch in the sidebar row, and in a small tag on
  the colour strip of each of that document's thumbnails. So the document is identifiable without colour, and with any
  number of documents.
- Hovering a thumbnail shows "<file name>, page N", where N is the page number in its source document.
- Two open files with the same name are told apart in the sidebar by adding their parent folder:
  "contratto.pdf — Documenti/2025".
- "N pagine" counts that document's pages **currently in the grid**, so it changes after deletes and moves. Singular
  and plural forms are translated ("1 pagina" / "N pagine", "1 page" / "N pages").
- A document whose pages were all deleted stays listed with "0 pagine". It is still open, and still protected as an
  original.

### Document colours
- A fixed palette of 8 colours per theme, assigned by `DocId % 8`. Two documents only share a colour from the ninth
  document on.
- Each colour appears in two places: the sidebar swatch, and a 4 px strip along the top edge of each of that
  document's thumbnails.
- Colours are chosen to stay distinguishable for common colour-vision deficiencies, with lightness spread as well as
  hue (an Okabe–Ito-like set). The document letter and the file name are the identity; the colour is an extra cue.
- The prototype's first two colours are the starting point: light `#2563EB` and `#EA580C`, dark `#60A5FA` and
  `#FB923C`.
- Tests check each colour against the sidebar background and against the page (white) at ≥ 3:1, in both themes.

### Theme and visual style
- Two hand-tuned `egui::Visuals` (light and dark), built from one palette module.
  - Light theme: the stone neutrals and teal accent of prototype B.
  - Dark theme: warm anthracite with a lighter teal accent, as on the dark board.
- Prototype values, the starting point (the tests decide where these fail):

  | Role | Light | Dark |
  |---|---|---|
  | Window / grid background | `#FAFAF9` | `#1A1918` |
  | Sidebar background | `#F5F5F4` | `#222120` |
  | Top bar background | `#FFFFFF` | `#1F1E1D` |
  | Card / field background | `#FFFFFF` | `#2A2826` |
  | Border | `#D6D3D1` / `#E7E5E4` | `#57534E` / `#3A3836` |
  | Text | `#1C1917` | `#F5F5F4` |
  | Secondary text | `#57534E` | `#A8A29E` |
  | Accent (primary button, selection) | `#0F766E` | `#2DD4BF` |
  | On-accent text | `#FFFFFF` | `#042F2E` |
  | Error toast | — | `#3F1D1D` background, `#FECACA` text |

  For an error toast in the light theme, the builder picks a red pair that passes the tests.
- Minimum window size 1024×600 (`with_min_inner_size`), so the 264 px sidebar plus the top bar's controls always
  fit. The ~2015 target laptop (1366×768) is above it.
- Applied through `ctx.set_theme(ThemePreference::{System, Light, Dark})` together with per-theme visuals, so
  "System" follows the OS live.
- Style values used everywhere:
  - corner radius 8 for buttons and fields, 4 for pages;
  - 40 px controls;
  - 16 px panel padding.
- The v0.1.3 selection (frame, number pill, badge) uses the theme's accent from this palette.
- `selection_accent` / `on_accent` are replaced by the palette's accent and on-accent colours. The contrast tests move
  with them and now also cover body text, secondary text and borders.
- Page thumbnails stay white in both themes, because they are the documents' own content.

### Font and icons
- Inter is embedded with `include_bytes!` and set as the first proportional family. egui's default fonts stay as
  fallbacks for missing glyphs and emoji.
  - Two static weights, Regular and SemiBold, from the official Inter release: a pinned version, its TTF files and
    OFL.txt committed under `assets/fonts/inter/`, with recorded sha256.
  - Use Inter 4.1 from github.com/rsms/inter releases (the zip's static TTFs). It is fetched once at development
    time, like PDFium; the app itself stays offline.
  - The packages get the font licence through `.github/workflows/release.yml`, which already copies
    `licenses/fonts/`. Add Inter's OFL.txt there as `Inter-OFL.txt`.
  - SemiBold is a named family used for the app name, headings and the primary button.
  - This adds roughly 0.6–0.8 MB to the binary.
- `src/icons.rs` holds small functions that draw stroke icons with the egui painter: plus, split (scissors), save
  (download arrow), close (×), check, and file-with-arrow for the empty state.
  - Each takes a rect and a colour and is decorative; its button always has a text label.

### Notifications (toasts)
- They stack in the top-right corner of the central area, newest on top. Nothing is ever dropped. When the stack is
  taller than half the central area, it becomes a scrollable list of that height. Errors are also logged to stderr.
- Success messages ("Salvato …", "Creati N file …") disappear after 5 s. Errors stay until closed.
- Every toast has a close button ("Chiudi messaggio" for screen readers).
- Each message keeps its `(key, args)` form, so it is translated when drawn and follows a language switch.
- Auto-dismiss schedules one `request_repaint_after` for the next expiry, never a repaint every frame.
- While a save or split runs, a **progress card** with "Salvataggio X / Y" and a bar sits on top of the stack instead
  of a toast.

### Empty state
- With no documents, the central area shows a large dashed panel:
  - the file icon;
  - "Trascina qui i PDF", then "oppure";
  - a primary **Aggiungi PDF** button;
  - the hint "Puoi unire, dividere, riordinare ed eliminare pagine. I file originali non vengono mai modificati."
- The top-bar controls are disabled (existing rules).

### Settings (`src/settings.rs`)
- `Settings`: a plain struct with `#[serde(default)]` and one field per setting. Today:
  - `language: LanguagePref`, one of `System | English | Italian`, serialised as `"system" | "en" | "it"`;
  - `theme: ThemePref`, one of `System | Light | Dark`, serialised as `"system" | "light" | "dark"`.
  - `Default` gives `System` for both.
- **Adding a setting** means adding a field and its default, plus a control in the UI. Old files still load, because
  missing fields take their defaults and unknown fields are ignored.
- `fn load_from(path: &Path) -> Settings`:
  - a missing file gives the defaults;
  - an unreadable or invalid file also gives the defaults, plus one `eprintln!`, and the file is left untouched.
- `fn save_to(path: &Path, s: &Settings) -> std::io::Result<()>`: creates the folder, writes a temp file in it, then
  renames it over `settings.toml`.
- If the file failed to parse at load, the first save renames it to `settings.toml.bak` before writing, so a
  hand-edited file is never silently lost.
- `fn config_path() -> Option<PathBuf>` resolves the location without any new crate:
  - Windows: `%APPDATA%\adiutor-pdf\settings.toml`;
  - macOS: `$HOME/Library/Application Support/adiutor-pdf/settings.toml`;
  - Linux and other Unix: `$XDG_CONFIG_HOME/adiutor-pdf/settings.toml`, else `$HOME/.config/adiutor-pdf/settings.toml`.
  - `None` means settings are not saved; the app still works.
- **When the app uses it:**
  - it loads before the window is created, so the first frame has the right language and theme;
  - it saves right after the user changes a setting;
  - a save failure shows one error toast ("Impossibile salvare le impostazioni: …") and the app keeps running.
- `LanguagePref::System` resolves through the existing `Lang::detect()`.
- New runtime crates: `serde` (already in the binary, now a direct dependency with `derive`) and `toml`. `toml`, with
  its small helper crates, is new to the binary; all are MIT/Apache, and the generated notices pick them up
  automatically.

### i18n
- New keys, English and Italian:
  - sidebar heading, empty documents label, page-count singular and plural;
  - language and theme labels, plus the System / Light / Dark and System / English / Italiano option labels;
  - the empty-state strings;
  - the settings-save error;
  - the progress card label.
- `action.save_all` becomes "Save PDF" / "Salva PDF". `toolbar.language` moves to the sidebar's label key.

### Version
- 0.2.0, because this is a visible redesign. This is my call, not an owner decision.

## Testing
- Unit tests for `settings`:
  - save, then load, round-trips;
  - a file with missing fields gives the defaults;
  - unknown fields are ignored;
  - a corrupt file gives the defaults and the file is unchanged;
  - after a save no temp file is left;
  - `config_path` per OS, through injected environment values (the env lookup goes through a helper that tests can
    drive).
- Palette tests, for both themes:
  - text against the background ≥ 4.5:1;
  - secondary text ≥ 4.5:1;
  - accent against the background ≥ 3:1;
  - on-accent text ≥ 4.5:1;
  - borders ≥ 3:1;
  - each document colour against the sidebar and against white ≥ 3:1.
- i18n: the existing key and placeholder parity tests cover the new keys, plus a test for the page-count
  singular/plural choice.
- kittest:
  - the sidebar lists two documents with their letters and counts;
  - the empty state shows the drop panel;
  - the theme combo switches the visuals.
- Manual, under Xvfb, with screenshots compared to the three prototype boards:
  - light, dark and empty;
  - toasts auto-dismiss;
  - the progress card;
  - the settings file written and reloaded after a restart.
- Manual on real machines (owner): the checklist gains sidebar, theme, persistence and font checks.

## Calls made by the designer (owner may revise)
- Version 0.2.0.
- Palette starting values as above.
- Inter 4.1.
- Minimum window size 1024×600.
- The toast stack never drops anything and scrolls past half the central area.
- A corrupt settings file is backed up as `.bak`.
- The thumbnail hover tooltip shows "<file name>, page N".
- Same-name files are disambiguated by their parent folder.
- Letters run in spreadsheet style.

## Left to the builder
- Exact palette values, within the contrast tests and close to the prototype.
- Icon geometry, toast and card sizes, and spacing details.
- How the panels are split across files. `ui.rs` is about 900 lines, so it is a good moment to split it, for example
  into `ui/sidebar.rs`, `ui/topbar.rs`, `ui/grid.rs`, `ui/toasts.rs` and `ui/modals.rs`, without changing behaviour.
