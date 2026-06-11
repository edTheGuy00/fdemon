//! NewSessionDialog QR pairing modal handlers
//!
//! Drives the ADB wireless QR pairing flow: opening the modal (`p`), minting
//! credentials and the QR payload, reacting to progress events from the
//! background mDNS/adb task, and refreshing the connected-device list once
//! the device is attached.

use tokio_util::sync::CancellationToken;

use crate::handler::{UpdateAction, UpdateResult};
use crate::new_session_dialog::{QrPairingPhase, TargetTab};
use crate::state::AppState;
use fdemon_daemon::{QrPairingCredentials, QrPairingEvent};

/// Open the QR pairing modal (or restart it with a fresh code).
///
/// Cancels any in-flight session, mints fresh credentials, stores the QR
/// payload for rendering, and dispatches the background pairing task.
/// Bound to `p` in the dialog and `r` inside the modal.
///
/// When `adb` is unavailable the modal opens directly in a failed state
/// explaining the problem; no background task is started.
pub fn handle_open_qr_pairing(state: &mut AppState) -> UpdateResult {
    if !state.tool_availability.adb {
        tracing::warn!("handle_open_qr_pairing: adb not available — not starting pairing task");
        state.new_session_dialog_state.open_qr_pairing_failed(
            "adb was not found on PATH. Install Android platform-tools \
             (or add it to PATH) to pair devices over Wi-Fi."
                .to_string(),
        );
        return UpdateResult::none();
    }

    let credentials = QrPairingCredentials::generate();
    let cancel_token = CancellationToken::new();
    let seq = state
        .new_session_dialog_state
        .begin_qr_pairing(credentials.qr_payload(), cancel_token.clone());

    tracing::info!(seq, service_name = %credentials.service_name, "starting ADB QR pairing");
    UpdateResult::action(UpdateAction::StartQrPairing {
        seq,
        credentials,
        cancel_token,
    })
}

/// Close the QR pairing modal, cancelling the background task.
pub fn handle_close_qr_pairing(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.cancel_qr_pairing();
    UpdateResult::none()
}

/// Handle a progress event from the background pairing task.
///
/// Stale events (seq mismatch — the session was cancelled or restarted) are
/// discarded.
pub fn handle_qr_pairing_progress(
    state: &mut AppState,
    seq: u64,
    event: QrPairingEvent,
) -> UpdateResult {
    let phase = match event {
        QrPairingEvent::PhoneFound { ip } => QrPairingPhase::Pairing { ip },
        QrPairingEvent::Paired { ip } => QrPairingPhase::Connecting { ip },
    };
    let applied = state
        .new_session_dialog_state
        .set_qr_pairing_phase(seq, phase);
    if !applied {
        tracing::debug!(seq, "ignoring stale QR pairing progress");
    }
    UpdateResult::none()
}

/// Handle successful completion: the device is now reachable via
/// `adb connect`. Closes the modal, switches to the Connected tab and runs a
/// foreground device discovery so the new device appears (mirrors
/// `handle_boot_completed`).
pub fn handle_qr_pairing_completed(
    state: &mut AppState,
    seq: u64,
    ip: String,
    connect_port: u16,
) -> UpdateResult {
    let dialog = &mut state.new_session_dialog_state;
    let is_current = dialog
        .qr_pairing_modal
        .as_ref()
        .is_some_and(|pairing| pairing.seq == seq);
    if !is_current {
        tracing::debug!(seq, "ignoring stale QR pairing completion");
        return UpdateResult::none();
    }

    tracing::info!(ip = %ip, port = connect_port, "QR pairing complete — refreshing devices");
    // Close the modal. The task already finished, so firing the token is a
    // no-op, but going through cancel_qr_pairing() keeps the invariant that
    // every session leaves the field with its token fired.
    dialog.cancel_qr_pairing();
    dialog.target_selector.set_tab(TargetTab::Connected);

    let Some(flutter) = state.flutter_executable() else {
        tracing::warn!("handle_qr_pairing_completed: no Flutter SDK — cannot discover devices");
        return UpdateResult::none();
    };
    state.new_session_dialog_state.target_selector.loading = true;
    UpdateResult::action(UpdateAction::DiscoverDevices { flutter })
}

/// Handle pairing failure: surface the error in the modal with a retry hint.
/// Stale failures are discarded.
pub fn handle_qr_pairing_failed(state: &mut AppState, seq: u64, error: String) -> UpdateResult {
    let applied = state
        .new_session_dialog_state
        .set_qr_pairing_phase(seq, QrPairingPhase::Failed { error });
    if !applied {
        tracing::debug!(seq, "ignoring stale QR pairing failure");
    }
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoadedConfigs;
    use crate::state::UiMode;
    use fdemon_daemon::test_utils::fake_flutter_sdk;
    use std::path::PathBuf;

    fn test_app_state() -> AppState {
        let mut state = AppState::with_settings(
            PathBuf::from("/test/project"),
            crate::config::Settings::default(),
        );
        state.project_name = Some("TestProject".to_string());
        state.ui_mode = UiMode::NewSessionDialog;
        state.show_new_session_dialog(LoadedConfigs::default());
        state.resolved_sdk = Some(fake_flutter_sdk());
        state.tool_availability.adb = true;
        state
    }

    fn modal(state: &AppState) -> Option<&crate::new_session_dialog::QrPairingState> {
        state.new_session_dialog_state.qr_pairing_modal.as_ref()
    }

    #[test]
    fn open_qr_pairing_stores_state_and_dispatches_action() {
        let mut state = test_app_state();

        let result = handle_open_qr_pairing(&mut state);

        let pairing = modal(&state).expect("qr modal open");
        assert_eq!(pairing.phase, QrPairingPhase::WaitingForScan);
        assert!(pairing.payload.starts_with("WIFI:T:ADB;S:fdemon-"));
        assert!(pairing.payload.ends_with(";;"));
        assert!(!pairing.cancel.is_cancelled());

        match result.action {
            Some(UpdateAction::StartQrPairing {
                seq, credentials, ..
            }) => {
                assert_eq!(seq, pairing.seq);
                assert_eq!(credentials.qr_payload(), pairing.payload);
            }
            other => panic!("expected StartQrPairing action, got {other:?}"),
        }
    }

    #[test]
    fn open_qr_pairing_without_adb_shows_failed_modal() {
        let mut state = test_app_state();
        state.tool_availability.adb = false;

        let result = handle_open_qr_pairing(&mut state);

        let pairing = modal(&state).expect("modal opens to show the error");
        assert!(
            matches!(&pairing.phase, QrPairingPhase::Failed { error } if error.contains("adb")),
            "expected Failed phase mentioning adb, got {:?}",
            pairing.phase
        );
        assert!(result.action.is_none(), "no background task without adb");
    }

    #[test]
    fn open_qr_pairing_again_cancels_previous_session() {
        let mut state = test_app_state();

        handle_open_qr_pairing(&mut state);
        let first_cancel = modal(&state).unwrap().cancel.clone();
        let first_seq = modal(&state).unwrap().seq;

        handle_open_qr_pairing(&mut state);

        assert!(first_cancel.is_cancelled(), "old session must be cancelled");
        let second = modal(&state).unwrap();
        assert!(second.seq > first_seq, "seq must advance");
        assert!(!second.cancel.is_cancelled());
    }

    #[test]
    fn close_qr_pairing_cancels_and_clears() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let token = modal(&state).unwrap().cancel.clone();

        handle_close_qr_pairing(&mut state);

        assert!(token.is_cancelled());
        assert!(modal(&state).is_none());
    }

    #[test]
    fn progress_event_updates_phase() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let seq = modal(&state).unwrap().seq;

        handle_qr_pairing_progress(
            &mut state,
            seq,
            QrPairingEvent::PhoneFound {
                ip: "192.168.1.42".to_string(),
            },
        );
        assert_eq!(
            modal(&state).unwrap().phase,
            QrPairingPhase::Pairing {
                ip: "192.168.1.42".to_string()
            }
        );

        handle_qr_pairing_progress(
            &mut state,
            seq,
            QrPairingEvent::Paired {
                ip: "192.168.1.42".to_string(),
            },
        );
        assert_eq!(
            modal(&state).unwrap().phase,
            QrPairingPhase::Connecting {
                ip: "192.168.1.42".to_string()
            }
        );
    }

    #[test]
    fn stale_progress_event_is_discarded() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let seq = modal(&state).unwrap().seq;

        handle_qr_pairing_progress(
            &mut state,
            seq + 99,
            QrPairingEvent::PhoneFound {
                ip: "10.0.0.1".to_string(),
            },
        );

        assert_eq!(
            modal(&state).unwrap().phase,
            QrPairingPhase::WaitingForScan,
            "stale event must not change phase"
        );
    }

    #[test]
    fn completed_closes_modal_switches_tab_and_refreshes() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Bootable);
        let seq = modal(&state).unwrap().seq;

        let result =
            handle_qr_pairing_completed(&mut state, seq, "192.168.1.42".to_string(), 40123);

        assert!(modal(&state).is_none(), "modal closed");
        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Connected
        );
        assert!(state.new_session_dialog_state.target_selector.loading);
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverDevices { .. })
        ));
    }

    #[test]
    fn stale_completed_is_discarded() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let seq = modal(&state).unwrap().seq;

        let result =
            handle_qr_pairing_completed(&mut state, seq + 1, "192.168.1.42".to_string(), 40123);

        assert!(modal(&state).is_some(), "modal stays open");
        assert!(result.action.is_none());
    }

    #[test]
    fn failed_sets_failed_phase() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let seq = modal(&state).unwrap().seq;

        handle_qr_pairing_failed(&mut state, seq, "adb pair failed".to_string());

        assert_eq!(
            modal(&state).unwrap().phase,
            QrPairingPhase::Failed {
                error: "adb pair failed".to_string()
            }
        );
    }

    #[test]
    fn stale_failed_is_discarded() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let seq = modal(&state).unwrap().seq;

        handle_qr_pairing_failed(&mut state, seq + 1, "boom".to_string());

        assert_eq!(modal(&state).unwrap().phase, QrPairingPhase::WaitingForScan);
    }

    #[test]
    fn hide_new_session_dialog_cancels_pairing_task() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let token = modal(&state).unwrap().cancel.clone();

        state.hide_new_session_dialog();

        assert!(
            token.is_cancelled(),
            "closing the dialog must stop the task"
        );
        assert!(state.new_session_dialog_state.qr_pairing_modal.is_none());
    }

    #[test]
    fn reopening_dialog_cancels_previous_pairing_task() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let token = modal(&state).unwrap().cancel.clone();

        // Re-open replaces the dialog state wholesale; the old task's token
        // must be fired before the state (and token clone) is dropped.
        state.show_new_session_dialog(LoadedConfigs::default());

        assert!(token.is_cancelled(), "reopen must stop the leaked task");
        assert!(state.new_session_dialog_state.qr_pairing_modal.is_none());
    }

    #[test]
    fn escape_closes_qr_modal_first() {
        let mut state = test_app_state();
        handle_open_qr_pairing(&mut state);
        let token = modal(&state).unwrap().cancel.clone();

        let result = crate::handler::new_session::handle_new_session_dialog_escape(&mut state);

        assert!(token.is_cancelled());
        assert!(modal(&state).is_none());
        assert!(result.message.is_none(), "Esc must only close the modal");
    }
}
