## Task: Extract VM Service Module from actions/mod.rs

**Objective**: Move `spawn_vm_service_connection` and `forward_vm_events` (including heartbeat logic) into `actions/vm_service.rs`.

**Depends on**: 02-extract-session-module

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Remove VM service functions
- `crates/fdemon-app/src/actions/vm_service.rs` — **NEW**

### Details

#### Functions to move

| Function | Current Lines (approx) | Purpose |
|----------|----------------------|---------|
| `spawn_vm_service_connection` | ~882-977 | Spawns tokio task, connects VmServiceClient, enables streams, sends Messages |
| `forward_vm_events` | ~985-1133 | Main event loop: stream events → Messages, heartbeat probe + failure counter |

#### Constants to move

These constants are used exclusively in `forward_vm_events`:

| Constant | Value | Purpose |
|----------|-------|---------|
| `HEARTBEAT_INTERVAL` | 30s | Interval between heartbeat probes |
| `HEARTBEAT_TIMEOUT` | 5s | Per-probe timeout |
| `MAX_HEARTBEAT_FAILURES` | 3 | Threshold before declaring connection dead |

#### Imports for vm_service.rs

```rust
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info, warn};

use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::vm_service::{
    enable_frame_tracking, flutter_error_to_log_entry, parse_flutter_error,
    parse_frame_timing, parse_gc_event, parse_log_record, vm_log_to_log_entry,
    VmClientEvent, VmServiceClient,
};
```

#### Update mod.rs

1. Add `mod vm_service;`
2. Update `handle_action` `ConnectVmService` arm to call `vm_service::spawn_vm_service_connection(...)`
3. Remove moved functions and constants from `mod.rs`
4. Remove now-unused imports from `mod.rs`

### Acceptance Criteria

1. `spawn_vm_service_connection` and `forward_vm_events` live in `actions/vm_service.rs`
2. `HEARTBEAT_INTERVAL`, `HEARTBEAT_TIMEOUT`, `MAX_HEARTBEAT_FAILURES` moved to `vm_service.rs`
3. The heartbeat bug fix from task 01 (`consecutive_failures = 0` in Reconnecting/Reconnected arms) is preserved
4. `vm_service.rs` has a `//!` module doc header
5. `cargo check --workspace` passes
6. `cargo test --workspace` passes
7. `cargo clippy --workspace -- -D warnings` clean

### Testing

Move the `test_heartbeat_constants_are_reasonable` test and the `test_heartbeat_counter_reset_on_reconnection` test (from task 01) into a `#[cfg(test)] mod tests` block in `vm_service.rs`.

### Notes

- `forward_vm_events` is `async fn` (not `pub`) — it's only called from `spawn_vm_service_connection` in the same module. Both can remain private to the `vm_service` module.
- `spawn_vm_service_connection` returns a `JoinHandle<()>` used by `handle_action` — its return type must be accessible from `mod.rs`.
- The `parse_*` and `*_to_log_entry` functions are imported from `fdemon_daemon::vm_service` — they stay as imports, not moved.
