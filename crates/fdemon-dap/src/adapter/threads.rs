//! # Thread / Isolate ID Mapping
//!
//! Provides [`ThreadMap`] which maintains a bidirectional mapping between
//! Dart VM isolate IDs (strings like `"isolates/12345"`) and DAP thread IDs
//! (monotonically increasing integers starting at 1).
//!
//! DAP clients use integer thread IDs in all debugging requests; the VM Service
//! uses opaque string isolate IDs. This map provides the translation layer.

use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// ThreadMap
// ─────────────────────────────────────────────────────────────────────────────

/// Bidirectional mapping between Dart isolate IDs and DAP thread IDs.
///
/// Thread IDs are monotonically increasing integers, starting at 1. Once an
/// isolate is assigned a thread ID, that assignment persists for the lifetime
/// of the session (even after the isolate exits).
///
/// # ID Stability
///
/// Thread IDs are never recycled. If isolate A exits and a new isolate B
/// starts, B receives a fresh ID rather than reusing A's. This matches the
/// DAP specification's expectation that thread IDs are stable references.
pub struct ThreadMap {
    /// Maps isolate ID → DAP thread ID.
    isolate_to_thread: HashMap<String, i64>,
    /// Maps DAP thread ID → isolate ID.
    thread_to_isolate: HashMap<i64, String>,
    /// Next thread ID to assign (1-based, monotonically increasing).
    next_id: i64,
}

impl ThreadMap {
    /// Create an empty [`ThreadMap`]. The next assigned ID will be 1.
    pub fn new() -> Self {
        Self {
            isolate_to_thread: HashMap::new(),
            thread_to_isolate: HashMap::new(),
            next_id: 1,
        }
    }

    /// Get or create a DAP thread ID for the given isolate ID.
    ///
    /// If the isolate already has an assigned thread ID, returns it unchanged.
    /// Otherwise, assigns the next monotonic ID and stores the mapping.
    ///
    /// # Returns
    ///
    /// The DAP thread ID (always >= 1).
    pub fn get_or_create(&mut self, isolate_id: &str) -> i64 {
        if let Some(&id) = self.isolate_to_thread.get(isolate_id) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.isolate_to_thread.insert(isolate_id.to_string(), id);
        self.thread_to_isolate.insert(id, isolate_id.to_string());
        id
    }

    /// Look up the DAP thread ID for an isolate ID.
    ///
    /// Returns `None` if the isolate has not been registered yet.
    pub fn thread_id_for(&self, isolate_id: &str) -> Option<i64> {
        self.isolate_to_thread.get(isolate_id).copied()
    }

    /// Look up the isolate ID for a DAP thread ID.
    ///
    /// Returns `None` if the thread ID has not been assigned.
    pub fn isolate_id_for(&self, thread_id: i64) -> Option<&str> {
        self.thread_to_isolate.get(&thread_id).map(String::as_str)
    }

    /// Return all currently registered threads as `(thread_id, isolate_id)` pairs.
    ///
    /// The order of iteration is unspecified.
    pub fn all_threads(&self) -> impl Iterator<Item = (i64, &str)> {
        self.thread_to_isolate
            .iter()
            .map(|(&id, isolate)| (id, isolate.as_str()))
    }

    /// Remove an isolate mapping (call when an isolate exits).
    ///
    /// Returns the DAP thread ID that was assigned to the isolate, or `None`
    /// if the isolate was not registered. The thread ID is **not** recycled;
    /// subsequent isolates receive higher IDs.
    pub fn remove(&mut self, isolate_id: &str) -> Option<i64> {
        if let Some(id) = self.isolate_to_thread.remove(isolate_id) {
            self.thread_to_isolate.remove(&id);
            Some(id)
        } else {
            None
        }
    }

    /// Return the total number of registered threads.
    pub fn len(&self) -> usize {
        self.isolate_to_thread.len()
    }

    /// Return `true` if no threads have been registered.
    pub fn is_empty(&self) -> bool {
        self.isolate_to_thread.is_empty()
    }
}

impl Default for ThreadMap {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_map_starts_empty() {
        let map = ThreadMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_thread_map_allocates_monotonic_ids() {
        let mut map = ThreadMap::new();
        let id1 = map.get_or_create("isolates/1");
        let id2 = map.get_or_create("isolates/2");
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_thread_map_ids_start_at_one() {
        let mut map = ThreadMap::new();
        let id = map.get_or_create("isolates/100");
        assert_eq!(id, 1, "First assigned ID must be 1");
    }

    #[test]
    fn test_thread_map_reuses_existing_id() {
        let mut map = ThreadMap::new();
        let id1 = map.get_or_create("isolates/1");
        let id2 = map.get_or_create("isolates/1");
        assert_eq!(id1, id2, "Same isolate must always get the same thread ID");
    }

    #[test]
    fn test_thread_map_different_isolates_get_different_ids() {
        let mut map = ThreadMap::new();
        let id1 = map.get_or_create("isolates/1");
        let id2 = map.get_or_create("isolates/2");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_thread_id_for_returns_none_for_unknown_isolate() {
        let map = ThreadMap::new();
        assert!(map.thread_id_for("isolates/999").is_none());
    }

    #[test]
    fn test_thread_id_for_returns_correct_id() {
        let mut map = ThreadMap::new();
        let id = map.get_or_create("isolates/42");
        assert_eq!(map.thread_id_for("isolates/42"), Some(id));
    }

    #[test]
    fn test_isolate_id_for_returns_none_for_unknown_thread() {
        let map = ThreadMap::new();
        assert!(map.isolate_id_for(99).is_none());
    }

    #[test]
    fn test_isolate_id_for_returns_correct_isolate() {
        let mut map = ThreadMap::new();
        let id = map.get_or_create("isolates/42");
        assert_eq!(map.isolate_id_for(id), Some("isolates/42"));
    }

    #[test]
    fn test_thread_map_bidirectional_consistency() {
        let mut map = ThreadMap::new();
        let thread_id = map.get_or_create("isolates/7");
        let isolate_id = map.isolate_id_for(thread_id).unwrap();
        let back = map.thread_id_for(isolate_id).unwrap();
        assert_eq!(back, thread_id);
    }

    #[test]
    fn test_thread_map_len_tracks_registrations() {
        let mut map = ThreadMap::new();
        assert_eq!(map.len(), 0);
        map.get_or_create("isolates/1");
        assert_eq!(map.len(), 1);
        map.get_or_create("isolates/2");
        assert_eq!(map.len(), 2);
        // Duplicate doesn't increase len.
        map.get_or_create("isolates/1");
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_thread_map_all_threads_returns_all_registered() {
        let mut map = ThreadMap::new();
        let id1 = map.get_or_create("isolates/1");
        let id2 = map.get_or_create("isolates/2");

        let mut threads: Vec<(i64, String)> = map
            .all_threads()
            .map(|(id, iso)| (id, iso.to_string()))
            .collect();
        threads.sort_by_key(|(id, _)| *id);

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0], (id1, "isolates/1".to_string()));
        assert_eq!(threads[1], (id2, "isolates/2".to_string()));
    }

    #[test]
    fn test_thread_map_monotonic_sequence_after_multiple_isolates() {
        let mut map = ThreadMap::new();
        for i in 1..=10 {
            let id = map.get_or_create(&format!("isolates/{}", i));
            assert_eq!(id, i as i64, "ID for isolate {} should be {}", i, i);
        }
    }

    // ── remove ────────────────────────────────────────────────────────────

    #[test]
    fn test_thread_map_monotonic_after_removal() {
        let mut map = ThreadMap::new();
        let id1 = map.get_or_create("isolates/1");
        map.remove("isolates/1");
        let id2 = map.get_or_create("isolates/2");
        assert!(id2 > id1, "IDs must be monotonic even after removal");
    }

    #[test]
    fn test_thread_map_all_threads_after_removal() {
        let mut map = ThreadMap::new();
        map.get_or_create("isolates/1");
        map.get_or_create("isolates/2");
        let threads: Vec<_> = map.all_threads().collect();
        assert_eq!(threads.len(), 2);
    }

    #[test]
    fn test_thread_map_remove_returns_thread_id() {
        let mut map = ThreadMap::new();
        let id = map.get_or_create("isolates/1");
        let removed = map.remove("isolates/1");
        assert_eq!(removed, Some(id));
    }

    #[test]
    fn test_thread_map_remove_unknown_returns_none() {
        let mut map = ThreadMap::new();
        assert_eq!(map.remove("isolates/99"), None);
    }

    #[test]
    fn test_thread_map_remove_cleans_both_sides() {
        let mut map = ThreadMap::new();
        let id = map.get_or_create("isolates/5");
        map.remove("isolates/5");
        // Both directions should be cleared.
        assert!(map.thread_id_for("isolates/5").is_none());
        assert!(map.isolate_id_for(id).is_none());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_thread_map_remove_only_affects_target_isolate() {
        let mut map = ThreadMap::new();
        let id1 = map.get_or_create("isolates/1");
        let id2 = map.get_or_create("isolates/2");
        map.remove("isolates/1");
        // isolates/2 should still be mapped.
        assert_eq!(map.thread_id_for("isolates/2"), Some(id2));
        assert_eq!(map.isolate_id_for(id2), Some("isolates/2"));
        assert_eq!(map.len(), 1);
        // isolates/1 is gone from both directions.
        assert!(map.thread_id_for("isolates/1").is_none());
        assert!(map.isolate_id_for(id1).is_none());
    }

    #[test]
    fn test_thread_map_double_remove_returns_none() {
        let mut map = ThreadMap::new();
        map.get_or_create("isolates/1");
        let first = map.remove("isolates/1");
        let second = map.remove("isolates/1");
        assert!(first.is_some());
        assert!(second.is_none(), "Double removal must return None");
    }

    #[test]
    fn test_thread_map_removed_id_not_reused_on_re_register() {
        let mut map = ThreadMap::new();
        let id1 = map.get_or_create("isolates/1");
        map.remove("isolates/1");
        // Re-registering the *same* isolate gives a new (higher) ID.
        let id2 = map.get_or_create("isolates/1");
        assert!(
            id2 > id1,
            "Re-registered isolate must get a fresh thread ID"
        );
    }
}
