# Manual test checklist

Tick an item only after doing it on a real build.

## Open
- [x] Pass two PDFs as command-line arguments: all pages appear in order, positions numbered from 1 (Xvfb, release build, screenshot)
- [ ] "Add PDF" button opens a multi-file picker filtered to `*.pdf`
- [ ] Dropping two PDFs on the window adds all their pages in order
- [ ] Empty workspace shows the hint; engine start failure shows the reason and searched locations
- [ ] The window opens maximized (needs a window manager; not observable under bare Xvfb)

## Thumbnails
- [x] Thumbnails fill in for the visible pages after load (Xvfb screenshot)
- [ ] Thumbnails fill in while scrolling a 300-page PDF
- [ ] Mixed page sizes (portrait and landscape) keep their aspect ratio

## Selection
- [x] Click selects one page; Ctrl+click toggles; Shift+click selects the range from the anchor (Xvfb, xdotool, screenshots)
- [x] Selected pages show a thick frame plus a check badge (not color alone)
- [x] Ctrl+A (Cmd+A on macOS) selects all pages; inside the Pages field it selects the field's text and no page (Xvfb, xdotool, screenshots)
- [x] Pages 1 and 2 selected, light and dark themes: rounded frame around the page only, number in a pill, check badge on the frame's top-right corner; the first row and column are not clipped (Xvfb screenshots)
- [x] Type `11-12` with two 10-page files and press Enter: the first two pages of the second file are selected (Xvfb; confirmed by Backspace removing B1 and B2)
- [x] Type `3-1`: field gets a red frame and "Invalid range: 3 is greater than 1" appears; editing the text clears it
- [ ] Italian messages for the range errors with `LANG=it_IT.UTF-8`

## Reorder/Delete
- [x] Select pages 2 and 4, drag them to the start: they land first in original order; insertion marker shown while dragging (Xvfb)
- [x] Delete removes the selected pages; Backspace inside the Pages field edits text and deletes no page
- [ ] Drag to the end of the list and between rows; drag an unselected page (it becomes the selection first)
## Save
- [x] Reorder, then "Save PDF": the output has the pages in the grid order (pypdf page widths [300, 700, 400, 500, 600]) (Xvfb + xdg-desktop-portal-gtk, xdotool, screenshots)
- [x] "Save selection" writes only the selected page; default name is `<first source>_edited.pdf` (Xvfb + xdg-desktop-portal-gtk, xdotool, screenshots)
- [x] Saving over an open original shows the blocked modal; "Choose another name" reopens the save dialog; the original is unchanged (Xvfb + xdg-desktop-portal-gtk, xdotool, sha256)
- [ ] Toasts stack top-right, each on one row with a compact × that dismisses only that toast (also by keyboard)
- [ ] Progress bar visible while saving a large file; Save/Split disabled meanwhile
## Split
- [x] Empty Pages field splits into one file per page (`A_p1.pdf` ... `A_p5.pdf`) (Xvfb + xdg-desktop-portal-gtk, xdotool, screenshots)
- [x] Split `1, 1` twice into the same folder gives `_p1.pdf`, `_p1 (2).pdf`, `_p1 (3).pdf`, `_p1 (4).pdf` (Xvfb + xdg-desktop-portal-gtk, xdotool, screenshots)
- [x] Split reports "Created N files in <folder>"
## Password
- [x] `tests/fixtures/encrypted.pdf`: wrong password shows "Wrong password, try again" (field masked), then `secret` opens it (Xvfb + xdg-desktop-portal-gtk, xdotool, screenshots)
- [ ] Cancel skips the file; several encrypted files queue one modal at a time
## Errors
- [x] Saving into a read-only folder shows "Cannot save ..." and leaves no `.adiutor-tmp` file (`chattr +i` folder, running as root)
- [x] Opening a corrupt file shows a message line at the bottom (Xvfb screenshot)

## Close
- [x] Delete a page, then close the window: "Unsaved changes" modal; Cancel keeps the app open; "Close anyway" exits (WM_DELETE_WINDOW sent with python-xlib)
- [ ] After saving, closing does not ask
- [ ] macOS: with unsaved changes, Cmd+Q shows the "Unsaved changes" modal
- [ ] macOS: with unsaved changes, Dock icon → Quit shows the modal; Cancel keeps the app open, "Close anyway" exits
- [ ] macOS: same with the app minimized, then hidden (Cmd+H is off, use the Dock menu "Hide"): the app comes forward or its Dock icon bounces
- [ ] macOS: with unsaved changes, Apple menu → Log Out: the modal appears; Cancel stops the logout (macOS may say the app interrupted it); note what macOS shows
- [ ] Minimize the window with unsaved changes, then close it from the taskbar / Dock / window menu (Windows, Linux, macOS): the modal appears when the window comes back
- [ ] macOS: without unsaved changes, Dock → Quit exits immediately
## Language
- [x] Language combo switches the whole UI between English and Italiano immediately (Xvfb screenshot)
- [ ] With `LANG=it_IT.UTF-8` the UI is Italian

## Performance
Generate a 300-page PDF: `python3 -c "from pypdf import PdfWriter as W; w=W(); [w.add_blank_page(595,842) for _ in range(300)]; w.write('300.pdf')"`.
- [ ] Startup to window: stopwatch from launch to first frame
- [ ] First thumbnails visible after opening the 300-page PDF: stopwatch
- [ ] Memory with the 300-page PDF loaded: `top` RES / Task Manager / Activity Monitor
- [x] Idle CPU after loading: `top -b -n 3 -d 1 -p <pid>` showed 0.0% (Xvfb, 12 + 300 pages loaded)

## Sidebar and identity
- [ ] Sidebar (264 px) lists each document with letter (A, B, ... AA after Z), colour, name and page count
- [ ] **Clear all** (sidebar) is disabled with no documents or while saving; with unsaved changes it asks first (Cancel and Esc keep everything); after clearing, the empty state shows, and newly added files get letter A again with their own thumbnails
- [ ] Cursor and text are vertically centred in the Pages and password fields
- [ ] Hovering a button changes its fill and border clearly; Save PDF turns a darker green; the pointer becomes a hand
- [ ] Thumbnails show the colour strip, letter tag and a 1 px border; hover shows "<file>, page N"
- [ ] With 27+ documents the letters continue AA, AB; colours repeat but letters stay unique
- [ ] Document identity is readable without colour (letters and names), also in both themes

## Themes
- [ ] Theme control switches System / Light / Dark immediately; text and borders stay legible in both
- [ ] System follows the OS light/dark setting

## Remembered settings
- [ ] Choose Italiano and Dark, quit, relaunch: both are restored
- [ ] `settings.toml` is in the per-user config folder (Windows `%APPDATA%\adiutor-pdf`, macOS `~/Library/Application Support/adiutor-pdf`, Linux `$XDG_CONFIG_HOME` or `~/.config`)
- [ ] Corrupt `settings.toml`: app starts with defaults; after changing a setting a `settings.toml.bak` copy exists
- [ ] Read-only config folder: app still works

## Notices and progress
- [ ] Success notice appears top right and disappears after about 5 s; error notices stay until closed
- [ ] Progress card shows while saving a large file; Save PDF / Split are disabled meanwhile
- [ ] Idle CPU stays about 0% with a notice shown and after it expires

## Empty state and window size
- [ ] With no documents the drop panel is shown; dropping PDFs on it adds them
- [ ] The window cannot be resized below 1024x600; the top bar and sidebar stay usable at that size
- [ ] Top-bar controls are at least 40 px tall and reachable by keyboard (Tab)

## Fonts
- [ ] Inter renders for text; Italian accents and emoji in file names fall back to egui's fonts without tofu boxes

## Screen reader
- [ ] Toast × is announced as "Close message"; Language and Theme combos are named; buttons are named without leading spaces (e.g. "Save PDF")
