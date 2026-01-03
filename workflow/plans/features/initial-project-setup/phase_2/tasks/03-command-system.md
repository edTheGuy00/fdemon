## Task: Command Injection System

**Objective**: Implement request ID tracking, response matching with timeout, and a `send_command()` abstraction for bidirectional daemon communication.

**Depends on**: 02-service-layer

---

### Scope

- `src/daemon/commands.rs`: **NEW** - Command building and request tracking
- `src/daemon/process.rs`: MODIFY - Wire command sending through service layer
- `src/daemon/mod.rs`: MODIFY - Re-export new types
- `src/services/flutter_controller.rs`: MODIFY - Complete implementation with response tracking

---

### Implementation Details

#### Problem Statement

Currently, `FlutterProcess::send_json()` sends raw JSON to the daemon but:
1. No request ID tracking
2. No way to match responses to requests
3. No timeout handling for stalled commands
4. No structured command building

This task creates a proper request/response system.

#### Request ID Tracking

```rust
// src/daemon/commands.rs

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Global request ID counter
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique request ID
pub fn next_request_id() -> u64 {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// A pending request awaiting response
struct PendingRequest {
    /// Channel to send the response
    response_tx: oneshot::Sender<CommandResponse>,
    /// When this request was created
    created_at: std::time::Instant,
    /// Description for logging
    description: String,
}

/// Response from a command
#[derive(Debug, Clone)]
pub struct CommandResponse {
    pub id: u64,
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

impl CommandResponse {
    pub fn from_daemon_response(id: u64, result: Option<Value>, error: Option<Value>) -> Self {
        Self {
            id,
            success: error.is_none(),
            result,
            error: error.map(|e| e.to_string()),
        }
    }
}

/// Tracks pending requests and matches responses
pub struct RequestTracker {
    /// Map of request ID to pending request
    pending: Arc<RwLock<HashMap<u64, PendingRequest>>>,
    /// Default timeout for requests
    default_timeout: Duration,
}

impl RequestTracker {
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            default_timeout,
        }
    }

    /// Register a new pending request
    /// Returns (request_id, receiver for response)
    pub async fn register(&self, description: &str) -> (u64, oneshot::Receiver<CommandResponse>) {
        let id = next_request_id();
        let (tx, rx) = oneshot::channel();

        let pending = PendingRequest {
            response_tx: tx,
            created_at: std::time::Instant::now(),
            description: description.to_string(),
        };

        self.pending.write().await.insert(id, pending);

        (id, rx)
    }

    /// Handle an incoming response from the daemon
    /// Returns true if the response was matched to a pending request
    pub async fn handle_response(&self, id: u64, result: Option<Value>, error: Option<Value>) -> bool {
        if let Some(pending) = self.pending.write().await.remove(&id) {
            let response = CommandResponse::from_daemon_response(id, result, error);
            let _ = pending.response_tx.send(response);
            true
        } else {
            false
        }
    }

    /// Cancel all pending requests (e.g., on shutdown)
    pub async fn cancel_all(&self) {
        let mut pending = self.pending.write().await;
        for (id, req) in pending.drain() {
            let _ = req.response_tx.send(CommandResponse {
                id,
                success: false,
                result: None,
                error: Some("Request cancelled".to_string()),
            });
        }
    }

    /// Remove stale requests that have timed out
    pub async fn cleanup_stale(&self, timeout: Duration) -> Vec<u64> {
        let mut pending = self.pending.write().await;
        let now = std::time::Instant::now();
        
        let stale: Vec<u64> = pending
            .iter()
            .filter(|(_, req)| now.duration_since(req.created_at) > timeout)
            .map(|(id, _)| *id)
            .collect();

        for id in &stale {
            if let Some(req) = pending.remove(id) {
                let _ = req.response_tx.send(CommandResponse {
                    id: *id,
                    success: false,
                    result: None,
                    error: Some("Request timed out".to_string()),
                });
            }
        }

        stale
    }

    /// Get the number of pending requests
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }
}

impl Default for RequestTracker {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}
```

#### Command Builder

```rust
// src/daemon/commands.rs (continued)

use serde_json::json;

/// Flutter daemon command types
#[derive(Debug, Clone)]
pub enum DaemonCommand {
    /// Hot reload: app.restart with fullRestart=false
    Reload { app_id: String },
    /// Hot restart: app.restart with fullRestart=true
    Restart { app_id: String },
    /// Stop the app
    Stop { app_id: String },
    /// Get daemon version
    Version,
    /// Shutdown daemon
    Shutdown,
    /// Enable device polling
    EnableDevices,
    /// Get list of devices
    GetDevices,
    /// Take screenshot
    Screenshot { app_id: String },
    /// Toggle debug painting
    ToggleDebugPaint { app_id: String },
    /// Toggle platform (iOS/Android)
    TogglePlatform { app_id: String },
}

impl DaemonCommand {
    /// Build the JSON-RPC request object
    pub fn build(&self, id: u64) -> String {
        let (method, params) = match self {
            DaemonCommand::Reload { app_id } => (
                "app.restart",
                json!({ "appId": app_id, "fullRestart": false, "pause": false }),
            ),
            DaemonCommand::Restart { app_id } => (
                "app.restart",
                json!({ "appId": app_id, "fullRestart": true, "pause": false }),
            ),
            DaemonCommand::Stop { app_id } => (
                "app.stop",
                json!({ "appId": app_id }),
            ),
            DaemonCommand::Version => (
                "daemon.version",
                json!({}),
            ),
            DaemonCommand::Shutdown => (
                "daemon.shutdown",
                json!({}),
            ),
            DaemonCommand::EnableDevices => (
                "device.enable",
                json!({}),
            ),
            DaemonCommand::GetDevices => (
                "device.getDevices",
                json!({}),
            ),
            DaemonCommand::Screenshot { app_id } => (
                "app.screenshot",
                json!({ "appId": app_id }),
            ),
            DaemonCommand::ToggleDebugPaint { app_id } => (
                "ext.flutter.debugPaint",
                json!({ "appId": app_id }),
            ),
            DaemonCommand::TogglePlatform { app_id } => (
                "ext.flutter.platformOverride",
                json!({ "appId": app_id }),
            ),
        };

        json!({
            "id": id,
            "method": method,
            "params": params,
        }).to_string()
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            DaemonCommand::Reload { .. } => "hot reload",
            DaemonCommand::Restart { .. } => "hot restart",
            DaemonCommand::Stop { .. } => "stop app",
            DaemonCommand::Version => "get version",
            DaemonCommand::Shutdown => "shutdown daemon",
            DaemonCommand::EnableDevices => "enable devices",
            DaemonCommand::GetDevices => "get devices",
            DaemonCommand::Screenshot { .. } => "screenshot",
            DaemonCommand::ToggleDebugPaint { .. } => "toggle debug paint",
            DaemonCommand::TogglePlatform { .. } => "toggle platform",
        }
    }
}
```

#### Command Sender

```rust
// src/daemon/commands.rs (continued)

use tokio::sync::mpsc;
use crate::common::prelude::*;

/// Sends commands to the daemon process with request tracking
pub struct CommandSender {
    /// Channel to send raw JSON to the daemon's stdin
    stdin_tx: mpsc::Sender<String>,
    /// Request tracker for response matching
    tracker: Arc<RequestTracker>,
}

impl CommandSender {
    pub fn new(stdin_tx: mpsc::Sender<String>, tracker: Arc<RequestTracker>) -> Self {
        Self { stdin_tx, tracker }
    }

    /// Send a command and wait for response
    pub async fn send(&self, command: DaemonCommand) -> Result<CommandResponse> {
        self.send_with_timeout(command, Duration::from_secs(30)).await
    }

    /// Send a command with custom timeout
    pub async fn send_with_timeout(
        &self,
        command: DaemonCommand,
        timeout: Duration,
    ) -> Result<CommandResponse> {
        // Register the pending request
        let (id, response_rx) = self.tracker.register(command.description()).await;

        // Build and send the JSON
        let json = command.build(id);
        let wrapped = format!("[{}]", json);

        tracing::debug!("Sending command #{}: {}", id, command.description());

        self.stdin_tx
            .send(wrapped)
            .await
            .map_err(|_| Error::channel_send("daemon stdin"))?;

        // Wait for response with timeout
        match tokio::time::timeout(timeout, response_rx).await {
            Ok(Ok(response)) => {
                tracing::debug!("Command #{} completed: success={}", id, response.success);
                Ok(response)
            }
            Ok(Err(_)) => {
                // Channel closed (request was cancelled)
                Err(Error::process("Command cancelled".to_string()))
            }
            Err(_) => {
                // Timeout - cleanup the pending request
                self.tracker.cleanup_stale(Duration::ZERO).await;
                Err(Error::process(format!(
                    "Command '{}' timed out after {:?}",
                    command.description(),
                    timeout
                )))
            }
        }
    }

    /// Send a fire-and-forget command (no response expected)
    pub async fn send_fire_and_forget(&self, command: DaemonCommand) -> Result<()> {
        let id = next_request_id();
        let json = command.build(id);
        let wrapped = format!("[{}]", json);

        tracing::debug!("Sending fire-and-forget #{}: {}", id, command.description());

        self.stdin_tx
            .send(wrapped)
            .await
            .map_err(|_| Error::channel_send("daemon stdin"))
    }

    /// Get the request tracker (for response handling)
    pub fn tracker(&self) -> &Arc<RequestTracker> {
        &self.tracker
    }
}
```

---

### Acceptance Criteria

1. [ ] `RequestTracker` tracks pending requests by ID
2. [ ] Request IDs are globally unique (atomic counter)
3. [ ] `handle_response()` matches responses to pending requests
4. [ ] Unmatched responses are logged but don't cause errors
5. [ ] `cleanup_stale()` removes and notifies timed-out requests
6. [ ] `DaemonCommand` enum covers reload, restart, stop, version
7. [ ] `build()` generates valid JSON-RPC format
8. [ ] Commands are wrapped in `[...]` brackets per daemon protocol
9. [ ] `send()` waits for response with 30s default timeout
10. [ ] `send_fire_and_forget()` doesn't wait for response
11. [ ] `cancel_all()` cleans up on shutdown
12. [ ] All new types re-exported from `daemon/mod.rs`
13. [ ] Unit tests for request tracking
14. [ ] Unit tests for command building

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_uniqueness() {
        let id1 = next_request_id();
        let id2 = next_request_id();
        let id3 = next_request_id();
        
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert!(id2 > id1);
        assert!(id3 > id2);
    }

    #[tokio::test]
    async fn test_request_tracker_register() {
        let tracker = RequestTracker::default();
        
        let (id1, _rx1) = tracker.register("test1").await;
        let (id2, _rx2) = tracker.register("test2").await;
        
        assert_ne!(id1, id2);
        assert_eq!(tracker.pending_count().await, 2);
    }

    #[tokio::test]
    async fn test_request_tracker_handle_response() {
        let tracker = RequestTracker::default();
        
        let (id, rx) = tracker.register("test").await;
        
        // Simulate response
        let matched = tracker.handle_response(id, Some(json!({"ok": true})), None).await;
        assert!(matched);
        
        // Receive the response
        let response = rx.await.unwrap();
        assert!(response.success);
        assert!(response.result.is_some());
    }

    #[tokio::test]
    async fn test_request_tracker_unmatched_response() {
        let tracker = RequestTracker::default();
        
        // Try to handle a response for non-existent request
        let matched = tracker.handle_response(9999, Some(json!({})), None).await;
        assert!(!matched);
    }

    #[tokio::test]
    async fn test_request_tracker_cleanup_stale() {
        let tracker = RequestTracker::new(Duration::from_millis(10));
        
        let (_id, _rx) = tracker.register("test").await;
        
        // Wait for it to become stale
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        let stale = tracker.cleanup_stale(Duration::from_millis(10)).await;
        assert_eq!(stale.len(), 1);
        assert_eq!(tracker.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_request_tracker_cancel_all() {
        let tracker = RequestTracker::default();
        
        let (_id1, rx1) = tracker.register("test1").await;
        let (_id2, rx2) = tracker.register("test2").await;
        
        tracker.cancel_all().await;
        
        assert_eq!(tracker.pending_count().await, 0);
        
        // Receivers should get cancellation responses
        let resp1 = rx1.await.unwrap();
        let resp2 = rx2.await.unwrap();
        
        assert!(!resp1.success);
        assert!(!resp2.success);
    }

    #[test]
    fn test_daemon_command_build_reload() {
        let cmd = DaemonCommand::Reload {
            app_id: "abc123".to_string(),
        };
        let json = cmd.build(1);
        
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "app.restart");
        assert_eq!(parsed["params"]["appId"], "abc123");
        assert_eq!(parsed["params"]["fullRestart"], false);
    }

    #[test]
    fn test_daemon_command_build_restart() {
        let cmd = DaemonCommand::Restart {
            app_id: "abc123".to_string(),
        };
        let json = cmd.build(1);
        
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["params"]["fullRestart"], true);
    }

    #[test]
    fn test_daemon_command_build_stop() {
        let cmd = DaemonCommand::Stop {
            app_id: "abc123".to_string(),
        };
        let json = cmd.build(2);
        
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 2);
        assert_eq!(parsed["method"], "app.stop");
    }

    #[test]
    fn test_daemon_command_build_version() {
        let cmd = DaemonCommand::Version;
        let json = cmd.build(1);
        
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["method"], "daemon.version");
    }

    #[test]
    fn test_daemon_command_description() {
        assert_eq!(DaemonCommand::Reload { app_id: "x".into() }.description(), "hot reload");
        assert_eq!(DaemonCommand::Restart { app_id: "x".into() }.description(), "hot restart");
        assert_eq!(DaemonCommand::Stop { app_id: "x".into() }.description(), "stop app");
    }

    #[tokio::test]
    async fn test_command_sender_with_response() {
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);
        let tracker = Arc::new(RequestTracker::default());
        let sender = CommandSender::new(stdin_tx, tracker.clone());

        // Spawn a task to simulate the daemon
        let tracker_clone = tracker.clone();
        tokio::spawn(async move {
            if let Some(json) = stdin_rx.recv().await {
                // Parse the request ID from the sent JSON
                let inner = json.trim_start_matches('[').trim_end_matches(']');
                let parsed: Value = serde_json::from_str(inner).unwrap();
                let id = parsed["id"].as_u64().unwrap();
                
                // Simulate response
                tracker_clone.handle_response(id, Some(json!({"code": 0})), None).await;
            }
        });

        let response = sender
            .send(DaemonCommand::Version)
            .await
            .unwrap();

        assert!(response.success);
    }

    #[tokio::test]
    async fn test_command_sender_timeout() {
        let (stdin_tx, _stdin_rx) = mpsc::channel::<String>(32);
        let tracker = Arc::new(RequestTracker::default());
        let sender = CommandSender::new(stdin_tx, tracker);

        // Send with very short timeout, no response will come
        let result = sender
            .send_with_timeout(DaemonCommand::Version, Duration::from_millis(10))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }
}
```

---

### Integration with FlutterProcess

Update `FlutterProcess` to use `CommandSender`:

```rust
// src/daemon/process.rs - additions

impl FlutterProcess {
    /// Get a command sender for this process
    pub fn command_sender(&self, tracker: Arc<RequestTracker>) -> CommandSender {
        CommandSender::new(self.stdin_tx.clone(), tracker)
    }
}
```

Update response handling to route through tracker:

```rust
// In the stdout reader, when a Response is detected:
if let DaemonMessage::Response { id, result, error } = message {
    if let Some(id) = id.as_u64() {
        request_tracker.handle_response(id, result, error).await;
    }
}
```

---

### Notes

- Request IDs are u64 to avoid overflow in long-running sessions
- The atomic counter is global; could be per-process if needed
- Fire-and-forget is useful for shutdown where we don't wait for response
- Consider adding exponential backoff for retries (future enhancement)
- The response channel uses oneshot for single-response semantics
- Cleanup should run periodically (e.g., every 10 seconds) via a background task

---

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/daemon/commands.rs` | CREATE | Command building and request tracking |
| `src/daemon/mod.rs` | MODIFY | Re-export CommandSender, RequestTracker, DaemonCommand |
| `src/daemon/process.rs` | MODIFY | Add command_sender() method |
| `src/services/flutter_controller.rs` | MODIFY | Use CommandSender for reload/restart |

---

## Completion Summary

**Status**: ✅ Done

**Date**: 2026-01-03

### Files Created/Modified

| File | Action | Description |
|------|--------|-------------|
| `src/daemon/commands.rs` | CREATED | Complete module with `RequestTracker`, `DaemonCommand`, `CommandSender`, `CommandResponse` |
| `src/daemon/mod.rs` | MODIFIED | Re-exported `next_request_id`, `CommandResponse`, `CommandSender`, `DaemonCommand`, `RequestTracker` |
| `src/daemon/process.rs` | MODIFIED | Added `stdin_sender()` and `command_sender()` methods |
| `src/services/flutter_controller.rs` | MODIFIED | Added `CommandSenderController` implementation using `CommandSender` |
| `src/services/mod.rs` | MODIFIED | Re-exported `CommandSenderController` |

### Key Components Implemented

1. **RequestTracker** (`commands.rs`)
   - `register(description)` -> `(id, Receiver<CommandResponse>)`
   - `handle_response(id, result, error)` -> `bool`
   - `cancel_all()` - Cancels pending requests on shutdown
   - `cleanup_stale(timeout)` -> `Vec<u64>` - Removes timed out requests
   - `pending_count()` -> `usize`

2. **DaemonCommand** enum (`commands.rs`)
   - `Reload`, `Restart`, `Stop` - App control
   - `Version`, `Shutdown` - Daemon lifecycle
   - `EnableDevices`, `GetDevices` - Device management
   - `Screenshot`, `ToggleDebugPaint`, `TogglePlatform` - Debug tools
   - `build(id)` - Generates JSON-RPC format
   - `description()` - Human-readable name

3. **CommandSender** (`commands.rs`)
   - `send(command)` - With 30s default timeout
   - `send_with_timeout(command, duration)` - Custom timeout
   - `send_fire_and_forget(command)` - No response wait
   - `tracker()` - Access to underlying RequestTracker

4. **CommandResponse** (`commands.rs`)
   - `id`, `success`, `result`, `error` fields
   - `from_daemon_response()`, `success()`, `error()` constructors

5. **CommandSenderController** (`flutter_controller.rs`)
   - Full `FlutterController` implementation using `CommandSender`
   - Proper request/response tracking with timeout handling
   - Reads `app_id` from `SharedState` for commands

### Testing Performed

```bash
cargo check   # ✅ Passes
cargo test    # ✅ 161 tests pass (145 lib + 16 integration)
cargo clippy  # ✅ No warnings
cargo fmt     # ✅ Applied
```

New tests added: 19 tests for command system

### Acceptance Criteria Status

- [x] `RequestTracker` tracks pending requests by ID
- [x] Request IDs are globally unique (atomic counter)
- [x] `handle_response()` matches responses to pending requests
- [x] Unmatched responses return false (logged but no error)
- [x] `cleanup_stale()` removes and notifies timed-out requests
- [x] `DaemonCommand` enum covers reload, restart, stop, version
- [x] `build()` generates valid JSON-RPC format
- [x] Commands are wrapped in `[...]` brackets per daemon protocol
- [x] `send()` waits for response with 30s default timeout
- [x] `send_fire_and_forget()` doesn't wait for response
- [x] `cancel_all()` cleans up on shutdown
- [x] All new types re-exported from `daemon/mod.rs`
- [x] Unit tests for request tracking
- [x] Unit tests for command building

### Notable Decisions/Tradeoffs

1. **Global atomic counter**: Request IDs are globally unique using `AtomicU64`. Could be per-process if needed, but global is simpler and sufficient.

2. **Two controller implementations**: Kept `DaemonFlutterController` (simple channel-based) alongside `CommandSenderController` (full request tracking). TUI can use either depending on needs.

3. **Fire-and-forget for shutdown**: `send_fire_and_forget()` doesn't register pending requests, useful for daemon shutdown where we don't wait.

4. **Oneshot channels**: Each request uses a `oneshot::channel` for single-response semantics.

### Risks/Limitations

- Periodic cleanup of stale requests should be implemented via a background task (not done in this task)
- Retry logic with exponential backoff is left as a future enhancement