//! Per-session native log tag discovery and visibility state.

use std::collections::{BTreeMap, BTreeSet};

/// Per-session state for native log tag discovery and filtering.
///
/// As native log events arrive, tags are added to `discovered_tags`.
/// Users can toggle individual tags on/off via the tag filter UI.
/// By default, all discovered tags are visible (not hidden).
///
/// # Tag visibility semantics
///
/// Tags not in `hidden_tags` are visible. Unknown tags (not yet seen) are also
/// treated as visible by default so that `is_tag_visible` can be called before
/// `observe_tag` without incorrectly hiding new tags.
///
/// # Filtering approach
///
/// Visibility filtering happens at the *handler* level: when a `NativeLog`
/// message arrives for a hidden tag, the log entry is **not added** to the
/// session log buffer. This avoids filling the ring buffer with entries the
/// user has explicitly hidden.
///
/// Consequence: un-hiding a tag only shows future entries, not historical
/// ones. This is consistent with how `LogSourceFilter` works elsewhere.
#[derive(Debug, Clone, Default)]
pub struct NativeTagState {
    /// All tags seen in this session's native log stream, ordered alphabetically.
    ///
    /// Key: tag name. Value: number of log entries with this tag (including
    /// hidden entries — the count reflects capture volume, not displayed entries).
    pub discovered_tags: BTreeMap<String, usize>,

    /// Tags that the user has explicitly hidden via the tag filter UI.
    ///
    /// Tags not in this set are visible by default.
    pub hidden_tags: BTreeSet<String>,
}

impl NativeTagState {
    /// Record a tag observation.
    ///
    /// Creates the entry if the tag is new, increments the count if it already
    /// exists. Called for *every* incoming native log event regardless of
    /// whether the tag is currently hidden, so the count reflects total capture
    /// volume.
    ///
    /// Tags are normalised to ASCII lowercase so that `"GoLog"` and `"golog"`
    /// map to the same entry, matching the case-insensitive behaviour of the
    /// daemon-layer `should_include_tag`.
    pub fn observe_tag(&mut self, tag: &str) {
        let key = tag.to_ascii_lowercase();
        *self.discovered_tags.entry(key).or_insert(0) += 1;
    }

    /// Whether a tag is currently visible (not hidden by the user).
    ///
    /// Unknown tags (not yet in `discovered_tags`) are considered visible so
    /// that new tags appear immediately when first seen.
    ///
    /// Tag lookup is case-insensitive: `"GoLog"` and `"golog"` resolve to the
    /// same visibility state.
    pub fn is_tag_visible(&self, tag: &str) -> bool {
        !self.hidden_tags.contains(&tag.to_ascii_lowercase() as &str)
    }

    /// Toggle a tag's visibility.
    ///
    /// Returns the new visibility state: `true` means now visible, `false`
    /// means now hidden.
    ///
    /// Tag names are normalised to ASCII lowercase before storage.
    pub fn toggle_tag(&mut self, tag: &str) -> bool {
        let key = tag.to_ascii_lowercase();
        if self.hidden_tags.contains(&key as &str) {
            self.hidden_tags.remove(&key as &str);
            true // now visible
        } else {
            self.hidden_tags.insert(key);
            false // now hidden
        }
    }

    /// Get all discovered tags sorted alphabetically with their log counts.
    ///
    /// Returns an iterator of `(tag_name, count)` pairs in alphabetical order.
    /// The `BTreeMap` guarantees this ordering without an additional sort step.
    pub fn sorted_tags(&self) -> Vec<(&String, &usize)> {
        self.discovered_tags.iter().collect()
    }

    /// Number of distinct tags discovered so far.
    pub fn tag_count(&self) -> usize {
        self.discovered_tags.len()
    }

    /// Number of tags currently hidden.
    pub fn hidden_count(&self) -> usize {
        self.hidden_tags.len()
    }

    /// Show all tags by clearing the hidden set.
    pub fn show_all(&mut self) {
        self.hidden_tags.clear();
    }

    /// Hide all discovered tags.
    ///
    /// Any tag not yet in `discovered_tags` will not be pre-hidden — it will
    /// appear visible when first seen and must be toggled separately.
    pub fn hide_all(&mut self) {
        self.hidden_tags = self.discovered_tags.keys().cloned().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observe_tag_creates_entry() {
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        assert_eq!(state.tag_count(), 1);
        // Tags are normalised to ASCII lowercase at storage time.
        assert_eq!(state.discovered_tags["golog"], 1);
    }

    #[test]
    fn test_observe_tag_increments_count() {
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        state.observe_tag("GoLog");
        state.observe_tag("GoLog");
        // Tags are normalised to ASCII lowercase at storage time.
        assert_eq!(state.discovered_tags["golog"], 3);
    }

    #[test]
    fn test_multiple_tags_sorted() {
        let mut state = NativeTagState::default();
        state.observe_tag("OkHttp");
        state.observe_tag("GoLog");
        state.observe_tag("MyPlugin");
        let tags: Vec<&String> = state.sorted_tags().iter().map(|(k, _)| *k).collect();
        // All stored as lowercase; BTreeMap sorts them lexicographically.
        assert_eq!(tags, vec!["golog", "myplugin", "okhttp"]);
    }

    #[test]
    fn test_toggle_tag_visibility() {
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        assert!(state.is_tag_visible("GoLog"));

        let visible = state.toggle_tag("GoLog");
        assert!(!visible);
        assert!(!state.is_tag_visible("GoLog"));

        let visible = state.toggle_tag("GoLog");
        assert!(visible);
        assert!(state.is_tag_visible("GoLog"));
    }

    #[test]
    fn test_show_all_clears_hidden() {
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        state.observe_tag("OkHttp");
        state.toggle_tag("GoLog");
        state.toggle_tag("OkHttp");
        assert_eq!(state.hidden_count(), 2);

        state.show_all();
        assert_eq!(state.hidden_count(), 0);
        assert!(state.is_tag_visible("GoLog"));
        assert!(state.is_tag_visible("OkHttp"));
    }

    #[test]
    fn test_hide_all() {
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        state.observe_tag("OkHttp");

        state.hide_all();
        assert!(!state.is_tag_visible("GoLog"));
        assert!(!state.is_tag_visible("OkHttp"));
        assert_eq!(state.hidden_count(), 2);
    }

    #[test]
    fn test_is_tag_visible_unknown_tag() {
        let state = NativeTagState::default();
        // Unknown tags are visible by default (not in hidden set)
        assert!(state.is_tag_visible("UnknownTag"));
    }

    #[test]
    fn test_default_state_empty() {
        let state = NativeTagState::default();
        assert_eq!(state.tag_count(), 0);
        assert_eq!(state.hidden_count(), 0);
    }

    #[test]
    fn test_sorted_tags_returns_all() {
        let mut state = NativeTagState::default();
        state.observe_tag("Zebra");
        state.observe_tag("Apple");
        state.observe_tag("Mango");
        let tags = state.sorted_tags();
        assert_eq!(tags.len(), 3);
        // All stored as lowercase; relative alphabetical order is preserved.
        assert_eq!(tags[0].0, "apple");
        assert_eq!(tags[1].0, "mango");
        assert_eq!(tags[2].0, "zebra");
    }

    #[test]
    fn test_observe_increments_count_for_hidden_tag() {
        // Count should track total volume even when tag is hidden
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        state.toggle_tag("GoLog"); // hide it
        state.observe_tag("GoLog"); // still increments count
                                    // Tags are normalised to ASCII lowercase at storage time.
        assert_eq!(state.discovered_tags["golog"], 2);
        assert!(!state.is_tag_visible("GoLog")); // still hidden
    }

    #[test]
    fn test_hide_all_only_hides_discovered() {
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        state.hide_all();

        // Known tag is hidden (case-insensitive lookup)
        assert!(!state.is_tag_visible("GoLog"));
        assert!(!state.is_tag_visible("golog"));
        // Unknown tag is still visible
        assert!(state.is_tag_visible("NewTag"));
    }

    // ── Case-insensitivity tests (Issue #8) ──────────────────────────────────

    #[test]
    fn test_observe_tag_normalises_case() {
        // "GoLog" and "golog" must collapse to a single entry.
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        state.observe_tag("golog");
        assert_eq!(state.tag_count(), 1, "case variants must map to one entry");
        assert_eq!(state.discovered_tags["golog"], 2);
    }

    #[test]
    fn test_observe_tag_mixed_case_variants_merge() {
        // All case variants of a tag collapse into a single lowercase entry.
        let mut state = NativeTagState::default();
        state.observe_tag("OKHTTP");
        state.observe_tag("OkHttp");
        state.observe_tag("okhttp");
        assert_eq!(state.tag_count(), 1);
        assert_eq!(state.discovered_tags["okhttp"], 3);
    }

    #[test]
    fn test_is_tag_visible_case_insensitive() {
        // Hiding via one case variant makes all variants hidden.
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        state.toggle_tag("golog"); // hide using lowercase
        assert!(!state.is_tag_visible("GoLog")); // query using original case
        assert!(!state.is_tag_visible("GOLOG"));
        assert!(!state.is_tag_visible("golog"));
    }

    #[test]
    fn test_toggle_tag_case_insensitive() {
        // Toggling via a different case variant affects the same entry.
        let mut state = NativeTagState::default();
        state.observe_tag("OkHttp");
        assert!(state.is_tag_visible("OkHttp"));

        let visible = state.toggle_tag("OKHTTP"); // hide via uppercase
        assert!(!visible);
        assert!(!state.is_tag_visible("okhttp"));

        let visible = state.toggle_tag("OkHttp"); // show via mixed case
        assert!(visible);
        assert!(state.is_tag_visible("OKHTTP"));
    }
}
