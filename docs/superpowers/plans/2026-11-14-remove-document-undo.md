# Remove Document, Undo and Redo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user remove one document from the sidebar, and undo and redo adding a document, removing a
document, deleting pages and moving pages.

**Architecture:**
- The undo history lives in the pure `Workspace` model as snapshots of its editable state: slots, removed flags,
  selection and a revision id.
- The UI only calls the new workspace methods:
  - a × on each sidebar row;
  - ↶ and ↷ in the top bar;
  - keyboard shortcuts next to the existing Delete and Ctrl+A handling.
- The PDF engine is unchanged: removed documents stay open in it.

**Tech Stack:** Rust 1.95, edition 2024, eframe/egui 0.37, egui_kittest (dev).

**Spec:** `docs/superpowers/specs/2026-11-14-remove-document-undo-design.md`

## Global Constraints

- No new dependencies.
- The en/it locale files keep identical keys and placeholders; the parity tests in `src/i18n.rs` must pass.
- Idle CPU stays at about 0%: add no repaint requests.
- `cargo fmt --check` passes.
- `cargo clippy --all-targets -- -D warnings` is clean with `--target` for each of x86_64-unknown-linux-gnu,
  x86_64-apple-darwin and x86_64-pc-windows-msvc.
- Docs and code comments are in English.
- History depth is 50 steps.
- Shortcuts:
  - Undo: Ctrl+Z, and Cmd+Z on macOS.
  - Redo: Ctrl+Y or Ctrl+Shift+Z on Windows/Linux, Cmd+Shift+Z on macOS.
- ↶ and ↷ are icon-only buttons with an accessible name and a tooltip. This is an owner exception to the
  "buttons always have a text label" rule.
- The × on a sidebar row is an icon-only button. Its accessible name is "Remove <display name>" / "Togli <display
  name>".
- Version: 0.3.0.

## Review Focus

1. **Undo while a save is running.** The file being written must not change, and the workspace must not be marked
   saved if the grid no longer matches. Test `undo_during_save_keeps_batch_check` (Task 1) pins the workspace side.
   The batch side is unchanged code.
2. **Undo across an add.** For example: remove A, add C, undo, undo. Each undo must go back exactly one step, and no
   page may disappear silently. Test `undo_steps_back_strictly_in_order` (Task 1).
3. **Ctrl+Z while typing in the Pages field** must edit the text, not the grid. Covered by the
   `!ctx.egui_wants_keyboard_input()` guard (Task 2), and checked under Xvfb.
4. **Letters after removing and undoing an add.** A later file must get the next letter, never a reused one. Test
   `letters_are_never_reused` (Task 1).
5. **Same-name files after a removal.** When one of two `c.pdf` files is removed, the other shows plain `c.pdf`.
   Test `display_names_ignore_removed_documents` (Task 1).

---

### Task 1: Workspace history, remove_document and saved state

**Files:**
- Modify: `src/workspace.rs` (model and tests)
- Modify: `src/ui/mod.rs`, only where `SourceDoc { .. }` is built on `Event::Opened`: add `removed: false`.

**Interfaces:**
- Produces:
  ```rust
  pub struct SourceDoc { pub id: DocId, pub path: PathBuf, pub page_sizes: Vec<(f32, f32)>, pub removed: bool }
  pub const HISTORY_LIMIT: usize = 50;
  impl Workspace {
      pub fn add_document(&mut self, doc: SourceDoc);          // now undoable
      pub fn remove_document(&mut self, id: DocId) -> usize;   // pages removed; undoable even when 0
      pub fn undo(&mut self) -> bool;                           // false when nothing to undo
      pub fn redo(&mut self) -> bool;
      pub fn can_undo(&self) -> bool;
      pub fn can_redo(&self) -> bool;
      // unchanged signatures: delete_selected, move_selected_to, is_dirty, mark_saved, doc_summaries
  }
  ```
- Design to implement:
  - A private `State { slots: Vec<Slot>, removed: Vec<bool>, selected: HashSet<SlotId>, rev: u64 }`.
    - `removed[i]` is the flag of `docs[i]`.
    - When a state is restored, documents whose index is past `removed.len()` are restored as removed.
  - Undo stack (at most `HISTORY_LIMIT`, dropping the oldest) and redo stack.
  - Every undoable edit that really changes something does three things: push the current state on the undo stack,
    clear the redo stack, and set `rev` to a fresh id from a counter.
    - No-op delete and move push nothing, as today.
    - `remove_document` of a document that is already removed, or does not exist, is a no-op.
  - `is_dirty()` is `saved_rev != Some(rev)`.
    - A new workspace starts with `saved_rev = Some(rev)`, so it is not dirty.
    - `mark_saved()` sets `saved_rev = Some(rev)`.
    - Undo and redo restore `rev`, so returning to the saved state makes the workspace clean.
  - Selection after `undo` or `redo`:
    - the slots present in the restored state but not in the state being left;
    - if there are none (a move, or undoing an add), the restored state's own `selected`;
    - the anchor is cleared.
  - `doc_summaries()` lists only documents that are not removed.
    - `display_names` runs over those documents only.
    - `letter` stays `doc_letter(position in docs)`, counting removed documents.
  - `is_source_path` still checks removed documents.

- [ ] **Step 1: Write the failing tests** in `src/workspace.rs` tests. Use the existing test helpers for building
  docs (check the current tests' helper names). Each test is listed with its assertions.
  - `remove_document_drops_pages_and_hides_row`:
    - add A (3 pages) and B (2 pages); `remove_document(0) == 3`;
    - `slots().len() == 2`;
    - `doc_summaries()` has one entry with letter `"B"`;
    - `can_undo()`.
  - `undo_restores_removed_document_and_selects_its_pages`:
    - after the remove above, `undo()` is true;
    - `slots().len() == 5` and A is listed again;
    - the 3 A pages are selected and the B pages are not.
  - `redo_reapplies_and_new_edit_clears_redo`:
    - remove, undo, `redo()` gives 2 slots;
    - undo again, then delete a selected page: `!can_redo()`.
  - `undo_steps_back_strictly_in_order`:
    - add A and B, remove A, add C;
    - first undo: C is hidden and A is still hidden;
    - second undo: A is back and C is still hidden;
    - `slots()` holds exactly A's and B's pages.
  - `letters_are_never_reused`:
    - add A and B, undo (B hidden), add C;
    - summaries show letters `["A", "C"]`.
  - `history_is_capped_at_50`:
    - add one 60-page document, then 51 single-page deletes;
    - exactly 50 `undo()` calls return true and the 51st returns false.
    - Note: the add is a step too; the cap drops the oldest steps.
  - `undo_to_saved_state_is_clean`:
    - add A, `mark_saved()`, select and delete a page: `is_dirty()`;
    - `undo()`: `!is_dirty()`;
    - `redo()`: `is_dirty()`.
  - `noop_edits_push_no_history`:
    - add A (one step), then run three no-ops: `delete_selected()` with an empty selection, a `move_selected_to`
      that changes nothing, and `remove_document(99)`;
    - the first `undo()` returns true (it undoes the add) and the second returns false.
  - `removing_document_with_no_pages_left_is_a_step`:
    - add A and B, delete all of A's pages, `remove_document(0) == 0`;
    - A is not listed;
    - `undo()` lists A again.
  - `undo_move_selects_moved_pages`:
    - add A (4 pages), select positions 0 and 1, `move_selected_to(4)`, `undo()`;
    - the original order is back and positions 0 and 1 are selected.
  - `display_names_ignore_removed_documents`:
    - add `/a/x/c.pdf` and `/b/y/c.pdf`, remove the first;
    - the remaining summary name is `"c.pdf"`.
  - `undo_during_save_keeps_batch_check`:
    - add A, record `all_pages()` as the "saving" pages, delete a page, `undo()`;
    - `all_pages()` equals the recorded pages again (the existing `Batch::finish` comparison relies on this).
  - Update any existing test that relied on `add_document` not being undoable, or on dirty being a plain flag. Keep
    every existing assertion's meaning.
- [ ] **Step 2: Run** `cargo test --lib workspace`. Expected: the new tests FAIL to compile, because
  `remove_document`, `undo` and the other new items are not defined yet.
- [ ] **Step 3: Implement** the model as described in the Interfaces block. Set `removed: false` where `ui/mod.rs`
  builds `SourceDoc`.
- [ ] **Step 4: Run** `cargo test`. Expected: every test passes; the lib test count is the old count plus 12.
- [ ] **Step 5: Run** fmt and clippy for the three targets. Expected: clean.
- [ ] **Step 6: Commit** `feat(workspace): undo/redo history, remove_document, saved-state tracking`.

### Task 2: Sidebar ×, undo/redo buttons and shortcuts, docs, version 0.3.0

**Files:**
- Modify:
  - `src/ui/sidebar.rs`: the × on each row;
  - `src/ui/icons.rs`: new `undo` and `redo` icons;
  - `src/ui/toolbar.rs`: ↶ and ↷;
  - `src/ui/mod.rs`: shortcuts and the toast;
  - `locales/en.txt`, `locales/it.txt`;
  - `README.md`, `docs/manual-test-checklist.md`, `Cargo.toml` / `Cargo.lock` (0.3.0).

**Interfaces:**
- Consumes (Task 1): `Workspace::{remove_document, undo, redo, can_undo, can_redo}`, `DocSummary`.
- Produces:
  ```rust
  // sidebar.rs: returns the id whose × was clicked
  pub(super) fn doc_list(ui: &mut egui::Ui, tr: &Tr, docs: &[DocSummary]) -> Option<DocId>;
  // icons.rs: same stroke style as the other icons
  pub fn undo(p: &Painter, rect: Rect, color: Color32);
  pub fn redo(p: &Painter, rect: Rect, color: Color32);
  // mod.rs
  fn shortcut(action: &str) -> &'static str;  // "undo" -> "Ctrl+Z" / "Cmd+Z"; "redo" -> "Ctrl+Y" / "Cmd+Shift+Z"
  ```
- Locale keys, en / it, with placeholders identical in both files:

  | Key | English | Italian |
  |---|---|---|
  | `sidebar.remove` | `Remove {file}` | `Togli {file}` |
  | `toast.removed_one` | `Removed {file} ({count} page). {shortcut} to undo` | `Tolto {file} ({count} pagina). {shortcut} per annullare` |
  | `toast.removed_many` | `Removed {file} ({count} pages). {shortcut} to undo` | `Tolto {file} ({count} pagine). {shortcut} per annullare` |
  | `action.undo` | `Undo ({shortcut})` | `Annulla ({shortcut})` |
  | `action.redo` | `Redo ({shortcut})` | `Ripristina ({shortcut})` |

- Behaviour to implement:
  - **×:** `icons::icon_button(ui, icons::close, &tr.tf("sidebar.remove", &[("file", &d.name)]))`, at the right end
    of each row; it stays enabled during a save. On click:
    - `let n = workspace.remove_document(id)`;
    - push a Success toast with `toast.removed_one` when `n == 1`, otherwise `toast.removed_many`, and the arguments
      `file` = the display name before removal, `count` = n, `shortcut` = `shortcut("undo")`.
  - **↶ / ↷:**
    - in the top bar's right-to-left group, placed so they appear just left of Split;
    - each is wrapped in `ui.add_enabled_ui(workspace.can_undo() / can_redo(), ..)`;
    - their names are `tr.tf("action.undo"/"action.redo", &[("shortcut", shortcut(..))])`;
    - they stay enabled during a save.
  - **Shortcuts in `ui()`:**
    - same guard as Delete: `!modal_open && !ctx.egui_wants_keyboard_input()`;
    - consume the most specific first:
      1. `COMMAND | SHIFT` + `Z` → redo;
      2. on non-macOS only, `COMMAND` + `Y` → redo;
      3. `COMMAND` + `Z` → undo.
  - **Clear all:** already builds `Workspace::new()`, which empties the history. Confirm it, and change nothing else.

- [ ] **Step 1: Write the failing kittests.**
  - `sidebar.rs` test `remove_button_has_accessible_name`:
    - render `doc_list` with one `DocSummary` named `contratto.pdf` in Italian;
    - `get_by_label("Togli contratto.pdf")` exists;
    - clicking it makes `doc_list` return `Some(id)`.
  - `toolbar.rs` (or `mod.rs`) test `undo_redo_disabled_without_history`:
    - render ↶ and ↷ the way the toolbar does, with `can_undo = false`;
    - the node labelled `Annulla (Ctrl+Z)` (or the macOS form under `cfg!`) is disabled.
    - If testing the real toolbar needs an Engine, factor the two buttons into a free function
      `undo_redo_buttons(ui, tr, can_undo, can_redo) -> (bool, bool)` and test that.
- [ ] **Step 2: Run** `cargo test --lib`. Expected: FAIL (missing items).
- [ ] **Step 3: Implement** the items in the Interfaces block. Add the locale keys to both files.
- [ ] **Step 4: Docs.**
  - README Usage: one short paragraph covering the × on a document, Undo/Redo with the shortcuts, what is undoable
    (add, remove, delete, move), the 50-step limit, and that Clear all empties the history.
  - Checklist: unticked items for
    - × removes the document and shows the toast;
    - letters stay;
    - Ctrl/Cmd+Z and redo work as listed;
    - the buttons are disabled when there is nothing to do;
    - Ctrl+Z in the Pages field edits the text;
    - undoing back to the saved state means closing does not ask;
    - undo during a save.
  - Version 0.3.0 in `Cargo.toml` and `Cargo.lock`.
- [ ] **Step 5: Run** `cargo test`, fmt and clippy for the three targets. Expected: all pass and clean.
- [ ] **Step 6: Run under Xvfb** at 1280×800 and at 1024×600 in Italian, with two PDFs. Take screenshots:
  - the sidebar with the × buttons;
  - the top bar with ↶ ↷ (both must fit at 1024 px);
  - the toast after a removal.

  View them. Save them in the session scratchpad and list their paths.
- [ ] **Step 7: Commit** `feat: remove a document, undo and redo; version 0.3.0`.
