## Task: Create Theme Module Structure

**Objective**: Create the centralized theme module with palette constants, semantic style builders, and Nerd Font icon constants. This is the foundation all other Phase 1 tasks depend on.

**Depends on**: None

### Scope

- `crates/fdemon-tui/src/theme/mod.rs` — **NEW** Public API, re-exports
- `crates/fdemon-tui/src/theme/palette.rs` — **NEW** Color constants
- `crates/fdemon-tui/src/theme/styles.rs` — **NEW** Semantic style builders
- `crates/fdemon-tui/src/theme/icons.rs` — **NEW** Nerd Font glyph constants
- `crates/fdemon-tui/src/lib.rs` — Add `pub(crate) mod theme;`

### Details

#### 1. Module Registration (`lib.rs`)

Add `pub(crate) mod theme;` to `crates/fdemon-tui/src/lib.rs` alongside existing modules.

#### 2. Palette (`theme/palette.rs`)

Define color constants that **map to the current named colors** used throughout the crate. This preserves existing visual appearance while centralizing definitions. The RGB values from the PLAN.md design tokens will replace these in Phase 2.

```rust
//! Color palette for the Cyber-Glass theme.
//!
//! Phase 1: Maps to existing named colors for zero visual regression.
//! Phase 2+: Will transition to RGB design token values.

use ratatui::style::Color;

// --- Background layers ---
pub const DEEPEST_BG: Color = Color::Black;         // Terminal background (Phase 2: Rgb(10,12,16))
pub const CARD_BG: Color = Color::Black;             // Panel/card backgrounds (Phase 2: Rgb(18,21,28))
pub const POPUP_BG: Color = Color::DarkGray;         // Modal/popup backgrounds (Phase 2: Rgb(28,33,43))
pub const SURFACE: Color = Color::Black;             // Elevated surface (Phase 2: Rgb(22,27,34))

// --- Borders ---
pub const BORDER_DIM: Color = Color::DarkGray;       // Inactive borders (Phase 2: Rgb(45,51,59))
pub const BORDER_ACTIVE: Color = Color::Cyan;        // Focused borders (Phase 2: Rgb(88,166,255))

// --- Accent ---
pub const ACCENT: Color = Color::Cyan;               // Primary accent (Phase 2: Rgb(88,166,255))
pub const ACCENT_DIM: Color = Color::DarkGray;       // Dimmed accent (Phase 2: Rgb(56,107,163))

// --- Text ---
pub const TEXT_PRIMARY: Color = Color::White;         // Primary text (Phase 2: Rgb(201,209,217))
pub const TEXT_SECONDARY: Color = Color::Gray;        // Secondary text (Phase 2: Rgb(125,133,144))
pub const TEXT_MUTED: Color = Color::DarkGray;        // Muted text (Phase 2: Rgb(72,79,88))
pub const TEXT_BRIGHT: Color = Color::White;          // Bright/emphasis text (Phase 2: Rgb(240,246,252))

// --- Status ---
pub const STATUS_GREEN: Color = Color::Green;        // Running/success (Phase 2: Rgb(16,185,129))
pub const STATUS_RED: Color = Color::Red;            // Error/stopped (Phase 2: Rgb(244,63,94))
pub const STATUS_YELLOW: Color = Color::Yellow;      // Warning/reloading (Phase 2: Rgb(234,179,8))
pub const STATUS_BLUE: Color = Color::Blue;          // Info (Phase 2: Rgb(56,189,248))
pub const STATUS_INDIGO: Color = Color::Magenta;     // Flutter messages (Phase 2: Rgb(129,140,248))

// --- Effects ---
pub const SHADOW: Color = Color::Black;              // Shadow color (Phase 2: Rgb(5,6,8))

// --- Gradients (approximate) ---
pub const GRADIENT_BLUE: Color = Color::Blue;        // Button gradient start (Phase 2: Rgb(37,99,235))
pub const GRADIENT_INDIGO: Color = Color::Magenta;   // Button gradient end (Phase 2: Rgb(99,102,241))

// --- Log level colors ---
pub const LOG_ERROR: Color = Color::Red;
pub const LOG_ERROR_MSG: Color = Color::LightRed;
pub const LOG_WARNING: Color = Color::Yellow;
pub const LOG_WARNING_MSG: Color = Color::Yellow;
pub const LOG_INFO: Color = Color::Green;
pub const LOG_INFO_MSG: Color = Color::White;
pub const LOG_DEBUG: Color = Color::DarkGray;
pub const LOG_DEBUG_MSG: Color = Color::DarkGray;

// --- Log source colors ---
pub const SOURCE_APP: Color = Color::Magenta;
pub const SOURCE_DAEMON: Color = Color::Yellow;
pub const SOURCE_FLUTTER: Color = Color::Blue;
pub const SOURCE_FLUTTER_ERROR: Color = Color::Red;
pub const SOURCE_WATCHER: Color = Color::Cyan;

// --- Search highlight ---
pub const SEARCH_HIGHLIGHT_FG: Color = Color::Black;
pub const SEARCH_HIGHLIGHT_BG: Color = Color::Yellow;
pub const SEARCH_CURRENT_FG: Color = Color::Black;
pub const SEARCH_CURRENT_BG: Color = Color::LightYellow;

// --- Stack trace ---
pub const STACK_FRAME_NUMBER: Color = Color::DarkGray;
pub const STACK_FUNCTION_PROJECT: Color = Color::White;
pub const STACK_FUNCTION_PACKAGE: Color = Color::DarkGray;
pub const STACK_FILE_PROJECT: Color = Color::Blue;
pub const STACK_FILE_PACKAGE: Color = Color::DarkGray;
pub const STACK_LOCATION_PROJECT: Color = Color::Cyan;
pub const STACK_LOCATION_PACKAGE: Color = Color::DarkGray;
pub const STACK_ASYNC_GAP: Color = Color::DarkGray;
pub const STACK_PUNCTUATION: Color = Color::DarkGray;

// --- Modal backgrounds (existing Rgb values preserved) ---
pub const MODAL_FUZZY_BG: Color = Color::Rgb(40, 40, 50);
pub const MODAL_FUZZY_QUERY_BG: Color = Color::Rgb(60, 60, 70);
pub const MODAL_DART_DEFINES_BG: Color = Color::Rgb(30, 30, 40);
pub const MODAL_DART_DEFINES_INPUT_ACTIVE_BG: Color = Color::Rgb(60, 60, 80);
pub const MODAL_DART_DEFINES_INPUT_INACTIVE_BG: Color = Color::Rgb(40, 40, 50);
pub const MODAL_DART_DEFINES_BUTTON_INACTIVE_BG: Color = Color::Rgb(50, 50, 60);
pub const MODAL_DART_DEFINES_CLEAR_BG: Color = Color::Rgb(20, 20, 30);
pub const LINK_BAR_BG: Color = Color::Rgb(30, 30, 30);
```

#### 3. Styles (`theme/styles.rs`)

Semantic style builder functions. These are the primary public API widgets will use.

```rust
//! Semantic style builders for the Cyber-Glass theme.

use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};
use super::palette;

// --- Text styles ---
pub fn text_primary() -> Style { Style::default().fg(palette::TEXT_PRIMARY) }
pub fn text_secondary() -> Style { Style::default().fg(palette::TEXT_SECONDARY) }
pub fn text_muted() -> Style { Style::default().fg(palette::TEXT_MUTED) }
pub fn text_bright() -> Style { Style::default().fg(palette::TEXT_BRIGHT) }

// --- Border styles ---
pub fn border_inactive() -> Style { Style::default().fg(palette::BORDER_DIM) }
pub fn border_active() -> Style { Style::default().fg(palette::BORDER_ACTIVE) }

// --- Accent styles ---
pub fn accent() -> Style { Style::default().fg(palette::ACCENT) }
pub fn accent_bold() -> Style { Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD) }

// --- Status styles ---
pub fn status_green() -> Style { Style::default().fg(palette::STATUS_GREEN) }
pub fn status_red() -> Style { Style::default().fg(palette::STATUS_RED) }
pub fn status_yellow() -> Style { Style::default().fg(palette::STATUS_YELLOW) }
pub fn status_blue() -> Style { Style::default().fg(palette::STATUS_BLUE) }

// --- Keybinding hint style ---
pub fn keybinding() -> Style { Style::default().fg(palette::STATUS_YELLOW) }

// --- Selection styles ---
pub fn selected_highlight() -> Style {
    Style::default().fg(palette::TEXT_BRIGHT).bg(palette::ACCENT).add_modifier(Modifier::BOLD)
}
// "Black on Cyan" - used for focused+selected items across widgets
pub fn focused_selected() -> Style {
    Style::default().fg(Color::Black).bg(palette::ACCENT).add_modifier(Modifier::BOLD)
}

// --- Block builders ---
pub fn glass_block(focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused { border_active() } else { border_inactive() })
}

pub fn modal_block(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_inactive())
        .style(Style::default().bg(palette::POPUP_BG))
}
```

**Note:** The exact function signatures may need adjustment during implementation. The above is a guideline. Additional helpers can be added as needed by Task 03 migration.

#### 4. Icons (`theme/icons.rs`)

Nerd Font glyph constants with ASCII fallbacks. In Phase 1 these are defined but NOT yet used in widgets (widgets continue using their current text — icon adoption happens in Phase 2+).

```rust
//! Nerd Font glyph constants.
//!
//! Each icon has a Nerd Font variant and an ASCII fallback.
//! Use the `icon()` function to select based on configuration.

pub const ICON_TERMINAL: &str = "\u{f120}";     // nf-fa-terminal
pub const ICON_SMARTPHONE: &str = "\u{f3cd}";   // nf-fa-mobile
pub const ICON_GLOBE: &str = "\u{f0ac}";        // nf-fa-globe
pub const ICON_MONITOR: &str = "\u{f108}";      // nf-fa-desktop
pub const ICON_ACTIVITY: &str = "\u{f0f1}";     // nf-fa-heartbeat
pub const ICON_PLAY: &str = "\u{f04b}";         // nf-fa-play
pub const ICON_STOP: &str = "\u{f04d}";         // nf-fa-stop
pub const ICON_REFRESH: &str = "\u{f021}";      // nf-fa-refresh
pub const ICON_ALERT: &str = "\u{f071}";        // nf-fa-warning
pub const ICON_CHECK: &str = "\u{f00c}";        // nf-fa-check
pub const ICON_CLOSE: &str = "\u{f00d}";        // nf-fa-close
pub const ICON_CHEVRON_R: &str = "\u{f054}";    // nf-fa-chevron_right
pub const ICON_CHEVRON_D: &str = "\u{f078}";    // nf-fa-chevron_down
pub const ICON_DOT: &str = "\u{f444}";          // nf-oct-dot_fill
pub const ICON_LAYERS: &str = "\u{f5fd}";       // nf-mdi-layers
pub const ICON_CPU: &str = "\u{f2db}";          // nf-fa-microchip
pub const ICON_SETTINGS: &str = "\u{f013}";     // nf-fa-cog
pub const ICON_ZAP: &str = "\u{f0e7}";          // nf-fa-bolt
pub const ICON_EYE: &str = "\u{f06e}";          // nf-fa-eye
pub const ICON_CODE: &str = "\u{f121}";         // nf-fa-code
pub const ICON_USER: &str = "\u{f007}";         // nf-fa-user
pub const ICON_INFO: &str = "\u{f05a}";         // nf-fa-info_circle
pub const ICON_KEYBOARD: &str = "\u{f11c}";     // nf-fa-keyboard_o
pub const ICON_COMMAND: &str = "\u{f120}";       // nf-fa-terminal
pub const ICON_SAVE: &str = "\u{f0c7}";         // nf-fa-floppy_o

// --- ASCII fallbacks ---
pub const ASCII_TERMINAL: &str = ">";
pub const ASCII_SMARTPHONE: &str = "[M]";
pub const ASCII_GLOBE: &str = "[W]";
pub const ASCII_MONITOR: &str = "[D]";
pub const ASCII_ACTIVITY: &str = "~";
pub const ASCII_PLAY: &str = ">";
pub const ASCII_STOP: &str = "x";
pub const ASCII_REFRESH: &str = "@";
pub const ASCII_ALERT: &str = "!";
pub const ASCII_CHECK: &str = "*";
pub const ASCII_CLOSE: &str = "x";
pub const ASCII_CHEVRON_R: &str = ">";
pub const ASCII_CHEVRON_D: &str = "v";
pub const ASCII_DOT: &str = "*";
pub const ASCII_LAYERS: &str = "#";
pub const ASCII_CPU: &str = "[C]";
pub const ASCII_SETTINGS: &str = "*";
pub const ASCII_ZAP: &str = "!";
pub const ASCII_EYE: &str = "o";
pub const ASCII_CODE: &str = "</>";
pub const ASCII_USER: &str = "@";
pub const ASCII_INFO: &str = "(i)";
pub const ASCII_KEYBOARD: &str = "[K]";
pub const ASCII_COMMAND: &str = "$";
pub const ASCII_SAVE: &str = "[S]";
```

#### 5. Module Hub (`theme/mod.rs`)

```rust
//! Centralized theme system for the Cyber-Glass TUI design.
//!
//! This module provides:
//! - `palette` — Raw color constants
//! - `styles` — Semantic style builder functions
//! - `icons` — Nerd Font glyph constants with ASCII fallbacks

pub mod icons;
pub mod palette;
pub mod styles;
```

### Acceptance Criteria

1. `crates/fdemon-tui/src/theme/` directory exists with 4 files (`mod.rs`, `palette.rs`, `styles.rs`, `icons.rs`)
2. `pub(crate) mod theme;` added to `lib.rs`
3. `palette.rs` defines all color constants from the codebase audit (background layers, borders, accent, text, status, log levels, log sources, search highlights, stack trace colors, modal backgrounds)
4. `styles.rs` defines semantic style builders (`text_primary()`, `border_active()`, `glass_block()`, etc.)
5. `icons.rs` defines all Nerd Font glyph constants with ASCII fallbacks
6. `cargo check -p fdemon-tui` passes
7. `cargo clippy -p fdemon-tui` passes with no warnings
8. No other files are modified (the theme module is additive-only in this task)

### Testing

The theme module is purely constants and builder functions. Add basic unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_constants_are_valid() {
        // Verify a few representative constants compile and are the expected type
        let _: Color = palette::ACCENT;
        let _: Color = palette::DEEPEST_BG;
        let _: Color = palette::STATUS_GREEN;
    }

    #[test]
    fn test_style_builders_return_styles() {
        let s = styles::text_primary();
        assert_eq!(s.fg, Some(palette::TEXT_PRIMARY));
    }

    #[test]
    fn test_glass_block_focused_vs_unfocused() {
        let focused = styles::glass_block(true);
        let unfocused = styles::glass_block(false);
        // Both should have rounded borders
        // Focused should use BORDER_ACTIVE color
        // Unfocused should use BORDER_DIM color
    }

    #[test]
    fn test_icon_constants_are_non_empty() {
        assert!(!icons::ICON_TERMINAL.is_empty());
        assert!(!icons::ASCII_TERMINAL.is_empty());
    }
}
```

### Notes

- The palette values in Phase 1 are **intentionally mapped to named colors** (e.g., `Color::Cyan` not `Color::Rgb(88,166,255)`). This ensures zero visual regression. The RGB transition happens in Phase 2.
- Some style builders like `focused_selected()` need `use ratatui::style::Color;` for the `Color::Black` reference. Consider whether to add a `palette::BLACK` constant or import Color directly.
- The `styles.rs` functions are guidelines — the implementor may need to add more helpers as they discover common patterns during Task 03 migration.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/lib.rs` | Added `pub(crate) mod theme;` to module declarations |
| `crates/fdemon-tui/src/theme/mod.rs` | Created theme module hub with public submodules (palette, styles, icons) |
| `crates/fdemon-tui/src/theme/palette.rs` | Created color constants for all categories (backgrounds, borders, accent, text, status, log levels, log sources, search highlights, stack trace colors, modal backgrounds) with `#![allow(dead_code)]` since these are infrastructure for Phase 2 |
| `crates/fdemon-tui/src/theme/styles.rs` | Created semantic style builder functions (text styles, border styles, accent styles, status styles, selection styles, block builders) with `#![allow(dead_code)]` |
| `crates/fdemon-tui/src/theme/icons.rs` | Created Nerd Font glyph constants and ASCII fallbacks with `#![allow(dead_code)]` |

### Notable Decisions/Tradeoffs

1. **Module-level `#![allow(dead_code)]` annotations**: Added to all three submodules (palette.rs, styles.rs, icons.rs) because these are infrastructure constants and functions that will be consumed in Phase 2 during widget migration. Without these annotations, clippy would fail with `-D warnings` since nothing uses them yet.

2. **Simplified Block tests**: The ratatui Block API doesn't expose getters for border_type, border_style, or style properties. Tests were simplified to verify that the builder functions successfully construct Block instances rather than asserting on internal state.

3. **Direct Color import in styles.rs**: Used `Color::Black` directly in `focused_selected()` style builder rather than adding a `palette::BLACK` constant, following the task's guideline to "Consider whether to add a palette::BLACK constant or import Color directly."

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui` - Passed (467 tests total, including 15 new theme tests)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed

### Risks/Limitations

1. **Dead code warnings**: The theme module constants and functions are intentionally unused in Phase 1. The `#![allow(dead_code)]` annotations suppress warnings but should be removed in Phase 2 once adoption begins.

2. **No visual changes**: As specified, this task creates infrastructure only. The theme is not yet used by any widgets. Widget migration happens in subsequent tasks (Task 03).

3. **Block builder tests are minimal**: Since Block doesn't expose internal state, tests only verify successful construction. Visual correctness will be validated during widget migration and snapshot testing.
