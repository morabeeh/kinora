//! A least-recently-used cache budgeted in **bytes** (brief §7.5): decoded pictures differ
//! wildly in size, so counting entries would be meaningless.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

#[derive(Debug)]
struct Entry<V> {
    value: Arc<V>,
    bytes: u64,
    last_used: u64,
}

/// LRU cache that keeps the total size of its values under a byte budget.
///
/// Values are shared (`Arc`), so evicting an entry that is still on screen frees nothing
/// until the screen lets go of it — but it never pulls the picture away from the viewer.
/// Keys can be **pinned** (the current item): pinned entries are never evicted.
#[derive(Debug)]
pub struct ByteLru<K, V> {
    entries: HashMap<K, Entry<V>>,
    pinned: HashSet<K>,
    budget: u64,
    used: u64,
    clock: u64,
}

impl<K: Eq + Hash + Clone, V> ByteLru<K, V> {
    /// An empty cache with a budget in bytes.
    #[must_use]
    pub fn new(budget: u64) -> Self {
        Self {
            entries: HashMap::new(),
            pinned: HashSet::new(),
            budget,
            used: 0,
            clock: 0,
        }
    }

    /// The value for `key`, marking it as recently used.
    pub fn get(&mut self, key: &K) -> Option<Arc<V>> {
        self.clock += 1;
        let clock = self.clock;
        self.entries.get_mut(key).map(|entry| {
            entry.last_used = clock;
            Arc::clone(&entry.value)
        })
    }

    /// `true` if `key` is cached (does not count as a use).
    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Adds or replaces an entry, then evicts least-recently-used, unpinned entries until
    /// the cache fits its budget again. The new entry itself is never evicted, even when it
    /// is bigger than the whole budget (it is about to be shown).
    pub fn insert(&mut self, key: K, value: Arc<V>, bytes: u64) {
        self.clock += 1;
        let protected = key.clone();
        if let Some(old) = self.entries.insert(
            key,
            Entry {
                value,
                bytes,
                last_used: self.clock,
            },
        ) {
            self.used = self.used.saturating_sub(old.bytes);
        }
        self.used = self.used.saturating_add(bytes);
        self.evict(Some(&protected));
    }

    /// Removes an entry.
    pub fn remove(&mut self, key: &K) -> Option<Arc<V>> {
        let entry = self.entries.remove(key)?;
        self.used = self.used.saturating_sub(entry.bytes);
        Some(entry.value)
    }

    /// Replaces the set of pinned keys (entries that are never evicted).
    pub fn set_pinned(&mut self, keys: impl IntoIterator<Item = K>) {
        self.pinned = keys.into_iter().collect();
        self.evict(None);
    }

    /// Changes the budget, evicting as needed.
    pub fn set_budget(&mut self, budget: u64) {
        self.budget = budget;
        self.evict(None);
    }

    /// Keeps only entries for which `keep` returns `true`.
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        let mut freed = 0u64;
        self.entries.retain(|key, entry| {
            let kept = keep(key);
            if !kept {
                freed = freed.saturating_add(entry.bytes);
            }
            kept
        });
        self.used = self.used.saturating_sub(freed);
    }

    /// Bytes currently cached.
    #[must_use]
    pub fn used_bytes(&self) -> u64 {
        self.used
    }

    /// The byte budget.
    #[must_use]
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn evict(&mut self, keep: Option<&K>) {
        while self.used > self.budget {
            let victim = self
                .entries
                .iter()
                .filter(|(key, _)| Some(*key) != keep && !self.pinned.contains(*key))
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone());
            let Some(victim) = victim else {
                break; // only pinned / protected entries left
            };
            self.remove(&victim);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(budget: u64) -> ByteLru<&'static str, u32> {
        ByteLru::new(budget)
    }

    #[test]
    fn evicts_least_recently_used_first() {
        let mut lru = cache(100);
        lru.insert("a", Arc::new(1), 40);
        lru.insert("b", Arc::new(2), 40);
        assert!(lru.get(&"a").is_some()); // "b" is now the oldest
        lru.insert("c", Arc::new(3), 40);
        assert!(lru.contains(&"a") && lru.contains(&"c"));
        assert!(!lru.contains(&"b"));
        assert_eq!(lru.used_bytes(), 80);
    }

    #[test]
    fn pinned_entries_survive() {
        let mut lru = cache(100);
        lru.insert("current", Arc::new(1), 60);
        lru.set_pinned(["current"]);
        lru.insert("next", Arc::new(2), 60);
        assert!(lru.contains(&"current"), "pinned");
        assert!(lru.contains(&"next"), "just inserted");
        assert_eq!(
            lru.used_bytes(),
            120,
            "over budget only because of protected entries"
        );
        lru.set_pinned(["next"]);
        assert!(!lru.contains(&"current"));
        assert_eq!(lru.used_bytes(), 60);
    }

    #[test]
    fn oversized_entry_is_kept_alone() {
        let mut lru = cache(10);
        lru.insert("small", Arc::new(1), 5);
        lru.insert("huge", Arc::new(2), 500);
        assert_eq!(lru.len(), 1);
        assert!(lru.contains(&"huge"));
    }

    #[test]
    fn replacing_updates_the_byte_count() {
        let mut lru = cache(100);
        lru.insert("a", Arc::new(1), 30);
        lru.insert("a", Arc::new(2), 50);
        assert_eq!(lru.used_bytes(), 50);
        assert_eq!(lru.get(&"a").as_deref(), Some(&2));
        assert_eq!(lru.remove(&"a").as_deref(), Some(&2));
        assert_eq!(lru.used_bytes(), 0);
        assert!(lru.is_empty());
    }

    #[test]
    fn shrinking_the_budget_and_retain() {
        let mut lru = cache(100);
        for (key, bytes) in [("a", 30), ("b", 30), ("c", 30)] {
            lru.insert(key, Arc::new(0), bytes);
        }
        lru.set_budget(60);
        assert_eq!(lru.len(), 2);
        assert!(!lru.contains(&"a"), "oldest went first");
        lru.retain(|key| *key == "c");
        assert_eq!(lru.len(), 1);
        assert_eq!(lru.used_bytes(), 30);
        assert_eq!(lru.budget(), 60);
    }

    #[test]
    fn shared_values_stay_alive_after_eviction() {
        let mut lru = cache(10);
        lru.insert("shown", Arc::new(7), 10);
        let on_screen = lru.get(&"shown").unwrap();
        lru.insert("other", Arc::new(8), 10);
        assert!(!lru.contains(&"shown"));
        assert_eq!(*on_screen, 7, "the viewer keeps its picture");
    }
}
