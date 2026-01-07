//! Mock Flutter daemon for integration testing
//!
//! Simulates the Flutter daemon's JSON-RPC protocol without
//! requiring an actual Flutter installation.

use flutter_demon::core::DaemonEvent;
use flutter_demon::daemon::DaemonCommand;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

/// Handle for interacting with the mock daemon from tests
pub struct MockDaemonHandle {
    /// Send commands to the daemon
    pub cmd_tx: mpsc::Sender<String>,
    /// Receive events from the daemon
    pub event_rx: mpsc::Receiver<DaemonEvent>,
    /// Control channel to configure mock behavior
    control_tx: mpsc::Sender<MockControl>,
}

impl MockDaemonHandle {
    /// Send a raw JSON command to the mock daemon
    pub async fn send_command(&self, json: &str) -> Result<(), mpsc::error::SendError<String>> {
        self.cmd_tx.send(json.to_string()).await
    }

    /// Send a typed DaemonCommand
    pub async fn send(&self, cmd: DaemonCommand) -> Result<(), mpsc::error::SendError<String>> {
        let id = 1; // Mock doesn't track IDs
        let json = cmd.build(id);
        self.send_command(&format!("[{}]", json)).await
    }

    /// Receive the next event (with timeout)
    pub async fn recv_event(&mut self) -> Option<DaemonEvent> {
        tokio::time::timeout(Duration::from_secs(1), self.event_rx.recv())
            .await
            .ok()
            .flatten()
    }

    /// Receive the next event, expecting it to be a specific type
    pub async fn expect_stdout(&mut self) -> Option<String> {
        match self.recv_event().await? {
            DaemonEvent::Stdout(line) => Some(line),
            _ => None,
        }
    }

    /// Configure the mock to return specific responses
    pub async fn set_response(&self, method: &str, response: serde_json::Value) {
        let _ = self
            .control_tx
            .send(MockControl::SetResponse {
                method: method.to_string(),
                response,
            })
            .await;
    }

    /// Queue an event to be sent
    pub async fn queue_event(&self, event: DaemonEvent) {
        let _ = self.control_tx.send(MockControl::QueueEvent(event)).await;
    }
}

/// Control messages for configuring mock behavior
#[allow(dead_code)]
enum MockControl {
    SetResponse {
        method: String,
        response: serde_json::Value,
    },
    QueueEvent(DaemonEvent),
    Shutdown,
}

/// Mock Flutter daemon for testing
pub struct MockFlutterDaemon {
    /// Channel to receive commands from test
    cmd_rx: mpsc::Receiver<String>,
    /// Channel to send events to test
    event_tx: mpsc::Sender<DaemonEvent>,
    /// Channel to receive control messages
    control_rx: mpsc::Receiver<MockControl>,
    /// Configured responses for methods
    responses: HashMap<String, serde_json::Value>,
    /// Queued events to send
    event_queue: Vec<DaemonEvent>,
    /// App ID for this mock session
    app_id: String,
}

impl MockFlutterDaemon {
    /// Create a new mock daemon and its handle
    pub fn new() -> (Self, MockDaemonHandle) {
        Self::with_app_id("test-app-001")
    }

    /// Create a new mock daemon with specific app ID
    pub fn with_app_id(app_id: &str) -> (Self, MockDaemonHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (event_tx, event_rx) = mpsc::channel(32);
        let (control_tx, control_rx) = mpsc::channel(32);

        let daemon = MockFlutterDaemon {
            cmd_rx,
            event_tx,
            control_rx,
            responses: HashMap::new(),
            event_queue: Vec::new(),
            app_id: app_id.to_string(),
        };

        let handle = MockDaemonHandle {
            cmd_tx,
            event_rx,
            control_tx,
        };

        (daemon, handle)
    }

    /// Run the mock daemon event loop
    pub async fn run(mut self) {
        // Send initial daemon.connected event
        self.send_daemon_connected().await;

        loop {
            tokio::select! {
                // Handle incoming commands
                Some(cmd) = self.cmd_rx.recv() => {
                    if !self.handle_command(&cmd).await {
                        break;
                    }
                }
                // Handle control messages
                Some(ctrl) = self.control_rx.recv() => {
                    match ctrl {
                        MockControl::SetResponse { method, response } => {
                            self.responses.insert(method, response);
                        }
                        MockControl::QueueEvent(event) => {
                            self.event_queue.push(event);
                        }
                        MockControl::Shutdown => break,
                    }
                }
                // Send queued events
                _ = tokio::time::sleep(Duration::from_millis(10)), if !self.event_queue.is_empty() => {
                    // Use remove(0) to maintain FIFO order
                    let event = self.event_queue.remove(0);
                    let _ = self.event_tx.send(event).await;
                }
                else => break,
            }
        }
    }

    /// Send daemon.connected event
    async fn send_daemon_connected(&self) {
        let json = r#"{"event":"daemon.connected","params":{"version":"0.6.1","pid":99999}}"#;
        let _ = self
            .event_tx
            .send(DaemonEvent::Stdout(format!("[{}]", json)))
            .await;
    }

    /// Handle an incoming command
    async fn handle_command(&mut self, cmd: &str) -> bool {
        // Strip brackets if present
        let json = cmd.trim().trim_start_matches('[').trim_end_matches(']');

        let parsed: serde_json::Value = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(_) => return true, // Ignore malformed commands
        };

        let method = parsed["method"].as_str().unwrap_or("");
        let id = &parsed["id"];

        match method {
            "app.restart" => {
                self.handle_reload(
                    id,
                    parsed["params"]["fullRestart"].as_bool().unwrap_or(false),
                )
                .await;
            }
            "app.stop" => {
                self.handle_stop(id).await;
            }
            "daemon.shutdown" => {
                self.send_response(id, json!({"code": 0})).await;
                return false;
            }
            "device.getDevices" => {
                self.handle_get_devices(id).await;
            }
            "device.enable" => {
                self.send_response(id, json!(null)).await;
            }
            _ => {
                // Check for configured response
                if let Some(response) = self.responses.get(method).cloned() {
                    self.send_response(id, response).await;
                }
            }
        }

        true
    }

    /// Handle hot reload/restart command
    async fn handle_reload(&self, id: &serde_json::Value, full_restart: bool) {
        let action = if full_restart { "restart" } else { "reload" };

        // Send progress start
        let progress_start = json!({
            "event": "app.progress",
            "params": {
                "appId": self.app_id,
                "id": format!("hot.{}", action),
                "message": format!("Performing hot {}...", action),
                "finished": false
            }
        });
        self.send_event(&progress_start).await;

        // Small delay to simulate reload time
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Send progress complete
        let progress_done = json!({
            "event": "app.progress",
            "params": {
                "appId": self.app_id,
                "id": format!("hot.{}", action),
                "message": "Reloaded 1 of 1 libraries in 50ms.",
                "finished": true
            }
        });
        self.send_event(&progress_done).await;

        // Send response
        self.send_response(id, json!({"code": 0, "message": "ok"}))
            .await;
    }

    /// Handle app.stop command
    async fn handle_stop(&self, id: &serde_json::Value) {
        let stop_event = json!({
            "event": "app.stop",
            "params": {
                "appId": self.app_id
            }
        });
        self.send_event(&stop_event).await;
        self.send_response(id, json!({"code": 0})).await;
    }

    /// Handle device.getDevices command
    async fn handle_get_devices(&self, id: &serde_json::Value) {
        let devices = json!([
            {
                "id": "emulator-5554",
                "name": "Android SDK built for x86",
                "platform": "android",
                "emulator": true
            }
        ]);
        self.send_response(id, devices).await;
    }

    /// Send a JSON event as stdout
    async fn send_event(&self, event: &serde_json::Value) {
        let json = serde_json::to_string(event).unwrap();
        let _ = self
            .event_tx
            .send(DaemonEvent::Stdout(format!("[{}]", json)))
            .await;
    }

    /// Send a response to a request
    async fn send_response(&self, id: &serde_json::Value, result: serde_json::Value) {
        let response = json!({
            "id": id,
            "result": result
        });
        let json = serde_json::to_string(&response).unwrap();
        let _ = self
            .event_tx
            .send(DaemonEvent::Stdout(format!("[{}]", json)))
            .await;
    }
}

/// Builder for creating mock daemon scenarios
pub struct MockScenarioBuilder {
    app_id: String,
    initial_events: Vec<serde_json::Value>,
    responses: HashMap<String, serde_json::Value>,
}

impl MockScenarioBuilder {
    pub fn new() -> Self {
        Self {
            app_id: "test-app-001".to_string(),
            initial_events: Vec::new(),
            responses: HashMap::new(),
        }
    }

    /// Set the app ID for this scenario
    pub fn with_app_id(mut self, id: &str) -> Self {
        self.app_id = id.to_string();
        self
    }

    /// Add app.start sequence to initial events
    pub fn with_app_started(mut self) -> Self {
        self.initial_events.push(json!({
            "event": "app.start",
            "params": {
                "appId": self.app_id,
                "deviceId": "emulator-5554",
                "directory": "/test/project",
                "supportsRestart": true
            }
        }));
        self.initial_events.push(json!({
            "event": "app.started",
            "params": {
                "appId": self.app_id
            }
        }));
        self
    }

    /// Add a custom response for a method
    pub fn with_response(mut self, method: &str, response: serde_json::Value) -> Self {
        self.responses.insert(method.to_string(), response);
        self
    }

    /// Build the mock daemon
    pub fn build(self) -> (MockFlutterDaemon, MockDaemonHandle) {
        let (mut daemon, handle) = MockFlutterDaemon::with_app_id(&self.app_id);
        daemon.responses = self.responses;

        // Queue initial events
        for event in self.initial_events {
            daemon.event_queue.push(DaemonEvent::Stdout(format!(
                "[{}]",
                serde_json::to_string(&event).unwrap()
            )));
        }

        (daemon, handle)
    }
}

impl Default for MockScenarioBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_daemon_sends_connected() {
        let (daemon, mut handle) = MockFlutterDaemon::new();
        tokio::spawn(daemon.run());

        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("daemon.connected")));
    }

    #[tokio::test]
    async fn test_mock_daemon_handles_reload() {
        let (daemon, mut handle) = MockFlutterDaemon::new();
        tokio::spawn(daemon.run());

        // Skip connected event
        handle.recv_event().await;

        // Send reload command
        handle
            .send(DaemonCommand::Reload {
                app_id: "test-app-001".into(),
            })
            .await
            .unwrap();

        // Should receive progress events
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("app.progress")));
    }

    #[tokio::test]
    async fn test_mock_daemon_handles_restart() {
        let (daemon, mut handle) = MockFlutterDaemon::new();
        tokio::spawn(daemon.run());

        // Skip connected event
        handle.recv_event().await;

        // Send restart command
        handle
            .send(DaemonCommand::Restart {
                app_id: "test-app-001".into(),
            })
            .await
            .unwrap();

        // Should receive progress events
        let event = handle.recv_event().await;
        assert!(
            matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("app.progress") && s.contains("restart"))
        );
    }

    #[tokio::test]
    async fn test_mock_daemon_handles_stop() {
        let (daemon, mut handle) = MockFlutterDaemon::new();
        tokio::spawn(daemon.run());

        // Skip connected event
        handle.recv_event().await;

        // Send stop command
        handle
            .send(DaemonCommand::Stop {
                app_id: "test-app-001".into(),
            })
            .await
            .unwrap();

        // Should receive app.stop event
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("app.stop")));
    }

    #[tokio::test]
    async fn test_mock_daemon_handles_get_devices() {
        let (daemon, mut handle) = MockFlutterDaemon::new();
        tokio::spawn(daemon.run());

        // Skip connected event
        handle.recv_event().await;

        // Send get devices command
        handle.send(DaemonCommand::GetDevices).await.unwrap();

        // Should receive devices list
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("emulator-5554")));
    }

    #[tokio::test]
    async fn test_mock_daemon_shutdown() {
        let (daemon, mut handle) = MockFlutterDaemon::new();
        tokio::spawn(daemon.run());

        // Skip connected event
        handle.recv_event().await;

        // Send shutdown command
        handle.send(DaemonCommand::Shutdown).await.unwrap();

        // Should receive response
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("\"code\":0")));
    }

    #[tokio::test]
    async fn test_mock_daemon_custom_response() {
        let (daemon, mut handle) = MockFlutterDaemon::new();

        // Set up custom response
        handle
            .set_response("custom.method", json!({"status": "ok"}))
            .await;

        tokio::spawn(daemon.run());

        // Skip connected event
        handle.recv_event().await;

        // Send custom command
        let cmd = r#"{"id":1,"method":"custom.method","params":{}}"#;
        handle.send_command(&format!("[{}]", cmd)).await.unwrap();

        // Should receive custom response
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("\"status\":\"ok\"")));
    }

    #[tokio::test]
    async fn test_mock_scenario_builder_basic() {
        let (daemon, mut handle) = MockScenarioBuilder::new()
            .with_app_id("scenario-app-001")
            .build();

        tokio::spawn(daemon.run());

        // Should receive daemon.connected
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("daemon.connected")));
    }

    #[tokio::test]
    async fn test_mock_scenario_builder_with_app_started() {
        let (daemon, mut handle) = MockScenarioBuilder::new()
            .with_app_id("scenario-app-002")
            .with_app_started()
            .build();

        tokio::spawn(daemon.run());

        // Skip daemon.connected
        handle.recv_event().await;

        // Should receive app.start
        let event = handle.recv_event().await;
        assert!(
            matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("app.start") && s.contains("scenario-app-002"))
        );

        // Should receive app.started
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("app.started")));
    }

    #[tokio::test]
    async fn test_mock_scenario_builder_with_custom_response() {
        let (daemon, mut handle) = MockScenarioBuilder::new()
            .with_response("test.method", json!({"result": "test"}))
            .build();

        tokio::spawn(daemon.run());

        // Skip daemon.connected
        handle.recv_event().await;

        // Send custom command
        let cmd = r#"{"id":1,"method":"test.method","params":{}}"#;
        handle.send_command(&format!("[{}]", cmd)).await.unwrap();

        // Should receive custom response
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("\"result\":\"test\"")));
    }

    #[tokio::test]
    async fn test_mock_daemon_queue_event() {
        let (daemon, mut handle) = MockFlutterDaemon::new();
        tokio::spawn(daemon.run());

        // Skip daemon.connected
        handle.recv_event().await;

        // Queue a custom event
        handle
            .queue_event(DaemonEvent::Stderr("Test error".to_string()))
            .await;

        // Should receive the queued event
        let event = handle.recv_event().await;
        assert!(matches!(event, Some(DaemonEvent::Stderr(s)) if s == "Test error"));
    }

    #[tokio::test]
    async fn test_mock_daemon_expect_stdout() {
        let (daemon, mut handle) = MockFlutterDaemon::new();
        tokio::spawn(daemon.run());

        // Should receive daemon.connected as stdout
        let stdout = handle.expect_stdout().await;
        assert!(stdout.is_some());
        assert!(stdout.unwrap().contains("daemon.connected"));
    }
}
