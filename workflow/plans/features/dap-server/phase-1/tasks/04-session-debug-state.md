## Task: Add Per-Session Debug State

**Objective**: Create a `DebugState` struct to track per-session debugging state (pause status, breakpoints, exception mode). Add it to `Session` alongside the existing `PerformanceState` and `NetworkState`.

**Depends on**: 01-debug-types (uses `ExceptionPauseMode`, `Breakpoint`, `IsolateRef`)

### Scope

- `crates/fdemon-app/src/session/debug_state.rs` — **NEW FILE**: `DebugState` struct and methods
- `crates/fdemon-app/src/session/session.rs` — Add `pub debug: DebugState` field to `Session`
- `crates/fdemon-app/src/session/handle.rs` — Add `debug_shutdown_tx` and `debug_task_handle` optional fields
- `crates/fdemon-app/src/session/mod.rs` — Add `pub mod debug_state;` and re-exports

### Details

#### 1. `DebugState` struct

Create `debug_state.rs` following the pattern of `session/performance.rs` (lines 36-71) and `session/network.rs` (lines 33-66):

```rust
use std::collections::HashMap;
use fdemon_daemon::vm_service::debugger_types::{ExceptionPauseMode, IsolateRef};

/// Reason why the debugger is paused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseReason {
    /// Hit a breakpoint.
    Breakpoint,
    /// An exception was thrown.
    Exception,
    /// Completed a single-step operation.
    Step,
    /// Manual pause requested.
    Interrupted,
    /// Paused at isolate start (before main).
    Entry,
    /// Paused at isolate exit.
    Exit,
    /// Paused after a service request (e.g., hot reload while paused).
    PostRequest,
}

/// A breakpoint tracked by the debug adapter.
/// Maps a DAP-assigned ID to a VM Service breakpoint ID for lifecycle management.
#[derive(Debug, Clone)]
pub struct TrackedBreakpoint {
    /// DAP-assigned breakpoint ID (monotonic integer, sent to IDE).
    pub dap_id: i64,
    /// VM Service breakpoint ID (e.g., "breakpoints/1").
    pub vm_id: String,
    /// Source URI where the breakpoint is set (e.g., "package:app/main.dart").
    pub uri: String,
    /// Line number in the source file.
    pub line: i32,
    /// Column number (if specified).
    pub column: Option<i32>,
    /// Whether the VM has resolved this breakpoint to a concrete location.
    pub verified: bool,
}

/// Per-session debug state tracking.
///
/// Each Flutter session has its own `DebugState` that tracks whether the
/// debugger is paused, which breakpoints are set, and the exception mode.
/// This state is updated by debug stream events and queried by the DAP adapter.
#[derive(Debug, Clone)]
pub struct DebugState {
    /// Whether the session's main isolate is currently paused.
    pub paused: bool,
    /// Why the debugger is paused (None when running).
    pub pause_reason: Option<PauseReason>,
    /// The isolate that is currently paused (None when running).
    pub paused_isolate_id: Option<String>,
    /// Tracked breakpoints, keyed by source URI.
    /// Each URI maps to the list of breakpoints set in that file.
    pub breakpoints: HashMap<String, Vec<TrackedBreakpoint>>,
    /// Current exception pause mode.
    pub exception_mode: ExceptionPauseMode,
    /// Known isolates in this session.
    pub isolates: Vec<IsolateRef>,
    /// Whether a DAP client is actively debugging this session.
    pub dap_attached: bool,
    /// Next DAP breakpoint ID to assign (monotonic counter).
    next_dap_bp_id: i64,
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
            paused: false,
            pause_reason: None,
            paused_isolate_id: None,
            breakpoints: HashMap::new(),
            exception_mode: ExceptionPauseMode::Unhandled,
            isolates: Vec::new(),
            dap_attached: false,
            next_dap_bp_id: 1,
        }
    }
}
```

#### 2. `DebugState` methods

```rust
impl DebugState {
    /// Allocates the next DAP breakpoint ID.
    pub fn next_breakpoint_id(&mut self) -> i64 {
        let id = self.next_dap_bp_id;
        self.next_dap_bp_id += 1;
        id
    }

    /// Tracks a new breakpoint.
    pub fn track_breakpoint(&mut self, bp: TrackedBreakpoint) {
        self.breakpoints
            .entry(bp.uri.clone())
            .or_default()
            .push(bp);
    }

    /// Removes a tracked breakpoint by VM Service ID.
    /// Returns the removed breakpoint, if found.
    pub fn untrack_breakpoint(&mut self, vm_id: &str) -> Option<TrackedBreakpoint> {
        for bps in self.breakpoints.values_mut() {
            if let Some(pos) = bps.iter().position(|bp| bp.vm_id == vm_id) {
                return Some(bps.remove(pos));
            }
        }
        None
    }

    /// Marks a breakpoint as verified (resolved by the VM).
    pub fn mark_breakpoint_verified(&mut self, vm_id: &str) {
        for bps in self.breakpoints.values_mut() {
            if let Some(bp) = bps.iter_mut().find(|bp| bp.vm_id == vm_id) {
                bp.verified = true;
            }
        }
    }

    /// Gets all breakpoints for a given source URI.
    pub fn breakpoints_for_uri(&self, uri: &str) -> &[TrackedBreakpoint] {
        self.breakpoints.get(uri).map_or(&[], |v| v.as_slice())
    }

    /// Gets all tracked breakpoints across all URIs.
    pub fn all_breakpoints(&self) -> impl Iterator<Item = &TrackedBreakpoint> {
        self.breakpoints.values().flat_map(|v| v.iter())
    }

    /// Marks the session as paused with the given reason and isolate.
    pub fn mark_paused(&mut self, reason: PauseReason, isolate_id: String) {
        self.paused = true;
        self.pause_reason = Some(reason);
        self.paused_isolate_id = Some(isolate_id);
    }

    /// Marks the session as resumed.
    pub fn mark_resumed(&mut self) {
        self.paused = false;
        self.pause_reason = None;
        self.paused_isolate_id = None;
    }

    /// Adds a known isolate to this session.
    pub fn add_isolate(&mut self, isolate: IsolateRef) {
        if !self.isolates.iter().any(|i| i.id == isolate.id) {
            self.isolates.push(isolate);
        }
    }

    /// Removes a known isolate from this session.
    pub fn remove_isolate(&mut self, isolate_id: &str) {
        self.isolates.retain(|i| i.id != isolate_id);
    }

    /// Clears all breakpoints (used on hot restart when breakpoints need to be re-applied).
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Resets state for a hot restart: clears pause state but preserves breakpoint configs.
    /// Breakpoint `verified` flags are reset since the VM invalidates breakpoints on restart.
    pub fn reset_for_hot_restart(&mut self) {
        self.paused = false;
        self.pause_reason = None;
        self.paused_isolate_id = None;
        self.isolates.clear();
        for bps in self.breakpoints.values_mut() {
            for bp in bps.iter_mut() {
                bp.verified = false;
            }
        }
    }
}
```

#### 3. Add `debug` field to `Session`

In `crates/fdemon-app/src/session/session.rs`, add:

```rust
use crate::session::debug_state::DebugState;

// In Session struct (alongside performance and network fields at ~line 125):
pub debug: DebugState,
```

In `Session::new()`, initialize:
```rust
debug: DebugState::default(),
```

#### 4. Add handle fields to `SessionHandle`

In `crates/fdemon-app/src/session/handle.rs`, add optional fields following the existing pattern (e.g., `performance_shutdown_tx`, `performance_task_handle`):

```rust
/// Shutdown signal for the debug event monitoring task.
pub debug_shutdown_tx: Option<Arc<tokio::sync::watch::Sender<bool>>>,
/// Handle for the debug event monitoring task.
pub debug_task_handle: Option<tokio::task::JoinHandle<()>>,
```

Initialize both to `None` in `SessionHandle::new()`.

#### 5. Module registration

In `crates/fdemon-app/src/session/mod.rs`:
```rust
pub mod debug_state;
pub use debug_state::{DebugState, PauseReason, TrackedBreakpoint};
```

### Acceptance Criteria

1. `DebugState::default()` creates a valid initial state (not paused, no breakpoints, Unhandled exception mode)
2. `track_breakpoint()` / `untrack_breakpoint()` correctly add/remove breakpoints
3. `mark_breakpoint_verified()` updates the correct breakpoint's `verified` flag
4. `breakpoints_for_uri()` returns correct breakpoints for a given URI
5. `mark_paused()` / `mark_resumed()` toggle pause state correctly
6. `add_isolate()` / `remove_isolate()` manage the isolate list without duplicates
7. `reset_for_hot_restart()` preserves breakpoint configs but resets verified flags and pause state
8. `next_breakpoint_id()` returns monotonically increasing IDs
9. `Session` has a `debug: DebugState` field initialized to default
10. `SessionHandle` has `debug_shutdown_tx` and `debug_task_handle` fields initialized to `None`
11. All new code compiles: `cargo check -p fdemon-app`
12. All existing tests pass: `cargo test -p fdemon-app`

### Testing

Comprehensive unit tests for all `DebugState` methods:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = DebugState::default();
        assert!(!state.paused);
        assert!(state.pause_reason.is_none());
        assert!(state.breakpoints.is_empty());
        assert_eq!(state.exception_mode, ExceptionPauseMode::Unhandled);
        assert!(!state.dap_attached);
    }

    #[test]
    fn test_track_and_untrack_breakpoint() {
        let mut state = DebugState::default();
        let bp = TrackedBreakpoint {
            dap_id: 1,
            vm_id: "breakpoints/1".to_string(),
            uri: "package:app/main.dart".to_string(),
            line: 42,
            column: None,
            verified: false,
        };
        state.track_breakpoint(bp);
        assert_eq!(state.breakpoints_for_uri("package:app/main.dart").len(), 1);

        let removed = state.untrack_breakpoint("breakpoints/1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().dap_id, 1);
        assert!(state.breakpoints_for_uri("package:app/main.dart").is_empty());
    }

    #[test]
    fn test_mark_breakpoint_verified() {
        let mut state = DebugState::default();
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 1,
            vm_id: "breakpoints/1".to_string(),
            uri: "package:app/main.dart".to_string(),
            line: 42,
            column: None,
            verified: false,
        });
        state.mark_breakpoint_verified("breakpoints/1");
        assert!(state.breakpoints_for_uri("package:app/main.dart")[0].verified);
    }

    #[test]
    fn test_pause_resume_cycle() {
        let mut state = DebugState::default();
        assert!(!state.paused);

        state.mark_paused(PauseReason::Breakpoint, "isolates/1".to_string());
        assert!(state.paused);
        assert_eq!(state.pause_reason, Some(PauseReason::Breakpoint));
        assert_eq!(state.paused_isolate_id.as_deref(), Some("isolates/1"));

        state.mark_resumed();
        assert!(!state.paused);
        assert!(state.pause_reason.is_none());
        assert!(state.paused_isolate_id.is_none());
    }

    #[test]
    fn test_isolate_management() {
        let mut state = DebugState::default();
        let isolate = IsolateRef { id: "isolates/1".to_string(), name: Some("main".to_string()) };

        state.add_isolate(isolate.clone());
        assert_eq!(state.isolates.len(), 1);

        // Duplicate add is idempotent
        state.add_isolate(isolate);
        assert_eq!(state.isolates.len(), 1);

        state.remove_isolate("isolates/1");
        assert!(state.isolates.is_empty());
    }

    #[test]
    fn test_reset_for_hot_restart() {
        let mut state = DebugState::default();
        state.mark_paused(PauseReason::Breakpoint, "isolates/1".to_string());
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 1,
            vm_id: "breakpoints/1".to_string(),
            uri: "package:app/main.dart".to_string(),
            line: 42,
            column: None,
            verified: true,
        });
        state.add_isolate(IsolateRef { id: "isolates/1".to_string(), name: Some("main".to_string()) });

        state.reset_for_hot_restart();

        // Pause state cleared
        assert!(!state.paused);
        assert!(state.pause_reason.is_none());
        // Isolates cleared (new ones will arrive after restart)
        assert!(state.isolates.is_empty());
        // Breakpoints preserved but unverified
        assert_eq!(state.breakpoints_for_uri("package:app/main.dart").len(), 1);
        assert!(!state.breakpoints_for_uri("package:app/main.dart")[0].verified);
    }

    #[test]
    fn test_next_breakpoint_id_monotonic() {
        let mut state = DebugState::default();
        assert_eq!(state.next_breakpoint_id(), 1);
        assert_eq!(state.next_breakpoint_id(), 2);
        assert_eq!(state.next_breakpoint_id(), 3);
    }

    #[test]
    fn test_all_breakpoints_iterator() {
        let mut state = DebugState::default();
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 1, vm_id: "bp/1".into(), uri: "a.dart".into(), line: 1, column: None, verified: true,
        });
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 2, vm_id: "bp/2".into(), uri: "b.dart".into(), line: 2, column: None, verified: false,
        });
        let all: Vec<_> = state.all_breakpoints().collect();
        assert_eq!(all.len(), 2);
    }
}
```

### Notes

- `DebugState` derives `Clone` so it can be used in the `Session` snapshot pattern (TEA state is cloneable for rendering).
- `TrackedBreakpoint` maps the dual-ID world: DAP assigns monotonic integer IDs that IDEs reference, while VM Service uses string IDs like `"breakpoints/1"`. Both must be tracked for proper lifecycle management.
- `reset_for_hot_restart()` is critical for Phase 4: after hot restart, the VM invalidates all breakpoint IDs. The DAP adapter will call this, then re-apply breakpoints from the preserved configs.
- `dap_attached` flag is set by the DAP adapter when a client connects. Used by the handler layer to decide whether debug events should be processed or ignored.
- The `debug_shutdown_tx` / `debug_task_handle` fields on `SessionHandle` are not used in Phase 1 but are added now to avoid touching the struct again in Phase 2 when the DAP server needs to manage per-session debug tasks.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/debug_state.rs` | NEW FILE: `DebugState`, `PauseReason`, `TrackedBreakpoint` structs with all methods and 22 unit tests |
| `crates/fdemon-app/src/session/session.rs` | Added `use super::debug_state::DebugState;`, `pub debug: DebugState` field to `Session`, and `debug: DebugState::default()` in `Session::new()` |
| `crates/fdemon-app/src/session/handle.rs` | Added `debug_shutdown_tx: Option<Arc<watch::Sender<bool>>>` and `debug_task_handle: Option<JoinHandle<()>>` fields to `SessionHandle`; updated `Debug` impl and `new()` to include them |
| `crates/fdemon-app/src/session/mod.rs` | Added `pub mod debug_state;` and `pub use debug_state::{DebugState, PauseReason, TrackedBreakpoint};` |

### Notable Decisions/Tradeoffs

1. **Import path for `IsolateRef`**: Imported directly from `fdemon_daemon::vm_service::debugger_types::IsolateRef` as specified in the task note. This avoids the re-export alias `DebugIsolateRef` at the vm_service level, which would be less clear at the usage site.

2. **`pub mod debug_state`**: Made the module fully `pub` (not `pub(crate)`) following the same visibility as the public API surface, since the task re-exports all three types.

3. **Test coverage**: Added 22 unit tests covering all methods, including edge cases (unknown IDs, duplicate isolate add, empty iterators, all PauseReason variants, hot-restart state preservation invariants).

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1197 + 4 ignored; 22 new debug_state tests all pass)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Applied (minor whitespace reformatting by rustfmt)

### Risks/Limitations

1. **Phase 2 scaffolding only**: `debug_shutdown_tx` and `debug_task_handle` on `SessionHandle` are `None` at initialization and remain unused until Phase 2 spawns the per-session debug event task. No runtime risk.
2. **`breakpoints` map retains empty `Vec` entries**: After `untrack_breakpoint` removes the last breakpoint for a URI, an empty `Vec` remains in the map. This is benign — `breakpoints_for_uri` returns an empty slice correctly — but callers iterating `all_breakpoints()` see no phantom entries since `flat_map` skips empty vecs.
