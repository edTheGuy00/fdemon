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
