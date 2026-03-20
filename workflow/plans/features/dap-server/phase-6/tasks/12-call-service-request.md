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
