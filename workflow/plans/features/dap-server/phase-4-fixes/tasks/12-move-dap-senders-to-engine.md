## Task: Move dap_debug_senders from AppState to Engine

**Objective**: Remove `dap_debug_senders` from `AppState` (TEA model) since it holds live channel infrastructure, not domain state. After task 03, forwarding happens in `handle_action()` which has access to Engine's copy.

**Depends on**: 03-tea-purity-event-forwarding

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/state.rs`: Remove `dap_debug_senders` field from `AppState`
- `crates/fdemon-app/src/engine.rs`: Engine already holds its own `Arc<Mutex<Vec<Sender>>>` — it becomes the sole owner
- `crates/fdemon-app/src/handler/devtools/debug.rs`: Remove any remaining references to `state.dap_debug_senders`

### Details

Currently both `AppState` and `Engine` hold a clone of the same `Arc<Mutex<Vec<Sender<DapDebugEvent>>>>`:

```rust
// engine.rs:224-226
let dap_debug_senders = Arc::new(Mutex::new(Vec::new()));
state.dap_debug_senders = dap_debug_senders.clone();
```

After task 03, `forward_dap_event()` is no longer called from handlers (which access `state`). Instead, forwarding happens in `handle_action()` which has direct access to `self.dap_debug_senders` on Engine.

**Steps:**
1. Remove `pub dap_debug_senders` from `AppState`
2. Remove `state.dap_debug_senders = ...` from Engine initialization
3. Update any test helpers that initialize `AppState` with this field
4. Verify Engine's `dap_debug_senders` field is sufficient for all use sites

### Acceptance Criteria

1. `AppState` no longer contains `dap_debug_senders`
2. Engine is the sole holder of the senders Arc
3. `cargo check --workspace` — Pass
4. `cargo test --workspace` — Pass

### Notes

- This is a cleanup task that follows naturally from task 03
- TEA model purity: `AppState` should contain only serializable domain state, not live channels

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Removed `pub dap_debug_senders` field (Arc/Mutex/Sender imports removed; field removed from `with_settings()` constructor) |
| `crates/fdemon-app/src/engine.rs` | Removed `state.dap_debug_senders = dap_debug_senders.clone()` assignment; updated comment — Engine is now sole owner |
| `crates/fdemon-app/src/handler/update.rs` | Updated doc comment to reference `UpdateAction::ForwardDapDebugEvents` instead of `state.dap_debug_senders` |
| `crates/fdemon-app/src/handler/mod.rs` | Added `ForwardDapDebugEvents(Vec<fdemon_dap::adapter::DebugEvent>)` variant to `UpdateAction`; added `message_and_action()` constructor to `UpdateResult` |
| `crates/fdemon-app/src/actions/mod.rs` | Added `ForwardDapDebugEvents` match arm in `handle_action()` with `try_send` + `retain` pruning logic |
| `crates/fdemon-app/src/handler/devtools/debug.rs` | Replaced `forward_dap_event(&state.dap_debug_senders, ...)` calls with `UpdateAction::ForwardDapDebugEvents` returns; added `UpdateAction` import; annotated `forward_dap_event` with `#[cfg(test)]` (now test-only); rewrote all tests using `extract_dap_event()` helper; updated stale doc comments |

### Notable Decisions/Tradeoffs

1. **Task 03 was not complete**: The IMPORTANT CONTEXT note claimed Task 03 was done, but `UpdateAction::ForwardDapDebugEvents` did not exist and `forward_dap_event(&state.dap_debug_senders, ...)` was still called directly from handlers. Implemented both tasks together since they are tightly coupled prerequisites.

2. **`forward_dap_event` made `#[cfg(test)]`**: After the TEA purity refactor, `forward_dap_event` is only called from test code (stale-sender pruning tests). Marked `#[cfg(test)]` to eliminate the `dead_code` compiler warning rather than deleting it, since it serves as a clean unit-testable shim for the forwarding logic.

3. **`message_and_action()` constructor**: Pause/Resume events must return both a follow-up `Message` (SuspendFileWatcher/ResumeFileWatcher) and `UpdateAction::ForwardDapDebugEvents`. Added `UpdateResult::message_and_action()` to make this composable without ad-hoc tuple returns.

4. **Test rewrite scope**: ~20+ tests that used `make_state_with_dap_sender()` / `rx.try_recv()` were rewritten to use `make_state_with_session()` + `extract_dap_event(result)` helper. The helper pattern is cleaner and decouples tests from channel wiring.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed (clean, no warnings)
- `cargo test --workspace` — Passed (1317 + 360 + 460 + 581 + 796 + others; 0 failures)
- `cargo clippy --workspace -- -D warnings` — Passed

### Risks/Limitations

1. **Pre-existing test failure (unrelated)**: The `adapter::tests::test_scopes_locals_scope_has_correct_var_ref_kind` test failure mentioned in session context was from uncommitted changes in `fdemon-dap/src/adapter/mod.rs` (task 13). That file was not modified by this task and the failure does not appear in the current test run.
