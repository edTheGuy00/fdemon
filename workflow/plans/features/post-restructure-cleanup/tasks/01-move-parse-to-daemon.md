## Task: Move DaemonMessage parse logic from fdemon-core to fdemon-daemon

**Objective**: Relocate ~306 lines of JSON-RPC protocol parsing from `fdemon-core/src/events.rs` to `fdemon-daemon/src/protocol.rs` as free functions, restoring fdemon-core's architectural role as a pure domain types crate.

**Review Issue**: #1 (MAJOR) - DaemonMessage::parse() in fdemon-core violates stated architecture

**Depends on**: None

### Scope

- `crates/fdemon-core/src/events.rs`: Remove 6 methods from `impl DaemonMessage` block (lines 236-556) and `LogEntryInfo` struct (lines 228-234)
- `crates/fdemon-core/src/lib.rs`: Remove `LogEntryInfo` from re-exports
- `crates/fdemon-daemon/src/protocol.rs`: Add free functions + `LogEntryInfo` struct
- `crates/fdemon-daemon/src/lib.rs`: Update re-exports to include new functions
- `crates/fdemon-app/src/handler/session.rs:19`: Update call site
- `crates/fdemon-app/src/process.rs:70`: Update call site
- `crates/fdemon-app/src/actions.rs:246`: Update call site
- `crates/fdemon-daemon/src/protocol.rs` (tests): Update ~30 test call sites
- `tests/fixture_parsing_test.rs`: Update 8 call sites
- `tests/e2e.rs`: Update 1 call site
- `tests/e2e/hot_reload.rs`: Update 1 call site
- `tests/e2e/daemon_interaction.rs`: Update 1 call site
- `docs/ARCHITECTURE.md`: Update 3 references to `DaemonMessage::parse()`

### Details

#### Why This Needs to Change

The architecture states fdemon-core should contain "pure business logic types with no infrastructure dependencies." `DaemonMessage::parse()` performs full JSON-RPC protocol parsing (`serde_json::from_str`, event name dispatch, `serde_json::from_value` deserialization). This is infrastructure logic that belongs in fdemon-daemon. The comment on line 240 ("For now, we'll parse directly using serde_json") confirms this was temporary.

#### Orphan Rule Workaround

Rust's orphan rule prevents `impl DaemonMessage` in fdemon-daemon since `DaemonMessage` is defined in fdemon-core. The solution is **free functions** in `fdemon-daemon/src/protocol.rs`:

```rust
// crates/fdemon-daemon/src/protocol.rs

/// Parses a JSON-RPC message from Flutter's --machine stdout.
pub fn parse_daemon_message(json: &str) -> Option<DaemonMessage> { ... }

/// Converts a DaemonMessage to a displayable log entry.
pub fn to_log_entry(msg: &DaemonMessage) -> Option<LogEntryInfo> { ... }

/// Parses a raw Flutter log line, detecting level and stripping prefixes.
pub fn parse_flutter_log(raw: &str, is_error: bool) -> (LogLevel, String) { ... }

/// Detects the log level from message content using pattern matching.
pub fn detect_log_level(message: &str) -> LogLevel { ... }
```

Internal helpers (not public):
```rust
fn parse_event(event: &str, params: serde_json::Value) -> DaemonMessage { ... }
fn unknown_event(event: &str, params: serde_json::Value) -> DaemonMessage { ... }
```

#### Methods to Keep in fdemon-core

These are pure data accessors and should remain as `impl DaemonMessage`:
- `app_id(&self) -> Option<&str>` (line 150)
- `is_error(&self) -> bool` (line 163)
- `summary(&self) -> String` (line 173)

#### LogEntryInfo Struct

Move `LogEntryInfo` (currently at `fdemon-core/src/events.rs:228-234`) to `fdemon-daemon/src/protocol.rs`. It is only used by the parsing/conversion code and does not need to be in core.

```rust
/// Intermediate log entry info produced by DaemonMessage conversion.
pub struct LogEntryInfo {
    pub level: LogLevel,
    pub message: String,
    pub source: LogSource,
}
```

#### Imports Needed in protocol.rs

```rust
use fdemon_core::ansi::{contains_word, strip_ansi_codes};
use fdemon_core::types::{LogLevel, LogSource};
use fdemon_core::{
    DaemonMessage, DaemonConnected, DaemonLogMessage, AppStart, AppStarted,
    AppStop, AppLog, AppProgress, AppDebugPort, DeviceInfo,
};
```

#### Consolidation Opportunity

`fdemon-daemon/src/protocol.rs` already contains a `RawMessage::parse()` (line 43) that does similar JSON-RPC parsing but returns a `RawMessage` struct. The moved `parse_daemon_message()` function should use `RawMessage` as an intermediary rather than duplicating JSON destructuring:

```rust
pub fn parse_daemon_message(json: &str) -> Option<DaemonMessage> {
    let raw = RawMessage::parse(json)?;
    match raw {
        RawMessage::Event { event, params } => Some(parse_event(&event, params)),
        RawMessage::Response { id, result, error } => {
            Some(DaemonMessage::Response { id, result, error })
        }
    }
}
```

#### Call Site Updates

All 3 production call sites change from:
```rust
DaemonMessage::parse(json)
```
to:
```rust
fdemon_daemon::parse_daemon_message(json)
// or with a use statement:
use fdemon_daemon::parse_daemon_message;
parse_daemon_message(json)
```

Similarly for `to_log_entry`:
```rust
// Before: msg.to_log_entry()
// After:  to_log_entry(&msg)
```

#### Note on serde_json in fdemon-core

`serde_json` cannot be removed from fdemon-core's dependencies because `DaemonMessage::Response` and `DaemonMessage::UnknownEvent` variants contain `serde_json::Value` fields. This is acceptable -- the type definition references Value, but the parsing/deserialization logic moves to daemon.

### Acceptance Criteria

1. `DaemonMessage::parse()` no longer exists as an inherent method in `fdemon-core/src/events.rs`
2. `parse_daemon_message()` free function exists in `fdemon-daemon/src/protocol.rs`
3. `LogEntryInfo` is defined in fdemon-daemon, not fdemon-core
4. All 3 production call sites updated and compiling
5. All ~40 test call sites updated and passing
6. `RawMessage` is used as intermediary (no duplicate JSON destructuring)
7. Pure methods (`app_id`, `is_error`, `summary`) remain in fdemon-core
8. `cargo test --workspace --lib` passes with 0 failures
9. `cargo clippy --workspace --lib -- -D warnings` passes
10. `docs/ARCHITECTURE.md` updated to reflect new function locations

### Testing

Existing protocol tests in `fdemon-daemon/src/protocol.rs` (~30 tests) validate parsing correctness. Update their call sites from `DaemonMessage::parse()` to `parse_daemon_message()`. No new test logic needed -- the behavior is identical, only the API shape changes.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_daemon_connected() {
        let json = r#"[{"event":"daemon.connected","params":{"version":"1.0"}}]"#;
        let msg = parse_daemon_message(json);
        assert!(matches!(msg, Some(DaemonMessage::Event { .. })));
    }
}
```

### Notes

- The stale comment "For now, we'll parse directly using serde_json" at events.rs:240 should be removed
- The `pub use fdemon_core::DaemonMessage` re-export in `fdemon-daemon/src/protocol.rs:7` can remain for convenience
- This task enables task 06 (standardize imports) which should follow after

---

## Completion Summary

**Status:** Not Started
