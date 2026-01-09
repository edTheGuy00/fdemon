## Task: Verify All Tests Pass

**Objective**: Run full verification to ensure all changes work correctly and no regressions were introduced.

**Depends on**: 01-update-keybindings-doc, 02-update-snapshot-tests, 03-update-e2e-tests

### Scope

- All test suites
- Linting and formatting
- Manual verification

### Details

**1. Full verification script:**

```bash
# Format check
cargo fmt --check

# Compilation
cargo check

# Linting
cargo clippy -- -D warnings

# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# E2E tests (with retry if using nextest)
./scripts/test-e2e.sh
# Or: cargo nextest run --test e2e
```

**2. Manual verification checklist:**

Run the app and verify each scenario:

```bash
cargo run -- tests/fixtures/simple_app
```

- [ ] App starts in Normal mode (not StartupDialog)
- [ ] Status bar shows "○ Not Connected"
- [ ] Log area shows "Not Connected" and "Press + to start a new session"
- [ ] Pressing '+' opens StartupDialog
- [ ] Pressing 'd' opens StartupDialog (same as '+')
- [ ] Pressing 'n' does nothing (no search active)
- [ ] Pressing '/' enters search mode
- [ ] After typing search and pressing Enter, 'n' finds next match
- [ ] Pressing ',' opens Settings panel
- [ ] From StartupDialog, selecting a device starts session
- [ ] After session starts, status shows appropriate state
- [ ] Pressing '+' with running session opens DeviceSelector

**3. Verify with auto_start enabled:**

Create a temp config with auto_start = true and verify that flow still works:
- App should show Loading screen
- Should attempt to discover devices
- Should launch session or fall back to dialog

### Acceptance Criteria

1. `cargo fmt --check` passes
2. `cargo check` passes with no errors
3. `cargo clippy -- -D warnings` passes with no warnings
4. `cargo test --lib` passes (all unit tests)
5. `cargo test --test '*'` passes (integration tests)
6. E2E tests pass (settings page tests unblocked)
7. Manual verification confirms expected behavior
8. No regressions in existing functionality

### Testing

**Automated:**
```bash
# Full verification in one command
cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
```

**E2E specific:**
```bash
cargo test --test e2e -- --nocapture
```

### Notes

- If any tests fail, investigate before marking complete
- Document any known limitations or deferred issues
- Consider running tests multiple times to catch flaky tests
- Check CI/CD if configured to ensure pipeline will pass

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (Summary of all files modified across all phases)

**Implementation Details:**
(Summary of key implementation decisions)

**Testing Performed:**
- `cargo fmt --check` - Pending
- `cargo check` - Pending
- `cargo clippy -- -D warnings` - Pending
- `cargo test --lib` - Pending
- `cargo test --test e2e` - Pending
- Manual verification - Pending

**Risks/Limitations:**
- (Any known issues or future work needed)
