//! # Thread / Isolate ID Mapping
//!
//! Provides two mapping types:
//!
//! - [`ThreadMap`] — Single-session bidirectional mapping between Dart VM
//!   isolate IDs (strings like `"isolates/12345"`) and DAP thread IDs
//!   (monotonically increasing integers starting at 1).
//!
//! - [`MultiSessionThreadMap`] — Multi-session thread ID namespace. Each
//!   session is assigned a thread ID range so that isolates from different
//!   sessions cannot collide:
//!   - Session 0: thread IDs 1000–1999
//!   - Session 1: thread IDs 2000–2999
//!   - Session 2: thread IDs 3000–3999
//!   - … up to Session 8 (thread IDs 9000–9999)
//!
//! DAP clients use integer thread IDs in all debugging requests; the VM Service
//! uses opaque string isolate IDs. These maps provide the translation layer.

use std::collections::HashMap;

use crate::protocol::types::DapThread;

// ─────────────────────────────────────────────────────────────────────────────
// Session ID type
// ─────────────────────────────────────────────────────────────────────────────

/// Opaque session identifier used by [`MultiSessionThreadMap`].
///
/// This type mirrors `fdemon-app`'s `SessionId = u64`. Using a plain integer
/// avoids adding an `uuid` dependency to this crate while remaining
/// unambiguous at call sites.
pub type DapSessionId = u64;

// ─────────────────────────────────────────────────────────────────────────────
// Thread ID namespacing constants
// ─────────────────────────────────────────────────────────────────────────────

/// Number of thread ID slots reserved per session.
///
/// Session 0 owns IDs 1000–1999, session 1 owns 2000–2999, etc.
/// This matches the `SessionManager`'s maximum of 9 concurrent sessions.
pub const THREADS_PER_SESSION: i64 = 1000;

/// Maximum number of concurrent sessions supported.
///
/// Mirrors `SessionManager`'s limit. Session indices 0..MAX_SESSIONS are valid.
pub const MAX_SESSIONS: usize = 9;

/// Calculate the base thread ID for the given session index.
///
/// # Formula
///
/// `(session_index + 1) * THREADS_PER_SESSION`
///
/// So session 0 starts at 1000, session 1 at 2000, etc.
pub fn session_thread_base(session_index: usize) -> i64 {
    (session_index as i64 + 1) * THREADS_PER_SESSION
}

/// Determine the session index from a namespaced DAP thread ID.
///
/// # Formula
///
/// `(thread_id / THREADS_PER_SESSION) - 1`
///
/// So thread ID 1042 → session 0, thread ID 2001 → session 1, etc.
///
/// Returns `usize::MAX` for thread IDs below `THREADS_PER_SESSION` (i.e.,
/// legacy non-namespaced IDs), which callers should treat as invalid.
pub fn session_index_from_thread_id(thread_id: i64) -> usize {
    if thread_id < THREADS_PER_SESSION {
        return usize::MAX;
    }
    (thread_id / THREADS_PER_SESSION - 1) as usize
}

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
// SessionThreads — per-session thread state
// ─────────────────────────────────────────────────────────────────────────────

/// Per-session thread tracking.
///
/// Each session owns a range of thread IDs:
/// `[thread_base, thread_base + THREADS_PER_SESSION)`.
/// Isolates within the session are assigned IDs starting at `thread_base` and
/// incrementing monotonically.
struct SessionThreads {
    /// Stable unique identifier for this session.
    session_id: DapSessionId,
    /// Human-readable session name (e.g., `"Pixel 7"` or `"Chrome"`).
    session_name: String,
    /// First thread ID in this session's range (e.g., 1000 for session 0).
    thread_base: i64,
    /// Next local ID offset to assign within this session's range.
    next_local_id: i64,
    /// Maps isolate ID → global thread ID (namespaced).
    isolate_to_thread: HashMap<String, i64>,
    /// Maps global thread ID → isolate ID.
    thread_to_isolate: HashMap<i64, String>,
    /// Maps global thread ID → human-readable display name (already prefixed).
    thread_names: HashMap<i64, String>,
}

impl SessionThreads {
    /// Create a new session thread tracker.
    ///
    /// `session_index` determines the thread ID range:
    /// - 0 → IDs 1000–1999
    /// - 1 → IDs 2000–2999
    /// - …
    fn new(session_id: DapSessionId, session_name: String, session_index: usize) -> Self {
        let thread_base = session_thread_base(session_index);
        Self {
            session_id,
            session_name,
            thread_base,
            next_local_id: 0,
            isolate_to_thread: HashMap::new(),
            thread_to_isolate: HashMap::new(),
            thread_names: HashMap::new(),
        }
    }

    /// Get or create a namespaced thread ID for the given isolate.
    ///
    /// Returns the global (namespaced) thread ID. If the isolate already has a
    /// thread ID, that ID is returned unchanged.
    fn get_or_create(&mut self, isolate_id: &str) -> i64 {
        if let Some(&id) = self.isolate_to_thread.get(isolate_id) {
            return id;
        }
        let global_id = self.thread_base + self.next_local_id;
        self.next_local_id += 1;
        self.isolate_to_thread
            .insert(isolate_id.to_string(), global_id);
        self.thread_to_isolate
            .insert(global_id, isolate_id.to_string());
        global_id
    }

    /// Remove an isolate from this session's mappings.
    ///
    /// Returns the global thread ID that was assigned, or `None` if the
    /// isolate was not registered. The thread ID slot is not recycled.
    fn remove(&mut self, isolate_id: &str) -> Option<i64> {
        if let Some(id) = self.isolate_to_thread.remove(isolate_id) {
            self.thread_to_isolate.remove(&id);
            self.thread_names.remove(&id);
            Some(id)
        } else {
            None
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MultiSessionThreadMap
// ─────────────────────────────────────────────────────────────────────────────

/// Thread ID map that supports multiple concurrent Flutter sessions.
///
/// Each session gets a dedicated thread ID range so that isolates from different
/// sessions cannot have colliding IDs:
///
/// | Session index | Thread ID range |
/// |:---:|:---:|
/// | 0 | 1000–1999 |
/// | 1 | 2000–2999 |
/// | 2 | 3000–3999 |
/// | … | … |
/// | 8 | 9000–9999 |
///
/// The `threads` response aggregates all sessions and prefixes each thread name
/// with the session name in brackets, e.g.:
///
/// ```text
/// { "id": 1000, "name": "[Pixel 7] main" }
/// { "id": 2000, "name": "[Chrome] main" }
/// ```
///
/// # Single-session Compatibility
///
/// In single-session mode thread IDs start at 1000 (session index 0).
/// The `DapAdapter` continues to use [`ThreadMap`] for its own single-session
/// state; this type is used by higher-level aggregation logic.
pub struct MultiSessionThreadMap {
    /// Per-session state, in insertion order (mirrors session index).
    sessions: Vec<SessionThreads>,
}

impl MultiSessionThreadMap {
    /// Create an empty [`MultiSessionThreadMap`].
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    /// Add a new session to the map.
    ///
    /// The session is assigned the next available index (0, 1, 2, …). Returns
    /// the session's thread base so the caller knows where thread IDs start.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if `sessions.len() >= MAX_SESSIONS`.
    pub fn add_session(
        &mut self,
        session_id: DapSessionId,
        session_name: impl Into<String>,
    ) -> i64 {
        let index = self.sessions.len();
        debug_assert!(
            index < MAX_SESSIONS,
            "Too many sessions: max {} but tried to add session index {}",
            MAX_SESSIONS,
            index,
        );
        let session = SessionThreads::new(session_id, session_name.into(), index);
        let base = session.thread_base;
        self.sessions.push(session);
        base
    }

    /// Remove a session by its ID.
    ///
    /// Returns all global thread IDs that were active in the removed session
    /// so callers can emit `thread exited` events for each.
    ///
    /// The removed session's entry is dropped and the remaining sessions keep
    /// their original indices and thread ID ranges.
    pub fn remove_session(&mut self, session_id: DapSessionId) -> Vec<i64> {
        if let Some(pos) = self
            .sessions
            .iter()
            .position(|s| s.session_id == session_id)
        {
            let removed = self.sessions.remove(pos);
            removed.thread_to_isolate.into_keys().collect()
        } else {
            Vec::new()
        }
    }

    /// Register or look up an isolate within the given session.
    ///
    /// Sets an optional display name that will be prefixed with the session
    /// name (e.g., `"[Pixel 7] main"`). If `isolate_name` is `None`, a
    /// default name is generated when the thread list is queried.
    ///
    /// Returns the global (namespaced) thread ID, or `None` if the session
    /// ID is not registered.
    pub fn add_isolate(
        &mut self,
        session_id: DapSessionId,
        isolate_id: &str,
        isolate_name: Option<&str>,
    ) -> Option<i64> {
        let session = self.session_mut(session_id)?;
        let global_id = session.get_or_create(isolate_id);
        if let Some(name) = isolate_name {
            let display = format!("[{}] {}", session.session_name, name);
            session.thread_names.insert(global_id, display);
        }
        Some(global_id)
    }

    /// Remove an isolate from the given session.
    ///
    /// Returns the global thread ID that was assigned to the isolate, or `None`
    /// if the session or isolate was not found.
    pub fn remove_isolate(&mut self, session_id: DapSessionId, isolate_id: &str) -> Option<i64> {
        self.session_mut(session_id)
            .and_then(|s| s.remove(isolate_id))
    }

    /// Look up the global thread ID for an isolate within a specific session.
    pub fn thread_id_for_isolate(&self, session_id: DapSessionId, isolate_id: &str) -> Option<i64> {
        self.session(session_id)
            .and_then(|s| s.isolate_to_thread.get(isolate_id).copied())
    }

    /// Look up the session and isolate ID for a global thread ID.
    ///
    /// Returns `(session_id, isolate_id)` if found, `None` otherwise.
    ///
    /// This is the primary routing function: given a thread ID from a DAP
    /// request (e.g., `continue`, `stepIn`), find which session backend
    /// should handle the request and which isolate to target.
    pub fn lookup_thread(&self, thread_id: i64) -> Option<(DapSessionId, &str)> {
        // Use index arithmetic to locate the session quickly.
        let session_idx = session_index_from_thread_id(thread_id);
        if session_idx == usize::MAX {
            return None;
        }
        // The session at position `session_idx` in the Vec owns this thread range,
        // provided its thread_base matches the expected value.
        //
        // After `remove_session` the Vec is compacted, so indices shift.
        // We therefore search by thread_base to handle gaps correctly.
        let expected_base = session_thread_base(session_idx);
        for session in &self.sessions {
            if session.thread_base == expected_base {
                if let Some(iso) = session.thread_to_isolate.get(&thread_id) {
                    return Some((session.session_id, iso.as_str()));
                }
            }
        }
        None
    }

    /// Return a flattened list of all threads across all sessions.
    ///
    /// Each thread name is prefixed with the owning session's name in brackets.
    /// Threads are sorted by ID for deterministic output.
    pub fn all_threads(&self) -> Vec<DapThread> {
        let mut threads = Vec::new();
        for session in &self.sessions {
            for &id in session.thread_to_isolate.keys() {
                let name = session
                    .thread_names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| format!("[{}] Thread {}", session.session_name, id));
                threads.push(DapThread { id, name });
            }
        }
        threads.sort_by_key(|t| t.id);
        threads
    }

    /// Return the number of registered sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Return the total number of threads across all sessions.
    pub fn total_thread_count(&self) -> usize {
        self.sessions
            .iter()
            .map(|s| s.thread_to_isolate.len())
            .sum()
    }

    /// Return `true` if there are no sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    fn session(&self, session_id: DapSessionId) -> Option<&SessionThreads> {
        self.sessions.iter().find(|s| s.session_id == session_id)
    }

    fn session_mut(&mut self, session_id: DapSessionId) -> Option<&mut SessionThreads> {
        self.sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
    }
}

impl Default for MultiSessionThreadMap {
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

    // ── ThreadMap tests ────────────────────────────────────────────────────

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

    // ── session_thread_base / session_index_from_thread_id ─────────────────

    #[test]
    fn test_thread_id_namespacing_base_values() {
        // Verify the fundamental namespace formula.
        assert_eq!(session_thread_base(0), 1000);
        assert_eq!(session_thread_base(1), 2000);
        assert_eq!(session_thread_base(2), 3000);
        assert_eq!(session_thread_base(8), 9000);
    }

    #[test]
    fn test_thread_id_namespacing_reverse_lookup() {
        // Verify that session_index_from_thread_id inverts session_thread_base.
        assert_eq!(session_index_from_thread_id(1042), 0);
        assert_eq!(session_index_from_thread_id(2001), 1);
        assert_eq!(session_index_from_thread_id(9000), 8);
        assert_eq!(session_index_from_thread_id(9999), 8);
    }

    #[test]
    fn test_session_thread_base_session_0() {
        assert_eq!(session_thread_base(0), 1000);
    }

    #[test]
    fn test_session_thread_base_session_1() {
        assert_eq!(session_thread_base(1), 2000);
    }

    #[test]
    fn test_session_thread_base_session_8() {
        assert_eq!(session_thread_base(8), 9000);
    }

    #[test]
    fn test_session_index_from_thread_id_lower_bound_session_0() {
        assert_eq!(session_index_from_thread_id(1000), 0);
    }

    #[test]
    fn test_session_index_from_thread_id_mid_session_0() {
        assert_eq!(session_index_from_thread_id(1042), 0);
    }

    #[test]
    fn test_session_index_from_thread_id_upper_bound_session_0() {
        assert_eq!(session_index_from_thread_id(1999), 0);
    }

    #[test]
    fn test_session_index_from_thread_id_session_1() {
        assert_eq!(session_index_from_thread_id(2000), 1);
        assert_eq!(session_index_from_thread_id(2001), 1);
        assert_eq!(session_index_from_thread_id(2999), 1);
    }

    #[test]
    fn test_session_index_from_thread_id_session_8() {
        assert_eq!(session_index_from_thread_id(9000), 8);
    }

    #[test]
    fn test_session_index_from_thread_id_legacy_returns_max() {
        // IDs below THREADS_PER_SESSION (1000) are legacy/invalid.
        assert_eq!(session_index_from_thread_id(1), usize::MAX);
        assert_eq!(session_index_from_thread_id(999), usize::MAX);
    }

    // ── MultiSessionThreadMap ─────────────────────────────────────────────

    #[test]
    fn test_multi_session_map_starts_empty() {
        let map = MultiSessionThreadMap::new();
        assert!(map.is_empty());
        assert_eq!(map.session_count(), 0);
        assert_eq!(map.total_thread_count(), 0);
    }

    #[test]
    fn test_multi_session_map_add_session_assigns_correct_base() {
        let mut map = MultiSessionThreadMap::new();
        let base0 = map.add_session(1, "Pixel 7");
        let base1 = map.add_session(2, "Chrome");
        assert_eq!(base0, 1000, "Session 0 should start at 1000");
        assert_eq!(base1, 2000, "Session 1 should start at 2000");
    }

    #[test]
    fn test_multi_session_map_add_isolate_returns_namespaced_id() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Pixel 7");
        let thread_id = map.add_isolate(1, "isolates/1", Some("main")).unwrap();
        assert!(
            (1000..2000).contains(&thread_id),
            "Thread ID {} must be in session 0 range [1000, 2000)",
            thread_id
        );
    }

    #[test]
    fn test_multi_session_map_two_sessions_no_id_collision() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Pixel 7");
        map.add_session(2, "Chrome");

        let tid0 = map.add_isolate(1, "isolates/1", Some("main")).unwrap();
        let tid1 = map.add_isolate(2, "isolates/1", Some("main")).unwrap();

        assert_ne!(
            tid0, tid1,
            "Same isolate ID in different sessions must get different thread IDs"
        );
        assert!(
            (1000..2000).contains(&tid0),
            "Session 0 thread in range [1000, 2000)"
        );
        assert!(
            (2000..3000).contains(&tid1),
            "Session 1 thread in range [2000, 3000)"
        );
    }

    #[test]
    fn test_multi_session_threads_response_has_prefixes() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Pixel 7");
        map.add_session(2, "Chrome");
        map.add_isolate(1, "isolates/1", Some("main"));
        map.add_isolate(2, "isolates/2", Some("main"));

        let threads = map.all_threads();
        assert_eq!(threads.len(), 2);
        // Thread names must include session prefix.
        let pixel_thread = threads.iter().find(|t| t.name.contains("[Pixel 7]"));
        let chrome_thread = threads.iter().find(|t| t.name.contains("[Chrome]"));
        assert!(
            pixel_thread.is_some(),
            "Expected [Pixel 7] prefix in thread names: {:?}",
            threads
        );
        assert!(
            chrome_thread.is_some(),
            "Expected [Chrome] prefix in thread names: {:?}",
            threads
        );
    }

    #[test]
    fn test_multi_session_map_all_threads_sorted_by_id() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "A");
        map.add_session(2, "B");
        map.add_isolate(2, "isolates/2", Some("main")); // session 1 first
        map.add_isolate(1, "isolates/1", Some("main")); // session 0 second

        let threads = map.all_threads();
        assert_eq!(threads.len(), 2);
        assert!(
            threads[0].id < threads[1].id,
            "Threads must be sorted by ID"
        );
    }

    #[test]
    fn test_multi_session_map_lookup_thread_returns_correct_session() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(10, "Pixel 7");
        map.add_session(20, "Chrome");
        let tid0 = map.add_isolate(10, "isolates/1", Some("main")).unwrap();
        let tid1 = map.add_isolate(20, "isolates/1", Some("main")).unwrap();

        let (found_sid0, iso0) = map.lookup_thread(tid0).unwrap();
        let (found_sid1, iso1) = map.lookup_thread(tid1).unwrap();

        assert_eq!(found_sid0, 10, "Thread {} should route to session 10", tid0);
        assert_eq!(found_sid1, 20, "Thread {} should route to session 20", tid1);
        assert_eq!(iso0, "isolates/1");
        assert_eq!(iso1, "isolates/1");
    }

    #[test]
    fn test_multi_session_map_lookup_thread_unknown_returns_none() {
        let map = MultiSessionThreadMap::new();
        assert!(map.lookup_thread(1000).is_none());
        assert!(map.lookup_thread(9999).is_none());
    }

    #[test]
    fn test_multi_session_map_remove_isolate_clears_thread() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Pixel 7");
        let tid = map.add_isolate(1, "isolates/1", Some("main")).unwrap();
        let removed = map.remove_isolate(1, "isolates/1");
        assert_eq!(removed, Some(tid));
        assert!(
            map.lookup_thread(tid).is_none(),
            "Removed thread must not be found"
        );
        assert_eq!(map.total_thread_count(), 0);
    }

    #[test]
    fn test_multi_session_map_remove_session_returns_thread_ids() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Pixel 7");
        let tid1 = map.add_isolate(1, "isolates/1", Some("main")).unwrap();
        let tid2 = map.add_isolate(1, "isolates/2", Some("worker")).unwrap();

        let mut removed = map.remove_session(1);
        removed.sort();

        assert_eq!(removed.len(), 2, "Should return 2 thread IDs");
        assert!(removed.contains(&tid1));
        assert!(removed.contains(&tid2));
        assert_eq!(map.session_count(), 0);
    }

    #[test]
    fn test_multi_session_map_thread_id_stable_across_calls() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Pixel 7");
        let id1 = map.add_isolate(1, "isolates/42", Some("main")).unwrap();
        let id2 = map.add_isolate(1, "isolates/42", Some("main")).unwrap();
        assert_eq!(id1, id2, "Thread ID for same isolate must be stable");
    }

    #[test]
    fn test_multi_session_map_thread_name_has_bracket_prefix() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Nexus 5");
        map.add_isolate(1, "isolates/1", Some("main"));

        let threads = map.all_threads();
        assert_eq!(threads.len(), 1);
        assert!(
            threads[0].name.starts_with("[Nexus 5]"),
            "Thread name '{}' should start with '[Nexus 5]'",
            threads[0].name
        );
    }

    #[test]
    fn test_multi_session_map_fallback_name_when_no_isolate_name() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Pixel 7");
        // Add isolate with no name.
        map.add_isolate(1, "isolates/1", None);

        let threads = map.all_threads();
        assert_eq!(threads.len(), 1);
        // Fallback name should still have session prefix.
        assert!(
            threads[0].name.contains("[Pixel 7]"),
            "Fallback name '{}' should contain session prefix",
            threads[0].name
        );
    }

    #[test]
    fn test_multi_session_map_session_index_routing() {
        // Verify that session_index_from_thread_id correctly routes to the
        // session that owns the thread.
        let mut map = MultiSessionThreadMap::new();
        map.add_session(100, "A");
        map.add_session(200, "B");
        let tid0 = map.add_isolate(100, "isolates/1", Some("main")).unwrap();
        let tid1 = map.add_isolate(200, "isolates/1", Some("main")).unwrap();

        let idx0 = session_index_from_thread_id(tid0);
        let idx1 = session_index_from_thread_id(tid1);
        assert_eq!(idx0, 0, "Thread {} should be in session 0", tid0);
        assert_eq!(idx1, 1, "Thread {} should be in session 1", tid1);
    }

    #[test]
    fn test_multi_session_map_total_thread_count() {
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "A");
        map.add_session(2, "B");

        assert_eq!(map.total_thread_count(), 0);
        map.add_isolate(1, "isolates/1", Some("main"));
        assert_eq!(map.total_thread_count(), 1);
        map.add_isolate(1, "isolates/2", Some("worker"));
        assert_eq!(map.total_thread_count(), 2);
        map.add_isolate(2, "isolates/3", Some("main"));
        assert_eq!(map.total_thread_count(), 3);
    }

    #[test]
    fn test_route_to_correct_session_by_thread_id() {
        // Thread ID 1000 → session 0, Thread ID 2000 → session 1.
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Pixel 7");
        map.add_session(2, "Chrome");
        let tid_session0 = map.add_isolate(1, "isolates/a", Some("main")).unwrap();
        let tid_session1 = map.add_isolate(2, "isolates/b", Some("main")).unwrap();

        // Confirm IDs are in the expected ranges.
        assert!((1000..2000).contains(&tid_session0));
        assert!((2000..3000).contains(&tid_session1));

        // Routing returns the correct session.
        let (sid0, iso0) = map.lookup_thread(tid_session0).unwrap();
        let (sid1, iso1) = map.lookup_thread(tid_session1).unwrap();
        assert_eq!(sid0, 1);
        assert_eq!(sid1, 2);
        assert_eq!(iso0, "isolates/a");
        assert_eq!(iso1, "isolates/b");
    }

    #[test]
    fn test_stepping_one_session_does_not_affect_other() {
        // Verify that thread IDs from different sessions are disjoint.
        let mut map = MultiSessionThreadMap::new();
        map.add_session(1, "Session A");
        map.add_session(2, "Session B");

        let tid_a = map.add_isolate(1, "isolates/1", Some("main")).unwrap();
        let tid_b = map.add_isolate(2, "isolates/1", Some("main")).unwrap();

        // Thread A routes only to session 1.
        let (sid_a, _) = map.lookup_thread(tid_a).unwrap();
        assert_eq!(sid_a, 1);
        // Thread B routes only to session 2.
        let (sid_b, _) = map.lookup_thread(tid_b).unwrap();
        assert_eq!(sid_b, 2);
        // A is NOT in session 2's range.
        assert!(
            session_index_from_thread_id(tid_a) != session_index_from_thread_id(tid_b),
            "Different sessions must use different index ranges"
        );
    }
}
