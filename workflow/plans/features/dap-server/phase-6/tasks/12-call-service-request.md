## Task: Implement callService Custom Request

**Objective**: Add the `callService` custom DAP request handler that forwards arbitrary VM Service RPCs. This enables DevTools integration — the VS Code Dart extension uses `callService` to invoke service extensions like `ext.flutter.debugDumpApp`, performance overlays, and widget inspector commands.

**Depends on**: 02-expand-backend-trait

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Add `callService` to the dispatch table with handler

### Details

#### Handler implementation:

```rust
"callService" => self.handle_call_service(request).await,
```

```rust
async fn handle_call_service(&mut self, request: &DapRequest) -> DapResponse {
    let args = request.arguments.as_ref()
        .ok_or("Missing arguments")?;

    let method = args.get("method")
        .and_then(|m| m.as_str())
        .ok_or("Missing 'method' argument")?;

    let params = args.get("params").cloned();

    tracing::debug!("callService: method={}, params={:?}", method, params);

    let result = self.backend.call_service(method, params).await
        .map_err(|e| format!("callService failed: {}", e))?;

    DapResponse::success(request, json!({ "result": result }))
}
```

#### Security considerations:

- Only accept from localhost connections (already enforced by default bind address `127.0.0.1`)
- Log all `callService` invocations at `debug` level for auditability
- Do NOT block any methods — the VM Service itself handles authorization

#### Common callService methods used by IDEs:

| Method | Purpose |
|---|---|
| `ext.flutter.debugDumpApp` | Widget inspector dump |
| `ext.flutter.debugDumpRenderTree` | Render tree dump |
| `ext.flutter.showPerformanceOverlay` | Toggle perf overlay |
| `ext.flutter.debugPaint` | Toggle debug painting |
| `ext.flutter.inspector.show` | Show widget inspector |
| `ext.flutter.reassemble` | Trigger hot reload |
| `ext.flutter.activeDevToolsServerAddress` | Set DevTools address |

### Acceptance Criteria

1. `callService` forwards method and params to VM Service
2. Response contains the raw VM Service result
3. Invalid method names return error (from VM Service, not adapter)
4. All invocations logged at debug level
5. 4+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_call_service_forwards_method() {
    // MockBackend: call_service("ext.flutter.debugDumpApp", None) returns JSON
    // Verify response body contains the result
}

#[tokio::test]
async fn test_call_service_missing_method_returns_error() {
    // Request with no "method" argument
    // Verify error response
}
```

### Notes

- This is a must-have for full DevTools integration. Without it, VS Code's Flutter extension cannot toggle debug features.
- The `callService` request is a custom Dart-specific request — not in the DAP specification. IDEs with Dart support expect it.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/handlers.rs` | Added `"callService"` to dispatch table; added `handle_call_service` method inside `impl<B: DebugBackend> DapAdapter<B>` |
| `crates/fdemon-dap/src/adapter/tests/call_service.rs` | New file — 8 unit tests covering all acceptance criteria |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Registered `mod call_service` |

### Notable Decisions/Tradeoffs

1. **Handler placement**: `handle_call_service` is placed at the end of the existing `impl` block in `handlers.rs`, consistent with how other handlers are organized. The dispatch entry is added before the catch-all `_` arm.
2. **Error wrapping**: Backend errors are wrapped as `"callService failed: {e}"` so the caller knows which request failed, while the raw VM Service error message is still included. This matches the pattern used by other handlers.
3. **Arguments handling**: Uses `match request.arguments.as_ref()` pattern (not the `parse_args` helper) because `callService` arguments are free-form JSON, not a typed struct.

### Testing Performed

- `cargo check -p fdemon-dap` — Passed
- `cargo test -p fdemon-dap call_service` — Passed (8/8 tests)
- `cargo test -p fdemon-dap` — Passed (717 tests)
- `cargo clippy -p fdemon-dap -- -D warnings` — Passed (no new warnings)
- `cargo fmt --all` — Passed

### Risks/Limitations

1. **No rate limiting**: `callService` is unrestricted per the spec — the VM Service handles authorization. Callers from localhost can invoke any service extension. This is by design and matches the Dart DDS adapter behaviour.
