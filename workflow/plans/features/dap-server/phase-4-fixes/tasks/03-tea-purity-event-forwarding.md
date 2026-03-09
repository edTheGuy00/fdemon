## Task: Move debug event forwarding out of TEA update() + fix channel-full handling

**Objective**: Eliminate the TEA purity violation where `forward_dap_event()` acquires a blocking mutex and performs channel sends inside the synchronous `update()` path. Also elevate `TrySendError::Full` from `debug!` to `warn!`.

**Depends on**: 01-fix-isolate-runnable-translation

**Severity**: Major

### Scope

- `crates/fdemon-app/src/handler/devtools/debug.rs`: Remove `forward_dap_event()` call from handlers; return events via `UpdateAction`
- `crates/fdemon-app/src/actions/mod.rs`: Add `ForwardDapDebugEvents` variant to `UpdateAction`
- `crates/fdemon-app/src/engine.rs`: Handle `ForwardDapDebugEvents` in `handle_action()`

### Details

**Current (violates TEA):**

`forward_dap_event()` at `debug.rs:323-357` acquires `std::sync::Mutex` and calls `try_send` inside the synchronous `update()` path:

```rust
pub(crate) fn forward_dap_event(
    dap_debug_senders: &Arc<Mutex<Vec<Sender<DapDebugEvent>>>>,
    dap_event: Option<DapDebugEvent>,
) {
    let Some(ev) = dap_event else { return; };
    match dap_debug_senders.lock() {
        Ok(mut senders) => {
            senders.retain(|tx| match tx.try_send(ev.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => {
                    tracing::debug!("DAP debug event channel full — ...");
                    true
                }
                Err(TrySendError::Closed(_)) => false,
            });
        }
        Err(e) => { tracing::warn!("lock poisoned: {}", e); }
    }
}
```

**Proposed fix:**

1. Add `UpdateAction::ForwardDapDebugEvents(Vec<DapDebugEvent>)` variant
2. In `handle_debug_event` / `handle_isolate_event`, collect `DapDebugEvent`s and return them via the `UpdateAction` instead of calling `forward_dap_event()` inline
3. Move the channel-send logic to `handle_action()` in `engine.rs`
4. Elevate `TrySendError::Full` log from `debug!` to `warn!` with message: `"DAP debug event channel full — event dropped, IDE may desync"`
5. Consider pruning the sender on `Full` (same as `Closed`) since a 64-item backlog means the session is likely broken

### Acceptance Criteria

1. `forward_dap_event()` is no longer called from inside any handler in `debug.rs`
2. `update()` returns `UpdateAction::ForwardDapDebugEvents(events)` when DAP events need forwarding
3. Channel sends happen in `handle_action()` (outside TEA update cycle)
4. `TrySendError::Full` logged at `warn!` level
5. All existing tests pass
6. `cargo test -p fdemon-app` — Pass

### Testing

- Existing handler tests verify correct `UpdateAction` is returned
- Add test: `handle_debug_event` returns `ForwardDapDebugEvents` with correct events
- Add test: `handle_isolate_event` returns `ForwardDapDebugEvents` for IsolateStart/Runnable/Exit

### Notes

- This task depends on 01 because the IsolateRunnable fix must be correct before refactoring the forwarding path
- The `dap_debug_senders` field on `AppState` becomes unnecessary after this change (see task 12)
- `handle_action()` already has access to the `Engine`'s `dap_debug_senders` Arc
