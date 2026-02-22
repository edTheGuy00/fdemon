## Task: Fix Hardcoded Settings Item Count

**Objective**: Replace hardcoded integer constants in `get_item_count_for_tab()` with dynamic computation from the actual item builder functions, making all settings items reachable via keyboard navigation.

**Depends on**: None

**Severity**: CRITICAL — 10 Project tab items permanently unreachable

### Scope

- `crates/fdemon-app/src/handler/settings_handlers.rs:348-368`: Rewrite `get_item_count_for_tab()`
- `crates/fdemon-app/src/handler/settings_handlers.rs:61,68`: Update call sites if signature changes
- `crates/fdemon-app/src/settings_items.rs`: Used as source of truth (read-only)

### Details

**Current state:** `get_item_count_for_tab()` returns hardcoded values:

| Tab | Hardcoded | Actual | Delta |
|-----|-----------|--------|-------|
| Project | 17 | 27 | -10 items unreachable |
| UserPrefs | 5 | 5 | OK |
| LaunchConfig | 10 | 7 * N (dynamic) | Wrong for any N |
| VSCodeConfig | 5 | 6 * N (dynamic) | Wrong for any N |

**Required changes:**

1. Change the function signature to accept `&AppState` (or at minimum `&Settings` + `&Path`) so dynamic tabs can load configs
2. For static tabs, call the item builder functions directly:
   - `SettingsTab::Project` → `project_settings_items(settings).len()`
   - `SettingsTab::UserPrefs` → `user_prefs_items(settings).len()`
3. For dynamic tabs, load configs and count:
   - `SettingsTab::LaunchConfig` → `launch_config_items(&load_launch_configs(project_path)).len()`
   - `SettingsTab::VSCodeConfig` → `vscode_config_items(&load_vscode_configs(project_path)).len()`
4. Update both call sites (`handle_settings_next_item:61`, `handle_settings_prev_item:68`) to pass the expanded arguments
5. Add regression tests (see Testing section)

**Important:** The `_settings` parameter (note leading underscore) is currently unused for the dynamic tabs. The function needs access to `project_path` to load configs. Both `handle_settings_next_item` and `handle_settings_prev_item` have access to the full `AppState`, so they can pass what's needed.

### Acceptance Criteria

1. All 27 Project tab items reachable by pressing j/Down repeatedly from the first item
2. LaunchConfig tab navigates the correct number of items based on loaded configs
3. VSCodeConfig tab navigates the correct number of items based on loaded configs
4. Regression tests pass asserting count == actual items for every tab type
5. `cargo test -p fdemon-app` passes
6. `cargo clippy -p fdemon-app` passes

### Testing

Add regression tests that prevent future drift:

```rust
#[test]
fn test_project_tab_count_matches_actual_items() {
    let settings = Settings::default();
    let count = get_item_count_for_tab(&settings, SettingsTab::Project);
    let items = crate::settings_items::project_settings_items(&settings);
    assert_eq!(count, items.len(), "Project tab count drifted from actual items");
}

#[test]
fn test_user_prefs_tab_count_matches_actual_items() {
    let settings = Settings::default();
    let count = get_item_count_for_tab(&settings, SettingsTab::UserPrefs);
    let items = crate::settings_items::user_prefs_items(&settings);
    assert_eq!(count, items.len(), "UserPrefs tab count drifted from actual items");
}
```

For the dynamic tabs, test with empty configs (0 items) and verify count is 0 rather than a hardcoded fallback.

### Notes

- The `settings_items` module generates items at render time too — calling the builders in the count function should be cheap (they return `Vec<SettingItem>`)
- If performance is a concern (unlikely), the count could be cached on `AppState` and invalidated on config reload
- The old stale comment `// behavior (2) + watcher (4) + ui (7) + devtools (2) + editor (2) = 17` should be removed entirely — dynamic computation makes it unnecessary

---

## Completion Summary

**Status:** Not Started
