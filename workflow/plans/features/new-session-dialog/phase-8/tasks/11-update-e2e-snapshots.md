## Task: Update E2E Snapshots

**Objective**: Update E2E snapshot tests to reflect the new NewSessionDialog UI.

**Depends on**: 10-fix-render-tests

**Estimated Time**: 20 minutes

**Priority**: 🟠 Major

**Source**: Risks/Tradeoffs Analyzer

### Scope

- `tests/e2e/`: Update snapshot baselines

### Problem

E2E snapshot tests validate that the TUI renders correctly in real terminal conditions. The startup screen and dialog flows have changed, but snapshots weren't updated.

### Process

1. Run E2E tests to identify failures:
   ```bash
   cargo test --test e2e
   ```

2. Review each failing snapshot:
   - Compare old vs new output
   - Verify new output is correct

3. Accept valid changes:
   ```bash
   cargo insta review
   ```

4. For tests that reference deleted behaviors:
   - Update test logic to use new dialog
   - Or remove test if behavior no longer exists

### Potentially Affected Snapshots

Based on review, check these snapshot files in `tests/e2e/snapshots/`:

- `startup_screen.snap` - Initial app state
- `quit_confirmation.snap` - Dialog rendering
- Any `device_selector_*.snap` files - Should be removed or renamed

### Acceptance Criteria

1. `cargo test --test e2e` passes
2. All E2E snapshots reflect NewSessionDialog UI
3. No snapshots reference DeviceSelector or StartupDialog
4. Manual review confirms UI looks correct

### Testing

```bash
# Run all E2E tests
cargo test --test e2e

# Review snapshot changes
cargo insta review

# Re-run to confirm
cargo test --test e2e
```

### Notes

- E2E tests run against a real PTY, so output may vary slightly by terminal
- Review each snapshot change carefully - don't blindly accept
- If a snapshot looks wrong, fix the underlying code rather than accepting bad output

---

## Completion Summary

**Status:** Not Started
