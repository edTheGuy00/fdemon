## Task: Settings Item Navigation Tests

**Objective**: Test navigating between settings items using arrow keys and j/k vim-style keys.

**Depends on**: 01-test-file-structure

### Scope

- `tests/e2e/settings_page.rs`: Add item navigation tests in the Item Navigation Tests section

### Details

Implement E2E tests that verify navigating through settings items works correctly with arrow keys and j/k.

**Tests to Implement:**

```rust
// ============================================================================
// Item Navigation Tests
// ============================================================================

#[tokio::test]
#[serial]
async fn test_arrow_keys_navigate_settings() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // First item should be selected by default (Auto Start on Project tab)
    session.expect("Auto Start").expect("first item visible");

    // Arrow down should move selection
    session.send_special(SpecialKey::ArrowDown).expect("arrow down");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    // Arrow down again
    session.send_special(SpecialKey::ArrowDown).expect("arrow down");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    // Arrow up should move back
    session.send_special(SpecialKey::ArrowUp).expect("arrow up");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    // No crash, navigation works
    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_jk_keys_navigate_settings() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // 'j' should move down (vim style)
    session.send_key('j').expect("send j");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    session.send_key('j').expect("send j");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    // 'k' should move up (vim style)
    session.send_key('k').expect("send k");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    // No crash, navigation works
    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_selection_wraps_at_top_boundary() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // At first item, arrow up should wrap to last item (or stay at first)
    // Behavior depends on implementation - test that it doesn't crash
    session.send_special(SpecialKey::ArrowUp).expect("arrow up at top");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    // Should still be functional
    session.send_special(SpecialKey::ArrowDown).expect("arrow down");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_selection_wraps_at_bottom_boundary() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // Navigate down many times to reach the bottom
    for _ in 0..20 {
        session.send_key('j').expect("send j");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // At last item, arrow down should wrap to first (or stay at last)
    session.send_special(SpecialKey::ArrowDown).expect("arrow down at bottom");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    // Should still be functional
    session.send_special(SpecialKey::ArrowUp).expect("arrow up");
    tokio::time::sleep(Duration::from_millis(SHORT_DELAY_MS)).await;

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_page_up_down_navigation() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // Page Down should jump multiple items
    session.send_special(SpecialKey::PageDown).expect("page down");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // Page Up should jump back
    session.send_special(SpecialKey::PageUp).expect("page up");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // Should still be functional
    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_home_end_navigation() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // Navigate down a few items first
    navigate_down(&mut session, 3).await.expect("navigate down");

    // Home should go to first item
    session.send_special(SpecialKey::Home).expect("home");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // End should go to last item
    session.send_special(SpecialKey::End).expect("end");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_gg_G_vim_navigation() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // Navigate down first
    navigate_down(&mut session, 3).await.expect("navigate down");

    // 'g' twice should go to top (vim gg)
    session.send_key('g').expect("send g");
    tokio::time::sleep(Duration::from_millis(50)).await;
    session.send_key('g').expect("send g");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // 'G' should go to bottom (vim G)
    session.send_key('G').expect("send G");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    session.quit().expect("quit");
}
```

### Acceptance Criteria

1. `test_arrow_keys_navigate_settings` - Arrow Up/Down navigate between items
2. `test_jk_keys_navigate_settings` - j/k keys navigate (vim style)
3. `test_selection_wraps_at_top_boundary` - Arrow up at top doesn't crash
4. `test_selection_wraps_at_bottom_boundary` - Arrow down at bottom doesn't crash
5. `test_page_up_down_navigation` - Page Up/Down jump multiple items
6. `test_home_end_navigation` - Home/End go to first/last item
7. `test_gg_G_vim_navigation` - gg goes to top, G goes to bottom
8. All tests pass without panics or hangs

### Testing

```bash
# Run item navigation tests
cargo nextest run --test e2e test_arrow
cargo nextest run --test e2e test_jk
cargo nextest run --test e2e test_selection
cargo nextest run --test e2e test_page
cargo nextest run --test e2e test_home
cargo nextest run --test e2e test_gg

# Run all navigation tests
cargo nextest run --test e2e nav
```

### Notes

- Some navigation keys (PageUp, PageDown, Home, End, gg, G) may not be implemented
- If a key is not implemented, the test documents expected behavior
- Mark tests as `#[ignore = "Not implemented: ..."]` if feature is missing
- `SpecialKey` enum in `pty_utils.rs` may need Home, End, PageUp, PageDown added
- Item wrapping behavior may be configurable; test current behavior

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/settings_page.rs` | Added 7 item navigation tests in the Item Navigation Tests section |

### Notable Decisions/Tradeoffs

1. **SpecialKey enum already complete**: The `SpecialKey` enum in `pty_utils.rs` already had `Home`, `End`, `PageUp`, and `PageDown` variants implemented, so no changes were needed there.
2. **Snake case lint for test name**: Added `#[allow(non_snake_case)]` attribute to `test_gg_G_vim_navigation` since the name intentionally reflects the vim navigation keys (gg and G) and is more readable this way.
3. **Dead code attribute**: Added `#[allow(dead_code)]` to the `goto_tab` helper function since it's defined for use in Task 03.

### Testing Performed

- `cargo check --test e2e` - Passed (1 expected warning for unused `goto_tab` helper)
- `cargo fmt` - Applied formatting to match project style
- `cargo clippy --test e2e -- -D warnings` - Passed with no warnings

### Risks/Limitations

1. **E2E tests not run**: The tests compile successfully but were not executed because they require a built binary and a Flutter environment. The tests are marked with `#[tokio::test]` and `#[serial]` attributes for proper async execution and serialization.
2. **Feature implementation dependency**: These tests verify navigation behavior that may not be fully implemented yet in the settings page. Tests will pass if navigation doesn't crash, even if the feature isn't complete.
3. **Timing-dependent**: Tests use sleep delays (SHORT_DELAY_MS, INPUT_DELAY_MS) which may need adjustment based on system performance and CI environment.
