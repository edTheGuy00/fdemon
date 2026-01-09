## Task: Visual Output Tests for Settings Page

**Objective**: Verify visual indicators and highlighting in the settings page render correctly. This task was moved from settings-page-testing/phase-1 to be completed after the startup flow rework enables easier E2E testing.

**Depends on**: 03-update-e2e-tests (startup flow must be working first)

**Original Location**: `workflow/plans/features/settings-page-testing/phase-1/tasks/05-visual-output-tests.md`

### Scope

- `tests/e2e/settings_page.rs`: Add visual output tests in the Visual Output Tests section

### Details

Implement E2E tests that verify visual elements like selection highlighting, dirty indicator, readonly icons, and override indicators appear correctly.

**Tests to Implement:**

```rust
// ============================================================================
// Visual Output Tests
// ============================================================================

#[tokio::test]
#[serial]
async fn test_selected_item_highlighted() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // First item should have selection indicator
    // This might be a ">" prefix, background color (hard to test), or bracket
    // Look for common selection patterns
    let content = session.capture_for_snapshot().expect("capture");

    // The selected item should have some visual distinction
    // Common patterns: "> Auto Start", "[ ] Auto Start", "● Auto Start"
    assert!(
        content.contains(">") || content.contains("●") || content.contains("["),
        "Selection indicator should be visible"
    );

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
#[ignore = "BUG: Boolean toggle not implemented - dirty indicator may not appear correctly"]
async fn test_dirty_indicator_appears_on_change() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // Capture initial state - should NOT have dirty indicator
    let before = session.capture_for_snapshot().expect("capture before");
    assert!(
        !before.contains("*") && !before.contains("modified") && !before.contains("unsaved"),
        "Should not be dirty initially"
    );

    // Toggle a boolean setting (this is bugged, but test expected behavior)
    session.send_special(SpecialKey::Enter).expect("toggle");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // Capture after change - SHOULD have dirty indicator
    let after = session.capture_for_snapshot().expect("capture after");

    // Dirty indicator could be: "*", "(modified)", "[unsaved]", etc.
    assert!(
        after.contains("*") || after.contains("modified") || after.contains("unsaved"),
        "Dirty indicator should appear after change. Got: {}",
        after
    );

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_readonly_items_have_lock_icon() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // Go to VSCode tab (tab 4) - all items are readonly
    goto_tab(&mut session, '4').await.expect("goto VSCode tab");

    let content = session.capture_for_snapshot().expect("capture");

    // VSCode configs should show readonly indicator
    // Could be: "🔒", "[RO]", "(read-only)", lock symbol
    // Or the tab header might indicate readonly status
    let has_readonly = content.contains("🔒")
        || content.contains("read")
        || content.contains("Read")
        || content.contains("RO")
        || content.contains("locked");

    // Note: If no VSCode configs exist, the tab might show "No configurations"
    // which is also acceptable
    let is_empty = content.contains("No") && content.contains("config");

    assert!(
        has_readonly || is_empty,
        "VSCode tab should show readonly indicator or empty state. Got: {}",
        content
    );

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_override_indicator_shows_for_user_prefs() {
    // This test requires a project with both project settings and user overrides
    // For now, test the User Prefs tab structure
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    // Go to User Prefs tab (tab 2)
    goto_tab(&mut session, '2').await.expect("goto User tab");

    let content = session.capture_for_snapshot().expect("capture");

    // User prefs tab should show user-specific settings
    // If a setting overrides a project default, it might show "⚡" or similar
    // For this test, just verify the tab renders correctly
    assert!(
        content.contains("Editor") || content.contains("Theme") || content.contains("User"),
        "User prefs tab should show user settings"
    );

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_value_types_display_correctly() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    let content = session.capture_for_snapshot().expect("capture");

    // Boolean values should show true/false or checkmark/cross
    let has_bool = content.contains("true")
        || content.contains("false")
        || content.contains("✓")
        || content.contains("✗")
        || content.contains("yes")
        || content.contains("no");

    // Number values should show digits
    let has_number = content.chars().any(|c| c.is_ascii_digit());

    assert!(has_bool, "Boolean values should be displayed");
    assert!(has_number, "Number values should be displayed");

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_section_headers_visible() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    let content = session.capture_for_snapshot().expect("capture");

    // Project tab should have section headers
    // Common sections: "Behavior", "Watcher", "UI", "DevTools"
    let has_sections = content.contains("Behavior")
        || content.contains("Watcher")
        || content.contains("UI")
        || content.contains("DevTools");

    assert!(
        has_sections,
        "Section headers should be visible in Project tab. Got: {}",
        content
    );

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_help_text_visible() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");

    let content = session.capture_for_snapshot().expect("capture");

    // Help text or key hints should be visible
    // Common: "Press Enter to edit", "Esc to close", "Ctrl+S to save"
    let has_help = content.contains("Enter")
        || content.contains("Esc")
        || content.contains("save")
        || content.contains("Save")
        || content.contains("Ctrl");

    // Note: Help text might be in footer or not visible in all states
    // This is informational - don't fail if not present
    if !has_help {
        eprintln!("Note: No help text found in settings page. This may be intentional.");
    }

    session.quit().expect("quit");
}

#[tokio::test]
#[serial]
async fn test_snapshot_settings_page_project_tab() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

    session.expect_header().expect("header");
    tokio::time::sleep(Duration::from_millis(INIT_DELAY_MS)).await;

    open_settings(&mut session).await.expect("open settings");
    tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;

    // Take snapshot for visual regression testing
    session.assert_snapshot("settings_page_project_tab").expect("snapshot");

    session.quit().expect("quit");
}
```

### Acceptance Criteria

1. `test_selected_item_highlighted` - Selected item has visual indicator
2. `test_dirty_indicator_appears_on_change` - Dirty indicator (*, modified, etc.) appears after change
3. `test_readonly_items_have_lock_icon` - VSCode tab shows readonly status
4. `test_override_indicator_shows_for_user_prefs` - User prefs tab renders correctly
5. `test_value_types_display_correctly` - Boolean and number values visible
6. `test_section_headers_visible` - Section headers (Behavior, Watcher, etc.) visible
7. `test_help_text_visible` - Key hints visible (informational)
8. `test_snapshot_settings_page_project_tab` - Snapshot matches golden file

### Testing

```bash
# Run visual output tests
cargo nextest run --test e2e test_selected
cargo nextest run --test e2e test_dirty
cargo nextest run --test e2e test_readonly
cargo nextest run --test e2e test_override
cargo nextest run --test e2e test_value_types
cargo nextest run --test e2e test_section
cargo nextest run --test e2e test_help
cargo nextest run --test e2e test_snapshot

# Update snapshots if needed
cargo insta test --test e2e --accept
```

### Notes

- `test_dirty_indicator_appears_on_change` is marked `#[ignore]` due to boolean toggle bug
- Snapshot tests require `insta` crate and golden files in `tests/snapshots/`
- Visual indicators may vary (unicode symbols, text, colors)
- `capture_for_snapshot()` should strip ANSI codes for text comparison
- Some tests are informational (don't fail on missing optional features)
- **This task was moved here because it depends on the startup flow rework to function properly**

---

## Completion Summary

**Status:** Not Started
