# Code Standards

This document defines the coding standards, idioms, and quality expectations for this project.

## Language & Runtime

- **Primary Language:** Rust
- **Secondary Languages:** Dart (Flutter integration)
- **Async Runtime:** Tokio

## Rust Idioms & Best Practices

### Ownership & Borrowing

| Check | What to Look For |
|-------|------------------|
| **Ownership** | Unnecessary clones, missing borrows, lifetime issues |
| **Error Handling** | Raw `unwrap()`, missing error context, swallowed errors |
| **Option/Result** | Proper use of `?`, `map`, `and_then`, `ok_or` |
| **Iteration** | Prefer iterators over index loops, avoid collect-then-iterate |
| **Mutability** | Minimize `mut`, prefer immutable bindings |
| **Pattern Matching** | Exhaustive matches, avoid catch-all `_` when variants matter |

### Error Handling

- All errors MUST use the `Error` enum from `common/error.rs`
- Use the `Result<T>` type alias from prelude, not `std::result::Result`
- Errors should be classified as `fatal` or `recoverable`
- Add rich context via `.context()` or `.with_context()`

### Logging

- Use `tracing` macros (`info!`, `warn!`, `error!`, `debug!`)
- NEVER use `println!` or `eprintln!` (stdout is owned by TUI)

### Module Organization

- Public API in `mod.rs`, implementation in submodules
- Files > 500 lines should be split into submodules
- Functions > 50 lines should be refactored

## Common Anti-Patterns

### ❌ Panicking in Library Code

```rust
// ❌ BAD: Panicking in library code
let value = some_option.unwrap();

// ✅ GOOD: Proper error handling
let value = some_option.ok_or_else(|| Error::config("missing value"))?;
```

### ❌ Ignoring Errors

```rust
// ❌ BAD: Ignoring errors
let _ = do_something();

// ✅ GOOD: Handle or propagate
do_something()?;
```

### ❌ Clone-Heavy Code

```rust
// ❌ BAD: Clone-heavy code
let items: Vec<_> = self.items.clone().into_iter().filter(...).collect();

// ✅ GOOD: Iterate by reference
let items: Vec<_> = self.items.iter().filter(...).cloned().collect();
```

### ❌ Stringly-Typed Errors

```rust
// ❌ BAD: Stringly-typed errors
Err("something went wrong".into())

// ✅ GOOD: Typed errors with context
Err(Error::config(format!("failed to parse {}: {}", path, e)))
```

### ❌ Magic Numbers

```rust
// ❌ BAD: Magic numbers
if items.len() > 100 { ... }

// ✅ GOOD: Named constants
const MAX_LOG_BUFFER_SIZE: usize = 100;
if items.len() > MAX_LOG_BUFFER_SIZE { ... }
```

## Red Flags

| Red Flag | Why It's Dangerous |
|----------|-------------------|
| `unwrap()` or `expect()` without justification | Panic in production |
| Index access without bounds checking | Panic on empty/short collections |
| Assumptions about data ordering | Race conditions, undefined behavior |
| Missing `else` branches | Unhandled cases silently pass |
| Mutable state shared across async boundaries | Data races |
| Early returns that skip cleanup | Resource leaks |
| Magic numbers without constants | Future confusion, maintenance burden |
| Negated conditions in complex logic | Easy to misread, invert incorrectly |

## Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Modules | `snake_case` | `log_view`, `session_manager` |
| Types | `PascalCase` | `AppState`, `LogEntry` |
| Functions | `snake_case` | `parse_message`, `handle_event` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_SESSIONS`, `DEFAULT_TIMEOUT` |
| Message variants | `PascalCase`, verb-based | `HotReload`, `ShowDeviceSelector` |

## Testing Standards

### Coverage Requirements

- All new public functions must have tests
- Edge cases must be covered (empty inputs, boundary conditions, error paths)
- Tests should be isolated (no shared mutable state)
- Use `tempdir()` for file-based tests

### Test Naming

Use descriptive names that describe the scenario and expected outcome:

```rust
// ✅ GOOD: Descriptive test names
#[test]
fn test_parse_invalid_json_returns_error() { ... }

#[test]
fn test_empty_log_buffer_returns_none() { ... }

// ❌ BAD: Vague test names
#[test]
fn test_parse() { ... }

#[test]
fn test_it_works() { ... }
```

## Documentation Requirements

### Public Items

All `pub` functions and types must have `///` doc comments:

```rust
/// Parses a JSON-RPC message from the Flutter daemon.
///
/// # Arguments
/// * `line` - Raw stdout line from the daemon
///
/// # Returns
/// * `Some(DaemonMessage)` if valid JSON-RPC
/// * `None` if not a daemon message
pub fn parse_message(line: &str) -> Option<DaemonMessage> { ... }
```

### Module Documentation

Each module should have a `//!` header explaining its purpose:

```rust
//! # Log View Widget
//!
//! This module provides a scrollable log viewer with filtering,
//! search, and syntax highlighting capabilities.
```

## Severity Levels

| Level | Meaning | Example |
|-------|---------|---------|
| 🔴 **CRITICAL** | Must fix before merge | Panics in production, data corruption, security issue |
| 🟠 **MAJOR** | Should fix before merge | Missing error handling, logic bugs, performance issue |
| 🟡 **MINOR** | Fix soon | Style violations, missing docs, minor inefficiencies |
| 🔵 **NITPICK** | Nice to have | Subjective style preferences, minor naming suggestions |

## Quality Metrics

When reviewing code, assess these dimensions:

| Metric | What to Evaluate |
|--------|------------------|
| **Rust Idioms** | Ownership, borrowing, iterators, pattern matching |
| **Error Handling** | Proper Result/Option usage, error context |
| **Testing** | Coverage, edge cases, test quality |
| **Documentation** | Public API docs, module headers |
| **Maintainability** | Code organization, naming, complexity |

---

## Architectural Code Patterns

These patterns show how the key architectural components are used throughout the codebase.

### Engine Usage

```rust
// Initialization
Engine::new(project_path)           // Creates engine with full initialization

// Message processing
engine.process_message(msg)         // Process single message through TEA
engine.drain_pending_messages()     // Process all pending messages
engine.flush_pending_logs()         // Flush batched logs and sync SharedState

// Service accessors
engine.flutter_controller()         // Get controller for current session
engine.log_service()                // Get log buffer access
engine.state_service()              // Get app state access

// Event broadcasting
engine.subscribe()                  // Subscribe to EngineEvents

// Lifecycle
engine.shutdown().await             // Stop watcher, cleanup sessions
```

### TUI Runner Pattern

```rust
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    let mut engine = Engine::new(project_path.to_path_buf());
    let mut term = ratatui::init();

    // TUI-specific startup
    startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);

    // Main loop
    while !engine.should_quit() {
        engine.drain_pending_messages();
        engine.flush_pending_logs();
        term.draw(|frame| render::view(frame, &mut engine.state))?;
        if let Some(message) = event::poll()? {
            engine.process_message(message);
        }
    }

    engine.shutdown().await;
    ratatui::restore();
    Ok(())
}
```

### Headless Runner Pattern

```rust
pub async fn run_headless(project_path: &Path) -> Result<()> {
    let mut engine = Engine::new(project_path.to_path_buf());

    // Headless-specific stdin reader
    spawn_stdin_reader(engine.msg_sender());

    // Auto-start Flutter session
    headless_auto_start(&mut engine).await;

    // Main loop
    loop {
        if engine.should_quit() { break; }
        match engine.msg_rx.recv().await {
            Some(msg) => {
                engine.process_message(msg);
                engine.flush_pending_logs();
                emit_headless_events(&engine.state);
            }
            None => break,
        }
    }

    engine.shutdown().await;
    Ok(())
}
```

### EngineEvent Subscription

```rust
let mut rx = engine.subscribe();

tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        match event {
            EngineEvent::ReloadStarted { session_id } => {
                // Track reload start time
            }
            EngineEvent::ReloadCompleted { session_id, time_ms } => {
                // Report reload performance
            }
            EngineEvent::LogBatch { session_id, entries } => {
                // Forward logs to MCP server
            }
            _ => {}
        }
    }
});
```

### Key Type Definitions

**AppState (Model):**
```rust
pub struct AppState {
    // UI mode
    pub ui_mode: UiMode,  // Normal, DeviceSelector, Loading, etc.

    // Multi-session support
    pub session_manager: SessionManager,
    pub device_selector: DeviceSelectorState,

    // Configuration
    pub settings: Settings,
    pub project_path: PathBuf,
    pub project_name: Option<String>,

    // Legacy single-session (backward compat)
    pub phase: AppPhase,
    pub logs: Vec<LogEntry>,
    pub log_view_state: LogViewState,
    pub current_app_id: Option<String>,
    pub device_name: Option<String>,
    pub reload_count: u32,
    // ...
}
```

**Message (Events):**
```rust
pub enum Message {
    // Input
    Key(KeyEvent),
    Daemon(DaemonEvent),
    Tick,

    // Navigation
    ScrollUp, ScrollDown, PageUp, PageDown,

    // Control
    HotReload, HotRestart, StopApp,
    ReloadStarted, ReloadCompleted { time_ms: u64 }, ReloadFailed { reason: String },

    // File watcher
    FilesChanged { count: usize },
    AutoReloadTriggered,

    // Device/session management
    ShowDeviceSelector, HideDeviceSelector,
    DeviceSelected { device: Device },
    SelectSessionByIndex(usize),
    NextSession, PreviousSession,
    CloseCurrentSession,

    // Lifecycle
    Quit,
}
```

**UpdateResult (Update Output):**
```rust
pub struct UpdateResult {
    pub message: Option<Message>,  // Follow-up message
    pub action: Option<UpdateAction>,  // Side effect for event loop
}

pub enum UpdateAction {
    SpawnTask(Task),
    DiscoverDevices,
    DiscoverEmulators,
    LaunchEmulator { emulator_id: String },
    SpawnSession { device: Device, config: Option<Box<LaunchConfig>> },
}
```