## Task: Wire Request Timeouts and Missing Custom Events

**Objective**: Apply `REQUEST_TIMEOUT` (10s) to all backend calls to prevent hung VM Service from blocking the DAP session indefinitely. Also emit missing custom DAP events: `dart.serviceExtensionAdded`, `dart.hotReloadComplete`, `dart.hotRestartComplete` (if not already done by Task 14), and the `restart` session-level request handler.

**Depends on**: 01-fix-variable-display-bugs through 16-completions-request (all prior tasks)

**Estimated Time**: 4–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Wrap backend calls with `tokio::time::timeout`; add `restart` handler
- `crates/fdemon-dap/src/adapter/variables.rs`: Wrap backend calls with timeout in `get_scope_variables` and `expand_object`
- `crates/fdemon-dap/src/adapter/events.rs`: Emit `dart.serviceExtensionAdded` on `ServiceExtensionAdded` isolate event

### Details

#### 1. Request timeouts

Remove `#[allow(dead_code)]` from `REQUEST_TIMEOUT` in `adapter/types.rs`.

Wrap all `backend.*()` calls with `tokio::time::timeout`:

```rust
use crate::adapter::types::REQUEST_TIMEOUT;

// Before:
let result = self.backend.get_stack(&isolate_id, limit).await?;

// After:
let result = tokio::time::timeout(REQUEST_TIMEOUT, self.backend.get_stack(&isolate_id, limit))
    .await
    .map_err(|_| format!("Request timed out after {}s", REQUEST_TIMEOUT.as_secs()))?
    .map_err(|e| e.to_string())?;
```

Apply to all backend calls in:
- `handlers.rs`: all handler functions
- `variables.rs`: `get_scope_variables`, `expand_object`
- `evaluate.rs`: already uses shorter timeouts for hover/toString (1s) — keep those

Consider a helper function:
```rust
async fn with_timeout<T, E: std::fmt::Display>(
    future: impl Future<Output = Result<T, E>>,
) -> Result<T, String> {
    tokio::time::timeout(REQUEST_TIMEOUT, future)
        .await
        .map_err(|_| format!("Request timed out after {}s", REQUEST_TIMEOUT.as_secs()))?
        .map_err(|e| e.to_string())
}
```

#### 2. `dart.serviceExtensionAdded` event

In `events.rs`, handle `IsolateEvent::ServiceExtensionAdded`:

```rust
DebugEvent::ServiceExtensionAdded { isolate_id, extension_rpc, method } => {
    self.send_event("dart.serviceExtensionAdded", json!({
        "extensionRPC": extension_rpc,
        "isolateId": isolate_id,
    }));
}
```

Check if `DebugEvent` already has a `ServiceExtensionAdded` variant — it may be an `IsolateEvent` variant that needs routing. If it arrives via the Isolate stream (not Debug stream), ensure the event routing in `handle_debug_event` handles it.

#### 3. `restart` session-level handler

Add `restart` to the dispatch table:

```rust
"restart" => self.handle_restart(request).await,
```

The `restart` request performs a hot restart (re-creates the main isolate):

```rust
async fn handle_restart(&mut self, request: &DapRequest) -> DapResponse {
    self.backend.hot_restart().await
        .map_err(|e| format!("Restart failed: {}", e))?;
    DapResponse::success(request, json!({}))
}
```

Re-add `supports_restart_request: Some(true)` to `fdemon_defaults()` (was removed in Task 01).

#### 4. Variable store memory cap

Add a safety cap to `VariableStore`:

```rust
const MAX_VARIABLE_REFS: usize = 10_000;

pub fn allocate(&mut self, var_ref: VariableRef) -> i64 {
    if self.references.len() >= MAX_VARIABLE_REFS {
        tracing::warn!("Variable reference store full ({} entries), returning 0", MAX_VARIABLE_REFS);
        return 0;  // Non-expandable
    }
    // ... existing allocation
}
```

### Acceptance Criteria

1. All backend calls wrapped with 10s timeout
2. Timeout errors return clear DAP error responses
3. `#[allow(dead_code)]` removed from `REQUEST_TIMEOUT`
4. `dart.serviceExtensionAdded` emitted on `ServiceExtensionAdded` events
5. `restart` request handler works (hot restart)
6. `supportsRestartRequest: true` re-added to capabilities
7. Variable store capped at 10,000 entries
8. 10+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_backend_timeout_returns_error() {
    // MockBackend: get_stack hangs forever
    // Verify timeout error after REQUEST_TIMEOUT
}

#[tokio::test]
async fn test_variable_store_cap() {
    // Allocate MAX_VARIABLE_REFS + 1 entries
    // Verify last allocation returns 0
}

#[tokio::test]
async fn test_restart_calls_hot_restart() {
    // Call handle_restart
    // Verify backend.hot_restart called
}
```

### Notes

- The `with_timeout` helper should be used consistently across all files to avoid duplicating timeout logic.
- Short timeouts (1s) for `toString()` and getter evaluation should be kept separate from the general 10s timeout.
- The variable store cap of 10,000 is generous — most debug sessions will never hit it. But widget trees with thousands of children could theoretically exhaust it in a single pause.
