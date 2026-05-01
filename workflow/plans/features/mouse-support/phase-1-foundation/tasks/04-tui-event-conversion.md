## Task: Convert crossterm `MouseEvent` to `MouseInput` in `fdemon-tui`

**Objective**: Extend `crates/fdemon-tui/src/event.rs::poll()` to handle `Event::Mouse(_)` from crossterm, convert it to the abstract `MouseInput` (or drop it for `Moved` and unsupported variants), and emit `Message::Mouse(input)` onto the engine's message channel. Do not change behavior for keyboard or resize events.

**Depends on**: 01-input-mouse-type, 02-message-and-handler-shell

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/event.rs` — add `mouse_event_to_input(...)`, `key_modifiers_to_set(...)`, extend `poll()` to convert `Event::Mouse(_)`, add unit tests

**Files Read (Dependencies):**
- `crates/fdemon-app/src/input_mouse.rs` — Task 01's enum shapes
- `crates/fdemon-app/src/message.rs` — Task 02's `Message::Mouse` variant
- `crates/fdemon-tui/src/event.rs` — current keyboard conversion (`key_event_to_input`, `poll`) for placement / style

### Details

The conversion is one-to-one with three quirks:

1. **Drop `MouseEventKind::Moved`.** No consumer; high volume.
2. **Drop unknown / future `MouseEventKind` variants** via a fall-through arm that returns `None`.
3. **Map `ScrollLeft`/`ScrollRight`** even though they are touchpad-only and rare. Phase 2 routes them to no-ops; we collect them at the boundary so the `MouseInput::Scroll` enum stays comprehensive.

`x`/`y` come from `MouseEvent { column, row, .. }` — note the field names. crossterm uses `column`/`row`, our abstraction uses `x`/`y` (which match ratatui's `Rect`).

**File edits — top of `crates/fdemon-tui/src/event.rs`:**

Add to the existing imports:

```rust
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    MouseButton as CtMouseButton, MouseEvent, MouseEventKind,
};
use fdemon_app::message::Message;
use fdemon_app::{InputKey, KeyModSet, MouseButton, MouseInput, ScrollDir};
```

(The existing imports already include some of these — adjust the `use` statement so it covers everything cleanly. Avoid double-imports of `KeyModifiers`.)

#### `key_modifiers_to_set` helper

```rust
/// Convert crossterm `KeyModifiers` (a bitflag) to our abstract `KeyModSet`.
///
/// Only Shift / Ctrl / Alt are propagated; other modifiers (Hyper, Meta,
/// Super) are dropped. Mouse handlers in Phase 2+ only consult Shift/Ctrl/Alt.
pub(crate) fn key_modifiers_to_set(m: KeyModifiers) -> KeyModSet {
    KeyModSet {
        shift: m.contains(KeyModifiers::SHIFT),
        ctrl: m.contains(KeyModifiers::CONTROL),
        alt: m.contains(KeyModifiers::ALT),
    }
}
```

#### `mouse_event_to_input` converter

```rust
/// Convert crossterm `MouseEvent` to abstract [`MouseInput`].
///
/// Returns `None` for `Moved` events (no consumer; high volume) and any
/// future `MouseEventKind` variants we have not explicitly mapped.
pub(crate) fn mouse_event_to_input(ev: MouseEvent) -> Option<MouseInput> {
    let modifiers = key_modifiers_to_set(ev.modifiers);
    let x = ev.column;
    let y = ev.row;

    match ev.kind {
        MouseEventKind::Down(button) => Some(MouseInput::Click {
            x, y,
            button: ct_button_to_abstract(button),
            modifiers,
        }),
        MouseEventKind::Up(button) => Some(MouseInput::Release {
            x, y,
            button: ct_button_to_abstract(button),
            modifiers,
        }),
        MouseEventKind::Drag(button) => Some(MouseInput::Drag {
            x, y,
            button: ct_button_to_abstract(button),
            modifiers,
        }),
        MouseEventKind::ScrollUp => Some(MouseInput::Scroll {
            x, y, direction: ScrollDir::Up, modifiers,
        }),
        MouseEventKind::ScrollDown => Some(MouseInput::Scroll {
            x, y, direction: ScrollDir::Down, modifiers,
        }),
        MouseEventKind::ScrollLeft => Some(MouseInput::Scroll {
            x, y, direction: ScrollDir::Left, modifiers,
        }),
        MouseEventKind::ScrollRight => Some(MouseInput::Scroll {
            x, y, direction: ScrollDir::Right, modifiers,
        }),
        MouseEventKind::Moved => None,
    }
}

fn ct_button_to_abstract(b: CtMouseButton) -> MouseButton {
    match b {
        CtMouseButton::Left => MouseButton::Left,
        CtMouseButton::Right => MouseButton::Right,
        CtMouseButton::Middle => MouseButton::Middle,
    }
}
```

#### Update `poll()`

In the existing `poll()` function (around line 37), extend the inner `match event` block:

```rust
match event {
    Event::Key(key) => {
        if key.kind == event::KeyEventKind::Press {
            if let Some(input_key) = key_event_to_input(key) {
                Ok(Some(Message::Key(input_key)))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    Event::Mouse(mouse) => {
        if let Some(input) = mouse_event_to_input(mouse) {
            Ok(Some(Message::Mouse(input)))
        } else {
            Ok(None)
        }
    }
    _ => Ok(None),
}
```

`Event::Resize`, `Event::Paste`, `Event::FocusGained`, `Event::FocusLost` continue to fall through the `_` arm. The poll-timeout `Tick` behavior is unchanged.

### Acceptance Criteria

1. `mouse_event_to_input` exhaustively matches every crossterm 0.29 `MouseEventKind` variant (compiler enforces — no `_` catch-all). New variants added in future crossterm releases must produce a compile error.
2. `MouseEventKind::Moved` returns `None`; all other variants return `Some(MouseInput::*)`.
3. `KeyModifiers::SHIFT | CONTROL | ALT` round-trip correctly through `key_modifiers_to_set`.
4. `poll()` emits `Message::Mouse(_)` for any non-`Moved` mouse event.
5. Behavior on `Event::Key`, `Event::Resize`, `Event::Paste`, etc. is unchanged.
6. `cargo check -p fdemon-tui --all-targets` passes.
7. `cargo test -p fdemon-tui event` passes — including the existing `key_event_to_input` tests and the new mouse tests below.
8. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` passes.

### Testing

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `event.rs`:

```rust
#[test]
fn test_mouse_down_left_converts_to_click() {
    let ev = MouseEvent {
        kind: MouseEventKind::Down(CtMouseButton::Left),
        column: 5,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    let input = mouse_event_to_input(ev).expect("must convert");
    assert_eq!(
        input,
        MouseInput::Click {
            x: 5, y: 10,
            button: MouseButton::Left,
            modifiers: KeyModSet::NONE,
        }
    );
}

#[test]
fn test_mouse_up_right_converts_to_release() {
    let ev = MouseEvent {
        kind: MouseEventKind::Up(CtMouseButton::Right),
        column: 0, row: 0,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
        mouse_event_to_input(ev),
        Some(MouseInput::Release { button: MouseButton::Right, .. })
    ));
}

#[test]
fn test_mouse_drag_middle_converts() {
    let ev = MouseEvent {
        kind: MouseEventKind::Drag(CtMouseButton::Middle),
        column: 1, row: 2,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
        mouse_event_to_input(ev),
        Some(MouseInput::Drag { button: MouseButton::Middle, .. })
    ));
}

#[test]
fn test_scroll_up_converts() {
    let ev = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 0, row: 0,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
        mouse_event_to_input(ev),
        Some(MouseInput::Scroll { direction: ScrollDir::Up, .. })
    ));
}

#[test]
fn test_scroll_down_converts() {
    let ev = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0, row: 0,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
        mouse_event_to_input(ev),
        Some(MouseInput::Scroll { direction: ScrollDir::Down, .. })
    ));
}

#[test]
fn test_scroll_left_right_converts() {
    let left = MouseEvent {
        kind: MouseEventKind::ScrollLeft,
        column: 0, row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let right = MouseEvent {
        kind: MouseEventKind::ScrollRight,
        column: 0, row: 0,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
        mouse_event_to_input(left),
        Some(MouseInput::Scroll { direction: ScrollDir::Left, .. })
    ));
    assert!(matches!(
        mouse_event_to_input(right),
        Some(MouseInput::Scroll { direction: ScrollDir::Right, .. })
    ));
}

#[test]
fn test_moved_drops_to_none() {
    let ev = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 0, row: 0,
        modifiers: KeyModifiers::NONE,
    };
    assert!(mouse_event_to_input(ev).is_none());
}

#[test]
fn test_modifiers_round_trip_shift_ctrl_alt() {
    let m = KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT;
    let s = key_modifiers_to_set(m);
    assert!(s.shift && s.ctrl && s.alt);
}

#[test]
fn test_modifiers_drops_unmapped() {
    // SUPER, HYPER, META should be ignored.
    let m = KeyModifiers::SUPER;
    let s = key_modifiers_to_set(m);
    assert_eq!(s, KeyModSet::NONE);
}

#[test]
fn test_xy_coordinate_propagation() {
    let ev = MouseEvent {
        kind: MouseEventKind::Down(CtMouseButton::Left),
        column: 42, row: 7,
        modifiers: KeyModifiers::NONE,
    };
    let input = mouse_event_to_input(ev).unwrap();
    assert_eq!(input.position(), (42, 7));
}
```

### Notes

- **Field-name mismatch.** crossterm's `MouseEvent` uses `column`/`row`; our `MouseInput` uses `x`/`y`. The conversion is the only place that needs to keep this straight.
- **Exhaustive match enforced.** No `_` arm in `mouse_event_to_input` so new crossterm variants force us to think about each one.
- **Helper visibility = `pub(crate)`.** Keeps the conversion functions reachable for tests within `fdemon-tui` but not exposed to dependents.
- **Modifier limitations.** `KeyModSet` deliberately covers only Shift / Ctrl / Alt. Phase 2+ Shift+wheel is the only modifier-aware mouse mapping in the near term.
- **No allocation in the hot path.** Conversion uses only `Copy` types — no `String`, no `Vec`. Benchmarks should remain identical to keyboard-only event polling.
