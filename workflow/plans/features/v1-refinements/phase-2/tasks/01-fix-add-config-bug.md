## Task: Fix "Add New Configuration" Navigation Bug

**Objective**: Make the "Add New Configuration" button in the LaunchConfig settings tab navigable and functional. Currently, `get_item_count_for_tab()` excludes it from the navigation range, and no handler dispatches `LaunchConfigCreate` when the button is selected.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/handler/settings_handlers.rs`: Fix `get_item_count_for_tab()` item count for LaunchConfig tab
- `crates/fdemon-app/src/settings_items.rs`: Handle add-new index in `get_selected_item()`
- `crates/fdemon-app/src/handler/settings_handlers.rs`: Wire `handle_settings_toggle_edit()` to dispatch `LaunchConfigCreate`

### Details

#### Bug 1: Item count excludes "Add New Configuration" row

`get_item_count_for_tab()` at `settings_handlers.rs:362-369` calculates the LaunchConfig tab count as `sum(launch_config_items(config, idx).len())` — which is `7 * N` for N configs. The "Add New Configuration" button is rendered at index `N*7` in the TUI (`settings_panel/mod.rs:789-794`), but navigation wraps at `N*7 - 1` because the count doesn't include it.

**Fix:** Add `+ 1` to the LaunchConfig item count **when configs exist** (the button is only rendered when there are configs — when empty, an empty state is shown instead). Check the TUI rendering logic at `settings_panel/mod.rs:726-795`: the button is only rendered inside the `if !configs.is_empty()` path.

```rust
// settings_handlers.rs — get_item_count_for_tab(), LaunchConfig branch
SettingsTab::LaunchConfig => {
    let configs = load_launch_configs(&state.project_path);
    let item_count: usize = configs.iter().enumerate()
        .map(|(idx, resolved)| launch_config_items(&resolved.config, idx).len())
        .sum();
    if item_count > 0 {
        item_count + 1  // +1 for "Add New Configuration" button
    } else {
        0
    }
}
```

#### Bug 2: `get_selected_item()` returns `None` for add-new index

`get_selected_item()` at `settings_items.rs:27-54` builds a flat `Vec<SettingItem>` and calls `.get(selected_index)`. When `selected_index == all_items.len()` (the add-new slot), `.get()` returns `None` and the entire toggle-edit action is silently skipped.

**Fix:** After building the items list for LaunchConfig, check if `selected_index == all_items.len()` and the tab is LaunchConfig. Return a sentinel `SettingItem` with a special ID:

```rust
// settings_items.rs — get_selected_item(), after building items
if view_state.active_tab == SettingsTab::LaunchConfig
    && view_state.selected_index == items.len()
    && !items.is_empty()
{
    return Some(SettingItem::new("launch.__add_new__", "Add New Configuration")
        .value(SettingValue::Bool(false))  // placeholder, won't be used
        .section("Actions".to_string()));
}
items.get(view_state.selected_index).cloned()
```

#### Bug 3: No dispatch to `LaunchConfigCreate` from settings toggle

`handle_settings_toggle_edit()` at `settings_handlers.rs:74-112` pattern-matches on `SettingValue` variants. There's no branch for the add-new sentinel.

**Fix:** Add an early-return check in `handle_settings_toggle_edit()` before the `SettingValue` match:

```rust
// settings_handlers.rs — handle_settings_toggle_edit(), before SettingValue match
if item.id == "launch.__add_new__" {
    return update(state, Message::LaunchConfigCreate);
}
```

`Message::LaunchConfigCreate` already exists (`message.rs:365`) and has a working handler (`update.rs:710-726`) that calls `create_default_launch_config()` → `add_launch_config()` → `mark_dirty()`.

### Acceptance Criteria

1. Navigation in the LaunchConfig tab reaches the "Add New Configuration" row (index `7*N`)
2. The add-new row is visually highlighted when selected (the TUI already renders `render_add_config_option` with `is_selected` check at `mod.rs:792`)
3. Pressing Enter on the add-new row creates a new launch configuration on disk (via `LaunchConfigCreate`)
4. Navigation wraps correctly: pressing Down on add-new goes to index 0; pressing Up on index 0 goes to add-new
5. When no configs exist (empty state), the item count is 0 and no add-new row is in the nav range
6. `cargo test -p fdemon-app` passes with all new and existing tests

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_launch_config_item_count_includes_add_new_button() {
        let temp = tempdir().unwrap();
        let mut state = AppState::new();
        state.project_path = temp.path().to_path_buf();
        state.settings_view_state.active_tab = SettingsTab::LaunchConfig;
        // Create a launch config file
        fdemon_app::config::launch::init_launch_file(temp.path()).unwrap();
        let count = get_item_count_for_tab(&state);
        // 7 items per config + 1 for add-new button
        assert_eq!(count, 8); // 7 items for 1 default config + 1 add button
    }

    #[test]
    fn test_launch_config_item_count_zero_when_no_configs() {
        let state = state_with_tab(SettingsTab::LaunchConfig);
        assert_eq!(get_item_count_for_tab(&state), 0);
    }

    #[test]
    fn test_get_selected_item_returns_add_new_sentinel() {
        // When selected_index == items.len() on LaunchConfig tab,
        // get_selected_item should return the add-new sentinel
    }

    #[test]
    fn test_toggle_edit_on_add_new_dispatches_launch_config_create() {
        // When handle_settings_toggle_edit is called with the add-new
        // sentinel selected, it should return Message::LaunchConfigCreate
    }
}
```

### Notes

- The sentinel ID `"launch.__add_new__"` uses double underscores to avoid collision with real config field IDs (which use the pattern `"launch.{idx}.{field}"`)
- `LaunchConfigCreate` handler at `update.rs:710-726` sets `settings_view_state.error` on failure — this is already correct behavior
- The TUI rendering at `settings_panel/mod.rs:884-897` already handles the visual rendering of the add-new button — no TUI changes needed for this task
- Watch out for the `load_launch_configs()` disk read in both `get_item_count_for_tab()` and `get_selected_item()` — they should return consistent results since both read from the same file

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/settings_handlers.rs` | Fixed `get_item_count_for_tab()` to add `+1` for the "Add New Configuration" button when configs exist; added early-return sentinel check in `handle_settings_toggle_edit()` to dispatch `LaunchConfigCreate`; added 4 new tests |
| `crates/fdemon-app/src/settings_items.rs` | Fixed `get_selected_item()` to return the `"launch.__add_new__"` sentinel `SettingItem` when `selected_index == items.len()` on the LaunchConfig tab with non-empty configs |
| `crates/fdemon-app/src/state.rs` | Fixed pre-existing import: changed `crate::new_session_dialog::state::{...}` to `crate::new_session_dialog::{...}` to resolve private module access compile error |

### Notable Decisions/Tradeoffs

1. **Redundant condition removed**: The task spec included `view_state.active_tab == SettingsTab::LaunchConfig` inside the `LaunchConfig` match arm of `get_selected_item()`. This check is always true inside that arm, so it was removed to keep the code clean.

2. **state.rs import fix**: A pre-existing compile error in `state.rs` used `crate::new_session_dialog::state::{DartDefinesModalState, FuzzyModalState}` where `state` is a private submodule. Fixed to use the public re-export path `crate::new_session_dialog::{...}`. This was blocking the compile and not part of the task spec, but necessary for compilation.

3. **Test setup uses `init_launch_file`**: The tests that require configs to exist use `init_launch_file` (which creates one default "Debug" config with 7 items), confirming the item count of 8 (7 + 1 add-new button).

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app settings_handlers` - Passed (9 tests, including 4 new)
- `cargo test --workspace --lib` - Passed (2,563 tests: 1065 + 360 + 375 + 763)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Double disk read per keypress**: Both `get_item_count_for_tab()` and `get_selected_item()` call `load_launch_configs()` independently (each reads from disk). This was pre-existing behavior and not introduced by this fix, but means two disk reads per navigation key on the LaunchConfig tab. Acceptable for a settings UI that is rarely used.
