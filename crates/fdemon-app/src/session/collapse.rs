//! Collapse state for stack traces.

use std::collections::HashSet;

/// Tracks which log entries have expanded/collapsed stack traces
#[derive(Debug, Clone, Default)]
pub struct CollapseState {
    /// Set of log entry IDs that are currently expanded
    /// (by default, entries are collapsed based on config)
    expanded_entries: HashSet<u64>,

    /// Set of log entry IDs that are explicitly collapsed
    /// (overrides default when default is expanded)
    collapsed_entries: HashSet<u64>,
}

impl CollapseState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if an entry's stack trace should be shown expanded
    pub fn is_expanded(&self, entry_id: u64, default_collapsed: bool) -> bool {
        if default_collapsed {
            // Default is collapsed, check if user expanded it
            self.expanded_entries.contains(&entry_id)
        } else {
            // Default is expanded, check if user collapsed it
            !self.collapsed_entries.contains(&entry_id)
        }
    }

    /// Toggle the collapse state of an entry
    pub fn toggle(&mut self, entry_id: u64, default_collapsed: bool) {
        if default_collapsed {
            if self.expanded_entries.contains(&entry_id) {
                self.expanded_entries.remove(&entry_id);
            } else {
                self.expanded_entries.insert(entry_id);
            }
        } else if self.collapsed_entries.contains(&entry_id) {
            self.collapsed_entries.remove(&entry_id);
        } else {
            self.collapsed_entries.insert(entry_id);
        }
    }

    /// Collapse all stack traces
    pub fn collapse_all(&mut self) {
        self.expanded_entries.clear();
        self.collapsed_entries.clear(); // Let default take over
    }

    /// Expand all stack traces for the given entry IDs
    pub fn expand_all(&mut self, entry_ids: impl Iterator<Item = u64>) {
        self.collapsed_entries.clear();
        self.expanded_entries.extend(entry_ids);
    }
}
