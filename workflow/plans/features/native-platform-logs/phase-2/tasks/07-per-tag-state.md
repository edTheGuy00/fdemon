## Task: Per-Tag State Tracking

**Objective**: Track discovered native log tags per session as they arrive, and add a `TagFilterState` that allows toggling individual tags on/off. This provides the data model for the per-tag filtering UI (task 09).

**Depends on**: None

### Scope

- `crates/fdemon-app/src/session/handle.rs` (or a new `session/native_tags.rs`): Add `NativeTagState` with discovered tags and filter state
- `crates/fdemon-app/src/handler/update.rs`: Update `NativeLog` handler to track discovered tags
- `crates/fdemon-core/src/types.rs`: Add `TagFilterState` if needed at the core level (or keep in app layer)

### Details

#### 1. Tag state data structure

Each session needs to track which native log tags have been seen and whether each is currently visible:

```rust
use std::collections::{BTreeMap, BTreeSet};

/// Per-session state for native log tag discovery and filtering.
///
/// As native log events arrive, tags are added to `discovered_tags`.
/// Users can toggle individual tags on/off via the tag filter UI.
/// By default, all discovered tags are visible (not hidden).
#[derive(Debug, Clone, Default)]
pub struct NativeTagState {
    /// All tags seen in this session's native log stream, ordered alphabetically.
    /// Key: tag name, Value: number of log entries with this tag.
    pub discovered_tags: BTreeMap<String, usize>,

    /// Tags that the user has explicitly hidden via the tag filter UI.
    /// Tags not in this set are visible by default.
    pub hidden_tags: BTreeSet<String>,
}

impl NativeTagState {
    /// Record a tag observation. Creates the entry if new, increments count if existing.
    pub fn observe_tag(&mut self, tag: &str) {
        *self.discovered_tags.entry(tag.to_string()).or_insert(0) += 1;
    }

    /// Whether a tag is currently visible (not hidden by the user).
    pub fn is_tag_visible(&self, tag: &str) -> bool {
        !self.hidden_tags.contains(tag)
    }

    /// Toggle a tag's visibility. Returns the new visibility state.
    pub fn toggle_tag(&mut self, tag: &str) -> bool {
        if self.hidden_tags.contains(tag) {
            self.hidden_tags.remove(tag);
            true // now visible
        } else {
            self.hidden_tags.insert(tag.to_string());
            false // now hidden
        }
    }

    /// Get all discovered tags sorted alphabetically.
    pub fn sorted_tags(&self) -> Vec<(&String, &usize)> {
        self.discovered_tags.iter().collect()
    }

    /// Number of distinct tags discovered.
    pub fn tag_count(&self) -> usize {
        self.discovered_tags.len()
    }

    /// Number of tags currently hidden.
    pub fn hidden_count(&self) -> usize {
        self.hidden_tags.len()
    }

    /// Show all tags (clear all hidden tags).
    pub fn show_all(&mut self) {
        self.hidden_tags.clear();
    }

    /// Hide all tags.
    pub fn hide_all(&mut self) {
        self.hidden_tags = self.discovered_tags.keys().cloned().collect();
    }
}
```

**Why `BTreeMap`/`BTreeSet`**: Provides stable alphabetical ordering for the UI, unlike `HashMap`/`HashSet` which have non-deterministic iteration order. The tag count is typically small (10-50 tags) so performance is not a concern.

#### 2. Add `NativeTagState` to `SessionHandle`

```rust
// In session/handle.rs, add to the SessionHandle struct:
pub native_tag_state: NativeTagState,
```

Initialize to `NativeTagState::default()` in the constructor. Reset when the session is stopped/restarted.

#### 3. Track tags in the `NativeLog` message handler

In `handler/update.rs`, the `Message::NativeLog` handler creates a `LogEntry` and queues it. Add tag observation:

```rust
Message::NativeLog { session_id, event } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Track the discovered tag
        handle.native_tag_state.observe_tag(&event.tag);

        // Check per-tag visibility before adding to log
        if !handle.native_tag_state.is_tag_visible(&event.tag) {
            // User has hidden this tag — skip the log entry
            return UpdateResult::none();
        }

        let entry = LogEntry::new(
            event.level,
            LogSource::Native { tag: event.tag },
            event.message,
        );
        handle.session.queue_log(entry);
    }
    UpdateResult::none()
}
```

**Important consideration**: Per-tag visibility filtering happens at the *display* level, not the *capture* level. The tag state records all observed tags regardless of visibility, so the tag count and UI reflect all available tags. However, hidden tags' log entries are **not added to the log buffer** to avoid filling it with filtered-out entries.

**Alternative approach**: Add all entries to the log buffer and filter at render time. This preserves history if the user un-hides a tag, but uses buffer space for invisible entries. The choice depends on whether retroactive un-hiding is important. For the initial implementation, filtering at the handler level (not adding hidden entries) is simpler. The user expectation should be documented.

#### 4. Add new `Message` variants for tag filtering

```rust
// In message.rs:
pub enum Message {
    // ... existing variants ...

    /// Toggle a specific native log tag's visibility in the current session.
    ToggleNativeTag { tag: String },

    /// Show all native log tags in the current session.
    ShowAllNativeTags,

    /// Hide all native log tags in the current session.
    HideAllNativeTags,

    /// Open the tag filter overlay.
    ShowTagFilter,

    /// Close the tag filter overlay.
    HideTagFilter,
}
```

#### 5. Handle tag filter messages in `update.rs`

```rust
Message::ToggleNativeTag { tag } => {
    if let Some(handle) = state.session_manager.active_session_mut() {
        let visible = handle.native_tag_state.toggle_tag(&tag);
        tracing::debug!("Tag '{}' is now {}", tag, if visible { "visible" } else { "hidden" });
    }
    UpdateResult::none()
}

Message::ShowAllNativeTags => {
    if let Some(handle) = state.session_manager.active_session_mut() {
        handle.native_tag_state.show_all();
    }
    UpdateResult::none()
}

Message::HideAllNativeTags => {
    if let Some(handle) = state.session_manager.active_session_mut() {
        handle.native_tag_state.hide_all();
    }
    UpdateResult::none()
}
```

`ShowTagFilter` and `HideTagFilter` set/clear a UI mode flag (handled in task 09).

#### 6. Reset tag state on session lifecycle events

Clear `native_tag_state` when:
- Session is stopped (`handle_session_stop`)
- Session is closed (`handle_close_session`)
- Native log capture stops (`NativeLogCaptureStopped`)

```rust
// In the appropriate handlers:
handle.native_tag_state = NativeTagState::default();
```

### Acceptance Criteria

1. `NativeTagState` struct exists with `discovered_tags` (BTreeMap) and `hidden_tags` (BTreeSet)
2. `observe_tag()` adds new tags and increments count for existing tags
3. `is_tag_visible()` returns `false` for hidden tags, `true` otherwise
4. `toggle_tag()` toggles between hidden and visible
5. `show_all()` clears all hidden tags
6. `hide_all()` hides all discovered tags
7. `sorted_tags()` returns tags in alphabetical order
8. `SessionHandle` has `native_tag_state` field, initialized to default
9. `NativeLog` handler calls `observe_tag()` for every incoming event
10. Hidden tags' log entries are not added to the log buffer
11. `Message::ToggleNativeTag`, `ShowAllNativeTags`, `HideAllNativeTags` are handled in `update.rs`
12. Tag state is reset on session stop/close
13. `cargo check --workspace` compiles
14. `cargo test -p fdemon-app` passes

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observe_tag_creates_entry() {
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        assert_eq!(state.tag_count(), 1);
        assert_eq!(state.discovered_tags["GoLog"], 1);
    }

    #[test]
    fn test_observe_tag_increments_count() {
        let mut state = NativeTagState::default();
        state.observe_tag("GoLog");
        state.observe_tag("GoLog");
        state.observe_tag("GoLog");
        assert_eq!(state.discovered_tags["GoLog"], 3);
    }

    #[test]
    fn test_multiple_tags_sorted() {
        let mut state = NativeTagState::default();
        state.observe_tag("OkHttp");
        state.observe_tag("GoLog");
        state.observe_tag("MyPlugin");
        let tags: Vec<&String> = state.sorted_tags().iter().map(|(k, _)| *k).collect();
        assert_eq!(tags, vec!["GoLog", "MyPlugin", "OkHttp"]);
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
}
```

### Notes

- **`BTreeMap` over `HashMap`**: Stable alphabetical ordering is important for the tag filter UI (task 09). Users expect a consistent tag list that doesn't shuffle between renders.
- **Filtering at handler vs render level**: The initial implementation filters at the handler level (hidden tags' entries are not added to the log buffer). This means un-hiding a tag only shows future entries, not historical ones. This is consistent with how the existing `LogSourceFilter` works (it filters the display, but the log buffer is a fixed ring). If retroactive filtering is desired later, the filter can be moved to render time.
- **Tag count per entry**: Storing the count per tag (`BTreeMap<String, usize>`) allows the UI to show frequency info (e.g., "GoLog (42)") which helps users identify important tags.
- **The `NativeTagState` struct lives in the app layer** (not fdemon-core) because it's session-specific state. If it needs to be shared with the TUI layer, it can be accessed via `SessionHandle`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/native_tags.rs` | New file: `NativeTagState` struct with `observe_tag`, `is_tag_visible`, `toggle_tag`, `sorted_tags`, `tag_count`, `hidden_count`, `show_all`, `hide_all` methods, plus 11 unit tests |
| `crates/fdemon-app/src/session/mod.rs` | Added `mod native_tags` and `pub use native_tags::NativeTagState` re-export |
| `crates/fdemon-app/src/session/handle.rs` | Added `use super::native_tags::NativeTagState`, `native_tag_state: NativeTagState` field on `SessionHandle`, initialized to `NativeTagState::default()` in constructor, added `native_tag_count` to `Debug` impl |
| `crates/fdemon-app/src/message.rs` | Added `ToggleNativeTag { tag: String }`, `ShowAllNativeTags`, `HideAllNativeTags`, `ShowTagFilter`, `HideTagFilter` message variants |
| `crates/fdemon-app/src/handler/update.rs` | Updated `NativeLog` handler to call `observe_tag()` and skip hidden tags; updated `NativeLogCaptureStopped` to reset tag state; added handlers for `ToggleNativeTag`, `ShowAllNativeTags`, `HideAllNativeTags`, `ShowTagFilter`, `HideTagFilter` |
| `crates/fdemon-app/src/handler/session.rs` | Added `native_tag_state` reset in `handle_session_exited` (process exit) and `handle_session_message_state` `AppStop` handler (app stop/restart) |
| `crates/fdemon-app/src/handler/tests.rs` | Added 11 handler tests covering: `observe_tag` on `NativeLog`, hidden tag filtering, `ToggleNativeTag`, `ShowAllNativeTags`, `HideAllNativeTags`, `NativeLogCaptureStopped` reset, `ShowTagFilter`/`HideTagFilter` no-op, and no-session edge cases |

### Notable Decisions/Tradeoffs

1. **No `active_session_mut()` needed**: The codebase uses the pattern `selected_id()` + `get_mut(id)` for accessing the active session. The `ToggleNativeTag`/`ShowAll`/`HideAll` handlers follow this pattern consistently.

2. **Tag state reset on 3 events**: `NativeLogCaptureStopped` (capture process exits), `handle_session_exited` (Flutter process exits), and `AppStop` daemon message (app stopped within session). This ensures clean state across hot restarts and session reuses.

3. **Phase 1 foundation merge**: The worktree needed a merge of `feature/native-platform-logs` (phase 1) before implementing this task, following the same pattern as the `agent-a0516f73` worktree.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1496 tests: 1485 pre-existing + 11 new)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Applied cleanly
