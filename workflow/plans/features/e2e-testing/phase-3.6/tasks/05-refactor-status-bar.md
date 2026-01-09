## Task: Refactor status_bar.rs to Directory Module

**Objective**: Extract the 700+ lines of tests from `status_bar.rs` into a separate `tests.rs` file to comply with the 500-line guideline.

**Depends on**: Wave 2 complete (especially 04-migrate-widget-tests)

### Scope

- `src/tui/widgets/status_bar.rs` → `src/tui/widgets/status_bar/mod.rs`
- **NEW** `src/tui/widgets/status_bar/tests.rs`
- `src/tui/widgets/mod.rs` - Update imports

### Details

**Current structure:**
```
src/tui/widgets/
├── status_bar.rs           # 1031 lines (widget + tests)
└── ...
```

**Target structure:**
```
src/tui/widgets/
├── status_bar/
│   ├── mod.rs              # ~331 lines (widget code only)
│   └── tests.rs            # ~700 lines (all tests)
└── ...
```

**Step-by-step:**

1. **Create directory:**
   ```bash
   mkdir -p src/tui/widgets/status_bar
   ```

2. **Move widget code to mod.rs:**
   - Lines 1-331 (widget implementation)
   - Keep all `use` statements
   - Add `#[cfg(test)] mod tests;` at the end

3. **Create tests.rs:**
   - Move lines 332-1031 (test module)
   - Remove the `#[cfg(test)]` wrapper (file-level tests.rs is implicitly test-only)
   - Add `use super::*;` at top
   - Update imports: `use crate::tui::test_utils::{test_device, TestTerminal};`

4. **Update widgets/mod.rs:**
   ```rust
   // Change from:
   mod status_bar;
   // To:
   mod status_bar;
   pub use status_bar::*;  // If needed for re-exports
   ```

**Example mod.rs structure:**
```rust
//! Status bar widget for displaying app state.

use ratatui::{...};
// ... other imports ...

/// Renders the status bar at the bottom of the screen.
pub fn render_status_bar(...) {
    // ... implementation ...
}

// ... other widget code ...

#[cfg(test)]
mod tests;
```

**Example tests.rs structure:**
```rust
use super::*;
use crate::app::state::AppState;
use crate::tui::test_utils::{test_device, TestTerminal};
// ... other test imports ...

#[test]
fn test_status_bar_displays_phase() {
    // ...
}

// ... all other tests ...
```

### Acceptance Criteria

1. `status_bar/mod.rs` contains only widget code (< 400 lines)
2. `status_bar/tests.rs` contains all test code
3. All status_bar tests pass
4. No public API changes
5. `cargo clippy` passes

### Testing

```bash
# Run status_bar tests
cargo test --lib status_bar

# Verify module compiles
cargo check

# Check for lint issues
cargo clippy -- -D warnings
```

### Notes

- Follow existing pattern from `log_view/` and `settings_panel/`
- Preserve all test functionality
- This is a pure refactoring - no test logic changes

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/status_bar.rs` | Deleted (old monolithic file with 1030 lines) |
| `src/tui/widgets/status_bar/mod.rs` | Created - 333 lines (widget implementation only) |
| `src/tui/widgets/status_bar/tests.rs` | Created - 696 lines (all 34 tests extracted) |

### Notable Decisions/Tradeoffs

1. **Module Structure**: Followed the existing pattern from `log_view/` directory - `mod.rs` declares `#[cfg(test)] mod tests;` at the end, and `tests.rs` contains all test code with `use super::*;` at the top.
2. **Import Preservation**: All imports from the original file were preserved exactly as they were, ensuring no functionality changes.
3. **No widgets/mod.rs Changes**: The `mod status_bar;` declaration in `widgets/mod.rs` works unchanged when converting from a file to a directory module - no modifications needed.

### Testing Performed

- `cargo test --lib status_bar` - PASSED (34/34 tests)
- `cargo check` - PASSED
- `cargo clippy -- -D warnings` - PASSED (no warnings)

### Risks/Limitations

None identified. This is a pure refactoring with no logic changes:
- All tests pass (34/34)
- Public API unchanged
- Line count reduced from 1030 to 333 in mod.rs (67.7% reduction)
- No clippy warnings introduced
