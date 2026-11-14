//! Workspace model: the ordered list of pages from all opened documents,
//! plus selection and dirty state. Pure data, no UI or PDF engine.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub type DocId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PageRef {
    pub doc: DocId,
    /// 0-based page index in its source document.
    pub index: u32,
}

/// Stable id of one grid cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotId(u64);

#[derive(Clone, Debug)]
pub struct SourceDoc {
    pub id: DocId,
    pub path: PathBuf,
    /// Page sizes in points (width, height).
    pub page_sizes: Vec<(f32, f32)>,
    /// Hidden from the sidebar and the grid (removed or undone). Slots of a removed
    /// document are gone; the entry stays so letters and ids are never reused.
    pub removed: bool,
}

impl SourceDoc {
    /// A document as the engine opened it.
    pub fn new(id: DocId, path: PathBuf, page_sizes: Vec<(f32, f32)>) -> Self {
        Self {
            id,
            path,
            page_sizes,
            removed: false,
        }
    }
}

/// One open document as the sidebar shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocSummary {
    pub id: DocId,
    pub letter: String,
    pub name: String,
    /// Pages of this document currently in the grid.
    pub pages_in_grid: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub id: SlotId,
    pub page: PageRef,
}

/// Maximum number of undo steps kept.
pub const HISTORY_LIMIT: usize = 50;

/// One point in the undo history.
#[derive(Clone)]
struct State {
    slots: Vec<Slot>,
    /// `removed[i]` is the flag of `docs[i]`; documents past the end were not added yet.
    removed: Vec<bool>,
    selected: HashSet<SlotId>,
    /// Identifies the content; equal revs mean the same saved-or-not state.
    rev: u64,
}

pub struct Workspace {
    docs: Vec<SourceDoc>,
    slots: Vec<Slot>,
    selected: HashSet<SlotId>,
    anchor: Option<SlotId>,
    next_slot: u64,
    undo: Vec<State>,
    redo: Vec<State>,
    rev: u64,
    next_rev: u64,
    saved_rev: Option<u64>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            docs: Vec::new(),
            slots: Vec::new(),
            selected: HashSet::new(),
            anchor: None,
            next_slot: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            rev: 0,
            next_rev: 1,
            saved_rev: Some(0),
        }
    }
}

impl Workspace {
    pub fn new() -> Self {
        Self::default()
    }

    fn snapshot(&self) -> State {
        State {
            slots: self.slots.clone(),
            removed: self.docs.iter().map(|d| d.removed).collect(),
            selected: self.selected.clone(),
            rev: self.rev,
        }
    }

    /// Records `before` as an undo step after a real change.
    fn commit(&mut self, before: State) {
        self.undo.push(before);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.rev = self.next_rev;
        self.next_rev += 1;
    }

    /// Switches to `to` and returns the state that was left.
    fn restore(&mut self, to: State) -> State {
        let mut left = self.snapshot();
        let gone: HashSet<SlotId> = left.slots.iter().map(|s| s.id).collect();
        let back: HashSet<SlotId> = to
            .slots
            .iter()
            .map(|s| s.id)
            .filter(|id| !gone.contains(id))
            .collect();
        self.selected = if back.is_empty() { to.selected } else { back };
        // Coming back to the state being left should select what this step touched.
        left.selected = self
            .selected
            .iter()
            .filter(|s| gone.contains(s))
            .copied()
            .collect();
        self.slots = to.slots;
        for (i, d) in self.docs.iter_mut().enumerate() {
            d.removed = to.removed.get(i).copied().unwrap_or(true);
        }
        self.rev = to.rev;
        self.anchor = None;
        left
    }

    pub fn undo(&mut self) -> bool {
        let Some(to) = self.undo.pop() else {
            return false;
        };
        let left = self.restore(to);
        self.redo.push(left);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(to) = self.redo.pop() else {
            return false;
        };
        let left = self.restore(to);
        self.undo.push(left);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Removes the document and its pages from the grid; returns how many pages went.
    /// A no-op for an unknown or already removed document.
    pub fn remove_document(&mut self, id: DocId) -> usize {
        let Some(i) = self.docs.iter().position(|d| d.id == id && !d.removed) else {
            return 0;
        };
        let before = self.snapshot();
        let n = self.slots.len();
        self.slots.retain(|s| s.page.doc != id);
        let kept: HashSet<SlotId> = self.slots.iter().map(|s| s.id).collect();
        self.selected.retain(|s| kept.contains(s));
        self.anchor = None;
        self.docs[i].removed = true;
        self.commit(before);
        n - self.slots.len()
    }

    /// Appends all pages of `doc` to the grid (an undoable step).
    pub fn add_document(&mut self, mut doc: SourceDoc) {
        let before = self.snapshot();
        doc.removed = false;
        for index in 0..doc.page_sizes.len() as u32 {
            let id = SlotId(self.next_slot);
            self.next_slot += 1;
            self.slots.push(Slot {
                id,
                page: PageRef { doc: doc.id, index },
            });
        }
        self.docs.push(doc);
        self.commit(before);
    }

    pub fn doc(&self, id: DocId) -> Option<&SourceDoc> {
        // DocId is the position in `docs` (both come from the engine's open order).
        self.docs.get(id).filter(|d| d.id == id)
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Size in points of the slot's page; (0, 0) if the document is unknown.
    pub fn page_size(&self, slot: &Slot) -> (f32, f32) {
        self.doc(slot.page.doc)
            .and_then(|d| d.page_sizes.get(slot.page.index as usize).copied())
            .unwrap_or((0.0, 0.0))
    }

    pub fn is_selected(&self, pos: usize) -> bool {
        self.slots
            .get(pos)
            .is_some_and(|s| self.selected.contains(&s.id))
    }

    /// Plain click.
    pub fn select_only(&mut self, pos: usize) {
        self.selected.clear();
        if let Some(s) = self.slots.get(pos) {
            self.selected.insert(s.id);
            self.anchor = Some(s.id);
        }
    }

    /// Ctrl/Cmd+click.
    pub fn toggle(&mut self, pos: usize) {
        if let Some(s) = self.slots.get(pos) {
            if !self.selected.remove(&s.id) {
                self.selected.insert(s.id);
            }
            self.anchor = Some(s.id);
        }
    }

    /// Shift+click: replaces the selection with the range from the anchor to `pos`.
    pub fn select_range_to(&mut self, pos: usize) {
        let anchor = self
            .anchor
            .and_then(|a| self.slots.iter().position(|s| s.id == a));
        let Some(anchor) = anchor else {
            return self.select_only(pos);
        };
        if pos >= self.slots.len() {
            return;
        }
        let (lo, hi) = (anchor.min(pos), anchor.max(pos));
        self.selected = self.slots[lo..=hi].iter().map(|s| s.id).collect();
    }

    /// Ctrl/Cmd+A: selects every page; the anchor becomes the first page.
    pub fn select_all(&mut self) {
        self.selected = self.slots.iter().map(|s| s.id).collect();
        self.anchor = self.slots.first().map(|s| s.id);
    }

    /// Replaces the selection (positions out of range are ignored).
    pub fn set_selection(&mut self, positions: &[usize]) {
        self.selected = positions
            .iter()
            .filter_map(|&p| self.slots.get(p).map(|s| s.id))
            .collect();
        self.anchor = None;
    }

    pub fn has_selection(&self) -> bool {
        !self.selected.is_empty()
    }

    /// Selected grid positions, ascending.
    pub fn selected_positions(&self) -> Vec<usize> {
        (0..self.slots.len())
            .filter(|&p| self.is_selected(p))
            .collect()
    }

    /// Moves the selected pages before the page that is at grid position `target`
    /// (`len` = end), keeping their relative order. If that page is itself
    /// selected, they go before the next unselected page after it.
    pub fn move_selected_to(&mut self, target: usize) {
        if self.selected.is_empty() {
            return;
        }
        let before = self.snapshot();
        let target = target.min(self.slots.len());
        let insert_at = (target..self.slots.len())
            .find(|&p| !self.is_selected(p))
            .unwrap_or(self.slots.len());
        let unselected_before = (0..insert_at).filter(|&p| !self.is_selected(p)).count();
        let (moved, mut rest): (Vec<Slot>, Vec<Slot>) = self
            .slots
            .iter()
            .partition(|s| self.selected.contains(&s.id));
        rest.splice(unselected_before..unselected_before, moved);
        if rest
            .iter()
            .map(|s| s.id)
            .ne(self.slots.iter().map(|s| s.id))
        {
            self.slots = rest;
            self.commit(before);
        }
    }

    pub fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let before = self.snapshot();
        let sel = std::mem::take(&mut self.selected);
        self.slots.retain(|s| !sel.contains(&s.id));
        self.anchor = None;
        self.commit(before);
    }

    pub fn pages_at(&self, positions: &[usize]) -> Vec<PageRef> {
        positions
            .iter()
            .filter_map(|&p| self.slots.get(p).map(|s| s.page))
            .collect()
    }

    /// Open documents in opening order with their letter, display name and page count.
    pub fn doc_summaries(&self) -> Vec<DocSummary> {
        let shown: Vec<(usize, &SourceDoc)> = self
            .docs
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.removed)
            .collect();
        let paths: Vec<PathBuf> = shown.iter().map(|(_, d)| d.path.clone()).collect();
        let names = display_names(&paths);
        let mut counts = vec![0; self.docs.len()];
        for s in &self.slots {
            if let Some(c) = counts.get_mut(s.page.doc) {
                *c += 1;
            }
        }
        shown
            .into_iter()
            .zip(names)
            .map(|((i, d), name)| DocSummary {
                id: d.id,
                letter: doc_letter(i),
                name,
                pages_in_grid: counts.get(d.id).copied().unwrap_or(0),
            })
            .collect()
    }

    pub fn all_pages(&self) -> Vec<PageRef> {
        self.slots.iter().map(|s| s.page).collect()
    }

    pub fn is_dirty(&self) -> bool {
        self.saved_rev != Some(self.rev)
    }

    pub fn mark_saved(&mut self) {
        self.saved_rev = Some(self.rev);
    }

    /// Identifies the current content, for `mark_saved_at`.
    pub fn rev(&self) -> u64 {
        self.rev
    }

    /// Marks the state that had `rev` as saved, e.g. when a save started there has finished.
    pub fn mark_saved_at(&mut self, rev: u64) {
        self.saved_rev = Some(rev);
    }

    /// True if `dest` is the file of one of the source documents.
    pub fn is_source_path(&self, dest: &Path) -> bool {
        let Some(dest) = canonical(dest) else {
            return false;
        };
        self.docs
            .iter()
            .filter_map(|d| canonical(&d.path))
            .any(|p| p == dest)
    }
}

/// Canonical path; for a missing file, canonical parent + file name.
fn canonical(p: &Path) -> Option<String> {
    let c = std::fs::canonicalize(p).ok().or_else(|| {
        let parent = std::fs::canonicalize(p.parent()?).ok()?;
        Some(parent.join(p.file_name()?))
    })?;
    Some(fs_key(&c.to_string_lossy(), FOLD_CASE))
}

/// Windows and macOS file systems are case-insensitive by default.
const FOLD_CASE: bool = cfg!(any(windows, target_os = "macos"));

/// Comparison key for file names and paths.
fn fs_key(s: &str, fold_case: bool) -> String {
    if fold_case {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// File names for Split, one per group (0-based positions), unique among
/// themselves and against `existing`. Clashes get " (2)", " (3)", ... before ".pdf".
fn split_names_with(
    stem: &str,
    groups: &[Vec<usize>],
    existing: &HashSet<String>,
    fold_case: bool,
) -> Vec<String> {
    let mut used: HashSet<String> = existing.iter().map(|n| fs_key(n, fold_case)).collect();
    groups
        .iter()
        .map(|g| {
            let (a, b) = (
                g.first().map_or(0, |p| p + 1),
                g.last().map_or(0, |p| p + 1),
            );
            let base = if a == b {
                format!("{stem}_p{a}")
            } else {
                format!("{stem}_p{a}-{b}")
            };
            let mut name = format!("{base}.pdf");
            let mut n = 2;
            while used.contains(&fs_key(&name, fold_case)) {
                name = format!("{base} ({n}).pdf");
                n += 1;
            }
            used.insert(fs_key(&name, fold_case));
            name
        })
        .collect()
}

/// Split file names (see `split_names_with`), also bumping every name for which `taken` reports an existing file: catches
/// clashes the name comparison misses (case-insensitive or normalizing file systems, new files).
pub fn free_split_names(
    stem: &str,
    groups: &[Vec<usize>],
    existing: &HashSet<String>,
    taken: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut existing = existing.clone();
    loop {
        let names = split_names_with(stem, groups, &existing, FOLD_CASE);
        // Clashes are never in `existing`, so it grows each round and the loop ends.
        let clashes: Vec<String> = names.iter().filter(|n| taken(n)).cloned().collect();
        if clashes.is_empty() {
            return names;
        }
        existing.extend(clashes);
    }
}

/// File stem of the source document of the first page in the grid.
pub fn split_stem(ws: &Workspace) -> Option<String> {
    let first = ws.slots().first()?;
    let doc = ws.doc(first.page.doc)?;
    Some(doc.path.file_stem()?.to_string_lossy().into_owned())
}

/// Spreadsheet-style label: 0 -> "A", 25 -> "Z", 26 -> "AA".
pub fn doc_letter(index: usize) -> String {
    let mut n = index + 1;
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// The path's file name, or "" if it has none.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// File names; those that collide get " — " and the last components of their parent folder: two,
/// or more while two same-named files would still read the same.
pub fn display_names(paths: &[PathBuf]) -> Vec<String> {
    let names: Vec<String> = paths.iter().map(|p| file_name(p)).collect();
    let parents: Vec<Vec<String>> = paths
        .iter()
        .map(|p| {
            p.parent()
                .into_iter()
                .flat_map(Path::components)
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect()
        })
        .collect();
    let label = |i: usize, k: usize| {
        let tail = &parents[i][parents[i].len().saturating_sub(k)..];
        if tail.is_empty() {
            names[i].clone()
        } else {
            format!("{} — {}", names[i], tail.join("/"))
        }
    };
    (0..paths.len())
        .map(|i| {
            let same: Vec<usize> = (0..paths.len()).filter(|&j| names[j] == names[i]).collect();
            if same.len() < 2 {
                return names[i].clone();
            }
            let deepest = same.iter().map(|&j| parents[j].len()).max().unwrap_or(0);
            let mut k = 2;
            while k < deepest && same.iter().any(|&j| j != i && label(j, k) == label(i, k)) {
                k += 1;
            }
            label(i, k)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws_with(counts: &[usize]) -> Workspace {
        let mut ws = Workspace::new();
        for (id, &n) in counts.iter().enumerate() {
            ws.add_document(SourceDoc {
                id,
                path: PathBuf::from(format!("d{id}.pdf")),
                page_sizes: vec![(595.0, 842.0); n],
                removed: false,
            });
        }
        ws
    }

    fn pages(ws: &Workspace) -> Vec<(usize, u32)> {
        ws.all_pages().iter().map(|p| (p.doc, p.index)).collect()
    }

    #[test]
    fn add_document_appends_pages() {
        let ws = ws_with(&[3, 2]);
        assert_eq!(pages(&ws), [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1)]);
        assert!(ws.is_dirty());
    }

    #[test]
    fn move_selected_to_keeps_relative_order() {
        let mut ws = ws_with(&[3, 2]);
        ws.set_selection(&[0, 2]);
        ws.move_selected_to(5);
        assert_eq!(pages(&ws), [(0, 1), (1, 0), (1, 1), (0, 0), (0, 2)]);
        assert_eq!(ws.selected_positions(), [3, 4]);
    }

    #[test]
    fn move_selected_to_front() {
        let mut ws = ws_with(&[3, 2]);
        ws.select_only(4);
        ws.move_selected_to(0);
        assert_eq!(pages(&ws)[0], (1, 1));
    }

    #[test]
    fn move_onto_selected_target_goes_before_next_unselected() {
        let mut ws = ws_with(&[3, 2]);
        ws.set_selection(&[1, 2]);
        ws.move_selected_to(1);
        assert_eq!(pages(&ws), [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1)]);
        ws.move_selected_to(2);
        assert_eq!(pages(&ws), [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1)]);
    }

    #[test]
    fn delete_selected() {
        let mut ws = ws_with(&[3, 2]);
        ws.set_selection(&[1, 3]);
        ws.delete_selected();
        assert_eq!(pages(&ws), [(0, 0), (0, 2), (1, 1)]);
        assert!(ws.selected_positions().is_empty());
    }

    #[test]
    fn delete_without_selection_is_not_dirty() {
        let mut ws = ws_with(&[3]);
        ws.mark_saved();
        ws.delete_selected();
        assert!(!ws.is_dirty());
        assert!(!ws.has_selection());
    }

    #[test]
    fn shift_click_selects_range() {
        let mut ws = ws_with(&[3, 2]);
        ws.select_only(1);
        ws.select_range_to(3);
        assert_eq!(ws.selected_positions(), [1, 2, 3]);
    }

    #[test]
    fn shift_click_without_anchor_selects_one() {
        let mut ws = ws_with(&[3, 2]);
        ws.select_range_to(2);
        assert_eq!(ws.selected_positions(), [2]);
    }

    #[test]
    fn toggle() {
        let mut ws = ws_with(&[3, 2]);
        ws.select_only(0);
        ws.toggle(2);
        ws.toggle(0);
        assert_eq!(ws.selected_positions(), [2]);
    }

    #[test]
    fn select_all_selects_every_page_without_dirtying() {
        let mut ws = ws_with(&[3, 2]);
        ws.mark_saved();
        ws.select_all();
        assert_eq!(ws.selected_positions(), [0, 1, 2, 3, 4]);
        assert!(!ws.is_dirty());
        ws.select_range_to(1); // the anchor is the first page
        assert_eq!(ws.selected_positions(), [0, 1]);
        let mut empty = Workspace::new();
        empty.select_all();
        assert!(!empty.has_selection());
    }

    #[test]
    fn mark_saved_clears_dirty() {
        let mut ws = ws_with(&[3]);
        ws.mark_saved();
        assert!(!ws.is_dirty());
        ws.select_only(0);
        ws.move_selected_to(2);
        assert!(ws.is_dirty());
    }

    #[test]
    fn move_to_same_place_is_not_dirty() {
        let mut ws = ws_with(&[3]);
        ws.mark_saved();
        ws.select_only(1);
        ws.move_selected_to(1);
        assert!(!ws.is_dirty());
    }

    #[test]
    fn split_names_basic() {
        let names = split_names_with("doc", &[vec![0], vec![1, 2, 3]], &HashSet::new(), FOLD_CASE);
        assert_eq!(names, ["doc_p1.pdf", "doc_p2-4.pdf"]);
    }

    #[test]
    fn split_names_are_unique_and_never_clash() {
        let existing = HashSet::from(["doc_p1.pdf".to_string()]);
        let names = split_names_with("doc", &[vec![0], vec![0]], &existing, FOLD_CASE);
        assert_eq!(names, ["doc_p1 (2).pdf", "doc_p1 (3).pdf"]);
    }

    #[test]
    fn split_names_fold_case_when_requested() {
        let existing = HashSet::from(["Doc_p1.PDF".to_string()]);
        let g = [vec![0]];
        assert_eq!(
            split_names_with("doc", &g, &existing, true),
            ["doc_p1 (2).pdf"]
        );
        assert_eq!(
            split_names_with("doc", &g, &existing, false),
            ["doc_p1.pdf"]
        );
    }

    #[test]
    fn free_split_names_skips_taken_files() {
        let taken = |n: &str| n == "doc_p1.pdf" || n == "doc_p1 (2).pdf";
        let names = free_split_names("doc", &[vec![0], vec![1]], &HashSet::new(), taken);
        assert_eq!(names, ["doc_p1 (3).pdf", "doc_p2.pdf"]);
    }

    #[test]
    fn split_stem_uses_first_page_source() {
        assert_eq!(split_stem(&ws_with(&[1])).as_deref(), Some("d0"));
        assert_eq!(split_stem(&Workspace::new()), None);
    }

    #[test]
    fn source_path_detection_uses_canonical_paths() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        std::fs::write(&a, b"x").unwrap();
        let mut ws = Workspace::new();
        ws.add_document(SourceDoc {
            id: 0,
            path: a,
            page_sizes: vec![(1.0, 1.0)],
            removed: false,
        });
        assert!(ws.is_source_path(&dir.path().join(".").join("a.pdf")));
        assert!(!ws.is_source_path(&dir.path().join("b.pdf")));
    }

    #[test]
    fn letters_spreadsheet_style() {
        let l: Vec<_> = [0, 25, 26, 27, 701, 702].map(doc_letter).into();
        assert_eq!(l, ["A", "Z", "AA", "AB", "ZZ", "AAA"]);
    }

    #[test]
    fn display_names_disambiguate_same_file_name() {
        let p = |s: &str| PathBuf::from(s);
        let got = display_names(&[p("/a/x/c.pdf"), p("/b/y/c.pdf"), p("/d.pdf")]);
        assert_eq!(got, ["c.pdf — a/x", "c.pdf — b/y", "d.pdf"]);
    }

    #[test]
    fn display_names_grow_until_unique() {
        let p = |s: &str| PathBuf::from(s);
        // Same last two folders: a third component tells them apart.
        let got = display_names(&[p("/a/x/y/c.pdf"), p("/b/x/y/c.pdf")]);
        assert_eq!(got, ["c.pdf — a/x/y", "c.pdf — b/x/y"]);
        // Three-way: only the pair that still collides grows.
        let got = display_names(&[p("/a/x/y/c.pdf"), p("/b/x/y/c.pdf"), p("/q/z/c.pdf")]);
        assert_eq!(got, ["c.pdf — a/x/y", "c.pdf — b/x/y", "c.pdf — q/z"]);
        // Different depths: the shorter path stops at its root.
        let got = display_names(&[p("/x/y/c.pdf"), p("/a/x/y/c.pdf")]);
        assert_eq!(got, ["c.pdf — x/y", "c.pdf — a/x/y"]);
    }

    #[test]
    fn doc_summaries_count_pages_in_grid() {
        let mut ws = ws_with(&[2, 1]);
        ws.select_only(2);
        ws.delete_selected();
        ws.select_only(0);
        ws.delete_selected();
        let s = ws.doc_summaries();
        assert_eq!(s[0].pages_in_grid, 1);
        assert_eq!((s[1].pages_in_grid, s[1].letter.as_str()), (0, "B"));
    }

    fn doc(id: usize, path: &str, n: usize) -> SourceDoc {
        SourceDoc {
            id,
            path: PathBuf::from(path),
            page_sizes: vec![(595.0, 842.0); n],
            removed: false,
        }
    }

    fn letters(ws: &Workspace) -> Vec<String> {
        ws.doc_summaries().into_iter().map(|d| d.letter).collect()
    }

    #[test]
    fn redo_move_selects_moved_pages() {
        let mut ws = ws_with(&[4]);
        ws.set_selection(&[0, 1]);
        ws.move_selected_to(4);
        ws.select_only(3);
        ws.undo();
        assert!(ws.redo());
        assert_eq!(pages(&ws), [(0, 2), (0, 3), (0, 0), (0, 1)]);
        assert_eq!(ws.selected_positions(), [2, 3]);
    }

    #[test]
    fn new_workspace_is_clean() {
        assert!(!Workspace::new().is_dirty());
    }

    #[test]
    fn undo_clears_anchor() {
        let mut ws = ws_with(&[4]);
        ws.select_only(0);
        ws.delete_selected();
        ws.select_only(2);
        ws.undo();
        ws.select_range_to(0); // no anchor: behaves like a plain click
        assert_eq!(ws.selected_positions(), [0]);
    }

    #[test]
    fn remove_document_drops_pages_and_hides_row() {
        let mut ws = ws_with(&[3, 2]);
        assert_eq!(ws.remove_document(0), 3);
        assert_eq!(ws.slots().len(), 2);
        assert_eq!(letters(&ws), ["B"]);
        assert!(ws.can_undo());
    }

    #[test]
    fn undo_restores_removed_document_and_selects_its_pages() {
        let mut ws = ws_with(&[3, 2]);
        ws.remove_document(0);
        assert!(ws.undo());
        assert_eq!(ws.slots().len(), 5);
        assert_eq!(letters(&ws), ["A", "B"]);
        assert_eq!(ws.selected_positions(), [0, 1, 2]);
    }

    #[test]
    fn redo_reapplies_and_new_edit_clears_redo() {
        let mut ws = ws_with(&[3, 2]);
        ws.remove_document(0);
        ws.undo();
        assert!(ws.can_redo());
        assert!(ws.redo());
        assert_eq!(ws.slots().len(), 2);
        ws.undo();
        ws.select_only(0);
        ws.delete_selected();
        assert!(!ws.can_redo());
    }

    #[test]
    fn undo_steps_back_strictly_in_order() {
        let mut ws = ws_with(&[2, 2]);
        ws.remove_document(0);
        ws.add_document(doc(2, "d2.pdf", 1));
        assert!(ws.undo());
        assert_eq!(letters(&ws), ["B"]);
        assert!(ws.undo());
        assert_eq!(letters(&ws), ["A", "B"]);
        assert_eq!(pages(&ws), [(0, 0), (0, 1), (1, 0), (1, 1)]);
    }

    #[test]
    fn letters_are_never_reused() {
        let mut ws = ws_with(&[1, 1]);
        ws.undo();
        ws.add_document(doc(2, "d2.pdf", 1));
        assert_eq!(letters(&ws), ["A", "C"]);
    }

    #[test]
    fn history_is_capped_at_50() {
        let mut ws = ws_with(&[60]);
        for _ in 0..51 {
            ws.select_only(0);
            ws.delete_selected();
        }
        assert_eq!((0..50).filter(|_| ws.undo()).count(), 50);
        assert!(!ws.undo());
        assert_eq!(ws.len(), 59);
    }

    #[test]
    fn undo_to_saved_state_is_clean() {
        let mut ws = ws_with(&[3]);
        ws.mark_saved();
        ws.select_only(0);
        ws.delete_selected();
        assert!(ws.is_dirty());
        ws.undo();
        assert!(!ws.is_dirty());
        ws.redo();
        assert!(ws.is_dirty());
    }

    #[test]
    fn noop_edits_push_no_history() {
        let mut ws = ws_with(&[3]);
        ws.delete_selected();
        ws.select_only(1);
        ws.move_selected_to(1);
        assert_eq!(ws.remove_document(99), 0);
        assert!(ws.undo());
        assert!(!ws.undo());
    }

    #[test]
    fn removing_document_with_no_pages_left_is_a_step() {
        let mut ws = ws_with(&[2, 1]);
        ws.set_selection(&[0, 1]);
        ws.delete_selected();
        assert_eq!(ws.remove_document(0), 0);
        assert_eq!(letters(&ws), ["B"]);
        assert!(ws.undo());
        assert_eq!(letters(&ws), ["A", "B"]);
    }

    #[test]
    fn undo_move_selects_moved_pages() {
        let mut ws = ws_with(&[4]);
        ws.set_selection(&[0, 1]);
        ws.move_selected_to(4);
        ws.select_only(3);
        assert!(ws.undo());
        assert_eq!(pages(&ws), [(0, 0), (0, 1), (0, 2), (0, 3)]);
        assert_eq!(ws.selected_positions(), [0, 1]);
    }

    #[test]
    fn display_names_ignore_removed_documents() {
        let mut ws = Workspace::new();
        ws.add_document(doc(0, "/a/x/c.pdf", 1));
        ws.add_document(doc(1, "/b/y/c.pdf", 1));
        ws.remove_document(0);
        assert_eq!(ws.doc_summaries()[0].name, "c.pdf");
    }

    #[test]
    fn save_finishing_after_undo_marks_the_written_state() {
        let mut ws = ws_with(&[3]);
        ws.mark_saved();
        ws.select_only(0);
        ws.delete_selected();
        // A save of this state starts, then the user undoes before it finishes.
        let r = ws.rev();
        ws.undo();
        ws.mark_saved_at(r);
        assert!(ws.is_dirty(), "the grid is in no file now");
        ws.redo();
        assert!(!ws.is_dirty());
    }
}
