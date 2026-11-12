# adiutor-pdf

A small, fully offline desktop app for PDF page operations: merge several PDFs, reorder and delete pages, and save
the result as one file or split it into pieces. Original files are never overwritten. English and Italian UI.

## Download

Get the package for your system from the [Releases](../../releases) page:

| System  | File                                    |
|---------|-----------------------------------------|
| Windows | `adiutor-pdf-vX.Y.Z-windows-x64.zip`    |
| macOS   | `adiutor-pdf-vX.Y.Z-macos-universal.zip` (Intel and Apple Silicon) |
| Linux   | `adiutor-pdf-vX.Y.Z-linux-x86_64.tar.gz` |

### First launch (builds are not code-signed)

- **Windows:** unzip, run `adiutor-pdf.exe`. If SmartScreen appears: **More info -> Run anyway**.
- **macOS:** unzip, then right-click `adiutor-pdf.app` -> **Open** -> Open. Or: System Settings -> Privacy &
  Security -> **Open Anyway**.
- **Linux:** extract the archive (`tar -xzf adiutor-pdf-*.tar.gz`), then run `./adiutor-pdf` in the extracted
  folder.

### Linux requirements

The archive bundles PDFium only. The system must provide:

- OpenGL 2 or newer
- `libxkbcommon-x11` (X11 sessions)
- an XDG desktop portal for the file dialogs, for example `xdg-desktop-portal-gtk`; without one, `zenity` is
  used instead

### macOS notes

- Choosing Quit from the Dock, or logging out with the app open, closes it **without** the
  unsaved-changes prompt (a limitation of winit 0.30, the windowing library). Cmd+Q and the window's close button
  do show the prompt.
- The default macOS app menu (Hide, About, ...) is intentionally turned off, so that Cmd+Q reaches the app and
  goes through the unsaved-changes prompt.

## Usage

1. **Add PDF** (or drop files on the window, or pass them as arguments). All pages appear in one grid, numbered
   from 1. Password-protected files ask for the password.
2. Click a page to select it; Ctrl+click (Cmd+click on macOS) toggles; Shift+click selects a range. Or type page
   numbers in **Pages** and press Enter or **Select**.
3. Drag selected pages to reorder them; press Delete or Backspace to remove them.
4. **Save all as one PDF** writes the grid in its current order. **Save selection** writes only the selected pages.
   **Split** writes one file per page-number group (see below) into a folder you choose. Closing the window with
   **Close anyway** during a split keeps the files already written (each one complete) and stops the rest.

### Page numbers

Positions in the current grid, starting at 1, comma-separated: `1-3, 5` selects pages 1, 2, 3 and 5. With
**Split**, each comma-separated item becomes its own file (`1-3, 5` gives two files); an empty field gives one
file per page.

## Build from source

Needs Rust 1.95+. On Linux also `libxkbcommon-dev libwayland-dev`. PDFium is loaded at runtime;
fetch a prebuilt copy (platform: `linux-x64`, `win-x64`, `mac-x64`, `mac-arm64`):

```sh
scripts/fetch-pdfium.sh linux-x64 pdfium
export ADIUTOR_PDFIUM_DIR="$PWD/pdfium/lib"   # pdfium/bin on Windows
cargo run --release
```

`ADIUTOR_PDFIUM_DIR` names the folder holding the PDFium library. Without it the app looks next to the
executable, in `../Frameworks` (macOS app bundle), then in the system library path.

## Making a release

1. Bump `version` in `Cargo.toml` and merge to `main`.
2. The *Release* workflow builds all three packages and creates a **draft** release `vX.Y.Z` (only if that
   release or tag does not exist yet).
3. Review the draft on the Releases page, then **Publish**. Publishing creates the tag.

## Licenses

adiutor-pdf is dual-licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

Release packages include the licenses of the third-party software they contain, in the `licenses/` folder:

- Rust crates compiled into the program: `licenses/THIRD-PARTY-LICENSES.html`. The Rust standard library is
  dual-licensed under MIT or Apache-2.0.
- PDFium and the libraries bundled with it: `licenses/pdfium/`.
- Fonts embedded by egui: `licenses/fonts/` — Ubuntu Light (Ubuntu Font Licence 1.0), Hack (MIT and Bitstream
  Vera License), Noto Emoji (SIL Open Font License 1.1) and emoji-icon-font (MIT).

The Linux build contains code generated from KDE Plasma Wayland protocol definitions, some of which are licensed
under LGPL-2.1-or-later (through the crate `wayland-protocols-plasma`, used by the windowing library). The Linux
package includes the license, a notice and the complete `wayland-protocols-plasma` crate with these protocol files
in `licenses/lgpl/`. The complete source of adiutor-pdf is in this repository and attached to each release. To
rebuild with modified protocol files, follow "Build from source" and point a `[patch.crates-io]` entry in
`Cargo.toml` at the modified crate folder (see `licenses/lgpl/NOTICE.txt`).

This software is based in part on the work of the Independent JPEG Group.

Portions of this software are copyright © 2026 The FreeType Project (www.freetype.org). All rights reserved.
