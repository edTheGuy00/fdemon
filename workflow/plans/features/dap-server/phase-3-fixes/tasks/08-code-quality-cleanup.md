## Task: Code Quality Cleanup (6 Minor Fixes)

**Objective**: Address 6 minor code quality issues identified in the Phase 3 review. These are independent, low-risk changes that can be batched into a single task.

**Depends on**: None

**Estimated Time**: 2–3 hours

**Severity**: MINOR — style/quality issues, no functional impact.

### Scope

Multiple files across `fdemon-dap` and `fdemon-app`.

### Fix List

#### Fix 8: Change `pub mod dap_backend` to `pub(crate)`

**File**: `crates/fdemon-app/src/handler/mod.rs:21`

```rust
// Before:
pub mod dap_backend;

// After:
pub(crate) mod dap_backend;
```

**Rationale**: All other handler submodules (daemon, dap, devtools, helpers, keys, log_view, etc.) are `pub(crate)`. This is the only one with full public visibility. `VmServiceBackend` should not be part of `fdemon-app`'s public API — it's an internal implementation detail.

**Verification**: `cargo build --workspace` succeeds. If the binary crate imports `dap_backend` directly, the import path needs updating (check with `grep -r "handler::dap_backend" src/`).

---

#### Fix 9: Remove stale `#[allow(dead_code)]` from `backend` field

**File**: `crates/fdemon-dap/src/adapter/mod.rs:295-296`

```rust
// Before:
#[allow(dead_code)]
backend: B,

// After:
backend: B,
```

**Rationale**: The `backend` field IS used by handlers (`handle_attach`, `handle_continue`, `handle_pause`, etc.). The annotation was left over from an earlier phase when handlers weren't yet implemented. `cargo clippy` should not warn about this field now.

**Verification**: `cargo clippy --workspace` passes without the suppression.

---

#### Fix 10: Log serialization errors instead of `unwrap_or_default()`

**Files**:
- `crates/fdemon-dap/src/server/session.rs:587`
- `crates/fdemon-dap/src/adapter/evaluate.rs:132`

**session.rs:587** — `Capabilities` serialization:
```rust
// Before:
let body = serde_json::to_value(&capabilities).unwrap_or_default();

// After:
let body = match serde_json::to_value(&capabilities) {
    Ok(v) => v,
    Err(e) => {
        tracing::error!("Failed to serialize DAP capabilities: {}", e);
        serde_json::Value::Object(Default::default())
    }
};
```

**evaluate.rs:132** — `EvaluateResponseBody` serialization:
```rust
// Before:
let body_json = serde_json::to_value(&body).unwrap_or_default();

// After:
let body_json = match serde_json::to_value(&body) {
    Ok(v) => v,
    Err(e) => {
        tracing::error!("Failed to serialize evaluate response: {}", e);
        return DapResponse::error(request, &format!("Internal error: {}", e));
    }
};
```

**Rationale**: `unwrap_or_default()` silently returns `Value::Null`, which would cause IDE breakage. While these structs should always serialize successfully, logging the error makes debugging much easier if a field type changes in the future.

---

#### Fix 13: Extract magic numbers into named constants

**Files**:
- `crates/fdemon-dap/src/server/mod.rs:331` — accept error backoff
- `crates/fdemon-app/src/actions/mod.rs` — channel capacity

```rust
// server/mod.rs — before:
tokio::time::sleep(std::time::Duration::from_millis(100)).await;

// After:
/// Backoff duration after a TCP accept error to prevent tight error loops
/// (e.g., file descriptor exhaustion).
const ACCEPT_ERROR_BACKOFF: std::time::Duration = std::time::Duration::from_millis(100);
// ...
tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
```

```rust
// actions/mod.rs — find the channel capacity `32` and extract:
/// Channel capacity for DAP server events (connect/disconnect/error notifications).
const DAP_EVENT_CHANNEL_CAPACITY: usize = 32;
```

---

#### Fix 14: Remove empty `transport/tcp.rs` re-export module

**File**: `crates/fdemon-dap/src/transport/tcp.rs`

The entire file is:
```rust
pub use crate::server::start as start_server;
```

**Action**: Remove this file. Update any imports that use `transport::tcp::start_server` to use `server::start` directly. Update `transport/mod.rs` to remove the `pub mod tcp;` declaration.

**Verification**: `grep -r "transport::tcp" crates/fdemon-dap/` — update all references.

---

#### Fix 15: Add security warning when binding to non-loopback

**File**: `crates/fdemon-dap/src/server/mod.rs` — in the `start()` function

```rust
// After binding successfully:
if config.bind_addr != "127.0.0.1" && config.bind_addr != "::1" && config.bind_addr != "localhost" {
    tracing::warn!(
        bind_addr = %config.bind_addr,
        "DAP server bound to non-loopback address. The 'evaluate' command allows \
         arbitrary code execution — binding to a network interface exposes this \
         to remote connections."
    );
}
```

**Rationale**: The `evaluate` DAP command runs arbitrary Dart expressions. Binding to `0.0.0.0` exposes this to the network. A warning in the logs gives the user visibility.

### Acceptance Criteria

1. `pub(crate) mod dap_backend` — compile succeeds, no external crate imports it directly
2. No `#[allow(dead_code)]` on `backend` field — clippy passes
3. Serialization errors are logged, not silently swallowed
4. Magic numbers replaced with named constants
5. `transport/tcp.rs` removed, no broken imports
6. Non-loopback bind produces a warning log
7. `cargo build --workspace` + `cargo test --workspace` + `cargo clippy --workspace` all pass

### Testing

- Existing tests pass (these are non-behavioral changes)
- New test for non-loopback warning: start server with `0.0.0.0`, verify warning is emitted (may require tracing test subscriber)
- Verify `cargo clippy` produces no new warnings

### Notes

- These are all independent, low-risk changes. If any single fix causes unexpected issues, it can be reverted without affecting the others.
- Fix 14 (remove tcp.rs) may require checking if any external code (tests, examples, docs) references `transport::tcp`.
