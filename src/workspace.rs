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
}

#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub id: SlotId,
    pub page: PageRef,
}

#[derive(Default)]
pub struct Workspace {
    docs: Vec<SourceDoc>,
    slots: Vec<Slot>,
    selected: HashSet<SlotId>,
    anchor: Option<SlotId>,
    next_slot: u64,
    dirty: bool,
}

impl Workspace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends all pages of `doc` to the grid and marks the workspace dirty.
    pub fn add_document(&mut self, doc: SourceDoc) {
        for index in 0..doc.page_sizes.len() as u32 {
            let id = SlotId(self.next_slot);
            self.next_slot += 1;
            self.slots.push(Slot {
                id,
                page: PageRef { doc: doc.id, index },
            });
        }
        self.docs.push(doc);
        self.dirty = true;
    }

    pub fn doc(&self, id: DocId) -> Option<&SourceDoc> {
        self.docs.iter().find(|d| d.id == id)
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

    /// Replaces the selection (positions out of range are ignored).
    pub fn set_selection(&mut self, positions: &[usize]) {
        self.selected = positions
            .iter()
            .filter_map(|&p| self.slots.get(p).map(|s| s.id))
            .collect();
        self.anchor = None;
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
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
            self.dirty = true;
        }
    }

    pub fn delete_selected(&mut self) {
        let sel = std::mem::take(&mut self.selected);
        self.slots.retain(|s| !sel.contains(&s.id));
        self.anchor = None;
        self.dirty = true;
    }

    pub fn pages_at(&self, positions: &[usize]) -> Vec<PageRef> {
        positions
            .iter()
            .filter_map(|&p| self.slots.get(p).map(|s| s.page))
            .collect()
    }

    pub fn all_pages(&self) -> Vec<PageRef> {
        self.slots.iter().map(|s| s.page).collect()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
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
pub fn split_names(stem: &str, groups: &[Vec<usize>], existing: &HashSet<String>) -> Vec<String> {
    split_names_with(stem, groups, existing, FOLD_CASE)
}

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

/// `split_names`, also bumping every name for which `taken` reports an existing file: catches
/// clashes the name comparison misses (case-insensitive or normalizing file systems, new files).
pub fn free_split_names(
    stem: &str,
    groups: &[Vec<usize>],
    existing: &HashSet<String>,
    taken: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut existing = existing.clone();
    loop {
        let names = split_names(stem, groups, &existing);
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
        let names = split_names("doc", &[vec![0], vec![1, 2, 3]], &HashSet::new());
        assert_eq!(names, ["doc_p1.pdf", "doc_p2-4.pdf"]);
    }

    #[test]
    fn split_names_are_unique_and_never_clash() {
        let existing = HashSet::from(["doc_p1.pdf".to_string()]);
        let names = split_names("doc", &[vec![0], vec![0]], &existing);
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
        });
        assert!(ws.is_source_path(&dir.path().join(".").join("a.pdf")));
        assert!(!ws.is_source_path(&dir.path().join("b.pdf")));
    }
}
