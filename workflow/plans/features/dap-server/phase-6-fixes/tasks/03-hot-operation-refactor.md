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
