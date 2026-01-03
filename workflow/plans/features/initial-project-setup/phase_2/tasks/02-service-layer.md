## Task: Service Layer Foundation

**Objective**: Create a service layer with traits and shared state infrastructure that both the TUI and future MCP server can use, enabling concurrent access and event broadcasting.

**Depends on**: 01-typed-protocol

---

### Scope

- `src/services/mod.rs`: **NEW** - Module definition and re-exports
- `src/services/flutter_controller.rs`: **NEW** - FlutterController trait and impl
- `src/services/log_service.rs`: **NEW** - LogService trait and impl
- `src/services/state_service.rs`: **NEW** - StateService trait and SharedState
- `src/lib.rs`: Add `services` module
- `src/daemon/process.rs`: Refactor to work with service layer

---

### Implementation Details

#### Why This Matters for MCP

From the [MCP Server Plan](../../../mcp-server/PLAN.md):

> The key architectural insight is introducing a **Service Layer** that both the TUI and MCP server can use.

This task creates the foundation that enables:
1. TUI handlers call service methods instead of directly manipulating daemon
2. Future MCP tool handlers call the same service methods
3. Shared state via `Arc<RwLock<T>>` for thread-safe concurrent access
4. Event broadcasting via `tokio::sync::broadcast` for multiple subscribers

#### Module Structure

```
src/services/
├── mod.rs                    # Module exports
├── flutter_controller.rs     # App control operations
├── log_service.rs            # Log buffer access
└── state_service.rs          # Shared state management
```

#### SharedState Structure

```rust
// src/services/state_service.rs

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use crate::core::{AppPhase, LogEntry};
use crate::daemon::{DaemonMessage, DeviceInfo};

/// Application run state with additional metadata
#[derive(Debug, Clone, Default)]
pub struct AppRunState {
    pub phase: AppPhase,
    pub app_id: Option<String>,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub platform: Option<String>,
    pub devtools_uri: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Local>>,
    pub last_reload_at: Option<chrono::DateTime<chrono::Local>>,
}

impl AppRunState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.phase, AppPhase::Running | AppPhase::Reloading)
    }

    pub fn session_duration(&self) -> Option<chrono::Duration> {
        self.started_at.map(|start| chrono::Local::now() - start)
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
        // Ignore send errors (no subscribers)
        let _ = self.event_tx.send(message);
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new(10_000)
    }
}

/// StateService trait for querying application state
pub trait StateService: Send + Sync {
    /// Get current application run state
    fn get_app_state(&self) -> impl std::future::Future<Output = AppRunState> + Send;
    
    /// Get list of available devices
    fn get_devices(&self) -> impl std::future::Future<Output = Vec<DeviceInfo>> + Send;
    
    /// Get project information
    fn get_project_info(&self) -> ProjectInfo;
}

/// Project information (static, doesn't need async)
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub name: String,
    pub path: std::path::PathBuf,
    pub flutter_version: Option<String>,
}

/// Default implementation using SharedState
pub struct SharedStateService {
    state: Arc<SharedState>,
    project_info: ProjectInfo,
}

impl SharedStateService {
    pub fn new(state: Arc<SharedState>, project_info: ProjectInfo) -> Self {
        Self { state, project_info }
    }
}

impl StateService for SharedStateService {
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
```

#### FlutterController Trait

```rust
// src/services/flutter_controller.rs

use std::sync::Arc;
use tokio::sync::mpsc;
use crate::common::prelude::*;
use super::state_service::SharedState;

/// Result of a reload operation
#[derive(Debug, Clone)]
pub struct ReloadResult {
    pub success: bool,
    pub time_ms: Option<u64>,
    pub message: Option<String>,
}

/// Result of a restart operation
#[derive(Debug, Clone)]
pub struct RestartResult {
    pub success: bool,
    pub message: Option<String>,
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
```

#### LogService Trait

```rust
// src/services/log_service.rs

use std::sync::Arc;
use tokio::sync::broadcast;
use crate::core::{LogEntry, LogLevel};
use super::state_service::SharedState;

/// Filter for querying logs
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub level: Option<LogLevel>,
    pub source: Option<String>,
    pub pattern: Option<String>,
    pub limit: Option<usize>,
}

impl LogFilter {
    pub fn errors() -> Self {
        Self {
            level: Some(LogLevel::Error),
            ..Default::default()
        }
    }

    pub fn warnings() -> Self {
        Self {
            level: Some(LogLevel::Warning),
            ..Default::default()
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Log buffer access and subscription
#[trait_variant::make(LogService: Send)]
pub trait LocalLogService {
    /// Get logs matching the filter
    async fn get_logs(&self, filter: Option<LogFilter>) -> Vec<LogEntry>;
    
    /// Get error-level logs
    async fn get_errors(&self) -> Vec<LogEntry>;
    
    /// Get the total log count
    async fn log_count(&self) -> usize;
    
    /// Clear all logs
    async fn clear(&self);
    
    /// Add a log entry
    async fn add_log(&self, entry: LogEntry);
}

/// Implementation using SharedState
pub struct SharedLogService {
    state: Arc<SharedState>,
}

impl SharedLogService {
    pub fn new(state: Arc<SharedState>) -> Self {
        Self { state }
    }
}

impl LocalLogService for SharedLogService {
    async fn get_logs(&self, filter: Option<LogFilter>) -> Vec<LogEntry> {
        let logs = self.state.logs.read().await;
        
        let filter = filter.unwrap_or_default();
        
        let mut result: Vec<LogEntry> = logs
            .iter()
            .filter(|log| {
                // Filter by level
                if let Some(level) = &filter.level {
                    if &log.level != level {
                        return false;
                    }
                }
                
                // Filter by pattern
                if let Some(pattern) = &filter.pattern {
                    if !log.message.contains(pattern) {
                        return false;
                    }
                }
                
                true
            })
            .cloned()
            .collect();
        
        // Apply limit (from end, most recent)
        if let Some(limit) = filter.limit {
            let start = result.len().saturating_sub(limit);
            result = result[start..].to_vec();
        }
        
        result
    }

    async fn get_errors(&self) -> Vec<LogEntry> {
        self.get_logs(Some(LogFilter::errors())).await
    }

    async fn log_count(&self) -> usize {
        self.state.logs.read().await.len()
    }

    async fn clear(&self) {
        self.state.logs.write().await.clear();
    }

    async fn add_log(&self, entry: LogEntry) {
        let mut logs = self.state.logs.write().await;
        logs.push(entry);
        
        // Trim if over max size
        if logs.len() > self.state.max_logs {
            let drain_count = logs.len() - self.state.max_logs;
            logs.drain(0..drain_count);
        }
    }
}
```

#### Module Exports

```rust
// src/services/mod.rs

mod flutter_controller;
mod log_service;
mod state_service;

pub use flutter_controller::{
    FlutterCommand,
    FlutterController,
    LocalFlutterController,
    DaemonFlutterController,
    ReloadResult,
    RestartResult,
};

pub use log_service::{
    LogFilter,
    LogService,
    LocalLogService,
    SharedLogService,
};

pub use state_service::{
    AppRunState,
    ProjectInfo,
    SharedState,
    SharedStateService,
    StateService,
};
```

---

### Acceptance Criteria

1. [ ] `services/` module created with all submodules
2. [ ] `SharedState` uses `Arc<RwLock<T>>` for all mutable state
3. [ ] `broadcast::channel` created for event distribution
4. [ ] `FlutterController` trait defined with reload/restart/stop
5. [ ] `LogService` trait defined with filtering capabilities
6. [ ] `StateService` trait defined with state queries
7. [ ] Default implementations created for all traits
8. [ ] `FlutterCommand` enum for command passing
9. [ ] `AppRunState` tracks app_id, device, timing info
10. [ ] Services re-exported from `services/mod.rs`
11. [ ] `lib.rs` updated to include `services` module
12. [ ] Unit tests for state management
13. [ ] Unit tests for log filtering

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shared_state_creation() {
        let state = SharedState::new(100);
        assert_eq!(state.max_logs, 100);
        
        let app_state = state.app_state.read().await;
        assert_eq!(app_state.phase, AppPhase::Initializing);
    }

    #[tokio::test]
    async fn test_app_run_state_is_running() {
        let mut state = AppRunState::new();
        assert!(!state.is_running());
        
        state.phase = AppPhase::Running;
        assert!(state.is_running());
        
        state.phase = AppPhase::Reloading;
        assert!(state.is_running());
        
        state.phase = AppPhase::Quitting;
        assert!(!state.is_running());
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
        
        state.broadcast(msg.clone());
        
        // Both subscribers should receive
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_log_service_filtering() {
        let state = Arc::new(SharedState::new(100));
        let service = SharedLogService::new(state.clone());
        
        // Add mixed logs
        service.add_log(LogEntry::info(LogSource::App, "info message")).await;
        service.add_log(LogEntry::error(LogSource::App, "error message")).await;
        service.add_log(LogEntry::warn(LogSource::App, "warning")).await;
        
        // Get all
        let all = service.get_logs(None).await;
        assert_eq!(all.len(), 3);
        
        // Get errors only
        let errors = service.get_errors().await;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("error"));
        
        // Get with limit
        let limited = service.get_logs(Some(LogFilter::default().with_limit(2))).await;
        assert_eq!(limited.len(), 2);
    }

    #[tokio::test]
    async fn test_log_service_max_size() {
        let state = Arc::new(SharedState::new(3)); // Max 3 logs
        let service = SharedLogService::new(state.clone());
        
        service.add_log(LogEntry::info(LogSource::App, "1")).await;
        service.add_log(LogEntry::info(LogSource::App, "2")).await;
        service.add_log(LogEntry::info(LogSource::App, "3")).await;
        service.add_log(LogEntry::info(LogSource::App, "4")).await;
        
        let logs = service.get_logs(None).await;
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].message, "2"); // First was trimmed
    }

    #[tokio::test]
    async fn test_log_service_clear() {
        let state = Arc::new(SharedState::new(100));
        let service = SharedLogService::new(state.clone());
        
        service.add_log(LogEntry::info(LogSource::App, "test")).await;
        assert_eq!(service.log_count().await, 1);
        
        service.clear().await;
        assert_eq!(service.log_count().await, 0);
    }

    #[tokio::test]
    async fn test_session_duration() {
        use chrono::{Duration, Local};
        
        let mut state = AppRunState::new();
        assert!(state.session_duration().is_none());
        
        state.started_at = Some(Local::now() - Duration::seconds(60));
        let duration = state.session_duration().unwrap();
        assert!(duration.num_seconds() >= 60);
    }
}
```

---

### Migration Notes

After this task, existing code needs to be updated:

1. **`app/state.rs`**: Consider deprecating in favor of SharedState, or keep for TUI-specific view state
2. **`app/handler.rs`**: Use LogService instead of direct log manipulation
3. **`main.rs`**: Create SharedState early and pass to services

The migration can happen incrementally—new code uses services, old code continues working.

---

### Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# Required for trait_variant macro (async traits)
trait-variant = "0.1"
```

If `trait-variant` is not available, use manual implementation with `async_trait` or boxed futures.

---

### Notes

- This task focuses on **defining the infrastructure**, not fully integrating it
- Task 03 (command system) will complete the FlutterController implementation
- Task 04 (reload commands) will wire TUI to use FlutterController
- Keep `AppState` in `app/state.rs` for now—it handles TUI-specific state like scroll position
- `SharedState` is for domain state that MCP needs access to
- The `trait_variant` crate allows async methods in traits with Send bounds
- Consider adding `tracing` instrumentation for debugging concurrent access

---

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/services/mod.rs` | CREATE | Module definition and re-exports |
| `src/services/flutter_controller.rs` | CREATE | Controller trait and impl |
| `src/services/log_service.rs` | CREATE | Log service trait and impl |
| `src/services/state_service.rs` | CREATE | Shared state and state service |
| `src/lib.rs` | MODIFY | Add `pub mod services;` |
| `Cargo.toml` | MODIFY | Add trait-variant dependency |

---

## Completion Summary

**Status**: ✅ Done

**Date**: 2026-01-03

### Files Created/Modified

| File | Action | Description |
|------|--------|-------------|
| `src/services/mod.rs` | CREATED | Module exports for all services |
| `src/services/state_service.rs` | CREATED | `SharedState`, `AppRunState`, `ProjectInfo`, `StateService` trait |
| `src/services/flutter_controller.rs` | CREATED | `FlutterController` trait, `DaemonFlutterController`, `FlutterCommand` enum |
| `src/services/log_service.rs` | CREATED | `LogService` trait, `SharedLogService`, `LogFilter` |
| `src/lib.rs` | MODIFIED | Added `pub mod services;` |
| `Cargo.toml` | MODIFIED | Added `trait-variant = "0.1"` dependency |

### Key Components Implemented

1. **SharedState** (`state_service.rs`)
   - `app_state: Arc<RwLock<AppRunState>>` - Thread-safe app state
   - `logs: Arc<RwLock<Vec<LogEntry>>>` - Log buffer for MCP access
   - `devices: Arc<RwLock<Vec<DeviceInfo>>>` - Device list
   - `event_tx: broadcast::Sender<DaemonMessage>` - Event broadcaster
   - `subscribe()` and `broadcast()` methods

2. **AppRunState** (`state_service.rs`)
   - Tracks: phase, app_id, device_id, device_name, platform, devtools_uri
   - Timing: started_at, last_reload_at
   - Methods: `is_running()`, `session_duration()`, `reset()`

3. **FlutterController** trait (`flutter_controller.rs`)
   - `reload()` -> `Result<ReloadResult>`
   - `restart()` -> `Result<RestartResult>`
   - `stop()` -> `Result<()>`
   - `is_running()` -> `bool`
   - `get_app_id()` -> `Option<String>`

4. **LogService** trait (`log_service.rs`)
   - `get_logs(filter)` -> `Vec<LogEntry>`
   - `get_errors()` -> `Vec<LogEntry>`
   - `log_count()` -> `usize`
   - `clear()` and `add_log()`
   - `LogFilter` with level, pattern, limit filtering

5. **StateService** trait (`state_service.rs`)
   - `get_app_state()` -> `AppRunState`
   - `get_devices()` -> `Vec<DeviceInfo>`
   - `get_project_info()` -> `ProjectInfo`

### Testing Performed

```bash
cargo check   # ✅ Passes
cargo test    # ✅ 142 tests pass (126 lib + 16 integration)
cargo clippy  # ✅ No warnings
cargo fmt     # ✅ Applied
```

New tests added: 32 tests across services module

### Acceptance Criteria Status

- [x] `services/` module created with all submodules
- [x] `SharedState` uses `Arc<RwLock<T>>` for all mutable state
- [x] `broadcast::channel` created for event distribution
- [x] `FlutterController` trait defined with reload/restart/stop
- [x] `LogService` trait defined with filtering capabilities
- [x] `StateService` trait defined with state queries
- [x] Default implementations created for all traits
- [x] `FlutterCommand` enum for command passing
- [x] `AppRunState` tracks app_id, device, timing info
- [x] Services re-exported from `services/mod.rs`
- [x] `lib.rs` updated to include `services` module
- [x] Unit tests for state management
- [x] Unit tests for log filtering

### Notable Decisions/Tradeoffs

1. **trait-variant crate**: Used `trait_variant::make` macro to create both `Local*` (no Send bound) and `*` (Send bound) versions of each trait for flexibility.

2. **LogService separation**: Created `SharedLogService` that takes an `Arc<RwLock<Vec<LogEntry>>>` directly rather than the full `SharedState`, allowing more flexible composition.

3. **Command channel**: `DaemonFlutterController` uses `mpsc::Sender<FlutterCommand>` for command passing. Actual result tracking will be completed in Task 03.

4. **ProjectInfo as sync**: `get_project_info()` is not async since project info is static and doesn't require locks.

### Risks/Limitations

- The `FlutterController` implementation currently returns placeholder success results. Task 03 will implement proper request ID tracking and response matching.
- No integration with existing `app/state.rs` yet - this will be done incrementally as described in migration notes.