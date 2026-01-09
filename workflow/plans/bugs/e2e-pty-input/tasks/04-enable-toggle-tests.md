## Task: Enable Toggle Tests

**Objective**: Remove `#[ignore]` attributes from toggle tests now that the PTY issue is fixed.

**Depends on**: 03-implement-fix

### Scope

- `tests/e2e/settings_page.rs` - Remove `#[ignore]` from toggle tests

### Details

After the PTY fix is implemented, the previously ignored toggle tests should now pass. Enable them and verify.

### Tests to Enable

1. `test_toggle_auto_start`
2. `test_toggle_auto_reload`
3. `test_toggle_devtools_auto_open`
4. `test_toggle_stack_trace_collapsed`
5. `test_dirty_indicator_appears_on_change`

### Steps

1. **Remove `#[ignore]` attributes**:

   Find and remove these lines:
   ```rust
   #[ignore = "E2E PTY issue: Enter/Space keys not triggering toggle..."]
   ```

2. **Run the toggle tests**:
   ```bash
   cargo test test_toggle_ --test e2e
   cargo test test_dirty_indicator --test e2e
   ```

3. **Verify all pass**

4. **Run full E2E suite**:
   ```bash
   cargo test --test e2e
   ```

5. **Run full verification**:
   ```bash
   cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
   ```

### Acceptance Criteria

1. All `#[ignore]` attributes removed from toggle tests
2. `test_toggle_auto_start` passes
3. `test_toggle_auto_reload` passes
4. `test_toggle_devtools_auto_open` passes
5. `test_toggle_stack_trace_collapsed` passes
6. `test_dirty_indicator_appears_on_change` passes
7. No regressions in other tests
8. All quality gates pass

### Notes

- If any test still fails, investigate before enabling
- The helper function `test_toggle_boolean_setting` may need updates based on the fix
- Consider updating test documentation to reflect the fix

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Test Results:**

(To be filled after enabling tests)
