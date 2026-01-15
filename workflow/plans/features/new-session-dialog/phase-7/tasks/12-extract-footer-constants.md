# Task: Extract Footer Strings to Constants

## Summary

Move hard-coded footer strings in the NewSessionDialog widget to module-level constants for maintainability.

**Priority:** Minor

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/mod.rs` | Modify (lines 75-83) |

## Problem

Footer keybinding strings are hard-coded inline:

```rust
// Somewhere around lines 75-83
"[1/2] Tab  [Tab] Pane  [↑↓] Navigate  [Enter] Select  [Esc] Close"
```

This makes them harder to maintain and update.

## Implementation

Extract to module constants at the top of the file:

```rust
// src/tui/widgets/new_session_dialog/mod.rs

/// Footer text shown when no modal is open
const FOOTER_MAIN: &str = "[1/2] Tab  [Tab] Pane  [↑↓] Navigate  [Enter] Select  [Esc] Close";

/// Footer text shown when fuzzy modal is open
const FOOTER_FUZZY_MODAL: &str = "[↑↓] Navigate  [Enter] Select  [Esc] Cancel  [Type] Filter";

/// Footer text shown when dart defines modal is open
const FOOTER_DART_DEFINES: &str = "[Tab] Pane  [↑↓] Navigate  [Enter] Edit/Save  [Esc] Close";

/// Footer text for target selector pane
const FOOTER_TARGET_SELECTOR: &str = "[↑↓] Navigate  [Enter] Select  [r] Refresh";

/// Footer text for launch context pane
const FOOTER_LAUNCH_CONTEXT: &str = "[↑↓] Navigate  [Enter] Activate  [←→] Change Mode";
```

Then use these constants in the render code:

```rust
fn render_footer(&self, area: Rect, buf: &mut Buffer) {
    let footer_text = if self.state.is_fuzzy_modal_open() {
        FOOTER_FUZZY_MODAL
    } else if self.state.is_dart_defines_modal_open() {
        FOOTER_DART_DEFINES
    } else {
        FOOTER_MAIN
    };

    // Render footer_text...
}
```

## Acceptance Criteria

1. All footer strings extracted to `const` declarations
2. Constants have doc comments explaining context
3. Render code uses constants instead of literals
4. No functional changes to footer behavior

## Testing

```bash
cargo fmt && cargo check && cargo clippy -- -D warnings
```

## Notes

- Use `const &str` for zero-runtime-cost constants
- Group related constants together
- Consider if context-sensitive footers (per pane) are needed
- This is a pure refactoring task - no behavior changes
