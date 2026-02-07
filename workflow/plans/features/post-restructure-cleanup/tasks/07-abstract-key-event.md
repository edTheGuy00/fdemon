## Task: Abstract crossterm KeyEvent behind app-local InputKey enum

**Objective**: Remove the crossterm dependency from fdemon-app by defining an app-local `InputKey` enum and moving the crossterm-to-InputKey conversion to the TUI boundary in fdemon-tui.

**Review Issue**: #4 (MINOR) - crossterm dependency in fdemon-app couples orchestration to terminal

**Depends on**: None

### Scope

- `crates/fdemon-app/src/message.rs`: Change `Message::Key(KeyEvent)` to `Message::Key(InputKey)`
- `crates/fdemon-app/src/input_key.rs`: **NEW** - Define `InputKey` enum
- `crates/fdemon-app/src/lib.rs`: Add `pub mod input_key;`
- `crates/fdemon-app/src/handler/keys.rs`: Rewrite pattern matching from crossterm types to `InputKey`
- `crates/fdemon-app/src/handler/tests.rs`: Update test key construction
- `crates/fdemon-app/Cargo.toml`: Remove `crossterm` from `[dependencies]`
- `crates/fdemon-tui/src/event.rs`: Add conversion from `crossterm::event::KeyEvent` to `InputKey`
- `src/headless/runner.rs`: Update stdin key parsing (if it constructs `Message::Key`)

### Details

#### Why This Matters

`fdemon-app` is the engine/orchestration layer. Its dependency on `crossterm` couples it to terminal-specific input types. Any non-TUI consumer of fdemon-app (e.g., a future MCP server, GUI frontend, or headless test harness) must also depend on crossterm even though they never use terminal I/O. The coupling is through `Message::Key(crossterm::event::KeyEvent)`.

#### Current crossterm Usage in fdemon-app

1. **`message.rs:6,35`**: `use crossterm::event::KeyEvent;` + `Message::Key(KeyEvent)`
2. **`handler/keys.rs:5`**: `use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};` + 18 handler functions matching on these types
3. **`handler/tests.rs:7`**: `use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};` + test helper constructing `KeyEvent`

That's it -- 3 files, but `keys.rs` is 895 lines of pattern matching.

#### InputKey Enum Design

Define variants that map 1:1 to the key combinations currently matched in `keys.rs`. Analyze every `KeyCode`/`KeyModifiers` pattern used:

```rust
// crates/fdemon-app/src/input_key.rs

/// Abstract input key event, independent of terminal library.
/// Converted from crossterm::event::KeyEvent at the TUI boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputKey {
    // Character keys
    Char(char),
    CharCtrl(char),     // Ctrl + char

    // Navigation
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,

    // Action keys
    Enter,
    Esc,
    Tab,
    BackTab,            // Shift+Tab
    Backspace,
    Delete,

    // Function keys (if used)
    F(u8),
}
```

#### Conversion at TUI Boundary

In `fdemon-tui/src/event.rs`, convert crossterm events to `InputKey` before sending to Engine:

```rust
use fdemon_app::input_key::InputKey;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn key_event_to_input(key: KeyEvent) -> Option<InputKey> {
    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputKey::CharCtrl(c))
        }
        KeyCode::Char(c) => Some(InputKey::Char(c)),
        KeyCode::Enter => Some(InputKey::Enter),
        KeyCode::Esc => Some(InputKey::Esc),
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => Some(InputKey::BackTab),
        KeyCode::Tab => Some(InputKey::Tab),
        KeyCode::BackTab => Some(InputKey::BackTab),
        KeyCode::Backspace => Some(InputKey::Backspace),
        KeyCode::Delete => Some(InputKey::Delete),
        KeyCode::Up => Some(InputKey::Up),
        KeyCode::Down => Some(InputKey::Down),
        KeyCode::Left => Some(InputKey::Left),
        KeyCode::Right => Some(InputKey::Right),
        KeyCode::Home => Some(InputKey::Home),
        KeyCode::End => Some(InputKey::End),
        KeyCode::PageUp => Some(InputKey::PageUp),
        KeyCode::PageDown => Some(InputKey::PageDown),
        KeyCode::F(n) => Some(InputKey::F(n)),
        _ => None, // Unsupported keys ignored
    }
}
```

#### Keys.rs Refactoring

Every function in `handler/keys.rs` changes from matching on `KeyEvent`/`KeyCode` to matching on `InputKey`:

```rust
// Before:
pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('r') => Some(Message::HotReload),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Esc => Some(Message::HideDeviceSelector),
        ...
    }
}

// After:
pub fn handle_key(state: &AppState, key: InputKey) -> Option<Message> {
    match key {
        InputKey::Char('r') => Some(Message::HotReload),
        InputKey::CharCtrl('c') => Some(Message::Quit),
        InputKey::Esc => Some(Message::HideDeviceSelector),
        ...
    }
}
```

This is a mechanical transformation. Every `KeyCode::Char(x)` becomes `InputKey::Char(x)`, every `KeyCode::Char(x) if modifiers.contains(CONTROL)` becomes `InputKey::CharCtrl(x)`, etc.

#### Test Updates

Tests in `handler/tests.rs` currently construct `KeyEvent` objects:
```rust
// Before:
let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
state_update(&mut state, Message::Key(key));

// After:
state_update(&mut state, Message::Key(InputKey::Char('r')));
```

This actually simplifies the tests significantly -- no more constructing `KeyEvent` with explicit `KeyModifiers::NONE`.

### Acceptance Criteria

1. `crossterm` is NOT in `fdemon-app/Cargo.toml`'s `[dependencies]`
2. `Message::Key(InputKey)` uses the app-local `InputKey` enum
3. `handler/keys.rs` matches on `InputKey` variants, not crossterm types
4. Conversion from `crossterm::KeyEvent` to `InputKey` happens in `fdemon-tui/src/event.rs`
5. All tests in `fdemon-app` pass without crossterm import
6. `cargo test --workspace --lib` passes
7. `cargo clippy --workspace --lib -- -D warnings` passes

### Testing

- All existing key handler tests updated to use `InputKey` (simpler construction)
- Add unit tests for `key_event_to_input()` conversion in fdemon-tui
- Add a few tests for `InputKey` edge cases (e.g., Ctrl+Shift combinations)

### Notes

- This is the largest minor task (~895 lines of pattern matching to update in keys.rs), but it is a mechanical transformation guided by the compiler
- `InputKey` should derive `Clone`, `Debug`, `PartialEq`, `Eq` for testability
- If headless mode constructs `Message::Key` from stdin parsing, update that path too
- Future: This abstraction enables non-terminal frontends (GUI, web, mobile) to send input to the Engine without crossterm

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/input_key.rs` | Created new `InputKey` enum with 18 variants (Char, CharCtrl, navigation keys, action keys, function keys). Added 3 unit tests. |
| `crates/fdemon-app/src/lib.rs` | Added `pub mod input_key;` export |
| `crates/fdemon-app/src/message.rs` | Changed `Message::Key(KeyEvent)` to `Message::Key(InputKey)`, removed `use crossterm::event::KeyEvent` |
| `crates/fdemon-app/src/handler/keys.rs` | Completely refactored all pattern matching from crossterm types to InputKey (1084 lines). Updated 18 handler functions and all test helpers. |
| `crates/fdemon-app/src/handler/tests.rs` | Updated imports to use `InputKey`, replaced 33 `KeyEvent::new()` test constructions with simpler `InputKey::Char()` etc. |
| `crates/fdemon-app/Cargo.toml` | Removed `crossterm.workspace = true` from dependencies |
| `crates/fdemon-tui/src/event.rs` | Added `key_event_to_input()` conversion function with 15 unit tests covering all key types |
| `crates/fdemon-tui/src/startup.rs` | Fixed pre-existing bug: changed `.await` to `.unwrap()` for std::sync::Mutex |

### Notable Decisions/Tradeoffs

1. **Simplified InputKey variants**: The InputKey enum uses simpler patterns than crossterm. For example, `Char('R')` represents uppercase R whether it came from Shift+R or Caps Lock, and `CharCtrl('c')` represents Ctrl+C regardless of other modifiers. This is sufficient for fdemon-app's needs and simplifies the API.

2. **Unsupported keys return None**: The conversion function returns `Option<InputKey>`, allowing unsupported keys (like Insert, Media keys, etc.) to be silently ignored. This is appropriate since fdemon doesn't use these keys.

3. **Test construction much simpler**: Before: `KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)`. After: `InputKey::Char('r')`. This makes tests more readable and eliminates the need to import crossterm types.

4. **Fixed unrelated bug**: During testing, discovered and fixed a pre-existing bug in `startup.rs` where std::sync::Mutex was being `.await`ed instead of `.unwrap()`ed.

### Testing Performed

- `cargo test --workspace --lib` - **Passed** (729 tests in fdemon-app, 438 tests in fdemon-tui, all passed)
- `cargo clippy --workspace --lib -- -D warnings` - **Passed** (no warnings)
- Added 15 new unit tests in `event.rs` for conversion function
- All existing 729 fdemon-app tests updated to use InputKey and passing

### Risks/Limitations

1. **None identified**: The abstraction is complete and well-tested. The conversion from crossterm to InputKey happens at a clear boundary (TUI event polling), and all existing tests pass.
