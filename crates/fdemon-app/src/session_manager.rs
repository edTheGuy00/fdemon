//! Manages multiple Flutter app sessions

use std::collections::HashMap;

use crate::config::{DevToolsSettings, LaunchConfig};
use fdemon_core::prelude::*;
use fdemon_daemon::{Device, FlutterProcess};

use super::session::{Session, SessionHandle, SessionId};

/// Maximum number of concurrent sessions
pub const MAX_SESSIONS: usize = 9;

/// Manages multiple Flutter app sessions
#[derive(Debug)]
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
        )
        .with_config(config);

        let id = session.id;
        let handle = SessionHandle::new(session);

        self.sessions.insert(id, handle);
        self.session_order.push(id);

        if self.session_order.len() == 1 {
            self.selected_index = 0;
        }

        Ok(id)
    }

    /// Create a new session for a device, applying DevTools configuration.
    ///
    /// Initialises `NetworkState` from the provided `devtools` settings so that
    /// `max_entries` and the initial `recording` flag are sourced from config
    /// rather than hard-coded defaults.
    pub fn create_session_configured(
        &mut self,
        device: &Device,
        devtools: &DevToolsSettings,
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
        )
        .with_network_config(devtools.max_network_entries, devtools.network_auto_record);

        let id = session.id;
        let handle = SessionHandle::new(session);

        self.sessions.insert(id, handle);
        self.session_order.push(id);

        if self.session_order.len() == 1 {
            self.selected_index = 0;
        }

        Ok(id)
    }

    /// Create a session with a launch configuration and DevTools configuration.
    pub fn create_session_with_config_configured(
        &mut self,
        device: &Device,
        config: LaunchConfig,
        devtools: &DevToolsSettings,
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
        )
        .with_config(config)
        .with_network_config(devtools.max_network_entries, devtools.network_auto_record);

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
            if !self.session_order.is_empty() && self.selected_index >= self.session_order.len() {
                self.selected_index = self.session_order.len() - 1;
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

    /// Get count of running sessions
    pub fn running_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|h| h.session.is_running())
            .count()
    }

    /// Get all app_ids for running sessions
    pub fn running_app_ids(&self) -> Vec<String> {
        self.sessions
            .values()
            .filter_map(|h| h.session.app_id.clone())
            .collect()
    }

    /// Get sessions that can be reloaded (have app_id and cmd_sender, not busy)
    /// Returns (session_id, app_id) pairs
    pub fn reloadable_sessions(&self) -> Vec<(SessionId, String)> {
        self.sessions
            .values()
            .filter_map(|h| {
                // Skip busy sessions
                if h.session.is_busy() {
                    return None;
                }
                // Need both app_id and cmd_sender
                let app_id = h.session.app_id.clone()?;
                if h.cmd_sender.is_some() {
                    Some((h.session.id, app_id))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if any session is busy (reloading)
    pub fn any_session_busy(&self) -> bool {
        self.sessions.values().any(|h| h.session.is_busy())
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

    // ─────────────────────────────────────────────────────────
    // Log Batching Support (Task 04)
    // ─────────────────────────────────────────────────────────

    /// Check if any session has pending batched logs that should be flushed
    pub fn any_pending_log_flush(&self) -> bool {
        self.sessions
            .values()
            .any(|h| h.session.should_flush_logs())
    }

    /// Flush pending batched logs for all sessions
    ///
    /// Flushes all pending logs regardless of whether thresholds are met.
    /// This is called before rendering to ensure all logs are visible.
    /// Returns total number of logs flushed across all sessions.
    pub fn flush_all_pending_logs(&mut self) -> usize {
        let mut total_flushed = 0;
        for handle in self.sessions.values_mut() {
            if handle.session.has_pending_logs() {
                total_flushed += handle.session.flush_batched_logs();
            }
        }
        total_flushed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::LogSource;

    fn test_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
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

        let id1 = manager
            .create_session(&test_device("d1", "Device 1"))
            .unwrap();
        let _id2 = manager
            .create_session(&test_device("d2", "Device 2"))
            .unwrap();
        let _id3 = manager
            .create_session(&test_device("d3", "Device 3"))
            .unwrap();

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

        // Verify id1 is still accessible
        assert!(manager.get(id1).is_some());
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
            manager
                .create_session(&test_device(&format!("d{}", i), &format!("D{}", i)))
                .unwrap();
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
    fn test_find_by_device_id() {
        let mut manager = SessionManager::new();

        let id1 = manager
            .create_session(&test_device("device-1", "D1"))
            .unwrap();
        let id2 = manager
            .create_session(&test_device("device-2", "D2"))
            .unwrap();

        assert_eq!(manager.find_by_device_id("device-1"), Some(id1));
        assert_eq!(manager.find_by_device_id("device-2"), Some(id2));
        assert_eq!(manager.find_by_device_id("device-3"), None);
    }

    #[test]
    fn test_running_sessions() {
        let mut manager = SessionManager::new();

        let id1 = manager.create_session(&test_device("d1", "D1")).unwrap();
        let _id2 = manager.create_session(&test_device("d2", "D2")).unwrap();

        assert!(!manager.has_running_sessions());

        manager
            .get_mut(id1)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        assert!(manager.has_running_sessions());
        assert_eq!(manager.running_sessions(), vec![id1]);
    }

    #[test]
    fn test_session_manager_is_empty() {
        let manager = SessionManager::new();
        assert!(manager.is_empty());

        let mut manager = SessionManager::new();
        manager.create_session(&test_device("d1", "D1")).unwrap();
        assert!(!manager.is_empty());
    }

    #[test]
    fn test_iter_sessions() {
        let mut manager = SessionManager::new();

        manager.create_session(&test_device("d1", "D1")).unwrap();
        manager.create_session(&test_device("d2", "D2")).unwrap();
        manager.create_session(&test_device("d3", "D3")).unwrap();

        let count = manager.iter().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_session_with_config() {
        let mut manager = SessionManager::new();
        let device = test_device("d1", "Device 1");
        let config = LaunchConfig {
            name: "My Config".to_string(),
            ..Default::default()
        };

        let id = manager.create_session_with_config(&device, config).unwrap();

        let session = &manager.get(id).unwrap().session;
        assert_eq!(session.name, "My Config");
        assert!(session.launch_config.is_some());
    }

    #[test]
    fn test_selected_mut() {
        let mut manager = SessionManager::new();

        let id = manager.create_session(&test_device("d1", "D1")).unwrap();

        {
            let selected = manager.selected_mut().unwrap();
            selected.session.log_info(LogSource::App, "Test log");
        }

        assert_eq!(manager.get(id).unwrap().session.logs.len(), 1);
    }

    #[test]
    fn test_select_invalid_index() {
        let mut manager = SessionManager::new();
        manager.create_session(&test_device("d1", "D1")).unwrap();

        assert!(!manager.select_by_index(5));
        assert_eq!(manager.selected_index(), 0);
    }

    #[test]
    fn test_select_invalid_id() {
        let mut manager = SessionManager::new();
        manager.create_session(&test_device("d1", "D1")).unwrap();

        assert!(!manager.select_by_id(999));
        assert_eq!(manager.selected_index(), 0);
    }

    #[test]
    fn test_session_manager_with_logging() {
        let mut manager = SessionManager::new();

        let id = manager
            .create_session(&test_device("d1", "Device"))
            .unwrap();

        let session = &mut manager.get_mut(id).unwrap().session;
        session.log_info(LogSource::App, "Starting...");
        session.mark_started("app-123".to_string());
        session.log_info(LogSource::Flutter, "App running");

        assert_eq!(session.logs.len(), 2);
        assert!(session.is_running());
    }

    #[test]
    fn test_empty_manager_selection() {
        let manager = SessionManager::new();

        assert!(manager.selected().is_none());
        assert!(manager.selected_id().is_none());
    }

    #[test]
    fn test_navigation_empty_manager() {
        let mut manager = SessionManager::new();

        // Should not panic
        manager.select_next();
        manager.select_previous();

        assert_eq!(manager.selected_index(), 0);
    }

    #[test]
    fn test_running_count() {
        let mut manager = SessionManager::new();

        let id1 = manager.create_session(&test_device("d1", "D1")).unwrap();
        let _id2 = manager.create_session(&test_device("d2", "D2")).unwrap();
        let id3 = manager.create_session(&test_device("d3", "D3")).unwrap();

        assert_eq!(manager.running_count(), 0);

        manager
            .get_mut(id1)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        assert_eq!(manager.running_count(), 1);

        manager
            .get_mut(id3)
            .unwrap()
            .session
            .mark_started("app-3".to_string());

        assert_eq!(manager.running_count(), 2);
    }

    #[test]
    fn test_running_app_ids() {
        let mut manager = SessionManager::new();

        let id1 = manager.create_session(&test_device("d1", "D1")).unwrap();
        let _id2 = manager.create_session(&test_device("d2", "D2")).unwrap();
        let id3 = manager.create_session(&test_device("d3", "D3")).unwrap();

        // No running sessions initially
        assert!(manager.running_app_ids().is_empty());

        // Mark some sessions as running with app_ids
        manager
            .get_mut(id1)
            .unwrap()
            .session
            .mark_started("app-123".to_string());
        manager
            .get_mut(id3)
            .unwrap()
            .session
            .mark_started("app-456".to_string());

        let app_ids = manager.running_app_ids();
        assert_eq!(app_ids.len(), 2);
        assert!(app_ids.contains(&"app-123".to_string()));
        assert!(app_ids.contains(&"app-456".to_string()));
    }
}
