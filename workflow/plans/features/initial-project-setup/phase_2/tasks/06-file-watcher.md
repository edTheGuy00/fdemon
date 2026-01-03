## Task: File Watcher Integration

**Objective**: Integrate `notify-debouncer-full` to watch the `lib/` folder for file changes, triggering automatic hot reload with 500ms debouncing.

**Depends on**: 04-reload-commands

---

### Scope

- `Cargo.toml`: MODIFY - Add notify and notify-debouncer-full dependencies
- `src/watcher/mod.rs`: **NEW** - File watcher module
- `src/lib.rs`: MODIFY - Add `pub mod watcher;`
- `src/app/message.rs`: MODIFY - Add file change messages
- `src/main.rs`: MODIFY - Integrate watcher into event loop

---

### Implementation Details

#### New Dependencies

```toml
# Cargo.toml

[dependencies]
# File watching
notify = "7"
notify-debouncer-full = "0.4"
```

#### File Change Messages

```rust
// src/app/message.rs - add to Message enum

/// All possible messages/actions in the application
#[derive(Debug, Clone)]
pub enum Message {
    // ... existing variants ...

    // ─────────────────────────────────────────────────────────
    // File Watcher Messages
    // ─────────────────────────────────────────────────────────
    /// File change detected in watched directory
    FileChanged {
        path: std::path::PathBuf,
        kind: FileChangeKind,
    },
    /// Multiple files changed (debounced batch)
    FilesChanged { count: usize },
    /// Auto-reload triggered by file watcher
    AutoReloadTriggered,
    /// Watcher error occurred
    WatcherError { message: String },
}

/// Kind of file change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}
```

#### Watcher Configuration

```rust
// src/watcher/mod.rs

use std::path::{Path, PathBuf};
use std::time::Duration;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{
    new_debouncer, DebouncedEventKind, Debouncer, FileIdMap,
};
use tokio::sync::mpsc;
use crate::app::message::{FileChangeKind, Message};
use crate::common::prelude::*;

/// Default debounce duration
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
            extensions: DART_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
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
```

#### File Watcher Implementation

```rust
// src/watcher/mod.rs (continued)

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
    /// Sends `Message::FileChanged` or `Message::AutoReloadTriggered` to the channel
    pub fn start(&mut self, message_tx: mpsc::Sender<Message>) -> Result<()> {
        let project_root = self.project_root.clone();
        let config = self.config.clone();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();

        self.stop_tx = Some(stop_tx);

        // Spawn the watcher in a blocking task
        tokio::task::spawn_blocking(move || {
            Self::run_watcher(project_root, config, message_tx, &mut stop_rx)
        });

        Ok(())
    }

    /// Stop the file watcher
    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Internal: run the blocking watcher
    fn run_watcher(
        project_root: PathBuf,
        config: WatcherConfig,
        message_tx: mpsc::Sender<Message>,
        stop_rx: &mut tokio::sync::oneshot::Receiver<()>,
    ) {
        use notify_debouncer_full::DebounceEventResult;

        let tx_clone = message_tx.clone();
        let extensions = config.extensions.clone();
        let auto_reload = config.auto_reload;

        // Create debounced watcher
        let mut debouncer = match new_debouncer(
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

                        tracing::debug!(
                            "File watcher detected {} changes",
                            relevant_events.len()
                        );

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
                            tracing::warn!("File watcher error: {:?}", error);
                            let _ = tx_clone.blocking_send(Message::WatcherError {
                                message: error.to_string(),
                            });
                        }
                    }
                }
            },
        ) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to create file watcher: {}", e);
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
                if let Err(e) = debouncer.watcher().watch(&full_path, RecursiveMode::Recursive) {
                    tracing::warn!("Failed to watch {}: {}", full_path.display(), e);
                } else {
                    tracing::info!("Watching: {}", full_path.display());
                }
            } else {
                tracing::warn!("Watch path does not exist: {}", full_path.display());
            }
        }

        // Keep running until stop signal
        loop {
            match stop_rx.try_recv() {
                Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    tracing::info!("File watcher stopping");
                    break;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running, sleep briefly
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    /// Check if watcher is running
    pub fn is_running(&self) -> bool {
        self.stop_tx.is_some()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}
```

#### Handler Integration

```rust
// src/app/handler.rs - add handlers for watcher messages

Message::AutoReloadTriggered => {
    if let Some(ctx) = ctx {
        // Only auto-reload if app is running and not already reloading
        if ctx.controller.is_running().await && !matches!(state.phase, AppPhase::Reloading) {
            state.log_info(LogSource::Watcher, "File change detected, reloading...");
            Some(UpdateAction::SpawnTask(Task::Reload))
        } else {
            // App not running or already reloading
            tracing::debug!("Auto-reload skipped: app not running or busy");
            None
        }
    } else {
        None
    }
}

Message::FilesChanged { count } => {
    state.log_info(
        LogSource::Watcher,
        format!("{} file(s) changed", count),
    );
    None
}

Message::WatcherError { message } => {
    state.log_error(LogSource::Watcher, format!("Watcher error: {}", message));
    None
}

Message::FileChanged { path, kind } => {
    let kind_str = match kind {
        FileChangeKind::Created => "created",
        FileChangeKind::Modified => "modified",
        FileChangeKind::Deleted => "deleted",
        FileChangeKind::Renamed => "renamed",
    };
    state.log_info(
        LogSource::Watcher,
        format!("File {}: {}", kind_str, path.display()),
    );
    None
}
```

#### Main Event Loop Integration

```rust
// src/main.rs or app runner - integrate watcher

use flutter_demon::watcher::{FileWatcher, WatcherConfig};

async fn run_app(project_path: PathBuf, message_tx: mpsc::Sender<Message>) -> Result<()> {
    // ... existing setup ...

    // Create and start file watcher
    let watcher_config = WatcherConfig::new()
        .with_debounce_ms(500)
        .with_auto_reload(true);

    let mut file_watcher = FileWatcher::new(project_path.clone(), watcher_config);
    
    if let Err(e) = file_watcher.start(message_tx.clone()) {
        tracing::warn!("Failed to start file watcher: {}", e);
        // Continue without file watching - non-fatal
    }

    // ... rest of event loop ...

    // Cleanup on shutdown
    file_watcher.stop();
}
```

---

### Visual Feedback

When auto-reload triggers:

```
┌─────────────────────────────────────────────────────────────────┐
│  🔥 Flutter Demon   [r] Reload  [R] Restart  [s] Stop          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  12:34:56  INF [flutter] App started on iPhone 15 Pro          │
│  12:35:01  INF [watch] File change detected, reloading...      │
│  12:35:01  INF [app] Reloaded in 245ms                         │
│  12:35:15  INF [watch] File change detected, reloading...      │
│  12:35:16  INF [app] Reloaded in 312ms                         │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  ● Running │ iPhone 15 Pro (ios) │ ⏱ 00:05:23 │ ↻ 12:35:16    │
└─────────────────────────────────────────────────────────────────┘
```

---

### Acceptance Criteria

1. [ ] `notify` and `notify-debouncer-full` added to Cargo.toml
2. [ ] `WatcherConfig` allows customizing paths, debounce, extensions
3. [ ] Default config watches `lib/` with 500ms debounce for `.dart` files
4. [ ] `FileWatcher::start()` begins watching specified paths
5. [ ] `FileWatcher::stop()` cleanly terminates the watcher
6. [ ] File changes trigger `Message::AutoReloadTriggered` when enabled
7. [ ] Debouncing coalesces rapid changes (e.g., save-all)
8. [ ] Only `.dart` files trigger reload by default
9. [ ] Non-existent paths logged as warning but don't crash
10. [ ] Watcher errors sent as `Message::WatcherError`
11. [ ] Auto-reload skipped if app not running
12. [ ] Auto-reload skipped if already reloading
13. [ ] Log messages show "[watch]" source for watcher events
14. [ ] Watcher stops cleanly on app shutdown
15. [ ] Unit tests for WatcherConfig

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        
        assert_eq!(config.debounce, Duration::from_millis(500));
        assert!(config.auto_reload);
        assert_eq!(config.paths, vec![PathBuf::from("lib")]);
        assert_eq!(config.extensions, vec!["dart".to_string()]);
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

    // Integration test with real filesystem (requires tempfile)
    #[tokio::test]
    #[ignore] // Requires real filesystem, run manually
    async fn test_file_watcher_integration() {
        use tempfile::TempDir;
        use tokio::fs;
        use tokio::time::sleep;

        let temp_dir = TempDir::new().unwrap();
        let lib_dir = temp_dir.path().join("lib");
        fs::create_dir(&lib_dir).await.unwrap();

        let (tx, mut rx) = mpsc::channel(32);
        let config = WatcherConfig::new().with_debounce_ms(100);
        let mut watcher = FileWatcher::new(temp_dir.path().to_path_buf(), config);

        watcher.start(tx).unwrap();

        // Wait for watcher to initialize
        sleep(Duration::from_millis(200)).await;

        // Create a dart file
        let dart_file = lib_dir.join("test.dart");
        fs::write(&dart_file, "void main() {}").await.unwrap();

        // Wait for debounce
        sleep(Duration::from_millis(300)).await;

        // Should receive auto-reload message
        let msg = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
        assert!(msg.is_ok());
        assert!(matches!(msg.unwrap(), Some(Message::AutoReloadTriggered)));

        watcher.stop();
    }
}
```

---

### Edge Cases

| Edge Case | Handling |
|-----------|----------|
| `lib/` doesn't exist | Log warning, don't crash, continue without watching |
| Permission denied on directory | Log warning, skip that path |
| Rapid saves (save-all) | Debouncer coalesces into single reload |
| Non-dart files changed | Filtered out, no reload triggered |
| App not running | Skip reload, log debug message |
| Already reloading | Skip reload, avoid queue buildup |
| Watcher thread panics | Error sent via channel, app continues |
| Config file in watched dir | Consider excluding `.dart_tool/`, `build/` |

---

### Future Enhancements

- Configuration via `fdemon.toml` (Phase 3)
- Watch additional paths (e.g., `assets/` for asset changes)
- Exclude patterns (e.g., `*.g.dart` generated files)
- Smart reload vs restart based on change type
- Show which file triggered reload in UI
- Pause/resume watching with keyboard shortcut

---

### Notes

- The watcher runs in a blocking thread via `spawn_blocking` because `notify` is synchronous
- `notify-debouncer-full` provides proper debouncing with file ID tracking
- Stop signal uses oneshot channel for clean shutdown
- Non-fatal errors (watcher fails to start) should not crash the app
- Consider adding `build/` and `.dart_tool/` to ignore list by default
- The 500ms debounce matches VS Code's default save delay

---

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `Cargo.toml` | MODIFY | Add notify and notify-debouncer-full |
| `src/watcher/mod.rs` | CREATE | FileWatcher and WatcherConfig |
| `src/lib.rs` | MODIFY | Add `pub mod watcher;` |
| `src/app/message.rs` | MODIFY | Add FileChanged, AutoReloadTriggered messages |
| `src/app/handler.rs` | MODIFY | Handle watcher messages |
| `src/main.rs` | MODIFY | Create and manage FileWatcher instance |

---

## Completion Summary

**Status**: ✅ Done

**Date**: 2026-01-03

### Files Modified

| File | Action | Description |
|------|--------|-------------|
| `Cargo.toml` | MODIFIED | Added `notify = "7"` and `notify-debouncer-full = "0.4"` dependencies |
| `src/watcher/mod.rs` | CREATED | Full implementation of `FileWatcher` and `WatcherConfig` with debouncing |
| `src/lib.rs` | MODIFIED | Added `pub mod watcher;` |
| `src/app/message.rs` | MODIFIED | Added `FilesChanged`, `AutoReloadTriggered`, `WatcherError` message variants |
| `src/app/handler.rs` | MODIFIED | Added handlers for all watcher messages with proper reload logic |
| `src/tui/mod.rs` | MODIFIED | Integrated `FileWatcher` creation, start, and stop in `run_with_project()` |

### Notable Decisions/Tradeoffs

1. **Direct Debouncer API**: Used the newer `debouncer.watch()` API directly instead of deprecated `debouncer.watcher().watch()`.

2. **Blocking Thread for Watcher**: The notify crate is synchronous, so the watcher runs in a blocking thread via `tokio::task::spawn_blocking`. Communication with the async world uses `blocking_send`.

3. **Message-based Integration**: File watcher sends messages (`AutoReloadTriggered`, `FilesChanged`, `WatcherError`) to the unified message channel, allowing the TEA update loop to handle them consistently.

4. **Auto-reload Guards**: The handler checks both `!state.is_busy()` and `current_app_id.is_some()` before triggering reload, preventing duplicate reloads or reloads when no app is running.

5. **Non-fatal Watcher Errors**: If the watcher fails to start (e.g., missing `lib/` directory), the error is logged but the app continues. This matches the spec requirement.

6. **Default Configuration**: Watches `lib/` directory only, 500ms debounce, `.dart` files only, auto-reload enabled by default.

### Testing Performed

```
cargo check    # ✅ Passed
cargo test     # ✅ 198 tests passed (15 new tests: 10 watcher + 5 handler)
cargo clippy   # ✅ No warnings
cargo fmt      # ✅ Applied formatting
```

New tests added:
- `watcher::tests::test_watcher_config_default`
- `watcher::tests::test_watcher_config_new`
- `watcher::tests::test_watcher_config_builder`
- `watcher::tests::test_watcher_config_builder_chaining`
- `watcher::tests::test_file_watcher_creation`
- `watcher::tests::test_file_watcher_stop_when_not_started`
- `watcher::tests::test_file_watcher_double_start_error`
- `watcher::tests::test_default_watch_paths`
- `watcher::tests::test_default_debounce`
- `watcher::tests::test_dart_extensions`
- `handler::tests::test_auto_reload_triggered_when_app_running`
- `handler::tests::test_auto_reload_skipped_when_no_app`
- `handler::tests::test_auto_reload_skipped_when_busy`
- `handler::tests::test_files_changed_logs_count`
- `handler::tests::test_watcher_error_logs_message`

### Risks/Limitations

1. **Platform Differences**: The `notify` crate behavior varies slightly between macOS (FSEvents), Linux (inotify), and Windows (ReadDirectoryChangesW). Testing on macOS primarily.

2. **Missing lib/ Directory**: If `lib/` doesn't exist, a warning is logged but watching silently continues without watching anything. Could be enhanced to retry when directory is created.

3. **No Exclusion Patterns**: Currently watches all `.dart` files in `lib/`. Future enhancement could exclude `*.g.dart`, `*.freezed.dart` and other generated files.

4. **Thread Lifecycle**: The blocking watcher thread polls for stop signals every 100ms. Slightly less responsive than event-based shutdown but simpler and reliable.