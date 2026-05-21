## Task: Add `FetchInspectorProperties` UpdateAction, Messages, hydration, and state cache fields

**Objective**: Scaffold the TEA plumbing for the Phase 2 properties fetch by adding (a) a new `UpdateAction::FetchInspectorProperties` variant, (b) three result `Message` variants (Fetched / FetchFailed / FetchTimeout), (c) the engine-side hydration that injects the `VmRequestHandle` into the action, and (d) two new cache-tracking fields on `InspectorState`. This task is intentionally pure scaffolding — no handler logic, no spawn task — to keep wave-1 work file-disjoint and parallel-safe.

**Depends on**: None

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mod.rs` — new `UpdateAction::FetchInspectorProperties` variant
- `crates/fdemon-app/src/message.rs` — new `Message::DevToolsInspectorPropertiesFetched / PropertiesFetchFailed / PropertiesFetchTimeout` variants
- `crates/fdemon-app/src/process.rs` — new `hydrate_fetch_inspector_properties` function + fallback dispatch in `route_message`
- `crates/fdemon-app/src/state.rs` — two new fields on `InspectorState` + matching clears in `reset()` and `reset_details_and_groups()`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — `VmRequestHandle` import path
- `crates/fdemon-app/src/handler/mod.rs:301–306` — existing `FetchLayoutData` shape (reference pattern)
- `crates/fdemon-app/src/message.rs:982–999` — existing `LayoutDataFetched / FetchFailed / FetchTimeout` shape (reference pattern)
- `crates/fdemon-app/src/process.rs:273–297` — existing `hydrate_fetch_layout_data` (reference pattern)
- `crates/fdemon-app/src/state.rs:239–353` — existing `InspectorState` field layout + reset functions

### Details

#### 1. `UpdateAction::FetchInspectorProperties` variant

In `crates/fdemon-app/src/handler/mod.rs`, near the existing `FetchLayoutData` variant (~line 301):

```rust
/// Fetch widget properties + render-object properties for the given widget.
///
/// Dispatched by `handle_open_details` when the selected widget changes.
/// The spawn task makes one `ext.flutter.inspector.getProperties` RPC, splits
/// the result into widget/render properties, then makes one further
/// `getProperties` per render-object property to fetch its sub-properties.
///
/// `vm_handle` is `None` when emitted from a handler; the engine hydrates it
/// in `process.rs::hydrate_fetch_inspector_properties` before dispatch.
FetchInspectorProperties {
    session_id: SessionId,
    node_id: String,
    vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
},
```

Place it immediately after `FetchLayoutData` to keep the two-related actions visually grouped.

#### 2. Three new `Message` variants

In `crates/fdemon-app/src/message.rs`, after the existing `LayoutDataFetchTimeout` variant (~line 997):

```rust
/// `ext.flutter.inspector.getProperties` succeeded.
///
/// `widget_properties` is the partition with `propertyType != "RenderObject"`;
/// `render_properties` contains the render-object nodes plus (already merged
/// in by the spawn task) the sub-properties of each render object.
DevToolsInspectorPropertiesFetched {
    session_id: SessionId,
    node_id: String,
    widget_properties: Vec<DiagnosticsNode>,
    render_properties: Vec<DiagnosticsNode>,
},

/// `getProperties` returned an error or the response failed to parse.
DevToolsInspectorPropertiesFetchFailed {
    session_id: SessionId,
    node_id: String,
    error: String,
},

/// `getProperties` exceeded its 10-second timeout.
DevToolsInspectorPropertiesFetchTimeout {
    session_id: SessionId,
    node_id: String,
},
```

The `node_id` field on each variant lets the handler stale-guard against rapid Enter→Esc→Enter cycles on different widgets (see task 06's `pending_properties_node_id` check).

Make sure `DiagnosticsNode` is imported at the top of `message.rs` (it likely is already, for the existing widget tree messages — verify with grep).

#### 3. `hydrate_fetch_inspector_properties` in `process.rs`

Mirror the existing `hydrate_fetch_layout_data` at `process.rs:273–297`. Add a sibling function:

```rust
fn hydrate_fetch_inspector_properties(
    action: UpdateAction,
    state: &AppState,
) -> Option<UpdateAction> {
    let UpdateAction::FetchInspectorProperties {
        session_id,
        node_id,
        vm_handle,
    } = action
    else {
        return Some(action);
    };

    let handle = vm_handle.or_else(|| {
        state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())
    });

    Some(UpdateAction::FetchInspectorProperties {
        session_id,
        node_id,
        vm_handle: handle,
    })
}
```

Wire it into the dispatch chain in `route_message` (look for where `hydrate_fetch_layout_data` is invoked, ~line 119–157 per the research). Pattern:

```rust
let action = match action {
    UpdateAction::FetchInspectorProperties { .. } => {
        hydrate_fetch_inspector_properties(action, &state)
    }
    UpdateAction::FetchLayoutData { .. } => hydrate_fetch_layout_data(action, &state),
    other => Some(other),
};
```

Also add a fallback in the no-handle branch — if `vm_handle` is `None` after hydration (no active VM service connection for this session), emit `DevToolsInspectorPropertiesFetchFailed { session_id, node_id, error: "no VM Service handle".into() }`. Pattern mirrors the existing `LayoutDataFetchFailed` fallback in `process.rs:119–132`.

#### 4. `InspectorState` cache fields

In `crates/fdemon-app/src/state.rs`, locate the `InspectorState` struct (around line 239) and add two new fields adjacent to the existing `properties` / `render_properties` fields (~lines 340–352):

```rust
/// `value_id` of the last widget whose properties were successfully fetched.
/// Used as a cache key by `handle_open_details` to skip re-dispatch when the
/// user closes + reopens Details on the same node.
pub last_fetched_properties_node_id: Option<String>,

/// `value_id` of the in-flight properties fetch, if any. Used as a stale
/// guard in `handle_properties_fetched`: if the user closes Details or
/// switches to a different node mid-flight, the late response is discarded.
pub pending_properties_node_id: Option<String>,
```

These mirror `last_fetched_node_id` and `pending_node_id` which already exist for the layout fetch (`state.rs:268–272`).

Update `Default for InspectorState` (and any explicit constructors) to initialize both fields to `None`.

#### 5. Reset functions

In `InspectorState::reset()` (`state.rs:413–443`), add clears for both new fields after the existing `properties` / `render_properties` clears:

```rust
self.last_fetched_properties_node_id = None;
self.pending_properties_node_id = None;
```

Same in `InspectorState::reset_details_and_groups()` (`state.rs:458–467`).

Order matters: both new fields go in BOTH reset paths so that:
- Session switch (full `reset()`) clears the cache.
- Tree refresh / hot restart (`reset_details_and_groups()`) clears the cache, forcing a re-fetch on the next Enter.

### Acceptance Criteria

1. `UpdateAction::FetchInspectorProperties` compiles and is exhaustively matched in all existing `match action` sites (compiler enforces).
2. The three new `Message` variants compile and are exhaustively matched in `handler/update.rs::route_message` (task 06 handles the actual routing; for this task it's acceptable to add a `_ => UpdateResult::none()` stub or a TODO match arm — but the `match` must be exhaustive).

   **Workflow note**: To keep task 03 file-disjoint from task 06, this task may add a temporary placeholder match arm pattern. The simplest pattern: leave the `Message` variants unmatched and let task 06 fail compilation on the missing arms, OR add a `Message::DevToolsInspectorPropertiesFetched { .. } | Message::DevToolsInspectorPropertiesFetchFailed { .. } | Message::DevToolsInspectorPropertiesFetchTimeout { .. } => UpdateResult::none()` stub line that task 06 will replace. Pick whichever the implementor finds cleaner.
3. `hydrate_fetch_inspector_properties` correctly clones the `VmRequestHandle` from `SessionManager`, returns the hydrated action, and the no-handle fallback emits `DevToolsInspectorPropertiesFetchFailed` (via the existing `msg_tx` channel).
4. `InspectorState.last_fetched_properties_node_id` and `pending_properties_node_id` exist, default to `None`, and are cleared by both `reset()` and `reset_details_and_groups()`.
5. `cargo check --workspace` passes after this task alone (with the stub Message match arm from #2).

### Testing

This task is mostly type/scaffolding; the existing test suite should continue to pass. Add focused unit tests for the new state-reset behavior:

```rust
#[test]
fn reset_clears_properties_cache_fields() {
    let mut state = InspectorState::default();
    state.last_fetched_properties_node_id = Some("objects/42".into());
    state.pending_properties_node_id = Some("objects/43".into());
    state.reset();
    assert!(state.last_fetched_properties_node_id.is_none());
    assert!(state.pending_properties_node_id.is_none());
}

#[test]
fn reset_details_and_groups_clears_properties_cache_fields() {
    let mut state = InspectorState::default();
    state.last_fetched_properties_node_id = Some("objects/42".into());
    state.pending_properties_node_id = Some("objects/43".into());
    state.reset_details_and_groups();
    assert!(state.last_fetched_properties_node_id.is_none());
    assert!(state.pending_properties_node_id.is_none());
}
```

Place these in the existing `state.rs` test module (where the other `reset_*` tests already live — grep for them to confirm location).

For the hydration: add an integration-style test (or a `process.rs`-local test if the file already has tests) verifying that an action with `vm_handle: None` returns `Some(_)` with a hydrated handle when the session has one, and emits the failure message when it doesn't. Pattern: copy the existing `hydrate_fetch_layout_data` test if one exists; otherwise this is mostly covered indirectly by task 05's tests.

### Notes

- **Why a separate scaffolding task?** Splitting the Message + Action + state-field scaffolding into wave 1 lets the daemon (task 02), the spawn task (task 05), and the handlers (task 06) all proceed without serializing through one big "everything in app crate" task. The cost is that this task leaves dangling/unhandled message variants; task 06 picks them up. Keep the stub arm minimal — task 06 will replace it.
- **Why store `node_id` in every result Message?** The handler in task 06 uses it as the stale-guard key. The user can press Enter on widget A, press Esc, navigate to widget B, press Enter — the widget-A fetch may still complete and produce a response. Without `node_id` in the message we can't tell which fetch's response we just got. Mirrors the existing layout pattern (which doesn't include `node_id` on the result message but compensates via `pending_node_id` comparison; including `node_id` here is a small ergonomic improvement).
- **`InspectorState` is also touched lightly by task 06** (`handle_open_details` will read these new fields) — but task 06 only READS them, doesn't add new ones. So no write conflict.
- **`fdemon_daemon::vm_service::VmRequestHandle`** — confirm the exact import path matches what `FetchLayoutData` already uses; the research shows this is the canonical path.
- The line count for `state.rs` is currently ~470+ lines; +2 fields + 2 reset lines + tests should land it well below 500.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Added `FetchInspectorProperties` variant after `FetchLayoutData` |
| `crates/fdemon-app/src/message.rs` | Added three new variants: `DevToolsInspectorPropertiesFetched`, `DevToolsInspectorPropertiesFetchFailed`, `DevToolsInspectorPropertiesFetchTimeout` |
| `crates/fdemon-app/src/handler/update.rs` | Added stub match arm for the three new Message variants (returns `UpdateResult::none()`) |
| `crates/fdemon-app/src/process.rs` | Added `hydrate_fetch_inspector_properties` function + wired into hydration chain + fallback dispatch for no-handle case |
| `crates/fdemon-app/src/actions/mod.rs` | Added `FetchInspectorProperties` match arm (stub warning log; task 05 will implement spawn) |
| `crates/fdemon-app/src/state.rs` | Added `last_fetched_properties_node_id` and `pending_properties_node_id` fields, initialized in `Default`, cleared in `reset()` and `reset_details_and_groups()`, plus two unit tests |
| `crates/fdemon-tui/src/runner.rs` | Added `FetchInspectorProperties` to non-runner variants list in `handle_runner_actions` |

### Notable Decisions/Tradeoffs

1. **`hydrate_fetch_inspector_properties` returns `Some` even when `vm_handle` is still `None`**: Unlike `hydrate_fetch_widget_tree` which returns `None` (discards) when no handle is available, the properties hydration follows the lighter `hydrate_fetch_layout_data` pattern — it returns `Some` with `vm_handle: None`. The fallback dispatch in `process_message`'s no-handle branch then emits `DevToolsInspectorPropertiesFetchFailed`. This ensures the loading spinner is never stuck because the `handle_action` side already logs a warning for the no-handle case. The task spec requested this specific pattern.

2. **Stub in `actions/mod.rs`**: Added a warn-log stub arm for `FetchInspectorProperties` in `handle_action` so the compiler is satisfied. Task 05 will replace this with the actual `spawn_fetch_inspector_properties` call.

3. **Clippy field_reassign_with_default**: Test initialization was refactored to use struct initializer syntax (`InspectorState { field: val, ..Default::default() }`) to satisfy the `-D warnings` clippy gate.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all test suites green, including new `reset_clears_properties_cache_fields` and `reset_details_and_groups_clears_properties_cache_fields` tests)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Stub arms**: `update.rs` has a TODO stub returning `UpdateResult::none()` for the three new Message variants. Task 06 must replace this. Until then, received properties responses are silently dropped (properties tab will show empty).
2. **`actions/mod.rs` stub**: `FetchInspectorProperties` in `handle_action` logs a warning and does nothing. Task 05 implements the spawn task.
