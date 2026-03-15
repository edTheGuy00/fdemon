## Task: Add `SharedSourceHandle` and `AppState` Storage

**Objective**: Create a `SharedSourceHandle` type for tracking globally-spawned custom sources, and add a storage field on `AppState` so the TEA loop can track which shared sources are running.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/session/handle.rs`: Define `SharedSourceHandle`
- `crates/fdemon-app/src/state.rs`: Add `shared_source_handles` field to `AppState`

### Details

#### 1. `SharedSourceHandle` Struct

In `session/handle.rs`, add alongside `CustomSourceHandle`:

```rust
/// Handle for a running shared custom log source process.
///
/// Structurally identical to `CustomSourceHandle` but stored at the `AppState`
/// level instead of per-session. Shared sources are spawned once and persist
/// until fdemon quits.
pub struct SharedSourceHandle {
    /// Human-readable source name — used as the log tag.
    pub name: String,
    /// Shutdown sender — send `true` to signal the capture task to stop.
    pub shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
    /// The background task handle — aborted as a fallback on shutdown.
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
    /// Whether this source was started before the Flutter app.
    pub start_before_app: bool,
}
```

#### 2. AppState Field

In `state.rs`, add to `AppState`:

```rust
/// Running shared custom source handles (project-level, not per-session).
///
/// One entry per configured custom source with `shared = true` that has been
/// successfully spawned. Cleaned up only on engine shutdown.
pub shared_source_handles: Vec<SharedSourceHandle>,
```

Initialize to `Vec::new()` in the constructor.

#### 3. Helper Methods on AppState

```rust
/// Shut down all shared custom sources.
///
/// Sends shutdown signal and aborts tasks. Called during engine shutdown.
pub fn shutdown_shared_sources(&mut self) {
    for mut handle in self.shared_source_handles.drain(..) {
        let _ = handle.shutdown_tx.send(true);
        if let Some(task) = handle.task_handle.take() {
            task.abort();
        }
    }
}

/// Returns the names of currently running shared sources.
pub fn running_shared_source_names(&self) -> Vec<String> {
    self.shared_source_handles.iter().map(|h| h.name.clone()).collect()
}

/// Returns true if a shared source with the given name is already running.
pub fn is_shared_source_running(&self, name: &str) -> bool {
    self.shared_source_handles.iter().any(|h| h.name == name)
}
```

### Acceptance Criteria

1. `SharedSourceHandle` struct defined with same fields as `CustomSourceHandle`
2. `AppState.shared_source_handles` field exists, initialized empty
3. `shutdown_shared_sources()`, `running_shared_source_names()`, `is_shared_source_running()` methods exist
4. All existing tests compile and pass (no behavioral change)

### Testing

```rust
#[test]
fn test_shared_source_handles_initialized_empty() { ... }

#[test]
fn test_is_shared_source_running() { ... }

#[test]
fn test_running_shared_source_names() { ... }

#[test]
fn test_shutdown_shared_sources_drains_handles() { ... }
```

### Notes

- `SharedSourceHandle` intentionally duplicates `CustomSourceHandle` rather than sharing a generic — the two have different ownership semantics (per-session vs. global) and may diverge in future
- The `Debug` impl for `AppState` should include the shared source count
