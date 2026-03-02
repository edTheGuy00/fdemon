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

---

## Responsive Layout Guidelines

These guidelines apply to all widgets that render inside a `Rect` allocated by the layout system. They codify the patterns established during the responsive session-dialog work and are generalized for use across the entire codebase.

### Principle 1: Decide layout variant based on available space, not orientation

**Statement**: Choose compact vs. expanded rendering by measuring the actual pixel (character cell) dimensions of the allocated `Rect`, not by inspecting which layout orientation was used.

**Rationale**: Orientation (horizontal vs. vertical) tells you the _direction_ content flows, not how much room exists in each dimension. A horizontally-arranged widget can have very little vertical space; a vertically-arranged widget can be tall. Tying compactness to orientation will render incorrectly whenever orientation and available space diverge.

```rust
// Anti-pattern: compact mode tied to layout orientation
fn render_vertical(&self, area: Rect, buf: &mut Buffer) {
    let widget = MyWidget::new().compact(true); // always compact in vertical
}
fn render_horizontal(&self, area: Rect, buf: &mut Buffer) {
    let widget = MyWidget::new().compact(false); // always expanded in horizontal
}

// Correct: compact mode tied to actual available space
fn render(&self, area: Rect, buf: &mut Buffer) {
    let compact = area.height < MIN_EXPANDED_HEIGHT;
    let widget = MyWidget::new().compact(compact);
}
```

### Principle 2: All content must fit within the allocated area

**Statement**: Every element rendered by a widget must fall within the `Rect` passed to its `render()` method. No coordinate should be computed by adding offsets to the last rendered element's position.

**Rationale**: Manual position arithmetic can produce `y` or `x` values that exceed the bounds of the allocated area, causing visual corruption or out-of-bounds panics in the terminal backend. The `Layout` system guarantees all returned chunks are within the parent `Rect` and clips gracefully when space is tight.

```rust
// Anti-pattern: manual position outside layout system
let button_y = last_field.y + last_field.height + 1;
let button_area = Rect { y: button_y, height: 3, ..area }; // can overflow!

// Correct: include every element in the layout system
let chunks = Layout::vertical([
    Constraint::Length(4), // field
    Constraint::Length(1), // spacer
    Constraint::Length(3), // button — position managed by layout
    Constraint::Min(0),    // absorber — clips silently if space runs out
])
.split(area);
let button_area = chunks[2]; // always within bounds
```

Include all visible elements — including buttons, footers, and spacers — as explicit `Layout` constraints. Use `Constraint::Min(0)` as the final slot to absorb any remaining space or gracefully discard overflowing elements.

### Principle 3: Scrollable lists must keep the selected item visible

**Statement**: Never use a hardcoded constant as the visible-height estimate when adjusting scroll offsets. Feed the real render-time height back to the handler layer via a `Cell<usize>` render-hint, and add a render-time clamp as a safety net.

**Rationale**: Hardcoded viewport height estimates are fragile — the real height varies with terminal size, layout mode, and surrounding content. A hardcoded estimate of `10` fails when the real height is `4` (small panel) or `30` (full terminal), causing the selected item to scroll off-screen.

```rust
// Anti-pattern: scroll adjustment with a hardcoded height estimate
state.list.adjust_scroll(DEFAULT_ESTIMATED_VISIBLE_HEIGHT); // fragile!

// Correct: Cell<usize> render-hint feedback

// State definition
pub struct ListState {
    pub selected_index: usize,
    pub scroll_offset: usize,
    /// Render-hint: actual visible height from the last rendered frame.
    /// Defaults to 0, which signals "not yet rendered — use fallback".
    pub last_known_visible_height: Cell<usize>,
}

// Renderer: write actual height each frame
fn render(&self, area: Rect, buf: &mut Buffer) {
    let visible_height = area.height as usize;
    // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
    self.state.last_known_visible_height.set(visible_height);

    // Safety net: clamp scroll so the selected item is visible this frame
    let corrected_scroll = calculate_scroll_offset(
        self.state.selected_index,
        visible_height,
        self.state.scroll_offset,
    );
    // Use corrected_scroll for rendering only — do not mutate state here
}

// Handler: read actual height with fallback
fn handle_scroll(state: &mut AppState) {
    let height = state.list.last_known_visible_height.get();
    let effective = if height > 0 { height } else { DEFAULT_HEIGHT };
    state.list.adjust_scroll(effective);
}
```

**TEA exception note**: Using `Cell<usize>` for a render-hint is a pragmatic exception to strict unidirectional data flow. It scopes the mutation to a single numeric hint value — not business logic — and avoids threading render geometry through the message bus. Annotate every call site with:

```rust
// EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
```

### Principle 4: Use named constants for layout thresholds

**Statement**: Every numeric threshold used in layout decisions must be a named constant with a doc comment explaining how the value was derived. Magic numbers are forbidden in layout code.

**Rationale**: Magic numbers in layout code carry no context about why they were chosen. When a widget is resized or its content changes, scattered literals must each be updated individually and the derivation must be re-understood. Named constants with derivation comments make threshold changes safe and self-documenting.

```rust
// Anti-pattern: magic numbers
fn render(&self, area: Rect, buf: &mut Buffer) {
    let compact = area.height < 29; // where did 29 come from?
    let chunks = Layout::vertical([
        // ...
        Constraint::Length(3), // why 3?
    ]);
}

// Correct: named constants with derivation comments
/// Minimum content-area height required for expanded rendering.
/// Derived from: 5 fields × 4 rows + 5 spacers + 1 button spacer + 3 button rows = 29.
const MIN_EXPANDED_HEIGHT: u16 = 29;

/// Height of the action button slot in the fields layout.
const BUTTON_HEIGHT: u16 = 3;

fn render(&self, area: Rect, buf: &mut Buffer) {
    let compact = area.height < MIN_EXPANDED_HEIGHT;
    let chunks = Layout::vertical([
        // ...
        Constraint::Length(BUTTON_HEIGHT),
        Constraint::Min(0),
    ]);
}
```

Group related thresholds near the widget module they control, not scattered across files.

### Principle 5: Use deterministic single-threshold layout decisions

**Statement**: When a widget switches between two display modes based on available space, use a single named threshold constant. The decision should be deterministic and stateless — the same dimensions always produce the same layout.

**Rationale**: A single threshold is simple, predictable, and requires no state tracking. Terminal resize is infrequent (especially while modal dialogs are open), and Ratatui's fast redraw cycle means any transient mode change during resize stabilizes within one frame. The complexity of stateful hysteresis (tracking previous mode, handling stale state on reopen) is not justified for TUI layout switching.

```rust
/// Minimum content-area height for expanded mode.
/// Expanded needs 5 fields x 4 rows + spacers + button = 29 rows.
const MIN_EXPANDED_HEIGHT: u16 = 29;

let compact = area.height < MIN_EXPANDED_HEIGHT;
```

### Anti-Pattern Summary

| Anti-Pattern | Why It's Wrong | Correct Approach |
|---|---|---|
| `compact(orientation == Vertical)` | Orientation does not indicate available space | Check `area.height < MIN_EXPANDED_HEIGHT` |
| Manual `Rect` computed outside the layout system | Can produce coordinates that overflow parent bounds | Include all elements in `Layout` with a `Constraint::Min(0)` absorber |
| `adjust_scroll(HARDCODED_HEIGHT)` | Real viewport height varies with terminal size and layout | Feed render-time height back via `Cell<usize>`; add render-time scroll clamp |
| Magic numbers for size thresholds | No rationale, maintenance burden when content changes | Named constants with doc comments explaining derivation |
| Stateful layout mode tracking | Adds complexity for negligible benefit in fast-redraw TUI | Single deterministic threshold; same dimensions always produce the same layout |