## Task: Remove Unused `_msg_tx` Parameter from `spawn_clear_http_profile`

**Objective**: Remove the unused `_msg_tx: mpsc::Sender<Message>` parameter from `spawn_clear_http_profile` and its call site, eliminating dead code.

**Depends on**: None

**Review Issue**: #2 (Major)

### Scope

- `crates/fdemon-app/src/actions/network.rs`: Lines 278-282 — remove `_msg_tx` from function signature
- `crates/fdemon-app/src/actions/mod.rs`: Line 317 — remove `msg_tx` from call site

### Details

**Function signature at `network.rs:278-282`:**

```rust
pub(super) fn spawn_clear_http_profile(
    session_id: SessionId,
    handle: VmRequestHandle,
    _msg_tx: mpsc::Sender<Message>,   // ← never used inside function body
) {
```

The underscore prefix silences the compiler warning, but the parameter is genuinely unused — the function's doc comment explicitly states "Fire-and-forget: errors are logged at warn level but do not propagate." No message is ever sent back.

**Call site at `mod.rs:312-324`:**

```rust
UpdateAction::ClearHttpProfile {
    session_id,
    vm_handle,
} => {
    if let Some(handle) = vm_handle {
        network::spawn_clear_http_profile(session_id, handle, msg_tx);  // ← remove msg_tx
    } else {
        ...
    }
}
```

**After fix:**

`network.rs`:
```rust
pub(super) fn spawn_clear_http_profile(
    session_id: SessionId,
    handle: VmRequestHandle,
) {
```

`mod.rs`:
```rust
network::spawn_clear_http_profile(session_id, handle);
```

### Acceptance Criteria

1. `spawn_clear_http_profile` signature has exactly 2 parameters: `session_id` and `handle`
2. Call site in `mod.rs` passes exactly 2 arguments
3. No `_`-prefixed parameters remain in any function in `actions/` (verify with grep)
4. `cargo clippy --workspace -- -D warnings` clean
5. `cargo test -p fdemon-app` passes

### Testing

No new tests needed — this is a signature cleanup. Removing an unused parameter cannot change behavior.

### Notes

- No other functions in the `actions/` module have unused `_`-prefixed parameters.
- The `_msg_tx` was likely left over from an earlier implementation that planned to send a completion message back. The current design uses fire-and-forget logging instead.
- After removing the parameter, the `ClearHttpProfile` arm in `mod.rs` no longer consumes `msg_tx`, which may trigger a Rust "unused variable" warning in the match arm. Verify that `msg_tx` is still consumed by other arms in the same match — it is (it is passed to `vm_service::spawn_vm_service_connection` and other spawn functions in earlier arms).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/network.rs` | Removed `_msg_tx: mpsc::Sender<Message>` parameter from `spawn_clear_http_profile` signature |
| `crates/fdemon-app/src/actions/mod.rs` | Removed `msg_tx` argument from `network::spawn_clear_http_profile(session_id, handle)` call site |

### Notable Decisions/Tradeoffs

1. **No import changes needed**: The `mpsc` and `Message` imports in `network.rs` remain valid because they are still used by `spawn_network_monitoring` and `spawn_fetch_http_request_detail`. No cleanup of the `use` block was required.
2. **No unused variable warning**: `msg_tx` continues to be consumed by other arms in the `handle_action` match (e.g., `spawn_vm_service_connection`, `spawn_network_monitoring`), so removing the argument from the `ClearHttpProfile` arm did not introduce any unused-variable warnings.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo test -p fdemon-app` - Passed (1164 unit tests + 1 doc test, 0 failures)
- Grep for `_`-prefixed parameters in `actions/` - No underscore-prefixed `Sender` parameters remain

### Risks/Limitations

1. **None**: This is a pure dead-code removal. No logic was changed — the function still operates fire-and-forget, logging errors at `warn` level.
