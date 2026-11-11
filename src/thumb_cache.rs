//! Bounded LRU map keyed by page; the UI stores egui texture handles in it.

use crate::workspace::PageRef;
use std::collections::HashMap;

pub struct ThumbCache<V> {
    cap: usize,
    tick: u64,
    map: HashMap<PageRef, (V, u64)>,
}

impl<V> ThumbCache<V> {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            tick: 0,
            map: HashMap::new(),
        }
    }

    /// Marks the entry as recently used.
    pub fn get(&mut self, key: &PageRef) -> Option<&V> {
        self.tick += 1;
        let tick = self.tick;
        self.map.get_mut(key).map(|(v, used)| {
            *used = tick;
            &*v
        })
    }

    /// Evicts the least recently used entry when over capacity.
    pub fn insert(&mut self, key: PageRef, value: V) {
        self.tick += 1;
        self.map.insert(key, (value, self.tick));
        // shortcut: linear scan for the oldest entry, fine at cap 300; use a linked list if the cap grows
        while self.map.len() > self.cap {
            let oldest = self
                .map
                .iter()
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, _)| *k);
            match oldest {
                Some(k) => self.map.remove(&k),
                None => break,
            };
        }
    }

    pub fn contains(&self, key: &PageRef) -> bool {
        self.map.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
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
        c.insert(p(0), 'a');
        c.insert(p(1), 'b');
        assert_eq!(c.get(&p(0)), Some(&'a'));
        c.insert(p(2), 'c');
        assert!(c.contains(&p(0)) && c.contains(&p(2)) && !c.contains(&p(1)));
    }

    #[test]
    fn insert_existing_updates_without_growing() {
        let mut c = ThumbCache::new(2);
        c.insert(p(0), 'a');
        c.insert(p(0), 'b');
        assert_eq!(c.len(), 1);
        assert_eq!(c.get(&p(0)), Some(&'b'));
    }
}
