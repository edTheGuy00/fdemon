## Task: Move DaemonMessage and Event Structs from daemon/ to core/

**Objective**: Eliminate the `core/ -> daemon/` dependency violation by moving `DaemonMessage` and its nine event structs into `core/events.rs`, so `core/` is a true leaf module with no internal dependencies.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `src/daemon/events.rs`: Move all 9 event structs to `core/events.rs`
- `src/daemon/protocol.rs`: Split `DaemonMessage` -- move enum + pure methods to `core/events.rs`, keep parsing in `daemon/`
- `src/core/events.rs`: Receive all moved types, remove `use crate::daemon::DaemonMessage`
- `src/core/mod.rs`: Update re-exports
- `src/daemon/mod.rs`: Update re-exports, add re-exports from `core/` for backward compat
- 7 consumer files: Update imports

### Details

#### Step 1: Move the 9 event structs from `daemon/events.rs` to `core/events.rs`

These structs are defined in `src/daemon/events.rs` (lines 1-101). They are all pure data with only `serde` derives:

```rust
// Move ALL of these from daemon/events.rs to core/events.rs:
pub struct DaemonConnected { pub version: String, pub pid: i64 }
pub struct DaemonLogMessage { pub level: String, pub message: String }
pub struct AppStart { pub device_id: String, pub app_id: String, ... }
pub struct AppStarted { pub app_id: String }
pub struct AppLog { pub app_id: String, pub log: String, pub error: bool }
pub struct AppProgress { pub app_id: String, pub id: String, pub message: Option<String>, ... }
pub struct AppStop { pub app_id: String }
pub struct AppDebugPort { pub app_id: String, pub port: i64, pub ws_uri: String, ... }
pub struct DeviceInfo { pub id: String, pub name: String, pub platform: String, ... }
```

All derive `Debug, Clone, Deserialize, Serialize`. None import anything from `daemon/`.

`DeviceInfo` also has an `impl` block with a `display_name()` method (pure logic).

#### Step 2: Move `DaemonMessage` enum definition to `core/events.rs`

Move from `src/daemon/protocol.rs:80-110`:

```rust
#[derive(Debug, Clone)]
pub enum DaemonMessage {
    DaemonConnected(DaemonConnected),
    DaemonLogMessage(DaemonLogMessage),
    AppStart(AppStart),
    AppStarted(AppStarted),
    AppStop(AppStop),
    AppLog(AppLog),
    AppProgress(AppProgress),
    AppDebugPort(AppDebugPort),
    DeviceAdded(DeviceInfo),
    DeviceRemoved(DeviceInfo),
    Response { id: serde_json::Value, result: Option<serde_json::Value>, error: Option<serde_json::Value> },
    UnknownEvent { event: String, params: serde_json::Value },
}
```

**Note**: The `Response` and `UnknownEvent` variants use `serde_json::Value`. The `core/` module already transitively depends on `serde` -- verify `serde_json` is accessible (it is in `Cargo.toml` as a direct dependency).

#### Step 3: Move pure methods to `core/events.rs`

These `impl DaemonMessage` methods do NOT depend on any `daemon/`-specific types:

```rust
// Move these from daemon/protocol.rs to core/events.rs:
impl DaemonMessage {
    pub fn app_id(&self) -> Option<&str>       // line 174
    pub fn is_error(&self) -> bool             // line 187
    pub fn summary(&self) -> String            // line 197
}
```

#### Step 4: Keep parsing methods in `daemon/protocol.rs`

These methods depend on `RawMessage` (daemon-internal JSON-RPC deserialization) and must stay:

```rust
// Keep in daemon/protocol.rs as impl DaemonMessage (Rust allows split impls):
impl DaemonMessage {
    pub fn parse(json: &str) -> Option<Self>                    // line 114
    fn from_raw(raw: RawMessage) -> Self                        // line 120
    fn parse_event(event: &str, params: Value) -> Self          // line 130
    fn unknown(event: &str, params: Value) -> Self              // line 166
    pub fn to_log_entry(&self) -> Option<LogEntryInfo>          // line 244
    pub fn parse_flutter_log(raw: &str, is_error: bool) -> (LogLevel, String)  // line 341
    pub fn detect_log_level(message: &str) -> LogLevel          // line 377
}
```

`daemon/protocol.rs` will now `use crate::core::events::{DaemonMessage, ...}` and add its impl block.

Also keep `RawMessage`, `LogEntryInfo`, and `strip_brackets` in `daemon/protocol.rs`.

#### Step 5: Update `core/events.rs` -- remove the violation

Before:
```rust
// src/core/events.rs:3
use crate::daemon::DaemonMessage;  // VIOLATION
```

After: `DaemonMessage` is defined in this same file. Remove the import.

#### Step 6: Update `core/mod.rs` re-exports

```rust
// src/core/mod.rs -- add re-exports for the new types
pub use events::{
    DaemonEvent, DaemonMessage,
    DaemonConnected, DaemonLogMessage,
    AppStart, AppStarted, AppStop, AppLog, AppProgress, AppDebugPort,
    DeviceInfo,
};
```

#### Step 7: Update `daemon/mod.rs` re-exports

Add backward-compat re-exports so consumers don't all need updating immediately:

```rust
// src/daemon/mod.rs -- re-export from core for backward compatibility
pub use crate::core::{
    DaemonMessage,
    DaemonConnected, DaemonLogMessage,
    AppStart, AppStarted, AppStop, AppLog, AppProgress, AppDebugPort,
    DeviceInfo,
};
```

This means existing `use crate::daemon::DaemonMessage` still compiles. However, the authoritative definitions are now in `core/`.

#### Step 8: Update consumer imports (can be done gradually via re-exports)

These files import `DaemonMessage` from `daemon/`:

| File | Line | Current Import | New Import |
|------|------|---------------|------------|
| `src/tui/process.rs` | 18 | `use crate::daemon::{..., DaemonMessage};` | Works via re-export (or change to `crate::core::DaemonMessage`) |
| `src/tui/actions.rs` | 17 | `use crate::daemon::{..., DaemonMessage, ...};` | Works via re-export |
| `src/app/handler/session.rs` | 9 | `use crate::daemon::{protocol, DaemonMessage};` | Works via re-export |
| `src/services/state_service.rs` | 12 | `use crate::daemon::{DaemonMessage, DeviceInfo};` | Change to `crate::core::{DaemonMessage, DeviceInfo}` |
| `src/headless/runner.rs` | 19 | `use crate::daemon::{..., DaemonMessage, ...};` | Works via re-export |

The event structs (`AppLog`, `AppStart`, etc.) are imported across many files from `daemon/events`. These will all work via re-exports from `daemon/mod.rs`.

### Acceptance Criteria

1. `src/core/events.rs` no longer has `use crate::daemon::*` imports
2. `DaemonMessage` enum is defined in `src/core/events.rs`
3. All 9 event structs are defined in `src/core/events.rs`
4. `DaemonMessage::parse()`, `from_raw()`, `to_log_entry()` remain in `src/daemon/protocol.rs`
5. `DaemonMessage::app_id()`, `is_error()`, `summary()` are in `src/core/events.rs`
6. `daemon/mod.rs` re-exports all moved types for backward compatibility
7. `cargo build` succeeds
8. `cargo test` passes with no regressions
9. `cargo clippy` is clean

### Testing

```bash
cargo test            # Full test suite
cargo test --lib      # Unit tests
cargo clippy          # Lints
```

Verify no test files needed changes (the re-exports from `daemon/` should make this transparent).

### Notes

- The `serde_json::Value` usage in `DaemonMessage::Response` and `UnknownEvent` means `core/` will have a dependency on `serde_json`. This is acceptable since `serde_json` is already in `Cargo.toml` and `core/` will need it for the workspace split anyway.
- `LogEntryInfo` (defined at `daemon/protocol.rs:499`) stays in `daemon/` -- it is only used within `daemon/protocol.rs` and `tui/actions.rs`.
- The `daemon/events.rs` file can either be deleted (if all content moves) or kept as a thin re-export file. Prefer deleting and having `daemon/mod.rs` re-export from `core/`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/core/events.rs` | Added 9 event structs (DaemonConnected, DaemonLogMessage, AppStart, AppStarted, AppLog, AppProgress, AppStop, AppDebugPort, DeviceInfo), DaemonMessage enum, and pure methods (app_id, is_error, summary) from daemon layer |
| `src/core/mod.rs` | Updated re-exports to include new types from events module |
| `src/daemon/protocol.rs` | Removed DaemonMessage enum definition and pure methods, kept parsing methods (parse, from_raw, parse_event, unknown, to_log_entry, parse_flutter_log, detect_log_level); updated imports to use core types |
| `src/daemon/events.rs` | Deleted - all content moved to core/events.rs |
| `src/daemon/mod.rs` | Removed events module declaration; added re-exports from core for backward compatibility |
| `src/services/state_service.rs` | Updated imports to use core::{DaemonMessage, DeviceInfo} instead of daemon |
| `tests/fixture_parsing_test.rs` | Updated imports to use daemon re-export (which now points to core) |

### Notable Decisions/Tradeoffs

1. **Split Implementation Pattern**: DaemonMessage now has split impl blocks - pure methods in core/events.rs and parsing methods in daemon/protocol.rs. This is allowed by Rust and maintains clean layer boundaries while keeping daemon-specific parsing logic separate.

2. **Backward Compatibility via Re-exports**: daemon/mod.rs re-exports all moved types from core, allowing existing consumer code to continue using `use crate::daemon::DaemonMessage` without modification. Only services/state_service.rs was updated to import directly from core as planned.

3. **File Deletion**: daemon/events.rs was deleted entirely rather than kept as a thin re-export file, as all its content was successfully moved to core and the re-exports from daemon/mod.rs provide the necessary backward compatibility.

4. **Test Updates**: Test code in daemon/protocol.rs updated to import event structs from crate::core instead of crate::daemon::events. Integration test uses daemon re-export path which works seamlessly.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1563 tests)
- `cargo test --test fixture_parsing_test` - Passed (7 integration tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None identified**: The split implementation pattern is standard Rust practice and the re-export strategy ensures backward compatibility. All tests pass with no regressions.
