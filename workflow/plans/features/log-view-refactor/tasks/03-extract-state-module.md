## Task: Extract State Module (LogViewState & FocusInfo)

**Objective**: Extract `LogViewState` and `FocusInfo` structs with their implementations into a dedicated `state.rs` file.

**Depends on**: `01-create-module-directory`

### Scope

- `src/tui/widgets/log_view.rs`: Lines 61-252
  - `const DEFAULT_BUFFER_LINES` (L61)
  - `FocusInfo` struct + impl (L73-87)
  - `LogViewState` struct + impl (L95-252)
- `src/tui/widgets/log_view/state.rs`: New file

### Implementation Details

1. **Create `src/tui/widgets/log_view/state.rs`**:

   ```rust
   //! State management for log view scrolling and focus tracking

   use std::collections::VecDeque;
   use crate::core::LogEntry;

   /// Default buffer lines for virtualized rendering
   const DEFAULT_BUFFER_LINES: usize = 10;

   // ─────────────────────────────────────────────────────────────────────────────
   // FocusInfo
   // ─────────────────────────────────────────────────────────────────────────────

   /// Information about the currently focused element in the log view.
   ///
   /// Updated during render to track which log entry and optional stack frame
   /// is at the "focus" position (top of visible area).
   #[derive(Debug, Default, Clone)]
   pub struct FocusInfo {
       /// Index of the focused entry in the log buffer
       pub entry_index: Option<usize>,
       /// ID of the focused entry (for stability across buffer changes)
       pub entry_id: Option<u64>,
       /// Index of the focused frame within a stack trace (if applicable)
       pub frame_index: Option<usize>,
   }

   impl FocusInfo {
       /// Create a new empty focus info
       pub fn new() -> Self {
           Self::default()
       }
   }

   // ─────────────────────────────────────────────────────────────────────────────
   // LogViewState
   // ─────────────────────────────────────────────────────────────────────────────

   /// State for log view scrolling with virtualization support
   #[derive(Debug)]
   pub struct LogViewState {
       /// Current vertical scroll offset from top
       pub offset: usize,
       /// Current horizontal scroll offset from left
       pub h_offset: usize,
       /// Whether auto-scroll is enabled (follow new content)
       pub auto_scroll: bool,
       /// Total number of lines in content
       pub total_lines: usize,
       /// Number of visible lines in viewport
       pub visible_lines: usize,
       /// Maximum line width in content
       pub max_line_width: usize,
       /// Visible width of the viewport
       pub visible_width: usize,
       /// Number of lines to render above/below visible area
       pub buffer_lines: usize,
       /// Information about the focused element
       pub focus_info: FocusInfo,
   }

   impl Default for LogViewState {
       fn default() -> Self {
           Self::new()
       }
   }

   impl LogViewState {
       // ... all methods from L123-252 of original file
       // Including: new, visible_range, set_buffer_lines, scroll_up, scroll_down,
       // scroll_to_top, scroll_to_bottom, page_up, page_down, update_content_size,
       // scroll_left, scroll_right, scroll_to_line_start, scroll_to_line_end,
       // update_horizontal_size, calculate_total_lines, calculate_total_lines_filtered
   }
   ```

2. **Update `mod.rs`** to declare and re-export:
   ```rust
   mod state;
   
   pub use state::{FocusInfo, LogViewState};
   ```

3. **Update imports** in the widget code:
   - The widget will use `use super::state::LogViewState;` or rely on re-exports

### Methods to Include in LogViewState

| Method | Lines | Description |
|--------|-------|-------------|
| `new()` | L123-135 | Constructor with defaults |
| `visible_range()` | L141-145 | Calculate visible line range with buffer |
| `set_buffer_lines()` | L148-150 | Configure buffer size |
| `scroll_up()` | L153-156 | Scroll up by n lines |
| `scroll_down()` | L159-167 | Scroll down by n lines |
| `scroll_to_top()` | L170-173 | Jump to top |
| `scroll_to_bottom()` | L176-179 | Jump to bottom, enable auto-scroll |
| `page_up()` | L182-185 | Scroll up by page |
| `page_down()` | L188-191 | Scroll down by page |
| `update_content_size()` | L194-202 | Update dimensions, handle auto-scroll |
| `scroll_left()` | L205-207 | Horizontal scroll left |
| `scroll_right()` | L210-213 | Horizontal scroll right |
| `scroll_to_line_start()` | L216-218 | Jump to line start |
| `scroll_to_line_end()` | L221-224 | Jump to line end |
| `update_horizontal_size()` | L227-236 | Update horizontal dimensions |
| `calculate_total_lines()` | L239-243 | Static method for line count |
| `calculate_total_lines_filtered()` | L246-251 | Static method for filtered line count |

### File Structure After This Task

```
src/tui/widgets/log_view/
├── mod.rs
├── state.rs     # NEW - LogViewState, FocusInfo
└── styles.rs
```

### Acceptance Criteria

1. `state.rs` contains `FocusInfo` and `LogViewState` with all methods
2. `DEFAULT_BUFFER_LINES` constant is in `state.rs` (not public)
3. Both types are re-exported from `mod.rs`
4. `cargo check` passes
5. All state-related tests pass (see Testing section)

### Testing

Run specific tests to verify state functionality:

```bash
cargo test log_view_state
cargo test scroll
cargo test visible_range
cargo test buffer_lines
cargo test horizontal_scroll
```

**Tests that should pass:**
- `test_log_view_state_default`
- `test_scroll_up_disables_auto_scroll`
- `test_scroll_to_bottom_enables_auto_scroll`
- `test_scroll_up_at_top`
- `test_update_content_size_auto_scrolls`
- `test_page_up_down`
- `test_horizontal_scroll_state_default`
- `test_scroll_left`
- `test_scroll_right`
- `test_scroll_to_line_start`
- `test_scroll_to_line_end`
- `test_no_horizontal_scroll_needed`
- `test_update_horizontal_size`
- `test_update_horizontal_size_clamps_offset`
- `test_visible_range_*` (multiple tests)
- `test_buffer_lines_*` (multiple tests)
- `test_calculate_total_lines_*` (multiple tests)

### Notes

- `calculate_total_lines` and `calculate_total_lines_filtered` are static methods that take `&VecDeque<LogEntry>` - they need the `crate::core::LogEntry` import
- `FocusInfo` is a simple data struct with no complex logic
- The constant `DEFAULT_BUFFER_LINES` should remain private to the module