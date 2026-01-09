# Bugfix Plan: Boolean Toggle Not Working in Settings Page

## TL;DR

Pressing Enter on a boolean setting in the settings page marks the setting as dirty but does not actually flip the boolean value (true → false or false → true). The `SettingsToggleBool` message handler in `update.rs` is incomplete—it recognizes boolean values but lacks the logic to toggle them.

---

## Bug Reports

### Bug 1: Boolean Settings Cannot Be Toggled

**Symptom:**
- User navigates to a boolean setting (e.g., "Auto Start")
- User presses Enter or Space to toggle the value
- The dirty indicator (*) appears, suggesting a change was made
- The actual value remains unchanged (still shows original true/false)
- Saving does not persist any change because no change actually occurred

**Expected Behavior:**
- Pressing Enter on a boolean setting should immediately flip the value
- `true` → `false` or `false` → `true`
- The displayed value should update immediately
- Dirty flag should only be set if value actually changed

**Root Cause Analysis:**

1. **Key binding works correctly:**
   - `src/app/handler/keys.rs:386` - Enter/Space on a setting triggers `Message::SettingsToggleEdit`

2. **Handler recognizes booleans but doesn't toggle:**
   - `src/app/handler/update.rs:1012-1046` - `SettingsToggleEdit` handler checks value type
   - Lines 1025-1028: Comment states "Bool toggles directly, Enum cycles" but...
   - No actual toggle logic is implemented

3. **Separate toggle message exists but is incomplete:**
   - `src/app/message.rs` - `Message::SettingsToggleBool` exists
   - `src/app/handler/update.rs:1102-1107` - Handler only marks `dirty = true`
   - Missing: Actual value flip and call to `apply_project_setting()` or `apply_user_preference()`

**Code Location:**

```rust
// src/app/handler/update.rs:1102-1107
Message::SettingsToggleBool => {
    if let UiMode::Settings(ref mut settings_state) = state.ui_mode {
        settings_state.dirty = true;  // Only this line exists
        // MISSING: Get current item, flip boolean, apply change
    }
    UpdateResult::none()
}
```

**Affected Files:**
- `src/app/handler/update.rs:1102-1107` - Primary fix location
- `src/app/handler/settings.rs` - Has `apply_project_setting()` helper ready to use
- `src/tui/widgets/settings_panel/mod.rs:890` - Has `get_selected_item()` to retrieve current item

---

## Affected Modules

| File | Changes Needed |
|------|----------------|
| `src/app/handler/update.rs` | Implement boolean toggle in `SettingsToggleBool` handler |
| `src/app/handler/settings.rs` | No changes needed - `apply_project_setting()` already handles booleans |
| `src/tui/widgets/settings_panel/mod.rs` | No changes needed - `get_selected_item()` already available |

---

## Fix Approach

### Phase 1: Implement Boolean Toggle

**Steps:**

1. **Get the currently selected item**
   - Access `SettingsPanel::get_selected_item()` with current state
   - This requires access to settings and project path

2. **Flip the boolean value**
   ```rust
   if let SettingValue::Bool(ref mut val) = item.value {
       *val = !*val;
   }
   ```

3. **Apply the change to settings struct**
   - Call `apply_project_setting()` for Project tab
   - Call `apply_user_preference()` for UserPrefs tab
   - Launch tab booleans need separate handling

4. **Mark dirty flag**
   - Only after successful toggle

5. **Return appropriate UpdateResult**

**Proposed Implementation:**

```rust
// src/app/handler/update.rs - Replace lines 1102-1107

Message::SettingsToggleBool => {
    if let UiMode::Settings(ref mut settings_state) = state.ui_mode {
        // Get the currently selected item
        if let Some(mut item) = SettingsPanel::get_selected_item(
            &state.settings,
            &state.settings_state.user_prefs,
            // ... other required params
        ) {
            // Toggle the boolean
            if let SettingValue::Bool(ref mut val) = item.value {
                *val = !*val;

                // Apply based on active tab
                match settings_state.active_tab {
                    SettingsTab::Project => {
                        apply_project_setting(&mut state.settings, &item);
                    }
                    SettingsTab::UserPrefs => {
                        apply_user_preference(&mut settings_state.user_prefs, &item);
                    }
                    SettingsTab::LaunchConfig => {
                        // Handle launch config booleans if any
                    }
                    SettingsTab::VSCodeConfig => {
                        // Read-only - should not reach here
                    }
                }

                settings_state.dirty = true;
            }
        }
    }
    UpdateResult::none()
}
```

**Measurable Outcomes:**
- Boolean settings can be toggled with Enter key
- Dirty indicator appears after toggle
- Value persists after save
- Unit tests pass

---

## Edge Cases & Risks

### Access to Settings Panel
- **Risk:** `SettingsPanel::get_selected_item()` may need restructuring to work from handler
- **Mitigation:** May need to store item key in state or refactor item retrieval

### VSCode Tab Booleans
- **Risk:** User somehow triggers toggle on read-only VSCode settings
- **Mitigation:** Check tab before toggling; ignore for VSCode tab

### Undo/Redo
- **Risk:** No undo mechanism for accidental toggles
- **Mitigation:** Document as limitation; user can toggle again

### Concurrent State
- **Risk:** State race if toggle happens during save
- **Mitigation:** Settings page is modal; no concurrent operations possible

---

## Testing

### Unit Tests to Add

```rust
// src/app/handler/tests.rs or update.rs test module

#[test]
fn test_settings_toggle_bool_flips_true_to_false() {
    let mut state = test_app_state();
    state.settings.behavior.auto_start = true;
    state.ui_mode = UiMode::Settings(SettingsViewState::new());
    // Position on auto_start setting

    let (new_state, _) = handler::update(state, Message::SettingsToggleBool);

    assert!(!new_state.settings.behavior.auto_start);
    if let UiMode::Settings(ref ss) = new_state.ui_mode {
        assert!(ss.dirty);
    }
}

#[test]
fn test_settings_toggle_bool_flips_false_to_true() {
    let mut state = test_app_state();
    state.settings.behavior.auto_start = false;
    state.ui_mode = UiMode::Settings(SettingsViewState::new());

    let (new_state, _) = handler::update(state, Message::SettingsToggleBool);

    assert!(new_state.settings.behavior.auto_start);
}

#[test]
fn test_settings_toggle_bool_ignored_for_vscode_tab() {
    // Should not toggle anything on VSCode tab (read-only)
}
```

### E2E Tests

See `workflow/plans/features/settings-page-testing/PLAN.md` Phase 2 for comprehensive E2E test approach.

---

## Task Dependency Graph

```
┌─────────────────────────────────────────┐
│  01-implement-boolean-toggle            │
│  (Primary fix in update.rs)             │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  02-add-unit-tests                      │
│  (Verify fix with unit tests)           │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  03-update-e2e-tests                    │
│  (Remove #[ignore] from toggle tests)   │
└─────────────────────────────────────────┘
```

---

## Success Criteria

### Bug Fixed When:
- [ ] Pressing Enter on boolean setting flips the value
- [ ] Displayed value updates immediately
- [ ] Dirty indicator appears after toggle
- [ ] Value persists to config file after save
- [ ] All boolean settings work: auto_start, auto_reload, devtools_auto_open, stack_trace_collapsed
- [ ] Unit tests pass for toggle behavior
- [ ] E2E tests (previously ignored) now pass

### No Regression When:
- [ ] Other setting types (string, number, enum, list) still work
- [ ] VSCode tab remains read-only
- [ ] Save functionality unchanged
- [ ] Edit mode for non-boolean types unaffected

---

## Milestone Deliverable

Boolean settings in the settings page can be toggled with Enter key. Users can enable/disable features like auto_start, auto_reload directly from the settings UI without manually editing config files.

---

## References

- [Settings Page Testing Plan](../../features/settings-page-testing/PLAN.md)
- [Log & Config Enhancements Plan](../../features/log-config-enhancements/PLAN.md) - Phase 4
- Source: `src/app/handler/update.rs:1102-1107`
- Helper: `src/app/handler/settings.rs` - `apply_project_setting()`, `apply_user_preference()`

---

**Document Version:** 1.0
**Created:** 2025-01-09
**Status:** Confirmed Bug - Ready for Implementation
