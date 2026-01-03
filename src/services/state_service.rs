//! Shared state management for concurrent access
//!
//! This module provides thread-safe state that can be accessed by both
//! the TUI and future MCP server handlers.

use std::sync::Arc;

use chrono::{DateTime, Duration, Local};
use tokio::sync::{broadcast, RwLock};

use crate::core::{AppPhase, LogEntry};
use crate::daemon::{DaemonMessage, DeviceInfo};

/// Application run state with metadata
#[derive(Debug, Clone, Default)]
pub struct AppRunState {
    pub phase: AppPhase,
    pub app_id: Option<String>,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub platform: Option<String>,
    pub devtools_uri: Option<String>,
    pub started_at: Option<DateTime<Local>>,
    pub last_reload_at: Option<DateTime<Local>>,
}

impl AppRunState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.phase, AppPhase::Running | AppPhase::Reloading)
    }

    pub fn session_duration(&self) -> Option<Duration> {
        self.started_at.map(|start| Local::now() - start)
    }

    /// Reset state when app stops
    pub fn reset(&mut self) {
        self.app_id = None;
        self.device_id = None;
        self.device_name = None;
        self.platform = None;
        self.devtools_uri = None;
        self.started_at = None;
        self.last_reload_at = None;
        self.phase = AppPhase::Initializing;
    }
}

/// Project information (static, doesn't need async)
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub name: String,
    pub path: std::path::PathBuf,
    pub flutter_version: Option<String>,
}

impl ProjectInfo {
    pub fn new(name: impl Into<String>, path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            flutter_version: None,
        }
    }
}

/// Centralized shared state accessible by TUI and future MCP
pub struct SharedState {
    /// Current application run state
    pub app_state: Arc<RwLock<AppRunState>>,

    /// Log buffer
    pub logs: Arc<RwLock<Vec<LogEntry>>>,

    /// Known devices
    pub devices: Arc<RwLock<Vec<DeviceInfo>>>,

    /// Event broadcaster for multiple subscribers
    pub event_tx: broadcast::Sender<DaemonMessage>,

    /// Maximum log buffer size
    pub max_logs: usize,
}

impl SharedState {
    pub fn new(max_logs: usize) -> Self {
        let (event_tx, _) = broadcast::channel(256);

        Self {
            app_state: Arc::new(RwLock::new(AppRunState::new())),
            logs: Arc::new(RwLock::new(Vec::new())),
            devices: Arc::new(RwLock::new(Vec::new())),
            event_tx,
            max_logs,
        }
    }

    /// Subscribe to daemon events
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonMessage> {
        self.event_tx.subscribe()
    }

    /// Broadcast a daemon message to all subscribers
    pub fn broadcast(&self, message: DaemonMessage) {
        // Ignore send errors (no subscribers is fine)
        let _ = self.event_tx.send(message);
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new(10_000)
    }
}

/// StateService trait for querying application state
#[trait_variant::make(StateService: Send)]
pub trait LocalStateService {
    /// Get current application run state
    async fn get_app_state(&self) -> AppRunState;

    /// Get list of available devices
    async fn get_devices(&self) -> Vec<DeviceInfo>;

    /// Get project information
    fn get_project_info(&self) -> ProjectInfo;
}

/// Default implementation using SharedState
pub struct SharedStateService {
    state: Arc<SharedState>,
    project_info: ProjectInfo,
}

impl SharedStateService {
    pub fn new(state: Arc<SharedState>, project_info: ProjectInfo) -> Self {
        Self {
            state,
            project_info,
        }
    }
}

impl LocalStateService for SharedStateService {
    async fn get_app_state(&self) -> AppRunState {
        self.state.app_state.read().await.clone()
    }

    async fn get_devices(&self) -> Vec<DeviceInfo> {
        self.state.devices.read().await.clone()
    }

    fn get_project_info(&self) -> ProjectInfo {
        self.project_info.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::DaemonConnected;

    #[tokio::test]
    async fn test_shared_state_creation() {
        let state = SharedState::new(100);
        assert_eq!(state.max_logs, 100);

        let app_state = state.app_state.read().await;
        assert_eq!(app_state.phase, AppPhase::Initializing);
    }

    #[test]
    fn test_app_run_state_new() {
        let state = AppRunState::new();
        assert_eq!(state.phase, AppPhase::Initializing);
        assert!(state.app_id.is_none());
    }

    #[test]
    fn test_app_run_state_is_running() {
        let mut state = AppRunState::new();
        assert!(!state.is_running());

        state.phase = AppPhase::Running;
        assert!(state.is_running());

        state.phase = AppPhase::Reloading;
        assert!(state.is_running());

        state.phase = AppPhase::Quitting;
        assert!(!state.is_running());

        state.phase = AppPhase::Initializing;
        assert!(!state.is_running());
    }

    #[test]
    fn test_app_run_state_reset() {
        let mut state = AppRunState {
            phase: AppPhase::Running,
            app_id: Some("app123".to_string()),
            device_id: Some("device123".to_string()),
            device_name: Some("iPhone".to_string()),
            platform: Some("ios".to_string()),
            devtools_uri: Some("ws://localhost:8080".to_string()),
            started_at: Some(Local::now()),
            last_reload_at: Some(Local::now()),
        };

        state.reset();

        assert_eq!(state.phase, AppPhase::Initializing);
        assert!(state.app_id.is_none());
        assert!(state.device_id.is_none());
        assert!(state.device_name.is_none());
        assert!(state.platform.is_none());
        assert!(state.devtools_uri.is_none());
        assert!(state.started_at.is_none());
        assert!(state.last_reload_at.is_none());
    }

    #[test]
    fn test_session_duration() {
        let mut state = AppRunState::new();
        assert!(state.session_duration().is_none());

        state.started_at = Some(Local::now() - Duration::seconds(60));
        let duration = state.session_duration().unwrap();
        assert!(duration.num_seconds() >= 60);
    }

    #[tokio::test]
    async fn test_event_broadcast() {
        let state = SharedState::new(100);

        let mut rx1 = state.subscribe();
        let mut rx2 = state.subscribe();

        // Create a test message
        let msg = DaemonMessage::DaemonConnected(DaemonConnected {
            version: "1.0".to_string(),
            pid: 123,
        });

        state.broadcast(msg);

        // Both subscribers should receive
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_broadcast_no_subscribers() {
        let state = SharedState::new(100);

        // Broadcasting with no subscribers should not panic
        let msg = DaemonMessage::DaemonConnected(DaemonConnected {
            version: "1.0".to_string(),
            pid: 123,
        });

        state.broadcast(msg);
        // No panic = success
    }

    #[test]
    fn test_project_info() {
        let info = ProjectInfo::new("my_app", "/path/to/app");
        assert_eq!(info.name, "my_app");
        assert_eq!(info.path, std::path::PathBuf::from("/path/to/app"));
        assert!(info.flutter_version.is_none());
    }

    #[tokio::test]
    async fn test_shared_state_service() {
        let state = Arc::new(SharedState::new(100));
        let project_info = ProjectInfo::new("test_app", "/test/path");
        let service = SharedStateService::new(state.clone(), project_info.clone());

        // Test get_project_info
        let info = service.get_project_info();
        assert_eq!(info.name, "test_app");

        // Test get_app_state
        let app_state = service.get_app_state().await;
        assert_eq!(app_state.phase, AppPhase::Initializing);

        // Test get_devices
        let devices = service.get_devices().await;
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_shared_state_default() {
        let state = SharedState::default();
        assert_eq!(state.max_logs, 10_000);
    }
}
