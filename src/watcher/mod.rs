//! File watcher module for auto-reload functionality
//!
//! Watches the `lib/` directory for Dart file changes and triggers
//! automatic hot reload with debouncing.

use std::path::PathBuf;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::app::message::Message;

/// Default debounce duration in milliseconds
pub const DEFAULT_DEBOUNCE_MS: u64 = 500;

/// Default paths to watch (relative to project root)
pub const DEFAULT_WATCH_PATHS: &[&str] = &["lib"];

/// File extensions to watch
pub const DART_EXTENSIONS: &[&str] = &["dart"];

/// Configuration for the file watcher
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Paths to watch (relative to project root)
    pub paths: Vec<PathBuf>,
    /// Debounce duration
    pub debounce: Duration,
    /// File extensions to watch (empty = all files)
    pub extensions: Vec<String>,
    /// Whether auto-reload is enabled
    pub auto_reload: bool,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            paths: DEFAULT_WATCH_PATHS.iter().map(PathBuf::from).collect(),
            debounce: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            extensions: DART_EXTENSIONS.iter().map(|s| (*s).to_string()).collect(),
            auto_reload: true,
        }
    }
}

impl WatcherConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom paths to watch
    pub fn with_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = paths;
        self
    }

    /// Set debounce duration in milliseconds
    pub fn with_debounce_ms(mut self, ms: u64) -> Self {
        self.debounce = Duration::from_millis(ms);
        self
    }

    /// Set file extensions to watch
    pub fn with_extensions(mut self, extensions: Vec<String>) -> Self {
        self.extensions = extensions;
        self
    }

    /// Enable or disable auto-reload
    pub fn with_auto_reload(mut self, enabled: bool) -> Self {
        self.auto_reload = enabled;
        self
    }
}

/// Manages file watching for a Flutter project
pub struct FileWatcher {
    /// Project root directory
    project_root: PathBuf,
    /// Configuration
    config: WatcherConfig,
    /// Handle to stop the watcher
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl FileWatcher {
    /// Create a new file watcher for the given project
    pub fn new(project_root: PathBuf, config: WatcherConfig) -> Self {
        Self {
            project_root,
            config,
            stop_tx: None,
        }
    }

    /// Start watching for file changes
    ///
    /// Sends `Message::AutoReloadTriggered` or `Message::FilesChanged` to the channel
    pub fn start(&mut self, message_tx: mpsc::Sender<Message>) -> Result<(), String> {
        if self.is_running() {
            return Err("Watcher is already running".to_string());
        }

        let project_root = self.project_root.clone();
        let config = self.config.clone();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();

        self.stop_tx = Some(stop_tx);

        // Spawn the watcher in a blocking task
        tokio::task::spawn_blocking(move || {
            Self::run_watcher(project_root, config, message_tx, stop_rx);
        });

        Ok(())
    }

    /// Stop the file watcher
    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Check if watcher is running
    pub fn is_running(&self) -> bool {
        self.stop_tx.is_some()
    }

    /// Internal: run the blocking watcher
    fn run_watcher(
        project_root: PathBuf,
        config: WatcherConfig,
        message_tx: mpsc::Sender<Message>,
        mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        let tx_clone = message_tx.clone();
        let extensions = config.extensions.clone();
        let auto_reload = config.auto_reload;

        // Create debounced watcher
        let debouncer_result = new_debouncer(
            config.debounce,
            None, // No tick rate override
            move |result: DebounceEventResult| {
                match result {
                    Ok(events) => {
                        // Filter events by extension
                        let relevant_events: Vec<_> = events
                            .iter()
                            .filter(|event| {
                                event.paths.iter().any(|path| {
                                    if extensions.is_empty() {
                                        return true;
                                    }
                                    path.extension()
                                        .and_then(|ext| ext.to_str())
                                        .map(|ext| extensions.iter().any(|e| e == ext))
                                        .unwrap_or(false)
                                })
                            })
                            .collect();

                        if relevant_events.is_empty() {
                            return;
                        }

                        debug!("File watcher detected {} change(s)", relevant_events.len());

                        // Send auto-reload message if enabled
                        if auto_reload {
                            let _ = tx_clone.blocking_send(Message::AutoReloadTriggered);
                        } else {
                            // Just notify about file changes
                            let _ = tx_clone.blocking_send(Message::FilesChanged {
                                count: relevant_events.len(),
                            });
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            warn!("File watcher error: {:?}", error);
                            let _ = tx_clone.blocking_send(Message::WatcherError {
                                message: error.to_string(),
                            });
                        }
                    }
                }
            },
        );

        let mut debouncer = match debouncer_result {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to create file watcher: {}", e);
                let _ = message_tx.blocking_send(Message::WatcherError {
                    message: format!("Failed to create watcher: {}", e),
                });
                return;
            }
        };

        // Add watched paths
        for relative_path in &config.paths {
            let full_path = project_root.join(relative_path);
            if full_path.exists() {
                if let Err(e) = debouncer.watch(&full_path, RecursiveMode::Recursive) {
                    warn!("Failed to watch {}: {}", full_path.display(), e);
                } else {
                    info!("Watching: {}", full_path.display());
                }
            } else {
                warn!("Watch path does not exist: {}", full_path.display());
            }
        }

        // Keep running until stop signal
        loop {
            match stop_rx.try_recv() {
                Ok(()) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    info!("File watcher stopping");
                    break;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running, sleep briefly
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();

        assert_eq!(config.debounce, Duration::from_millis(500));
        assert!(config.auto_reload);
        assert_eq!(config.paths, vec![PathBuf::from("lib")]);
        assert_eq!(config.extensions, vec!["dart".to_string()]);
    }

    #[test]
    fn test_watcher_config_new() {
        let config = WatcherConfig::new();

        assert_eq!(config.debounce, Duration::from_millis(DEFAULT_DEBOUNCE_MS));
        assert!(config.auto_reload);
    }

    #[test]
    fn test_watcher_config_builder() {
        let config = WatcherConfig::new()
            .with_debounce_ms(1000)
            .with_auto_reload(false)
            .with_paths(vec![PathBuf::from("lib"), PathBuf::from("test")])
            .with_extensions(vec!["dart".to_string(), "yaml".to_string()]);

        assert_eq!(config.debounce, Duration::from_millis(1000));
        assert!(!config.auto_reload);
        assert_eq!(config.paths.len(), 2);
        assert_eq!(config.extensions.len(), 2);
    }

    #[test]
    fn test_file_watcher_creation() {
        let project_root = PathBuf::from("/tmp/test_project");
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(project_root.clone(), config);

        assert_eq!(watcher.project_root, project_root);
        assert!(!watcher.is_running());
    }

    #[tokio::test]
    async fn test_file_watcher_stop_when_not_started() {
        let project_root = PathBuf::from("/tmp/test_project");
        let config = WatcherConfig::default();
        let mut watcher = FileWatcher::new(project_root, config);

        // Should not panic
        watcher.stop();
        assert!(!watcher.is_running());
    }

    #[tokio::test]
    async fn test_file_watcher_double_start_error() {
        let project_root = PathBuf::from("/tmp/test_project");
        let config = WatcherConfig::default();
        let mut watcher = FileWatcher::new(project_root, config);

        let (tx, _rx) = mpsc::channel(32);

        // First start should succeed
        let result1 = watcher.start(tx.clone());
        assert!(result1.is_ok());
        assert!(watcher.is_running());

        // Second start should fail
        let result2 = watcher.start(tx);
        assert!(result2.is_err());
        assert!(result2.unwrap_err().contains("already running"));

        watcher.stop();
    }

    #[test]
    fn test_default_watch_paths() {
        assert_eq!(DEFAULT_WATCH_PATHS, &["lib"]);
    }

    #[test]
    fn test_default_debounce() {
        assert_eq!(DEFAULT_DEBOUNCE_MS, 500);
    }

    #[test]
    fn test_dart_extensions() {
        assert!(DART_EXTENSIONS.contains(&"dart"));
    }

    #[test]
    fn test_watcher_config_builder_chaining() {
        let config = WatcherConfig::new()
            .with_debounce_ms(200)
            .with_auto_reload(true)
            .with_paths(vec![PathBuf::from("src")])
            .with_extensions(vec!["dart".to_string()]);

        assert_eq!(config.debounce, Duration::from_millis(200));
        assert!(config.auto_reload);
        assert_eq!(config.paths, vec![PathBuf::from("src")]);
        assert_eq!(config.extensions, vec!["dart".to_string()]);
    }
}
