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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/protocol.rs` | Added free functions `parse_daemon_message()`, `to_log_entry()`, `parse_flutter_log()`, `detect_log_level()`, helper functions `parse_event()` and `unknown_event()`, and moved `LogEntryInfo` struct from core. Updated ~30 test call sites to use new free functions. Uses `RawMessage` as intermediary to avoid duplicate JSON parsing. |
| `crates/fdemon-core/src/events.rs` | Removed ~306 lines of parsing methods (`parse()`, `parse_event()`, `unknown()`, `to_log_entry()`, `parse_flutter_log()`, `detect_log_level()`) and `LogEntryInfo` struct from `impl DaemonMessage`. Kept pure data accessor methods (`app_id()`, `is_error()`, `summary()`). |
| `crates/fdemon-core/src/lib.rs` | Removed `LogEntryInfo` from re-exports. |
| `crates/fdemon-daemon/src/lib.rs` | Added re-exports for `parse_daemon_message`, `to_log_entry`, `parse_flutter_log`, `detect_log_level`, and `LogEntryInfo`. |
| `crates/fdemon-app/src/handler/session.rs` | Updated imports and call sites: `DaemonMessage::parse()` → `parse_daemon_message()`, `msg.to_log_entry()` → `to_log_entry(&msg)`. |
| `crates/fdemon-app/src/handler/daemon.rs` | Updated call site: `msg.to_log_entry()` → `fdemon_daemon::to_log_entry(&msg)`. |
| `crates/fdemon-app/src/process.rs` | Updated imports and call sites for `parse_daemon_message()` and `strip_brackets()`. |
| `crates/fdemon-app/src/actions.rs` | Updated call sites to use `fdemon_daemon::parse_daemon_message()` and `fdemon_daemon::strip_brackets()`. Removed unused `protocol` import. |
| `tests/e2e.rs` | Updated `load_daemon_message()` helper to use `parse_daemon_message()`. |
| `tests/e2e/hot_reload.rs` | Updated call site from `DaemonMessage::parse()` to `fdemon_daemon::parse_daemon_message()`. |
| `tests/e2e/daemon_interaction.rs` | Updated call site from `DaemonMessage::parse()` to `fdemon_daemon::parse_daemon_message()`. |
| `tests/fixture_parsing_test.rs` | Updated imports and all 8 call sites to use `parse_daemon_message()`. |
| `docs/ARCHITECTURE.md` | Updated 3 references from `DaemonMessage::parse()` to `parse_daemon_message()` and documented new function locations. |

### Notable Decisions/Tradeoffs

1. **Free Functions Instead of Inherent Methods**: Used free functions (`parse_daemon_message()`, `to_log_entry()`, etc.) instead of inherent methods to work around Rust's orphan rule, which prevents implementing methods on types defined in another crate.

2. **RawMessage as Intermediary**: Leveraged the existing `RawMessage::parse()` function in `protocol.rs` to avoid duplicating JSON destructuring logic. This consolidation makes the code more maintainable and reduces potential bugs.

3. **Kept Pure Accessor Methods in Core**: Left `app_id()`, `is_error()`, and `summary()` methods on `DaemonMessage` in fdemon-core since these are pure data accessors with no infrastructure dependencies, aligning with the architectural goal of keeping domain logic in core.

4. **Minimal Import Changes**: Most call sites only required updating from method syntax (`msg.to_log_entry()`) to function syntax (`to_log_entry(&msg)`), minimizing disruption to existing code.

### Testing Performed

- `cargo test --workspace --lib` - Passed (1,532 unit tests across all crates)
- `cargo clippy --workspace --lib -- -D warnings` - Passed (0 warnings)

All test call sites updated successfully:
- ~30 tests in `crates/fdemon-daemon/src/protocol.rs`
- 8 tests in `tests/fixture_parsing_test.rs`
- 1 test in `tests/e2e.rs`
- 1 test in `tests/e2e/hot_reload.rs`
- 1 test in `tests/e2e/daemon_interaction.rs`

### Risks/Limitations

1. **API Breaking Change**: This is a breaking change for any external consumers using `DaemonMessage::parse()`. However, since the crates are not yet published, this is acceptable. The new API is more consistent with Rust idioms for cross-crate functionality.

2. **serde_json in Core**: `serde_json` remains in fdemon-core's dependencies because `DaemonMessage::Response` and `DaemonMessage::UnknownEvent` variants contain `serde_json::Value` fields. This is acceptable - the type definition references Value, but the parsing/deserialization logic now lives in daemon as intended.
