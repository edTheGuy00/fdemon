## Task: Add Network Filter Input Mode

**Objective**: Wire the `/` key in the Network panel to activate a text-input mode for entering a filter string. The `NetworkFilterChanged` message, `NetworkState::set_filter()`, and `filtered_entries()` are already fully implemented — this task adds the missing UI input mechanism to emit the message.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/state.rs`: MODIFIED — Add `NetworkFilterInput` variant to network-related UI state (or add a field to `NetworkMonitorState` if centralized there)
- `crates/fdemon-app/src/session/network.rs`: MODIFIED — Add `filter_input_active: bool` and `filter_input_buffer: String` fields
- `crates/fdemon-app/src/message.rs`: MODIFIED — Add `NetworkEnterFilterMode`, `NetworkExitFilterMode`, `NetworkFilterInput(char)`, `NetworkFilterBackspace` messages
- `crates/fdemon-app/src/handler/keys.rs`: MODIFIED — Add `/` binding in Network panel, route char/backspace/escape in filter input mode
- `crates/fdemon-app/src/handler/devtools/network.rs`: MODIFIED — Add handlers for filter input messages
- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`: MODIFIED — Render filter input bar when active

### Details

#### 1. Add filter input state to `NetworkState` (`session/network.rs`)

Add two fields for the input mode:

```rust
/// Whether the filter text input is currently active.
pub filter_input_active: bool,
/// Buffer for the filter text being typed (committed on Enter).
pub filter_input_buffer: String,
```

Initialize both in `Default` impl: `filter_input_active: false`, `filter_input_buffer: String::new()`.
`reset()` should clear both.

#### 2. Add messages (`message.rs`)

Add the following message variants (group near the existing `NetworkFilterChanged`):

```rust
/// Enter network filter input mode (activates text input).
NetworkEnterFilterMode,
/// Exit network filter input mode (cancel, discard buffer).
NetworkExitFilterMode,
/// Commit the filter input buffer (apply filter and exit input mode).
NetworkCommitFilter,
/// Append a character to the filter input buffer.
NetworkFilterInput(char),
/// Delete last character from filter input buffer.
NetworkFilterBackspace,
```

#### 3. Add key bindings (`handler/keys.rs`)

In `handle_key_devtools()`, handle the filter input mode first (before other Network panel bindings):

```rust
// ── Network filter input mode ────────────────────────────────────────
// When filter input is active, route keys to the filter buffer.
if in_network {
    let filter_active = active_id.and_then(|_| {
        state.session_manager.selected()
            .map(|h| h.session.network.filter_input_active)
    }).unwrap_or(false);

    if filter_active {
        return match key {
            InputKey::Escape => Some(Message::NetworkExitFilterMode),
            InputKey::Enter => Some(Message::NetworkCommitFilter),
            InputKey::Backspace => Some(Message::NetworkFilterBackspace),
            InputKey::Char(c) if !c.is_control() => Some(Message::NetworkFilterInput(c)),
            _ => None,
        };
    }
}
```

Add the `/` binding in the existing Network panel section:

```rust
InputKey::Char('/') if in_network => Some(Message::NetworkEnterFilterMode),
```

#### 4. Implement handlers (`handler/devtools/network.rs`)

```rust
/// Enter filter input mode — copy current filter into the input buffer.
pub(crate) fn handle_enter_filter_mode(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.filter_input_buffer = handle.session.network.filter.clone();
        handle.session.network.filter_input_active = true;
    }
    UpdateResult::none()
}

/// Exit filter input mode — discard the buffer, keep the old filter.
pub(crate) fn handle_exit_filter_mode(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.filter_input_active = false;
        handle.session.network.filter_input_buffer.clear();
    }
    UpdateResult::none()
}

/// Commit the filter input — apply the buffer as the active filter and exit input mode.
pub(crate) fn handle_commit_filter(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        let new_filter = handle.session.network.filter_input_buffer.clone();
        handle.session.network.set_filter(new_filter);
        handle.session.network.filter_input_active = false;
        handle.session.network.filter_input_buffer.clear();
    }
    UpdateResult::none()
}

/// Append a character to the filter input buffer.
pub(crate) fn handle_filter_input(state: &mut AppState, c: char) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.filter_input_buffer.push(c);
    }
    UpdateResult::none()
}

/// Delete last character from filter input buffer.
pub(crate) fn handle_filter_backspace(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.network.filter_input_buffer.pop();
    }
    UpdateResult::none()
}
```

#### 5. Wire in `update.rs`

Add match arms for all new messages:

```rust
Message::NetworkEnterFilterMode => devtools::network::handle_enter_filter_mode(state),
Message::NetworkExitFilterMode => devtools::network::handle_exit_filter_mode(state),
Message::NetworkCommitFilter => devtools::network::handle_commit_filter(state),
Message::NetworkFilterInput(c) => devtools::network::handle_filter_input(state, c),
Message::NetworkFilterBackspace => devtools::network::handle_filter_backspace(state),
```

#### 6. Render filter input bar (`widgets/devtools/network/mod.rs`)

When `filter_input_active` is true, render a filter input bar at the top of the Network panel (or at the bottom, where the key hints are). Replace the normal header/hint area with:

```
Filter: api/users█   (Enter to apply, Esc to cancel)
```

When `filter_input_active` is false but `filter` is non-empty, show an indicator:

```
Filter: "api/users" (3 of 42 requests)  — press / to edit, Ctrl+X to clear filter
```

The exact rendering location and style should follow the existing pattern used by the search input in Normal mode (see `widgets/log_view/` for reference).

#### 7. Clear filter on Ctrl+X

The existing `Ctrl+x` binding in the Network panel clears recorded requests (`ClearNetworkProfile`). Consider whether Ctrl+X should also clear the filter, or if a separate binding is needed. The simplest approach: if filter is active, `Ctrl+X` clears the filter; if no filter, `Ctrl+X` clears requests. Alternatively, the filter is always cleared along with the request history. Use your judgment here — follow the pattern that feels most intuitive.

### Acceptance Criteria

1. `/` key in Network panel enters filter input mode
2. Characters typed in filter mode appear in the input buffer
3. `Backspace` removes the last character
4. `Enter` commits the filter (applies it to the request list) and exits input mode
5. `Esc` cancels and exits input mode without changing the filter
6. Filter input bar renders visually when active
7. Active filter indicator shows when filter is non-empty
8. Key bindings in filter mode do not conflict with other Network panel bindings
9. `NetworkFilterChanged` message (existing) is correctly emitted on commit
10. `cargo test -p fdemon-app -- filter` passes
11. `cargo test -p fdemon-app -- devtools` passes

### Testing

```bash
cargo test -p fdemon-app -- filter
cargo test -p fdemon-app -- network
cargo test -p fdemon-app -- handle_key_devtools
```

Add handler tests:

```rust
#[test]
fn test_enter_filter_mode_copies_existing_filter() { ... }

#[test]
fn test_exit_filter_mode_discards_buffer() { ... }

#[test]
fn test_commit_filter_applies_buffer() { ... }

#[test]
fn test_filter_input_appends_char() { ... }

#[test]
fn test_filter_backspace_removes_char() { ... }

#[test]
fn test_filter_mode_keys_routed_before_panel_keys() { ... }
```

### Notes

- **Existing infrastructure**: `NetworkFilterChanged` message and `NetworkState::set_filter()` are fully implemented with 5 existing tests. The handler `handle_network_filter_changed()` in `handler/devtools/network.rs:225` is wired in `update.rs:1633`. This task only adds the UI input mechanism.
- **Pattern reference**: The log search input mode (`/` in Normal mode → search input → `Enter` to commit) follows the same interaction pattern. Check `handler/keys.rs` for how `UiMode::SearchInput` is handled.
- **Live filtering vs commit**: The implementation above uses a "commit on Enter" model (buffer is separate from active filter). An alternative is live filtering (filter updates as you type). The commit model is simpler and matches the existing search pattern. If live filtering is desired, emit `NetworkFilterChanged` on each `NetworkFilterInput` instead.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/network.rs` | Added `filter_input_active: bool` and `filter_input_buffer: String` fields to `NetworkState`; updated `Default` impl and `reset()` method |
| `crates/fdemon-app/src/message.rs` | Added 5 new message variants: `NetworkEnterFilterMode`, `NetworkExitFilterMode`, `NetworkCommitFilter`, `NetworkFilterInput(char)`, `NetworkFilterBackspace` |
| `crates/fdemon-app/src/handler/keys.rs` | Added filter input mode interception block before `match key` in `handle_key_devtools()`; added `/` key binding for `NetworkEnterFilterMode`; added `network_filter_key_tests` module with 8 tests |
| `crates/fdemon-app/src/handler/devtools/network.rs` | Added 5 handler functions: `handle_enter_filter_mode`, `handle_exit_filter_mode`, `handle_commit_filter`, `handle_filter_input`, `handle_filter_backspace`; added 10 unit tests |
| `crates/fdemon-app/src/handler/update.rs` | Wired 5 new messages to their handler functions in the `update()` match |
| `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` | Added `render_filter_input_bar()` method; modified `render()` to show filter bar and shrink content area when `filter_input_active` is true; added 6 render tests |
| `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` | Added 6 filter input bar render tests |

### Notable Decisions/Tradeoffs

1. **Commit-on-Enter model**: Used separate buffer (`filter_input_buffer`) from active filter (`filter`). This matches the existing log search pattern (`UiMode::SearchInput`) where buffer is only applied on Enter. Esc cleanly discards the buffer with no side effects.

2. **Key interception placement**: The filter input mode early-return block is placed before the `match key {}` arm so that no Network panel binding (j/k navigation, detail tabs, space for recording) leaks through during text input. This is critical for characters like `j`, `k`, `g`, `h`, `q`, `s`, `t`, `/` which all have Network panel bindings.

3. **Filter bar rendering**: The filter bar is rendered at the top of the usable area (row 0 of content, after footer reservation), shrinking the content area by one row. An additional minimum-size check after shrinking ensures graceful degradation on very small terminals.

4. **`palette::STATUS_BLUE`** was used for the filter prompt label (there is no `ACCENT_CYAN` in the palette), giving a visually distinct blue color that indicates an interactive input mode.

5. **`reset()` explicit fields**: The `reset()` method explicitly sets `filter_input_active: false` and `filter_input_buffer: String::new()` instead of relying on `..Self::default()`, to make the reset intent clear for future maintainers.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo fmt --all` — Passed (no changes)
- `cargo test -p fdemon-app -- filter` — Passed (57 tests, 17 new)
- `cargo test -p fdemon-app -- network` — Passed (65 tests, 17 new)
- `cargo test -p fdemon-app -- devtools` — Passed (127 tests)
- `cargo test -p fdemon-tui -- network` — Passed (127 tests, 6 new)
- `cargo clippy --workspace` — Passed (0 warnings)

### Risks/Limitations

1. **E2E tests**: The binary E2E test suite has 25 pre-existing failures (settings page and TUI interaction tests requiring a Flutter device). These failures are unrelated to this task and existed before these changes.

2. **No Ctrl+X filter clear**: The task mentioned optionally clearing the filter with Ctrl+X. The implementation leaves Ctrl+X to always clear request history (existing behavior), not the filter. The filter is cleared by entering filter mode (/) and committing an empty buffer.
