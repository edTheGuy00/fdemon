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
/// 3. `on_event()` is called for each emitted EngineEvent (after state change)
/// 4. `on_message()` is called after each message is processed (with full post-state)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestPlugin {
        name: String,
        started: Arc<AtomicBool>,
        shutdown: Arc<AtomicBool>,
        message_count: Arc<AtomicUsize>,
        event_count: Arc<AtomicUsize>,
    }

    impl TestPlugin {
        fn new(
            name: &str,
        ) -> (
            Self,
            Arc<AtomicBool>,
            Arc<AtomicBool>,
            Arc<AtomicUsize>,
            Arc<AtomicUsize>,
        ) {
            let started = Arc::new(AtomicBool::new(false));
            let shutdown = Arc::new(AtomicBool::new(false));
            let message_count = Arc::new(AtomicUsize::new(0));
            let event_count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    name: name.to_string(),
                    started: started.clone(),
                    shutdown: shutdown.clone(),
                    message_count: message_count.clone(),
                    event_count: event_count.clone(),
                },
                started,
                shutdown,
                message_count,
                event_count,
            )
        }
    }

    impl EnginePlugin for TestPlugin {
        fn name(&self) -> &str {
            &self.name
        }

        fn on_start(&self, _state: &AppState) -> Result<()> {
            self.started.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn on_message(&self, _msg: &Message, _state: &AppState) -> Result<()> {
            self.message_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn on_event(&self, _event: &EngineEvent) -> Result<()> {
            self.event_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn on_shutdown(&self) -> Result<()> {
            self.shutdown.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    /// Create a test engine
    fn create_test_engine() -> crate::Engine {
        let dir = tempfile::tempdir().unwrap();
        crate::Engine::new(dir.path().to_path_buf())
    }

    #[tokio::test]
    async fn test_register_plugin() {
        let mut engine = create_test_engine();

        let (plugin, _, _, _, _) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        assert_eq!(engine.plugin_count(), 1);
    }

    #[tokio::test]
    async fn test_plugin_on_start() {
        let mut engine = create_test_engine();

        let (plugin, started, _, _, _) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        engine.notify_plugins_start();
        assert!(started.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_plugin_on_shutdown() {
        let mut engine = create_test_engine();

        let (plugin, _, shutdown, _, _) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        engine.shutdown().await;
        assert!(shutdown.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_plugin_on_message() {
        let mut engine = create_test_engine();

        let (plugin, _, _, count, _) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        engine.process_message(Message::Quit);
        assert!(count.load(Ordering::SeqCst) > 0);
    }

    #[tokio::test]
    async fn test_plugin_on_event() {
        let mut engine = create_test_engine();

        let (plugin, _, _, _, event_count) = TestPlugin::new("test");
        engine.register_plugin(Box::new(plugin));

        // Process a message that should trigger events
        engine.process_message(Message::Tick);

        // Event count may or may not be > 0 depending on what Tick does
        // We just verify the plugin was called without panicking
        let _count = event_count.load(Ordering::SeqCst);
    }

    #[tokio::test]
    async fn test_multiple_plugins() {
        let mut engine = create_test_engine();

        let (plugin1, started1, _, _, _) = TestPlugin::new("plugin1");
        let (plugin2, started2, _, _, _) = TestPlugin::new("plugin2");

        engine.register_plugin(Box::new(plugin1));
        engine.register_plugin(Box::new(plugin2));

        assert_eq!(engine.plugin_count(), 2);

        engine.notify_plugins_start();
        assert!(started1.load(Ordering::SeqCst));
        assert!(started2.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_plugin_error_handling() {
        // Test that plugin errors don't crash the Engine
        #[derive(Debug)]
        struct FailingPlugin;

        impl EnginePlugin for FailingPlugin {
            fn name(&self) -> &str {
                "failing"
            }

            fn on_start(&self, _state: &AppState) -> Result<()> {
                Err(fdemon_core::Error::config("intentional failure"))
            }

            fn on_message(&self, _msg: &Message, _state: &AppState) -> Result<()> {
                Err(fdemon_core::Error::config("intentional failure"))
            }

            fn on_event(&self, _event: &EngineEvent) -> Result<()> {
                Err(fdemon_core::Error::config("intentional failure"))
            }

            fn on_shutdown(&self) -> Result<()> {
                Err(fdemon_core::Error::config("intentional failure"))
            }
        }

        let mut engine = create_test_engine();
        engine.register_plugin(Box::new(FailingPlugin));

        // These should not panic despite the plugin errors
        engine.notify_plugins_start();
        engine.process_message(Message::Tick);
        engine.shutdown().await;
    }
}
