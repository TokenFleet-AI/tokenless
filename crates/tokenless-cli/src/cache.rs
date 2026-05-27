//! Predictive compression cache backed by an LRU map with blake3 hashing.
//!
//! All compression/rewriting/encoding operations are pure functions —
//! same input always produces the same output. This cache skips redundant
//! computation when the same input is seen repeatedly within a session.
//!
//! Set `TOKENLESS_CACHE_SIZE=0` to disable caching (default: 512 entries).

use std::sync::{LazyLock, Mutex};

/// An LRU cache keyed by blake3 hash (first 8 bytes as u64).
///
/// Uses a simple Vec-based LRU: most-recently-used entries are at the front.
/// When the cache is full, the least-recently-used entry (at the back) is evicted.
pub struct PredictCache {
    entries: Vec<(u64, String)>,
    max_entries: usize,
}

impl PredictCache {
    /// Create a new cache with the given capacity.
    /// Use 0 to disable caching entirely.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries.min(1)),
            max_entries,
        }
    }

    fn hash_key(input: &str) -> u64 {
        let hash = blake3::hash(input.as_bytes());
        u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap_or([0; 8]))
    }

    /// Look up a cached result. Returns `Some(String)` on hit, `None` on miss.
    /// On hit, promotes the entry to MRU position.
    pub fn get(&mut self, input: &str) -> Option<String> {
        if self.max_entries == 0 {
            return None;
        }
        let key = Self::hash_key(input);
        if let Some(pos) = self.entries.iter().position(|(k, _)| *k == key) {
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(self.entries[0].1.clone())
        } else {
            None
        }
    }

    /// Insert a result into the cache. Evicts LRU entry if full.
    pub fn insert(&mut self, input: &str, output: &str) {
        if self.max_entries == 0 {
            return;
        }
        let key = Self::hash_key(input);
        // Remove existing entry for same key if any
        if let Some(pos) = self.entries.iter().position(|(k, _)| *k == key) {
            self.entries.remove(pos);
        }
        self.entries.insert(0, (key, output.to_string()));
        // Evict LRU if over capacity
        while self.entries.len() > self.max_entries {
            self.entries.pop();
        }
    }

    /// Return the number of entries currently cached.
    #[cfg(test)]
    #[must_use]
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Global cache instance, protected by a Mutex.
/// Default capacity is 512; set `TOKENLESS_CACHE_SIZE=0` to disable.
static CACHE: LazyLock<Mutex<PredictCache>> = LazyLock::new(|| Mutex::new(PredictCache::new(512)));

/// Get a reference to the global cache.
///
/// # Panics
///
/// Panics if the mutex is poisoned (unrecoverable).
#[allow(clippy::unwrap_used)]
fn cache() -> std::sync::MutexGuard<'static, PredictCache> {
    CACHE.lock().unwrap_or_else(|e| {
        CACHE.clear_poison();
        e.into_inner()
    })
}

/// Check cache for `input`. Returns `Some(cached_output)` on hit, `None` on miss.
pub fn cache_get(input: &str) -> Option<String> {
    cache().get(input)
}

/// Store `output` as the cached result for `input`.
pub fn cache_insert(input: &str, output: &str) {
    cache().insert(input, output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit_and_miss() {
        let input = r#"{"key":"value"}"#;
        let output = r#"{"key":"value-compressed"}"#;

        // Miss
        assert!(cache().get(input).is_none());

        // Insert
        cache().insert(input, output);

        // Hit
        assert_eq!(cache().get(input).unwrap(), output);
    }
}
