//! Flutter app control operations
//!
//! This module provides the FlutterController trait for controlling a running
//! Flutter application. Both the TUI and future MCP handlers use this trait.

use std::sync::Arc;

use tokio::sync::mpsc;

use super::state_service::SharedState;
use crate::common::prelude::*;
use crate::daemon::{CommandSender, DaemonCommand};

/// Result of a reload operation
#[derive(Debug, Clone)]
pub struct ReloadResult {
    pub success: bool,
    pub time_ms: Option<u64>,
    pub message: Option<String>,
}

impl ReloadResult {
    pub fn success(time_ms: Option<u64>) -> Self {
        Self {
            success: true,
            time_ms,
            message: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            time_ms: None,
            message: Some(message.into()),
        }
    }
}

/// Result of a restart operation
#[derive(Debug, Clone)]
pub struct RestartResult {
    pub success: bool,
    pub message: Option<String>,
}

impl RestartResult {
    pub fn success() -> Self {
        Self {
            success: true,
            message: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
        }
    }
}

/// Commands that can be sent to the daemon
#[derive(Debug, Clone)]
pub enum FlutterCommand {
    Reload,
    Restart,
    Stop,
    TogglePlatform,
    ToggleDebugPainting,
    Screenshot,
}

/// Flutter app control operations
///
/// Both TUI and future MCP handlers use this trait.
#[trait_variant::make(FlutterController: Send)]
pub trait LocalFlutterController {
    /// Hot reload the running app
    async fn reload(&self) -> Result<ReloadResult>;

    /// Hot restart the running app
    async fn restart(&self) -> Result<RestartResult>;

    /// Stop the running app
    async fn stop(&self) -> Result<()>;

    /// Check if an app is currently running
    async fn is_running(&self) -> bool;

    /// Get the current app ID
    async fn get_app_id(&self) -> Option<String>;
}

/// Implementation using daemon process
pub struct DaemonFlutterController {
    /// Channel to send commands to the daemon
    command_tx: mpsc::Sender<FlutterCommand>,
    /// Shared state for reading current state
    state: Arc<SharedState>,
}

impl DaemonFlutterController {
    pub fn new(command_tx: mpsc::Sender<FlutterCommand>, state: Arc<SharedState>) -> Self {
        Self { command_tx, state }
    }
}

impl LocalFlutterController for DaemonFlutterController {
    async fn reload(&self) -> Result<ReloadResult> {
        self.command_tx
            .send(FlutterCommand::Reload)
            .await
            .map_err(|_| Error::channel_send("reload command"))?;

        // Actual result tracking will be implemented in Task 03
        Ok(ReloadResult {
            success: true,
            time_ms: None,
            message: Some("Reload requested".to_string()),
        })
    }

    async fn restart(&self) -> Result<RestartResult> {
        self.command_tx
            .send(FlutterCommand::Restart)
            .await
            .map_err(|_| Error::channel_send("restart command"))?;

        Ok(RestartResult {
            success: true,
            message: Some("Restart requested".to_string()),
        })
    }

    async fn stop(&self) -> Result<()> {
        self.command_tx
            .send(FlutterCommand::Stop)
            .await
            .map_err(|_| Error::channel_send("stop command"))?;
        Ok(())
    }

    async fn is_running(&self) -> bool {
        self.state.app_state.read().await.is_running()
    }

    async fn get_app_id(&self) -> Option<String> {
        self.state.app_state.read().await.app_id.clone()
    }
}

/// Implementation using CommandSender for proper request/response tracking
///
/// This implementation sends commands directly to the daemon with request ID
/// tracking and response matching.
pub struct CommandSenderController {
    /// Command sender for daemon communication
    sender: CommandSender,
    /// Shared state for reading current state
    state: Arc<SharedState>,
}

impl CommandSenderController {
    pub fn new(sender: CommandSender, state: Arc<SharedState>) -> Self {
        Self { sender, state }
    }
}

impl LocalFlutterController for CommandSenderController {
    async fn reload(&self) -> Result<ReloadResult> {
        let app_id = self
            .state
            .app_state
            .read()
            .await
            .app_id
            .clone()
            .ok_or_else(|| Error::daemon("No app running"))?;

        let response = self.sender.send(DaemonCommand::Reload { app_id }).await?;

        if response.success {
            // Extract reload time from response if available
            let time_ms = response
                .result
                .as_ref()
                .and_then(|r| r.get("code"))
                .and_then(|c| c.as_u64());

            Ok(ReloadResult::success(time_ms))
        } else {
            Ok(ReloadResult::failure(
                response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn restart(&self) -> Result<RestartResult> {
        let app_id = self
            .state
            .app_state
            .read()
            .await
            .app_id
            .clone()
            .ok_or_else(|| Error::daemon("No app running"))?;

        let response = self.sender.send(DaemonCommand::Restart { app_id }).await?;

        if response.success {
            Ok(RestartResult::success())
        } else {
            Ok(RestartResult::failure(
                response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn stop(&self) -> Result<()> {
        let app_id = self
            .state
            .app_state
            .read()
            .await
            .app_id
            .clone()
            .ok_or_else(|| Error::daemon("No app running"))?;

        let response = self.sender.send(DaemonCommand::Stop { app_id }).await?;

        if response.success {
            Ok(())
        } else {
            Err(Error::daemon(
                response
                    .error
                    .unwrap_or_else(|| "Failed to stop app".to_string()),
            ))
        }
    }

    async fn is_running(&self) -> bool {
        self.state.app_state.read().await.is_running()
    }

    async fn get_app_id(&self) -> Option<String> {
        self.state.app_state.read().await.app_id.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AppPhase;

    #[test]
    fn test_reload_result_success() {
        let result = ReloadResult::success(Some(250));
        assert!(result.success);
        assert_eq!(result.time_ms, Some(250));
        assert!(result.message.is_none());
    }

    #[test]
    fn test_reload_result_failure() {
        let result = ReloadResult::failure("Compile error");
        assert!(!result.success);
        assert!(result.time_ms.is_none());
        assert_eq!(result.message, Some("Compile error".to_string()));
    }

    #[test]
    fn test_restart_result_success() {
        let result = RestartResult::success();
        assert!(result.success);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_restart_result_failure() {
        let result = RestartResult::failure("Failed to restart");
        assert!(!result.success);
        assert_eq!(result.message, Some("Failed to restart".to_string()));
    }

    #[test]
    fn test_flutter_command_variants() {
        // Just ensure all variants exist and can be created
        let _reload = FlutterCommand::Reload;
        let _restart = FlutterCommand::Restart;
        let _stop = FlutterCommand::Stop;
        let _toggle_platform = FlutterCommand::TogglePlatform;
        let _toggle_debug = FlutterCommand::ToggleDebugPainting;
        let _screenshot = FlutterCommand::Screenshot;
    }

    #[tokio::test]
    async fn test_daemon_controller_reload() {
        let (tx, mut rx) = mpsc::channel(10);
        let state = Arc::new(SharedState::new(100));
        let controller = DaemonFlutterController::new(tx, state);

        let result = controller.reload().await;
        assert!(result.is_ok());

        // Check command was sent
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, FlutterCommand::Reload));
    }

    #[tokio::test]
    async fn test_daemon_controller_restart() {
        let (tx, mut rx) = mpsc::channel(10);
        let state = Arc::new(SharedState::new(100));
        let controller = DaemonFlutterController::new(tx, state);

        let result = controller.restart().await;
        assert!(result.is_ok());

        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, FlutterCommand::Restart));
    }

    #[tokio::test]
    async fn test_daemon_controller_stop() {
        let (tx, mut rx) = mpsc::channel(10);
        let state = Arc::new(SharedState::new(100));
        let controller = DaemonFlutterController::new(tx, state);

        let result = controller.stop().await;
        assert!(result.is_ok());

        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, FlutterCommand::Stop));
    }

    #[tokio::test]
    async fn test_daemon_controller_is_running() {
        let (tx, _rx) = mpsc::channel(10);
        let state = Arc::new(SharedState::new(100));
        let controller = DaemonFlutterController::new(tx, state.clone());

        // Initially not running
        assert!(!controller.is_running().await);

        // Set to running
        state.app_state.write().await.phase = AppPhase::Running;
        assert!(controller.is_running().await);
    }

    #[tokio::test]
    async fn test_daemon_controller_get_app_id() {
        let (tx, _rx) = mpsc::channel(10);
        let state = Arc::new(SharedState::new(100));
        let controller = DaemonFlutterController::new(tx, state.clone());

        // Initially no app_id
        assert!(controller.get_app_id().await.is_none());

        // Set app_id
        state.app_state.write().await.app_id = Some("test-app-123".to_string());
        assert_eq!(
            controller.get_app_id().await,
            Some("test-app-123".to_string())
        );
    }

    #[tokio::test]
    async fn test_daemon_controller_channel_closed() {
        let (tx, rx) = mpsc::channel(10);
        let state = Arc::new(SharedState::new(100));
        let controller = DaemonFlutterController::new(tx, state);

        // Drop receiver to close channel
        drop(rx);

        // Should return error
        let result = controller.reload().await;
        assert!(result.is_err());
    }
}
