//! # Debug Event Handler
//!
//! Handles VM Service debug and isolate stream events, updating per-session
//! `DebugState` AND forwarding translated events to all connected DAP adapter
//! sessions (Phase 4, Task 01).
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
//! 5. **Phase 4, Task 01**: After updating `DebugState`, the handler forwards
//!    a translated [`fdemon_dap::adapter::DebugEvent`] to all registered DAP
//!    client senders in `state.dap_debug_senders`. Stale senders (dropped
//!    receivers from disconnected clients) are pruned by the `retain` pattern.
//! 6. **Phase 4, Task 03**: Pause events emit `Message::SuspendFileWatcher`
//!    and resume events emit `Message::ResumeFileWatcher` as follow-up
//!    messages so the file-watcher gate is updated in the same TEA cycle.
//!
//! The `dap_attached` flag on `DebugState` guards whether DAP events should
//! be emitted for DAP-specific use-cases.

use crate::handler::UpdateResult;
use crate::message::Message;
use crate::session::debug_state::PauseReason;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_daemon::vm_service::debugger_types::{DebugEvent, IsolateEvent};
use fdemon_dap::adapter::{DebugEvent as DapDebugEvent, PauseReason as DapPauseReason};

/// Handles a debug stream event for the given session.
///
/// Updates the per-session `DebugState` based on the incoming VM Service debug
/// event, then forwards a translated [`DapDebugEvent`] to all registered DAP
/// client senders in `state.dap_debug_senders` (Phase 4, Task 01).
///
/// **Phase 4, Task 03**: Pause events emit `Message::SuspendFileWatcher` as
/// a follow-up message (when `settings.dap.suppress_reload_on_pause` is
/// `true`) and resume events emit `Message::ResumeFileWatcher` so the TEA
/// loop can update the file-watcher gate in the same cycle.
///
/// Stale senders (where the receiving DAP session has disconnected) are
/// pruned automatically using the `retain` pattern — `try_send` returns `Err`
/// for a closed channel.
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

    // Classify the event as a pause, resume, or neither before consuming it.
    // This drives both the DAP translation and the file-watcher gate follow-up.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DebugEventKind {
        Pause,
        Resume,
        Other,
    }

    let event_kind = match &event {
        DebugEvent::PauseStart { .. }
        | DebugEvent::PauseBreakpoint { .. }
        | DebugEvent::PauseException { .. }
        | DebugEvent::PauseExit { .. }
        | DebugEvent::PauseInterrupted { .. }
        | DebugEvent::PausePostRequest { .. } => DebugEventKind::Pause,
        DebugEvent::Resume { .. } => DebugEventKind::Resume,
        _ => DebugEventKind::Other,
    };

    // Translate the VM Service event to a DAP debug event *before* mutating
    // state, so we can forward it after the match without borrow issues.
    let dap_event: Option<DapDebugEvent> = match &event {
        DebugEvent::PauseStart { isolate, .. } => Some(DapDebugEvent::Paused {
            isolate_id: isolate.id.clone(),
            reason: DapPauseReason::Entry,
            breakpoint_id: None,
        }),
        DebugEvent::PauseBreakpoint {
            isolate,
            breakpoint,
            ..
        } => Some(DapDebugEvent::Paused {
            isolate_id: isolate.id.clone(),
            reason: DapPauseReason::Breakpoint,
            // Pass the VM breakpoint ID through to the adapter so it can
            // evaluate conditional breakpoint expressions.
            breakpoint_id: breakpoint.as_ref().map(|bp| bp.id.clone()),
        }),
        DebugEvent::PauseException { isolate, .. } => Some(DapDebugEvent::Paused {
            isolate_id: isolate.id.clone(),
            reason: DapPauseReason::Exception,
            breakpoint_id: None,
        }),
        DebugEvent::PauseExit { isolate, .. } => Some(DapDebugEvent::Paused {
            isolate_id: isolate.id.clone(),
            reason: DapPauseReason::Exit,
            breakpoint_id: None,
        }),
        DebugEvent::PauseInterrupted { isolate, .. } => Some(DapDebugEvent::Paused {
            isolate_id: isolate.id.clone(),
            reason: DapPauseReason::Interrupted,
            breakpoint_id: None,
        }),
        DebugEvent::PausePostRequest { isolate, .. } => Some(DapDebugEvent::Paused {
            isolate_id: isolate.id.clone(),
            reason: DapPauseReason::Interrupted,
            breakpoint_id: None,
        }),
        DebugEvent::Resume { isolate } => Some(DapDebugEvent::Resumed {
            isolate_id: isolate.id.clone(),
        }),
        DebugEvent::BreakpointResolved { breakpoint, .. } => {
            // Extract line/column from the breakpoint location map if present.
            let (line, column) = match &breakpoint.location {
                Some(loc) => {
                    let l = loc.get("line").and_then(|v| v.as_i64()).map(|v| v as i32);
                    let c = loc.get("column").and_then(|v| v.as_i64()).map(|v| v as i32);
                    (l, c)
                }
                None => (None, None),
            };
            Some(DapDebugEvent::BreakpointResolved {
                vm_breakpoint_id: breakpoint.id.clone(),
                line,
                column,
            })
        }
        // Events that don't map to a DAP event directly.
        DebugEvent::BreakpointAdded { .. }
        | DebugEvent::BreakpointRemoved { .. }
        | DebugEvent::BreakpointUpdated { .. }
        | DebugEvent::Inspect { .. } => None,
    };

    // Mutate per-session DebugState.
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

    // Forward the translated event to all connected DAP adapters (Phase 4).
    forward_dap_event(&state.dap_debug_senders, dap_event);

    // Phase 4, Task 03: emit a file-watcher gate message when the debugger
    // transitions to/from a paused state and the setting is enabled.
    // Only emit SuspendFileWatcher when not already suspended (idempotent).
    if state.settings.dap.suppress_reload_on_pause {
        match event_kind {
            DebugEventKind::Pause if !state.file_watcher_suspended => {
                tracing::debug!("Debugger paused — suspending auto-reload");
                return UpdateResult::message(Message::SuspendFileWatcher);
            }
            DebugEventKind::Resume if state.file_watcher_suspended => {
                tracing::debug!("Debugger resumed — resuming auto-reload");
                return UpdateResult::message(Message::ResumeFileWatcher);
            }
            _ => {}
        }
    }

    UpdateResult::none()
}

/// Handles an isolate lifecycle event for the given session.
///
/// Updates the per-session `DebugState` isolate tracking based on the incoming
/// VM Service isolate event. Clears pause state if the paused isolate exits.
/// Forwards translated [`DapDebugEvent::IsolateStart`] and
/// [`DapDebugEvent::IsolateExit`] events to connected DAP adapters
/// (Phase 4, Task 01) so their thread maps stay accurate.
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

    // Translate to a DAP event before mutating state.
    let dap_event: Option<DapDebugEvent> = match &event {
        IsolateEvent::IsolateStart { isolate } => Some(DapDebugEvent::IsolateStart {
            isolate_id: isolate.id.clone(),
            name: isolate.name.clone().unwrap_or_default(),
        }),
        IsolateEvent::IsolateRunnable { isolate } => Some(DapDebugEvent::IsolateStart {
            isolate_id: isolate.id.clone(),
            name: isolate.name.clone().unwrap_or_default(),
        }),
        IsolateEvent::IsolateExit { isolate } => Some(DapDebugEvent::IsolateExit {
            isolate_id: isolate.id.clone(),
        }),
        IsolateEvent::IsolateUpdate { .. }
        | IsolateEvent::IsolateReload { .. }
        | IsolateEvent::ServiceExtensionAdded { .. } => None,
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

    // Forward the translated event to all connected DAP adapters (Phase 4).
    forward_dap_event(&state.dap_debug_senders, dap_event);

    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Forward a [`DapDebugEvent`] to all registered DAP client senders.
///
/// Uses `try_send` (non-blocking) so a slow DAP client does not stall the TEA
/// loop. If `try_send` returns an error the sender is considered stale (the
/// receiver was dropped because the DAP client disconnected) and is removed
/// from the registry via `retain`.
///
/// This function is a no-op when `dap_event` is `None` or when the registry
/// is empty / the lock is poisoned.
pub(crate) fn forward_dap_event(
    dap_debug_senders: &std::sync::Arc<
        std::sync::Mutex<Vec<tokio::sync::mpsc::Sender<DapDebugEvent>>>,
    >,
    dap_event: Option<DapDebugEvent>,
) {
    let Some(ev) = dap_event else {
        return;
    };

    match dap_debug_senders.lock() {
        Ok(mut senders) => {
            // Remove stale senders while iterating. `try_send` returns `Err`
            // when the channel is full (Err::Full) or closed (Err::Closed).
            // We only prune on `Closed`; a full channel means the adapter is
            // temporarily slow, so we keep the sender and skip this event.
            senders.retain(|tx| match tx.try_send(ev.clone()) {
                Ok(()) => true,
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    tracing::debug!(
                        "DAP debug event channel full — adapter is slow, event dropped"
                    );
                    true // keep the sender; don't prune on backpressure
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    tracing::debug!("Pruning stale DAP debug sender (client disconnected)");
                    false // prune: receiver is dropped
                }
            });
        }
        Err(e) => {
            tracing::warn!("dap_debug_senders lock poisoned: {}", e);
        }
    }
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
        // Phase 4, Task 03: pause emits SuspendFileWatcher when suppress is enabled (default).
        assert!(matches!(result.message, Some(Message::SuspendFileWatcher)));

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(debug.paused);
        assert_eq!(debug.pause_reason, Some(PauseReason::Breakpoint));
        assert_eq!(debug.paused_isolate_id.as_deref(), Some("isolates/1"));
    }

    #[test]
    fn test_pause_breakpoint_no_suspend_when_setting_disabled() {
        let (mut state, session_id) = make_state_with_session();
        state.settings.dap.suppress_reload_on_pause = false;

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
        // Setting disabled — no SuspendFileWatcher emitted.
        assert!(result.message.is_none());
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

    // ── DAP event forwarding tests (Phase 4, Task 01) ──────────────────────

    /// Helper: create a state with a session and one registered DAP sender.
    /// Returns `(state, session_id, receiver)`.
    fn make_state_with_dap_sender() -> (
        AppState,
        SessionId,
        tokio::sync::mpsc::Receiver<DapDebugEvent>,
    ) {
        let (state, session_id) = make_state_with_session();
        let (tx, rx) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        state.dap_debug_senders.lock().unwrap().push(tx);
        (state, session_id, rx)
    }

    #[test]
    fn test_pause_breakpoint_forwarded_to_dap_sender() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

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

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::Paused {
                isolate_id, reason, ..
            } => {
                assert_eq!(isolate_id, "isolates/1");
                assert_eq!(reason, fdemon_dap::adapter::PauseReason::Breakpoint);
            }
            other => panic!("Expected Paused event, got {:?}", other),
        }
    }

    #[test]
    fn test_pause_exception_forwarded_with_exception_reason() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = DebugEvent::PauseException {
            isolate: IsolateRef {
                id: "isolates/2".into(),
                name: None,
            },
            top_frame: None,
            exception: None,
        };

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::Paused {
                isolate_id, reason, ..
            } => {
                assert_eq!(isolate_id, "isolates/2");
                assert_eq!(reason, fdemon_dap::adapter::PauseReason::Exception);
            }
            other => panic!("Expected Paused event, got {:?}", other),
        }
    }

    #[test]
    fn test_pause_interrupted_forwarded_with_interrupted_reason() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = DebugEvent::PauseInterrupted {
            isolate: IsolateRef {
                id: "isolates/3".into(),
                name: None,
            },
            top_frame: None,
            at_async_suspension: false,
        };

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::Paused {
                isolate_id, reason, ..
            } => {
                assert_eq!(isolate_id, "isolates/3");
                assert_eq!(reason, fdemon_dap::adapter::PauseReason::Interrupted);
            }
            other => panic!("Expected Paused event, got {:?}", other),
        }
    }

    #[test]
    fn test_pause_exit_forwarded_with_exit_reason() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = DebugEvent::PauseExit {
            isolate: IsolateRef {
                id: "isolates/4".into(),
                name: None,
            },
            top_frame: None,
        };

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::Paused {
                isolate_id, reason, ..
            } => {
                assert_eq!(isolate_id, "isolates/4");
                assert_eq!(reason, fdemon_dap::adapter::PauseReason::Exit);
            }
            other => panic!("Expected Paused event, got {:?}", other),
        }
    }

    #[test]
    fn test_pause_start_forwarded_with_entry_reason() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = DebugEvent::PauseStart {
            isolate: IsolateRef {
                id: "isolates/5".into(),
                name: None,
            },
            top_frame: None,
        };

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::Paused {
                isolate_id, reason, ..
            } => {
                assert_eq!(isolate_id, "isolates/5");
                assert_eq!(reason, fdemon_dap::adapter::PauseReason::Entry);
            }
            other => panic!("Expected Paused event, got {:?}", other),
        }
    }

    #[test]
    fn test_resume_event_forwarded_to_dap_sender() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
        };

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::Resumed { isolate_id } => {
                assert_eq!(isolate_id, "isolates/1");
            }
            other => panic!("Expected Resumed event, got {:?}", other),
        }
    }

    #[test]
    fn test_breakpoint_resolved_forwarded_to_dap_sender() {
        use crate::session::debug_state::TrackedBreakpoint;

        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        // Pre-track the breakpoint.
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
                location: Some(serde_json::json!({"line": 43, "column": 5})),
            },
        };

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::BreakpointResolved {
                vm_breakpoint_id,
                line,
                column,
            } => {
                assert_eq!(vm_breakpoint_id, "breakpoints/1");
                assert_eq!(line, Some(43));
                assert_eq!(column, Some(5));
            }
            other => panic!("Expected BreakpointResolved event, got {:?}", other),
        }
    }

    #[test]
    fn test_breakpoint_added_does_not_forward_dap_event() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

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

        handle_debug_event(&mut state, session_id, event);

        // BreakpointAdded has no DAP equivalent — channel should be empty.
        assert!(
            rx.try_recv().is_err(),
            "BreakpointAdded should not produce a DAP event"
        );
    }

    #[test]
    fn test_breakpoint_removed_does_not_forward_dap_event() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

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

        handle_debug_event(&mut state, session_id, event);

        assert!(
            rx.try_recv().is_err(),
            "BreakpointRemoved should not produce a DAP event"
        );
    }

    #[test]
    fn test_stale_sender_pruned_on_debug_event() {
        let (mut state, session_id) = make_state_with_session();

        // Register two senders; drop the first receiver immediately to make it stale.
        let (tx1, rx1) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        let (tx2, mut rx2) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        state.dap_debug_senders.lock().unwrap().push(tx1);
        state.dap_debug_senders.lock().unwrap().push(tx2);
        drop(rx1); // rx1 is closed — tx1 becomes stale

        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };
        handle_debug_event(&mut state, session_id, event);

        // The live receiver should have gotten the event.
        assert!(
            rx2.try_recv().is_ok(),
            "Live sender should receive the event"
        );

        // The stale sender should have been pruned.
        let count = state.dap_debug_senders.lock().unwrap().len();
        assert_eq!(
            count, 1,
            "Stale sender should be pruned; only 1 should remain"
        );
    }

    #[test]
    fn test_multiple_senders_all_receive_event() {
        let (mut state, session_id) = make_state_with_session();

        let (tx1, mut rx1) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        let (tx2, mut rx2) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        let (tx3, mut rx3) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        {
            let mut senders = state.dap_debug_senders.lock().unwrap();
            senders.push(tx1);
            senders.push(tx2);
            senders.push(tx3);
        }

        let event = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
            breakpoint: None,
            pause_breakpoints: vec![],
            at_async_suspension: false,
        };

        handle_debug_event(&mut state, session_id, event);

        assert!(rx1.try_recv().is_ok(), "Sender 1 should receive the event");
        assert!(rx2.try_recv().is_ok(), "Sender 2 should receive the event");
        assert!(rx3.try_recv().is_ok(), "Sender 3 should receive the event");
    }

    #[test]
    fn test_no_senders_no_panic_on_debug_event() {
        // Registry is empty — forwarding should be a no-op, not a panic.
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };

        // Should not panic.
        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_debug_event_with_unknown_session_does_not_forward() {
        let (mut state, _) = make_state_with_session();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        state.dap_debug_senders.lock().unwrap().push(tx);

        // Use a non-existent session ID.
        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };
        handle_debug_event(&mut state, 9999, event);

        // No session means no forwarding.
        assert!(
            rx.try_recv().is_err(),
            "Unknown session should not forward events"
        );
    }

    // ── Isolate event forwarding tests ─────────────────────────────────────

    #[test]
    fn test_isolate_start_forwarded_to_dap_sender() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = IsolateEvent::IsolateStart {
            isolate: IsolateRef {
                id: "isolates/10".into(),
                name: Some("worker".into()),
            },
        };

        handle_isolate_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::IsolateStart { isolate_id, name } => {
                assert_eq!(isolate_id, "isolates/10");
                assert_eq!(name, "worker");
            }
            other => panic!("Expected IsolateStart event, got {:?}", other),
        }
    }

    #[test]
    fn test_isolate_start_with_no_name_uses_empty_string() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = IsolateEvent::IsolateStart {
            isolate: IsolateRef {
                id: "isolates/11".into(),
                name: None,
            },
        };

        handle_isolate_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::IsolateStart { isolate_id, name } => {
                assert_eq!(isolate_id, "isolates/11");
                assert_eq!(
                    name, "",
                    "Missing isolate name should default to empty string"
                );
            }
            other => panic!("Expected IsolateStart event, got {:?}", other),
        }
    }

    #[test]
    fn test_isolate_runnable_forwarded_as_isolate_start() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = IsolateEvent::IsolateRunnable {
            isolate: IsolateRef {
                id: "isolates/12".into(),
                name: Some("main".into()),
            },
        };

        handle_isolate_event(&mut state, session_id, event);

        // IsolateRunnable is forwarded as IsolateStart to the DAP adapter
        // (the adapter needs to know the isolate is ready).
        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::IsolateStart { isolate_id, .. } => {
                assert_eq!(isolate_id, "isolates/12");
            }
            other => panic!("Expected IsolateStart event, got {:?}", other),
        }
    }

    #[test]
    fn test_isolate_exit_forwarded_to_dap_sender() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        // First start the isolate.
        handle_isolate_event(
            &mut state,
            session_id,
            IsolateEvent::IsolateStart {
                isolate: IsolateRef {
                    id: "isolates/13".into(),
                    name: Some("main".into()),
                },
            },
        );
        let _ = rx.try_recv(); // consume the IsolateStart event

        // Then exit it.
        let event = IsolateEvent::IsolateExit {
            isolate: IsolateRef {
                id: "isolates/13".into(),
                name: None,
            },
        };

        handle_isolate_event(&mut state, session_id, event);

        let received = rx
            .try_recv()
            .expect("should have received a DAP IsolateExit event");
        match received {
            DapDebugEvent::IsolateExit { isolate_id } => {
                assert_eq!(isolate_id, "isolates/13");
            }
            other => panic!("Expected IsolateExit event, got {:?}", other),
        }
    }

    #[test]
    fn test_isolate_update_does_not_forward_dap_event() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = IsolateEvent::IsolateUpdate {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("renamed".into()),
            },
        };

        handle_isolate_event(&mut state, session_id, event);

        assert!(
            rx.try_recv().is_err(),
            "IsolateUpdate should not produce a DAP event"
        );
    }

    #[test]
    fn test_isolate_reload_does_not_forward_dap_event() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = IsolateEvent::IsolateReload {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };

        handle_isolate_event(&mut state, session_id, event);

        // IsolateReload has no corresponding DAP event.
        assert!(
            rx.try_recv().is_err(),
            "IsolateReload should not produce a DAP event"
        );
    }

    #[test]
    fn test_service_extension_added_does_not_forward_dap_event() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = IsolateEvent::ServiceExtensionAdded {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            extension_rpc: "ext.flutter.inspector.show".to_string(),
        };

        handle_isolate_event(&mut state, session_id, event);

        assert!(
            rx.try_recv().is_err(),
            "ServiceExtensionAdded should not produce a DAP event"
        );
    }

    #[test]
    fn test_stale_sender_pruned_on_isolate_event() {
        let (mut state, session_id) = make_state_with_session();

        // Register two senders; drop the first receiver.
        let (tx1, rx1) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        let (tx2, mut rx2) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        state.dap_debug_senders.lock().unwrap().push(tx1);
        state.dap_debug_senders.lock().unwrap().push(tx2);
        drop(rx1); // stale

        let event = IsolateEvent::IsolateStart {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: Some("main".into()),
            },
        };

        handle_isolate_event(&mut state, session_id, event);

        assert!(rx2.try_recv().is_ok(), "Live sender should receive event");

        let count = state.dap_debug_senders.lock().unwrap().len();
        assert_eq!(count, 1, "Stale sender should be pruned");
    }

    #[test]
    fn test_all_senders_pruned_when_all_disconnected() {
        let (mut state, session_id) = make_state_with_session();

        let (tx1, rx1) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        let (tx2, rx2) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        state.dap_debug_senders.lock().unwrap().push(tx1);
        state.dap_debug_senders.lock().unwrap().push(tx2);
        drop(rx1);
        drop(rx2); // both receivers dropped

        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };

        handle_debug_event(&mut state, session_id, event);

        let count = state.dap_debug_senders.lock().unwrap().len();
        assert_eq!(count, 0, "All stale senders should be pruned");
    }

    #[test]
    fn test_debug_state_still_updated_with_dap_senders() {
        // Verify that adding DAP senders doesn't break the existing DebugState update.
        let (mut state, session_id, _rx) = make_state_with_dap_sender();

        let event = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
            breakpoint: None,
            pause_breakpoints: vec![],
            at_async_suspension: false,
        };

        handle_debug_event(&mut state, session_id, event);

        let debug = &state.session_manager.get(session_id).unwrap().session.debug;
        assert!(
            debug.paused,
            "DebugState must still be updated when DAP senders are registered"
        );
        assert_eq!(debug.pause_reason, Some(PauseReason::Breakpoint));
    }

    #[test]
    fn test_debug_state_still_updated_without_dap_senders() {
        // Verify existing behavior is preserved when no DAP senders are registered.
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    #[test]
    fn test_pause_post_request_forwarded_as_interrupted() {
        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        let event = DebugEvent::PausePostRequest {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
        };

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should have received a DAP event");
        match received {
            DapDebugEvent::Paused {
                isolate_id, reason, ..
            } => {
                assert_eq!(isolate_id, "isolates/1");
                // PausePostRequest is mapped to Interrupted (no dedicated DAP reason).
                assert_eq!(reason, fdemon_dap::adapter::PauseReason::Interrupted);
            }
            other => panic!("Expected Paused event, got {:?}", other),
        }
    }

    #[test]
    fn test_forward_dap_event_no_op_on_none() {
        // forward_dap_event should be a no-op when dap_event is None.
        let senders = std::sync::Arc::new(std::sync::Mutex::new(vec![
            tokio::sync::mpsc::channel::<DapDebugEvent>(4).0,
        ]));

        // None event — should not modify senders or panic.
        forward_dap_event(&senders, None);

        assert_eq!(
            senders.lock().unwrap().len(),
            1,
            "Sender list should be unchanged after a None event"
        );
    }

    #[test]
    fn test_forward_dap_event_empty_registry_no_op() {
        // forward_dap_event with empty registry should not panic.
        let senders = std::sync::Arc::new(std::sync::Mutex::new(Vec::<
            tokio::sync::mpsc::Sender<DapDebugEvent>,
        >::new()));

        let ev = DapDebugEvent::Resumed {
            isolate_id: "isolates/1".to_string(),
        };

        forward_dap_event(&senders, Some(ev));

        assert_eq!(
            senders.lock().unwrap().len(),
            0,
            "Empty registry should remain empty"
        );
    }

    #[test]
    fn test_isolate_event_with_unknown_session_does_not_forward() {
        let (mut state, _) = make_state_with_session();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<DapDebugEvent>(16);
        state.dap_debug_senders.lock().unwrap().push(tx);

        let event = IsolateEvent::IsolateStart {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };
        handle_isolate_event(&mut state, 9999, event);

        assert!(
            rx.try_recv().is_err(),
            "Unknown session should not forward isolate events"
        );
    }

    #[test]
    fn test_full_channel_sender_retained_not_pruned() {
        // When the channel is full, the sender should be retained (not pruned).
        // The event is silently dropped but the sender stays for future events.
        let (mut state, session_id) = make_state_with_session();

        // Capacity of 1 — will fill up quickly.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<DapDebugEvent>(1);
        state.dap_debug_senders.lock().unwrap().push(tx);

        // Fill the channel.
        let fill_event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };
        handle_debug_event(&mut state, session_id, fill_event);
        assert_eq!(rx.try_recv().is_ok(), true, "Channel should have one event");

        // Fill again without consuming.
        let fill_event2 = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };
        handle_debug_event(&mut state, session_id, fill_event2);

        // Now send a 3rd event when channel is full.
        let overflow_event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };
        handle_debug_event(&mut state, session_id, overflow_event);

        // Sender should still be in the registry even though events were dropped.
        let count = state.dap_debug_senders.lock().unwrap().len();
        assert_eq!(
            count, 1,
            "Full-channel sender should be retained, not pruned"
        );

        // Drain the channel.
        let _ = rx.try_recv();
    }

    #[test]
    fn test_breakpoint_resolved_no_location_has_none_line_column() {
        use crate::session::debug_state::TrackedBreakpoint;

        let (mut state, session_id, mut rx) = make_state_with_dap_sender();

        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .debug
            .track_breakpoint(TrackedBreakpoint {
                dap_id: 1,
                vm_id: "breakpoints/2".to_string(),
                uri: "package:app/lib.dart".to_string(),
                line: 10,
                column: None,
                verified: false,
            });

        let event = DebugEvent::BreakpointResolved {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            breakpoint: fdemon_daemon::vm_service::debugger_types::Breakpoint {
                id: "breakpoints/2".to_string(),
                breakpoint_number: 2,
                enabled: true,
                resolved: true,
                location: None, // no location info
            },
        };

        handle_debug_event(&mut state, session_id, event);

        let received = rx.try_recv().expect("should forward BreakpointResolved");
        match received {
            DapDebugEvent::BreakpointResolved { line, column, .. } => {
                assert!(line.is_none(), "No location should produce None line");
                assert!(column.is_none(), "No location should produce None column");
            }
            other => panic!("Expected BreakpointResolved event, got {:?}", other),
        }
    }

    // ── Phase 4, Task 03: Coordinated Pause / File-Watcher Gate tests ─────────

    #[test]
    fn test_pause_emits_suspend_when_setting_enabled() {
        let (mut state, session_id) = make_state_with_session();
        // suppress_reload_on_pause defaults to true.
        assert!(state.settings.dap.suppress_reload_on_pause);

        let event = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
            breakpoint: None,
            pause_breakpoints: vec![],
            at_async_suspension: false,
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(matches!(result.message, Some(Message::SuspendFileWatcher)));
    }

    #[test]
    fn test_pause_does_not_emit_suspend_when_already_suspended() {
        let (mut state, session_id) = make_state_with_session();
        // Simulate already suspended.
        state.file_watcher_suspended = true;

        let event = DebugEvent::PauseException {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
            exception: None,
        };

        let result = handle_debug_event(&mut state, session_id, event);
        // Already suspended — no follow-up message.
        assert!(result.message.is_none());
    }

    #[test]
    fn test_pause_interrupted_emits_suspend_when_not_suspended() {
        let (mut state, session_id) = make_state_with_session();
        assert!(!state.file_watcher_suspended);

        let event = DebugEvent::PauseInterrupted {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
            at_async_suspension: false,
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(matches!(result.message, Some(Message::SuspendFileWatcher)));
    }

    #[test]
    fn test_pause_post_request_emits_suspend() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::PausePostRequest {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(matches!(result.message, Some(Message::SuspendFileWatcher)));
    }

    #[test]
    fn test_pause_start_emits_suspend() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::PauseStart {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(matches!(result.message, Some(Message::SuspendFileWatcher)));
    }

    #[test]
    fn test_resume_emits_resume_when_suspended() {
        let (mut state, session_id) = make_state_with_session();
        state.file_watcher_suspended = true;

        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(matches!(result.message, Some(Message::ResumeFileWatcher)));
    }

    #[test]
    fn test_resume_does_not_emit_resume_when_not_suspended() {
        let (mut state, session_id) = make_state_with_session();
        // Not suspended — Resume event should not generate ResumeFileWatcher.
        assert!(!state.file_watcher_suspended);

        let event = DebugEvent::Resume {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_non_pause_event_never_emits_suspend() {
        let (mut state, session_id) = make_state_with_session();

        let event = DebugEvent::BreakpointAdded {
            breakpoint: fdemon_daemon::vm_service::debugger_types::Breakpoint {
                id: "bp-1".to_string(),
                breakpoint_number: 1,
                enabled: true,
                resolved: false,
                location: None,
            },
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_pause_with_suppress_disabled_emits_no_suspend() {
        let (mut state, session_id) = make_state_with_session();
        state.settings.dap.suppress_reload_on_pause = false;

        for event in [DebugEvent::PauseBreakpoint {
            isolate: IsolateRef {
                id: "isolates/1".into(),
                name: None,
            },
            top_frame: None,
            breakpoint: None,
            pause_breakpoints: vec![],
            at_async_suspension: false,
        }] {
            let result = handle_debug_event(&mut state, session_id, event);
            assert!(
                result.message.is_none(),
                "Should not emit SuspendFileWatcher when setting is false"
            );
        }
    }
}
