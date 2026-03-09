## Task: Type Safety Improvements for DebugBackend Trait

**Objective**: Replace stringly-typed patterns in the `DebugBackend` trait with proper Rust types: typed error enum instead of `Result<_, String>`, and a typed enum for exception pause modes instead of `&str`.

**Depends on**: None

**Estimated Time**: 2–3 hours

**Severity**: MINOR — correctness and maintainability improvement, no runtime behavior change.

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: `DebugBackend` trait definition (lines 55–125)
- `crates/fdemon-app/src/handler/dap_backend.rs`: `VmServiceBackend` implementation
- `crates/fdemon-dap/src/adapter/mod.rs`: `NoopBackend` implementation
- `crates/fdemon-dap/src/adapter/breakpoints.rs`: Breakpoint handlers using backend results
- `crates/fdemon-dap/src/adapter/evaluate.rs`: Evaluate handlers using backend results
- Test files with `MockBackend`

### Details

#### Fix 11: Typed Errors in `DebugBackend` Trait

**Current**: All trait methods return `Result<_, String>`:
```rust
async fn pause(&self, isolate_id: &str) -> Result<(), String>;
async fn resume(&self, isolate_id: &str, step: Option<StepMode>) -> Result<(), String>;
async fn add_breakpoint(...) -> Result<BreakpointResult, String>;
// ... etc
```

**Problem**: Per CODE_STANDARDS.md, "All errors MUST use the `Error` enum" and "stringly-typed errors" are listed as an anti-pattern.

**Fix**: Define a `BackendError` enum in `fdemon-dap/src/adapter/mod.rs`:

```rust
/// Errors returned by [`DebugBackend`] implementations.
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    /// The requested isolate was not found or is no longer running.
    #[error("isolate not found: {0}")]
    IsolateNotFound(String),

    /// A VM Service RPC call failed.
    #[error("VM Service error: {0}")]
    VmServiceError(String),

    /// The backend is not connected to a VM Service.
    #[error("not connected")]
    NotConnected,

    /// The operation is not supported by this backend.
    #[error("not supported: {0}")]
    NotSupported(String),
}
```

Then update all trait methods:
```rust
async fn pause(&self, isolate_id: &str) -> Result<(), BackendError>;
async fn resume(&self, isolate_id: &str, step: Option<StepMode>) -> Result<(), BackendError>;
// ... etc
```

Update callers in adapter handlers to convert `BackendError` to DAP error responses:
```rust
match self.backend.pause(isolate_id).await {
    Ok(()) => DapResponse::success(request, None),
    Err(e) => DapResponse::error(request, &e.to_string()),
}
```

Update `VmServiceBackend` in `dap_backend.rs`:
```rust
async fn pause(&self, isolate_id: &str) -> Result<(), BackendError> {
    debugger::pause(&self.handle, isolate_id)
        .await
        .map_err(|e| BackendError::VmServiceError(e.to_string()))
}
```

Update `NoopBackend`:
```rust
async fn pause(&self, _isolate_id: &str) -> Result<(), BackendError> {
    Err(BackendError::NotConnected)
}
```

---

#### Fix 12: Typed Enum for Exception Pause Mode

**Current**: `set_exception_pause_mode` takes `mode: &str`:
```rust
// Trait:
async fn set_exception_pause_mode(&self, isolate_id: &str, mode: &str) -> Result<(), String>;

// Implementation in dap_backend.rs:
async fn set_exception_pause_mode(&self, isolate_id: &str, mode: &str) -> Result<(), String> {
    let vm_mode = match mode {
        "All" => ExceptionPauseMode::All,
        "Unhandled" => ExceptionPauseMode::Unhandled,
        _ => ExceptionPauseMode::None,  // silently swallows unknown modes
    };
    // ...
}
```

**Problem**: The wildcard `_` arm silently maps unrecognized modes (including typos like `"all"` or future DAP values like `"UserUnhandled"`) to `None` without warning or error.

**Fix**: Define a `DapExceptionPauseMode` enum in `fdemon-dap`:

```rust
/// Exception pause mode as specified in DAP `setExceptionBreakpoints`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DapExceptionPauseMode {
    /// Pause on all exceptions (caught and uncaught).
    All,
    /// Pause only on uncaught exceptions.
    Unhandled,
    /// Do not pause on exceptions.
    None,
}
```

Update the trait:
```rust
async fn set_exception_pause_mode(
    &self,
    isolate_id: &str,
    mode: DapExceptionPauseMode,
) -> Result<(), BackendError>;
```

Move the string → enum conversion to the adapter layer (where the DAP request is parsed), not the backend:
```rust
// In the adapter's setExceptionBreakpoints handler:
let mode = match filter_id.as_str() {
    "All" => DapExceptionPauseMode::All,
    "Unhandled" => DapExceptionPauseMode::Unhandled,
    "None" => DapExceptionPauseMode::None,
    other => {
        tracing::warn!("Unknown exception pause mode: {}", other);
        return DapResponse::error(request, &format!("Unknown exception filter: {}", other));
    }
};
self.backend.set_exception_pause_mode(isolate_id, mode).await?;
```

### Acceptance Criteria

1. All `DebugBackend` methods return `Result<_, BackendError>` instead of `Result<_, String>`
2. `BackendError` has at least `NotConnected`, `VmServiceError`, `IsolateNotFound` variants
3. `set_exception_pause_mode` accepts `DapExceptionPauseMode` enum, not `&str`
4. Unknown exception filter strings produce a DAP error response (not silent fallback)
5. All existing tests pass (update `MockBackend` and test assertions)
6. `cargo clippy --workspace` passes
7. No new `String` error types introduced elsewhere

### Testing

- Update all `MockBackend` methods to return `BackendError`
- Update test assertions that check error strings
- New test: `set_exception_pause_mode` with unknown filter returns error
- New test: `BackendError::NotConnected` produces appropriate DAP error response

### Notes

- This task touches the `DebugBackend` trait which is a central abstraction. The change is mechanical (find/replace error types, update match arms) but wide-reaching. Run `cargo test --workspace` frequently during implementation.
- If `thiserror` is not already a dependency of `fdemon-dap`, add it. Check `Cargo.toml`.
- The `BackendError` enum may grow in future phases — keep variants focused on current needs, not hypothetical future errors.
- Consider whether `BackendError` should implement `From<fdemon_core::Error>` for ergonomic conversion from the core error type.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/Cargo.toml` | Added `thiserror.workspace = true` to `[dependencies]` |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `BackendError` enum (with `Clone` derive), added `DapExceptionPauseMode` enum, updated `LocalDebugBackend`/`DebugBackend` trait to use `BackendError` return types and `DapExceptionPauseMode` parameter, updated `DynDebugBackendInner` and `DynDebugBackend` wrappers, updated `DapAdapter.exception_mode` field type, updated `handle_set_exception_breakpoints` to validate unknown filters and return DAP error, updated `exception_filter_to_mode` to return `DapExceptionPauseMode`, updated all test mock backends, added new tests |
| `crates/fdemon-dap/src/lib.rs` | Added `BackendError` and `DapExceptionPauseMode` to re-exports |
| `crates/fdemon-dap/src/server/session.rs` | Updated `NoopBackend` impl to use `BackendError`, updated `MockBackend` in tests to use `BackendError` and `DapExceptionPauseMode` |
| `crates/fdemon-dap/src/server/mod.rs` | Updated `MockBackendInner` in tests to use `BackendError` and `DapExceptionPauseMode` |
| `crates/fdemon-dap/src/adapter/evaluate.rs` | Updated `get_root_library_id` to call `.map_err(|e| e.to_string())?` on backend, updated error propagation in `handle_evaluate`, updated `MockBackend` in tests to use `BackendError` and `DapExceptionPauseMode` |
| `crates/fdemon-app/src/handler/dap_backend.rs` | Updated `VmServiceBackend` `DebugBackend` impl to return `BackendError` (via `.map_err(|e| BackendError::VmServiceError(e.to_string()))`), changed `set_exception_pause_mode` to take `DapExceptionPauseMode` and match on enum variants, updated `DynDebugBackendInner` impl to use `BackendError` types and `DapExceptionPauseMode` parameter |

### Notable Decisions/Tradeoffs

1. **`BackendError` derives `Clone`**: The `MockBackend` in `evaluate.rs` stores `eval_result: Result<serde_json::Value, BackendError>` and calls `.clone()` to return it from multiple async fns. Adding `Clone` to `BackendError` was the cleanest fix. All variants hold `String` or are unit variants, so `Clone` is cheap.

2. **`get_root_library_id` stays `Result<String, String>`**: This internal helper (called only by `handle_evaluate`) keeps its `String` error type to avoid a two-step propagation. The call site converts to `BackendError::VmServiceError(e)` before returning.

3. **`DynDebugBackendInner.set_exception_pause_mode_boxed` takes `DapExceptionPauseMode` by value**: Since it's `Copy`, no lifetime is needed. This is cleaner than the original `&'a str` design.

4. **Unknown exception filter returns DAP error**: The adapter now validates all filters before calling the backend. Any unknown filter string produces a `DapResponse::error(...)` instead of a silent fallback to `None`.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test --workspace` — Passed (3,387 tests: 0 failed, 69 ignored)
- `cargo fmt --all` — Passed (no formatting issues)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **`BackendError::Clone`**: Adding `Clone` makes the error type slightly heavier, but all variants are already `Clone`-capable (`String` is `Clone`, unit variants copy trivially). No semantic risk.
2. **String-based error wrapping**: All VM Service errors are wrapped via `BackendError::VmServiceError(e.to_string())`, losing the original error type. This is acceptable for Phase 3 — future phases can add structured error conversion if needed.
