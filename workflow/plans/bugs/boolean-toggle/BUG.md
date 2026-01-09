# Bugfix Plan: Boolean Toggle Not Working in Settings Page

## TL;DR

Pressing Enter on a boolean setting in the settings page marks the setting as dirty but does not flip the boolean value (true → false or false → true). The `SettingsToggleBool` message handler in `update.rs:1102-1107` is a stub—it only calls `mark_dirty()` without actually toggling the value. All infrastructure (item retrieval, apply functions) is ready; the handler just needs to be completed.

---

## Bug Report

### Bug: Boolean Settings Cannot Be Toggled

**Symptom:**
- User navigates to a boolean setting (e.g., "Auto Start", "Auto Reload")
- User presses Enter to toggle the value
- The dirty indicator (*) appears, suggesting a change was made
- The actual value remains unchanged (still shows original true/false)
- Saving does not persist any change because no change actually occurred

**Expected Behavior:**
- Pressing Enter on a boolean setting should immediately flip the value
- `true` → `false` or `false` → `true`
- The displayed value should update immediately
- Dirty flag should be set after toggle

**Root Cause Analysis:**

1. **Key binding dispatches correct message:**
   - `src/app/handler/keys.rs:398` - Enter on a boolean item triggers `Message::SettingsToggleEdit`
   - `src/app/handler/keys.rs:422-428` - In edit mode, Enter dispatches `Message::SettingsToggleBool`

2. **SettingsToggleEdit handler recognizes but doesn't toggle:**
   - `src/app/handler/update.rs:1012-1046` - Handler checks value type
   - Lines 1025-1028: Comment states "Bool toggles directly" but contains no toggle logic:
   ```rust
   SettingValue::Bool(_) | SettingValue::Enum { .. } => {
       // These don't use traditional edit mode
       // Bool toggles directly, Enum cycles
   }
   ```
   - This branch is effectively a no-op

3. **SettingsToggleBool handler is incomplete:**
   - `src/app/handler/update.rs:1102-1107`
   - Only marks dirty, does NOT flip the value:
   ```rust
   Message::SettingsToggleBool => {
       // Toggle boolean setting value
       // Actual implementation needs access to settings item
       state.settings_view_state.mark_dirty();
       UpdateResult::none()
   }
   ```

---

## Affected Modules

| File | Changes Needed |
|------|----------------|
| `src/app/handler/update.rs:1102-1107` | **PRIMARY FIX** - Implement toggle logic in `SettingsToggleBool` handler |
| `src/app/handler/update.rs:1012-1046` | **SECONDARY** - Fix `SettingsToggleEdit` to dispatch toggle for booleans |
| `src/app/handler/settings.rs` | No changes - `apply_project_setting()` already handles booleans |
| `src/tui/widgets/settings_panel/mod.rs` | No changes - `get_selected_item()` already available |

---

## Fix Approach

### Phase 1: Implement Boolean Toggle

The fix requires completing the `SettingsToggleBool` handler with this logic:

1. **Get the currently selected item** via `SettingsPanel::get_selected_item()`
2. **Flip the boolean value** in the item
3. **Apply the change** via the appropriate apply function based on active tab
4. **Mark dirty** after successful toggle

**Proposed Implementation:**

```rust
// src/app/handler/update.rs - Replace lines 1102-1107

Message::SettingsToggleBool => {
    use crate::config::{SettingValue, SettingsTab};
    use crate::tui::widgets::SettingsPanel;

    let panel = SettingsPanel::new(&state.settings, &state.project_path);
    if let Some(mut item) = panel.get_selected_item(&state.settings_view_state) {
        // Only toggle if it's a boolean value
        if let SettingValue::Bool(val) = &item.value {
            // Create new item with flipped value
            let new_value = SettingValue::Bool(!val);
            let mut toggled_item = item.clone();
            toggled_item.value = new_value;

            // Apply based on active tab
            match state.settings_view_state.active_tab {
                SettingsTab::Project => {
                    apply_project_setting(&mut state.settings, &toggled_item);
                    state.settings_view_state.mark_dirty();
                }
                SettingsTab::UserPrefs => {
                    apply_user_preference(
                        &mut state.settings_view_state.user_prefs,
                        &toggled_item
                    );
                    state.settings_view_state.mark_dirty();
                }
                SettingsTab::LaunchConfig => {
                    // Handle launch config booleans (auto_start)
                    if let Some(config) = state.settings_view_state
                        .launch_configs
                        .get_mut(state.settings_view_state.selected_launch_config)
                    {
                        apply_launch_config_change(config, &toggled_item);
                        state.settings_view_state.mark_dirty();
                    }
                }
                SettingsTab::VSCodeConfig => {
                    // Read-only tab - ignore toggle
                }
            }
        }
    }
    UpdateResult::none()
}
```

### Phase 2: Fix SettingsToggleEdit Flow (Optional Enhancement)

Currently, pressing Enter on a boolean enters a confusing state where nothing happens. The `SettingsToggleEdit` handler should dispatch `SettingsToggleBool` directly for boolean items:

```rust
// src/app/handler/update.rs - In SettingsToggleEdit handler (lines 1024-1028)
// Replace the no-op with direct toggle dispatch:

SettingValue::Bool(_) => {
    // Bool toggles directly without edit mode
    return update(state, Message::SettingsToggleBool);
}
SettingValue::Enum { .. } => {
    // Enums cycle through options
    return update(state, Message::SettingsCycleEnumNext);
}
```

---

## Boolean Settings Affected

These settings are boolean type and affected by this bug:

| ID | Label | Tab |
|----|-------|-----|
| `behavior.auto_start` | Auto Start | Project |
| `behavior.confirm_quit` | Confirm Quit | Project |
| `watcher.auto_reload` | Auto Reload | Project |
| `ui.show_timestamps` | Show Timestamps | Project |
| `ui.compact_logs` | Compact Logs | Project |
| `ui.stack_trace_collapsed` | Collapse Stack Traces | Project |
| `devtools.auto_open` | Auto Open DevTools | Project |
| `launch.X.auto_start` | Auto Start | Launch Config |

---

## Edge Cases & Risks

### Access Pattern
- **Risk:** Handler needs access to SettingsPanel which requires settings and project_path
- **Mitigation:** Both are available on `state`; pattern already used in `SettingsToggleEdit`

### VSCode Tab Protection
- **Risk:** User could somehow trigger toggle on read-only VSCode settings
- **Mitigation:** Check `active_tab` and ignore for `VSCodeConfig`

### Launch Config Booleans
- **Risk:** Launch config index might be out of bounds
- **Mitigation:** Use `.get_mut()` with bounds check before applying

### No Undo
- **Risk:** Accidental toggle has no undo
- **Mitigation:** Document as limitation; user can toggle again; changes not saved until explicit save

---

## Task Dependency Graph

```
┌─────────────────────────────────────────┐
│  01-implement-toggle-handler            │
│  (Complete SettingsToggleBool)          │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  02-fix-toggle-edit-dispatch            │
│  (SettingsToggleEdit dispatches toggle) │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  03-enable-e2e-tests                    │
│  (Remove #[ignore] from toggle tests)   │
└─────────────────────────────────────────┘
```

---

## Success Criteria

### Bug Fixed When:
- [ ] Pressing Enter on boolean setting flips the value
- [ ] Displayed value updates immediately after toggle
- [ ] Dirty indicator appears after toggle
- [ ] Value persists to config file after save
- [ ] All boolean settings work across all tabs (Project, Launch Config)
- [ ] Unit tests pass for toggle behavior
- [ ] E2E tests (previously ignored) now pass

### No Regression When:
- [ ] Other setting types (string, number, enum, list) still work
- [ ] VSCode tab remains read-only
- [ ] Save functionality unchanged
- [ ] Edit mode for non-boolean types unaffected

---

## Testing References

### Existing Tests (Currently Ignored)

**E2E Tests** (`tests/e2e/settings_page.rs`):
- `test_boolean_toggle_changes_value` - line ~768
- `test_toggle_auto_start`
- `test_toggle_auto_reload`
- `test_toggle_devtools_auto_open`
- `test_toggle_stack_trace_collapsed`

**Unit Tests** (`src/app/handler/tests.rs`):
- `test_settings_toggle_bool_flips_value` - line ~2108 (ignored, demonstrates bug)
- `test_settings_toggle_bool_sets_dirty_flag` - line ~2144 (passes, dirty flag works)

---

## Milestone Deliverable

Boolean settings in the settings page can be toggled with Enter key. Users can enable/disable features like auto_start, auto_reload directly from the settings UI without manually editing config files.

---

## References

- [Settings Page Testing Plan](../../features/settings-page-testing/PLAN.md)
- Source: `src/app/handler/update.rs:1102-1107`
- Helpers: `src/app/handler/settings.rs` - `apply_project_setting()`, `apply_user_preference()`, `apply_launch_config_change()`
- Item retrieval: `src/tui/widgets/settings_panel/mod.rs:890-915` - `get_selected_item()`

---

**Document Version:** 2.0
**Created:** 2025-01-09
**Updated:** 2025-01-09
**Status:** Confirmed Bug - Ready for Implementation
