## Task: Expand DebugBackend Trait with Phase 6 Methods

**Objective**: Add all new methods required by Phase 6 features to the `DebugBackend` trait and implement them in `VmServiceBackend`. These methods are prerequisites for globals scope, `callService`, `updateDebugOptions`, `restartFrame`, and other Phase 6 tasks.

**Depends on**: None

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/backend.rs`: Add new methods to `LocalDebugBackend` trait and `DynDebugBackendInner` trait, add delegation in `DynDebugBackend`
- `crates/fdemon-app/src/handler/dap_backend.rs`: Implement new methods in `VmServiceBackend`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/vm_service/debugger.rs`: Existing RPC wrappers for new methods to call
- `crates/fdemon-daemon/src/vm_service/client.rs`: `VmRequestHandle::request()` for raw RPC calls

### Details

Add these methods to the `DebugBackend` trait:

#### 1. `get_isolate(isolate_id) -> Result<Value>`
Calls `getIsolate` VM Service RPC. Returns the full isolate object including `rootLib`, `libraries[]`, `pauseEvent`, etc. This is the reliable way to get `rootLib` (vs the current `get_vm()` heuristic) and is needed for globals scope (library enumeration) and `updateDebugOptions` (setting library debuggability).

```rust
// In LocalDebugBackend trait:
async fn get_isolate(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError>;

// In VmServiceBackend:
async fn get_isolate(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError> {
    self.handle.request("getIsolate", Some(serde_json::json!({
        "isolateId": isolate_id
    }))).await.map_err(|e| BackendError::VmServiceError(e.to_string()))
}
```

#### 2. `call_service(method, params) -> Result<Value>`
Forwards arbitrary VM Service RPCs. Used by `callService` custom DAP request for DevTools integration.

```rust
async fn call_service(&self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value, BackendError>;

// Implementation:
async fn call_service(&self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value, BackendError> {
    self.handle.request(method, params)
        .await.map_err(|e| BackendError::VmServiceError(e.to_string()))
}
```

#### 3. `set_library_debuggable(isolate_id, library_id, is_debuggable) -> Result<()>`
Calls `setLibraryDebuggable` VM Service RPC. Used by `updateDebugOptions` to toggle SDK/external library stepping.

```rust
async fn set_library_debuggable(&self, isolate_id: &str, library_id: &str, is_debuggable: bool) -> Result<(), BackendError>;

// Implementation:
async fn set_library_debuggable(&self, isolate_id: &str, library_id: &str, is_debuggable: bool) -> Result<(), BackendError> {
    self.handle.request("setLibraryDebuggable", Some(serde_json::json!({
        "isolateId": isolate_id,
        "libraryId": library_id,
        "isDebuggable": is_debuggable
    }))).await.map(|_| ()).map_err(|e| BackendError::VmServiceError(e.to_string()))
}
```

#### 4. `get_source_report(isolate_id, script_id, report_kinds, token_pos, end_token_pos) -> Result<Value>`
Calls `getSourceReport` VM Service RPC. Used by `breakpointLocations` to find valid breakpoint positions.

```rust
async fn get_source_report(
    &self,
    isolate_id: &str,
    script_id: &str,
    report_kinds: &[&str],
    token_pos: Option<i64>,
    end_token_pos: Option<i64>,
) -> Result<serde_json::Value, BackendError>;
```

#### 5. Update `DynDebugBackendInner` and `DynDebugBackend`

For each new method, add the `_boxed` variant to `DynDebugBackendInner` and the delegation in `DynDebugBackend`. Follow the existing pattern:

```rust
// DynDebugBackendInner:
fn get_isolate_boxed(&self, isolate_id: &str) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, BackendError>> + Send + '_>>;

// DynDebugBackend:
pub async fn get_isolate(&self, isolate_id: &str) -> Result<serde_json::Value, BackendError> {
    self.inner.get_isolate_boxed(isolate_id).await
}
```

#### 6. Update MockBackend in test files

Add default implementations for all new methods in `MockBackend` that return sensible defaults (e.g., empty JSON objects or `Ok(())`).

### Acceptance Criteria

1. `DebugBackend` trait has `get_isolate`, `call_service`, `set_library_debuggable`, `get_source_report` methods
2. `VmServiceBackend` implements all four methods correctly
3. `DynDebugBackend` wrapper delegates all four methods
4. `MockBackend` in tests has default implementations
5. `cargo check --workspace` passes
6. `cargo test --workspace` passes
7. 8+ new unit tests for the new backend methods

### Testing

```rust
#[tokio::test]
async fn test_get_isolate_returns_isolate_data() {
    // MockBackend returns a JSON object with rootLib, libraries[], etc.
    // Verify the response structure
}

#[tokio::test]
async fn test_call_service_forwards_method_and_params() {
    // MockBackend records the method and params passed
    // Verify correct forwarding
}
```

### Notes

- `get_source_report` wraps the VM Service's `getSourceReport` which takes `reports` as an array of `SourceReportKind` strings (e.g., `["PossibleBreakpoints"]`). The `forceCompile: true` parameter should be included for breakpoint location accuracy.
- The `get_isolate` method replaces the fragile `get_vm()` → scan isolates heuristic used in `evaluate.rs:get_root_library_id`. Once this task is done, `get_root_library_id` should be updated to use `get_isolate` instead (can be done in a follow-up or as part of Task 03).
