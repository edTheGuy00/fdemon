## Task: Session Manager for Multi-Instance Support

**Objective**: Create a `Session` struct to encapsulate per-instance state (logs, app_id, device info, phase) and a `SessionManager` to coordinate multiple simultaneously running Flutter app instances.

**Depends on**: [01-config-module](01-config-module.md)

---

### Scope

- `src/app/session.rs`: **NEW** - Session struct for per-instance state
- `src/app/session_manager.rs`: **NEW** - SessionManager for coordinating multiple sessions
- `src/app/mod.rs`: Add module declarations and re-exports
- `src/app/state.rs`: Refactor to extract session-specific state

---

### Implementation Details

#### Design Rationale

The current `AppState` conflates global UI state with per-session Flutter app state. For multi-instance support, we need:

1. **Session**: Encapsulates one running Flutter app instance
   - Owns its own log buffer
   - Tracks its device, app_id, phase
   - Manages its FlutterProcess

2. **SessionManager**: Coordinates multiple sessions
   - Tracks active/selected session
   - Routes messages to correct session
   - Manages session lifecycle (create, switch, close)

3. **AppState**: Becomes global UI state only
   - Selected session index
   - Global settings
   - UI mode (normal, device selector, etc.)

#### Session Struct (`src/app/session.rs`)

```rust
//! Per-instance session state for a running Flutter app

use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Local};
use tokio::sync::mpsc;

use crate::config::LaunchConfig;
use crate::core::{AppPhase, LogEntry, LogSource};
use crate::daemon::{CommandSender, FlutterProcess, RequestTracker};
use crate::tui::widgets::LogViewState;

/// Unique identifier for a session
pub type SessionId = u64;

/// Generate a new unique session ID
static SESSION_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn next_session_id() -> SessionId {
    SESSION_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// A single Flutter app session
#[derive(Debug)]
pub struct Session {
    /// Unique session identifier
    pub id: SessionId,
    
    /// Display name for this session (device name or config name)
    pub name: String,
    
    /// Current phase of this session
    pub phase: AppPhase,
    
    /// Log buffer for this session
    pub logs: Vec<LogEntry>,
    
    /// Log view scroll state
    pub log_view_state: LogViewState,
    
    /// Maximum log buffer size
    pub max_logs: usize,
    
    // ─────────────────────────────────────────────────────────
    // Device & App Tracking
    // ─────────────────────────────────────────────────────────
    
    /// Device ID this session is running on
    pub device_id: String,
    
    /// Device display name
    pub device_name: String,
    
    /// Platform (e.g., "ios", "android", "macos")
    pub platform: String,
    
    /// Whether device is emulator/simulator
    pub is_emulator: bool,
    
    /// Current app ID (from daemon's app.start event)
    pub app_id: Option<String>,
    
    /// Launch configuration used
    pub launch_config: Option<LaunchConfig>,
    
    // ─────────────────────────────────────────────────────────
    // Timing
    // ─────────────────────────────────────────────────────────
    
    /// When this session was created
    pub created_at: DateTime<Local>,
    
    /// When the Flutter app started running
    pub started_at: Option<DateTime<Local>>,
    
    /// When the current reload started (for timing)
    pub reload_start_time: Option<Instant>,
    
    /// Last successful reload time
    pub last_reload_time: Option<DateTime<Local>>,
    
    /// Total reload count this session
    pub reload_count: u32,
}

impl Session {
    /// Create a new session for a device
    pub fn new(device_id: String, device_name: String, platform: String, is_emulator: bool) -> Self {
        Self {
            id: next_session_id(),
            name: device_name.clone(),
            phase: AppPhase::Initializing,
            logs: Vec::new(),
            log_view_state: LogViewState::new(),
            max_logs: 10_000,
            device_id,
            device_name,
            platform,
            is_emulator,
            app_id: None,
            launch_config: None,
            created_at: Local::now(),
            started_at: None,
            reload_start_time: None,
            last_reload_time: None,
            reload_count: 0,
        }
    }
    
    /// Create session with a launch configuration
    pub fn with_config(mut self, config: LaunchConfig) -> Self {
        self.name = config.name.clone();
        self.launch_config = Some(config);
        self
    }
    
    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        self.logs.push(entry);
        
        // Trim if over max size
        if self.logs.len() > self.max_logs {
            let drain_count = self.logs.len() - self.max_logs;
            self.logs.drain(0..drain_count);
            
            // Adjust scroll offset
            self.log_view_state.offset = self.log_view_state.offset.saturating_sub(drain_count);
        }
    }
    
    /// Add an info log
    pub fn log_info(&mut self, source: LogSource, message: impl Into<String>) {
        self.add_log(LogEntry::info(source, message));
    }
    
    /// Add an error log
    pub fn log_error(&mut self, source: LogSource, message: impl Into<String>) {
        self.add_log(LogEntry::error(source, message));
    }
    
    /// Clear all logs
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.log_view_state.offset = 0;
    }
    
    /// Mark session as started
    pub fn mark_started(&mut self, app_id: String) {
        self.app_id = Some(app_id);
        self.started_at = Some(Local::now());
        self.phase = AppPhase::Running;
    }
    
    /// Mark session as stopped
    pub fn mark_stopped(&mut self) {
        self.phase = AppPhase::Stopped;
    }
    
    /// Called when a reload starts
    pub fn start_reload(&mut self) {
        self.reload_start_time = Some(Instant::now());
        self.phase = AppPhase::Reloading;
    }
    
    /// Called when a reload completes successfully
    pub fn complete_reload(&mut self) {
        self.reload_count += 1;
        self.last_reload_time = Some(Local::now());
        self.reload_start_time = None;
        self.phase = AppPhase::Running;
    }
    
    /// Get elapsed time since reload started
    pub fn reload_elapsed(&self) -> Option<std::time::Duration> {
        self.reload_start_time.map(|start| start.elapsed())
    }
    
    /// Calculate session duration from start time
    pub fn session_duration(&self) -> Option<chrono::Duration> {
        self.started_at.map(|start| Local::now() - start)
    }
    
    /// Format session duration as HH:MM:SS
    pub fn session_duration_display(&self) -> Option<String> {
        self.session_duration().map(|d| {
            let total_secs = d.num_seconds().max(0);
            let hours = total_secs / 3600;
            let minutes = (total_secs % 3600) / 60;
            let seconds = total_secs % 60;
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        })
    }
    
    /// Check if session is running
    pub fn is_running(&self) -> bool {
        matches!(self.phase, AppPhase::Running | AppPhase::Reloading)
    }
    
    /// Check if session is in a busy state (reload/restart in progress)
    pub fn is_busy(&self) -> bool {
        matches!(self.phase, AppPhase::Reloading)
    }
    
    /// Get status indicator character
    pub fn status_icon(&self) -> &'static str {
        match self.phase {
            AppPhase::Initializing => "○",
            AppPhase::Running => "●",
            AppPhase::Reloading => "↻",
            AppPhase::Stopped => "○",
            AppPhase::Quitting => "×",
        }
    }
    
    /// Get a short display title for tabs
    pub fn tab_title(&self) -> String {
        let icon = self.status_icon();
        let name = if self.name.len() > 15 {
            format!("{}…", &self.name[..14])
        } else {
            self.name.clone()
        };
        format!("{} {}", icon, name)
    }
}

/// Handle for controlling a session's Flutter process
pub struct SessionHandle {
    /// The session state
    pub session: Session,
    
    /// The Flutter process (if running)
    pub process: Option<FlutterProcess>,
    
    /// Command sender for this session
    pub cmd_sender: Option<CommandSender>,
    
    /// Request tracker for response matching
    pub request_tracker: Arc<RequestTracker>,
}

impl SessionHandle {
    /// Create a new session handle
    pub fn new(session: Session) -> Self {
        Self {
            session,
            process: None,
            cmd_sender: None,
            request_tracker: Arc::new(RequestTracker::default()),
        }
    }
    
    /// Attach a Flutter process to this session
    pub fn attach_process(&mut self, process: FlutterProcess) {
        let sender = process.command_sender(self.request_tracker.clone());
        self.cmd_sender = Some(sender);
        self.process = Some(process);
        self.session.phase = AppPhase::Initializing;
    }
    
    /// Check if process is running
    pub fn has_process(&self) -> bool {
        self.process.is_some()
    }
    
    /// Get the app_id if available
    pub fn app_id(&self) -> Option<&str> {
        self.session.app_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_creation() {
        let session = Session::new(
            "device-123".to_string(),
            "iPhone 15 Pro".to_string(),
            "ios".to_string(),
            true,
        );
        
        assert_eq!(session.device_id, "device-123");
        assert_eq!(session.device_name, "iPhone 15 Pro");
        assert_eq!(session.name, "iPhone 15 Pro");
        assert!(session.is_emulator);
        assert_eq!(session.phase, AppPhase::Initializing);
        assert!(session.logs.is_empty());
    }
    
    #[test]
    fn test_session_id_uniqueness() {
        let s1 = Session::new("a".into(), "A".into(), "ios".into(), false);
        let s2 = Session::new("b".into(), "B".into(), "ios".into(), false);
        let s3 = Session::new("c".into(), "C".into(), "ios".into(), false);
        
        assert_ne!(s1.id, s2.id);
        assert_ne!(s2.id, s3.id);
        assert_ne!(s1.id, s3.id);
    }
    
    #[test]
    fn test_session_logging() {
        let mut session = Session::new("d".into(), "Device".into(), "android".into(), false);
        
        session.log_info(LogSource::App, "Test message");
        session.log_error(LogSource::Daemon, "Error message");
        
        assert_eq!(session.logs.len(), 2);
    }
    
    #[test]
    fn test_session_log_trimming() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        session.max_logs = 5;
        
        for i in 0..10 {
            session.log_info(LogSource::App, format!("Message {}", i));
        }
        
        assert_eq!(session.logs.len(), 5);
        // Should have messages 5-9
        assert!(session.logs[0].message.contains("5"));
        assert!(session.logs[4].message.contains("9"));
    }
    
    #[test]
    fn test_session_lifecycle() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        
        assert_eq!(session.phase, AppPhase::Initializing);
        assert!(session.app_id.is_none());
        
        session.mark_started("app-123".to_string());
        assert_eq!(session.phase, AppPhase::Running);
        assert_eq!(session.app_id, Some("app-123".to_string()));
        assert!(session.started_at.is_some());
        
        session.start_reload();
        assert_eq!(session.phase, AppPhase::Reloading);
        assert!(session.reload_start_time.is_some());
        
        session.complete_reload();
        assert_eq!(session.phase, AppPhase::Running);
        assert_eq!(session.reload_count, 1);
        assert!(session.last_reload_time.is_some());
        
        session.mark_stopped();
        assert_eq!(session.phase, AppPhase::Stopped);
    }
    
    #[test]
    fn test_session_status_icons() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        
        assert_eq!(session.status_icon(), "○"); // Initializing
        
        session.phase = AppPhase::Running;
        assert_eq!(session.status_icon(), "●");
        
        session.phase = AppPhase::Reloading;
        assert_eq!(session.status_icon(), "↻");
        
        session.phase = AppPhase::Stopped;
        assert_eq!(session.status_icon(), "○");
    }
    
    #[test]
    fn test_tab_title_truncation() {
        let short = Session::new("d".into(), "iPhone".into(), "ios".into(), false);
        assert_eq!(short.tab_title(), "○ iPhone");
        
        let long = Session::new("d".into(), "Very Long Device Name Here".into(), "ios".into(), false);
        assert!(long.tab_title().contains("…"));
        assert!(long.tab_title().len() < 20);
    }
}
```

#### Session Manager (`src/app/session_manager.rs`)

```rust
//! Manages multiple Flutter app sessions

use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::common::prelude::*;
use crate::config::LaunchConfig;
use crate::core::DaemonEvent;
use crate::daemon::{Device, FlutterProcess};

use super::session::{Session, SessionHandle, SessionId};

/// Maximum number of concurrent sessions
pub const MAX_SESSIONS: usize = 9;

/// Manages multiple Flutter app sessions
pub struct SessionManager {
    /// All session handles indexed by session ID
    sessions: HashMap<SessionId, SessionHandle>,
    
    /// Order of session IDs (for tab ordering)
    session_order: Vec<SessionId>,
    
    /// Currently selected/focused session
    selected_index: usize,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            session_order: Vec::new(),
            selected_index: 0,
        }
    }
    
    /// Create a new session for a device
    pub fn create_session(&mut self, device: &Device) -> Result<SessionId> {
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(Error::config(format!(
                "Maximum of {} concurrent sessions reached",
                MAX_SESSIONS
            )));
        }
        
        let session = Session::new(
            device.id.clone(),
            device.name.clone(),
            device.platform.clone(),
            device.emulator,
        );
        
        let id = session.id;
        let handle = SessionHandle::new(session);
        
        self.sessions.insert(id, handle);
        self.session_order.push(id);
        
        // Auto-select if first session
        if self.session_order.len() == 1 {
            self.selected_index = 0;
        }
        
        Ok(id)
    }
    
    /// Create a session with a launch configuration
    pub fn create_session_with_config(
        &mut self,
        device: &Device,
        config: LaunchConfig,
    ) -> Result<SessionId> {
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(Error::config(format!(
                "Maximum of {} concurrent sessions reached",
                MAX_SESSIONS
            )));
        }
        
        let session = Session::new(
            device.id.clone(),
            device.name.clone(),
            device.platform.clone(),
            device.emulator,
        ).with_config(config);
        
        let id = session.id;
        let handle = SessionHandle::new(session);
        
        self.sessions.insert(id, handle);
        self.session_order.push(id);
        
        if self.session_order.len() == 1 {
            self.selected_index = 0;
        }
        
        Ok(id)
    }
    
    /// Remove a session
    pub fn remove_session(&mut self, session_id: SessionId) -> Option<SessionHandle> {
        if let Some(pos) = self.session_order.iter().position(|&id| id == session_id) {
            self.session_order.remove(pos);
            
            // Adjust selected index if needed
            if !self.session_order.is_empty() {
                if self.selected_index >= self.session_order.len() {
                    self.selected_index = self.session_order.len() - 1;
                }
            }
        }
        
        self.sessions.remove(&session_id)
    }
    
    /// Get a session by ID
    pub fn get(&self, session_id: SessionId) -> Option<&SessionHandle> {
        self.sessions.get(&session_id)
    }
    
    /// Get a mutable session by ID
    pub fn get_mut(&mut self, session_id: SessionId) -> Option<&mut SessionHandle> {
        self.sessions.get_mut(&session_id)
    }
    
    /// Get the currently selected session
    pub fn selected(&self) -> Option<&SessionHandle> {
        self.session_order
            .get(self.selected_index)
            .and_then(|id| self.sessions.get(id))
    }
    
    /// Get the currently selected session mutably
    pub fn selected_mut(&mut self) -> Option<&mut SessionHandle> {
        let id = self.session_order.get(self.selected_index).copied();
        id.and_then(move |id| self.sessions.get_mut(&id))
    }
    
    /// Get the selected session's ID
    pub fn selected_id(&self) -> Option<SessionId> {
        self.session_order.get(self.selected_index).copied()
    }
    
    /// Get the selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }
    
    /// Select session by index (0-based)
    pub fn select_by_index(&mut self, index: usize) -> bool {
        if index < self.session_order.len() {
            self.selected_index = index;
            true
        } else {
            false
        }
    }
    
    /// Select session by ID
    pub fn select_by_id(&mut self, session_id: SessionId) -> bool {
        if let Some(pos) = self.session_order.iter().position(|&id| id == session_id) {
            self.selected_index = pos;
            true
        } else {
            false
        }
    }
    
    /// Select next session (wraps around)
    pub fn select_next(&mut self) {
        if !self.session_order.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.session_order.len();
        }
    }
    
    /// Select previous session (wraps around)
    pub fn select_previous(&mut self) {
        if !self.session_order.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.session_order.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }
    
    /// Get number of sessions
    pub fn len(&self) -> usize {
        self.sessions.len()
    }
    
    /// Check if there are no sessions
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
    
    /// Iterate over all sessions in order
    pub fn iter(&self) -> impl Iterator<Item = &SessionHandle> {
        self.session_order
            .iter()
            .filter_map(|id| self.sessions.get(id))
    }
    
    /// Iterate over all sessions mutably
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SessionHandle> {
        let order = &self.session_order;
        self.sessions
            .iter_mut()
            .filter(|(id, _)| order.contains(id))
            .map(|(_, handle)| handle)
    }
    
    /// Get session tab titles for display
    pub fn tab_titles(&self) -> Vec<String> {
        self.session_order
            .iter()
            .filter_map(|id| self.sessions.get(id))
            .map(|h| h.session.tab_title())
            .collect()
    }
    
    /// Find session by app_id
    pub fn find_by_app_id(&self, app_id: &str) -> Option<SessionId> {
        self.sessions
            .iter()
            .find(|(_, h)| h.session.app_id.as_deref() == Some(app_id))
            .map(|(id, _)| *id)
    }
    
    /// Find session by device_id
    pub fn find_by_device_id(&self, device_id: &str) -> Option<SessionId> {
        self.sessions
            .iter()
            .find(|(_, h)| h.session.device_id == device_id)
            .map(|(id, _)| *id)
    }
    
    /// Get all running sessions
    pub fn running_sessions(&self) -> Vec<SessionId> {
        self.sessions
            .iter()
            .filter(|(_, h)| h.session.is_running())
            .map(|(id, _)| *id)
            .collect()
    }
    
    /// Check if any session is running
    pub fn has_running_sessions(&self) -> bool {
        self.sessions.values().any(|h| h.session.is_running())
    }
    
    /// Attach a Flutter process to a session
    pub fn attach_process(&mut self, session_id: SessionId, process: FlutterProcess) -> bool {
        if let Some(handle) = self.sessions.get_mut(&session_id) {
            handle.attach_process(process);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn test_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            sdk: None,
            is_supported: true,
        }
    }
    
    #[test]
    fn test_create_session() {
        let mut manager = SessionManager::new();
        let device = test_device("id1", "iPhone 15");
        
        let id = manager.create_session(&device).unwrap();
        
        assert_eq!(manager.len(), 1);
        assert!(manager.get(id).is_some());
        assert_eq!(manager.selected_id(), Some(id));
    }
    
    #[test]
    fn test_multiple_sessions() {
        let mut manager = SessionManager::new();
        
        let id1 = manager.create_session(&test_device("d1", "Device 1")).unwrap();
        let id2 = manager.create_session(&test_device("d2", "Device 2")).unwrap();
        let id3 = manager.create_session(&test_device("d3", "Device 3")).unwrap();
        
        assert_eq!(manager.len(), 3);
        
        // First session should be selected
        assert_eq!(manager.selected_id(), Some(id1));
        
        // Tab titles should be in order
        let titles = manager.tab_titles();
        assert_eq!(titles.len(), 3);
    }
    
    #[test]
    fn test_session_navigation() {
        let mut manager = SessionManager::new();
        
        let id1 = manager.create_session(&test_device("d1", "D1")).unwrap();
        let id2 = manager.create_session(&test_device("d2", "D2")).unwrap();
        let id3 = manager.create_session(&test_device("d3", "D3")).unwrap();
        
        assert_eq!(manager.selected_index(), 0);
        
        manager.select_next();
        assert_eq!(manager.selected_index(), 1);
        assert_eq!(manager.selected_id(), Some(id2));
        
        manager.select_next();
        assert_eq!(manager.selected_index(), 2);
        
        manager.select_next(); // Wrap around
        assert_eq!(manager.selected_index(), 0);
        
        manager.select_previous(); // Wrap around backwards
        assert_eq!(manager.selected_index(), 2);
        
        manager.select_by_index(1);
        assert_eq!(manager.selected_id(), Some(id2));
        
        manager.select_by_id(id3);
        assert_eq!(manager.selected_index(), 2);
    }
    
    #[test]
    fn test_remove_session() {
        let mut manager = SessionManager::new();
        
        let id1 = manager.create_session(&test_device("d1", "D1")).unwrap();
        let id2 = manager.create_session(&test_device("d2", "D2")).unwrap();
        let id3 = manager.create_session(&test_device("d3", "D3")).unwrap();
        
        manager.select_by_id(id3);
        assert_eq!(manager.selected_index(), 2);
        
        manager.remove_session(id3);
        assert_eq!(manager.len(), 2);
        assert_eq!(manager.selected_index(), 1); // Adjusted
        
        manager.remove_session(id1);
        assert_eq!(manager.len(), 1);
        assert_eq!(manager.selected_id(), Some(id2));
    }
    
    #[test]
    fn test_max_sessions() {
        let mut manager = SessionManager::new();
        
        for i in 0..MAX_SESSIONS {
            manager.create_session(&test_device(&format!("d{}", i), &format!("D{}", i))).unwrap();
        }
        
        assert_eq!(manager.len(), MAX_SESSIONS);
        
        // Should fail to create more
        let result = manager.create_session(&test_device("extra", "Extra"));
        assert!(result.is_err());
    }
    
    #[test]
    fn test_find_by_app_id() {
        let mut manager = SessionManager::new();
        
        let id1 = manager.create_session(&test_device("d1", "D1")).unwrap();
        let id2 = manager.create_session(&test_device("d2", "D2")).unwrap();
        
        manager.get_mut(id1).unwrap().session.app_id = Some("app-123".to_string());
        manager.get_mut(id2).unwrap().session.app_id = Some("app-456".to_string());
        
        assert_eq!(manager.find_by_app_id("app-123"), Some(id1));
        assert_eq!(manager.find_by_app_id("app-456"), Some(id2));
        assert_eq!(manager.find_by_app_id("app-999"), None);
    }
    
    #[test]
    fn test_running_sessions() {
        let mut manager = SessionManager::new();
        
        let id1 = manager.create_session(&test_device("d1", "D1")).unwrap();
        let id2 = manager.create_session(&test_device("d2", "D2")).unwrap();
        
        assert!(!manager.has_running_sessions());
        
        manager.get_mut(id1).unwrap().session.mark_started("app-1".to_string());
        
        assert!(manager.has_running_sessions());
        assert_eq!(manager.running_sessions(), vec![id1]);
    }
}
```

---

### Acceptance Criteria

1. [ ] `src/app/session.rs` created with `Session` and `SessionHandle` structs
2. [ ] `src/app/session_manager.rs` created with `SessionManager` struct
3. [ ] Session IDs are globally unique
4. [ ] Session tracks device info, logs, phase, and timing
5. [ ] SessionManager supports up to 9 concurrent sessions
6. [ ] Tab navigation works (next/previous/by-index/by-id)
7. [ ] Sessions can be created, accessed, and removed
8. [ ] `find_by_app_id()` and `find_by_device_id()` work correctly
9. [ ] `tab_titles()` returns properly formatted tab labels
10. [ ] Session log buffer respects max_logs limit
11. [ ] All new code has unit tests
12. [ ] `cargo test` passes
13. [ ] `cargo clippy` has no warnings

---

### Testing

Unit tests are included in the implementation above. Additional integration consideration:

```rust
#[test]
fn test_session_manager_with_logging() {
    let mut manager = SessionManager::new();
    
    let id = manager.create_session(&test_device("d1", "Device")).unwrap();
    
    let session = &mut manager.get_mut(id).unwrap().session;
    session.log_info(LogSource::App, "Starting...");
    session.mark_started("app-123".to_string());
    session.log_info(LogSource::Flutter, "App running");
    
    assert_eq!(session.logs.len(), 2);
    assert!(session.is_running());
}
```

---

### Notes

- Session IDs use an atomic counter for uniqueness
- The `SessionHandle` owns the `FlutterProcess` for proper cleanup
- Tab titles are truncated to 15 characters for display
- The manager maintains insertion order for consistent tab ordering
- When removing sessions, the selected index is automatically adjusted
- The maximum of 9 sessions matches the `1-9` keyboard shortcuts for direct access

---

### Files to Create/Modify

| File | Action |
|------|--------|
| `src/app/session.rs` | Create with Session and SessionHandle |
| `src/app/session_manager.rs` | Create with SessionManager |
| `src/app/mod.rs` | Add `pub mod session;` and `pub mod session_manager;` |