//! # Debug Event Handler
//!
//! Handles VM Service debug and isolate stream events, updating per-session
//! `DebugState`. This module is the app-layer counterpart to the daemon-layer
//! debug RPCs and event types (DAP Server Phase 1).
//!
//! ## Event flow
//!
//! 1. The VM Service event forwarding loop in `actions/vm_service.rs` receives
//!    "Debug" and "Isolate" stream notifications.
//! 2. They are parsed into `DebugEvent` / `IsolateEvent` and wrapped in
//!    `Message::VmServiceDebugEvent` / `Message::VmServiceIsolateEvent`.
//! 3. The TEA `update()` function dispatches them here.
//! 4. These handlers mutate per-session `DebugState` to reflect the current
//!    debugger state.
//!
//! The DAP adapter (Phase 2+) reads `DebugState` when deciding what DAP events
//! to send to the IDE. The `dap_attached` flag on `DebugState` guards whether
//! DAP events should be emitted.

use crate::handler::UpdateResult;
use crate::session::debug_state::PauseReason;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_daemon::vm_service::debugger_types::{DebugEvent, IsolateEvent};

/// Handles a debug stream event for the given session.
///
/// Updates the per-session `DebugState` based on the incoming VM Service debug
/// event. Currently returns `UpdateResult::none()` for all events — in Phase 3+,
/// some events (e.g. `PauseBreakpoint`) will return `UpdateAction` variants to
/// notify a connected DAP client.
///
/// No-op if the session does not exist (e.g. race condition between session
/// close and an in-flight event).
pub fn handle_debug_event(
    state: &mut AppState,
    session_id: SessionId,
    event: DebugEvent,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };

    match event {
        DebugEvent::PauseStart { isolate, .. } => {
            handle
                .session
                .debug
                .mark_paused(PauseReason::Entry, isolate.id);
        }
        DebugEvent::PauseBreakpoint { isolate, .. } => {
            handle
                .session
                .debug
                .mark_paused(PauseReason::Breakpoint, isolate.id);
        }
        DebugEvent::PauseException { isolate, .. } => {
            handle
                .session
                .debug
                .mark_paused(PauseReason::Exception, isolate.id);
        }
        DebugEvent::PauseExit { isolate, .. } => {
            handle
                .session
                .debug
                .mark_paused(PauseReason::Exit, isolate.id);
        }
        DebugEvent::PauseInterrupted { isolate, .. } => {
            handle
                .session
                .debug
                .mark_paused(PauseReason::Interrupted, isolate.id);
        }
        DebugEvent::PausePostRequest { isolate, .. } => {
            handle
                .session
                .debug
                .mark_paused(PauseReason::PostRequest, isolate.id);
        }
        DebugEvent::Resume { .. } => {
            handle.session.debug.mark_resumed();
        }
        DebugEvent::BreakpointAdded { breakpoint, .. } => {
            // Breakpoint tracking is managed by the DAP adapter (TrackedBreakpoint).
            // BreakpointAdded events confirm VM-side creation — log for debugging.
            tracing::debug!("Breakpoint added: {}", breakpoint.id);
        }
        DebugEvent::BreakpointResolved { breakpoint, .. } => {
            handle
                .session
                .debug
                .mark_breakpoint_verified(&breakpoint.id);
        }
        DebugEvent::BreakpointRemoved { breakpoint, .. } => {
            // Remove from tracked breakpoints so DebugState stays consistent
            // with the VM. This covers VM-initiated removals (e.g., hot restart
            // clearing breakpoints) in addition to user-initiated ones.
            handle.session.debug.untrack_breakpoint(&breakpoint.id);
            tracing::debug!("Breakpoint removed: {}", breakpoint.id);
        }
        DebugEvent::BreakpointUpdated { breakpoint, .. } => {
            // Breakpoint metadata updates (e.g., resolved location) are informational.
            // Full breakpoint sync will be implemented in Phase 3 (DAP adapter).
            tracing::debug!("Breakpoint updated: {}", breakpoint.id);
        }
        DebugEvent::Inspect { inspectee, .. } => {
            tracing::debug!("Inspect event: {:?}", inspectee.kind);
        }
    }

    UpdateResult::none()
}

/// Handles an isolate lifecycle event for the given session.
///
/// Updates the per-session `DebugState` isolate tracking based on the incoming
/// VM Service isolate event. Clears pause state if the paused isolate exits.
///
/// No-op if the session does not exist (e.g. race condition between session
/// close and an in-flight event).
pub fn handle_isolate_event(
    state: &mut AppState,
    session_id: SessionId,
    event: IsolateEvent,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };

    match event {
        IsolateEvent::IsolateStart { isolate } => {
            handle.session.debug.add_isolate(isolate);
        }
        IsolateEvent::IsolateRunnable { isolate } => {
            // Isolate is ready for VM Service commands.
            // Ensure it's tracked (IsolateStart may have been missed on reconnect).
            handle.session.debug.add_isolate(isolate);
        }
        IsolateEvent::IsolateExit { isolate } => {
            handle.session.debug.remove_isolate(&isolate.id);
            // If the paused isolate exited, clear pause state to reflect reality.
            if handle.session.debug.paused_isolate_id.as_deref() == Some(&isolate.id) {
                handle.session.debug.mark_resumed();
            }
        }
        IsolateEvent::IsolateUpdate { .. } => {
            // Name/metadata change — no action needed for debug state.
        }
        IsolateEvent::IsolateReload { .. } => {
            // Hot reload completed. Breakpoints may need re-verification.
            // Full handling occurs in Phase 4 (coordinated reload).
            tracing::debug!("Isolate reload event for session {session_id}");
        }
        IsolateEvent::ServiceExtensionAdded { .. } => {
            // Service extensions are handled by the existing Extension stream handler.
        }
    }

    UpdateResult::none()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use fdemon_daemon::vm_service::debugger_types::IsolateRef;

    fn make_state_with_session() -> (AppState, SessionId) {
        let mut state = AppState::new();
        let device = fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let session_id = state.session_manager.create_session(&device).unwrap();
        (state, session_id)
    }

    // -- handle_debug_event --------------------------------------------------

    #[test]
    fn test_pause_breakpoint_updates_debug_state() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
            top_frame: None,
            breakpoint: None,
            pause_breakpoints: vec![],
            at_async_suspension: false,
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.action.is_none());
        assert!(result.message.is_none());

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(debug.paused);
        assert_eq!(debug.pause_reason, Some(PauseReason::Breakpoint));
        assert_eq!(debug.paused_isolate_id.as_deref(), Some("isolates/1"));
    }

    #[test]
    fn test_pause_start_maps_to_entry_reason() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::PauseStart {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
            top_frame: None,
        };
        handle_debug_event(&mut state, session_id, event);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(debug.paused);
        assert_eq!(debug.pause_reason, Some(PauseReason::Entry));
    }

    #[test]
    fn test_pause_exception_maps_to_exception_reason() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::PauseException {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
            exception: None,
        };
        handle_debug_event(&mut state, session_id, event);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert_eq!(debug.pause_reason, Some(PauseReason::Exception));
    }

    #[test]
    fn test_pause_exit_maps_to_exit_reason() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::PauseExit {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
        };
        handle_debug_event(&mut state, session_id, event);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert_eq!(debug.pause_reason, Some(PauseReason::Exit));
    }

    #[test]
    fn test_pause_interrupted_maps_to_interrupted_reason() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::PauseInterrupted {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
            at_async_suspension: false,
        };
        handle_debug_event(&mut state, session_id, event);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert_eq!(debug.pause_reason, Some(PauseReason::Interrupted));
    }

    #[test]
    fn test_pause_post_request_maps_to_post_request_reason() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::PausePostRequest {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
        };
        handle_debug_event(&mut state, session_id, event);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert_eq!(debug.pause_reason, Some(PauseReason::PostRequest));
    }

    #[test]
    fn test_resume_clears_pause_state() {
        let (mut state, session_id) = make_state_with_session();

        // First pause.
        let pause_event = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
            top_frame: None,
            breakpoint: None,
            pause_breakpoints: vec![],
            at_async_suspension: false,
        };
        handle_debug_event(&mut state, session_id, pause_event);

        // Then resume.
        let resume_event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
        };
        handle_debug_event(&mut state, session_id, resume_event);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(!debug.paused);
        assert!(debug.pause_reason.is_none());
        assert!(debug.paused_isolate_id.is_none());
    }

    #[test]
    fn test_breakpoint_resolved_marks_verified() {
        use crate::session::debug_state::TrackedBreakpoint;

        let (mut state, session_id) = make_state_with_session();

        // Track the breakpoint first.
        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .debug
            .track_breakpoint(TrackedBreakpoint {
                dap_id: 1,
                vm_id: "breakpoints/1".to_string(),
                uri: "package:app/main.dart".to_string(),
                line: 42,
                column: None,
                verified: false,
            });

        // Send BreakpointResolved.
        let event = DebugEvent::BreakpointResolved {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            breakpoint: fdemon_daemon::vm_service::debugger_types::Breakpoint {
                id: "breakpoints/1".to_string(),
                breakpoint_number: 1,
                enabled: true,
                resolved: true,
                location: None,
            },
        };
        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.action.is_none());

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(debug.breakpoints_for_uri("package:app/main.dart")[0].verified);
    }

    #[test]
    fn test_breakpoint_added_returns_none() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::BreakpointAdded {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            breakpoint: fdemon_daemon::vm_service::debugger_types::Breakpoint {
                id: "breakpoints/1".to_string(),
                breakpoint_number: 1,
                enabled: true,
                resolved: false,
                location: None,
            },
        };
        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    #[test]
    fn test_breakpoint_removed_untracks() {
        use crate::session::debug_state::TrackedBreakpoint;

        let (mut state, session_id) = make_state_with_session();

        // Setup: track a breakpoint in DebugState.
        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .debug
            .track_breakpoint(TrackedBreakpoint {
                dap_id: 1,
                vm_id: "breakpoints/1".to_string(),
                uri: "package:app/main.dart".to_string(),
                line: 10,
                column: None,
                verified: true,
            });

        // Confirm it is tracked.
        {
            let debug = &state.session_manager.get(session_id).unwrap().session.debug;
            assert_eq!(debug.breakpoints_for_uri("package:app/main.dart").len(), 1);
        }

        // Dispatch BreakpointRemoved event.
        let event = DebugEvent::BreakpointRemoved {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            breakpoint: fdemon_daemon::vm_service::debugger_types::Breakpoint {
                id: "breakpoints/1".to_string(),
                breakpoint_number: 1,
                enabled: true,
                resolved: true,
                location: None,
            },
        };
        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.action.is_none());
        assert!(result.message.is_none());

        // Assert: breakpoint is no longer in DebugState.
        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(
            debug
                .breakpoints_for_uri("package:app/main.dart")
                .is_empty(),
            "BreakpointRemoved should untrack the breakpoint from DebugState"
        );
    }

    #[test]
    fn test_unknown_session_returns_none_for_debug_event() {
        let mut state = AppState::new();

        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };
        let result = handle_debug_event(&mut state, 9999, event);
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    // -- handle_isolate_event ------------------------------------------------

    #[test]
    fn test_isolate_start_tracks_isolate() {
        let (mut state, session_id) = make_state_with_session();

        let event = IsolateEvent::IsolateStart {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
        };
        let result = handle_isolate_event(&mut state, session_id, event);
        assert!(result.action.is_none());

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert_eq!(debug.isolates.len(), 1);
        assert_eq!(debug.isolates[0].id, "isolates/1");
    }

    #[test]
    fn test_isolate_runnable_tracks_isolate() {
        let (mut state, session_id) = make_state_with_session();

        let event = IsolateEvent::IsolateRunnable {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
        };
        handle_isolate_event(&mut state, session_id, event);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert_eq!(debug.isolates.len(), 1);
    }

    #[test]
    fn test_isolate_runnable_is_idempotent_after_isolate_start() {
        let (mut state, session_id) = make_state_with_session();

        handle_isolate_event(
            &mut state,
            session_id,
            IsolateEvent::IsolateStart {
                isolate: IsolateRef {
                    id: "isolates/1".into(),
                    name: None,
                },
            },
        );
        handle_isolate_event(
            &mut state,
            session_id,
            IsolateEvent::IsolateRunnable {
                isolate: IsolateRef {
                    id: "isolates/1".into(),
                    name: None,
                },
            },
        );

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        // Deduplicated — still only 1 isolate.
        assert_eq!(debug.isolates.len(), 1);
    }

    #[test]
    fn test_isolate_exit_removes_isolate() {
        let (mut state, session_id) = make_state_with_session();

        handle_isolate_event(
            &mut state,
            session_id,
            IsolateEvent::IsolateStart {
                isolate: IsolateRef {
                    id: "isolates/1".into(),
                    name: None,
                },
            },
        );

        handle_isolate_event(
            &mut state,
            session_id,
            IsolateEvent::IsolateExit {
                isolate: IsolateRef {
                    id: "isolates/1".into(),
                    name: None,
                },
            },
        );

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(debug.isolates.is_empty());
    }

    #[test]
    fn test_isolate_exit_clears_pause_if_paused_isolate() {
        let (mut state, session_id) = make_state_with_session();

        // Pause on isolate 1.
        let pause = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
            top_frame: None,
            breakpoint: None,
            pause_breakpoints: vec![],
            at_async_suspension: false,
        };
        handle_debug_event(&mut state, session_id, pause);

        // Isolate 1 exits.
        let exit = IsolateEvent::IsolateExit {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
        };
        handle_isolate_event(&mut state, session_id, exit);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(
            !debug.paused,
            "Pause state should be cleared when paused isolate exits"
        );
        assert!(debug.pause_reason.is_none());
    }

    #[test]
    fn test_isolate_exit_does_not_clear_pause_for_other_isolate() {
        let (mut state, session_id) = make_state_with_session();

        // Pause on isolate 1.
        handle_debug_event(
            &mut state,
            session_id,
            DebugEvent::PauseBreakpoint {
                isolate: IsolateRef {
                    id: "isolates/1".into(),
                    name: None,
                },
                top_frame: None,
                breakpoint: None,
                pause_breakpoints: vec![],
                at_async_suspension: false,
            },
        );

        // Isolate 2 exits (different from the paused one).
        handle_isolate_event(
            &mut state,
            session_id,
            IsolateEvent::IsolateExit {
                isolate: IsolateRef {
                    id: "isolates/2".into(),
                    name: None,
                },
            },
        );

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        // Isolate 1 is still paused — only isolate 2 exited.
        assert!(
            debug.paused,
            "Pause state should be preserved when a different isolate exits"
        );
        assert_eq!(debug.paused_isolate_id.as_deref(), Some("isolates/1"));
    }

    #[test]
    fn test_isolate_update_is_noop() {
        let (mut state, session_id) = make_state_with_session();

        let result = handle_isolate_event(
            &mut state,
            session_id,
            IsolateEvent::IsolateUpdate {
                isolate: IsolateRef {
                    id: "isolates/1".into(),
                    name: Some("renamed".into()),
                },
            },
        );
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    #[test]
    fn test_unknown_session_returns_none_for_isolate_event() {
        let mut state = AppState::new();

        let result = handle_isolate_event(
            &mut state,
            9999,
            IsolateEvent::IsolateStart {
                isolate: IsolateRef {
                    id: "isolates/1".into(),
                    name: None,
                },
            },
        );
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }
}
