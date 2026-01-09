# Plan: Settings Page & Configuration E2E Testing

## TL;DR

Comprehensive E2E test suite for the settings page and configuration system. Tests config file creation, settings persistence, boolean toggle functionality (known bug), and different project types (VSCode vs FDemon configs). Primary goal is **bug detection**, not test passage—tests should expose issues, and bugs should be reported rather than masked.

---

## Background

### Current State

The settings page was implemented as part of the Log & Config Enhancements feature (Phase 4). While unit tests exist in `src/tui/widgets/settings_panel/tests.rs` (40+ tests), there are **no E2E tests** that verify:

1. End-to-end user workflows (open settings → navigate → edit → save)
2. Config file creation when `.fdemon/` doesn't exist
3. Config file updates when settings change
4. Different project configurations (VSCode-only, FDemon-only, mixed)
5. Boolean toggle functionality (confirmed bug: Enter key doesn't toggle)

### Startup Flow (Simplified Testing)

The [startup flow rework](../startup-flow-rework/PLAN.md) has been completed, which **significantly simplifies E2E testing**:

- **Before**: App started in `StartupDialog` mode, tests needed workarounds to escape
- **After**: App starts directly in `UiMode::Normal` with "Not Connected" state

This means tests can immediately interact with the TUI (open settings, navigate, etc.) without needing to dismiss dialogs or wait for Flutter device discovery.

### Known Bugs

**Bug #1: Boolean Toggle Does Nothing**
- **Location:** `src/app/handler/update.rs:1102-1107`
- **Symptom:** Pressing Enter on a boolean setting marks dirty but doesn't flip the value
- **Root Cause:** `SettingsToggleBool` handler only sets `dirty = true` without actually toggling the boolean
- **Impact:** Critical UX issue - users cannot change boolean settings

### Testing Philosophy

> **Goal: Catch bugs, not make tests pass.**

If a test reveals a bug:
1. Document the bug in the test file with `// BUG:` comment
2. Create a bug report in `workflow/plans/bugs/`
3. Mark the test as `#[ignore]` with reason until bug is fixed
4. Do NOT modify the test to pass around the bug

---

## Affected Modules

### Test Files (NEW)

- `tests/e2e/settings_page.rs` - **NEW** Settings page E2E tests
- `tests/e2e/config_files.rs` - **NEW** Config file lifecycle tests
- `tests/e2e/project_types.rs` - **NEW** Different project configuration tests
- `tests/fixtures/vscode_only_app/` - **NEW** App with only .vscode/launch.json
- `tests/fixtures/fdemon_only_app/` - **NEW** App with only .fdemon/ config
- `tests/fixtures/mixed_config_app/` - **NEW** App with both config sources
- `tests/fixtures/no_config_app/` - **NEW** App with no configuration

### Source Files Under Test

- `src/tui/widgets/settings_panel/` - Settings panel widget
- `src/config/settings.rs` - Settings loading/saving
- `src/config/launch.rs` - Launch config management
- `src/config/vscode.rs` - VSCode config parsing
- `src/app/handler/update.rs` - Message handlers (boolean toggle bug)
- `src/app/handler/settings.rs` - Settings application helpers

---

## Development Phases

### Phase 1: Settings Page E2E Tests

**Goal**: Verify end-to-end settings page workflows using PTY-based testing.

#### Steps

1. **Create Test File Structure**
   - Create `tests/e2e/settings_page.rs`
   - Add module to `tests/e2e/mod.rs`
   - Import required utilities from `pty_utils.rs`

2. **Settings Page Navigation Tests**
   ```rust
   // Tests to implement:
   - test_settings_opens_on_comma_key
   - test_settings_closes_on_escape
   - test_settings_closes_on_q_key
   - test_settings_shows_all_four_tabs
   ```

3. **Tab Navigation Tests**
   ```rust
   // Tests to implement:
   - test_tab_switching_with_number_keys
   - test_tab_switching_with_tab_key
   - test_tab_wrapping_at_boundaries
   - test_vscode_tab_shows_readonly_indicator
   ```

4. **Settings Item Navigation Tests**
   ```rust
   // Tests to implement:
   - test_arrow_keys_navigate_settings
   - test_jk_keys_navigate_settings
   - test_selection_wraps_at_boundaries
   - test_selection_resets_on_tab_change
   ```

5. **Visual Output Verification**
   ```rust
   // Tests to implement:
   - test_selected_item_highlighted
   - test_dirty_indicator_appears_on_change
   - test_readonly_items_have_lock_icon
   - test_override_indicator_shows_for_user_prefs
   ```

**Milestone**: Settings page navigation and visual output verified end-to-end.

---

### Phase 2: Boolean Toggle Bug Verification

**Goal**: Create tests that expose the boolean toggle bug and document it properly.

#### Steps

1. **Create Boolean Toggle E2E Tests**
   ```rust
   #[tokio::test]
   #[serial]
   #[ignore = "BUG: Boolean toggle not implemented - see update.rs:1102"]
   async fn test_boolean_toggle_changes_value() {
       // This test SHOULD pass but currently FAILS
       // Pressing Enter on a boolean setting should flip true <-> false
       // Currently it only marks dirty without changing the value
   }
   ```

2. **Create Unit Tests for Boolean Toggle**
   - Add tests in `src/app/handler/update.rs` test module
   - Verify `SettingsToggleBool` message handling
   - Document expected vs actual behavior

3. **Create Bug Report**
   - Create `workflow/plans/bugs/boolean-toggle/BUG.md`
   - Document symptom, root cause, affected files
   - Include fix approach for implementer

4. **Test Different Boolean Settings**
   ```rust
   // Tests for each boolean setting:
   - test_toggle_auto_start
   - test_toggle_auto_reload
   - test_toggle_devtools_auto_open
   - test_toggle_stack_trace_collapsed
   ```

**Milestone**: Boolean toggle bug is documented with failing tests; bug report created.

---

### Phase 3: Config File Lifecycle Tests

**Goal**: Test config file creation, modification, and persistence.

#### Steps

1. **Create Test Fixtures**
   - `tests/fixtures/no_config_app/` - Flutter app with no .fdemon or .vscode
   - Minimal `pubspec.yaml` and `lib/main.dart`

2. **Config Directory Creation Tests**
   ```rust
   // Tests to implement:
   - test_fdemon_dir_created_on_first_save
   - test_config_toml_created_with_defaults
   - test_settings_local_added_to_gitignore
   - test_launch_toml_created_when_saving_launch_config
   ```

3. **Config File Update Tests**
   ```rust
   // Tests to implement:
   - test_setting_change_persists_to_file
   - test_atomic_write_pattern (temp file + rename)
   - test_existing_config_values_preserved
   - test_multiple_changes_before_save
   ```

4. **Config File Integrity Tests**
   ```rust
   // Tests to implement:
   - test_invalid_config_handled_gracefully
   - test_partial_config_merged_with_defaults
   - test_config_reload_after_external_edit
   ```

5. **User Preferences Tests**
   ```rust
   // Tests to implement:
   - test_user_prefs_saved_to_settings_local
   - test_user_prefs_override_project_settings
   - test_user_prefs_gitignored
   ```

**Milestone**: Config file creation and updates work correctly; files persist as expected.

---

### Phase 4: Project Type Configuration Tests

**Goal**: Test settings page behavior with different project configurations.

#### Steps

1. **Create Test Fixtures**
   - `tests/fixtures/vscode_only_app/` - Has .vscode/launch.json only
   - `tests/fixtures/fdemon_only_app/` - Has .fdemon/config.toml only
   - `tests/fixtures/mixed_config_app/` - Has both config sources

2. **VSCode-Only Project Tests**
   ```rust
   // Tests to implement:
   - test_vscode_configs_displayed_readonly
   - test_vscode_tab_shows_lock_icon
   - test_cannot_edit_vscode_settings
   - test_project_tab_shows_defaults_without_fdemon
   ```

3. **FDemon-Only Project Tests**
   ```rust
   // Tests to implement:
   - test_fdemon_configs_fully_editable
   - test_vscode_tab_shows_empty_message
   - test_launch_configs_editable
   ```

4. **Mixed Configuration Tests**
   ```rust
   // Tests to implement:
   - test_both_config_sources_displayed
   - test_fdemon_launch_configs_editable
   - test_vscode_launch_configs_readonly
   - test_priority_indicator_shown
   ```

5. **No Configuration Tests**
   ```rust
   // Tests to implement:
   - test_empty_project_shows_defaults
   - test_can_create_new_config
   - test_first_save_creates_fdemon_dir
   ```

**Milestone**: All project types handled correctly; mixed configs prioritized properly.

---

### Phase 5: Settings Edit Mode Tests

**Goal**: Test all setting value types and edit workflows.

#### Steps

1. **String Value Edit Tests**
   ```rust
   // Tests to implement:
   - test_enter_starts_string_edit
   - test_typing_updates_edit_buffer
   - test_escape_cancels_edit
   - test_enter_confirms_edit
   - test_backspace_deletes_character
   ```

2. **Number Value Edit Tests**
   ```rust
   // Tests to implement:
   - test_plus_minus_increment_decrement
   - test_direct_number_input
   - test_invalid_number_rejected
   - test_number_bounds_enforced
   ```

3. **Enum Value Cycle Tests**
   ```rust
   // Tests to implement:
   - test_enter_cycles_enum_forward
   - test_enum_wraps_at_end
   - test_enum_options_displayed
   ```

4. **List Value Edit Tests**
   ```rust
   // Tests to implement:
   - test_enter_on_list_expands
   - test_add_item_to_list
   - test_remove_item_from_list
   - test_empty_list_handled
   ```

5. **Dirty State Management Tests**
   ```rust
   // Tests to implement:
   - test_change_sets_dirty_flag
   - test_save_clears_dirty_flag
   - test_escape_without_save_warns
   - test_multiple_changes_single_dirty
   ```

**Milestone**: All setting types editable; dirty state managed correctly.

---

## Edge Cases & Risks

### Test Flakiness
- **Risk:** PTY-based tests are timing-sensitive
- **Mitigation:** Use consistent delays from `pty_utils.rs`; add `#[serial]` attribute; use retry via nextest

### File System Race Conditions
- **Risk:** Config file tests may race with file watchers
- **Mitigation:** Use `tempdir()` for isolation; pause watcher during tests

### Platform Differences
- **Risk:** PTY behavior differs across macOS/Linux
- **Mitigation:** Focus on macOS (primary dev platform); document Linux differences

### Mock vs Real Behavior
- **Risk:** Unit tests pass but E2E fails due to integration issues
- **Mitigation:** Prioritize E2E tests; use real file system in fixtures

### Known Bug Impact
- **Risk:** Boolean toggle bug causes multiple test failures
- **Mitigation:** Mark affected tests with `#[ignore]`; create clear bug report

---

## Test File Structure

```
tests/
├── e2e/
│   ├── mod.rs                   # Add new test modules
│   ├── settings_page.rs         # NEW - Settings page navigation/visual tests
│   ├── config_files.rs          # NEW - Config file lifecycle tests
│   └── project_types.rs         # NEW - Different project config tests
└── fixtures/
    ├── simple_app/              # Existing - basic Flutter app
    ├── vscode_only_app/         # NEW - only .vscode/launch.json
    │   ├── pubspec.yaml
    │   ├── lib/main.dart
    │   └── .vscode/
    │       └── launch.json
    ├── fdemon_only_app/         # NEW - only .fdemon/ config
    │   ├── pubspec.yaml
    │   ├── lib/main.dart
    │   └── .fdemon/
    │       ├── config.toml
    │       └── launch.toml
    ├── mixed_config_app/        # NEW - both config sources
    │   ├── pubspec.yaml
    │   ├── lib/main.dart
    │   ├── .fdemon/
    │   │   └── config.toml
    │   └── .vscode/
    │       └── launch.json
    └── no_config_app/           # NEW - no configuration
        ├── pubspec.yaml
        └── lib/main.dart

workflow/plans/bugs/
└── boolean-toggle/              # NEW - Bug report for toggle issue
    └── BUG.md
```

---

## Test Patterns Reference

### E2E Test Pattern (PTY)

With the startup flow rework complete, tests start directly in Normal mode with "Not Connected" state. No need to escape dialogs.

```rust
#[tokio::test]
#[serial]
async fn test_settings_page_opens() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn");

    // App starts directly in Normal mode - wait for "Not Connected" state
    session.expect("Not Connected").expect("startup complete");

    // Open settings immediately - no dialog to dismiss!
    session.send_key(',').expect("send comma");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify settings page
    session.expect("Settings").expect("settings visible");
    session.expect("Project").expect("project tab visible");

    session.quit().expect("quit");
}
```

### Bug-Exposing Test Pattern
```rust
#[tokio::test]
#[serial]
#[ignore = "BUG: Boolean toggle not implemented - see workflow/plans/bugs/boolean-toggle/BUG.md"]
async fn test_boolean_toggle_actually_changes_value() {
    // This test documents expected behavior
    // It is ignored until the bug is fixed
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn");

    // App starts in Normal mode - ready immediately
    session.expect("Not Connected").expect("startup complete");

    // Open settings and navigate to boolean setting
    session.send_key(',').expect("send comma");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Navigate to a boolean setting (e.g., auto_reload)
    // Press Enter to toggle...

    // EXPECTED: Value should flip from true to false
    // ACTUAL: Value remains unchanged (dirty flag set but no toggle)

    session.quit().expect("quit");
}
```

### Config File Test Pattern
```rust
#[tokio::test]
#[serial]
async fn test_config_file_created_on_save() {
    let temp = tempdir().expect("tempdir");
    let project_path = temp.path().join("test_app");

    // Copy fixture without .fdemon/
    copy_fixture("no_config_app", &project_path);

    // Verify no .fdemon exists
    assert!(!project_path.join(".fdemon").exists());

    // Spawn fdemon - starts directly in Normal mode
    let mut session = FdemonSession::spawn(&project_path).expect("spawn");
    session.expect("Not Connected").expect("startup complete");

    // Open settings, make change, save
    session.send_key(',').expect("open settings");
    tokio::time::sleep(Duration::from_millis(200)).await;
    // ... navigate and edit ...
    session.send_key('s').expect("save"); // Ctrl+S or 's' depending on mode

    // Verify .fdemon created
    assert!(project_path.join(".fdemon").exists());
    assert!(project_path.join(".fdemon/config.toml").exists());

    session.quit().expect("quit");
}
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Settings page opens/closes via keyboard shortcuts
- [ ] All four tabs navigable and display correct content
- [ ] Item navigation works with arrow/j/k keys
- [ ] Visual indicators (selection, dirty, readonly) appear correctly
- [ ] 10+ E2E tests passing

### Phase 2 Complete When:
- [ ] Boolean toggle bug documented with failing tests
- [ ] Bug report created in `workflow/plans/bugs/boolean-toggle/`
- [ ] Tests marked `#[ignore]` with clear reason
- [ ] Unit tests added for `SettingsToggleBool` handler

### Phase 3 Complete When:
- [ ] `.fdemon/` directory created on first save
- [ ] Config files persist changes correctly
- [ ] Atomic write pattern verified
- [ ] User preferences properly gitignored
- [ ] 8+ config lifecycle tests passing

### Phase 4 Complete When:
- [ ] VSCode-only projects show readonly configs
- [ ] FDemon-only projects fully editable
- [ ] Mixed config projects show both with priority
- [ ] No-config projects can create new config
- [ ] 4 test fixtures created
- [ ] 12+ project type tests passing

### Phase 5 Complete When:
- [ ] All value types (bool, number, string, enum, list) editable
- [ ] Edit mode enter/exit works correctly
- [ ] Dirty state managed properly
- [ ] 15+ edit mode tests passing

---

## Known Issues to Test

| Issue | Test File | Expected Behavior | Actual Behavior |
|-------|-----------|-------------------|-----------------|
| Boolean toggle | `settings_page.rs` | Enter flips true↔false | Only marks dirty |
| Enum cycling | `settings_page.rs` | TBD - verify works | TBD |
| List editing | `settings_page.rs` | TBD - verify works | TBD |

---

## Dependencies

### Existing
- `expectrl` - PTY interaction
- `tokio` - Async runtime
- `serial_test` - Test serialization
- `tempfile` - Temporary directories
- `insta` - Snapshot testing (optional)

### No New Dependencies Required

All testing infrastructure is already in place from E2E testing phases 1-3.

---

## References

- [E2E Testing Plan](../e2e-testing/PLAN.md) - Phases 1-3 completed
- [Startup Flow Rework](../startup-flow-rework/PLAN.md) - Enables simpler E2E testing (completed)
- [Log & Config Enhancements](../log-config-enhancements/PLAN.md) - Settings UI in Phase 4
- [PTY Utilities](../../../../tests/e2e/pty_utils.rs) - `FdemonSession`, `TestFixture`
- [Settings Panel Tests](../../../../src/tui/widgets/settings_panel/tests.rs) - Existing unit tests
- [Update Handler](../../../../src/app/handler/update.rs) - Boolean toggle bug location

---

**Document Version:** 1.1
**Created:** 2025-01-09
**Updated:** 2025-01-09 - Updated for startup flow rework (simpler testing)
**Status:** Draft - Awaiting Approval
