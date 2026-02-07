## Task: Lock Down fdemon-daemon Public API

**Objective**: Define a clean public API for `fdemon-daemon` by internalizing protocol wire-format types, parsing helpers, and global counters that are implementation details.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-daemon/src/lib.rs`: Remove internal items from re-exports
- `crates/fdemon-daemon/src/protocol.rs`: Make `RawMessage`, `LogEntryInfo`, `strip_brackets()` internal
- `crates/fdemon-daemon/src/commands.rs`: Make `next_request_id()` internal

### Details

#### 1. Internalize Protocol Wire-Format Types

The `protocol.rs` module exposes JSON-RPC wire-format internals that downstream crates should never use directly. The public API should be the parsed functions.

**In `protocol.rs`**, change visibility:

| Item | Current | New | Reason |
|------|---------|-----|--------|
| `RawMessage` enum | `pub enum` | `pub(crate) enum` | Wire-format envelope with raw `serde_json::Value` -- only used by `parse_daemon_message()` |
| `LogEntryInfo` struct | `pub struct` | `pub(crate) struct` | Intermediate conversion type -- only used by `to_log_entry()` |
| `strip_brackets()` | `pub fn` | `pub(crate) fn` | Low-level bracket-stripping helper -- only used by `parse_daemon_message()` |

**In `lib.rs`**, update the protocol re-exports:

```rust
// BEFORE:
pub use protocol::{
    detect_log_level, parse_daemon_message, parse_flutter_log, strip_brackets, to_log_entry,
    LogEntryInfo, RawMessage,
};

// AFTER:
pub use protocol::{detect_log_level, parse_daemon_message, parse_flutter_log, to_log_entry};
```

#### 2. Internalize Global Request ID Counter

`next_request_id()` is a global atomic counter that should be internal to `CommandSender`. No external code should generate request IDs independently.

**In `commands.rs`**, change visibility:

| Item | Current | New | Reason |
|------|---------|-----|--------|
| `next_request_id()` | `pub fn` | `pub(crate) fn` | Global counter -- only used by `CommandSender::send()` and `DaemonCommand::to_json()` |

**In `lib.rs`**, update the commands re-exports:

```rust
// BEFORE:
pub use commands::{
    next_request_id, CommandResponse, CommandSender, DaemonCommand, RequestTracker,
};

// AFTER:
pub use commands::{CommandResponse, CommandSender, DaemonCommand, RequestTracker};
```

#### 3. Verify No External Breakage

Before making changes, verify no other crate depends on the items being internalized:

```bash
# Search for RawMessage usage outside fdemon-daemon
grep -r "RawMessage" crates/fdemon-core/ crates/fdemon-app/ crates/fdemon-tui/ src/

# Search for LogEntryInfo usage outside fdemon-daemon
grep -r "LogEntryInfo" crates/fdemon-core/ crates/fdemon-app/ crates/fdemon-tui/ src/

# Search for strip_brackets usage outside fdemon-daemon
grep -r "strip_brackets" crates/fdemon-core/ crates/fdemon-app/ crates/fdemon-tui/ src/

# Search for next_request_id usage outside fdemon-daemon
grep -r "next_request_id" crates/fdemon-core/ crates/fdemon-app/ crates/fdemon-tui/ src/
```

If any external usage is found, update the external code first (replace with higher-level API calls) before internalizing.

#### 4. Review Remaining Exports

Confirm these items are correctly public and should stay:

| Item | Module | Keep Public? | Reason |
|------|--------|-------------|--------|
| `FlutterProcess` | `process.rs` | Yes | Core daemon API |
| `CommandSender` | `commands.rs` | Yes | Used by Engine for sending commands |
| `DaemonCommand` | `commands.rs` | Yes | Command type for JSON-RPC |
| `CommandResponse` | `commands.rs` | Yes | Response type |
| `RequestTracker` | `commands.rs` | Yes | Used by SessionHandle |
| `Device` | `devices.rs` | Yes | Core domain type |
| `discover_devices()` | `devices.rs` | Yes | Public API |
| `Emulator` | `emulators.rs` | Yes | Core domain type |
| `BootCommand` | `lib.rs` | Yes | Used by app crate |
| `parse_daemon_message()` | `protocol.rs` | Yes | Public parsing API |
| `to_log_entry()` | `protocol.rs` | Yes | Public conversion API |
| `detect_log_level()` | `protocol.rs` | Yes | Public log level detection |
| `parse_flutter_log()` | `protocol.rs` | Yes | Public log parsing |

### Acceptance Criteria

1. `RawMessage` is not accessible from outside `fdemon-daemon`
2. `LogEntryInfo` is not accessible from outside `fdemon-daemon`
3. `strip_brackets()` is not accessible from outside `fdemon-daemon`
4. `next_request_id()` is not accessible from outside `fdemon-daemon`
5. `lib.rs` re-exports only the intended public API
6. `cargo check -p fdemon-daemon` passes
7. `cargo test -p fdemon-daemon` passes
8. `cargo check --workspace` passes
9. `cargo test --workspace` passes

### Testing

```bash
# Crate-level verification
cargo check -p fdemon-daemon
cargo test -p fdemon-daemon

# Full workspace verification
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
```

### Notes

- Tests within `fdemon-daemon` can still access `pub(crate)` items via `use super::*`
- The `test_utils` module is already gated behind `#[cfg(any(test, feature = "test-helpers"))]` -- no changes needed
- The `BootCommand` type in `lib.rs` is a public API item (not in any submodule) -- keep as-is
- The `DaemonMessage` re-export from `fdemon-core` should remain for convenience
- Do NOT change `pub mod` declarations to `pub(crate) mod` in this task -- only change individual item visibility and `lib.rs` re-exports
