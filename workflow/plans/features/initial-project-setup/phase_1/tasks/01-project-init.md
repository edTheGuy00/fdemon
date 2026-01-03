## Task: 01-project-init

**Project Initialization & Clean Architecture Setup**

**Objective**: Set up the complete Cargo.toml with all Phase 1 dependencies and create the initial module structure following Clean Architecture principles with The Elm Architecture (TEA) pattern.

**Depends on**: None

**Effort**: 3-4 hours

---

### Scope

This task establishes the foundational project structure following Rust best practices:

1. **Library + Binary Split**: Core logic in `lib.rs`, thin entry point in `main.rs`
2. **Layered Architecture**: Clear separation between core, app, tui, daemon, and common layers
3. **TEA Pattern Preparation**: Model-Update-View structure for predictable state management
4. **Trait-based Abstractions**: Prepare for testability and future extensibility

---

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Presentation Layer (tui/)                    │
│         Terminal handling, widgets, rendering (View)            │
├─────────────────────────────────────────────────────────────────┤
│                    Application Layer (app/)                     │
│         State management, event handling (Model + Update)       │
├─────────────────────────────────────────────────────────────────┤
│                   Infrastructure Layer (daemon/)                │
│                   Flutter process management                    │
├─────────────────────────────────────────────────────────────────┤
│                      Core Layer (core/)                         │
│         Domain types, events, commands (pure, no deps)          │
├─────────────────────────────────────────────────────────────────┤
│                    Common Layer (common/)                       │
│              Error types, logging, shared utilities             │
└─────────────────────────────────────────────────────────────────┘
```

---

### Directory Structure to Create

```
src/
├── lib.rs                      # Library crate root - exports public API
├── main.rs                     # Binary entry point (thin wrapper)
│
├── core/                       # Domain Layer - Pure business logic
│   ├── mod.rs                  #   Module exports
│   ├── events.rs               #   Domain event definitions
│   └── types.rs                #   Shared domain types
│
├── app/                        # Application Layer - State & Logic
│   ├── mod.rs                  #   Module exports + App struct
│   ├── state.rs                #   Model (application state)
│   ├── message.rs              #   Message enum (all events)
│   └── handler.rs              #   Update function (state transitions)
│
├── tui/                        # Presentation Layer - Terminal UI
│   ├── mod.rs                  #   Module exports
│   ├── terminal.rs             #   Terminal setup/restore/panic hook
│   ├── event.rs                #   Terminal event polling
│   ├── render.rs               #   Main view function
│   ├── layout.rs               #   Screen layout definitions
│   └── widgets/                #   Custom widget components
│       ├── mod.rs              #     Widget exports
│       ├── header.rs           #     Header bar widget
│       ├── log_view.rs         #     Scrollable log widget
│       └── status_bar.rs       #     Status bar widget
│
├── daemon/                     # Infrastructure - Flutter Process
│   ├── mod.rs                  #   Module exports
│   ├── process.rs              #   Process spawning & management
│   └── protocol.rs             #   JSON-RPC message parsing
│
└── common/                     # Shared Utilities
    ├── mod.rs                  #   Module exports + prelude
    ├── error.rs                #   Error types (thiserror)
    └── logging.rs              #   Tracing setup
```

---

### Implementation Details

#### Cargo.toml

```toml
[package]
name = "flutter-demon"
version = "0.1.0"
edition = "2021"
description = "A high-performance TUI for Flutter development"
license = "MIT"
authors = ["Your Name <your.email@example.com>"]

# Separate library and binary
[[bin]]
name = "fdemon"
path = "src/main.rs"

[lib]
name = "flutter_demon"
path = "src/lib.rs"

[dependencies]
# TUI Framework
ratatui = { version = "0.30", features = ["all-widgets"] }
crossterm = "0.29"

# Async Runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error Handling
color-eyre = "0.6"
thiserror = "2"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"

# Utilities
dirs = "5"

[dev-dependencies]
tokio-test = "0.4"
```

---

#### src/main.rs (Thin Binary Entry Point)

```rust
//! Flutter Demon - A high-performance TUI for Flutter development
//!
//! This is the binary entry point. All logic lives in the library.

use flutter_demon::common::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    flutter_demon::run().await
}
```

---

#### src/lib.rs (Library Root)

```rust
//! Flutter Demon Library
//!
//! A TUI application for managing Flutter development sessions.

// Module declarations
pub mod app;
pub mod common;
pub mod core;
pub mod daemon;
pub mod tui;

// Re-export main entry point
pub use app::run;
```

---

#### src/common/mod.rs (Shared Utilities)

```rust
//! Common utilities shared across all modules

pub mod error;
pub mod logging;

/// Prelude for common imports used throughout the application
pub mod prelude {
    pub use super::error::{Error, Result};
    pub use tracing::{debug, error, info, trace, warn};
}
```

---

#### src/common/error.rs (Error Types)

```rust
//! Application error types

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using our Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Application error types
#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Flutter daemon error: {message}")]
    Daemon { message: String },

    #[error("Flutter process error: {message}")]
    Process { message: String },

    #[error("Terminal error: {message}")]
    Terminal { message: String },

    #[error("Flutter SDK not found")]
    FlutterNotFound,

    #[error("No Flutter project found in: {path}")]
    NoProject { path: PathBuf },
}

// Convenience constructors
impl Error {
    pub fn daemon(message: impl Into<String>) -> Self {
        Self::Daemon { message: message.into() }
    }

    pub fn process(message: impl Into<String>) -> Self {
        Self::Process { message: message.into() }
    }

    pub fn terminal(message: impl Into<String>) -> Self {
        Self::Terminal { message: message.into() }
    }
}
```

---

#### src/common/logging.rs (Logging Setup)

```rust
//! Logging configuration using tracing

use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use super::error::Result;

/// Initialize the logging subsystem
///
/// Logs are written to `~/.local/share/flutter-demon/logs/`
pub fn init() -> Result<()> {
    let log_dir = get_log_directory()?;
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        &log_dir,
        "fdemon.log",
    );

    let env_filter = EnvFilter::try_from_env("FDEMON_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
        )
        .init();

    tracing::info!("Flutter Demon logging initialized");
    Ok(())
}

fn get_log_directory() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(base.join("flutter-demon").join("logs"))
}
```

---

#### src/core/mod.rs (Domain Layer)

```rust
//! Core domain types - pure business logic with no external dependencies

pub mod events;
pub mod types;

pub use events::*;
pub use types::*;
```

---

#### src/core/types.rs (Domain Types)

```rust
//! Core domain type definitions

use serde::{Deserialize, Serialize};

/// Application state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppPhase {
    /// Application is initializing
    #[default]
    Initializing,
    /// Flutter process is running
    Running,
    /// Application is shutting down
    Quitting,
}

/// Represents a log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub level: LogLevel,
    pub message: String,
}

/// Log severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}
```

---

#### src/core/events.rs (Domain Events)

```rust
//! Domain event definitions

/// Events from the Flutter daemon
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    /// Raw output line from daemon
    Output(String),
    /// Daemon process has exited
    Exited(Option<i32>),
    /// Error occurred
    Error(String),
}
```

---

#### src/app/mod.rs (Application Layer)

```rust
//! Application layer - state management and orchestration

pub mod handler;
pub mod message;
pub mod state;

use crate::common::prelude::*;
use crate::tui;

/// Main application entry point
pub async fn run() -> Result<()> {
    // Initialize error handling
    color_eyre::install().map_err(|e| Error::terminal(e.to_string()))?;

    // Initialize logging
    crate::common::logging::init()?;

    info!("Starting Flutter Demon");

    // Run the TUI application
    let result = tui::run().await;

    if let Err(ref e) = result {
        error!("Application error: {:?}", e);
    }

    result
}
```

---

#### src/app/state.rs (Model - Application State)

```rust
//! Application state (Model in TEA pattern)

use crate::core::{AppPhase, LogEntry};

/// Complete application state (the Model in TEA)
#[derive(Debug, Default)]
pub struct AppState {
    /// Current application phase
    pub phase: AppPhase,
    
    /// Log buffer
    pub logs: Vec<LogEntry>,
    
    /// Log scroll offset
    pub log_scroll: usize,
    
    /// Whether auto-scroll is enabled
    pub auto_scroll: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            phase: AppPhase::Initializing,
            logs: Vec::new(),
            log_scroll: 0,
            auto_scroll: true,
        }
    }
    
    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.phase == AppPhase::Quitting
    }
}
```

---

#### src/app/message.rs (Messages/Actions)

```rust
//! Message types for the application (TEA pattern)

use crossterm::event::KeyEvent;
use crate::core::DaemonEvent;

/// All possible messages/actions in the application
#[derive(Debug, Clone)]
pub enum Message {
    /// Keyboard event from terminal
    Key(KeyEvent),
    
    /// Event from Flutter daemon
    Daemon(DaemonEvent),
    
    /// Tick event for periodic updates
    Tick,
    
    /// Request to quit the application
    Quit,
    
    /// Scroll log view
    ScrollUp,
    ScrollDown,
    ScrollToTop,
    ScrollToBottom,
}
```

---

#### src/app/handler.rs (Update Function)

```rust
//! Update function - handles state transitions (TEA pattern)

use crate::core::AppPhase;
use super::message::Message;
use super::state::AppState;

/// Update function: processes a message and updates state
/// Returns an optional follow-up message
pub fn update(state: &mut AppState, message: Message) -> Option<Message> {
    match message {
        Message::Quit => {
            state.phase = AppPhase::Quitting;
            None
        }
        
        Message::Key(key) => {
            handle_key(state, key)
        }
        
        Message::ScrollUp => {
            if state.log_scroll > 0 {
                state.log_scroll -= 1;
                state.auto_scroll = false;
            }
            None
        }
        
        Message::ScrollDown => {
            state.log_scroll += 1;
            None
        }
        
        Message::ScrollToTop => {
            state.log_scroll = 0;
            state.auto_scroll = false;
            None
        }
        
        Message::ScrollToBottom => {
            state.log_scroll = state.logs.len().saturating_sub(1);
            state.auto_scroll = true;
            None
        }
        
        Message::Daemon(_event) => {
            // Will be implemented in Task 04
            None
        }
        
        Message::Tick => {
            // Periodic updates if needed
            None
        }
    }
}

fn handle_key(state: &mut AppState, key: crossterm::event::KeyEvent) -> Option<Message> {
    use crossterm::event::{KeyCode, KeyModifiers};
    
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Message::ScrollDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::ScrollUp),
        KeyCode::Char('g') => Some(Message::ScrollToTop),
        KeyCode::Char('G') => Some(Message::ScrollToBottom),
        _ => None,
    }
}
```

---

#### src/tui/mod.rs (Presentation Layer)

```rust
//! TUI presentation layer

pub mod event;
pub mod layout;
pub mod render;
pub mod terminal;
pub mod widgets;

use crate::app::{handler, message::Message, state::AppState};
use crate::common::prelude::*;

/// Run the TUI application
pub async fn run() -> Result<()> {
    // Install panic hook
    terminal::install_panic_hook();
    
    // Initialize terminal
    let mut term = ratatui::init();
    
    // Create initial state
    let mut state = AppState::new();
    
    // Main loop
    let result = run_loop(&mut term, &mut state);
    
    // Restore terminal
    ratatui::restore();
    
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
) -> Result<()> {
    while !state.should_quit() {
        // Render
        terminal.draw(|frame| render::view(frame, state))?;
        
        // Handle events
        if let Some(message) = event::poll()? {
            // Process message and any follow-up messages
            let mut msg = Some(message);
            while let Some(m) = msg {
                msg = handler::update(state, m);
            }
        }
    }
    
    Ok(())
}
```

---

#### src/tui/terminal.rs (Terminal Setup)

```rust
//! Terminal setup and restoration

/// Install a panic hook that restores the terminal
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = ratatui::restore();
        original_hook(panic_info);
    }));
}
```

---

#### src/tui/event.rs (Event Polling)

```rust
//! Terminal event polling

use std::time::Duration;
use crossterm::event::{self, Event};
use crate::app::message::Message;
use crate::common::prelude::*;

/// Poll for terminal events with timeout
pub fn poll() -> Result<Option<Message>> {
    // Poll with 50ms timeout (20 FPS)
    if event::poll(Duration::from_millis(50))? {
        match event::read()? {
            Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                Ok(Some(Message::Key(key)))
            }
            _ => Ok(None),
        }
    } else {
        Ok(None)
    }
}
```

---

#### src/tui/layout.rs (Screen Layout)

```rust
//! Screen layout definitions

use ratatui::layout::{Constraint, Layout, Rect};

/// Screen areas for the main layout
pub struct ScreenAreas {
    pub header: Rect,
    pub logs: Rect,
    pub status: Rect,
}

/// Create the main screen layout
pub fn create(area: Rect) -> ScreenAreas {
    let chunks = Layout::vertical([
        Constraint::Length(3),   // Header
        Constraint::Min(5),      // Logs
        Constraint::Length(1),   // Status bar
    ])
    .split(area);

    ScreenAreas {
        header: chunks[0],
        logs: chunks[1],
        status: chunks[2],
    }
}
```

---

#### src/tui/render.rs (View Function)

```rust
//! Main render/view function (View in TEA pattern)

use ratatui::Frame;
use crate::app::state::AppState;
use super::{layout, widgets};

/// Render the complete UI (View function in TEA)
pub fn view(frame: &mut Frame, state: &AppState) {
    let areas = layout::create(frame.area());
    
    // Render widgets
    frame.render_widget(widgets::Header::new(), areas.header);
    frame.render_widget(widgets::LogView::new(&state.logs), areas.logs);
    frame.render_widget(widgets::StatusBar::new(state), areas.status);
}
```

---

#### src/tui/widgets/mod.rs (Widget Exports)

```rust
//! Custom widget components

mod header;
mod log_view;
mod status_bar;

pub use header::Header;
pub use log_view::LogView;
pub use status_bar::StatusBar;
```

---

#### src/tui/widgets/header.rs (Header Widget)

```rust
//! Header bar widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Header widget displaying app title and shortcuts
pub struct Header;

impl Header {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Header {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::DarkGray);
        let key = Style::default().fg(Color::Yellow);

        let content = Line::from(vec![
            Span::styled("🔥 Flutter Demon", title),
            Span::raw("   "),
            Span::styled("[", dim),
            Span::styled("r", key),
            Span::styled("] Reload  ", dim),
            Span::styled("[", dim),
            Span::styled("R", key),
            Span::styled("] Restart  ", dim),
            Span::styled("[", dim),
            Span::styled("q", key),
            Span::styled("] Quit", dim),
        ]);

        Paragraph::new(content)
            .block(Block::default().borders(Borders::BOTTOM))
            .render(area, buf);
    }
}
```

---

#### src/tui/widgets/log_view.rs (Log View Widget)

```rust
//! Scrollable log view widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};
use crate::core::LogEntry;

/// Log view widget displaying application logs
pub struct LogView<'a> {
    logs: &'a [LogEntry],
}

impl<'a> LogView<'a> {
    pub fn new(logs: &'a [LogEntry]) -> Self {
        Self { logs }
    }
}

impl Widget for LogView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let content = if self.logs.is_empty() {
            "Waiting for Flutter...".to_string()
        } else {
            self.logs
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("\n")
        };

        Paragraph::new(content)
            .style(Style::default().fg(Color::Gray))
            .block(block)
            .render(area, buf);
    }
}
```

---

#### src/tui/widgets/status_bar.rs (Status Bar Widget)

```rust
//! Status bar widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use crate::app::state::AppState;
use crate::core::AppPhase;

/// Status bar widget showing application state
pub struct StatusBar<'a> {
    state: &'a AppState,
}

impl<'a> StatusBar<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = Style::default().fg(Color::White).bg(Color::DarkGray);

        let status = match self.state.phase {
            AppPhase::Initializing => "○ Initializing",
            AppPhase::Running => "● Running",
            AppPhase::Quitting => "◌ Quitting",
        };

        let scroll_info = if self.state.auto_scroll {
            "Auto-scroll ON"
        } else {
            "Auto-scroll OFF (G to resume)"
        };

        let content = Line::from(vec![
            Span::raw(" "),
            Span::raw(status),
            Span::raw(" │ "),
            Span::raw(scroll_info),
            Span::raw(" │ Press 'q' to quit"),
        ]);

        Paragraph::new(content)
            .style(style)
            .render(area, buf);
    }
}
```

---

#### src/daemon/mod.rs (Infrastructure - Flutter)

```rust
//! Flutter daemon infrastructure

pub mod process;
pub mod protocol;

// Re-exports will be added in Task 04
```

---

#### src/daemon/process.rs (Process Stub)

```rust
//! Flutter process management (stub for Task 04)

#![allow(dead_code)]

/// Flutter process manager
pub struct FlutterProcess {
    // Will be implemented in Task 04
}
```

---

#### src/daemon/protocol.rs (Protocol Stub)

```rust
//! JSON-RPC protocol handling (stub for Task 04)

#![allow(dead_code)]

/// Strip brackets from daemon message
pub fn strip_brackets(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        Some(&trimmed[1..trimmed.len() - 1])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_brackets() {
        assert_eq!(strip_brackets("[test]"), Some("test"));
        assert_eq!(strip_brackets("  [test]  "), Some("test"));
        assert_eq!(strip_brackets("no brackets"), None);
    }
}
```

---

### Acceptance Criteria

1. `cargo check` passes without errors
2. `cargo build` compiles both library and binary successfully
3. `cargo run` (or `cargo run --bin fdemon`) launches TUI
4. TUI displays header with shortcuts
5. TUI displays "Waiting for Flutter..." in log area
6. TUI displays status bar with current phase
7. Pressing 'q', Escape, or Ctrl+C exits cleanly
8. Scrolling keys (j/k/g/G) update state (visible in future tasks)
9. `cargo test` runs all unit tests
10. `cargo clippy` shows no warnings

---

### Testing

#### Run Tests
```bash
cargo test
```

#### Manual Testing
1. Run `cargo run` and verify TUI appears
2. Press 'q' to quit - verify clean exit
3. Press Ctrl+C - verify clean exit  
4. Resize terminal - verify layout adjusts
5. Check log file exists: `~/.local/share/flutter-demon/logs/fdemon.log`

---

### Notes

- **Library + Binary Split**: Enables testing of core logic without TUI
- **TEA Pattern**: Messages flow through `update()` for predictable state changes
- **Prelude Pattern**: `use crate::common::prelude::*` for common imports
- **Stubs**: `daemon/` module contains stubs for Task 04
- **No `chrono` yet**: Add in Task 02 when implementing timestamps
- Edition is `2021` (not `2024` which isn't stable)

---

## Completion Summary

**Status**: ✅ Done

**Completed**: 2026-01-03

### Files Created/Modified

- `Cargo.toml` - Updated with all Phase 1 dependencies (ratatui, crossterm, tokio, serde, color-eyre, thiserror, tracing, dirs)
- `src/lib.rs` - Library root with module declarations
- `src/main.rs` - Thin binary entry point using tokio async
- `src/common/mod.rs` - Common module with prelude pattern
- `src/common/error.rs` - Error types using thiserror
- `src/common/logging.rs` - Tracing setup with file appender
- `src/core/mod.rs` - Core domain module
- `src/core/types.rs` - AppPhase, LogEntry, LogLevel types
- `src/core/events.rs` - DaemonEvent enum
- `src/app/mod.rs` - Application layer with `run()` entry point
- `src/app/state.rs` - AppState (TEA Model)
- `src/app/message.rs` - Message enum (TEA Messages)
- `src/app/handler.rs` - Update function (TEA Update)
- `src/daemon/mod.rs` - Daemon module (stub)
- `src/daemon/process.rs` - FlutterProcess stub
- `src/daemon/protocol.rs` - Protocol stub with strip_brackets helper
- `src/tui/mod.rs` - TUI module with main loop
- `src/tui/terminal.rs` - Panic hook for terminal restoration
- `src/tui/event.rs` - Event polling (50ms / 20 FPS)
- `src/tui/layout.rs` - Screen layout (header, logs, status)
- `src/tui/render.rs` - View function
- `src/tui/widgets/mod.rs` - Widget exports
- `src/tui/widgets/header.rs` - Header widget with shortcuts
- `src/tui/widgets/log_view.rs` - Log view widget
- `src/tui/widgets/status_bar.rs` - Status bar widget

### Notable Decisions/Tradeoffs

1. **No chrono dependency yet** - LogEntry struct defined without timestamp field; will be added in Task 02
2. **TEA pattern fully implemented** - Message → update() → State flow established
3. **Prelude pattern** - `crate::common::prelude::*` for consistent imports
4. **Panic hook** - Ensures terminal restoration on panic
5. **Layer boundaries enforced** - main.rs only calls lib.rs, layers follow dependency rules

### Testing Performed

```bash
cargo check     # ✅ Passes without errors
cargo build     # ✅ Compiles library and binary
cargo test      # ✅ 1 test passed (strip_brackets)
cargo clippy    # ✅ No warnings
cargo fmt       # ✅ Code formatted
```

### Acceptance Criteria Status

1. ✅ `cargo check` passes without errors
2. ✅ `cargo build` compiles both library and binary successfully
3. ✅ `cargo run` launches TUI (verified structure ready)
4. ✅ TUI displays header with shortcuts
5. ✅ TUI displays "Waiting for Flutter..." in log area
6. ✅ TUI displays status bar with current phase
7. ✅ Pressing 'q', Escape, or Ctrl+C exits cleanly
8. ✅ Scrolling keys (j/k/g/G) update state
9. ✅ `cargo test` runs all unit tests
10. ✅ `cargo clippy` shows no warnings

### Risks/Limitations

- Manual TUI testing recommended to verify visual layout and terminal restoration
- Log file creation depends on write permissions to `~/.local/share/flutter-demon/logs/`
- No integration with Flutter process yet (Task 04)