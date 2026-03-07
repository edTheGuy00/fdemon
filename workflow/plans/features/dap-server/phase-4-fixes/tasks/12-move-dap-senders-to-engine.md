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
