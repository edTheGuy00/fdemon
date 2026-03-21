## Task: Adapter Module Cleanup (Visibility, Dead Code, Error Types, Docs)

**Objective**: Fix `exception_refs` visibility (L2), remove dead error constants (L3), fix `get_source` error type (L7), update stale module docs (L10), and extract duplicated `get_source_report` params (L11).

**Depends on**: None

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/mod.rs`: Fix L2 (visibility) and L10 (stale docs)
- `crates/fdemon-dap/src/adapter/types.rs`: Fix L3 (dead constants)
- `crates/fdemon-dap/src/adapter/backend.rs`: Fix L7 (error type)
- `crates/fdemon-app/src/handler/dap_backend.rs`: Fix L7 (implementation) and L11 (param duplication)

**Files Read (Dependencies):**
- None

### Details

#### Fix 1: L2 — Restrict `exception_refs` to `pub(crate)` (mod.rs:173)

```rust
// Before:
pub exception_refs: HashMap<i64, ExceptionRef>,

// After:
pub(crate) exception_refs: HashMap<i64, ExceptionRef>,
```

All accesses are within the `fdemon-dap` crate (`variables.rs`, `handlers.rs`, `events.rs`). No external crate consumer needs `pub`.

#### Fix 2: L3 — Remove dead error constants (types.rs:256–273)

Remove the four unused constants and their `#[allow(dead_code)]` attributes:

```rust
// REMOVE these four:
#[allow(dead_code)]
pub(crate) const ERR_NOT_CONNECTED: i64 = 1000;
#[allow(dead_code)]
pub(crate) const ERR_NO_DEBUG_SESSION: i64 = 1001;
#[allow(dead_code)]
pub(crate) const ERR_THREAD_NOT_FOUND: i64 = 1002;
#[allow(dead_code)]
pub(crate) const ERR_EVAL_FAILED: i64 = 1003;
#[allow(dead_code)]
pub(crate) const ERR_TIMEOUT: i64 = 1004;

// KEEP (actually used):
pub(crate) const ERR_VM_DISCONNECTED: i64 = 1005;
```

If structured error codes are needed in the future, they can be re-added when there are actual consumers.

#### Fix 3: L7 — Change `get_source` error type to `BackendError` (backend.rs)

In the `LocalDebugBackend` trait:
```rust
// Before:
async fn get_source(&self, isolate_id: &str, script_id: &str) -> Result<String, String>;

// After:
async fn get_source(&self, isolate_id: &str, script_id: &str) -> Result<String, BackendError>;
```

Update the corresponding methods in:
- `DynDebugBackendInner::get_source_boxed` — change return type
- `DynDebugBackend::get_source` — update delegation
- `NoopBackend::get_source` — return `Err(BackendError::NotConnected)`
- `VmServiceBackend::get_source` in `dap_backend.rs` — wrap string error: `.map_err(|e| BackendError::VmServiceError(e))`

Also update any callers that match on `Err(String)` to match on `Err(BackendError)` instead.

#### Fix 4: L10 — Update stale module docs (mod.rs:1–20)

Replace the four-module doc list with the actual nine modules:

```rust
//! ## Sub-modules
//!
//! - [`backend`] — `DebugBackend` trait and dynamic dispatch wrapper
//! - [`breakpoints`] — Breakpoint state, conditional/logpoint handling
//! - [`evaluate`] — Expression evaluation, `handle_evaluate`
//! - [`events`] — Debug event handling, progress events, auto-resume
//! - [`handlers`] — DAP request handlers (restart, loaded sources, completions, etc.)
//! - [`stack`] — Frame/variable/source-reference stores, `handle_stack_trace`, `handle_scopes`
//! - [`threads`] — Thread/isolate ID mapping, `handle_threads`, `handle_attach`
//! - [`types`] — Constants, enums, timeout definitions
//! - [`variables`] — Variable expansion, globals, exceptions, getters, toString enrichment
```

#### Fix 5: L11 — Extract `get_source_report` params helper (dap_backend.rs)

The JSON parameter construction is duplicated between `get_source_report` (lines 323–338) and `get_source_report_boxed` (lines 534–551). Extract a private helper:

```rust
fn build_source_report_params(
    isolate_id: &str,
    script_id: &str,
    report_kinds: &[String],
    token_pos: Option<i64>,
    end_token_pos: Option<i64>,
) -> serde_json::Value {
    let mut params = serde_json::json!({
        "isolateId": isolate_id,
        "scriptId": script_id,
        "reports": report_kinds,
        "forceCompile": true,
    });
    if let Some(tp) = token_pos { params["tokenPos"] = serde_json::json!(tp); }
    if let Some(etp) = end_token_pos { params["endTokenPos"] = serde_json::json!(etp); }
    params
}
```

Then both `get_source_report` and `get_source_report_boxed` call this helper instead of inlining the construction.

### Acceptance Criteria

1. `exception_refs` field is `pub(crate)`, not `pub` — verify no compile errors
2. No `#[allow(dead_code)]` on error constants in `types.rs`
3. `get_source` returns `Result<String, BackendError>` — consistent with all other backend methods
4. Module doc in `mod.rs` lists all 9 current sub-modules
5. `get_source_report` param construction is not duplicated
6. `cargo test --workspace` passes (both `fdemon-dap` and `fdemon-app` crates)
7. `cargo clippy --workspace` clean

### Testing

No new tests needed — these are cleanup/refactoring changes. All existing tests must pass. The `get_source` error type change may require updating test assertions that match on `Err(String)`.

### Notes

- Fix 3 (L7) touches both `fdemon-dap` and `fdemon-app` — ensure both crates compile. The `NoopBackend` in test helpers also needs updating.
- Fix 5 (L11) is in `fdemon-app/handler/dap_backend.rs` — the helper function should be `fn` (not `async fn`) since it's pure parameter construction.
