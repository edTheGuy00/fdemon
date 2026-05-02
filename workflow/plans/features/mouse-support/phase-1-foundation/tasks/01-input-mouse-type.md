## Task: Add `MouseInput` abstract type to `fdemon-app`

**Objective**: Create a terminal-library-agnostic `MouseInput` enum (and supporting `MouseButton`, `ScrollDir`, `KeyModSet` types) that mirrors how `InputKey` abstracts keyboard input. Re-export it from the crate root so the TUI layer can construct it at the crossterm boundary.

**Depends on**: None

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/input_mouse.rs` — **NEW**: enum definitions, constructors, `Debug`/`Clone`/`PartialEq` derives, unit tests
- `crates/fdemon-app/src/lib.rs` — register the new `pub(crate) mod input_mouse;` and add `pub use input_mouse::{MouseInput, MouseButton, ScrollDir, KeyModSet};`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/input_key.rs` — pattern reference (visibility, derives, doc comments, test layout)
- `crates/fdemon-app/src/lib.rs` — to find the right place to insert the module declaration and re-exports

### Details

Mirror the structure of `input_key.rs` exactly so future readers can see the symmetry between keyboard and mouse handling. Coordinates are `u16` to match crossterm and ratatui `Rect` fields. `KeyModSet` is a small bitset wrapper independent of crossterm so `fdemon-app` keeps zero dependency on `crossterm`.

`MouseInput::Moved` is intentionally omitted — high-volume motion events have no current consumer and we drop them at the TUI boundary. `MouseInput::Drag` is included so a future panel-resize feature can subscribe without re-plumbing types, but Phase 1 has no consumer for it.

**File contents — `crates/fdemon-app/src/input_mouse.rs`:**

```rust
//! Abstract mouse input event, independent of terminal library.
//!
//! This module mirrors [`crate::input_key`] for mouse interactions. It defines
//! the [`MouseInput`] enum which abstracts pointer input from the underlying
//! terminal library (crossterm). This keeps `fdemon-app` free of any
//! `crossterm` dependency and lets future non-TUI consumers (a GUI front-end,
//! an MCP server) deliver mouse events the same way they deliver key events.
//!
//! The terminal-library-specific conversion lives in `fdemon-tui::event`,
//! which translates `crossterm::event::MouseEvent` into [`MouseInput`].
//!
//! ## Variants we deliberately omit
//!
//! - `Moved` (motion without a button): high-volume, no current consumer.
//!   Dropped at the TUI boundary in [`fdemon_tui::event::poll`].

/// Mouse button identifier, independent of terminal library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Scroll-wheel direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollDir {
    Up,
    Down,
    /// Touchpad horizontal scroll (rare on external mice).
    Left,
    /// Touchpad horizontal scroll (rare on external mice).
    Right,
}

/// Modifier-key bitset attached to mouse events.
///
/// Independent of `crossterm::event::KeyModifiers` so `fdemon-app` does not
/// take a transitive dependency on the terminal library.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct KeyModSet {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl KeyModSet {
    /// Empty modifier set.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
    };

    /// Convenience constructor for tests.
    pub const fn new(shift: bool, ctrl: bool, alt: bool) -> Self {
        Self { shift, ctrl, alt }
    }
}

/// Abstract mouse input event.
///
/// Coordinates use the same `(column, row)` convention as ratatui's `Rect`
/// and crossterm's `MouseEvent`. The origin is the top-left of the terminal
/// screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseInput {
    /// Primary button pressed at `(x, y)`.
    Click {
        x: u16,
        y: u16,
        button: MouseButton,
        modifiers: KeyModSet,
    },
    /// Primary button released at `(x, y)`.
    Release {
        x: u16,
        y: u16,
        button: MouseButton,
        modifiers: KeyModSet,
    },
    /// Drag motion at `(x, y)` with `button` held down.
    Drag {
        x: u16,
        y: u16,
        button: MouseButton,
        modifiers: KeyModSet,
    },
    /// Wheel scroll at `(x, y)`.
    Scroll {
        x: u16,
        y: u16,
        direction: ScrollDir,
        modifiers: KeyModSet,
    },
}

impl MouseInput {
    /// Returns the `(x, y)` cell coordinate of the event regardless of variant.
    pub fn position(&self) -> (u16, u16) {
        match *self {
            MouseInput::Click { x, y, .. }
            | MouseInput::Release { x, y, .. }
            | MouseInput::Drag { x, y, .. }
            | MouseInput::Scroll { x, y, .. } => (x, y),
        }
    }

    /// Returns the modifier set attached to the event.
    pub fn modifiers(&self) -> KeyModSet {
        match *self {
            MouseInput::Click { modifiers, .. }
            | MouseInput::Release { modifiers, .. }
            | MouseInput::Drag { modifiers, .. }
            | MouseInput::Scroll { modifiers, .. } => modifiers,
        }
    }
}
```

**Re-exports in `crates/fdemon-app/src/lib.rs`:**

Insert next to the existing `pub(crate) mod input_key;`:

```rust
pub(crate) mod input_mouse;
```

Insert next to the existing `pub use input_key::InputKey;`:

```rust
// Re-export mouse input types used by TUI for event conversion
pub use input_mouse::{KeyModSet, MouseButton, MouseInput, ScrollDir};
```

### Acceptance Criteria

1. `crates/fdemon-app/src/input_mouse.rs` exists with `MouseInput`, `MouseButton`, `ScrollDir`, `KeyModSet` and the `Debug`/`Clone`/`Copy`/`PartialEq`/`Eq`/`Hash` derives shown above.
2. `MouseInput::position()` returns the embedded `(x, y)` for every variant.
3. `MouseInput::modifiers()` returns the embedded `KeyModSet` for every variant.
4. `KeyModSet::NONE` and `KeyModSet::new(...)` are usable as `const` expressions.
5. The four types are re-exported from `fdemon_app` crate root.
6. `cargo check -p fdemon-app` passes.
7. `cargo test -p fdemon-app input_mouse` runs the unit tests below and they all pass.
8. No new external dependencies added to `crates/fdemon-app/Cargo.toml`.
9. No `crossterm` import in `input_mouse.rs`.

### Testing

Place these in a `#[cfg(test)] mod tests` block at the bottom of `input_mouse.rs`. Mirror `input_key.rs::tests` style.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_click_constructible_and_eq() {
        let a = MouseInput::Click {
            x: 10,
            y: 5,
            button: MouseButton::Left,
            modifiers: KeyModSet::NONE,
        };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn test_position_returns_xy_for_each_variant() {
        let click = MouseInput::Click {
            x: 1, y: 2, button: MouseButton::Left, modifiers: KeyModSet::NONE,
        };
        let release = MouseInput::Release {
            x: 3, y: 4, button: MouseButton::Right, modifiers: KeyModSet::NONE,
        };
        let drag = MouseInput::Drag {
            x: 5, y: 6, button: MouseButton::Middle, modifiers: KeyModSet::NONE,
        };
        let scroll = MouseInput::Scroll {
            x: 7, y: 8, direction: ScrollDir::Up, modifiers: KeyModSet::NONE,
        };
        assert_eq!(click.position(), (1, 2));
        assert_eq!(release.position(), (3, 4));
        assert_eq!(drag.position(), (5, 6));
        assert_eq!(scroll.position(), (7, 8));
    }

    #[test]
    fn test_modifiers_returns_attached_modset() {
        let mods = KeyModSet::new(true, false, true);
        let click = MouseInput::Click {
            x: 0, y: 0, button: MouseButton::Left, modifiers: mods,
        };
        assert_eq!(click.modifiers(), mods);
    }

    #[test]
    fn test_keymodset_none_is_empty() {
        assert!(!KeyModSet::NONE.shift);
        assert!(!KeyModSet::NONE.ctrl);
        assert!(!KeyModSet::NONE.alt);
    }

    #[test]
    fn test_keymodset_new_round_trip() {
        let m = KeyModSet::new(true, true, false);
        assert!(m.shift);
        assert!(m.ctrl);
        assert!(!m.alt);
    }

    #[test]
    fn test_mouse_button_inequality() {
        assert_ne!(MouseButton::Left, MouseButton::Right);
        assert_ne!(MouseButton::Right, MouseButton::Middle);
    }

    #[test]
    fn test_scroll_dir_variants_distinct() {
        assert_ne!(ScrollDir::Up, ScrollDir::Down);
        assert_ne!(ScrollDir::Left, ScrollDir::Right);
    }

    #[test]
    fn test_debug_format_contains_variant_name() {
        let click = MouseInput::Click {
            x: 0, y: 0, button: MouseButton::Left, modifiers: KeyModSet::NONE,
        };
        let s = format!("{:?}", click);
        assert!(s.contains("Click"));
    }
}
```

### Notes

- **No `Moved` variant.** This is a deliberate omission documented at the top of the module.
- **`Copy` is safe.** All fields are `Copy` (`u16`, enum, small bitset), keeping ergonomics good and avoiding unnecessary clones in the hit-test path.
- **Module visibility is `pub(crate)`.** Outside crates use the re-exports from `fdemon_app` root, matching the existing `input_key` pattern.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a1882d6ad45f4f17e

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/input_mouse.rs` | New file: `MouseInput`, `MouseButton`, `ScrollDir`, `KeyModSet` types with full derives, `position()` and `modifiers()` helpers, 8 unit tests |
| `crates/fdemon-app/src/lib.rs` | Added `pub(crate) mod input_mouse;` declaration; added `pub use input_mouse::{KeyModSet, MouseButton, MouseInput, ScrollDir};` re-export |

### Notable Decisions/Tradeoffs

1. **Exact specification compliance**: Implemented the file contents verbatim as specified in the task, including all doc comments and deliberate omissions (`Moved` variant).
2. **No external deps added**: All types are pure Rust stdlib — no `crossterm` import, no new `Cargo.toml` entries.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app input_mouse` - Passed (8 tests, 0 failed)

### Risks/Limitations

1. **No consumers yet**: `MouseInput` is defined but not yet wired into `Message` or any handler — this is intentional per task scope; subsequent tasks (02 through 06) handle that.
