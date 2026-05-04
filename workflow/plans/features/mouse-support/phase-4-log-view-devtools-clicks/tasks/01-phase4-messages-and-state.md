## Task: Phase-4 Messages, AppState Fields & Dispatch Stubs

**Objective**: Define every new `Message` variant Phase 4 will emit, add the `AppState::last_log_click` field that powers double-click detection, and wire dispatch arms in `handler/update.rs` so the rest of the phase can compile against stable APIs. Stub functions are added in `handler/log_view.rs` and `handler/devtools/inspector.rs` to keep the build green; Tasks 03 and 04 fill in the bodies.

**Depends on**: None (Wave 1)

**Estimated Time**: 1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs`: Add four new `Message` variants and (if not already present) ensure `Eq + PartialEq` semantics still derive cleanly. Document each variant with a `///` rustdoc comment.
- `crates/fdemon-app/src/state.rs`: Add `pub last_log_click: Option<LogClickStamp>` to `AppState`. Define `pub struct LogClickStamp { pub entry_id: u64, pub at: std::time::Instant }` (in `state.rs`, alongside other small state types). Initialise to `None` in `AppState::new()`.
- `crates/fdemon-app/src/handler/update.rs`: Add four `Message::*` match arms that delegate to (yet-to-be-filled) handler functions in `handler/log_view.rs` and `handler/devtools/inspector.rs`.
- `crates/fdemon-app/src/handler/log_view.rs`: Add stub `pub fn handle_click_log_row(state: &mut AppState, entry_id: u64, frame_index: Option<usize>) -> UpdateResult { UpdateResult::none() }` and `pub fn handle_toggle_stack_trace_for_entry(state: &mut AppState, entry_id: u64) -> UpdateResult { UpdateResult::none() }`. Bodies are Task 03's responsibility.
- `crates/fdemon-app/src/handler/devtools/inspector.rs`: Add stub `pub fn handle_inspector_select_row(state: &mut AppState, index: usize) -> UpdateResult { UpdateResult::none() }` and `pub fn handle_inspector_toggle_node(state: &mut AppState, index: usize) -> UpdateResult { UpdateResult::none() }`. Bodies are Task 04's responsibility.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/keys.rs` (for the existing `Message::ToggleStackTrace` emission pattern at line 255)
- `crates/fdemon-app/src/session/session.rs` (for `focused_entry_id`, `toggle_stack_trace` — informs handler signatures in Task 03)
- `crates/fdemon-app/src/state.rs::InspectorState` (for `visible_nodes`, `is_expanded` — informs handler signatures in Task 04)

### Details

#### New `Message` variants

Add to the `Message` enum in `message.rs`. Place them in a new `// ── Mouse Click Messages (Phase 4) ──` section near the existing `// ── Mouse ──` section:

```rust
/// Click on a single log-view row.
///
/// Emitted by the per-frame mouse region registry when the user left-clicks
/// inside the log content area. `entry_id` is the [`LogEntry::id`] of the
/// clicked entry; `frame_index` is `Some(i)` when the click landed on the
/// i-th visible stack-frame line under that entry, or `None` for the
/// message-line click.
///
/// Handler at [`crate::handler::log_view::handle_click_log_row`] updates
/// `AppState::last_log_click` for double-click detection. When the same
/// entry is clicked twice within 400 ms, a follow-up
/// [`Message::ToggleStackTraceForEntry`] is emitted via
/// [`UpdateResult::message`].
ClickLogRow {
    entry_id: u64,
    frame_index: Option<usize>,
},

/// Toggle stack trace expand / collapse for a *specific* log entry.
///
/// Emitted as a follow-up to [`Message::ClickLogRow`] when a double click is
/// detected. Distinct from [`Message::ToggleStackTrace`], which operates on
/// the scroll-focused entry — the click target is rarely the focused entry,
/// so the click flow needs an absolute-id variant.
ToggleStackTraceForEntry { entry_id: u64 },

/// Click on a row in the widget inspector tree.
///
/// `index` is the absolute position into `InspectorState::visible_nodes()`
/// at render time — the registry stored this index when recording the row's
/// rect. The handler sets `inspector.selected_index = index` and dispatches
/// a layout fetch under the same debounce / cache rules as
/// [`InspectorNav::Up`] / [`InspectorNav::Down`].
DevToolsInspectorSelectRow { index: usize },

/// Click on the leading expansion glyph (▶ / ▼ / ●) of a tree row.
///
/// Selects the row first (same as [`Message::DevToolsInspectorSelectRow`])
/// then toggles the node's `expanded` set if the node has children. No-op
/// for leaf nodes.
DevToolsInspectorToggleNode { index: usize },
```

#### `AppState::last_log_click`

In `state.rs`:

```rust
/// Click stamp recorded by [`handler::log_view::handle_click_log_row`]
/// to detect double-clicks within the 400 ms window.
#[derive(Debug, Clone, Copy)]
pub struct LogClickStamp {
    pub entry_id: u64,
    pub at: std::time::Instant,
}

// In `AppState`:
pub struct AppState {
    // ...
    /// Most recent log-row click, used for double-click detection.
    /// Cleared whenever a double-click is consumed or the selected
    /// session changes.
    pub last_log_click: Option<LogClickStamp>,
}

// In `AppState::new()`:
last_log_click: None,
```

#### Dispatch arms in `handler/update.rs`

Add the four new arms in the existing `match msg { ... }`. Place them adjacent to the existing `Message::ToggleStackTrace` arm (around line 682) and the DevTools section:

```rust
Message::ClickLogRow { entry_id, frame_index } => {
    crate::handler::log_view::handle_click_log_row(state, entry_id, frame_index)
}

Message::ToggleStackTraceForEntry { entry_id } => {
    crate::handler::log_view::handle_toggle_stack_trace_for_entry(state, entry_id)
}

Message::DevToolsInspectorSelectRow { index } => {
    crate::handler::devtools::inspector::handle_inspector_select_row(state, index)
}

Message::DevToolsInspectorToggleNode { index } => {
    crate::handler::devtools::inspector::handle_inspector_toggle_node(state, index)
}
```

#### Stub function signatures

In `handler/log_view.rs` (append at the bottom, before the existing tests module if present):

```rust
/// Stub. Body added in Phase 4 Task 03.
pub fn handle_click_log_row(
    _state: &mut AppState,
    _entry_id: u64,
    _frame_index: Option<usize>,
) -> UpdateResult {
    UpdateResult::none()
}

/// Stub. Body added in Phase 4 Task 03.
pub fn handle_toggle_stack_trace_for_entry(
    _state: &mut AppState,
    _entry_id: u64,
) -> UpdateResult {
    UpdateResult::none()
}
```

In `handler/devtools/inspector.rs` (append before the existing `#[cfg(test)] mod tests { ... }`):

```rust
/// Stub. Body added in Phase 4 Task 04.
pub fn handle_inspector_select_row(
    _state: &mut AppState,
    _index: usize,
) -> UpdateResult {
    UpdateResult::none()
}

/// Stub. Body added in Phase 4 Task 04.
pub fn handle_inspector_toggle_node(
    _state: &mut AppState,
    _index: usize,
) -> UpdateResult {
    UpdateResult::none()
}
```

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes after this task — every new variant has a dispatch arm; every dispatch arm has a stub function.
2. `cargo test --workspace` passes (no behavioural changes — stubs return `UpdateResult::none()`).
3. `cargo fmt --all -- --check` passes.
4. `cargo clippy --workspace --all-targets -- -D warnings` passes — `_state` / `_entry_id` etc. are explicitly underscore-prefixed in the stubs to avoid unused-arg warnings.
5. `Message::ClickLogRow`, `Message::ToggleStackTraceForEntry`, `Message::DevToolsInspectorSelectRow`, `Message::DevToolsInspectorToggleNode` exist with the field shapes specified above.
6. `AppState::new()` sets `last_log_click: None`.
7. `LogClickStamp { entry_id: u64, at: std::time::Instant }` is defined in `state.rs` and is `Copy + Clone + Debug`.
8. Each stub function has a `/// Stub. Body added in Phase 4 Task NN.` doc-comment so reviewers don't mistake it for production logic.

### Testing

Add no production tests in this task — the stubs return `None`, so there is nothing testable yet. Existing tests must continue passing.

If a `tests.rs` already exists in the touched modules, ensure no test references the new variant in a way that would force a stub to do work.

### Notes

- **Why a `Copy` `LogClickStamp`.** `Instant: Copy`, `u64: Copy`. Making `LogClickStamp` `Copy` simplifies the read-then-clear pattern in Task 03 (`let last = state.last_log_click; state.last_log_click = None;` instead of `take()`).
- **Why stub bodies don't `todo!()`.** A `todo!()` panic would fire during integration tests in Task 10 if the dispatch arms get hit before Tasks 03/04 land. Returning `UpdateResult::none()` is harmless — it just means the click is ignored until the body is filled in.
- **No `Eq` derivation needed for new variants.** `Message` already does not derive `Eq` because it carries function pointers and `Box<...>`. The new variants use only `u64`, `Option<usize>`, and `usize` — they are trivially `PartialEq` if the parent enum is. Don't add `Eq` derives.
- **`handler/log_view.rs` exists already** — the file currently houses helpers like `scroll_to_log_entry`. Append the new functions; don't create a new file.
