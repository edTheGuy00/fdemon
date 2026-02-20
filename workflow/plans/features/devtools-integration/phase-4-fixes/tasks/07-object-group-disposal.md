## Task: Add VM Object Group Disposal

**Objective**: Dispose VM object groups when refreshing the widget tree and when exiting DevTools mode, preventing memory leaks on the Flutter VM side during long debugging sessions.

**Depends on**: 02-fix-vm-connection, 04-session-switch-reset

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/actions.rs`: Add `disposeGroup` call before new tree/layout fetches
- `crates/fdemon-app/src/state.rs`: Track current object group names in `InspectorState` and `LayoutExplorerState`
- `crates/fdemon-app/src/handler/devtools.rs`: Dispose groups on DevTools exit
- `crates/fdemon-app/src/handler/mod.rs`: Add `DisposeObjectGroups` action variant (optional)

### Details

#### Current Problem

`spawn_fetch_widget_tree` (actions.rs:865) creates VM object group `"fdemon-inspector-1"` and `spawn_fetch_layout_data` (actions.rs:1059) creates `"devtools-layout"`. Neither ever calls `disposeGroup`. Each fetch implicitly creates references in the VM that are held by the object group. Over many refreshes (user pressing `r` repeatedly), memory accumulates on the Flutter VM.

The daemon crate already has `ObjectGroupManager` (extensions/inspector.rs:52-151) with `dispose_all()` and `create_group()`, but the actions layer uses `VmRequestHandle` directly and doesn't leverage this.

#### Fix Strategy

Since the actions layer works with `VmRequestHandle` (not `VmServiceClient`), the simplest approach is to issue a raw `disposeGroup` extension call before each new fetch.

**Step 1 — Dispose before re-fetch** (actions.rs):

In `spawn_fetch_widget_tree`, before the `call_extension` for the new tree:

```rust
// Dispose previous object group before creating a new one
let mut dispose_args = HashMap::new();
dispose_args.insert("objectGroup".to_string(), object_group.to_string());
let _ = handle.call_extension(
    "ext.flutter.inspector.disposeGroup",
    &isolate_id,
    Some(dispose_args),
).await;
// Ignore result — disposal failure is non-fatal
```

Apply the same pattern in `spawn_fetch_layout_data` for group `"devtools-layout"`.

**Step 2 — Dispose on DevTools exit** (handler/devtools.rs + actions.rs):

When the user presses `Esc` to exit DevTools mode, dispose both object groups. This requires an action since it's an RPC call (side effect).

Option A (simple): Add an `UpdateAction::DisposeDevToolsGroups { session_id, vm_handle: Option<VmRequestHandle> }` that disposes both `"fdemon-inspector-1"` and `"devtools-layout"`. Return it from `handle_exit_devtools_mode`. Hydrate in `process.rs`.

Option B (simpler): Don't add a new action. Just let the disposal happen naturally on the next fetch. Accept that groups leak if the user exits DevTools without re-entering. This is acceptable for a first pass — the groups are small.

**Recommended: Option A** for correctness, but Option B is acceptable if time is constrained.

**Step 3 — Track group existence** (state.rs — optional):

Add boolean flags to track whether groups exist:

```rust
pub struct InspectorState {
    // ... existing fields ...
    pub has_object_group: bool,
}
```

Set `true` after successful fetch, `false` after disposal. Use this to skip disposal calls when no group exists (avoids unnecessary RPC).

#### Extension Call Details

The Flutter inspector extension for disposing groups:
- Method: `ext.flutter.inspector.disposeGroup`
- Parameters: `{ "objectGroup": "<group-name>" }`
- Response: ignored (void)

The extension is available when `ext.flutter.inspector` is registered (which happens when the inspector service extension is loaded). If the extension isn't available, the call will fail silently (which is fine).

### Acceptance Criteria

1. Before each widget tree fetch, the previous `"fdemon-inspector-1"` group is disposed
2. Before each layout data fetch, the previous `"devtools-layout"` group is disposed
3. Disposal failures are logged at debug level but do not block the fetch
4. (If Option A) Exiting DevTools mode disposes both groups
5. All existing tests pass
6. No regressions in widget tree or layout explorer functionality

### Testing

```rust
// Testing disposal is tricky since it requires a VM Service mock.
// Focus on testing that the disposal call is made (via a mock/spy handle)
// or that the state tracking flags are correct.

#[test]
fn test_inspector_has_object_group_set_after_fetch() {
    // After WidgetTreeFetched message:
    // state.devtools_view_state.inspector.has_object_group == true
}

#[test]
fn test_inspector_has_object_group_cleared_after_reset() {
    // After InspectorState::reset():
    // state.devtools_view_state.inspector.has_object_group == false
}
```

### Notes

- The `ObjectGroupManager` in `fdemon-daemon/src/vm_service/extensions/inspector.rs` is the proper abstraction, but it requires a `VmServiceClient` reference. The actions layer only has `VmRequestHandle`. Long-term, the actions should use typed extension wrappers. For now, inline `disposeGroup` calls are acceptable.
- `disposeGroup` is idempotent — calling it on a non-existent group returns successfully. So calling it before the first fetch (when no group exists yet) is safe.
- The fixed group names (`"fdemon-inspector-1"`, `"devtools-layout"`) mean only one group of each type exists at a time. This is correct for a single-inspector view. If multi-pane inspector is added later, group names should include a unique suffix.
- This task is lower priority than the critical fixes but prevents real VM memory leaks. It should be completed before the phase 4 fixes are considered done.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `has_object_group: bool` to `InspectorState` and `LayoutExplorerState`; clear the flag in both `reset()` methods |
| `crates/fdemon-app/src/handler/mod.rs` | Added `UpdateAction::DisposeDevToolsGroups { session_id, vm_handle }` variant |
| `crates/fdemon-app/src/handler/devtools.rs` | Updated `handle_exit_devtools_mode` to return `DisposeDevToolsGroups` action when VM is connected; set `has_object_group = true` in `handle_widget_tree_fetched` and `handle_layout_data_fetched` |
| `crates/fdemon-app/src/actions.rs` | Added `disposeGroup` call before each widget tree fetch and layout data fetch; added `spawn_dispose_devtools_groups` function; handled `DisposeDevToolsGroups` in `handle_action` dispatch |
| `crates/fdemon-app/src/process.rs` | Added `hydrate_dispose_devtools_groups` function and wired it into the hydration chain in `process_message` |
| `crates/fdemon-app/src/handler/tests.rs` | Added 8 new tests covering `has_object_group` tracking, reset behavior, and `DisposeDevToolsGroups` action generation |

### Notable Decisions/Tradeoffs

1. **Option A implemented (dispose on exit)**: The task recommended Option A for correctness. `handle_exit_devtools_mode` now returns `DisposeDevToolsGroups` when a VM connection is active. Hydration in `process.rs` follows the same pattern as other VM-handle actions — silently discards when no handle is available.

2. **Always-dispose pattern in actions**: The `disposeGroup` call is issued unconditionally before each tree/layout fetch (not guarded by `has_object_group`). This matches the "idempotent" guarantee from the task description and simplifies the action layer. The `has_object_group` flag is available for future optimizations but is not currently used as a gate in `actions.rs`.

3. **Non-fatal disposal failures**: All disposal failures are logged at `tracing::debug` level and do not block the subsequent fetch. This follows the task's "Ignore result — disposal failure is non-fatal" requirement.

4. **Disposal in a loop**: `spawn_dispose_devtools_groups` iterates over both group names in a for loop rather than two separate awaits, keeping the pattern DRY and easy to extend.

5. **Pre-existing clippy errors in fdemon-tui**: `cargo clippy --workspace -- -D warnings` fails due to 8 pre-existing `needless_borrows_for_generic_args` warnings in `fdemon-tui`. These are unrelated to this task and were present before these changes. `cargo clippy -p fdemon-app -- -D warnings` passes cleanly.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test -p fdemon-app` — Passed (845 unit tests + 1 doc-test; 8 new tests added)
- `cargo clippy -p fdemon-app -- -D warnings` — Passed
- `cargo clippy --workspace -- -D warnings` — Pre-existing fdemon-tui failures (not introduced by this task)

### Risks/Limitations

1. **No disposal on session close**: If the user closes a DevTools-active session without pressing Esc first, the object groups on the Flutter VM are not explicitly disposed. They will be cleaned up naturally when the Flutter process exits. A future improvement could trigger disposal on `SessionClosed` events when DevTools was active.

2. **`has_object_group` not used as a guard in actions.rs**: The flag tracks state but doesn't currently short-circuit the `disposeGroup` RPC call. Since `disposeGroup` is idempotent, this is safe but does issue one extra RPC on the first fetch. Using the flag as a guard would require passing it from state to the action, which would add complexity without meaningful benefit given the idempotency guarantee.
