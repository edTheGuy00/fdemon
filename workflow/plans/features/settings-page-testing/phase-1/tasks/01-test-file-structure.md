## Task: Create Test File Structure

**Objective**: Set up the test file structure for settings page E2E tests, including the main test file and module integration.

**Depends on**: None

### Scope

- `tests/e2e/mod.rs`: Add `mod settings_page;` declaration
- `tests/e2e/settings_page.rs`: **NEW** Main test file with imports and helper functions

### Details

Create the foundational test file with proper imports, helper functions, and module organization.

**File: `tests/e2e/settings_page.rs`**

```rust
//! E2E tests for the settings page functionality.
//!
//! Tests navigation, tab switching, item selection, and visual output
//! of the settings page accessible via the `,` key.

use std::time::Duration;
use serial_test::serial;

use crate::e2e::pty_utils::{FdemonSession, SpecialKey, TestFixture};

// Timing constants (use values from pty_utils or define locally)
const INIT_DELAY_MS: u64 = 500;
const INPUT_DELAY_MS: u64 = 200;
const SHORT_DELAY_MS: u64 = 50;

/// Helper: Open settings page and wait for it to appear
async fn open_settings(session: &mut FdemonSession) -> Result<(), Box<dyn std::error::Error>> {
    session.send_key(',')?;
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;
    session.expect("Settings")?;
    Ok(())
}

/// Helper: Navigate to a specific tab by number (1-4)
async fn goto_tab(session: &mut FdemonSession, tab_num: char) -> Result<(), Box<dyn std::error::Error>> {
    session.send_key(tab_num)?;
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;
    Ok(())
}

/// Helper: Navigate down N items
async fn navigate_down(session: &mut FdemonSession, count: usize) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..count {
        session.send_special(SpecialKey::ArrowDown)?;
        tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;
    }
    Ok(())
}

// ============================================================================
// Navigation Tests (Task 02)
// ============================================================================

// Tests will be added in task 02

// ============================================================================
// Tab Navigation Tests (Task 03)
// ============================================================================

// Tests will be added in task 03

// ============================================================================
// Item Navigation Tests (Task 04)
// ============================================================================

// Tests will be added in task 04

// ============================================================================
// Visual Output Tests (Task 05)
// ============================================================================

// Tests will be added in task 05
```

**Update: `tests/e2e/mod.rs`**

Add the module declaration:
```rust
pub mod settings_page;
```

### Acceptance Criteria

1. `tests/e2e/settings_page.rs` exists with proper module documentation
2. Helper functions `open_settings`, `goto_tab`, `navigate_down` implemented
3. Module registered in `tests/e2e/mod.rs`
4. File compiles without errors: `cargo check --test e2e`
5. Placeholder sections for subsequent tasks are in place

### Testing

```bash
# Verify compilation
cargo check --test e2e

# Verify module is recognized (should show 0 tests initially)
cargo test --test e2e settings_page -- --list
```

### Notes

- Use the same timing constants as other E2E tests for consistency
- Helper functions reduce code duplication in subsequent tasks
- Keep placeholder sections organized by task for easy navigation
- Consider adding a `close_settings` helper if needed

---

## Completion Summary

**Status:** Not Started
