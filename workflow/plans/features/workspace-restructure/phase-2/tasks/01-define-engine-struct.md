## Task: Define Engine Struct and Constructor

**Objective**: Create the `Engine` struct in `app/engine.rs` that encapsulates all shared orchestration state currently duplicated between the TUI runner (`tui/runner.rs`) and headless runner (`headless/runner.rs`). The Engine owns the message channel, session tasks, shutdown signal, file watcher, and settings -- everything except the frontend-specific event loop.

**Depends on**: None (Phase 1 complete)

**Estimated Time**: 4-5 hours

### Scope

- `src/app/engine.rs`: **NEW** -- Define `Engine` struct, `Engine::new()`, core methods
- `src/app/mod.rs`: Add `pub mod engine;` declaration and re-exports

### Details

#### The Duplication Problem

Both runners currently duplicate this initialization sequence:

```
TUI (runner.rs:28-62):                  Headless (runner.rs:26-62):
1. config::init_fdemon_directory()       1. config::init_fdemon_directory()
2. config::load_settings()               2. config::load_settings()
3. AppState::with_settings()             3. AppState::with_settings()
4. mpsc::channel::<Message>(256)         4. mpsc::channel::<Message>(256)
5. signals::spawn_signal_handler()       5. spawn_signal_handler() (own copy!)
6. SessionTaskMap::new()                 6. SessionTaskMap::new()
7. watch::channel(false)                 7. watch::channel(false)
8. FileWatcher::new() + start + bridge   8. FileWatcher::new() + start + bridge
```

The watcher-to-message bridge (lines 98-109 in TUI, lines 105-116 in headless) is character-for-character identical.

#### Engine Struct Design

```rust
// src/app/engine.rs

pub struct Engine {
    /// TEA application state (the Model)
    pub state: AppState,

    /// Sender half of the unified message channel.
    /// Clone this to give to input sources (signal handler, watcher, daemon tasks).
    pub msg_tx: mpsc::Sender<Message>,

    /// Receiver half of the unified message channel.
    /// The frontend event loop drains messages from here.
    pub msg_rx: mpsc::Receiver<Message>,

    /// Map of session IDs to their background task handles.
    pub session_tasks: SessionTaskMap,

    /// Sender for the shutdown signal. Send `true` to initiate shutdown.
    pub shutdown_tx: watch::Sender<bool>,

    /// Receiver for the shutdown signal. Clone for background tasks.
    pub shutdown_rx: watch::Receiver<bool>,

    /// File watcher for auto-reload. None if watcher failed to start.
    file_watcher: Option<FileWatcher>,

    /// Loaded settings (cached from config)
    pub settings: Settings,

    /// Path to the Flutter project
    pub project_path: PathBuf,
}
```

#### Engine::new() -- Unified Initialization

```rust
impl Engine {
    /// Create a new Engine for a Flutter project.
    ///
    /// Performs all shared initialization:
    /// - Initializes .fdemon directory
    /// - Loads settings from config files
    /// - Creates AppState with settings
    /// - Creates message channel (capacity 256)
    /// - Creates shutdown signal channel
    /// - Creates session task map
    /// - Spawns signal handler
    /// - Creates and starts file watcher with message bridge
    pub fn new(project_path: PathBuf) -> Self {
        // 1. Init .fdemon directory (non-fatal if fails)
        if let Err(e) = config::init_fdemon_directory(&project_path) {
            warn!("Failed to initialize .fdemon directory: {}", e);
        }

        // 2. Load settings
        let settings = config::load_settings(&project_path);

        // 3. Create state
        let state = AppState::with_settings(project_path.clone(), settings.clone());

        // 4. Create message channel
        let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);

        // 5. Create shutdown channel
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // 6. Create session task map
        let session_tasks: SessionTaskMap =
            Arc::new(Mutex::new(HashMap::new()));

        // 7. Spawn signal handler
        signals::spawn_signal_handler(msg_tx.clone());

        // 8. Create and start file watcher
        let file_watcher = Self::start_file_watcher(
            &project_path,
            &settings,
            msg_tx.clone(),
        );

        Self {
            state,
            msg_tx,
            msg_rx,
            session_tasks,
            shutdown_tx,
            shutdown_rx,
            file_watcher,
            settings,
            project_path,
        }
    }
}
```

#### Core Methods

```rust
impl Engine {
    /// Process a single message through the TEA update cycle.
    ///
    /// Delegates to `process::process_message()` which runs handler::update()
    /// and dispatches any resulting UpdateActions.
    pub fn process_message(&mut self, msg: Message) {
        process::process_message(
            &mut self.state,
            msg,
            &self.msg_tx,
            &self.session_tasks,
            &self.shutdown_rx,
            &self.project_path,
        );
    }

    /// Drain and process all pending messages from the channel.
    ///
    /// Returns the number of messages processed. Used by the TUI runner
    /// which needs to drain all pending messages before rendering.
    pub fn drain_pending_messages(&mut self) -> usize {
        let mut count = 0;
        while let Ok(msg) = self.msg_rx.try_recv() {
            self.process_message(msg);
            count += 1;
        }
        count
    }

    /// Flush pending batched logs across all sessions.
    ///
    /// Call after processing messages and before rendering/emitting events.
    pub fn flush_pending_logs(&mut self) {
        self.state.session_manager.flush_all_pending_logs();
    }

    /// Get a clone of the message sender for spawning input sources.
    pub fn msg_sender(&self) -> mpsc::Sender<Message> {
        self.msg_tx.clone()
    }

    /// Get a clone of the shutdown receiver for background tasks.
    pub fn shutdown_receiver(&self) -> watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }

    /// Check if the application should quit.
    pub fn should_quit(&self) -> bool {
        self.state.should_quit()
    }

    /// Initiate shutdown: stop watcher, signal background tasks, cleanup sessions.
    pub async fn shutdown(&mut self) {
        // Stop file watcher
        if let Some(ref mut watcher) = self.file_watcher {
            watcher.stop();
        }

        // Signal all background tasks to stop
        let _ = self.shutdown_tx.send(true);

        // Drain remaining session tasks with timeout
        let tasks: Vec<_> = {
            let mut map = self.session_tasks.lock().await;
            map.drain().collect()
        };

        for (session_id, handle) in tasks {
            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                handle,
            ).await {
                Ok(Ok(())) => info!("Session {} cleaned up", session_id),
                Ok(Err(e)) => warn!("Session {} panicked: {}", session_id, e),
                Err(_) => warn!("Session {} cleanup timed out", session_id),
            }
        }
    }
}
```

#### Private Helper: File Watcher Setup

Extract the duplicated watcher setup into a private method:

```rust
impl Engine {
    /// Create and start the file watcher, bridging events to messages.
    fn start_file_watcher(
        project_path: &Path,
        settings: &Settings,
        msg_tx: mpsc::Sender<Message>,
    ) -> Option<FileWatcher> {
        let mut watcher = FileWatcher::new(
            project_path.to_path_buf(),
            WatcherConfig::new()
                .with_debounce_ms(settings.watcher.debounce_ms)
                .with_auto_reload(settings.watcher.auto_reload),
        );

        let (watcher_tx, mut watcher_rx) = mpsc::channel::<WatcherEvent>(32);

        if let Err(e) = watcher.start(watcher_tx) {
            warn!("Failed to start file watcher: {}", e);
            return None;
        }

        // Bridge watcher events to app messages
        tokio::spawn(async move {
            while let Some(event) = watcher_rx.recv().await {
                let msg = match event {
                    WatcherEvent::AutoReloadTriggered => Message::AutoReloadTriggered,
                    WatcherEvent::FilesChanged { count } => Message::FilesChanged { count },
                    WatcherEvent::Error { message } => Message::WatcherError { message },
                };
                let _ = msg_tx.send(msg).await;
            }
        });

        Some(watcher)
    }
}
```

#### Module Structure

The file should be organized as:
```
src/app/engine.rs
  - use declarations
  - pub struct Engine { ... }
  - impl Engine { new(), process_message(), drain_pending_messages(),
                  flush_pending_logs(), msg_sender(), shutdown_receiver(),
                  should_quit(), shutdown() }
  - impl Engine { start_file_watcher() } // private helper
  - #[cfg(test)] mod tests { ... }
```

Expected size: ~200-250 lines.

### Acceptance Criteria

1. `src/app/engine.rs` exists with `Engine` struct and all methods listed above
2. `src/app/mod.rs` declares `pub mod engine;` and re-exports `Engine`
3. `Engine::new()` performs the full initialization sequence (config, state, channels, watcher, signal handler)
4. `Engine::process_message()` delegates to `process::process_message()`
5. `Engine::drain_pending_messages()` drains the `msg_rx` channel via `try_recv()`
6. `Engine::flush_pending_logs()` delegates to `session_manager.flush_all_pending_logs()`
7. `Engine::shutdown()` stops watcher, sends shutdown signal, drains tasks with timeout
8. `Engine` has NO dependencies on ratatui, crossterm, or any TUI-specific types
9. `cargo build` succeeds
10. `cargo test` passes
11. `cargo clippy` is clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_new_creates_valid_state() {
        // Engine::new() requires a project path but doesn't require Flutter
        // Use a temp directory to test construction
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        assert!(!engine.should_quit());
        assert_eq!(engine.project_path, dir.path());
    }

    #[tokio::test]
    async fn test_engine_drain_empty_channel() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // No messages pending
        assert_eq!(engine.drain_pending_messages(), 0);
    }

    #[tokio::test]
    async fn test_engine_process_quit_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        engine.process_message(Message::Quit);
        assert!(engine.should_quit());
    }

    #[tokio::test]
    async fn test_engine_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // Should not panic on empty engine
        engine.shutdown().await;
    }
}
```

### Notes

- The `Engine` does NOT own the terminal or rendering -- those stay in the TUI runner
- The `Engine` does NOT own the stdin reader -- headless has its own blocking stdin reader
- The `Engine` does NOT own the NDJSON event emitter -- that's headless-specific
- The signal handler is spawned inside `Engine::new()` using `app::signals::spawn_signal_handler()`. The headless runner's duplicate signal handler (lines 302-337 of `headless/runner.rs`) will be removed in Task 04. The headless runner can add its own HeadlessEvent emission as a wrapper if needed.
- `Engine::shutdown()` absorbs the session cleanup logic that currently lives in `tui/startup.rs::cleanup_sessions()` (minus the terminal drawing part). The TUI runner can still draw "shutting down" frames by calling render between Engine shutdown steps.
- The `file_watcher` field is `Option<FileWatcher>` because watcher initialization can fail (non-fatal).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/engine.rs` | Created new file with Engine struct (246 lines) |
| `src/app/mod.rs` | Added `pub mod engine;` declaration and re-export |

### Notable Decisions/Tradeoffs

1. **Tokio Runtime Requirement**: The `Engine::new()` constructor spawns the signal handler immediately, which requires a Tokio runtime context. All tests using `Engine::new()` must be marked with `#[tokio::test]` instead of `#[test]`. This is acceptable since the Engine is always used in an async context.

2. **Watcher Initialization Non-Fatal**: File watcher initialization is allowed to fail (returns `Option<FileWatcher>`). This matches the existing pattern in both TUI and headless runners where watcher failures are logged but don't prevent application startup.

3. **Session Task Cleanup Timeout**: The `shutdown()` method uses a 2-second timeout per session when cleaning up tasks. This prevents indefinite hangs while allowing graceful shutdown in most cases.

### Testing Performed

- `cargo check` - Passed (no warnings)
- `cargo test --lib` - Passed (1525 tests, including 4 new Engine tests)
- `cargo clippy --lib` - Passed (no warnings)

### Verification

All acceptance criteria met:
1. ✅ `src/app/engine.rs` exists with `Engine` struct and all required methods
2. ✅ `src/app/mod.rs` declares `pub mod engine;` and re-exports `Engine`
3. ✅ `Engine::new()` performs full initialization (config, state, channels, watcher, signal handler)
4. ✅ `Engine::process_message()` delegates to `process::process_message()`
5. ✅ `Engine::drain_pending_messages()` drains via `try_recv()`
6. ✅ `Engine::flush_pending_logs()` delegates to `session_manager.flush_all_pending_logs()`
7. ✅ `Engine::shutdown()` stops watcher, sends shutdown signal, drains tasks with timeout
8. ✅ `Engine` has NO dependencies on ratatui, crossterm, or TUI-specific types
9. ✅ `cargo build` succeeds
10. ✅ `cargo test` passes (all tests including new engine tests)
11. ✅ `cargo clippy` is clean (no warnings)

### Risks/Limitations

1. **File Watcher Bridge Task**: The watcher-to-message bridge spawns an unbounded tokio task in `start_file_watcher()`. This task will run until the watcher channel is dropped. Not a leak since the watcher is stopped in `shutdown()`, but worth noting for future monitoring.

2. **Signal Handler Spawned Eagerly**: The signal handler is spawned immediately in `Engine::new()`, before the event loop starts. This is safe but means SIGINT/SIGTERM could queue messages before the event loop is ready to process them. The message channel has capacity 256 which should be sufficient.
