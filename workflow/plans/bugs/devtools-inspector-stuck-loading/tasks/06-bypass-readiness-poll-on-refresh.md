## Task: Bypass Readiness Poll on `r` Refresh

**Objective**: When the user presses `r` to refresh the widget tree, skip the `isWidgetTreeReady` poll entirely (or use a single-attempt fast-path). The Flutter framework is already running by then, so polling is wasted budget.

**Depends on**: 04-resolve-flutter-ui-isolate

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/inspector/mod.rs`: Plumb a `skip_readiness_poll: bool` field (or an enum `FetchTrigger { Initial, Refresh, AutoRehydrate }`) through `UpdateAction::FetchWidgetTree` and into `spawn_fetch_widget_tree`. When the trigger is `Refresh`, skip `poll_widget_tree_ready`.
- `crates/fdemon-app/src/state.rs`: Add a method to determine whether to skip — e.g., `inspector.has_ever_rendered_tree() -> bool` returns `true` once `root` was successfully populated at least once (sticky flag survives clears).
- `crates/fdemon-app/src/handler/update.rs` (`Message::RequestWidgetTree` handler, lines 1877-1907): Set the trigger field on the `UpdateAction::FetchWidgetTree` it returns.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`: To confirm when `inspector.root` is set/cleared.
- `crates/fdemon-app/src/actions/inspector/widget_tree.rs`: To understand the readiness poll API after task 05.

### Details

Decision: introduce a small enum to make intent explicit at the spawn point:

```rust
// in UpdateAction or as a sibling type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchTrigger {
    /// First fetch after entering DevTools or switching to Inspector panel.
    Initial,
    /// User pressed `r` to refresh; framework is already running.
    Refresh,
    /// Programmatic rehydrate (e.g., after focused panel changes).
    AutoRehydrate,
}
```

In `spawn_fetch_widget_tree`:

```rust
if trigger != FetchTrigger::Refresh {
    poll_widget_tree_ready(&handle, &isolate_id, session_id, &config).await?;
}
let response = try_fetch_widget_tree(...).await?;
```

Setting the trigger:
- `handle_enter_devtools_mode` / `handle_switch_panel(Inspector)` → `FetchTrigger::Initial`.
- `Message::RequestWidgetTree` from `r` → `FetchTrigger::Refresh` **only if** `inspector.has_ever_rendered_tree()`. If the panel hasn't loaded yet (e.g., user pressed `r` during the first load), fall back to `Initial` so polling still applies.

### Acceptance Criteria

1. Pressing `r` after the inspector has ever loaded the tree skips the readiness poll. The RPC fires within ~100 ms of the key press (verifiable via task 01 instrumentation).
2. Initial open path still polls (with the shortened budget from task 05).
3. `inspector.has_ever_rendered_tree()` returns `true` after the first successful render; survives the next fetch start (i.e., does not get cleared by `record_fetch_start`).
4. Unit tests cover: Initial fetch polls, Refresh fetch skips, Refresh-before-first-load polls.
5. `cargo test --workspace` and clippy pass.

### Testing

```rust
#[test]
fn refresh_after_render_skips_readiness_poll() {
    let mut state = AppState::test_default();
    let inspector = active_inspector_state(&mut state).unwrap();
    // Simulate first successful render
    handle_widget_tree_fetched(&mut state, WidgetTreeFetched { /* ... */ });
    let inspector = active_inspector_state(&mut state).unwrap();
    assert!(inspector.has_ever_rendered_tree());

    let result = handle_request_widget_tree(&mut state, /* session id */);
    let action = result.action.unwrap();
    match action {
        UpdateAction::FetchWidgetTree { trigger, .. } => {
            assert_eq!(trigger, FetchTrigger::Refresh);
        }
        _ => panic!("expected FetchWidgetTree"),
    }
}

#[test]
fn refresh_before_first_render_uses_initial_trigger() { /* ... */ }
```

### Notes

- The `has_ever_rendered_tree` flag is a small sticky bit; keep it on `InspectorState`. Reset only on session destruction, not on debounce-clear or fetch-failure.
- If `UpdateAction::FetchWidgetTree` is constructed in multiple places, audit all sites to pass the right trigger value.
- Coordinate with task 04 if the cached isolate id needs invalidation logic that overlaps with this trigger.
