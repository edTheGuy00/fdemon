## Task: Wire properties handlers + extend `handle_open_details` to dispatch the fetch

**Objective**: Add the TEA-side handlers for the three properties-fetch `Message` variants from task 03, and extend `handle_open_details` to dispatch the new `FetchInspectorProperties` action (alongside the existing `FetchLayoutData` dispatch). This is where the cache predicate (`last_fetched_properties_node_id`) and stale-response guard (`pending_properties_node_id`) live.

**Depends on**: 03 (Messages + UpdateAction + state fields)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/update.rs` — route the three new `Message` variants to handler functions
- `crates/fdemon-app/src/handler/devtools/inspector.rs` — three new handler functions + `handle_open_details` extension

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs:301–382` — existing `handle_layout_data_*` handlers (the pattern to mirror)
- `crates/fdemon-app/src/handler/devtools/inspector.rs:544–557` — existing `handle_open_details` (to extend)
- `crates/fdemon-app/src/handler/mod.rs` — `UpdateAction::FetchInspectorProperties` definition (task 03)
- `crates/fdemon-app/src/message.rs` — three new property `Message` variants (task 03)
- `crates/fdemon-app/src/state.rs` — `InspectorState.last_fetched_properties_node_id`, `pending_properties_node_id`, `properties`, `render_properties`, `properties_loading`, `properties_error`

### Details

#### 1. Route the three Messages in `handler/update.rs`

Find the section that routes `LayoutDataFetched / FetchFailed / FetchTimeout` (~lines 2108–2157 per the research). Add three sibling arms:

```rust
Message::DevToolsInspectorPropertiesFetched {
    session_id,
    node_id,
    widget_properties,
    render_properties,
} => devtools::handle_inspector_properties_fetched(
    state,
    session_id,
    node_id,
    widget_properties,
    render_properties,
),

Message::DevToolsInspectorPropertiesFetchFailed { session_id, node_id, error } => {
    devtools::handle_inspector_properties_fetch_failed(state, session_id, node_id, error)
}

Message::DevToolsInspectorPropertiesFetchTimeout { session_id, node_id } => {
    devtools::handle_inspector_properties_fetch_timeout(state, session_id, node_id)
}
```

Also re-export the three handler functions from `handler/devtools/mod.rs` (look at how `handle_layout_data_fetched / _failed / _timeout` are re-exported, around `handler/devtools/mod.rs:16–21` per the research).

#### 2. Three handler functions in `handler/devtools/inspector.rs`

##### `handle_inspector_properties_fetched`

```rust
/// Apply a successful `getProperties` response to `InspectorState`.
///
/// Stale-guarded: if `pending_properties_node_id` no longer matches `node_id`
/// (the user closed Details or selected a different widget mid-flight), the
/// response is discarded silently and `properties_loading` is left untouched
/// (the newer in-flight fetch will resolve it).
pub fn handle_inspector_properties_fetched(
    state: &mut AppState,
    session_id: SessionId,
    node_id: String,
    widget_properties: Vec<DiagnosticsNode>,
    render_properties: Vec<DiagnosticsNode>,
) -> UpdateResult {
    if !is_current_session(state, session_id) {
        return UpdateResult::none();
    }

    let inspector = &mut state.devtools_view_state.inspector;

    // Stale-guard: if the in-flight node id no longer matches, discard.
    if inspector.pending_properties_node_id.as_deref() != Some(node_id.as_str()) {
        return UpdateResult::none();
    }

    inspector.properties = widget_properties;
    inspector.render_properties = render_properties;
    inspector.properties_loading = false;
    inspector.properties_error = None;
    inspector.last_fetched_properties_node_id = inspector.pending_properties_node_id.take();

    UpdateResult::none()
}
```

##### `handle_inspector_properties_fetch_failed`

```rust
pub fn handle_inspector_properties_fetch_failed(
    state: &mut AppState,
    session_id: SessionId,
    node_id: String,
    error: String,
) -> UpdateResult {
    if !is_current_session(state, session_id) {
        return UpdateResult::none();
    }

    let inspector = &mut state.devtools_view_state.inspector;

    if inspector.pending_properties_node_id.as_deref() != Some(node_id.as_str()) {
        return UpdateResult::none();
    }

    inspector.properties_loading = false;
    inspector.properties_error = Some(map_rpc_error(&error));
    inspector.pending_properties_node_id = None;
    // last_fetched_properties_node_id deliberately not updated; cache stays
    // empty so the next Enter retries.

    UpdateResult::none()
}
```

##### `handle_inspector_properties_fetch_timeout`

```rust
pub fn handle_inspector_properties_fetch_timeout(
    state: &mut AppState,
    session_id: SessionId,
    node_id: String,
) -> UpdateResult {
    if !is_current_session(state, session_id) {
        return UpdateResult::none();
    }

    let inspector = &mut state.devtools_view_state.inspector;

    if inspector.pending_properties_node_id.as_deref() != Some(node_id.as_str()) {
        return UpdateResult::none();
    }

    inspector.properties_loading = false;
    inspector.properties_error = Some(DevToolsError::new(
        "Request timed out",
        "Press [r] to retry",
    ));
    inspector.pending_properties_node_id = None;

    UpdateResult::none()
}
```

`is_current_session` is the existing helper used by `handle_layout_data_fetched` (verify with grep). `map_rpc_error` likewise — reuse whatever helper the layout handlers use.

#### 3. Extend `handle_open_details`

The current handler at `inspector.rs:544–557` returns one action (`FetchLayoutData`) when needed. Extend it to also dispatch `FetchInspectorProperties` when needed, applying the cache predicate:

```rust
pub fn handle_open_details(state: &mut AppState) -> UpdateResult {
    let session_id = match state.session_manager.current_session_id() {
        Some(id) => id,
        None => return UpdateResult::none(),
    };

    let inspector = &mut state.devtools_view_state.inspector;
    let Some(node_id) = inspector.selected_value_id() else {
        return UpdateResult::none();
    };

    if inspector.details_open {
        // Already open on the same node — no-op (existing Phase 1 guard).
        return UpdateResult::none();
    }

    inspector.details_open = true;
    inspector.details_tab = DetailsTab::Properties;
    inspector.details_node_id = Some(node_id.clone());

    let mut actions: Vec<UpdateAction> = Vec::new();

    // (A) Properties fetch — skip when cached and no prior error.
    let need_properties = inspector.last_fetched_properties_node_id.as_deref()
        != Some(node_id.as_str())
        || inspector.properties_error.is_some();
    if need_properties && !inspector.properties_loading {
        inspector.properties_loading = true;
        inspector.properties_error = None;
        inspector.pending_properties_node_id = Some(node_id.clone());
        actions.push(UpdateAction::FetchInspectorProperties {
            session_id,
            node_id: node_id.clone(),
            vm_handle: None,
        });
    }

    // (B) Layout fetch — existing logic, kept verbatim.
    let need_layout = inspector.last_fetched_node_id.as_deref() != Some(node_id.as_str())
        && !inspector.layout_loading;
    if need_layout {
        inspector.layout_loading = true;
        inspector.pending_node_id = Some(node_id.clone());
        inspector.layout_last_fetch_time = Some(Instant::now());
        actions.push(UpdateAction::FetchLayoutData {
            session_id,
            node_id,
            vm_handle: None,
        });
    }

    UpdateResult::actions(actions)
}
```

**Critical design check before coding**:
- Verify `UpdateResult::actions(Vec<UpdateAction>)` exists. If `UpdateResult` is currently shaped to carry exactly one optional action, the implementor must either:
  - (a) Extend `UpdateResult` to support a `Vec`, OR
  - (b) Pick one action to return synchronously (e.g., `FetchInspectorProperties`) and emit the other via a follow-up `Message` (e.g., add `Message::RequestLayoutData` is already a chain message per `message.rs:976–999` — reuse it).
- Document the chosen approach in the completion summary.

If approach (b) is chosen: the `handle_open_details` body returns `UpdateResult::action(FetchInspectorProperties)` and synthesizes `Message::RequestLayoutData` if needed (probably via `msg_tx`/`UpdateResult::with_message` if such a primitive exists; otherwise check how `handle_inspector_navigate` chains the layout-fetch — research notes that pattern).

#### 4. Re-exports

Update `handler/devtools/mod.rs` (line 16–21 per the research) to re-export the three new handlers:

```rust
pub use inspector::{
    handle_inspector_properties_fetched,
    handle_inspector_properties_fetch_failed,
    handle_inspector_properties_fetch_timeout,
    handle_layout_data_fetch_failed,
    handle_layout_data_fetch_timeout,
    handle_layout_data_fetched,
    // ... existing re-exports
};
```

### Acceptance Criteria

1. The three new `Message` variants are routed in `handler/update.rs` to their respective handler functions.
2. `handle_inspector_properties_fetched` stores `widget_properties` into `inspector.properties` and `render_properties` into `inspector.render_properties`, updates `last_fetched_properties_node_id`, clears `properties_loading` + `properties_error` + `pending_properties_node_id`.
3. Stale-response guard: when `pending_properties_node_id` doesn't match the response's `node_id`, the response is discarded with no state mutation.
4. Cross-session guard: when `session_id` doesn't match the current session, the response is discarded.
5. `handle_open_details` now dispatches `FetchInspectorProperties` when the cache predicate `last_fetched_properties_node_id != Some(details_node_id) || properties_error.is_some()` is true and `!properties_loading`. Layout fetch dispatch is preserved.
6. Re-opening Details on the same node within the same session does NOT re-dispatch `FetchInspectorProperties` (cache hit).

### Testing

Add to the existing `handler/devtools/inspector.rs` test module (search for `handle_layout_data_fetched_*` tests for the pattern):

```rust
#[test]
fn properties_fetched_stores_into_state() {
    let mut state = make_state_with_inspector_open("objects/42");
    state.devtools_view_state.inspector.pending_properties_node_id = Some("objects/42".into());
    state.devtools_view_state.inspector.properties_loading = true;

    let widget_props = vec![sample_diagnostic("name", "value", None)];
    let render_props = vec![sample_diagnostic("renderObject", "RenderFlex", Some("RenderObject"))];

    handle_inspector_properties_fetched(
        &mut state,
        SessionId::dummy(),
        "objects/42".into(),
        widget_props.clone(),
        render_props.clone(),
    );

    let i = &state.devtools_view_state.inspector;
    assert_eq!(i.properties, widget_props);
    assert_eq!(i.render_properties, render_props);
    assert!(!i.properties_loading);
    assert_eq!(i.last_fetched_properties_node_id.as_deref(), Some("objects/42"));
    assert!(i.pending_properties_node_id.is_none());
}

#[test]
fn properties_fetched_discards_stale_response() {
    let mut state = make_state_with_inspector_open("objects/B");
    state.devtools_view_state.inspector.pending_properties_node_id = Some("objects/B".into());
    state.devtools_view_state.inspector.properties_loading = true;

    // A's response arrives late, while B is in-flight.
    handle_inspector_properties_fetched(
        &mut state,
        SessionId::dummy(),
        "objects/A".into(),
        vec![sample_diagnostic("stale", "stale", None)],
        vec![],
    );

    let i = &state.devtools_view_state.inspector;
    assert!(i.properties.is_empty(), "stale response must not mutate properties");
    assert!(i.properties_loading, "loading flag should still be set for in-flight B");
}

#[test]
fn properties_fetch_timeout_sets_error() {
    let mut state = make_state_with_inspector_open("objects/42");
    state.devtools_view_state.inspector.pending_properties_node_id = Some("objects/42".into());
    state.devtools_view_state.inspector.properties_loading = true;

    handle_inspector_properties_fetch_timeout(&mut state, SessionId::dummy(), "objects/42".into());

    let i = &state.devtools_view_state.inspector;
    assert!(!i.properties_loading);
    let err = i.properties_error.as_ref().expect("error set");
    assert!(err.summary.contains("timed out"));
}

#[test]
fn open_details_cache_hit_skips_dispatch() {
    let mut state = make_state_with_selected_widget("objects/42");
    state.devtools_view_state.inspector.last_fetched_properties_node_id = Some("objects/42".into());
    state.devtools_view_state.inspector.last_fetched_node_id = Some("objects/42".into());

    let result = handle_open_details(&mut state);

    // Cache hits on both layout and properties → no action.
    assert!(result.actions().is_empty(), "no fetch should be dispatched on cache hit");
}

#[test]
fn open_details_dispatches_properties_fetch_on_cache_miss() {
    let mut state = make_state_with_selected_widget("objects/42");
    // No cache.
    let result = handle_open_details(&mut state);

    let actions = result.actions();
    assert!(actions.iter().any(|a| matches!(
        a,
        UpdateAction::FetchInspectorProperties { node_id, .. } if node_id == "objects/42"
    )));
}
```

The helpers `make_state_with_inspector_open`, `make_state_with_selected_widget`, `sample_diagnostic` likely exist in this file's test module already (used for the layout-fetch tests). Confirm by grep; reuse them.

### Notes

- `is_current_session(state, session_id)` may exist under a slightly different name; check the layout handlers for the canonical helper.
- `map_rpc_error(&error)` likewise — reuse whatever wraps an RPC error string in a `DevToolsError` for the existing layout error path.
- This task is one of the larger ones in Phase 2 — ~3 handlers + extension to `handle_open_details` + tests for each + the multi-action decision. The implementor should read this task end-to-end before starting and decide upfront whether to extend `UpdateResult` to support multiple actions (likely cleaner) or to chain via a Message (lower-risk if `UpdateResult` is widely used elsewhere).
- The `pending_properties_node_id` stale-guard pattern is identical to layout's `pending_node_id`; if `handle_layout_data_fetched` has more nuanced guards (e.g., comparing against `selected_value_id()` rather than `pending_node_id`), mirror that nuance here too.
- No changes to mouse-region handling — Phase 2 doesn't add new click targets.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Added `extra_actions: Vec<UpdateAction>` to `UpdateResult`; added `UpdateResult::actions_vec(Vec<UpdateAction>)` constructor; added `actions() -> Vec<UpdateAction>` accessor; updated all direct struct literal initializers |
| `crates/fdemon-app/src/process.rs` | Updated message processing loop to drain both `action` and `extra_actions` via a combined iterator; all actions share the same hydration + dispatch path |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | Added three handler functions (`handle_inspector_properties_fetched`, `handle_inspector_properties_fetch_failed`, `handle_inspector_properties_fetch_timeout`); extended `handle_open_details` to dispatch both `FetchInspectorProperties` and `FetchLayoutData` via `UpdateResult::actions_vec`; updated two existing Phase 1 tests to use `result.actions()` instead of `result.action`; added 8 new tests for the properties handlers |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Re-exported the three new handler functions; added `extra_actions: Vec::new()` to one direct struct literal in `handle_enter_devtools_mode` |
| `crates/fdemon-app/src/handler/update.rs` | Replaced stub match arms for the three `DevToolsInspectorProperties*` messages with real handler dispatch; added `extra_actions: Vec::new()` to two direct struct literal sites |
| `crates/fdemon-app/src/handler/flutter_version/navigation.rs` | Added `extra_actions: Vec::new()` to one direct struct literal site |

### Notable Decisions/Tradeoffs

1. **Multi-action approach — `extra_actions: Vec<UpdateAction>` on `UpdateResult`**: The task noted two options: extend `UpdateResult` to support multiple actions, or use a chain message. Chose option (a) — extend `UpdateResult` — because it keeps the dispatch co-located with the state mutations in `handle_open_details` (no split across two TEA cycles), is easier to test (one `result.actions()` call), and avoids the nuance that a `RequestLayoutData` chain message goes through the `vm_connected` check in `update.rs` which would bypass the state-setting done inside `handle_open_details`. The `extra_actions` field is `pub(crate)` so it doesn't escape to external consumers. The existing `action: Option<UpdateAction>` field is preserved for full backward compatibility; `actions_vec()` packs the first element into `action` so all existing `result.action.is_some()` / `result.action.is_none()` tests on paths that return single-action results continue to work unchanged.

2. **Stale-response guard — `pending_properties_node_id` vs `selected_value_id()`**: The layout handler uses a two-part check comparing `pending_node_id` against `selected_value_id()`. For properties the guard compares `pending_properties_node_id` against the `node_id` argument of the incoming response. This is the pattern described in the task. The semantic is: "is the incoming response for the same widget we last asked for?" — if not, discard. This is correct for properties because unlike layout (which auto-fetches on navigation), properties are only fetched when Details opens, so the pending ID is the authoritative source of truth.

3. **`handle_open_details` borrow splitting**: The new implementation uses two explicit `{}` blocks to scope the mutable borrows of `inspector` (first for properties fetch decision, then for layout fetch decision). This is necessary because `state.session_manager.selected()` must be called between them, and Rust's borrow checker requires the `inspector` borrow to end before accessing other fields of `state`.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2362 fdemon-app lib tests + all other crates)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **`UpdateResult` struct literal sites**: Four places across the codebase directly construct `UpdateResult { message, action }` by name. All four were updated to include `extra_actions: Vec::new()`. If new code is added in future that constructs `UpdateResult` by struct literal it must include the new field — but the compiler will catch this at compile time (the struct is non-exhaustive from a literal perspective since `extra_actions` has no default via `..Default::default()` unless `Default` is derived). Since `#[derive(Default)]` is present and `Vec::new()` is the default, callers could use `UpdateResult { ..Default::default() }` for partial construction.

2. **Two existing `handle_open_details` tests updated**: `handle_open_details_dispatches_fetch_layout_when_data_stale` and `handle_open_details_skips_fetch_when_data_already_cached` were updated to use `result.actions()` since the function now potentially returns two actions. The Phase 1 contract is preserved: layout IS dispatched when stale, and is NOT dispatched when cached — the assertions were rewritten to check the actions collection rather than the single `result.action`.
