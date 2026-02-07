# Extension API

This document describes how to extend Flutter Demon with custom functionality using the Engine's extension points.

## Overview

Flutter Demon provides two extension mechanisms:

1. **Event Subscription** (`Engine::subscribe()`) — Async broadcast channel for observing domain events. Best for read-only consumers that need async processing (logging, metrics, remote forwarding).

2. **Plugin Trait** (`EnginePlugin`) — Synchronous lifecycle callbacks for tighter integration. Best for features that need to react to every message or participate in the Engine lifecycle.

Both mechanisms were added in Phase 4 of the workspace restructure to support pro features like MCP server integration and remote SSH capabilities.

---

## Event Subscription

### Subscribing to Events

The `Engine::subscribe()` method returns a broadcast receiver that gets `EngineEvent` values after each message processing cycle. Multiple subscribers are supported.

```rust
use fdemon_app::{Engine, EngineEvent};

let engine = Engine::new(project_path);
let mut rx = engine.subscribe();

tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        match event {
            EngineEvent::ReloadCompleted { session_id, time_ms } => {
                println!("Session {} reloaded in {}ms", session_id, time_ms);
            }
            EngineEvent::LogBatch { session_id, entries } => {
                for entry in entries {
                    println!("[{}] {}", entry.level.prefix(), entry.message);
                }
            }
            EngineEvent::PhaseChanged { session_id, old_phase, new_phase } => {
                println!("Session {} phase: {:?} -> {:?}", session_id, old_phase, new_phase);
            }
            _ => {}
        }
    }
});
```

### Available Events

| Event | Description | Payload |
|-------|-------------|---------|
| `SessionCreated` | New session created (device selected) | `session_id`, `device` |
| `SessionStarted` | Flutter process started | `session_id`, `device_id`, `device_name`, `platform`, `pid` |
| `SessionStopped` | Flutter process exited | `session_id`, `reason` |
| `SessionRemoved` | Session removed from manager | `session_id` |
| `PhaseChanged` | App phase transition | `session_id`, `old_phase`, `new_phase` |
| `ReloadStarted` | Hot reload initiated | `session_id` |
| `ReloadCompleted` | Hot reload finished successfully | `session_id`, `time_ms` |
| `ReloadFailed` | Hot reload failed | `session_id`, `reason` |
| `RestartStarted` | Hot restart initiated | `session_id` |
| `RestartCompleted` | Hot restart finished | `session_id` |
| `LogEntry` | Single new log entry | `session_id`, `entry` |
| `LogBatch` | Multiple log entries (high-volume) | `session_id`, `entries` |
| `DevicesDiscovered` | Device list updated | `devices` |
| `FilesChanged` | Source files changed | `count`, `auto_reload_triggered` |
| `Shutdown` | Engine shutting down | — |

### Handling Lagged Events

If a subscriber falls behind, older events are dropped. Detect this using the `RecvError::Lagged` variant:

```rust
use tokio::sync::broadcast::error::RecvError;

loop {
    match rx.recv().await {
        Ok(event) => {
            // Process event
        }
        Err(RecvError::Lagged(n)) => {
            eprintln!("Dropped {} events due to lag", n);
        }
        Err(RecvError::Closed) => {
            break;
        }
    }
}
```

---

## Plugin Trait

### Implementing a Plugin

The `EnginePlugin` trait provides synchronous lifecycle hooks into the Engine. All methods have default no-op implementations, so you only override what you need.

```rust
use fdemon_app::{AppState, Engine, EngineEvent, EnginePlugin, Message};
use fdemon_core::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
struct MetricsPlugin {
    reload_count: AtomicUsize,
}

impl EnginePlugin for MetricsPlugin {
    fn name(&self) -> &str {
        "metrics"
    }

    fn on_start(&self, _state: &AppState) -> Result<()> {
        info!("Metrics plugin started");
        Ok(())
    }

    fn on_message(&self, _msg: &Message, _state: &AppState) -> Result<()> {
        // Called after every message is processed through TEA
        Ok(())
    }

    fn on_event(&self, event: &EngineEvent) -> Result<()> {
        // Called for each EngineEvent (equivalent to subscribe())
        if matches!(event, EngineEvent::ReloadCompleted { .. }) {
            self.reload_count.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    fn on_shutdown(&self) -> Result<()> {
        let count = self.reload_count.load(Ordering::SeqCst);
        info!("Total reloads: {}", count);
        Ok(())
    }
}
```

### Plugin Lifecycle

1. Plugin is registered via `Engine::register_plugin()`
2. `on_start()` is called when the Engine begins its event loop
3. `on_message()` is called after each message is processed
4. `on_event()` is called for each emitted `EngineEvent`
5. `on_shutdown()` is called when the Engine shuts down

### Registering a Plugin

```rust
let mut engine = Engine::new(project_path);

// Register before starting the event loop
engine.register_plugin(Box::new(MetricsPlugin {
    reload_count: AtomicUsize::new(0),
}));

// Notify plugins that engine is starting
engine.notify_plugins_start();

// ... run event loop ...
```

### Thread Safety

Plugins must be `Send + Sync` because the Engine may process messages from multiple async tasks. Use `Arc`, `AtomicUsize`, or other thread-safe primitives for shared state.

---

## Service Layer

The Engine exposes service traits for programmatic access to Flutter operations. These services abstract daemon operations and provide a consistent interface for both TUI and pro features.

### FlutterController

Control the running Flutter application:

```rust
use fdemon_app::services::FlutterController;

// Get controller for the current session
if let Some(controller) = engine.flutter_controller() {
    // Hot reload
    let result = controller.reload().await?;
    if result.success {
        println!("Reload completed in {:?}ms", result.time_ms);
    }

    // Hot restart
    let result = controller.restart().await?;

    // Stop the app
    controller.stop().await?;

    // Check if running
    if controller.is_running().await {
        println!("App is running");
    }
}
```

**Available operations:**
- `reload()` — Trigger hot reload
- `restart()` — Trigger hot restart
- `stop()` — Stop the Flutter app
- `is_running()` — Check if app is running
- `get_app_id()` — Get the current app ID

### LogService

Access the log buffer:

```rust
use fdemon_app::services::LogService;

let log_service = engine.log_service();

// Get recent logs
let logs = log_service.get_logs(100).await;

// Get log count
let count = log_service.count().await;

// Filter logs (future extension point)
// let filtered = log_service.filter(predicate).await;
```

### StateService

Query app state:

```rust
use fdemon_app::services::StateService;

let state_service = engine.state_service();

// Get current phase
let phase = state_service.phase().await;

// Get project info
let info = state_service.project_info().await;
println!("Project: {}", info.name);
println!("Path: {:?}", info.path);

// Check if app is running
let is_running = state_service.is_running().await;
```

---

## Pro Repo Integration

The pro repo includes this repo as a git submodule and depends on specific crates:

```toml
# flutter-demon-pro/crates/fdemon-mcp/Cargo.toml
[dependencies]
fdemon-core = { path = "../../core/crates/fdemon-core" }
fdemon-app = { path = "../../core/crates/fdemon-app" }
```

**Dependency Strategy:**
- `fdemon-core` — For domain types (`LogEntry`, `AppPhase`, `Error`)
- `fdemon-app` — For Engine, services, and extension points
- `fdemon-daemon` — Usually not needed (abstracted by services)
- `fdemon-tui` — Never needed (pro features don't render to terminal)

### MCP Server Example

An MCP server implementation would:

1. **Create an Engine**
   ```rust
   let mut engine = Engine::new(project_path);
   ```

2. **Register an MCP plugin** (optional, for logging/metrics)
   ```rust
   engine.register_plugin(Box::new(McpPlugin::new()));
   ```

3. **Subscribe to EngineEvents** for log/reload forwarding
   ```rust
   let mut rx = engine.subscribe();
   tokio::spawn(async move {
       while let Ok(event) = rx.recv().await {
           // Forward events to MCP clients
       }
   });
   ```

4. **Use service traits** for remote control
   ```rust
   // Handle MCP request for hot reload
   if let Some(controller) = engine.flutter_controller() {
       let result = controller.reload().await?;
       // Send result back to MCP client
   }
   ```

### Desktop App Example

A desktop app (e.g., Tauri-based GUI) would:

1. **Depend on `fdemon-app`** (not `fdemon-tui`)
   ```toml
   [dependencies]
   fdemon-app = { path = "../core/crates/fdemon-app" }
   fdemon-core = { path = "../core/crates/fdemon-core" }
   ```

2. **Create an Engine**
   ```rust
   let mut engine = Engine::new(project_path);
   ```

3. **Implement its own rendering** (web UI via Tauri, etc.)
   - Don't use `fdemon-tui` crate
   - Render `engine.state` in your UI framework

4. **Use `Engine::subscribe()`** for state change notifications
   ```rust
   let mut rx = engine.subscribe();
   tokio::spawn(async move {
       while let Ok(event) = rx.recv().await {
           // Update UI based on events
           match event {
               EngineEvent::LogBatch { entries, .. } => {
                   // Append logs to UI
               }
               EngineEvent::PhaseChanged { new_phase, .. } => {
                   // Update status indicator
               }
               _ => {}
           }
       }
   });
   ```

---

## API Stability

**Phase 4 Extension Points (Stable):**
- `Engine::subscribe()` — Event subscription
- `EnginePlugin` trait — Plugin hooks
- Service traits (`FlutterController`, `LogService`, `StateService`)

**Public Types (Stable):**
- `Engine` — Core orchestration
- `EngineEvent` — Domain events
- `AppState` — TEA model (read-only access)
- `Message` — TEA messages (for plugins)

**Internal Types (Unstable):**
- Types marked `pub(crate)` are internal implementation details
- Only use items exported from `lib.rs` of each crate

---

## Examples

### Simple Event Logger

```rust
use fdemon_app::{Engine, EngineEvent};

let engine = Engine::new(project_path);
let mut rx = engine.subscribe();

tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        println!("[EVENT] {}: {:?}", event.event_type(), event);
    }
});
```

### Reload Performance Monitor

```rust
use fdemon_app::{Engine, EngineEvent, EnginePlugin, AppState, Message};
use fdemon_core::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
struct ReloadMonitor {
    total_time: AtomicU64,
    count: AtomicU64,
}

impl EnginePlugin for ReloadMonitor {
    fn name(&self) -> &str {
        "reload_monitor"
    }

    fn on_event(&self, event: &EngineEvent) -> Result<()> {
        if let EngineEvent::ReloadCompleted { time_ms, .. } = event {
            self.total_time.fetch_add(*time_ms, Ordering::SeqCst);
            self.count.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    fn on_shutdown(&self) -> Result<()> {
        let total = self.total_time.load(Ordering::SeqCst);
        let count = self.count.load(Ordering::SeqCst);
        if count > 0 {
            info!("Average reload time: {}ms", total / count);
        }
        Ok(())
    }
}
```

### Remote Control Handler

```rust
use fdemon_app::{Engine, services::FlutterController};

async fn handle_remote_command(engine: &Engine, command: &str) -> Result<String> {
    match command {
        "reload" => {
            if let Some(controller) = engine.flutter_controller() {
                let result = controller.reload().await?;
                if result.success {
                    Ok(format!("Reload completed in {:?}ms", result.time_ms))
                } else {
                    Ok(format!("Reload failed: {}", result.message.unwrap_or_default()))
                }
            } else {
                Ok("No active session".to_string())
            }
        }
        "restart" => {
            if let Some(controller) = engine.flutter_controller() {
                controller.restart().await?;
                Ok("Restart initiated".to_string())
            } else {
                Ok("No active session".to_string())
            }
        }
        _ => Ok(format!("Unknown command: {}", command)),
    }
}
```

---

## Troubleshooting

### Plugin Not Called

- Verify you called `engine.register_plugin()` before `notify_plugins_start()`
- Check that plugin methods return `Ok(())`, not `Err(...)`
- Plugin errors are logged but don't crash the Engine

### Events Not Received

- Verify you called `engine.subscribe()` before messages are processed
- Check for `RecvError::Lagged` — you may be falling behind
- Ensure the receiver is not dropped while events are being emitted

### Service Methods Return None

- `engine.flutter_controller()` returns `None` if no session is active
- Call methods after a Flutter session has been created
- Check `engine.state.session_manager.selected()` to verify session exists

---

## See Also

- [Architecture Documentation](./ARCHITECTURE.md) — Overall system architecture
- [Development Guide](./DEVELOPMENT.md) — Build and test commands
- [Code Standards](./CODE_STANDARDS.md) — Coding conventions
