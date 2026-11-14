# Remove a document, undo and redo — design

Status: draft for owner approval. Part of issue sidussoftware/adiutor-pdf#6. The other two actions in #6 stay
open there: selecting a document's pages by clicking it, and reordering documents by dragging.

## Intent

The owner merges several PDFs and notices that one of them is wrong. Today the only ways out are deleting its pages
one selection at a time, or **Clear all**, which starts over. The owner wants to remove that one document in one
click. Because a click can be a mistake, and because Ctrl+A followed by Delete already wipes the grid with no way
back, every page edit should also be undoable.

## Goals

- Remove one document, with all its pages, from the sidebar in one click.
- Undo and redo four edits: adding a document (Add PDF, drop, command line), removing a document, deleting pages
  (Delete/Backspace), and moving pages by drag.
- Keep the app light on 2015 hardware: no noticeable memory or CPU cost, and idle CPU stays at about 0%.

## Non-goals

- Undo for Clear all, or for selection changes (click, Ctrl/Shift+click, Ctrl+A, Pages + Select).
- The other #6 actions: click a document to select its pages, and drag to reorder documents.
- Undo that survives a restart.

## Anti-goals

- Undo that silently changes what a running save writes.
- Letters or colours of the remaining documents changing when one is removed.
- A remove or undo that needs the file to be read again from disk.

## Decisions (owner)

1. **Remove control.** Each sidebar document row has an always-visible **×** button on its right.
   - Its accessible name and hover tooltip are "Remove <display name>" / "Togli <display name>".
   - It is an icon-only button with the 40×40 hit area, like the toast close button.
2. **No confirmation.** A click removes the document's pages from the grid at once. A success toast then says
   "Removed contratto.pdf (8 pages). Ctrl+Z to undo" / "Tolto contratto.pdf (8 pagine). Ctrl+Z per annullare".
   - On macOS the toast says Cmd+Z.
   - Singular and plural page forms follow the existing `sidebar.pages_one` / `pages_many` convention.
3. **Letters stay.** The other documents keep their letters and colours. If A is removed, B stays B and C stays C.
   - A file added afterwards gets the next letter in opening order.
   - After Clear all, letters restart at A, as they do today.
4. **What undo covers.**
   - Undoable: add document, remove document, delete pages, move pages by drag.
   - Not undoable: selection changes, Clear all.
   - Undo goes back strictly one step at a time, in order. Example: remove A, add C, then Ctrl+Z takes C away, and a
     second Ctrl+Z brings A back.
   - The owner first chose to leave Add PDF out, then included it: otherwise undoing an older step would silently
     drop the pages of files added afterwards.
5. **Redo.**
   - Windows and Linux: Ctrl+Y or Ctrl+Shift+Z. macOS: Cmd+Shift+Z.
   - Any new undoable edit discards the redo history, as in every editor.
6. **History depth: 50 steps.** At 51 the oldest step is dropped.
7. **Buttons.** The top bar gets **↶** (undo) and **↷** (redo) as icon-only buttons.
   - They sit on the right, before Split.
   - Each has an accessible name and a hover tooltip that shows its shortcut: "Undo (Ctrl+Z)" / "Annulla (Ctrl+Z)"
     and "Redo (Ctrl+Y)" / "Ripristina (Ctrl+Y)", with Cmd on macOS.
   - Each is disabled when there is nothing to undo or redo.
   - This is a deliberate owner exception to the "buttons always have a text label" rule.
8. **Saved state.** If the user saves and then undoes back to exactly the saved state, the workspace is no longer
   "unsaved". Moving away from it again makes it unsaved. This governs the close and Clear all prompts.
9. **Pages field.** While the Pages text field has keyboard focus, Ctrl+Z / Ctrl+Y act on the field's text (egui's
   own text undo), not on the grid.
10. **During a save.** Undo and redo stay enabled. A save already sent to the engine writes the pages it was given,
    so undo cannot change that file.
    - The existing end-of-batch check (`Batch::finish` compares the saved pages with the current grid) decides
      whether to mark the workspace saved, as it does today for edits made during a save.
11. **Clear all** empties the undo and redo history.
12. **Selection after undo or redo.** The slots that the step brought back are selected, so the user sees what
    changed. For an undone delete or remove, those are the pages that came back. For an undone or redone move, they
    are the moved pages.
13. **A removed or un-added document stays open in the PDF engine** until Clear all or app exit. Undoing a removal therefore
    brings back its pages with their thumbnails, without reading the file again.

## Approaches considered

- **Snapshots (chosen by the owner).** Before each undoable edit, the workspace stores a copy of its page list (the
  slots), which documents are removed, and the selection. Undo restores the previous copy.
  - Little code and hard to get wrong. A future edit becomes undoable by taking a snapshot.
  - About 16 bytes per page per step: 1,000 pages × 50 steps is under 1 MB.
- **Operation log with inverses (rejected).** Each edit records how to reverse itself.
  - Less memory.
  - Every edit needs its own inverse, written and tested, and a bug there corrupts the grid. The memory saving does
    not matter at this app's sizes.

## Design

### Model (`src/workspace.rs`, pure, unit-tested)

- `SourceDoc` gains a `removed: bool`, false when added.
  - Documents are never deleted from `docs`, because `DocId` indexes both `docs` and the engine's documents.
  - Letters keep using the position in `docs` (removed documents included), so they never shift.
  - Colours keep using `DocId % 8`.
- `doc_summaries()` returns only documents that are not removed.
  - Display-name disambiguation considers only those documents too, so removing one of two `c.pdf` files makes the
    other plain `c.pdf` again.
- `add_document` becomes undoable: it pushes a snapshot like the other edits. Undoing it marks the document removed,
  and redo brings it back. Its letter stays reserved, so a file added after the undo gets the next letter.
- New `remove_document(id: DocId) -> usize`: drops every slot of that document, marks it removed, clears selection
  and anchor for those slots, and returns the number of pages removed. It is undoable.
- `is_source_path` keeps checking removed documents. Their files are still open in the engine, so saving over them
  must stay blocked as it is today.
- **History.** One snapshot holds the slot list, the `removed` flags and the selected slot ids.
  - Each undoable edit (`add_document`, `remove_document`, `delete_selected`, `move_selected_to`) pushes the pre-edit snapshot onto
    the undo stack, but only if it really changes something; the existing "no change, not dirty" rules apply. It
    then clears the redo stack.
  - The undo stack is capped at 50.
  - `undo()` / `redo()` swap the current state with the top of the other stack and return whether anything
    happened.
  - `can_undo()` / `can_redo()` feed the buttons.
  - `SlotId`s are kept in snapshots, so restored slots are the same cells (the thumbnail cache and selection key by
    them).
- **Saved state.** The workspace keeps a counter that changes on every edit, undo and redo, plus the counter value
  at the last save. `is_dirty()` is true when they differ.
  - `mark_saved()` records the current value.

### UI

- **Sidebar (`src/ui/sidebar.rs`):** a × icon button at the right of each document row. A click calls
  `remove_document` and pushes the toast.
- **Top bar (`src/ui/toolbar.rs`):** ↶ and ↷ icon buttons, built with `icons::icon_button` and placed before Split.
  - Two new icons go in `src/ui/icons.rs`, with the same stroke style as the others.
- **Shortcuts (`src/ui/mod.rs`, next to the existing Delete and Ctrl+A handling):**
  - Ctrl/Cmd+Z undoes.
  - Ctrl+Y, Ctrl/Cmd+Shift+Z redo.
  - Both use the same guards as Delete: no modal open, and the Pages field not focused (decision 9).
- **Clear all** also resets the history.
- **Locales (en/it, same keys and placeholders):**
  - `sidebar.remove` (`{file}`);
  - `toast.removed_one` / `toast.removed_many` (`{file}`, `{count}`, `{shortcut}`);
  - `action.undo` and `action.redo` (with `{shortcut}`, for the tooltip and accessible name).

## Constraints

- Rust 1.95, edition 2024, egui/eframe 0.37. No new dependencies.
- en/it locale parity tests must pass.
- Idle CPU stays at about 0%: no repaint requests are added.
- clippy `-D warnings` is clean on linux-gnu, apple-darwin and windows-msvc.

## Calls made by the designer (from the builder check)

- **Removing the last visible document** behaves like Ctrl+A followed by Delete:
  - the grid shows its empty state;
  - the workspace stays unsaved, so closing asks first;
  - Save, Save selection and Split follow the grid, as today.
- **Clear all after the last removal.** Clear all is enabled only while the sidebar lists at least one document, so
  it is disabled once the last one is removed. Undo can still bring documents back.
- **The × during a save** stays enabled, like undo and redo (decision 10). `Batch::finish` already handles edits
  made during a save.
- **Removing a document with no pages left in the grid** still counts as an edit: its row disappears and the step is
  undoable. The toast then says 0 pages, using the plural form.
- **Several files added at once** (a multi-file pick or a drop) are one undo step per file. The engine opens them
  one at a time and reports each separately.
- **Undoing an add shows no toast.** The sidebar row and the pages disappearing make the change visible.
- **Names in the toast and tooltip** are the display names shown just before the removal.

## Left to the builder

- Icon geometry for ↶ and ↷.
- Exact spacing of the × in the row.
- Internal names and struct layout of the history.
- Test structure: workspace unit tests for each decision above, and a kittest for the sidebar × accessible name and
  the disabled state of ↶ / ↷.

## Version

This ships as 0.3.0: new user-facing features.
