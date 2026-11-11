//! LRU map keyed by page and bounded by the total size of its values; the UI stores egui texture
//! handles in it.

use crate::workspace::PageRef;
use std::collections::HashMap;

pub struct ThumbCache<V> {
    max_bytes: usize,
    bytes: usize,
    tick: u64,
    /// value, last use, size in bytes
    map: HashMap<PageRef, (V, u64, usize)>,
}

impl<V> ThumbCache<V> {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            bytes: 0,
            tick: 0,
            map: HashMap::new(),
        }
    }

    /// Marks the entry as recently used.
    pub fn get(&mut self, key: &PageRef) -> Option<&V> {
        self.tick += 1;
        let tick = self.tick;
        self.map.get_mut(key).map(|(v, used, _)| {
            *used = tick;
            &*v
        })
    }

    /// Evicts least recently used entries until the total is within `max_bytes`; the new entry
    /// is always kept, even if it alone is larger.
    pub fn insert(&mut self, key: PageRef, value: V, bytes: usize) {
        self.tick += 1;
        if let Some((_, _, old)) = self.map.insert(key, (value, self.tick, bytes)) {
            self.bytes -= old;
        }
        self.bytes += bytes;
        // shortcut: linear scan for the oldest entry, fine for the few hundred thumbnails the UI's
        // budget holds; use a linked list if the budget or entry count grows a lot
        while self.bytes > self.max_bytes && self.map.len() > 1 {
            let oldest = self
                .map
                .iter()
                .min_by_key(|(_, (_, t, _))| *t)
                .map(|(k, _)| *k);
            if let Some((_, _, b)) = oldest.and_then(|k| self.map.remove(&k)) {
                self.bytes -= b;
            }
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(index: u32) -> PageRef {
        PageRef { doc: 0, index }
    }

    #[test]
    fn evicts_least_recently_used() {
        let mut c = ThumbCache::new(2);
        c.insert(p(0), 'a', 1);
        c.insert(p(1), 'b', 1);
        assert_eq!(c.get(&p(0)), Some(&'a'));
        c.insert(p(2), 'c', 1);
        assert_eq!(c.get(&p(1)), None);
        assert_eq!(c.get(&p(0)), Some(&'a'));
        assert_eq!(c.get(&p(2)), Some(&'c'));
    }

    #[test]
    fn insert_existing_updates_without_growing() {
        let mut c = ThumbCache::new(2);
        c.insert(p(0), 'a', 1);
        c.insert(p(0), 'b', 1);
        // If the update had counted its bytes twice, this insert would evict one.
        c.insert(p(1), 'c', 1);
        assert_eq!(c.get(&p(0)), Some(&'b'));
        assert_eq!(c.get(&p(1)), Some(&'c'));
    }

    #[test]
    fn evicts_by_bytes_and_keeps_an_oversized_new_entry() {
        let mut c = ThumbCache::new(10);
        c.insert(p(0), 'a', 4);
        c.insert(p(1), 'b', 4);
        c.insert(p(2), 'c', 4); // 12 > 10: the oldest goes
        assert_eq!(c.get(&p(0)), None);
        assert_eq!(c.get(&p(1)), Some(&'b'));
        assert_eq!(c.get(&p(2)), Some(&'c'));
        c.insert(p(3), 'd', 50); // larger than the budget on its own
        assert_eq!(c.get(&p(1)), None);
        assert_eq!(c.get(&p(2)), None);
        assert_eq!(c.get(&p(3)), Some(&'d'));
        c.insert(p(4), 'e', 1); // the oversized entry is evicted in turn
        assert_eq!(c.get(&p(3)), None);
        assert_eq!(c.get(&p(4)), Some(&'e'));
    }
}
