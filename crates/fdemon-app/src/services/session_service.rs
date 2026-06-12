//! Session and device control for external consumers
//!
//! This module provides the SessionService trait so remote-control consumers
//! (the future MCP server) can list, start, and stop Flutter sessions without
//! direct access to the Engine. Reads come from [`SharedState`] snapshots
//! (synced by the Engine after each TEA cycle); control operations are
//! dispatched as [`Message`]s through the Engine's message channel, reusing
//! the same handler machinery as the keyboard user.

use std::sync::Arc;

use tokio::sync::mpsc;

use super::state_service::SharedState;
use crate::message::Message;
use crate::session::SessionId;
use fdemon_core::prelude::*;
use fdemon_core::AppPhase;

/// Point-in-time view of one active session.
///
/// Synced from `AppState::session_manager` into [`SharedState::sessions`]
/// by the Engine after each message-processing cycle.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub session_id: SessionId,
    /// Display name (device name or launch-config name)
    pub name: String,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub phase: AppPhase,
    /// Daemon-assigned app id, once the app has started
    pub app_id: Option<String>,
    /// Full browser DevTools URL (`base_url?uri=<ws_uri>`), once the daemon
    /// has reported both the DevTools endpoint and the VM Service URI
    pub devtools_url: Option<String>,
}

/// Session lifecycle control and inspection
///
/// Both TUI-side consumers and future MCP handlers use this trait.
#[trait_variant::make(SessionService: Send)]
pub trait LocalSessionService {
    /// List all active sessions (id, device, phase, DevTools URL).
    async fn list_sessions(&self) -> Vec<SessionSnapshot>;

    /// Start a new session on the device with the given id.
    ///
    /// The device must be present in the device cache (populated by device
    /// discovery). The request is dispatched through the Engine's message
    /// channel; success means the request was queued, not that the session
    /// started. Subscribe to `EngineEvent::SessionStarted` for the outcome.
    async fn start_session(&self, device_id: &str) -> Result<()>;

    /// Stop the app and remove the session with the given id.
    ///
    /// Dispatched through the Engine's message channel; unknown ids are
    /// ignored by the handler with a warning.
    async fn stop_session(&self, session_id: SessionId) -> Result<()>;

    /// DevTools URL for a running session, if the daemon has reported one.
    async fn get_devtools_url(&self, session_id: SessionId) -> Option<String>;
}

/// Implementation backed by [`SharedState`] snapshots and the Engine's
/// message channel. Cheap to clone-construct and `Send + 'static`, so it can
/// be moved into spawned tokio tasks.
pub struct SharedSessionService {
    state: Arc<SharedState>,
    msg_tx: mpsc::Sender<Message>,
}

impl SharedSessionService {
    pub fn new(state: Arc<SharedState>, msg_tx: mpsc::Sender<Message>) -> Self {
        Self { state, msg_tx }
    }
}

impl SessionService for SharedSessionService {
    async fn list_sessions(&self) -> Vec<SessionSnapshot> {
        self.state.sessions.read().await.clone()
    }

    async fn start_session(&self, device_id: &str) -> Result<()> {
        self.msg_tx
            .send(Message::StartSessionOnDevice {
                device_id: device_id.to_string(),
            })
            .await
            .map_err(|_| Error::channel_send("start session command"))
    }

    async fn stop_session(&self, session_id: SessionId) -> Result<()> {
        self.msg_tx
            .send(Message::StopSessionById { session_id })
            .await
            .map_err(|_| Error::channel_send("stop session command"))
    }

    async fn get_devtools_url(&self, session_id: SessionId) -> Option<String> {
        self.state
            .sessions
            .read()
            .await
            .iter()
            .find(|s| s.session_id == session_id)
            .and_then(|s| s.devtools_url.clone())
    }
}

#[cfg(test)]
mod tests {
    // Import only the Send-variant trait: `SharedSessionService` implements
    // both variants (Local via the trait_variant blanket impl), so importing
    // both would make plain method-call syntax ambiguous.
    use super::{SessionService, SessionSnapshot, SharedSessionService};
    use crate::message::Message;
    use crate::services::SharedState;
    use crate::session::SessionId;
    use fdemon_core::AppPhase;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn snapshot(session_id: SessionId, devtools_url: Option<String>) -> SessionSnapshot {
        SessionSnapshot {
            session_id,
            name: "Pixel 6".to_string(),
            device_id: "emulator-5554".to_string(),
            device_name: "Pixel 6".to_string(),
            platform: "android".to_string(),
            phase: AppPhase::Running,
            app_id: Some("app-1".to_string()),
            devtools_url,
        }
    }

    fn create_service() -> (
        SharedSessionService,
        mpsc::Receiver<Message>,
        Arc<SharedState>,
    ) {
        let state = Arc::new(SharedState::new(100));
        let (tx, rx) = mpsc::channel(10);
        let service = SharedSessionService::new(state.clone(), tx);
        (service, rx, state)
    }

    #[tokio::test]
    async fn test_list_sessions_empty_by_default() {
        let (service, _rx, _state) = create_service();
        assert!(service.list_sessions().await.is_empty());
    }

    #[tokio::test]
    async fn test_list_sessions_returns_synced_snapshots() {
        let (service, _rx, state) = create_service();
        *state.sessions.write().await = vec![snapshot(1, None), snapshot(2, None)];

        let sessions = service.list_sessions().await;
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, 1);
        assert_eq!(sessions[0].device_id, "emulator-5554");
        assert_eq!(sessions[0].phase, AppPhase::Running);
    }

    #[tokio::test]
    async fn test_start_session_sends_message() {
        let (service, mut rx, _state) = create_service();

        service.start_session("emulator-5554").await.unwrap();

        match rx.try_recv().unwrap() {
            Message::StartSessionOnDevice { device_id } => {
                assert_eq!(device_id, "emulator-5554");
            }
            other => panic!("expected StartSessionOnDevice, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_stop_session_sends_message() {
        let (service, mut rx, _state) = create_service();

        service.stop_session(7).await.unwrap();

        match rx.try_recv().unwrap() {
            Message::StopSessionById { session_id } => assert_eq!(session_id, 7),
            other => panic!("expected StopSessionById, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_start_session_channel_closed_returns_error() {
        let (service, rx, _state) = create_service();
        drop(rx);

        assert!(service.start_session("dev-1").await.is_err());
        assert!(service.stop_session(1).await.is_err());
    }

    #[tokio::test]
    async fn test_get_devtools_url_for_known_session() {
        let (service, _rx, state) = create_service();
        *state.sessions.write().await = vec![
            snapshot(1, Some("http://127.0.0.1:9100?uri=ws".to_string())),
            snapshot(2, None),
        ];

        assert_eq!(
            service.get_devtools_url(1).await,
            Some("http://127.0.0.1:9100?uri=ws".to_string())
        );
        assert_eq!(service.get_devtools_url(2).await, None);
        assert_eq!(service.get_devtools_url(99).await, None);
    }

    #[tokio::test]
    async fn test_session_service_usable_from_spawned_task() {
        let (service, mut rx, _state) = create_service();

        let handle = tokio::spawn(async move { service.start_session("dev-1").await });
        handle.await.unwrap().unwrap();

        assert!(matches!(
            rx.try_recv().unwrap(),
            Message::StartSessionOnDevice { .. }
        ));
    }
}
