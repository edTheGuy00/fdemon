## Task: Project Selector

**Objective**: Create an interactive terminal-based selector that allows users to choose from multiple discovered Flutter projects before the main TUI starts.

**Depends on**: [01-discovery-module](01-discovery-module.md)

---

### Scope

- `src/tui/selector.rs`: New module for project selection UI
- `src/tui/mod.rs`: Add `pub mod selector;` export

---

### Implementation Details

#### Public API

```rust
use std::path::PathBuf;
use crate::common::prelude::*;

/// Result of project selection
pub enum SelectionResult {
    /// User selected a project
    Selected(PathBuf),
    /// User cancelled selection (pressed 'q' or Ctrl+C)
    Cancelled,
}

/// Display a project selector and wait for user input
/// 
/// # Arguments
/// * `projects` - List of discovered project paths to display
/// * `searched_from` - Base path that was searched (for display context)
/// 
/// # Returns
/// * `Result<SelectionResult>` - Selected project or cancellation
/// 
/// # Panics
/// * Panics if `projects` is empty (caller should handle this case)
pub fn select_project(
    projects: &[PathBuf], 
    searched_from: &Path
) -> Result<SelectionResult>;
```

#### UI Design

```
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│  Flutter Demon                                                     │
│                                                                    │
│  Multiple Flutter projects found in:                               │
│  /Users/ed/Dev/zabin/flutter-demon                                 │
│                                                                    │
│  Select a project:                                                 │
│                                                                    │
│    [1] sample                                                      │
│    [2] examples/counter_app                                        │
│    [3] examples/todo_app                                           │
│                                                                    │
│  Enter number (1-3) or 'q' to quit: _                              │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

#### Implementation Approach

1. **Raw Terminal Mode**: Use crossterm to enter raw mode for single-keypress input
2. **Display Menu**: Print the selection menu to stdout
3. **Read Input**: Wait for valid keypress (1-9, q, Ctrl+C)
4. **Restore Terminal**: Ensure terminal is restored even on error/panic
5. **Return Result**: Return selected path or cancellation

#### Key Features

- **Relative Paths**: Display paths relative to `searched_from` for readability
- **Single Keypress**: User presses a single key (no Enter required)
- **Number Keys**: Support 1-9 for selection (up to 9 projects)
- **Cancel Options**: 'q', 'Q', Escape, or Ctrl+C to cancel
- **Input Validation**: Ignore invalid keypresses, wait for valid input
- **Limit Display**: If >9 projects, show first 9 with a note about more

#### Color Scheme (using crossterm)

```rust
use crossterm::style::{Color, Stylize};

// Title: Bold cyan
"Flutter Demon".bold().cyan()

// Path context: Dim white
searched_from.display().to_string().dim()

// Project numbers: Bold yellow
format!("[{}]", i + 1).bold().yellow()

// Project paths: White
project_path.display().to_string().white()

// Prompt: Bold white
"Enter number (1-3) or 'q' to quit: ".bold()
```

---

### Acceptance Criteria

1. Selector displays all projects with numbered options
2. Pressing a number key (1-9) selects the corresponding project
3. Pressing 'q', 'Q', or Escape returns `SelectionResult::Cancelled`
4. Pressing Ctrl+C returns `SelectionResult::Cancelled`
5. Invalid keys are ignored (no error, just wait for valid input)
6. Paths are displayed relative to the searched directory
7. Terminal is properly restored after selection (even on panic)
8. Works with 1 project (though caller should auto-select in this case)
9. Works with up to 9 projects
10. Shows truncation message if more than 9 projects

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Note: Interactive tests are difficult to automate.
    // These tests focus on helper functions.

    #[test]
    fn test_format_relative_path() {
        let base = PathBuf::from("/home/user/projects");
        let project = PathBuf::from("/home/user/projects/my_app");
        
        let relative = format_relative_path(&project, &base);
        assert_eq!(relative, "my_app");
    }

    #[test]
    fn test_format_relative_path_nested() {
        let base = PathBuf::from("/home/user/projects");
        let project = PathBuf::from("/home/user/projects/examples/counter");
        
        let relative = format_relative_path(&project, &base);
        assert_eq!(relative, "examples/counter");
    }

    #[test]
    fn test_format_relative_path_same() {
        let base = PathBuf::from("/home/user/projects");
        let project = PathBuf::from("/home/user/projects");
        
        let relative = format_relative_path(&project, &base);
        assert_eq!(relative, ".");
    }

    #[test]
    fn test_validate_selection_valid() {
        let projects = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        
        assert!(validate_selection('1', &projects).is_some());
        assert!(validate_selection('3', &projects).is_some());
        assert!(validate_selection('4', &projects).is_none()); // Out of range
        assert!(validate_selection('0', &projects).is_none()); // Zero not valid
        assert!(validate_selection('a', &projects).is_none()); // Letter not valid
    }
}
```

---

### Error Handling

| Scenario | Handling |
|----------|----------|
| Crossterm fails to enter raw mode | Return `Error::Terminal` |
| stdout write fails | Return `Error::Io` |
| Signal during selection | Return `SelectionResult::Cancelled` |
| Empty projects list | Panic (caller's responsibility) |

---

### Helper Functions

```rust
/// Format a project path relative to the base path
fn format_relative_path(project: &Path, base: &Path) -> String;

/// Validate and convert a key press to a project index
fn validate_selection(key: char, projects: &[PathBuf]) -> Option<usize>;

/// Check if a key press is a cancellation request
fn is_cancel_key(key: KeyCode) -> bool;
```

---

### Notes

- This runs BEFORE the main TUI initializes (simpler state management)
- Uses crossterm directly (not ratatui) for simplicity
- Could be enhanced later with arrow key navigation
- Terminal restoration is critical - use `Drop` guard pattern
- Consider accessibility: high-contrast colors, clear prompts

---

## Completion Summary

**Status**: ✅ Done

**Files Modified**:
- `src/tui/selector.rs` (created) - Interactive project selector implementation
- `src/tui/mod.rs` - Added `pub mod selector` and re-exports

**Notable Decisions/Tradeoffs**:
- Used `RawModeGuard` RAII pattern for terminal restoration (Drop trait)
- Uses `terminal::is_raw_mode_enabled().unwrap_or(false)` for safe raw mode checking
- Color scheme: Cyan title, Yellow number brackets, DarkGrey paths
- Maximum 9 projects displayed (single-digit selection limitation)
- Paths displayed relative to `searched_from` for readability
- Cancel keys: 'q', 'Q', Escape, Ctrl+C

**Public API**:
- `SelectionResult` enum: `Selected(PathBuf)` or `Cancelled`
- `select_project(projects, searched_from)` - main entry point
- `format_relative_path(project, base)` - helper for path display
- `validate_selection(key, projects)` - validates number key input
- `is_cancel_key(code, modifiers)` - checks for cancel keys

**Testing Performed**:
- `cargo fmt` - Passed (no formatting issues)
- `cargo check` - No warnings
- `cargo test` - All 68 tests passed (8 new selector tests)

**Test Coverage**:
- `test_format_relative_path` - Basic relative path formatting
- `test_format_relative_path_nested` - Nested subdirectory formatting
- `test_format_relative_path_same` - Same directory returns "."
- `test_format_relative_path_outside` - Outside base returns absolute path
- `test_validate_selection_valid` - Valid number key selection
- `test_validate_selection_max_projects` - Respects MAX_DISPLAY_PROJECTS limit
- `test_is_cancel_key` - All cancel key combinations
- `test_selection_result_eq` - SelectionResult equality

**Risks/Limitations**:
- Interactive functionality difficult to test automatically
- Limited to 9 projects max (single digit keys)
- No arrow key navigation (could be added later)
- Assumes stdout is a terminal (no TTY detection)