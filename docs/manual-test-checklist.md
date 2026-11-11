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
## Split
## Password
## Errors
- [ ] Opening a corrupt file shows a message line at the bottom

## Close
## Language
- [ ] With `LANG=it_IT.UTF-8` the UI is Italian

## Performance
Generate a 300-page PDF: `python3 -c "from pypdf import PdfWriter as W; w=W(); [w.add_blank_page(595,842) for _ in range(300)]; w.write('300.pdf')"`.
- [ ] Startup to window: stopwatch from launch to first frame
- [ ] First thumbnails visible after opening the 300-page PDF: stopwatch
- [ ] Memory with the 300-page PDF loaded: `top` RES / Task Manager / Activity Monitor
- [x] Idle CPU after loading: `top -b -n 3 -d 1 -p <pid>` showed 0.0% (Xvfb, 12 + 300 pages loaded)
