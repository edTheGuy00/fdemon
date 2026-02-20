## Task: Performance Polish — Debounce & Lazy Loading

**Objective**: Add debounce/cooldown to rapid DevTools interactions (overlay toggles, tree refresh) to prevent RPC spam, and implement tree depth configuration for large widget trees.

**Depends on**: 02-connection-state-ui (for timeout infrastructure)

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-app/src/handler/devtools.rs`: Add debounce logic for overlay toggles and tree refresh
- `crates/fdemon-app/src/state.rs`: Add cooldown timestamps to `DevToolsViewState`
- `crates/fdemon-app/src/actions.rs`: Pass `tree_max_depth` to widget tree fetch RPC

### Details

#### 1. Overlay Toggle Debounce

**Problem**: The Phase 4 review identified that pressing `Ctrl+r`/`Ctrl+p`/`Ctrl+d` rapidly fires multiple RPC calls in quick succession. Each toggle does a read-then-set cycle (get current bool → set opposite), so rapid presses can interleave and produce unexpected results.

**Solution**: Add a cooldown per overlay toggle. If the same overlay was toggled within the last 500ms, ignore the keypress.

```rust
// In DevToolsViewState:
pub struct DevToolsViewState {
    // ... existing fields ...
    pub last_overlay_toggle: Option<std::time::Instant>,
}

// In handle_toggle_debug_overlay():
fn handle_toggle_debug_overlay(state: &mut AppState, overlay: DebugOverlay) -> Option<UpdateAction> {
    let now = std::time::Instant::now();
    if let Some(last) = state.devtools_view_state.last_overlay_toggle {
        if now.duration_since(last) < Duration::from_millis(500) {
            return None;  // Debounce — too soon
        }
    }
    state.devtools_view_state.last_overlay_toggle = Some(now);
    // ... existing toggle logic ...
}
```

#### 2. Widget Tree Refresh Cooldown

**Problem**: Pressing `r` rapidly in the inspector triggers multiple `FetchWidgetTree` RPCs. Each fetch is expensive (fetches the entire tree from the VM).

**Solution**: Add a cooldown for tree refresh. If a fetch was requested within the last 2 seconds and a request is already in-flight (`loading = true`), ignore the keypress.

```rust
// In handle_request_widget_tree():
fn handle_request_widget_tree(state: &mut AppState) -> Option<UpdateAction> {
    let inspector = &state.devtools_view_state.inspector;

    // Already loading — debounce
    if inspector.loading {
        return None;
    }

    // Cooldown: don't allow refresh within 2 seconds of last fetch
    if let Some(last) = inspector.last_fetch_time {
        if std::time::Instant::now().duration_since(last) < Duration::from_secs(2) {
            return None;
        }
    }

    state.devtools_view_state.inspector.loading = true;
    state.devtools_view_state.inspector.last_fetch_time = Some(std::time::Instant::now());
    // ... dispatch FetchWidgetTree action ...
}
```

Add `last_fetch_time: Option<std::time::Instant>` to `InspectorState`.

#### 3. Tree Depth Configuration

**Problem**: Apps with thousands of widgets produce very large trees that are slow to fetch and parse.

**Solution**: Use the `tree_max_depth` config field (from Task 01) to limit fetch depth.

Check if the Flutter `getRootWidgetTree` / `getRootWidgetSummaryTree` RPC supports a depth parameter. Based on the Flutter DevTools protocol:
- `getRootWidgetSummaryTree` accepts `subtreeDepth` parameter (defaults to unlimited)
- `getRootWidgetTree` also accepts `subtreeDepth`

Pass `subtreeDepth` when `tree_max_depth > 0`:

```rust
// In spawn_fetch_widget_tree():
let mut params = serde_json::Map::new();
params.insert("objectGroup".into(), json!("fdemon-inspector-1"));
if tree_max_depth > 0 {
    params.insert("subtreeDepth".into(), json!(tree_max_depth));
}
```

When `tree_max_depth = 0`, omit the parameter (unlimited depth, current behavior).

#### 4. Layout Fetch Debounce

Apply the same loading-check debounce to layout data fetches. The layout fetch already happens on panel switch, but if the user rapidly switches back and forth between Inspector and Layout, multiple fetches could be triggered.

Check: does switching to Layout always trigger a fetch, or only if data is stale? If it always fetches, add a staleness check — only re-fetch if the selected inspector node changed since the last layout fetch.

### Acceptance Criteria

1. Rapid `Ctrl+r`/`Ctrl+p`/`Ctrl+d` presses (within 500ms) only trigger one RPC call
2. Rapid `r` presses in inspector (within 2s) only trigger one tree fetch
3. `tree_max_depth` config value is passed as `subtreeDepth` to the RPC when > 0
4. `tree_max_depth = 0` fetches the full tree (current behavior)
5. Layout fetch is not re-triggered if the selected node hasn't changed
6. No regressions in normal overlay toggle or tree refresh behavior

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_toggle_debounce_blocks_rapid_presses() {
        let mut state = AppState::default();
        // First toggle — should succeed
        state.devtools_view_state.last_overlay_toggle = None;
        let result = handle_toggle_debug_overlay(&mut state, DebugOverlay::RepaintRainbow);
        assert!(result.is_some());

        // Immediate second toggle — should be debounced
        let result = handle_toggle_debug_overlay(&mut state, DebugOverlay::RepaintRainbow);
        assert!(result.is_none());
    }

    #[test]
    fn test_overlay_toggle_allowed_after_cooldown() {
        let mut state = AppState::default();
        state.devtools_view_state.last_overlay_toggle =
            Some(std::time::Instant::now() - Duration::from_secs(1));
        let result = handle_toggle_debug_overlay(&mut state, DebugOverlay::RepaintRainbow);
        assert!(result.is_some());
    }

    #[test]
    fn test_tree_refresh_debounce_while_loading() {
        let mut state = AppState::default();
        state.devtools_view_state.inspector.loading = true;
        let result = handle_request_widget_tree(&mut state);
        assert!(result.is_none());
    }

    #[test]
    fn test_tree_refresh_debounce_cooldown() {
        let mut state = AppState::default();
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.last_fetch_time =
            Some(std::time::Instant::now());
        let result = handle_request_widget_tree(&mut state);
        assert!(result.is_none()); // Within 2-second cooldown
    }
}
```

### Notes

- **The 500ms overlay debounce and 2s refresh cooldown** are sensible defaults. These could be made configurable later but hard-coding is fine for Phase 5.
- **`Instant::now()` in handlers**: The TEA pattern prefers pure functions, but `Instant::now()` is a minor pragmatic exception already used elsewhere (e.g., debounce in the file watcher). It doesn't affect testability since we can set `last_*_time` directly in tests.
- **`subtreeDepth` support**: Verify this parameter exists in the Flutter version(s) that fdemon targets. If not supported, the parameter will be silently ignored by the VM.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `last_fetch_time: Option<Instant>` to `InspectorState`; added `is_fetch_debounced()` and `record_fetch_start()` methods; updated `reset()` to clear `last_fetch_time`. Added `last_fetched_node_id: Option<String>` and `pending_node_id: Option<String>` to `LayoutExplorerState`; updated `reset()` to clear both. |
| `crates/fdemon-app/src/handler/update.rs` | Added 2-second cooldown check (`is_fetch_debounced()`) to `Message::RequestWidgetTree` handler; replaced manual `loading = true` with `record_fetch_start()`; added `pending_node_id` tracking to `Message::RequestLayoutData` handler. |
| `crates/fdemon-app/src/handler/devtools.rs` | Updated `handle_switch_panel` (Layout branch) to skip fetch when `last_fetched_node_id` matches selected node; added `pending_node_id` tracking when fetch starts. Updated `handle_layout_data_fetched` to promote `pending_node_id` to `last_fetched_node_id` on success. Updated `handle_layout_data_fetch_failed` and `handle_layout_data_fetch_timeout` to clear `pending_node_id` on failure. Added 13 new unit tests. |

### Notable Decisions/Tradeoffs

1. **`last_fetch_time` set at fetch start, not completion**: The cooldown starts when a fetch is dispatched, not when it completes. This prevents queue buildup — if a fetch takes 3s, the cooldown is already expired by the time the next press arrives. This matches the task spec: "Set `last_fetch_time` when starting a fetch."

2. **`pending_node_id` rendezvous pattern**: Since `LayoutDataFetched` doesn't carry the node ID, a `pending_node_id` field is stored in `LayoutExplorerState` when dispatch happens and promoted to `last_fetched_node_id` on success. This avoids changing the `Message` type and keeps all state co-located.

3. **Overlay toggle debounce already complete**: `last_overlay_toggle`, `is_overlay_toggle_debounced()`, and `record_overlay_toggle()` were already implemented by Task 05, as noted in the task context. Verified present in `state.rs` and wired into `update.rs` for `Message::ToggleDebugOverlay`. No further changes needed.

4. **`subtreeDepth` already wired**: `tree_max_depth` was already passed as `subtreeDepth` in `spawn_fetch_widget_tree` in `actions.rs` (lines 941-943). Verified present. No changes needed.

5. **Pre-existing clippy warning**: `handler/session.rs:43` has a pre-existing `unnecessary_unwrap` warning that predates this task, as documented in the task instructions. All new code is warning-free.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app` — Passed (882 tests, 13 new tests added)
- `cargo check --workspace` — Passed
- `cargo clippy --workspace` — 1 pre-existing warning in `session.rs:43` (excluded per task instructions), no new warnings

### Risks/Limitations

1. **Cooldown bypasses on timeout**: If `WidgetTreeFetchTimeout` fires, `loading` is set to `false` but `last_fetch_time` remains set from the fetch start. The user must wait up to 2 seconds before retrying. This is the intended behavior per the task spec, and the timeout error message already says "Press [r] to retry."

2. **Layout staleness on session switch**: `DevToolsViewState::reset()` calls `layout_explorer.reset()` which clears `last_fetched_node_id`, so session switches always trigger a fresh fetch. This is correct behavior.
