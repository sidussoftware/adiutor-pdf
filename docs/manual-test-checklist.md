# Manual test checklist

Tick an item only after doing it on a real build. Tasks 7 and 8 add their checks to the empty sections.

## Open
- [x] Pass two PDFs as command-line arguments: all pages appear in order, positions numbered from 1 (Xvfb, release build, screenshot)
- [ ] "Add PDF" button opens a multi-file picker filtered to `*.pdf`
- [ ] Dropping two PDFs on the window adds all their pages in order
- [ ] Empty workspace shows the hint; engine start failure shows the reason and searched locations

## Thumbnails
- [x] Thumbnails fill in for the visible pages after load (Xvfb screenshot)
- [ ] Thumbnails fill in while scrolling a 300-page PDF
- [ ] Mixed page sizes (portrait and landscape) keep their aspect ratio

## Selection
- [x] Click selects one page; Ctrl+click toggles; Shift+click selects the range from the anchor (Xvfb, xdotool, screenshots)
- [x] Selected pages show a thick frame plus a check badge (not color alone)
- [x] Type `11-12` with two 10-page files and press Enter: the first two pages of the second file are selected (Xvfb; confirmed by Backspace removing B1 and B2)
- [x] Type `3-1`: field gets a red frame and "Invalid range: 3 is greater than 1" appears; editing the text clears it
- [ ] Italian messages for the range errors with `LANG=it_IT.UTF-8`

## Reorder/Delete
- [x] Select pages 2 and 4, drag them to the start: they land first in original order; insertion marker shown while dragging (Xvfb)
- [x] Delete removes the selected pages; Backspace inside the Pages field edits text and deletes no page
- [ ] Drag to the end of the list and between rows; drag an unselected page (it becomes the selection first)
## Save
- [x] Reorder, then "Save all as one PDF": the output has the pages in the grid order (pypdf page widths [300, 700, 400, 500, 600]) (Xvfb + xdg-desktop-portal-gtk, xdotool, screenshots)
- [x] "Save selection" writes only the selected page; default name is `<first source>_edited.pdf` (Xvfb + xdg-desktop-portal-gtk, xdotool, screenshots)
- [x] Saving over an open source file shows "Choose another name..." and leaves the file unchanged (Xvfb + xdg-desktop-portal-gtk, xdotool, screenshots)
- [x] Messages stack at the bottom and each Close button dismisses its own line
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
- [ ] macOS: with unsaved changes, Apple menu → Log Out: the modal appears and Cancel stops the logout
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
