## Task: Extract Performance Module from actions/mod.rs

**Objective**: Move `spawn_performance_polling` into `actions/performance.rs`.

**Depends on**: 03-extract-vm-service-module

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Remove performance functions
- `crates/fdemon-app/src/actions/performance.rs` — **NEW**

### Details

#### Functions to move

| Function | Current Lines (approx) | Purpose |
|----------|----------------------|---------|
| `spawn_performance_polling` | ~703-881 | Periodic memory usage + allocation profile polling via VM Service |

#### Constants to move

| Constant | Value | Purpose |
|----------|-------|---------|
| `PERF_POLL_MIN_MS` | 500ms | Minimum memory polling interval |
| `ALLOC_PROFILE_POLL_MIN_MS` | 1000ms | Minimum allocation profile polling interval |

#### Imports for performance.rs

```rust
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::vm_service::{ext, VmRequestHandle};
```

#### Update mod.rs

1. Add `mod performance;`
2. Update `handle_action` `StartPerformanceMonitoring` arm to call `performance::spawn_performance_polling(...)`
3. Remove moved function and constants from `mod.rs`

### Acceptance Criteria

1. `spawn_performance_polling` lives in `actions/performance.rs`
2. `PERF_POLL_MIN_MS` and `ALLOC_PROFILE_POLL_MIN_MS` moved to `performance.rs`
3. `performance.rs` has a `//!` module doc header
4. `cargo check --workspace` passes
5. `cargo test --workspace` passes
6. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new tests needed — pure move refactoring. All existing tests must pass.

### Notes

- `spawn_performance_polling` creates a `watch::channel` internally and returns `()` (it sends `VmServicePerformanceMonitoringStarted` via `msg_tx`). Verify the return type matches what `handle_action` expects.
- The function has extensive doc comments (~40 lines) — move them with the function.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/performance.rs` | **NEW** — 246 lines. Contains `PERF_POLL_MIN_MS`, `ALLOC_PROFILE_POLL_MIN_MS`, and `spawn_performance_polling` with full doc comments and `//!` module header |
| `crates/fdemon-app/src/actions/mod.rs` | Reduced from ~1,464 lines (pre-task) to 1,243 lines. Removed `PERF_POLL_MIN_MS` constant, `ALLOC_PROFILE_POLL_MIN_MS` constant, and `spawn_performance_polling` function. Added `pub(super) mod performance;`. Updated `StartPerformanceMonitoring` arm to call `performance::spawn_performance_polling(...)`. Removed unused `info` from `tracing` import. |

### Notable Decisions/Tradeoffs

1. **Imports trimmed**: The task spec suggested `use tokio::sync::{mpsc, watch}` for `performance.rs`, but the actual implementation only uses `mpsc` directly (the `watch` channel is constructed via `tokio::sync::watch::channel` inline). The import was trimmed to what clippy requires: `use tokio::sync::mpsc`. Similarly, `use tracing::info` (used for the shutdown log message) was used instead of `{debug, warn}`.
2. **`info!` import removed from mod.rs**: After extraction, `info!` (bare, non-qualified) was no longer referenced in `mod.rs`. It was removed to keep the import clean and satisfy `-D warnings`.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test --workspace` — Passed (2,803 tests run: 1161 + 360 + 383 + 773 + 10 + 16 + 80 + 7 + 1 + 5 + 7 = all ok, 0 failed)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Pure refactoring, no behavioral change**: All function logic, doc comments, and constant values are identical to the original. No risks introduced.
