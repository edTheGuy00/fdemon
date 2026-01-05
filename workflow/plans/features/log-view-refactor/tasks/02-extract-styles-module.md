## Task: Extract stack_trace_styles Module

**Objective**: Extract the `stack_trace_styles` module from `log_view.rs` into its own `styles.rs` file.

**Depends on**: `01-create-module-directory`

### Scope

- `src/tui/widgets/log_view.rs`: Lines 22-58 (stack_trace_styles module)
- `src/tui/widgets/log_view/styles.rs`: New file

### Implementation Details

1. **Create `src/tui/widgets/log_view/styles.rs`**:
   
   Copy the `stack_trace_styles` module content (L22-58) to the new file:
   
   ```rust
   //! Stack trace styling constants for log view rendering

   use ratatui::style::{Color, Modifier, Style};

   /// Frame number (#0, #1, etc.)
   pub const FRAME_NUMBER: Style = Style::new().fg(Color::DarkGray);

   /// Function name for project frames
   pub const FUNCTION_PROJECT: Style = Style::new().fg(Color::White);

   /// Function name for package frames
   pub const FUNCTION_PACKAGE: Style = Style::new().fg(Color::DarkGray);

   /// File path for project frames (clickable in Phase 3)
   pub const FILE_PROJECT: Style = Style::new()
       .fg(Color::Blue)
       .add_modifier(Modifier::UNDERLINED);

   /// File path for package frames
   pub const FILE_PACKAGE: Style = Style::new().fg(Color::DarkGray);

   /// Line/column numbers for project frames
   pub const LOCATION_PROJECT: Style = Style::new().fg(Color::Cyan);

   /// Line/column numbers for package frames
   pub const LOCATION_PACKAGE: Style = Style::new().fg(Color::DarkGray);

   /// Async suspension marker
   pub const ASYNC_GAP: Style = Style::new()
       .fg(Color::DarkGray)
       .add_modifier(Modifier::ITALIC);

   /// Punctuation (parentheses, colons)
   pub const PUNCTUATION: Style = Style::new().fg(Color::DarkGray);

   /// Indentation for stack frames
   pub const INDENT: &str = "    ";
   ```

2. **Update module declaration** in `mod.rs`:
   ```rust
   pub mod styles;
   ```

3. **Update references** in the main widget code:
   - Change `use stack_trace_styles::*;` to `use crate::tui::widgets::log_view::styles::*;`
   - Or use `use super::styles::*;` within the log_view module

### File Structure After This Task

```
src/tui/widgets/log_view/
├── mod.rs
└── styles.rs    # NEW - stack trace styling constants
```

### Acceptance Criteria

1. `styles.rs` contains all stack trace style constants
2. Constants are publicly accessible via `log_view::styles::`
3. Original `mod stack_trace_styles` block removed from source (in final cleanup task)
4. `cargo check` passes
5. No changes to runtime behavior

### Testing

- Run `cargo check` to verify compilation
- Run `cargo test` - style constant tests should still pass:
  - `test_stack_frame_styles_module_constants`

### Notes

- The module is renamed from `stack_trace_styles` to just `styles` for brevity
- If other code references `stack_trace_styles`, add a re-export: `pub use styles as stack_trace_styles;`
- The constants are `pub` so they can be accessed from the widget implementation

---

## Completion Summary

**Status:** Done

**Files modified:**
- Created: `src/tui/widgets/log_view/styles.rs` (37 lines)
- Modified: `src/tui/widgets/log_view/mod.rs` (2226 lines, down from 2263)
  - Added `pub mod styles;` declaration
  - Removed inline `mod stack_trace_styles` block (~37 lines)
  - Updated 4 occurrences of `use stack_trace_styles::*;` to `use styles::*;`

**Notable decisions/tradeoffs:**
- Used `use styles::*;` instead of a re-export alias since all usages were internal to the module
- Made the styles module `pub` so it can be accessed externally via `log_view::styles::`

**Testing performed:**
- `cargo check` - PASSED (no errors)
- `cargo test log_view` - PASSED (77/77 tests, including `test_stack_frame_styles_module_constants`)

**Risks/limitations:**
- None. The extraction was straightforward with no behavioral changes.