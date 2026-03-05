//! # Breakpoint State
//!
//! Provides [`BreakpointState`] which tracks the mapping between DAP
//! breakpoint IDs (integers) and VM Service breakpoint IDs (strings), and
//! records resolution status for each breakpoint.
//!
//! Breakpoints are set via `setBreakpoints` DAP requests (implemented in
//! Task 05). This module provides the scaffolding needed to track them.

use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// BreakpointEntry
// ─────────────────────────────────────────────────────────────────────────────

/// A tracked breakpoint: DAP ID ↔ VM Service ID mapping plus resolution state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakpointEntry {
    /// DAP breakpoint ID (assigned by the adapter, integer, 1-based).
    pub dap_id: i64,
    /// VM Service breakpoint ID (assigned by the Dart VM, opaque string).
    pub vm_id: String,
    /// The source URI this breakpoint was set in.
    pub uri: String,
    /// The requested line (1-based).
    pub line: Option<i32>,
    /// The requested column (1-based), if specified.
    pub column: Option<i32>,
    /// Whether the VM has confirmed this breakpoint is at a valid location.
    pub verified: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// BreakpointState
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks all active breakpoints across sources.
///
/// Maintains two indices for O(1) lookup:
/// - DAP ID → entry (for client-initiated operations)
/// - VM ID → DAP ID (for VM-initiated resolution events)
///
/// # Lifecycle
///
/// When `setBreakpoints` is called for a source, the adapter first removes all
/// existing breakpoints for that source, then adds the new set. This replaces
/// the full set atomically from the client's perspective.
pub struct BreakpointState {
    /// All tracked breakpoints, keyed by DAP breakpoint ID.
    by_dap_id: HashMap<i64, BreakpointEntry>,
    /// Maps VM Service breakpoint ID → DAP breakpoint ID.
    vm_id_to_dap_id: HashMap<String, i64>,
    /// Next DAP breakpoint ID to assign (1-based, monotonically increasing).
    next_dap_id: i64,
}

impl BreakpointState {
    /// Create a new empty [`BreakpointState`].
    pub fn new() -> Self {
        Self {
            by_dap_id: HashMap::new(),
            vm_id_to_dap_id: HashMap::new(),
            next_dap_id: 1,
        }
    }

    /// Register a new breakpoint and return its assigned DAP ID.
    ///
    /// The `vm_id` is the ID returned by the VM Service after adding the
    /// breakpoint. The `line` and `column` are the actual location (which may
    /// differ from the requested location).
    pub fn add(
        &mut self,
        vm_id: impl Into<String>,
        uri: impl Into<String>,
        line: Option<i32>,
        column: Option<i32>,
        verified: bool,
    ) -> i64 {
        let dap_id = self.next_dap_id;
        self.next_dap_id += 1;
        let vm_id = vm_id.into();

        self.vm_id_to_dap_id.insert(vm_id.clone(), dap_id);
        self.by_dap_id.insert(
            dap_id,
            BreakpointEntry {
                dap_id,
                vm_id,
                uri: uri.into(),
                line,
                column,
                verified,
            },
        );
        dap_id
    }

    /// Remove a breakpoint by its DAP ID.
    ///
    /// Returns the removed entry, or `None` if no breakpoint with that ID exists.
    pub fn remove_by_dap_id(&mut self, dap_id: i64) -> Option<BreakpointEntry> {
        if let Some(entry) = self.by_dap_id.remove(&dap_id) {
            self.vm_id_to_dap_id.remove(&entry.vm_id);
            Some(entry)
        } else {
            None
        }
    }

    /// Remove all breakpoints for a given source URI.
    ///
    /// Returns the list of removed entries (in unspecified order). The caller
    /// should remove each from the VM Service using the `vm_id` field.
    pub fn remove_all_for_uri(&mut self, uri: &str) -> Vec<BreakpointEntry> {
        let to_remove: Vec<i64> = self
            .by_dap_id
            .values()
            .filter(|e| e.uri == uri)
            .map(|e| e.dap_id)
            .collect();

        to_remove
            .into_iter()
            .filter_map(|id| self.remove_by_dap_id(id))
            .collect()
    }

    /// Look up a breakpoint by its DAP ID.
    pub fn lookup_by_dap_id(&self, dap_id: i64) -> Option<&BreakpointEntry> {
        self.by_dap_id.get(&dap_id)
    }

    /// Look up a breakpoint by its VM Service ID.
    pub fn lookup_by_vm_id(&self, vm_id: &str) -> Option<&BreakpointEntry> {
        let dap_id = self.vm_id_to_dap_id.get(vm_id)?;
        self.by_dap_id.get(dap_id)
    }

    /// Mark a breakpoint as resolved and update its location.
    ///
    /// Called when a `BreakpointResolved` VM Service event arrives. Returns
    /// a reference to the updated entry, or `None` if the VM ID is not tracked.
    pub fn resolve_breakpoint(
        &mut self,
        vm_id: &str,
        line: Option<i32>,
        column: Option<i32>,
    ) -> Option<&BreakpointEntry> {
        let dap_id = *self.vm_id_to_dap_id.get(vm_id)?;
        if let Some(entry) = self.by_dap_id.get_mut(&dap_id) {
            entry.verified = true;
            if line.is_some() {
                entry.line = line;
            }
            if column.is_some() {
                entry.column = column;
            }
        }
        self.by_dap_id.get(&dap_id)
    }

    /// Return the total number of tracked breakpoints.
    pub fn len(&self) -> usize {
        self.by_dap_id.len()
    }

    /// Return `true` if no breakpoints are tracked.
    pub fn is_empty(&self) -> bool {
        self.by_dap_id.is_empty()
    }

    /// Iterate over all breakpoints in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = &BreakpointEntry> {
        self.by_dap_id.values()
    }

    /// Find breakpoints for a source URI, returning all entries.
    ///
    /// Returns an iterator over breakpoints that match the given URI.
    pub fn iter_for_uri<'a>(&'a self, uri: &'a str) -> impl Iterator<Item = &'a BreakpointEntry> {
        self.by_dap_id.values().filter(move |e| e.uri == uri)
    }

    /// Find the DAP ID for an existing breakpoint at the given URI and line.
    ///
    /// Returns `None` if no breakpoint exists at that location.
    pub fn find_by_source_line(&self, uri: &str, line: i64) -> Option<i64> {
        self.by_dap_id
            .values()
            .find(|e| e.uri == uri && e.line == Some(line as i32))
            .map(|e| e.dap_id)
    }
}

impl Default for BreakpointState {
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
    fn test_breakpoint_state_starts_empty() {
        let state = BreakpointState::new();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn test_breakpoint_state_add_returns_monotonic_dap_ids() {
        let mut state = BreakpointState::new();
        let id1 = state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        let id2 = state.add("bp/2", "file:///lib/main.dart", Some(20), None, false);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_breakpoint_state_add_first_id_is_one() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/1", "file:///lib/main.dart", Some(5), None, false);
        assert_eq!(id, 1, "First DAP breakpoint ID must be 1");
    }

    #[test]
    fn test_breakpoint_state_lookup_by_dap_id() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/42", "file:///lib/foo.dart", Some(15), Some(3), true);
        let entry = state.lookup_by_dap_id(id).expect("Entry should exist");
        assert_eq!(entry.dap_id, id);
        assert_eq!(entry.vm_id, "bp/42");
        assert_eq!(entry.uri, "file:///lib/foo.dart");
        assert_eq!(entry.line, Some(15));
        assert_eq!(entry.column, Some(3));
        assert!(entry.verified);
    }

    #[test]
    fn test_breakpoint_state_lookup_by_dap_id_returns_none_for_unknown() {
        let state = BreakpointState::new();
        assert!(state.lookup_by_dap_id(99).is_none());
    }

    #[test]
    fn test_breakpoint_state_lookup_by_vm_id() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/99", "file:///lib/bar.dart", Some(7), None, false);
        let entry = state.lookup_by_vm_id("bp/99").expect("Entry should exist");
        assert_eq!(entry.dap_id, id);
    }

    #[test]
    fn test_breakpoint_state_lookup_by_vm_id_returns_none_for_unknown() {
        let state = BreakpointState::new();
        assert!(state.lookup_by_vm_id("bp/unknown").is_none());
    }

    #[test]
    fn test_breakpoint_state_remove_by_dap_id() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        let removed = state
            .remove_by_dap_id(id)
            .expect("Should remove existing entry");
        assert_eq!(removed.dap_id, id);
        assert!(state.lookup_by_dap_id(id).is_none());
        assert!(state.lookup_by_vm_id("bp/1").is_none());
        assert!(state.is_empty());
    }

    #[test]
    fn test_breakpoint_state_remove_by_dap_id_returns_none_for_unknown() {
        let mut state = BreakpointState::new();
        assert!(state.remove_by_dap_id(99).is_none());
    }

    #[test]
    fn test_breakpoint_state_remove_all_for_uri() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        state.add("bp/2", "file:///lib/main.dart", Some(20), None, false);
        state.add("bp/3", "file:///lib/other.dart", Some(5), None, false);

        let removed = state.remove_all_for_uri("file:///lib/main.dart");
        assert_eq!(
            removed.len(),
            2,
            "Should remove 2 breakpoints from main.dart"
        );
        assert_eq!(state.len(), 1, "other.dart breakpoint should remain");
        // The remaining breakpoint is in other.dart.
        assert!(state.lookup_by_vm_id("bp/3").is_some());
    }

    #[test]
    fn test_breakpoint_state_remove_all_for_uri_unknown_returns_empty() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        let removed = state.remove_all_for_uri("file:///lib/does_not_exist.dart");
        assert!(removed.is_empty());
        assert_eq!(state.len(), 1);
    }

    #[test]
    fn test_breakpoint_state_resolve_breakpoint() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);

        let entry = state
            .resolve_breakpoint("bp/1", Some(11), Some(3))
            .expect("Should resolve known breakpoint");
        assert!(entry.verified);
        assert_eq!(entry.line, Some(11));
        assert_eq!(entry.column, Some(3));
    }

    #[test]
    fn test_breakpoint_state_resolve_preserves_existing_line_when_none_passed() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        // Resolve with no new line — should keep existing line.
        state.resolve_breakpoint("bp/1", None, None);
        let entry = state.lookup_by_vm_id("bp/1").unwrap();
        assert_eq!(
            entry.line,
            Some(10),
            "Line should be preserved when None is passed"
        );
    }

    #[test]
    fn test_breakpoint_state_resolve_unknown_vm_id_returns_none() {
        let mut state = BreakpointState::new();
        assert!(state
            .resolve_breakpoint("bp/unknown", Some(5), None)
            .is_none());
    }

    #[test]
    fn test_breakpoint_state_len_tracks_additions_and_removals() {
        let mut state = BreakpointState::new();
        assert_eq!(state.len(), 0);
        let id = state.add("bp/1", "file:///lib/main.dart", Some(1), None, false);
        assert_eq!(state.len(), 1);
        state.add("bp/2", "file:///lib/main.dart", Some(2), None, false);
        assert_eq!(state.len(), 2);
        state.remove_by_dap_id(id);
        assert_eq!(state.len(), 1);
    }

    #[test]
    fn test_breakpoint_state_iter_returns_all_entries() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/a.dart", Some(1), None, false);
        state.add("bp/2", "file:///lib/b.dart", Some(2), None, false);
        state.add("bp/3", "file:///lib/c.dart", Some(3), None, false);

        let count = state.iter().count();
        assert_eq!(count, 3);
    }
}
