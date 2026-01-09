## Task: Create Tests for Individual Boolean Settings

**Objective**: Add comprehensive E2E tests for each boolean setting in the application, ensuring all toggleable options are covered by tests.

**Depends on**: Task 03 (bug report must exist for `#[ignore]` references)

### Scope

- `tests/e2e/settings_page.rs`: Add individual tests for each boolean setting

### Details

Create E2E tests for each boolean setting in the configuration. Each test should:
1. Navigate to the specific setting
2. Attempt to toggle it
3. Verify the expected behavior (will fail due to bug)

Known boolean settings to test:
- `auto_start` - Auto-start Flutter on launch
- `auto_reload` - Hot reload on file save
- `devtools_auto_open` - Auto-open DevTools
- `stack_trace_collapsed` - Collapse stack traces by default

```rust
#[tokio::test]
#[serial]
#[ignore = "BUG: Boolean toggle not implemented - see workflow/plans/bugs/boolean-toggle/BUG.md"]
async fn test_toggle_auto_start() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn");

    // App starts directly in Normal mode (startup flow rework)
    session.expect("Not Connected").expect("startup complete");

    // Open settings - no dialog to dismiss!
    session.send_key(',').expect("send comma");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Navigate to auto_start setting in Project tab
    // (may need multiple j/k presses depending on order)

    // Press Enter to toggle
    session.send_key('\r').expect("send enter");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify toggle occurred
    // EXPECTED: Value changes
    // ACTUAL: Value unchanged (bug)

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
#[ignore = "BUG: Boolean toggle not implemented - see workflow/plans/bugs/boolean-toggle/BUG.md"]
async fn test_toggle_auto_reload() {
    // Similar structure for auto_reload setting
}

#[tokio::test]
#[serial]
#[ignore = "BUG: Boolean toggle not implemented - see workflow/plans/bugs/boolean-toggle/BUG.md"]
async fn test_toggle_devtools_auto_open() {
    // Similar structure for devtools setting
}

#[tokio::test]
#[serial]
#[ignore = "BUG: Boolean toggle not implemented - see workflow/plans/bugs/boolean-toggle/BUG.md"]
async fn test_toggle_stack_trace_collapsed() {
    // Similar structure for stack trace setting
}
```

### Helper Function

Consider creating a helper to reduce duplication:

```rust
async fn test_toggle_boolean_setting(setting_name: &str, tab_index: usize, item_index: usize) {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn");

    // App starts directly in Normal mode (startup flow rework)
    session.expect("Not Connected").expect("startup complete");

    // Open settings - no dialog to dismiss!
    session.send_key(',').expect("send comma");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Navigate to correct tab
    for _ in 0..tab_index {
        session.send_key('\t').expect("tab");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Navigate to correct item
    for _ in 0..item_index {
        session.send_key('j').expect("down");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Toggle and verify
    session.send_key('\r').expect("enter");
    tokio::time::sleep(Duration::from_millis(200)).await;

    session.quit().expect("quit");
}
```

### Acceptance Criteria

1. Tests exist for each boolean setting:
   - `test_toggle_auto_start`
   - `test_toggle_auto_reload`
   - `test_toggle_devtools_auto_open`
   - `test_toggle_stack_trace_collapsed`
2. All tests are marked `#[ignore]` with bug report reference
3. Tests are organized with clear navigation to each setting
4. Helper function exists if beneficial for reducing duplication

### Testing

```bash
# List all boolean toggle tests
cargo test test_toggle_ -- --list

# Run all (should all be ignored)
cargo test test_toggle_

# Run ignored tests to see failures
cargo test test_toggle_ -- --ignored
```

### Notes

- Verify the exact location of each boolean setting in the UI
- Settings may be in different tabs (Project, Watcher, UI, etc.)
- Navigation indices may vary—verify with manual testing first
- Once the bug is fixed, remove `#[ignore]` from all these tests

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/settings_page.rs` | Added 4 boolean setting toggle tests and 1 helper function (150+ lines) |

### Notable Decisions/Tradeoffs

1. **Helper Function Pattern**: Created `test_toggle_boolean_setting()` helper to eliminate duplication across the 4 tests. Each test calls this helper with the setting name and navigation index.

2. **Navigation Strategy**: Used down arrow count (0-based index) to navigate to each setting. Indices verified from `src/tui/widgets/settings_panel/items.rs`:
   - `auto_start` (index 0) - Behavior section
   - `auto_reload` (index 4) - Watcher section
   - `stack_trace_collapsed` (index 9) - UI section
   - `devtools_auto_open` (index 12) - DevTools section

3. **Verification Approach**: Tests capture screen content before and after toggle, verify:
   - Correct setting is selected
   - Initial boolean value is identified (true/false)
   - Value should flip after Enter press (this is the bug)
   - Dirty indicator should appear

4. **Test Documentation**: Each test includes doc comments with location information for future reference.

### Testing Performed

- `cargo check` - Passed
- `cargo test test_toggle_ -- --list` - All 4 tests listed correctly
- `cargo test test_toggle_auto` - Confirmed properly ignored
- `cargo test test_toggle_devtools` - Confirmed properly ignored
- `cargo test test_toggle_stack` - Confirmed properly ignored
- `cargo test --lib` - Passed (1329 tests)
- `cargo test --test e2e settings_page::` - 22 passed, 4 ignored (my tests), 1 pre-existing failure
- `cargo clippy -- -D warnings` - Passed
- `cargo fmt -- --check` - Passed

### Risks/Limitations

1. **Navigation Indices**: If settings items are reordered in the future, these navigation indices will need updating. Documented in test comments for maintainability.

2. **String Matching**: Tests rely on string matching for boolean values ("true"/"false"). If display format changes (e.g., to checkboxes or icons), tests will need adjustment.

3. **Pre-existing E2E Instability**: One pre-existing settings test fails (`test_readonly_items_have_lock_icon`). This is not related to the changes in this task.
