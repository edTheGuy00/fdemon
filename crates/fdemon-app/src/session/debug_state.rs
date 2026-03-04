//! Per-session debug state for the DAP adapter.
//!
//! Tracks whether the debugger is paused, which breakpoints are set,
//! the current exception mode, and known isolates. Updated by debug stream
//! events and queried by the DAP adapter.

use std::collections::HashMap;

use fdemon_daemon::vm_service::debugger_types::{ExceptionPauseMode, IsolateRef};

// ---------------------------------------------------------------------------
// PauseReason
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// TrackedBreakpoint
// ---------------------------------------------------------------------------

/// A breakpoint tracked by the debug adapter.
///
/// Maps a DAP-assigned ID to a VM Service breakpoint ID for lifecycle
/// management. The DAP adapter and the VM Service use different ID schemes:
/// DAP assigns monotonic integer IDs that IDEs reference, while VM Service
/// uses string IDs like `"breakpoints/1"`. Both must be tracked for proper
/// lifecycle management.
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

// ---------------------------------------------------------------------------
// DebugState
// ---------------------------------------------------------------------------

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

impl DebugState {
    /// Allocates the next DAP breakpoint ID.
    ///
    /// IDs are monotonically increasing integers starting from 1.
    /// Each call increments the internal counter and returns the allocated ID.
    pub fn next_breakpoint_id(&mut self) -> i64 {
        let id = self.next_dap_bp_id;
        self.next_dap_bp_id += 1;
        id
    }

    /// Tracks a new breakpoint.
    ///
    /// The breakpoint is indexed by its source URI so that all breakpoints
    /// in a file can be retrieved efficiently via [`breakpoints_for_uri`].
    pub fn track_breakpoint(&mut self, bp: TrackedBreakpoint) {
        self.breakpoints.entry(bp.uri.clone()).or_default().push(bp);
    }

    /// Removes a tracked breakpoint by VM Service ID.
    ///
    /// Returns the removed breakpoint if found, or `None` if no breakpoint
    /// with the given VM Service ID exists.
    pub fn untrack_breakpoint(&mut self, vm_id: &str) -> Option<TrackedBreakpoint> {
        for bps in self.breakpoints.values_mut() {
            if let Some(pos) = bps.iter().position(|bp| bp.vm_id == vm_id) {
                return Some(bps.remove(pos));
            }
        }
        None
    }

    /// Marks a breakpoint as verified (resolved by the VM).
    ///
    /// Called when the VM Service emits a `BreakpointResolved` event.
    /// If no breakpoint with the given VM Service ID exists, this is a no-op.
    pub fn mark_breakpoint_verified(&mut self, vm_id: &str) {
        for bps in self.breakpoints.values_mut() {
            if let Some(bp) = bps.iter_mut().find(|bp| bp.vm_id == vm_id) {
                bp.verified = true;
            }
        }
    }

    /// Gets all breakpoints for a given source URI.
    ///
    /// Returns an empty slice if no breakpoints are set for the URI.
    pub fn breakpoints_for_uri(&self, uri: &str) -> &[TrackedBreakpoint] {
        self.breakpoints.get(uri).map_or(&[], |v| v.as_slice())
    }

    /// Gets all tracked breakpoints across all URIs.
    pub fn all_breakpoints(&self) -> impl Iterator<Item = &TrackedBreakpoint> {
        self.breakpoints.values().flat_map(|v| v.iter())
    }

    /// Marks the session as paused with the given reason and isolate.
    ///
    /// Called when a `Pause*` debug event is received from the VM Service.
    pub fn mark_paused(&mut self, reason: PauseReason, isolate_id: String) {
        self.paused = true;
        self.pause_reason = Some(reason);
        self.paused_isolate_id = Some(isolate_id);
    }

    /// Marks the session as resumed.
    ///
    /// Called when a `Resume` debug event is received from the VM Service.
    pub fn mark_resumed(&mut self) {
        self.paused = false;
        self.pause_reason = None;
        self.paused_isolate_id = None;
    }

    /// Adds a known isolate to this session.
    ///
    /// If the isolate with the same ID already exists, this is a no-op
    /// (deduplication by ID).
    pub fn add_isolate(&mut self, isolate: IsolateRef) {
        if !self.isolates.iter().any(|i| i.id == isolate.id) {
            self.isolates.push(isolate);
        }
    }

    /// Removes a known isolate from this session.
    ///
    /// Called when the VM Service emits an `IsolateExit` event.
    /// If the isolate ID is not found, this is a no-op.
    pub fn remove_isolate(&mut self, isolate_id: &str) {
        self.isolates.retain(|i| i.id != isolate_id);
    }

    /// Clears all breakpoints.
    ///
    /// Used on hot restart when breakpoints need to be re-applied after
    /// the VM discards all previous breakpoint state.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Resets state for a hot restart: clears pause state but preserves breakpoint configs.
    ///
    /// After a hot restart the VM invalidates all breakpoint IDs, so the
    /// `verified` flag is reset on every breakpoint — the DAP adapter will
    /// re-apply them and mark them verified again when the VM responds.
    /// Isolates are also cleared because new isolates will arrive as fresh
    /// `IsolateRunnable` events after the restart.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    fn test_default_paused_isolate_id_is_none() {
        let state = DebugState::default();
        assert!(state.paused_isolate_id.is_none());
    }

    #[test]
    fn test_default_isolates_is_empty() {
        let state = DebugState::default();
        assert!(state.isolates.is_empty());
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
        assert!(state
            .breakpoints_for_uri("package:app/main.dart")
            .is_empty());
    }

    #[test]
    fn test_untrack_breakpoint_returns_none_for_unknown_vm_id() {
        let mut state = DebugState::default();
        let removed = state.untrack_breakpoint("breakpoints/99");
        assert!(removed.is_none());
    }

    #[test]
    fn test_track_multiple_breakpoints_in_same_uri() {
        let mut state = DebugState::default();
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 1,
            vm_id: "bp/1".into(),
            uri: "package:app/main.dart".into(),
            line: 10,
            column: None,
            verified: false,
        });
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 2,
            vm_id: "bp/2".into(),
            uri: "package:app/main.dart".into(),
            line: 20,
            column: None,
            verified: false,
        });
        assert_eq!(state.breakpoints_for_uri("package:app/main.dart").len(), 2);
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
    fn test_mark_breakpoint_verified_noop_for_unknown_id() {
        let mut state = DebugState::default();
        // Should not panic when ID is not found
        state.mark_breakpoint_verified("breakpoints/99");
    }

    #[test]
    fn test_breakpoints_for_uri_returns_empty_slice_for_unknown_uri() {
        let state = DebugState::default();
        let bps = state.breakpoints_for_uri("package:unknown/file.dart");
        assert!(bps.is_empty());
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
    fn test_mark_paused_with_all_reasons() {
        let reasons = [
            PauseReason::Breakpoint,
            PauseReason::Exception,
            PauseReason::Step,
            PauseReason::Interrupted,
            PauseReason::Entry,
            PauseReason::Exit,
            PauseReason::PostRequest,
        ];
        for reason in reasons {
            let mut state = DebugState::default();
            state.mark_paused(reason.clone(), "isolates/1".into());
            assert!(state.paused);
            assert_eq!(state.pause_reason.as_ref(), Some(&reason));
        }
    }

    #[test]
    fn test_isolate_management() {
        let mut state = DebugState::default();
        let isolate = IsolateRef {
            id: "isolates/1".to_string(),
            name: Some("main".to_string()),
        };

        state.add_isolate(isolate.clone());
        assert_eq!(state.isolates.len(), 1);

        // Duplicate add is idempotent
        state.add_isolate(isolate);
        assert_eq!(state.isolates.len(), 1);

        state.remove_isolate("isolates/1");
        assert!(state.isolates.is_empty());
    }

    #[test]
    fn test_remove_isolate_noop_for_unknown_id() {
        let mut state = DebugState::default();
        state.add_isolate(IsolateRef {
            id: "isolates/1".into(),
            name: None,
        });
        // Removing an unknown ID must not remove the existing isolate
        state.remove_isolate("isolates/99");
        assert_eq!(state.isolates.len(), 1);
    }

    #[test]
    fn test_add_multiple_isolates() {
        let mut state = DebugState::default();
        state.add_isolate(IsolateRef {
            id: "isolates/1".into(),
            name: Some("main".into()),
        });
        state.add_isolate(IsolateRef {
            id: "isolates/2".into(),
            name: Some("background".into()),
        });
        assert_eq!(state.isolates.len(), 2);
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
        state.add_isolate(IsolateRef {
            id: "isolates/1".to_string(),
            name: Some("main".to_string()),
        });

        state.reset_for_hot_restart();

        // Pause state cleared
        assert!(!state.paused);
        assert!(state.pause_reason.is_none());
        assert!(state.paused_isolate_id.is_none());
        // Isolates cleared (new ones will arrive after restart)
        assert!(state.isolates.is_empty());
        // Breakpoints preserved but unverified
        assert_eq!(state.breakpoints_for_uri("package:app/main.dart").len(), 1);
        assert!(!state.breakpoints_for_uri("package:app/main.dart")[0].verified);
    }

    #[test]
    fn test_reset_for_hot_restart_preserves_exception_mode() {
        let mut state = DebugState::default();
        state.exception_mode = ExceptionPauseMode::All;
        state.reset_for_hot_restart();
        assert_eq!(state.exception_mode, ExceptionPauseMode::All);
    }

    #[test]
    fn test_reset_for_hot_restart_preserves_dap_attached() {
        let mut state = DebugState::default();
        state.dap_attached = true;
        state.reset_for_hot_restart();
        assert!(state.dap_attached);
    }

    #[test]
    fn test_next_breakpoint_id_monotonic() {
        let mut state = DebugState::default();
        assert_eq!(state.next_breakpoint_id(), 1);
        assert_eq!(state.next_breakpoint_id(), 2);
        assert_eq!(state.next_breakpoint_id(), 3);
    }

    #[test]
    fn test_clear_breakpoints() {
        let mut state = DebugState::default();
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 1,
            vm_id: "bp/1".into(),
            uri: "a.dart".into(),
            line: 1,
            column: None,
            verified: true,
        });
        assert!(!state.breakpoints.is_empty());
        state.clear_breakpoints();
        assert!(state.breakpoints.is_empty());
    }

    #[test]
    fn test_all_breakpoints_iterator() {
        let mut state = DebugState::default();
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 1,
            vm_id: "bp/1".into(),
            uri: "a.dart".into(),
            line: 1,
            column: None,
            verified: true,
        });
        state.track_breakpoint(TrackedBreakpoint {
            dap_id: 2,
            vm_id: "bp/2".into(),
            uri: "b.dart".into(),
            line: 2,
            column: None,
            verified: false,
        });
        let all: Vec<_> = state.all_breakpoints().collect();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_all_breakpoints_iterator_empty() {
        let state = DebugState::default();
        let all: Vec<_> = state.all_breakpoints().collect();
        assert!(all.is_empty());
    }

    #[test]
    fn test_pause_reason_eq() {
        assert_eq!(PauseReason::Breakpoint, PauseReason::Breakpoint);
        assert_ne!(PauseReason::Breakpoint, PauseReason::Exception);
    }
}
