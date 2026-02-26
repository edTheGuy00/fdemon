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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/vm_service.rs` | NEW — 273 lines. Contains `spawn_vm_service_connection`, `forward_vm_events`, the three heartbeat constants, and the two heartbeat tests. Has `//!` module doc header. |
| `crates/fdemon-app/src/actions/mod.rs` | Removed `spawn_vm_service_connection`, `forward_vm_events`, `HEARTBEAT_INTERVAL`, `HEARTBEAT_TIMEOUT`, `MAX_HEARTBEAT_FAILURES`, and their tests (~260 lines removed). Added `pub(super) mod vm_service;`. Updated `ConnectVmService` arm to call `vm_service::spawn_vm_service_connection(...)`. Trimmed now-unused imports (`VmClientEvent`, `VmServiceClient`, `enable_frame_tracking`, `flutter_error_to_log_entry`, `parse_flutter_error`, `parse_frame_timing`, `parse_gc_event`, `parse_log_record`, `vm_log_to_log_entry`, `debug`, `error`, `info`). |

### Notable Decisions/Tradeoffs

1. **`pub(super)` visibility for `vm_service` module**: `spawn_vm_service_connection` needs to be accessible from `mod.rs` but not from outside the `actions` module. Used `pub(super) fn` on the function and `pub(super) mod vm_service` on the module declaration — consistent with how `session.rs` uses `pub(super)`.
2. **`watch` and `info` kept in `mod.rs`**: After extraction, `watch::Receiver<bool>` is still used in `handle_action`'s signature (for `shutdown_rx`) and `info!` is used in `spawn_performance_polling`. Both were retained in the imports.
3. **Heartbeat bug fix preserved exactly**: The `consecutive_failures = 0` assignments in the `Reconnecting` and `Reconnected` arms of `forward_vm_events` are present in `vm_service.rs` as extracted — no behavioral change.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (2,803 tests across all crates: 1161 + 360 + 383 + 773 + 10 + 16 + 80 + 7 + 1 + 5 + 0 + 7; 69 ignored)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Pure refactoring**: No behavioral changes introduced. The extraction is a mechanical move of code between files.
