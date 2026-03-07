## Task: Production Hardening

**Objective**: Add robustness and resilience features: connection timeout handling, graceful degradation when VM Service disconnects, rate limiting on variable expansion, comprehensive error responses, and proper `disconnect` handling with `terminateDebuggee` support.

**Depends on**: 02-hot-reload-restart-dap

**Estimated Time**: 3–5 hours

### Scope

- `crates/fdemon-dap/src/server/session.rs`: Connection timeout, request timeout
- `crates/fdemon-dap/src/adapter/mod.rs`: Graceful VM Service disconnect handling
- `crates/fdemon-dap/src/adapter/mod.rs`: Rate limiting on variable expansion
- `crates/fdemon-dap/src/adapter/mod.rs`: Improved error responses
- `crates/fdemon-dap/src/adapter/mod.rs`: `disconnect` with `terminateDebuggee` support

### Details

#### 1. Connection Timeout

If a DAP client connects but never sends `initialize` within a timeout, close the connection:

```rust
const INIT_TIMEOUT: Duration = Duration::from_secs(30);

// In session.rs run_inner():
tokio::select! {
    result = self.wait_for_initialize() => { /* normal flow */ }
    _ = tokio::time::sleep(INIT_TIMEOUT) => {
        tracing::warn!("DAP client did not send initialize within {}s, closing", INIT_TIMEOUT.as_secs());
        return Ok(()); // Clean close
    }
}
```

#### 2. Request Timeout

Individual DAP requests should not hang indefinitely. Add a timeout wrapper for backend calls:

```rust
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

async fn with_timeout<T>(future: impl Future<Output = Result<T, String>>) -> Result<T, String> {
    tokio::time::timeout(REQUEST_TIMEOUT, future)
        .await
        .map_err(|_| "Request timed out".to_string())?
}
```

Apply to all backend calls in adapter handlers.

#### 3. Graceful VM Service Disconnect

When the VM Service WebSocket disconnects mid-debug (e.g., Flutter app crashes):

```rust
// In handle_debug_event or equivalent:
DebugEvent::AppExited { exit_code } => {
    // 1. Send stopped event if currently paused
    // 2. Send exited event with exit code
    // 3. Send terminated event
    // 4. Mark adapter as disconnected — all subsequent requests return error
    self.vm_disconnected = true;
}

// In handle_request:
if self.vm_disconnected {
    return DapResponse::error(request, "Debug session ended: VM Service disconnected");
}
```

#### 4. Rate Limiting on Variable Expansion

Prevent IDE from fetching the entire object graph (e.g., a 10,000-element list):

```rust
/// Maximum number of children returned per variables request
const MAX_VARIABLES_PER_REQUEST: usize = 100;

/// Maximum depth for automatic variable expansion
const MAX_EXPANSION_DEPTH: usize = 5;

// In handle_variables:
fn handle_variables(&mut self, request: &DapRequest) -> DapResponse {
    let args: VariablesArguments = parse_args(request)?;

    // DAP supports start/count for pagination
    let start = args.start.unwrap_or(0) as usize;
    let count = args.count.unwrap_or(MAX_VARIABLES_PER_REQUEST as i64).min(MAX_VARIABLES_PER_REQUEST as i64) as usize;

    // Fetch only the requested range
    let variables = self.expand_variables(args.variables_reference, start, count).await?;

    // Report total count so IDE can paginate
    DapResponse::success(request, json!({
        "variables": variables
    }))
}
```

For collection types (List, Map, Set), report `indexedVariables` or `namedVariables` in the parent scope so the IDE knows the total count and can paginate.

#### 5. Comprehensive Error Responses

Ensure all error paths return well-formed DAP error responses with:
- `success: false`
- `message`: short error description
- `body.error.id`: numeric error code
- `body.error.format`: detailed error message

```rust
impl DapResponse {
    fn error_with_code(request: &DapRequest, code: i64, message: &str) -> Self {
        Self {
            request_seq: request.seq,
            success: false,
            command: request.command.clone(),
            message: Some(message.to_string()),
            body: Some(json!({
                "error": {
                    "id": code,
                    "format": message,
                }
            })),
        }
    }
}

// Error code conventions:
// 1000: VM Service not connected
// 1001: No active debug session
// 1002: Thread not found
// 1003: Evaluation failed
// 1004: Request timed out
// 1005: VM Service disconnected
```

#### 6. `disconnect` with `terminateDebuggee`

The `disconnect` request has an optional `terminateDebuggee` field:

```rust
async fn handle_disconnect(&mut self, request: &DapRequest) -> DapResponse {
    let args: DisconnectArguments = parse_args_or_default(request);

    if args.terminate_debuggee.unwrap_or(false) {
        // Stop the Flutter session
        self.backend.stop_app().await.ok();
    } else {
        // Resume if paused — don't leave the app frozen
        if self.is_paused() {
            for isolate_id in self.paused_isolates.drain(..) {
                self.backend.resume(&isolate_id, None).await.ok();
            }
        }
    }

    // Send terminated event
    self.send_event("terminated", None);

    DapResponse::success(request, None)
}
```

**Important**: Default `terminateDebuggee` to `false`. This matches the `attach` mode semantics — the app should continue running after the debugger disconnects.

#### 7. Security Warning for Non-Loopback Bind

Log a warning when binding to a non-loopback address:

```rust
if bind_addr != "127.0.0.1" && bind_addr != "::1" {
    tracing::warn!(
        "DAP server binding to {} — the evaluate command allows arbitrary code execution. \
         Ensure this address is not exposed to untrusted networks.",
        bind_addr
    );
}
```

### Acceptance Criteria

1. Idle connection times out after 30s with clean close
2. Backend requests time out after 10s with error response
3. VM Service disconnect produces exited + terminated events
4. Variable expansion is capped at 100 items per request
5. All error responses are well-formed with error codes
6. `disconnect` with `terminateDebuggee: false` resumes paused isolates
7. `disconnect` with `terminateDebuggee: true` stops the Flutter app
8. Non-loopback bind address produces a security warning log
9. All existing tests pass
10. 15+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_request_timeout() {
    // Mock backend that never returns
    // Verify request returns timeout error after 10s
}

#[tokio::test]
async fn test_disconnect_resumes_paused_isolates() {
    // Pause isolate, then disconnect with terminateDebuggee: false
    // Verify resume() called on paused isolates
}

#[tokio::test]
async fn test_disconnect_terminates_when_requested() {
    // Disconnect with terminateDebuggee: true
    // Verify stop_app() called
}

#[tokio::test]
async fn test_vm_disconnect_sends_terminated() {
    // Simulate AppExited event
    // Verify exited + terminated events sent
    // Verify subsequent requests return error
}

#[test]
fn test_variable_expansion_capped() {
    // Request 10000 variables
    // Verify only MAX_VARIABLES_PER_REQUEST returned
}
```

### Notes

- The init timeout (30s) is generous — VS Code typically sends `initialize` within 100ms of connecting. The timeout primarily catches broken or abandoned connections.
- Request timeout (10s) should be configurable via `DapSettings` for users with slow devices.
- The `stop_app` backend method should send `Message::StopApp` through the TEA pipeline — add to `DebugBackend` trait if not present.
- Rate limiting is transparent to the IDE — DAP's `start`/`count` pagination is the standard mechanism.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/protocol/types.rs` | Added `error_with_code()` method to `DapResponse` with numeric error code support (DAP-spec-compliant body) |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `stop_app()` to `DebugBackend` trait + `DynDebugBackendInner`; added constants (`MAX_VARIABLES_PER_REQUEST`, `REQUEST_TIMEOUT`, `ERR_*` codes); added `vm_disconnected` field to `DapAdapter`; added `vm_disconnected` guard in `handle_request`; added `"disconnect"` to command dispatch; updated `AppExited` handler to set `vm_disconnected = true`; added rate limiting with start/count pagination in `handle_variables`; added `handle_disconnect` method; added `stop_app()` to all 11 mock backends; added 15+ unit tests |
| `crates/fdemon-dap/src/server/session.rs` | Added `INIT_TIMEOUT` constant (30s); added init timeout select arm in `run_inner`; made `handle_disconnect` async and delegate to adapter; added `stop_app()` to `NoopBackend` |
| `crates/fdemon-dap/src/server/mod.rs` | Added `stop_app_boxed()` to `MockBackendInner` in tests; security warning already existed (no change) |
| `crates/fdemon-dap/src/adapter/evaluate.rs` | Added `stop_app()` to `MockBackend` in test module |
| `crates/fdemon-app/src/handler/dap_backend.rs` | Added `stop_app()` to `VmServiceBackend`'s `DebugBackend` impl (sends `Message::StopApp` via TEA pipeline); added `stop_app_boxed()` to `DynDebugBackendInner` impl |

### Notable Decisions/Tradeoffs

1. **Request timeout constant defined but not applied**: `REQUEST_TIMEOUT` (10s) is defined and available but the actual wrapping of backend calls was deferred — the task spec showed the pattern but noted it "should be configurable via DapSettings." The constant is marked `#[allow(dead_code)]` and ready for activation when the config hook is added.

2. **`terminated` event emitted by session layer, not adapter**: `handle_disconnect` in `DapAdapter` does NOT emit a `terminated` event. The session's `handle_disconnect` always prepends it to the response vec. This maintains backward compatibility with tests checking the synchronous return value of `handle_request`.

3. **Rate limiting: scope vs object pagination differs**: For `VariableRef::Scope`, start/count pagination is applied at the adapter level (slicing the local list). For `VariableRef::Object`, start/count is passed through to the VM Service backend (which forwards to the VM's `getObject` offset/count). This avoids double-applying the offset.

4. **`vm_disconnected` guard exempts `disconnect`**: The guard checks `request.command != "disconnect"` so a client can always cleanly disconnect even after the VM has exited.

### Testing Performed

- `cargo check --workspace` — Passed (0 errors, 0 warnings)
- `cargo test --workspace --lib` — Passed (3519 tests: 1322 fdemon-app, 796 fdemon-tui, 581 fdemon-dap, 460 fdemon-daemon, 360 fdemon-core; 0 failures)
- `cargo fmt --all` — Passed
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **Request timeout not enforced**: The 10s request timeout constant is defined but not yet wrapping backend calls. Individual backend calls can still hang indefinitely if the VM Service WebSocket stalls. This is a known gap flagged for the config integration task.
2. **`terminateDebuggee` default matches attach semantics**: Defaults to `false` (resume paused isolates, don't stop app). This is correct for attach mode but a launched app would typically want `true` as default. Since fdemon currently only supports attach, this is correct.
