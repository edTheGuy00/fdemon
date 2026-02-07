## Task: Define EngineEvent Enum for Event Broadcasting

**Objective**: Create the `EngineEvent` enum in `app/engine_event.rs` that wraps domain-level events for external consumers. This is the extension point that pro features (MCP server, remote SSH) will subscribe to via `Engine.subscribe()`. The enum must capture meaningful state changes without exposing internal `Message` or `AppState` details.

**Depends on**: None (Phase 1 complete)

**Estimated Time**: 2-3 hours

### Scope

- `src/app/engine_event.rs`: **NEW** -- Define `EngineEvent` enum and conversion helpers
- `src/app/mod.rs`: Add `pub mod engine_event;` declaration

### Details

#### Design Rationale

The internal `Message` enum has ~100+ variants, many of which are UI-specific (scroll, key events, dialog navigation). External consumers need a curated set of domain events. `EngineEvent` is that curated set -- it represents "things that happened" rather than "things to do."

#### EngineEvent Enum

```rust
// src/app/engine_event.rs

use crate::app::session::SessionId;
use crate::core::{AppPhase, LogEntry, LogLevel};
use crate::daemon::Device;

/// Domain events emitted by the Engine for external consumers.
///
/// This is the primary extension point for pro features. An MCP server
/// or remote SSH client subscribes to these events via `Engine::subscribe()`.
///
/// Events are broadcast after each message processing cycle, so subscribers
/// see a consistent view of state changes.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    // ─────────────────────────────────────────────────────────
    // Session Lifecycle
    // ─────────────────────────────────────────────────────────

    /// A new session was created (device selected, not yet running)
    SessionCreated {
        session_id: SessionId,
        device: Device,
    },

    /// A session's Flutter process has started
    SessionStarted {
        session_id: SessionId,
        device_id: String,
        device_name: String,
        platform: String,
        pid: Option<u32>,
    },

    /// A session has stopped (process exited or was killed)
    SessionStopped {
        session_id: SessionId,
        reason: Option<String>,
    },

    /// A session was removed from the session manager
    SessionRemoved {
        session_id: SessionId,
    },

    // ─────────────────────────────────────────────────────────
    // App Phase Changes
    // ─────────────────────────────────────────────────────────

    /// The app phase changed for a session
    PhaseChanged {
        session_id: SessionId,
        old_phase: AppPhase,
        new_phase: AppPhase,
    },

    // ─────────────────────────────────────────────────────────
    // Hot Reload / Restart
    // ─────────────────────────────────────────────────────────

    /// Hot reload started for a session
    ReloadStarted {
        session_id: SessionId,
    },

    /// Hot reload completed successfully
    ReloadCompleted {
        session_id: SessionId,
        time_ms: u64,
    },

    /// Hot reload failed
    ReloadFailed {
        session_id: SessionId,
        reason: String,
    },

    /// Hot restart started for a session
    RestartStarted {
        session_id: SessionId,
    },

    /// Hot restart completed
    RestartCompleted {
        session_id: SessionId,
    },

    // ─────────────────────────────────────────────────────────
    // Logging
    // ─────────────────────────────────────────────────────────

    /// A new log entry was added to a session
    LogEntry {
        session_id: SessionId,
        entry: LogEntry,
    },

    /// Batch of log entries (for high-volume logging)
    LogBatch {
        session_id: SessionId,
        entries: Vec<LogEntry>,
    },

    // ─────────────────────────────────────────────────────────
    // Device Discovery
    // ─────────────────────────────────────────────────────────

    /// Connected devices list was updated
    DevicesDiscovered {
        devices: Vec<Device>,
    },

    // ─────────────────────────────────────────────────────────
    // File Watcher
    // ─────────────────────────────────────────────────────────

    /// Files changed (auto-reload may have been triggered)
    FilesChanged {
        count: usize,
        auto_reload_triggered: bool,
    },

    // ─────────────────────────────────────────────────────────
    // Engine Lifecycle
    // ─────────────────────────────────────────────────────────

    /// Engine is shutting down
    Shutdown,
}

impl EngineEvent {
    /// Returns a short string label for this event type (for logging/debugging).
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::SessionCreated { .. } => "session_created",
            Self::SessionStarted { .. } => "session_started",
            Self::SessionStopped { .. } => "session_stopped",
            Self::SessionRemoved { .. } => "session_removed",
            Self::PhaseChanged { .. } => "phase_changed",
            Self::ReloadStarted { .. } => "reload_started",
            Self::ReloadCompleted { .. } => "reload_completed",
            Self::ReloadFailed { .. } => "reload_failed",
            Self::RestartStarted { .. } => "restart_started",
            Self::RestartCompleted { .. } => "restart_completed",
            Self::LogEntry { .. } => "log_entry",
            Self::LogBatch { .. } => "log_batch",
            Self::DevicesDiscovered { .. } => "devices_discovered",
            Self::FilesChanged { .. } => "files_changed",
            Self::Shutdown => "shutdown",
        }
    }
}
```

#### Why Not Reuse HeadlessEvent?

The existing `HeadlessEvent` in `headless/mod.rs` is serialization-focused (has `serde::Serialize`, emits NDJSON to stdout). `EngineEvent` is a Rust-native enum designed for in-process subscribers. The headless runner will convert `EngineEvent` -> `HeadlessEvent` for NDJSON emission in Task 04.

#### What This Task Does NOT Do

- Does NOT add the `broadcast::channel` to Engine (that's Task 06)
- Does NOT emit events from `process_message()` (that's Task 06)
- Does NOT modify the headless runner (that's Task 04)
- Just defines the data types

### Acceptance Criteria

1. `src/app/engine_event.rs` exists with `EngineEvent` enum
2. `EngineEvent` has all variants listed above (session lifecycle, phase, reload, logs, devices, watcher, shutdown)
3. `EngineEvent` derives `Debug` and `Clone` (required for `broadcast::Sender`)
4. `event_type()` method returns a descriptive string for each variant
5. `EngineEvent` does NOT depend on ratatui, crossterm, or TUI types
6. `EngineEvent` does NOT depend on serde (serialization is headless-specific)
7. `src/app/mod.rs` declares `pub mod engine_event;`
8. `cargo build` succeeds
9. `cargo test` passes
10. `cargo clippy` is clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_event_type_labels() {
        let event = EngineEvent::Shutdown;
        assert_eq!(event.event_type(), "shutdown");

        let event = EngineEvent::ReloadStarted { session_id: 1 };
        assert_eq!(event.event_type(), "reload_started");
    }

    #[test]
    fn test_engine_event_clone() {
        let event = EngineEvent::SessionStarted {
            session_id: 1,
            device_id: "device-1".to_string(),
            device_name: "Pixel 6".to_string(),
            platform: "android".to_string(),
            pid: Some(12345),
        };
        let cloned = event.clone();
        assert_eq!(cloned.event_type(), "session_started");
    }
}
```

### Notes

- `EngineEvent` must implement `Clone` because `broadcast::Sender::send()` requires `T: Clone`
- The `LogEntry` type from `core::types` already implements `Clone`, so `EngineEvent::LogEntry` works
- The `Device` type from `daemon::devices` already implements `Clone`
- Keep this enum minimal -- start with the events that map naturally to the headless runner's `HeadlessEvent` variants, since we need a 1:1 mapping for Task 04
- Future pro features may need additional event variants (e.g., `DevToolsConnected`, `VMServiceEvent`), but those should be added when needed

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/engine_event.rs` | Created new file with EngineEvent enum, event_type() method, and comprehensive tests |
| `src/app/mod.rs` | Added `pub mod engine_event;` declaration |

### Notable Decisions/Tradeoffs

1. **Removed LogLevel import**: The initial implementation included `LogLevel` in the imports but it wasn't used in the EngineEvent definition. Removed it to keep imports clean.
2. **Comprehensive test coverage**: Added 8 tests covering all aspects: type labels, cloning, Debug trait, and testing with actual Device and LogEntry instances to ensure all types implement Clone correctly.
3. **No serde dependency**: As per the design, EngineEvent does NOT derive Serialize/Deserialize. This is intentional - serialization is handled by converting EngineEvent -> HeadlessEvent in Task 04.

### Testing Performed

- `cargo check` - Passed (no warnings)
- `cargo test --lib engine_event` - Passed (8/8 tests)
- `cargo clippy --lib -- -D warnings` - Passed (no warnings)

All acceptance criteria met:
1. ✅ `src/app/engine_event.rs` exists with EngineEvent enum
2. ✅ EngineEvent has all 15 variants (session lifecycle, phase, reload, logs, devices, watcher, shutdown)
3. ✅ EngineEvent derives Debug and Clone
4. ✅ event_type() method returns descriptive string for each variant
5. ✅ EngineEvent does NOT depend on ratatui, crossterm, or TUI types
6. ✅ EngineEvent does NOT depend on serde
7. ✅ `src/app/mod.rs` declares `pub mod engine_event;`
8. ✅ `cargo build` succeeds
9. ✅ `cargo test` passes (all engine_event tests pass)
10. ✅ `cargo clippy` is clean

### Risks/Limitations

1. **No risks identified**: The implementation is straightforward and follows the existing patterns in the codebase. All dependencies (SessionId, AppPhase, LogEntry, Device) already implement Clone as required.
