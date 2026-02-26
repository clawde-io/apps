// SPDX-License-Identifier: MIT
// Completion LRU cache (Sprint GG, CC.3).
//
// Caches recent completion results so identical or near-identical cursor
// positions skip the provider round-trip.
//
// Cache key = SHA-256( last-512-bytes of prefix  +  first-128-bytes of suffix ).
// Capacity: 256 entries (configurable).

use std::collections::HashMap;
use std::collections::VecDeque;

use sha2::{Digest, Sha256};

use super::engine::Insertion;

/// An entry stored in the completion cache.
#[derive(Clone)]
pub struct CacheEntry {
    pub insertions: Vec<Insertion>,
    pub created_at: std::time::Instant,
}

/// LRU cache for completion results.
///
/// Thread-safety: wrap in `Mutex<CompletionCache>` for shared use.
pub struct CompletionCache {
    capacity: usize,
    map: HashMap<String, CacheEntry>,
    /// Key insertion order (front = oldest, back = newest).
    order: VecDeque<String>,
    pub hits: u64,
    pub misses: u64,
}

impl CompletionCache {
    /// Create a new cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            hits: 0,
            misses: 0,
        }
    }

    /// Compute the cache key for the given prefix and suffix slices.
    pub fn cache_key(prefix: &str, suffix: &str) -> String {
        // Use last 512 bytes of prefix and first 128 bytes of suffix.
        let prefix_slice = if prefix.len() > 512 {
            &prefix[prefix.len() - 512..]
        } else {
            prefix
        };
        let suffix_slice = if suffix.len() > 128 {
            &suffix[..128]
        } else {
            suffix
        };

        let mut hasher = Sha256::new();
        hasher.update(prefix_slice.as_bytes());
        hasher.update(b"\0");
        hasher.update(suffix_slice.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Look up a cache entry. Returns `Some(entry)` on hit.
    pub fn get(&mut self, key: &str) -> Option<&CacheEntry> {
        if self.map.contains_key(key) {
            // Move to back (most recently used).
            self.order.retain(|k| k != key);
            self.order.push_back(key.to_string());
            self.hits += 1;
            self.map.get(key)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert a new entry. Evicts the least-recently-used entry if at capacity.
    pub fn insert(&mut self, key: String, entry: CacheEntry) {
        if self.map.contains_key(&key) {
            self.order.retain(|k| k != &key);
        } else if self.map.len() >= self.capacity {
            if let Some(evict) = self.order.pop_front() {
                self.map.remove(&evict);
            }
        }
        self.order.push_back(key.clone());
        self.map.insert(key, entry);
    }

    /// Hit rate as a value 0.0–1.0.  Returns 0.0 if no requests yet.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Current number of entries in the cache.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_insertion(text: &str) -> Insertion {
        Insertion {
            text: text.to_string(),
            start_line: 0,
            end_line: 0,
            confidence: 1.0,
        }
    }

    #[test]
    fn cache_key_deterministic() {
        let k1 = CompletionCache::cache_key("hello", "world");
        let k2 = CompletionCache::cache_key("hello", "world");
        assert_eq!(k1, k2);
    }

    #[test]
    fn cache_key_differs_on_change() {
        let k1 = CompletionCache::cache_key("hello", "world");
        let k2 = CompletionCache::cache_key("hello", "different");
        assert_ne!(k1, k2);
    }

    #[test]
    fn cache_hit_and_miss() {
        let mut cache = CompletionCache::new(4);
        let key = CompletionCache::cache_key("prefix", "suffix");
        assert!(cache.get(&key).is_none());
        assert_eq!(cache.misses, 1);

        cache.insert(
            key.clone(),
            CacheEntry {
                insertions: vec![make_insertion("x")],
                created_at: std::time::Instant::now(),
            },
        );
        assert!(cache.get(&key).is_some());
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn cache_evicts_lru() {
        let mut cache = CompletionCache::new(2);
        let k1 = "key1".to_string();
        let k2 = "key2".to_string();
        let k3 = "key3".to_string();

        cache.insert(k1.clone(), CacheEntry { insertions: vec![], created_at: std::time::Instant::now() });
        cache.insert(k2.clone(), CacheEntry { insertions: vec![], created_at: std::time::Instant::now() });
        // k1 is LRU — inserting k3 should evict k1
        cache.insert(k3.clone(), CacheEntry { insertions: vec![], created_at: std::time::Instant::now() });

        assert_eq!(cache.len(), 2);
        assert!(cache.map.contains_key(&k2));
        assert!(cache.map.contains_key(&k3));
        assert!(!cache.map.contains_key(&k1));
    }

    #[test]
    fn hit_rate_calculation() {
        let mut cache = CompletionCache::new(4);
        assert_eq!(cache.hit_rate(), 0.0);

        let k = CompletionCache::cache_key("a", "b");
        cache.get(&k); // miss
        cache.insert(k.clone(), CacheEntry { insertions: vec![], created_at: std::time::Instant::now() });
        cache.get(&k); // hit
        assert!((cache.hit_rate() - 0.5).abs() < 1e-9);
    }
}
