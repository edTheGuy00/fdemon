## Task: Add EnginePlugin Trait

**Objective**: Create an `EnginePlugin` trait that provides a callback-based extension mechanism for the Engine. This complements the existing event-based `Engine::subscribe()` approach by allowing plugins to hook into the Engine lifecycle with tighter integration (e.g., receiving messages before processing, modifying behavior).

**Depends on**: 03-lock-down-fdemon-app

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-app/src/plugin.rs`: **NEW** - EnginePlugin trait definition
- `crates/fdemon-app/src/engine.rs`: Add plugin registration and lifecycle hooks
- `crates/fdemon-app/src/lib.rs`: Export plugin module and trait

### Details

#### 1. Create Plugin Module

Create `crates/fdemon-app/src/plugin.rs` with the EnginePlugin trait:

```rust
//! Plugin system for extending Engine functionality.
//!
//! The `EnginePlugin` trait allows external code (e.g., pro features like MCP server,
//! remote SSH) to hook into the Engine lifecycle. Plugins receive callbacks for
//! engine start, message processing, and shutdown.
//!
//! For simpler use cases (read-only event observation), prefer `Engine::subscribe()`
//! which provides a broadcast channel of `EngineEvent` values.

use std::fmt;

use crate::engine_event::EngineEvent;
use crate::message::Message;
use crate::state::AppState;
use fdemon_core::prelude::*;

/// Extension trait for Engine plugins.
///
/// Plugins hook into the Engine lifecycle to add functionality without modifying
/// the core Engine code. Each callback has a default no-op implementation,
/// so plugins only need to override the hooks they care about.
///
/// # Plugin Lifecycle
///
/// 1. Plugin is registered via `Engine::register_plugin()`
/// 2. `on_start()` is called when the Engine begins its event loop
/// 3. `on_message()` is called after each message is processed
/// 4. `on_event()` is called for each emitted EngineEvent
/// 5. `on_shutdown()` is called when the Engine shuts down
///
/// # Thread Safety
///
/// Plugins must be `Send + Sync` because the Engine may process messages
/// from multiple async tasks.
pub trait EnginePlugin: Send + Sync + fmt::Debug {
    /// Unique name for this plugin (for logging and identification).
    fn name(&self) -> &str;

    /// Called when the Engine starts its event loop.
    ///
    /// Use this to initialize plugin state, start background tasks, etc.
    /// The AppState is provided read-only for initial state inspection.
    fn on_start(&self, _state: &AppState) -> Result<()> {
        Ok(())
    }

    /// Called after a message has been processed through the TEA update cycle.
    ///
    /// The message and resulting state are provided for inspection.
    /// This is called for every message, including internal ones.
    fn on_message(&self, _msg: &Message, _state: &AppState) -> Result<()> {
        Ok(())
    }

    /// Called for each EngineEvent emitted after message processing.
    ///
    /// This is equivalent to subscribing via `Engine::subscribe()` but
    /// with synchronous, in-process delivery.
    fn on_event(&self, _event: &EngineEvent) -> Result<()> {
        Ok(())
    }

    /// Called when the Engine is shutting down.
    ///
    /// Use this to clean up resources, flush buffers, close connections.
    /// This is called before the Engine's own shutdown logic.
    fn on_shutdown(&self) -> Result<()> {
        Ok(())
    }
}
```

#### 2. Add Plugin Storage to Engine

**In `engine.rs`**, add a plugins vector and registration methods:

```rust
use crate::plugin::EnginePlugin;

pub struct Engine {
    // ... existing fields ...

    /// Registered plugins
    plugins: Vec<Box<dyn EnginePlugin>>,
}
```

Initialize in `Engine::new()`:
```rust
plugins: Vec::new(),
```

Add registration methods:
```rust
/// Register a plugin with the Engine.
///
/// Plugins receive lifecycle callbacks (on_start, on_message, on_event, on_shutdown).
/// Multiple plugins can be registered. They are called in registration order.
pub fn register_plugin(&mut self, plugin: Box<dyn EnginePlugin>) {
    info!("Registering plugin: {}", plugin.name());
    self.plugins.push(plugin);
}

/// Get the number of registered plugins.
pub fn plugin_count(&self) -> usize {
    self.plugins.len()
}
```

#### 3. Add Plugin Lifecycle Hooks

**In `engine.rs`**, call plugin hooks at appropriate points:

**`process_message()`** -- call `on_message()` after processing:
```rust
pub fn process_message(&mut self, msg: Message) {
    let pre = StateSnapshot::capture(&self.state);

    process::process_message(
        &mut self.state, msg, &self.msg_tx, &self.session_tasks,
        &self.shutdown_rx, &self.project_path,
    );

    let post = StateSnapshot::capture(&self.state);
    self.emit_events(&pre, &post);

    // Notify plugins (after state update and event emission)
    self.notify_plugins_message(&msg, &self.state);
}
```

Note: This requires cloning `msg` before passing to `process::process_message` since it takes ownership, or changing the approach. The simplest option is to capture the message type info before processing:

```rust
pub fn process_message(&mut self, msg: Message) {
    let pre = StateSnapshot::capture(&self.state);

    // Clone message for plugin notification (lightweight for most variants)
    let msg_for_plugins = msg.clone();

    process::process_message(
        &mut self.state, msg, &self.msg_tx, &self.session_tasks,
        &self.shutdown_rx, &self.project_path,
    );

    let post = StateSnapshot::capture(&self.state);
    self.emit_events(&pre, &post);

    // Notify plugins
    self.notify_plugins_message(&msg_for_plugins);
}
```

**`shutdown()`** -- call `on_shutdown()` before cleanup:
```rust
pub async fn shutdown(&mut self) {
    // Notify plugins first
    self.notify_plugins_shutdown();

    // Emit shutdown event
    self.emit(EngineEvent::Shutdown);
    // ... rest of shutdown logic ...
}
```

**`emit_events()`** -- call `on_event()` for each event:
```rust
fn emit(&self, event: EngineEvent) {
    // Notify event subscribers (broadcast channel)
    let _ = self.event_tx.send(event.clone());

    // Notify plugins
    for plugin in &self.plugins {
        if let Err(e) = plugin.on_event(&event) {
            warn!("Plugin '{}' on_event error: {}", plugin.name(), e);
        }
    }
}
```

#### 4. Add Plugin Notification Helpers

```rust
/// Notify all plugins that a message was processed.
fn notify_plugins_message(&self, msg: &Message) {
    for plugin in &self.plugins {
        if let Err(e) = plugin.on_message(msg, &self.state) {
            warn!("Plugin '{}' on_message error: {}", plugin.name(), e);
        }
    }
}

/// Notify all plugins about startup.
pub fn notify_plugins_start(&self) {
    for plugin in &self.plugins {
        if let Err(e) = plugin.on_start(&self.state) {
            warn!("Plugin '{}' on_start error: {}", plugin.name(), e);
        }
    }
}

/// Notify all plugins about shutdown.
fn notify_plugins_shutdown(&self) {
    for plugin in &self.plugins {
        if let Err(e) = plugin.on_shutdown() {
            warn!("Plugin '{}' on_shutdown error: {}", plugin.name(), e);
        }
    }
}
```

#### 5. Export from lib.rs

**In `lib.rs`**, add the plugin module and re-export the trait:

```rust
pub mod plugin;

// Re-export primary types
pub use engine::Engine;
pub use engine_event::EngineEvent;
pub use plugin::EnginePlugin;
// ... existing re-exports ...
```

#### 6. Add Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestPlugin {
        name: String,
        started: Arc<AtomicBool>,
        shutdown: Arc<AtomicBool>,
        message_count: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl TestPlugin {
        fn new(name: &str) -> (Self, Arc<AtomicBool>, Arc<AtomicBool>, Arc<std::sync::atomic::AtomicUsize>) {
            let started = Arc::new(AtomicBool::new(false));
            let shutdown = Arc::new(AtomicBool::new(false));
            let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            (
                Self {
                    name: name.to_string(),
                    started: started.clone(),
                    shutdown: shutdown.clone(),
                    message_count: count.clone(),
                },
                started, shutdown, count,
            )
        }
    }

    impl EnginePlugin for TestPlugin {
        fn name(&self) -> &str { &self.name }

        fn on_start(&self, _state: &AppState) -> Result<()> {
            self.started.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn on_message(&self, _msg: &Message, _state: &AppState) -> Result<()> {
            self.message_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn on_shutdown(&self) -> Result<()> {
            self.shutdown.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_register_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let (plugin, _, _, _) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        assert_eq!(engine.plugin_count(), 1);
    }

    #[tokio::test]
    async fn test_plugin_on_start() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let (plugin, started, _, _) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        engine.notify_plugins_start();
        assert!(started.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_plugin_on_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let (plugin, _, shutdown, _) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        engine.shutdown().await;
        assert!(shutdown.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_plugin_on_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let (plugin, _, _, count) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        engine.process_message(Message::Quit);
        assert!(count.load(Ordering::SeqCst) > 0);
    }
}
```

### Acceptance Criteria

1. `EnginePlugin` trait exists in `crates/fdemon-app/src/plugin.rs`
2. Trait has `name()`, `on_start()`, `on_message()`, `on_event()`, `on_shutdown()` methods
3. All methods except `name()` have default no-op implementations
4. `Engine::register_plugin()` method exists
5. Plugins are notified on message processing and shutdown
6. Plugin errors are logged but do not crash the Engine
7. `EnginePlugin` is re-exported from `fdemon-app` crate root
8. Tests verify plugin lifecycle (register, start, message, shutdown)
9. `cargo check -p fdemon-app` passes
10. `cargo test -p fdemon-app` passes
11. `cargo check --workspace` passes
12. `cargo test --workspace` passes

### Testing

```bash
# Crate-level verification
cargo check -p fdemon-app
cargo test -p fdemon-app

# Full workspace verification
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
```

### Notes

- The plugin trait is intentionally minimal -- start with 4 hooks and expand based on actual pro feature needs
- Do NOT add `on_pre_message()` (before processing) -- this adds complexity and the use case is unclear
- Plugin errors are logged with `warn!` but do NOT propagate to the caller. A misbehaving plugin should not crash the Engine
- The `on_event()` hook provides synchronous in-process delivery. For async consumers, `Engine::subscribe()` is preferred
- `Message` needs to be `Clone` for the plugin notification in `process_message()`. Check if it already is -- if not, derive `Clone` on `Message` (it should be lightweight since most variants are small)
- The `notify_plugins_start()` method is `pub` because the runner (TUI or headless) calls it after registering plugins and before entering the event loop
- Do NOT add an `unregister_plugin()` method -- plugins are registered at startup and live for the Engine's lifetime
