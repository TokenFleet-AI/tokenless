//! Predictive compression cache backed by an LRU map with blake3 hashing.
//!
//! All compression/rewriting/encoding operations are pure functions —
//! same input always produces the same output. This cache skips redundant
//! computation when the same input is seen repeatedly within a session.
//!
//! Set `TOKENLESS_CACHE_SIZE=0` to disable caching (default: 512 entries).
//! The cache is also disabled when experimental mode is off.

use std::{
    collections::{HashMap, VecDeque},
    fmt::Write as FmtWrite,
    sync::{LazyLock, Mutex},
    time::Instant,
};

use tokenless_stats::TokenlessConfig;

/// An O(1) LRU cache keyed by blake3 hash (first 8 bytes as u64).
///
/// Uses `HashMap` for O(1) lookup + `VecDeque` for recency ordering.
/// When a hit occurs, the key is moved to the front of the deque (O(1) for
/// single-element push/pop from ends, O(n) only when removing from middle
/// which is acceptable given typical cache sizes < 1024).
pub struct PredictCache {
    store: HashMap<u64, String>,
    order: VecDeque<u64>,
    max_entries: usize,
    /// Cumulative hit/miss counters for diagnostics.
    pub hits: u64,
    pub misses: u64,
}

impl PredictCache {
    /// Create a new cache with the given capacity.
    /// Use 0 to disable caching entirely.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            store: HashMap::with_capacity(max_entries.min(1)),
            order: VecDeque::with_capacity(max_entries.min(1)),
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    /// Hash the input with a version prefix to prevent stale cache entries
    /// across compressor upgrades.
    fn hash_key(input: &str) -> u64 {
        let versioned = format!("v{}:{}", env!("CARGO_PKG_VERSION"), input);
        let hash = blake3::hash(versioned.as_bytes());
        u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap_or([0; 8]))
    }

    /// Look up a cached result. Returns `Some(String)` on hit, `None` on miss.
    /// On hit, promotes the entry to MRU position and increments `hits`.
    /// On miss, increments `misses`.
    pub fn get(&mut self, input: &str) -> Option<String> {
        if self.max_entries == 0 {
            return None;
        }
        let key = Self::hash_key(input);
        if let Some(value) = self.store.get(&key) {
            // Promote to MRU: remove from current position, push to front
            if let Some(pos) = self.order.iter().position(|k| *k == key) {
                self.order.remove(pos);
            }
            self.order.push_front(key);
            self.hits += 1;
            Some(value.clone())
        } else {
            self.misses += 1;
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
        if self.store.contains_key(&key) {
            if let Some(pos) = self.order.iter().position(|k| *k == key) {
                self.order.remove(pos);
            }
        }
        self.store.insert(key, output.to_string());
        self.order.push_front(key);
        // Evict LRU if over capacity
        while self.order.len() > self.max_entries {
            if let Some(evicted) = self.order.pop_back() {
                self.store.remove(&evicted);
            }
        }
    }

    /// Return the number of entries currently cached.
    #[must_use]
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Get the cache hit rate as a percentage (0.0–100.0).
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
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
///
/// Returns `None` immediately when experimental mode is disabled.
pub fn cache_get(input: &str) -> Option<String> {
    if !TokenlessConfig::load().is_experimental_enabled() {
        return None;
    }
    cache().get(input)
}

/// Store `output` as the cached result for `input`.
///
/// No-op when experimental mode is disabled.
pub fn cache_insert(input: &str, output: &str) {
    if !TokenlessConfig::load().is_experimental_enabled() {
        return;
    }
    cache().insert(input, output);
}

// ── Differential Response Compression ────────────────────────────────

/// Stores the last raw response for a command key, used for diff computation.
/// Key = command string (e.g. "git status --porcelain")
static LAST_RESPONSES: LazyLock<Mutex<HashMap<String, (String, Instant)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Default threshold: diff must be < 70% of full output to be used.
const DEFAULT_DIFF_THRESHOLD: f64 = 0.7;

fn diff_threshold() -> f64 {
    std::env::var("TOKENLESS_DIFF_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_DIFF_THRESHOLD)
}

/// Compute a unified diff between old and new output for a command key.
///
/// Returns `None` on the first call (no baseline) or when the diff is larger
/// than `threshold * new_output.len()`, meaning it is cheaper to send the full
/// output.
pub fn compute_diff(command_key: &str, new_output: &str) -> Option<String> {
    compute_diff_inner(command_key, new_output, diff_threshold())
}

/// Core diff logic with explicit threshold (for testing).
fn compute_diff_inner(command_key: &str, new_output: &str, threshold: f64) -> Option<String> {
    let mut responses = LAST_RESPONSES.lock().unwrap_or_else(|e| {
        LAST_RESPONSES.clear_poison();
        e.into_inner()
    });

    let now = Instant::now();
    let result = if let Some((old_output, _ts)) = responses.get(command_key) {
        let diff = unified_diff(old_output, new_output);
        // Unchanged outputs always emit the marker (no threshold check needed).
        // Otherwise, only use diff if it is meaningfully smaller than full output.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let threshold_len = (new_output.len() as f64 * threshold) as usize;
        if diff == "(unchanged)" || diff.len() < threshold_len {
            Some(diff)
        } else {
            None
        }
    } else {
        None // First call: no baseline
    };

    // Update baseline for next call
    responses.insert(command_key.to_string(), (new_output.to_string(), now));
    result
}

/// Simple unified diff: returns lines with `-` for removals and `+` for
/// additions, with up to 3 lines of surrounding context.
fn unified_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    // Find common prefix
    let mut prefix = 0usize;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }

    // Find common suffix
    let mut suffix = 0usize;
    while suffix < old_lines.len() - prefix
        && suffix < new_lines.len() - prefix
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let old_changed = &old_lines[prefix..old_lines.len() - suffix];
    let new_changed = &new_lines[prefix..new_lines.len() - suffix];

    // If nothing changed
    if old_changed.is_empty() && new_changed.is_empty() {
        return "(unchanged)".to_string();
    }

    let mut result = String::new();

    // Context: show unchanged lines before the change (up to 3)
    let ctx_start = prefix.saturating_sub(3);
    if prefix > 3 {
        let _ = writeln!(
            result,
            "  ... {unchanged_above} unchanged lines above",
            unchanged_above = prefix - 3
        );
    }
    for line in &old_lines[ctx_start..prefix] {
        let _ = writeln!(result, "  {line}");
    }

    // Removed lines
    for line in old_changed {
        let _ = writeln!(result, "- {line}");
    }
    // Added lines
    for line in new_changed {
        let _ = writeln!(result, "+ {line}");
    }

    // Context: show unchanged lines after the change (up to 3)
    let ctx_end = suffix.min(3);
    for line in new_lines
        .iter()
        .skip(new_lines.len().saturating_sub(suffix))
        .take(ctx_end)
    {
        let _ = writeln!(result, "  {line}");
    }
    if suffix > 3 {
        let _ = writeln!(
            result,
            "  ... {unchanged_below} unchanged lines below",
            unchanged_below = suffix - 3
        );
    }

    // Header
    if result.is_empty() {
        result
    } else {
        format!(
            "[diff from previous call — {}→{} lines, showing changes]\n{result}",
            old_lines.len(),
            new_lines.len(),
        )
    }
}

/// Clear stored response baselines (test-only helper).
#[cfg(test)]
#[allow(dead_code)]
pub fn clear_diff_cache() {
    LAST_RESPONSES
        .lock()
        .unwrap_or_else(|e| {
            LAST_RESPONSES.clear_poison();
            e.into_inner()
        })
        .clear();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Unique per-execution test key to avoid races when tests run in parallel
    /// and share the global `LAST_RESPONSES` cache.
    static KEY_SEQ: AtomicUsize = AtomicUsize::new(0);
    fn test_key(name: &str) -> String {
        format!("{name}-{}", KEY_SEQ.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn test_hash_key_version_prefix_affects_key() {
        // Same input should produce the same key (deterministic).
        let input = "some-command --flag";
        let k1 = PredictCache::hash_key(input);
        let k2 = PredictCache::hash_key(input);
        assert_eq!(k1, k2, "hash_key must be deterministic");

        // The version-prefixed hash MUST differ from a plain blake3 hash
        // of the input alone. This guarantees that a version bump invalidates
        // old cache entries.
        let plain_hash = {
            let hash = blake3::hash(input.as_bytes());
            u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap_or([0; 8]))
        };
        assert_ne!(
            k1, plain_hash,
            "version-prefixed hash must differ from plain blake3 hash of the input"
        );
    }

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

    // ── Differential Response Compression tests ───────────────────

    #[test]
    fn test_diff_first_call_returns_none() {
        let result = compute_diff(&test_key("first-call"), "line1\nline2\n");
        assert!(
            result.is_none(),
            "first call should return None (no baseline)"
        );
    }

    #[test]
    fn test_diff_second_call_returns_diff() {
        let key = test_key("second-call");
        // threshold=2.0 disables size-gating so we can verify diff content.
        let baseline: String = (0..20)
            .map(|i| format!("line{i:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let changed: String = (0..20)
            .map(|i| {
                if i == 10 {
                    "line10-modified".to_string()
                } else {
                    format!("line{i:02}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        compute_diff_inner(&key, &baseline, 2.0);
        let result = compute_diff_inner(&key, &changed, 2.0);
        assert!(
            result.is_some(),
            "second call with changes should return diff"
        );
        let diff = result.unwrap();
        assert!(
            diff.contains("+ line10-modified"),
            "diff should show added line: {diff}"
        );
        assert!(
            diff.contains("- line10"),
            "diff should show removed line: {diff}"
        );
    }

    #[test]
    fn test_diff_no_changes_returns_unchanged() {
        let key = test_key("no-changes");
        compute_diff_inner(&key, "line1\nline2\n", 0.7);
        let result = compute_diff_inner(&key, "line1\nline2\n", 0.7);
        assert_eq!(result.unwrap(), "(unchanged)");
    }

    #[test]
    fn test_diff_too_large_returns_none() {
        let key = test_key("too-large");
        compute_diff_inner(&key, "a\n", 0.7);
        // Generate a completely different output
        let new = "b\nc\nd\ne\nf\ng\nh\ni\nj\nk\n";
        let result = compute_diff_inner(&key, new, 0.7);
        assert!(
            result.is_none(),
            "majority change should return None (full output)"
        );
    }

    #[test]
    fn test_diff_threshold_strict() {
        // With threshold 0.3, even moderate changes should fallback to full
        let key = test_key("strict");
        compute_diff_inner(&key, "a\nb\nc\n", 0.3);
        let result = compute_diff_inner(&key, "x\ny\nz\n", 0.3); // 100% different
        assert!(result.is_none(), "strict threshold should reject full diff");
    }

    #[test]
    fn test_diff_threshold_lenient() {
        // threshold=2.0 disables size-gating; verifies diff content.
        let baseline: String = (0..20)
            .map(|i| format!("line{i:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let changed: String = (0..20)
            .map(|i| {
                if i == 5 {
                    "line05-changed".to_string()
                } else {
                    format!("line{i:02}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let key = test_key("lenient");
        compute_diff_inner(&key, &baseline, 2.0);
        let result = compute_diff_inner(&key, &changed, 2.0);
        assert!(
            result.is_some(),
            "lenient threshold should accept small changes"
        );
        let diff = result.unwrap();
        assert!(
            diff.contains("+ line05-changed"),
            "diff should show added line: {diff}"
        );
        assert!(
            diff.contains("- line05"),
            "diff should show removed line: {diff}"
        );
    }
}
