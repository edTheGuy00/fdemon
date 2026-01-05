## Task: Migrate Widget Implementation to mod.rs

**Objective**: Move the `LogView` struct, its implementation, and the trait implementations (`StatefulWidget`, `Widget`) into the new `mod.rs` file as the main widget code.

**Depends on**: `01-create-module-directory`, `02-extract-styles-module`, `03-extract-state-module`

### Scope

- `src/tui/widgets/log_view.rs`: Lines 255-1212
  - `LogView` struct (L255-272)
  - `impl LogView` (L274-1000) - All formatting and helper methods
  - `impl StatefulWidget for LogView` (L1002-1204)
  - `impl Widget for LogView` (L1207-1212)
- `src/tui/widgets/log_view/mod.rs`: Update with widget code

### Implementation Details

1. **Update `src/tui/widgets/log_view/mod.rs`** with the complete widget implementation:

   ```rust
   //! Scrollable log view widget with rich formatting

   use std::collections::VecDeque;

   use crate::core::{
       FilterState, LogEntry, LogLevel, LogLevelFilter, LogSource, LogSourceFilter, SearchState,
       StackFrame,
   };
   use crate::tui::hyperlinks::LinkHighlightState;
   use ratatui::{
       buffer::Buffer,
       layout::Rect,
       style::{Color, Modifier, Style},
       text::{Line, Span},
       widgets::{
           Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
           StatefulWidget, Widget,
       },
   };

   mod state;
   pub mod styles;

   #[cfg(test)]
   mod tests;

   // Re-export public types
   pub use state::{FocusInfo, LogViewState};

   // Use styles internally
   use styles as stack_trace_styles;

   /// Log view widget with rich formatting
   pub struct LogView<'a> {
       logs: &'a VecDeque<LogEntry>,
       title: &'a str,
       show_timestamps: bool,
       show_source: bool,
       filter_state: Option<&'a FilterState>,
       search_state: Option<&'a SearchState>,
       collapse_state: Option<&'a crate::app::session::CollapseState>,
       default_collapsed: bool,
       max_collapsed_frames: usize,
       link_highlight_state: Option<&'a LinkHighlightState>,
   }

   impl<'a> LogView<'a> {
       // ... all methods from L275-1000
   }

   impl StatefulWidget for LogView<'_> {
       // ... implementation from L1002-1204
   }

   impl Widget for LogView<'_> {
       // ... implementation from L1207-1212
   }
   ```

2. **Update imports** to use the local modules:
   - Replace `use stack_trace_styles::*;` with `use styles::*;` in formatting methods
   - Or use `use crate::tui::widgets::log_view::styles::*;`

3. **Verify internal references**:
   - Methods that reference `LogViewState` should work via `use super::state::LogViewState`
   - Methods that reference `FocusInfo` should work similarly

### Methods in LogView impl to Migrate

| Method | Lines | Description |
|--------|-------|-------------|
| `new()` | L275-288 | Constructor |
| `title()` | L290-293 | Builder: set title |
| `show_timestamps()` | L295-298 | Builder: toggle timestamps |
| `show_source()` | L300-303 | Builder: toggle source |
| `filter_state()` | L306-309 | Builder: set filter |
| `search_state()` | L312-315 | Builder: set search |
| `collapse_state()` | L318-321 | Builder: set collapse state |
| `default_collapsed()` | L324-327 | Builder: set default collapsed |
| `max_collapsed_frames()` | L330-333 | Builder: set max frames |
| `link_highlight_state()` | L336-341 | Builder: set link state |
| `level_style()` | L344-365 | Static: get style for log level |
| `level_icon()` | L368-375 | Static: get icon for log level |
| `format_message()` | L378-391 | Format log message |
| `source_style()` | L394-402 | Static: get style for log source |
| `link_badge()` | L409-417 | Create link badge span |
| `link_text_style()` | L420-424 | Get link text style |
| `insert_link_badge_into_spans()` | L430-472 | Insert badge into span list |
| `format_entry()` | L475-523 | Format a log entry line |
| `format_message_with_highlights()` | L526-587 | Format with search highlights |
| `format_stack_frame()` | L591-649 | Format stack frame spans |
| `format_stack_frame_line()` | L653-655 | Format stack frame as Line |
| `format_stack_frame_line_with_links()` | L661-756 | Format with link badges |
| `format_collapsed_indicator()` | L759-778 | Format "N more frames..." line |
| `is_entry_expanded()` | L781-788 | Check if entry is expanded |
| `calculate_entry_lines()` | L791-807 | Calculate lines for entry |
| `render_empty()` | L810-838 | Render empty state |
| `build_title()` | L841-876 | Build title with filter info |
| `render_no_matches()` | L879-907 | Render no matches state |
| `line_width()` | L910-912 | Calculate line width |
| `apply_horizontal_scroll()` | L915-999 | Apply horizontal scrolling |

### File Structure After This Task

```
src/tui/widgets/log_view/
├── mod.rs       # UPDATED - Full widget implementation
├── state.rs     # LogViewState, FocusInfo
└── styles.rs    # Stack trace styling constants
```

### Acceptance Criteria

1. `mod.rs` contains the complete `LogView` struct and all implementations
2. All builder methods work correctly
3. `StatefulWidget` and `Widget` trait implementations are functional
4. Styles are accessed via the `styles` module
5. `cargo check` passes
6. `cargo build` succeeds

### Testing

Run the full test suite to verify widget functionality:

```bash
cargo test log_view
```

**Key tests to verify:**
- `test_format_entry_includes_timestamp`
- `test_format_entry_no_timestamp`
- `test_format_entry_no_source`
- `test_level_styles_are_distinct`
- `test_source_styles_are_distinct`
- `test_warning_has_bold_modifier`
- `test_error_has_bold_modifier`
- `test_build_title_*` (multiple tests)
- `test_filter_state_builder`
- `test_search_state_builder`
- `test_format_message_with_highlights_*` (multiple tests)
- `test_format_stack_frame_*` (multiple tests)
- `test_format_collapsed_indicator_*` (multiple tests)
- `test_calculate_entry_lines_*` (multiple tests)
- `test_is_entry_expanded_*` (multiple tests)
- `test_line_width`
- `test_apply_horizontal_scroll_*` (multiple tests)

### Notes

- The `#[allow(dead_code)]` annotations on `format_stack_frame` and `format_stack_frame_line` should be preserved - these are used in tests
- Some methods are `fn` (private) and some are `pub fn` - maintain the same visibility
- The `'a` lifetime on `LogView<'a>` must be preserved for all borrowed references
- Import `ParsedStackTrace` from `crate::core::stack_trace` if needed by methods

---

## Completion Summary

**Status:** Done

**Files:**
- `src/tui/widgets/log_view/mod.rs` (2036 lines) - Contains complete widget implementation:
  - `LogView<'a>` struct with all fields
  - `impl LogView` with all 22+ builder and formatting methods
  - `impl StatefulWidget for LogView` (render logic)
  - `impl Widget for LogView` (non-stateful wrapper)
  - Tests (will be extracted in Task 05)

**Notable decisions/tradeoffs:**
- The widget code was already in mod.rs from Task 01 (we moved everything at once instead of creating a skeleton)
- Tasks 02-03 extracted styles and state, leaving the widget implementation in place
- This task was effectively pre-completed by our efficient approach in Task 01
- The `#[allow(dead_code)]` annotations are preserved on test helper methods

**Verification:**
- `cargo check` - PASSED (no errors)
- `cargo test log_view` - PASSED (77/77 tests)
- All widget-related tests pass including:
  - Builder tests (`test_*_builder`)
  - Formatting tests (`test_format_*`)
  - Style tests (`test_*_style*`)
  - Scroll tests (`test_apply_horizontal_scroll_*`)

**Current file structure:**
```
src/tui/widgets/log_view/
├── mod.rs      # 2036 lines (widget + tests)
├── state.rs    # 199 lines
└── styles.rs   # 37 lines
```

**Risks/limitations:**
- mod.rs still contains ~1000 lines of tests that will be extracted in Task 05