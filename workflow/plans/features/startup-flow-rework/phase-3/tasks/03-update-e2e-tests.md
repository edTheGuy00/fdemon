## Task: Update E2E Tests

**Objective**: Update E2E test utilities and tests to work with the new startup flow where the app starts in Normal mode.

**Depends on**: Phase 1 and Phase 2 complete

### Scope

- `tests/e2e/pty_utils.rs`: Update helper functions
- `tests/e2e/settings_page.rs`: Remove workarounds, re-enable tests
- `tests/e2e/snapshots/`: E2E snapshots may need updates
- Other E2E test files as needed

### Details

**1. Update pty_utils.rs helpers:**

The `expect_header()` function checks for "Flutter Demon" text. It should still work, but we may want to add a new helper:

```rust
/// Wait for the app to reach Normal mode with "Not Connected" state
pub async fn expect_not_connected(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    self.expect("Not Connected").await?;
    self.expect("Press + to start a new session").await?;
    Ok(())
}
```

**2. Update settings_page.rs tests:**

Current workaround (from BUG.md):
```rust
// Old approach - had to escape from device selector first
session.expect_header().expect("header");
session.send_special(SpecialKey::Escape).expect("close device selector");
tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;
open_settings(&mut session).await.expect("open settings");
```

New approach:
```rust
// New approach - app starts in Normal mode directly
session.expect_header().expect("header");
// No need to escape from dialog!
open_settings(&mut session).await.expect("open settings");
```

**3. Re-enable ignored tests:**

Remove `#[ignore]` from settings page tests that were blocked by the startup dialog issue.

**4. Update E2E snapshots:**

Check `tests/e2e/snapshots/` for any files that contain "Waiting for Flutter...":
```bash
grep -l "Waiting for Flutter" tests/e2e/snapshots/
```

These may need regeneration.

**5. Verify test fixture configuration:**

Ensure `tests/fixtures/simple_app/.fdemon/config.toml` has `auto_start = false`:
```toml
[behavior]
auto_start = false
```

### Acceptance Criteria

1. E2E tests start and app is in Normal mode (not StartupDialog)
2. Settings page tests pass without workarounds
3. All previously-ignored tests can be re-enabled
4. E2E snapshots updated if needed
5. `cargo test --test e2e` passes (or `cargo nextest run --test e2e`)

### Testing

```bash
# Run E2E tests
cargo test --test e2e -- --nocapture

# Or with nextest for retry capability
cargo nextest run --test e2e

# Run specific test file
cargo test --test e2e settings_page -- --nocapture
```

### Notes

- E2E tests use PTY (pseudo-terminal) for realistic testing
- Tests are serial (`#[serial]` attribute) to avoid conflicts
- The startup flow change should significantly simplify E2E test setup
- Some tests may still need to interact with StartupDialog if they test session creation

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**
(To be filled after implementation)

**Testing Performed:**
- `cargo test --test e2e` - Pending
- Settings page tests - Pending
