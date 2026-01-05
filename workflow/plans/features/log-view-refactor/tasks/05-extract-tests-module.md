## Task: Extract Tests to Separate File

**Objective**: Move all unit tests from the inline `mod tests` block into a dedicated `tests.rs` file, following the pattern established in `src/app/handler/tests.rs`.

**Depends on**: `01-create-module-directory`, `02-extract-styles-module`, `03-extract-state-module`, `04-migrate-widget-implementation`

### Scope

- `src/tui/widgets/log_view.rs`: Lines 1215-2262 (~1047 lines of tests)
- `src/tui/widgets/log_view/tests.rs`: New file

### Implementation Details

1. **Create `src/tui/widgets/log_view/tests.rs`**:

   ```rust
   //! Tests for log_view widget module

   use super::*;
   use crate::app::session::CollapseState;
   use crate::core::{
       FilterState, LogEntry, LogLevel, LogLevelFilter, LogSource, LogSourceFilter,
       ParsedStackTrace, SearchState, StackFrame,
   };
   use ratatui::style::{Color, Modifier};
   use std::collections::VecDeque;

   // Import styles for constant tests
   use super::styles as stack_trace_styles;

   fn make_entry(level: LogLevel, source: LogSource, msg: &str) -> LogEntry {
       LogEntry::new(level, source, msg)
   }

   /// Helper to create a VecDeque of log entries for tests
   fn logs_from(entries: Vec<LogEntry>) -> VecDeque<LogEntry> {
       VecDeque::from(entries)
   }

   // ... all test functions from L1228-2262
   ```

2. **Update `mod.rs`** to include the test module conditionally:
   ```rust
   #[cfg(test)]
   mod tests;
   ```

3. **Update test imports** as needed:
   - `use super::*;` brings in `LogView`, `LogViewState`, `FocusInfo`
   - `use super::styles::*;` or `use super::styles as stack_trace_styles;` for style constants
   - External crate imports remain the same

### Test Categories to Migrate

| Category | Test Count | Lines | Description |
|----------|------------|-------|-------------|
| LogViewState basics | 6 | L1228-1292 | State creation, scrolling |
| Format entry | 4 | L1295-1326 | Entry formatting |
| Level/source styles | 4 | L1329-1355 | Style assertions |
| Filter tests | 8 | L1362-1476 | Filter state, filtering |
| Search highlighting | 8 | L1485-1638 | Search state, highlights |
| Stack frame formatting | 7 | L1647-1811 | Frame rendering |
| Collapsible traces | 10 | L1820-1978 | Collapse state |
| Horizontal scroll | 12 | L1985-2146 | H-scroll behavior |
| Visible range | 9 | L2153-2261 | Range calculations |

**Total: ~68 test functions**

### Helper Functions to Include

```rust
fn make_entry(level: LogLevel, source: LogSource, msg: &str) -> LogEntry {
    LogEntry::new(level, source, msg)
}

fn logs_from(entries: Vec<LogEntry>) -> VecDeque<LogEntry> {
    VecDeque::from(entries)
}
```

### File Structure After This Task

```
src/tui/widgets/log_view/
├── mod.rs       # Widget implementation + #[cfg(test)] mod tests;
├── state.rs     # LogViewState, FocusInfo
├── styles.rs    # Stack trace styling constants
└── tests.rs     # NEW - All unit tests (~1047 lines)
```

### Acceptance Criteria

1. `tests.rs` contains all ~68 test functions
2. `tests.rs` includes helper functions (`make_entry`, `logs_from`)
3. All imports are correctly updated for the new module location
4. `mod.rs` declares `#[cfg(test)] mod tests;`
5. `cargo test log_view` passes with all tests
6. Test count remains the same (no tests lost)

### Testing

Run the complete test suite for log_view:

```bash
# Run all log_view tests
cargo test log_view

# Run with verbose output to see all test names
cargo test log_view -- --nocapture

# Count tests to verify none were lost
cargo test log_view 2>&1 | grep -c "test result"
```

**Verify test categories pass:**

```bash
# State tests
cargo test log_view_state
cargo test scroll
cargo test visible_range

# Format tests
cargo test format_entry
cargo test format_message
cargo test format_stack_frame
cargo test format_collapsed

# Style tests
cargo test level_style
cargo test source_style
cargo test stack_frame_styles

# Filter tests
cargo test filter
cargo test build_title

# Search tests
cargo test highlights
cargo test search

# Collapse tests
cargo test collapse
cargo test is_entry_expanded
cargo test calculate_entry_lines

# Horizontal scroll tests
cargo test horizontal_scroll
cargo test scroll_left
cargo test scroll_right
cargo test apply_horizontal_scroll
cargo test line_width
```

### Import Reference

The tests need access to these items:

**From parent module (`use super::*`):**
- `LogView`
- `LogViewState`
- `FocusInfo`

**From styles module (`use super::styles`):**
- `FRAME_NUMBER`
- `FUNCTION_PROJECT`
- `FUNCTION_PACKAGE`
- `FILE_PROJECT`
- `FILE_PACKAGE`
- `LOCATION_PROJECT`
- `LOCATION_PACKAGE`
- `ASYNC_GAP`
- `PUNCTUATION`
- `INDENT`

**From crate:**
- `crate::core::{LogEntry, LogLevel, LogSource, FilterState, SearchState, ...}`
- `crate::core::{LogLevelFilter, LogSourceFilter}`
- `crate::core::{StackFrame, ParsedStackTrace}`
- `crate::app::session::CollapseState`

**From ratatui:**
- `ratatui::style::{Color, Modifier, Style}`

### Notes

- This is the largest migration task (~1047 lines)
- The pattern matches `src/app/handler/tests.rs` exactly
- Tests use `use super::*;` to access private items like `format_stack_frame`
- Some tests call static methods like `LogView::level_style()` - ensure these remain accessible
- The `#[cfg(test)]` attribute on `mod tests;` ensures tests are only compiled during `cargo test`
