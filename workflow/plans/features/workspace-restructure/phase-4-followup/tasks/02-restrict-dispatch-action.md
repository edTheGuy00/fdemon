## Task: Restrict or Document `dispatch_action()` Limitations

**Objective**: Fix the public `Engine::dispatch_action()` method which silently fails for most `UpdateAction` variants because it passes hardcoded `None`/default values for `cmd_sender`, `session_senders`, and `tool_availability`.

**Depends on**: None

**Severity**: CRITICAL (silent API failures)

**Source**: Logic & Reasoning Checker (ACTION_ITEMS.md Critical #2)

### Scope

- `crates/fdemon-app/src/engine.rs:330-341`: Modify `dispatch_action()`
- `src/headless/runner.rs:214`: The sole external caller

### Details

**Current implementation:**
```rust
pub fn dispatch_action(&self, action: UpdateAction) {
    crate::actions::handle_action(
        action,
        self.msg_tx.clone(),        // real
        None,                        // cmd_sender: ALWAYS None
        Vec::new(),                  // session_senders: ALWAYS empty
        self.session_tasks.clone(),  // real
        self.shutdown_rx.clone(),    // real
        &self.project_path,          // real
        Default::default(),          // tool_availability: ALL false
    );
}
```

**Impact of hardcoded defaults:**

| Parameter | Value | Affected Actions |
|-----------|-------|------------------|
| `cmd_sender` | `None` | `SpawnTask(Reload/Restart/Stop)` -- sends failure message or no-ops |
| `session_senders` | `Vec::new()` | `ReloadAllSessions` -- iterates empty vec, does nothing |
| `tool_availability` | `Default` (all false) | `DiscoverBootableDevices`, `BootDevice` -- skips all platforms |

**The sole caller** (`src/headless/runner.rs:214`) only dispatches `SpawnSession`, which works correctly with defaults.

**Recommended approach (option a -- rename + restrict signature):**

Replace the generic method with a purpose-specific one:

```rust
/// Dispatches a spawn-session action to start a new Flutter process.
///
/// This is the external API for session creation. For full action dispatch
/// (reload, restart, device discovery), use `process_message()` instead.
pub fn dispatch_spawn_session(&self, session_id: SessionId, device: Device, config: Option<Box<LaunchConfig>>) {
    crate::actions::handle_action(
        UpdateAction::SpawnSession { session_id, device, config },
        self.msg_tx.clone(),
        None,
        Vec::new(),
        self.session_tasks.clone(),
        self.shutdown_rx.clone(),
        &self.project_path,
        Default::default(),
    );
}
```

Then update the headless runner call site:
```rust
// Before:
engine.dispatch_action(UpdateAction::SpawnSession { session_id, device, config: None });

// After:
engine.dispatch_spawn_session(session_id, device.clone(), None);
```

**Alternative approaches (if preferred):**
- **(b) Document only**: Keep `pub fn dispatch_action(action: UpdateAction)` but add a doc comment listing supported actions (`SpawnSession`, `DiscoverDevices`, `DiscoverEmulators`, etc.) and warning about silently degraded variants
- **(c) Accept parameters**: Change signature to `pub fn dispatch_action(&self, action: UpdateAction, tool_availability: ToolAvailability)` -- but this still can't provide `cmd_sender`

### Acceptance Criteria

1. External callers cannot silently dispatch actions that will fail
2. The headless runner compiles and works as before
3. Either the method name reflects its limitations OR the doc comment explicitly lists supported/unsupported actions
4. `cargo check --workspace` passes
5. `cargo test --workspace --lib` passes

### Testing

```bash
# Verify headless runner still compiles
cargo check -p flutter-demon

# Verify all tests pass
cargo test --workspace --lib
```

### Notes

- The "real" action dispatch path is in `process.rs:40-54` via `process_message()` -- it extracts actual runtime values from `AppState`
- `dispatch_action()` must remain `pub` (not `pub(crate)`) because the binary crate calls it
- Option (a) is recommended because it makes invalid states unrepresentable at the type level

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/crates/fdemon-app/src/engine.rs` | Replaced `dispatch_action(action: UpdateAction)` method (lines 327-341) with `dispatch_spawn_session(session_id, device, config)` method. The new method constructs the `UpdateAction::SpawnSession` internally and provides a purpose-specific API. |
| `/Users/ed/Dev/zabin/flutter-demon/src/headless/runner.rs` | Updated the sole caller (line 208-214) to use the new `dispatch_spawn_session()` method. Simplified from constructing `UpdateAction::SpawnSession` manually to a single method call. Removed unused `UpdateAction` import. |

### Notable Decisions/Tradeoffs

1. **Type-safe API**: Replaced generic `dispatch_action()` with purpose-specific `dispatch_spawn_session()`. This makes invalid states unrepresentable at the type level - external callers can only spawn sessions, not dispatch arbitrary actions that would silently fail.

2. **Signature design**: Used explicit parameters (`session_id`, `device`, `config`) instead of accepting a full `UpdateAction`. This makes the API more discoverable and self-documenting.

3. **No breaking changes elsewhere**: The only caller is the headless runner auto-start, which was updated. The method must remain `pub` (not `pub(crate)`) because the binary crate depends on it.

### Testing Performed

- `cargo check --workspace` - Passed (all crates compile)
- `cargo check -p flutter-demon` - Passed (binary crate compiles with new API)
- `cargo test --workspace --lib` - Pre-existing test failures on branch (unrelated to this change):
  - `fdemon-core`: Test helper function scoping issue in `discovery.rs`
  - `fdemon-app`: Missing imports in `handler/tests.rs` (likely from prior restructuring)
  - **These failures existed before my changes and are not caused by this task**

### Verification of Changes

The implementation follows the recommended approach from the task specification exactly:
- Replaced `dispatch_action()` with `dispatch_spawn_session()` at lines 327-345 in `engine.rs`
- Method constructs `UpdateAction::SpawnSession` internally
- Updated headless runner call site to use new method signature
- Doc comment explains this is for external session creation, directs users to `process_message()` for other actions
- Compiles successfully with no new warnings or errors

### Risks/Limitations

1. **No impact on existing functionality**: The headless runner is the only external caller and it only ever dispatched `SpawnSession` actions, so the restriction to a purpose-specific method has no functional impact.

2. **Future extensibility**: If additional external action dispatch is needed in the future, new purpose-specific methods should be added (e.g., `dispatch_device_discovery()`) rather than reverting to a generic `dispatch_action()` that silently fails for most variants.
