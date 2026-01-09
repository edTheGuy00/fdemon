## Task: Settings Page Navigation Tests

**Objective**: Test opening and closing the settings page via keyboard shortcuts.

**Depends on**: 01-test-file-structure

### Scope

- `tests/e2e/settings_page.rs`: Add navigation tests in the Navigation Tests section

### Details

Implement E2E tests that verify the settings page can be opened and closed using the expected keyboard shortcuts.

**Tests to Implement:**

```rust
// ============================================================================
// Navigation Tests
// ============================================================================

#[tokio::test]
#[serial]
async fn test_settings_opens_on_comma_key() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    // Wait for app to initialize
    session.expect_header().expect("header should appear");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    // Press comma to open settings
    session.send_key(',').expect("send comma key");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // Verify settings page appears
    session.expect("Settings").expect("settings title should appear");

    session.quit().expect("quit gracefully");
}

#[tokio::test]
#[serial]
async fn test_settings_closes_on_escape() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    // Open settings
    open_settings(&mut session).await.expect("open settings");

    // Press Escape to close
    session.send_special(SpecialKey::Escape).expect("send escape");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // Verify we're back to main view (settings title should be gone)
    // The header should still be visible
    session.expect_header().expect("back to main view");

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_settings_closes_on_q_key() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    // Open settings
    open_settings(&mut session).await.expect("open settings");

    // Press 'q' to close
    session.send_key('q').expect("send q key");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // Verify back to main view
    session.expect_header().expect("back to main view");

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_settings_shows_all_four_tabs() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    // Open settings
    open_settings(&mut session).await.expect("open settings");

    // Verify all four tabs are visible
    session.expect("Project").expect("Project tab");
    session.expect("User").expect("User tab");
    session.expect("Launch").expect("Launch tab");
    session.expect("VSCode").expect("VSCode tab");

    session.quit().expect("quit");
}
```

### Acceptance Criteria

1. `test_settings_opens_on_comma_key` - Settings page opens when `,` is pressed
2. `test_settings_closes_on_escape` - Settings page closes when `Escape` is pressed
3. `test_settings_closes_on_q_key` - Settings page closes when `q` is pressed
4. `test_settings_shows_all_four_tabs` - All four tab names visible in header
5. All tests pass: `cargo nextest run --test e2e settings_page`

### Testing

```bash
# Run all navigation tests
cargo nextest run --test e2e test_settings_opens
cargo nextest run --test e2e test_settings_closes
cargo nextest run --test e2e test_settings_shows

# Run with output for debugging
cargo test --test e2e test_settings_opens -- --nocapture
```

### Notes

- If a test fails due to a bug (not test issue), mark with `#[ignore]` and document
- Use `expect_header()` to verify return to main view after closing settings
- The `,` key binding is in `src/app/handler/keys.rs`
- Settings page rendering is in `src/tui/widgets/settings_panel/`

---

## Completion Summary

**Status:** Not Started
