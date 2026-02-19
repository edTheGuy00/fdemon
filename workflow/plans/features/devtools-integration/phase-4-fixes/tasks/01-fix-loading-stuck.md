## Task: Fix Loading State Stuck Forever When VM Not Connected

**Objective**: Prevent the Inspector and Layout Explorer panels from showing a permanent "Loading..." spinner when the VM Service is not connected. Guard `loading = true` behind `vm_connected` checks and ensure hydration discards produce user-visible errors.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Guard `RequestWidgetTree` and `RequestLayoutData` handlers
- `crates/fdemon-app/src/process.rs`: Send failure messages when hydration discards actions
- `crates/fdemon-app/src/handler/tests.rs`: Add regression tests

### Details

#### Root Cause

`RequestWidgetTree` (update.rs:1317-1323) unconditionally sets `inspector.loading = true`, then returns `UpdateAction::FetchWidgetTree { vm_handle: None }`. In `process.rs:106-128`, `hydrate_fetch_widget_tree` returns `None` when `vm_request_handle` is absent. The `and_then` chain (process.rs:49) silently discards the action. No `WidgetTreeFetchFailed` message is ever sent, so `loading` stays `true` forever.

The same pattern affects `RequestLayoutData` (update.rs:1333-1343) and `hydrate_fetch_layout_data` (process.rs:135-160).

#### Correct Pattern (Already Exists)

`handle_enter_devtools_mode` (devtools.rs:23-37) and `handle_switch_panel` (devtools.rs:53-105) correctly guard on `vm_connected` before setting `loading = true`:

```rust
if handle.session.vm_connected {
    state.devtools_view_state.inspector.loading = true;
    return UpdateResult::action(UpdateAction::FetchWidgetTree { ... });
}
```

#### Fix Strategy (Two-Layer Defense)

**Layer 1 — Guard in update.rs** (prevents the bug):

In the `RequestWidgetTree` handler, check `vm_connected` before setting `loading`:

```rust
Message::RequestWidgetTree { session_id } => {
    let vm_connected = state.session_manager.get(session_id)
        .map(|h| h.session.vm_connected)
        .unwrap_or(false);

    if vm_connected {
        state.devtools_view_state.inspector.loading = true;
        UpdateResult::action(UpdateAction::FetchWidgetTree {
            session_id,
            vm_handle: None,
        })
    } else {
        state.devtools_view_state.inspector.error =
            Some("VM Service not connected — cannot fetch widget tree".to_string());
        UpdateResult::none()
    }
}
```

Apply the same pattern to `RequestLayoutData`.

**Layer 2 — Failure message from process.rs** (defense in depth):

When hydration discards an action, send a failure message back via `msg_tx`. This handles the race condition where `vm_connected` is true when the handler runs but the handle disappears before hydration. In `process_message` (process.rs), after the hydration chain, check if the original action was `FetchWidgetTree` or `FetchLayoutData` but the hydrated result is `None`:

```rust
// After hydration chain:
if action.is_none() {
    // Check if the pre-hydration action was a fetch that got discarded
    match &pre_hydration_action {
        Some(UpdateAction::FetchWidgetTree { session_id, .. }) => {
            let _ = msg_tx.try_send(Message::WidgetTreeFetchFailed {
                session_id: *session_id,
                error: "VM Service handle unavailable".to_string(),
            });
        }
        Some(UpdateAction::FetchLayoutData { session_id, .. }) => {
            let _ = msg_tx.try_send(Message::LayoutDataFetchFailed {
                session_id: *session_id,
                error: "VM Service handle unavailable".to_string(),
            });
        }
        _ => {}
    }
}
```

This requires saving the action before the hydration chain runs (`let pre_hydration_action = action.clone();` or capture the variant).

### Acceptance Criteria

1. Pressing `r` in Inspector when VM is not connected shows an error message instead of stuck loading
2. Pressing `r` in Inspector when VM IS connected still works (loads widget tree)
3. If VM disconnects between handler and hydration, a failure message clears loading state
4. Same behavior for Layout Explorer data fetch
5. `handle_enter_devtools_mode` and `handle_switch_panel` continue to work unchanged
6. All existing tests pass + new regression tests added

### Testing

```rust
#[test]
fn test_request_widget_tree_without_vm_sets_error() {
    // Create state with a session but vm_connected = false
    // Send Message::RequestWidgetTree
    // Assert: inspector.loading == false
    // Assert: inspector.error == Some("VM Service not connected...")
}

#[test]
fn test_request_widget_tree_with_vm_sets_loading() {
    // Create state with a session and vm_connected = true
    // Send Message::RequestWidgetTree
    // Assert: inspector.loading == true
    // Assert: action is Some(FetchWidgetTree { .. })
}

#[test]
fn test_request_layout_data_without_vm_sets_error() {
    // Same pattern for layout
}
```

### Notes

- The `hydrate_toggle_overlay` function (process.rs:162-191) has the same discard pattern but doesn't set a loading flag, so it doesn't cause a stuck UI. No fix needed there.
- `pre_hydration_action` capture: `UpdateAction` derives `Clone` (check mod.rs), so `.clone()` before the chain works. If not, capture just the variant discriminant and session_id.
- The error message should be user-friendly: "VM Service not connected" not "vm_request_handle is None"

---

## Completion Summary

**Status:** Not Started
