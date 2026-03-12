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

/// Events emitted by the file watcher.
/// Consumers map these to their own message types.
#[derive(Debug, Clone)]
pub enum WatcherEvent {
    /// File changes detected and auto-reload is enabled
    AutoReloadTriggered,
    /// File changes detected but auto-reload is disabled
    FilesChanged { count: usize },
    /// Watcher encountered an error
    Error { message: String },
}

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
    /// Sends `WatcherEvent` variants to the channel
    pub fn start(&mut self, event_tx: mpsc::Sender<WatcherEvent>) -> Result<(), String> {
        if self.is_running() {
            return Err("Watcher is already running".to_string());
        }

        let project_root = self.project_root.clone();
        let config = self.config.clone();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();

        self.stop_tx = Some(stop_tx);

        // Spawn the watcher in a blocking task
        tokio::task::spawn_blocking(move || {
            Self::run_watcher(project_root, config, event_tx, stop_rx);
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
        event_tx: mpsc::Sender<WatcherEvent>,
        mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        let tx_clone = event_tx.clone();
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

                        // Send auto-reload event if enabled
                        if auto_reload {
                            let _ = tx_clone.blocking_send(WatcherEvent::AutoReloadTriggered);
                        } else {
                            // Just notify about file changes
                            let _ = tx_clone.blocking_send(WatcherEvent::FilesChanged {
                                count: relevant_events.len(),
                            });
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            warn!("File watcher error: {:?}", error);
                            let _ = tx_clone.blocking_send(WatcherEvent::Error {
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
                let _ = event_tx.blocking_send(WatcherEvent::Error {
                    message: format!("Failed to create watcher: {}", e),
                });
                return;
            }
        };

        // Add watched paths
        for resolved in resolve_watch_paths(&project_root, &config.paths) {
            if resolved.exists() {
                if let Err(e) = debouncer.watch(&resolved, RecursiveMode::Recursive) {
                    warn!("Failed to watch {}: {}", resolved.display(), e);
                } else {
                    info!("Watching: {}", resolved.display());
                }
            } else {
                warn!("Watch path does not exist: {}", resolved.display());
                let _ = event_tx.blocking_send(WatcherEvent::Error {
                    message: format!("Watch path does not exist: {}", resolved.display()),
                });
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

/// Resolve a list of config paths against a project root, returning canonical
/// absolute paths ready for the OS-level watcher.
///
/// For each path in `config_paths`:
/// - Absolute paths are used as-is.
/// - Relative paths (including those with `../` components) are joined onto
///   `project_root` then canonicalized to resolve `..` and symlinks.
/// - If canonicalization fails (path does not exist), the raw joined path is
///   returned so that the caller can emit an appropriate warning rather than
///   silently dropping the entry.
pub(crate) fn resolve_watch_paths(
    project_root: &std::path::Path,
    config_paths: &[PathBuf],
) -> Vec<PathBuf> {
    config_paths
        .iter()
        .map(|p| {
            let full = if p.is_absolute() {
                p.clone()
            } else {
                project_root.join(p)
            };
            // Canonicalize resolves `..` and symlinks.
            // Falls back to the raw path when the directory doesn't exist yet.
            full.canonicalize().unwrap_or(full)
        })
        .collect()
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

        let (tx, _rx) = mpsc::channel::<WatcherEvent>(32);

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

    // ─────────────────────────────────────────────────────────
    // Path resolution tests (resolve_watch_paths)
    // ─────────────────────────────────────────────────────────

    /// Default WatcherConfig has ["lib"] as its paths list.
    #[test]
    fn test_default_paths_is_lib() {
        let config = WatcherConfig::default();
        assert_eq!(config.paths, vec![PathBuf::from("lib")]);
    }

    /// A single relative path `"lib"` is joined onto project_root and
    /// canonicalized when the directory exists.
    #[test]
    fn test_resolve_single_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().to_path_buf();

        // Create the `lib` directory so canonicalize() succeeds.
        let lib_dir = project_root.join("lib");
        std::fs::create_dir_all(&lib_dir).unwrap();

        let resolved = resolve_watch_paths(&project_root, &[PathBuf::from("lib")]);

        assert_eq!(resolved.len(), 1);
        // canonicalize() produces the real, absolute path.
        let expected = lib_dir.canonicalize().unwrap();
        assert_eq!(resolved[0], expected);
    }

    /// A path containing `../` components is resolved correctly as long as
    /// the target directory actually exists.
    #[test]
    fn test_resolve_parent_relative_path() {
        // Directory layout:  root/project/  and  root/shared/
        // From project/, `../shared` resolves to root/shared/.
        //
        // Canonicalize project_root so that `..` resolution works correctly
        // on platforms where tempdir returns a non-canonical path (e.g. macOS
        // where /var is a symlink to /private/var).
        let root = tempfile::tempdir().unwrap();
        let project_root_raw = root.path().join("project");
        std::fs::create_dir_all(&project_root_raw).unwrap();
        // Use the canonical form so intermediate `..` steps resolve correctly.
        let project_root = project_root_raw.canonicalize().unwrap();

        let shared_dir = root.path().join("shared");
        std::fs::create_dir_all(&shared_dir).unwrap();

        // "../shared" from `project_root` should resolve to `shared_dir`.
        let resolved = resolve_watch_paths(&project_root, &[PathBuf::from("../shared")]);

        assert_eq!(resolved.len(), 1);
        let expected = shared_dir.canonicalize().unwrap();
        assert_eq!(resolved[0], expected);
    }

    /// Multiple relative paths are all resolved independently.
    #[test]
    fn test_resolve_multiple_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().to_path_buf();

        std::fs::create_dir_all(project_root.join("lib")).unwrap();
        std::fs::create_dir_all(project_root.join("test")).unwrap();

        let paths = vec![PathBuf::from("lib"), PathBuf::from("test")];
        let resolved = resolve_watch_paths(&project_root, &paths);

        assert_eq!(resolved.len(), 2);
        assert_eq!(
            resolved[0],
            project_root.join("lib").canonicalize().unwrap()
        );
        assert_eq!(
            resolved[1],
            project_root.join("test").canonicalize().unwrap()
        );
    }

    /// An absolute path is returned as-is (not joined with project_root).
    #[test]
    fn test_resolve_absolute_path_used_as_is() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().to_path_buf();

        // Create a separate absolute directory (not inside project_root).
        let absolute_dir = tempfile::tempdir().unwrap();
        let abs_path = absolute_dir.path().to_path_buf();

        let resolved = resolve_watch_paths(&project_root, &[abs_path.clone()]);

        assert_eq!(resolved.len(), 1);
        // The result must not start with project_root.
        assert!(!resolved[0].starts_with(&project_root));
        // It must equal the canonicalized form of abs_path.
        assert_eq!(resolved[0], abs_path.canonicalize().unwrap());
    }

    /// Mixed absolute and relative paths both resolve correctly in the same call.
    #[test]
    fn test_resolve_mixed_absolute_and_relative() {
        let project_dir = tempfile::tempdir().unwrap();
        let project_root = project_dir.path().to_path_buf();
        std::fs::create_dir_all(project_root.join("lib")).unwrap();

        let absolute_dir = tempfile::tempdir().unwrap();
        let abs_path = absolute_dir.path().to_path_buf();

        let paths = vec![PathBuf::from("lib"), abs_path.clone()];
        let resolved = resolve_watch_paths(&project_root, &paths);

        assert_eq!(resolved.len(), 2);

        // Relative path resolves under project_root.
        assert_eq!(
            resolved[0],
            project_root.join("lib").canonicalize().unwrap()
        );
        // Absolute path is unchanged.
        assert_eq!(resolved[1], abs_path.canonicalize().unwrap());
    }

    /// A non-existent relative path does not panic; the raw (unresolved) path
    /// is returned so the caller can warn about it.
    #[test]
    fn test_resolve_nonexistent_path_does_not_crash() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().to_path_buf();

        // "does_not_exist" is never created.
        let paths = vec![PathBuf::from("does_not_exist")];
        let resolved = resolve_watch_paths(&project_root, &paths);

        // Should return exactly one entry — the raw joined path.
        assert_eq!(resolved.len(), 1);
        // The path should not exist.
        assert!(!resolved[0].exists());
        // But it should still start with project_root (joined, not canonicalized).
        assert!(resolved[0].starts_with(&project_root));
    }

    /// An empty paths list produces an empty result; no panic.
    #[test]
    fn test_resolve_empty_paths_list() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().to_path_buf();

        let resolved = resolve_watch_paths(&project_root, &[]);

        assert!(resolved.is_empty());
    }
}
