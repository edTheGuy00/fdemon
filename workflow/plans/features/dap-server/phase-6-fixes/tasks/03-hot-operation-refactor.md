## Task: Refactor Hot Operation Handlers and Align `restart`

**Objective**: Extract the duplicated hot-reload/hot-restart logic into a shared helper, and make the standard DAP `restart` request delegate to it — fixing both H3 (restart inconsistency) and L8 (handler duplication).

**Depends on**: 01-handlers-critical-fixes (shared file: handlers.rs)

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Extract shared helper, refactor three handlers

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/events.rs`: Understand `send_event` signatures

### Details

#### Current State

Three handlers with near-identical structure:

| Handler | Lines | Backend Call | Progress Title | Completion Event |
|---------|-------|-------------|----------------|------------------|
| `handle_hot_reload` | 940–982 (43 lines) | `hot_reload()` | `"Hot Reload"` | `"dart.hotReloadComplete"` |
| `handle_hot_restart` | 1001–1043 (43 lines) | `hot_restart()` | `"Hot Restart"` | `"dart.hotRestartComplete"` |
| `handle_restart` | 1686–1699 (14 lines) | `hot_restart()` | *(none)* | *(none)* |

`handle_hot_reload` and `handle_hot_restart` are byte-for-byte identical except for 3 strings and 1 method name. `handle_restart` calls `hot_restart()` but misses progress events, the completion event, and returns `Some(json!({}))` instead of `None`.

#### The Fix

**Step 1:** Extract a private async helper that encapsulates the shared pattern:

```rust
async fn execute_hot_operation<F, Fut>(
    &mut self,
    request: &DapRequest,
    title: &str,
    complete_event: &str,
    operation: F,
) -> DapResponse
where
    F: FnOnce(&dyn DynDebugBackend) -> Fut,
    Fut: std::future::Future<Output = Result<(), BackendError>>,
{
    // 1. Allocate progress ID if client_supports_progress
    // 2. Send progressStart
    // 3. Call with_timeout(operation(self.backend))
    // 4. Send progressEnd (always, even on error)
    // 5. On success: send complete_event, return success(None)
    // 6. On error: return error response
}
```

**Step 2:** Rewrite all three handlers as one-liners:

```rust
pub(super) async fn handle_hot_reload(&mut self, request: &DapRequest) -> DapResponse {
    self.execute_hot_operation(request, "Hot Reload", "dart.hotReloadComplete", |b| b.hot_reload()).await
}

pub(super) async fn handle_hot_restart(&mut self, request: &DapRequest) -> DapResponse {
    self.execute_hot_operation(request, "Hot Restart", "dart.hotRestartComplete", |b| b.hot_restart()).await
}

pub(super) async fn handle_restart(&mut self, request: &DapRequest) -> DapResponse {
    self.execute_hot_operation(request, "Hot Restart", "dart.hotRestartComplete", |b| b.hot_restart()).await
}
```

This ensures `handle_restart` now:
- Emits `progressStart`/`progressEnd` events
- Emits `dart.hotRestartComplete` on success
- Returns `None` body (consistent with `handle_hot_restart`)

**Note on `on_hot_restart()`**: Research confirmed that neither `handle_hot_restart` nor `handle_restart` currently calls `on_hot_restart()`. State invalidation is triggered by the VM's `IsolateExit`/`IsolateRunnable` event sequence, not by the outgoing RPC. This task does not change that behavior — it just ensures `handle_restart` is now consistent with `handle_hot_restart`.

### Acceptance Criteria

1. `handle_restart`, `handle_hot_restart`, and `handle_hot_reload` all delegate to a shared helper
2. `handle_restart` now emits progress events and `dart.hotRestartComplete` — identical to `handle_hot_restart`
3. No duplicated handler logic (the 43-line clone is eliminated)
4. Net reduction of ~35+ lines in handlers.rs
5. Existing tests pass: `cargo test -p fdemon-dap`
6. `cargo clippy -p fdemon-dap` clean

### Testing

```rust
#[tokio::test]
async fn test_restart_emits_progress_events() {
    // Send standard DAP restart request
    // Assert progressStart and progressEnd events emitted
}

#[tokio::test]
async fn test_restart_emits_hot_restart_complete_event() {
    // Send standard DAP restart request (successful)
    // Assert dart.hotRestartComplete event emitted
}

#[tokio::test]
async fn test_restart_error_still_emits_progress_end() {
    // Mock backend.hot_restart() returns error
    // Send restart request
    // Assert progressEnd still emitted
}
```

### Notes

- The closure-based approach may need a `Pin<Box<dyn Future>>` wrapper depending on the trait object constraints of `DynDebugBackend`. If the generic approach is too complex, a simpler enum-based approach (e.g., `HotOp::Reload | HotOp::Restart`) with a match inside the helper also works.
- The response body change (`Some(json!({}))` → `None`) for `handle_restart` is a minor behavioral change. DAP spec allows either for success responses.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/handlers.rs` | Added `HotOp` enum; extracted `execute_hot_operation` async helper; refactored `handle_hot_reload`, `handle_hot_restart`, `handle_restart` to delegate to helper |
| `crates/fdemon-dap/src/adapter/tests/progress_reporting.rs` | Added 5 new tests: `test_restart_emits_progress_events`, `test_restart_emits_hot_restart_complete_event`, `test_restart_error_still_emits_progress_end`, `test_restart_no_completion_event_on_failure`, `test_restart_progress_start_has_hot_restart_title` |
| `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs` | Updated `test_restart_response_body_is_empty_object` → `test_restart_response_body_is_none` to match new `None` body behavior |

### Notable Decisions/Tradeoffs

1. **Enum-based dispatch over closures**: Used `HotOp` enum rather than a generic closure/`Pin<Box<dyn Future>>` approach. The closure approach would require splitting the borrow of `&self.backend` from `&mut self` for event emission, which Rust doesn't allow in a single method call. The enum approach is cleaner and has zero overhead.

2. **Error message capitalization preserved**: The helper uses a separate `err_prefix` field (`"Hot reload"`, `"Hot restart"`) for error messages to preserve the existing lowercase style expected by tests. The `title` field (`"Hot Reload"`, `"Hot Restart"`) keeps proper title case for the progress event body.

3. **Response body change accepted**: `handle_restart` now returns `None` body on success (was `Some(json!({}))`). DAP spec allows either; this makes `restart` consistent with `hotRestart`. The test was updated to reflect the new behavior.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-dap` - Passed
- `cargo test -p fdemon-dap` - Passed (833 tests, 0 failed)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed (clean)

### Risks/Limitations

1. **Error message format change**: The error prefix changed from `"Hot reload failed"` to still `"Hot reload failed"` (preserved), but note that `handle_restart` error messages changed from `"Restart failed: {e}"` to `"Hot restart failed: {e}"`. This is more descriptive and consistent.
