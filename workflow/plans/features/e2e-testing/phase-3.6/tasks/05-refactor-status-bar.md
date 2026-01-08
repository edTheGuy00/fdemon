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

**Status:** ❌ Not done

**Files Modified:**
- (pending)

**Testing Performed:**
- (pending)
