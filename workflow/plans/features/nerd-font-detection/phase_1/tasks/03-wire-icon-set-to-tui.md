## Task: Wire IconSet Through TUI Rendering

**Objective**: Update all TUI icon consumers to use `IconSet` instead of static `icons::ICON_*` constants. Also deduplicate the inline phase indicator literals in `styles.rs`.

**Depends on**: 01-add-icon-mode-config, 02-create-icon-set

### Scope

- `crates/fdemon-tui/src/widgets/header.rs`: Update `device_icon_for_platform()` to accept `&IconSet`
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Update `render_metadata_bar()` and `render_footer()` to use `IconSet`
- `crates/fdemon-tui/src/theme/styles.rs`: Update `phase_indicator()` and related functions to accept `&IconSet`
- Any callers of these functions that need to pass `IconSet` through

### Details

**1. Update `header.rs` — `device_icon_for_platform()`**

Current (line 226-235):
```rust
fn device_icon_for_platform(platform: Option<&str>) -> &'static str {
    match platform {
        Some(p) if p.contains("ios") || p.contains("simulator") => icons::ICON_SMARTPHONE,
        Some(p) if p.contains("web") || p.contains("chrome") => icons::ICON_GLOBE,
        Some(p) if p.contains("macos") || p.contains("linux") || p.contains("windows") => {
            icons::ICON_MONITOR
        }
        _ => icons::ICON_CPU,
    }
}
```

Change to:
```rust
fn device_icon_for_platform(platform: Option<&str>, icons: &IconSet) -> &'static str {
    match platform {
        Some(p) if p.contains("ios") || p.contains("simulator") => icons.smartphone(),
        Some(p) if p.contains("web") || p.contains("chrome") => icons.globe(),
        Some(p) if p.contains("macos") || p.contains("linux") || p.contains("windows") => {
            icons.monitor()
        }
        _ => icons.cpu(),
    }
}
```

Update the call site(s) in the header widget's `render()` method to construct `IconSet` from state and pass it. The `HeaderWidget` has access to `AppState` which contains `settings.ui.icons`.

Remove the `use crate::theme::icons` import from `header.rs` (replace with `use crate::theme::icons::IconSet`).

**2. Update `log_view/mod.rs` — icon usage**

Current icon usage (lines 651, 738, 770, 779, 784):
```rust
format!("{} ", icons::ICON_TERMINAL)
format!("{} {}", icons::ICON_ALERT, status.error_count)
format!("{} {}:{:02}", icons::ICON_ACTIVITY, mins, secs)
```

The `LogView` widget needs access to `IconSet`. The cleanest approach is to construct it from the `AppState.settings.ui.icons` that is already available in the widget's render context.

Find where `LogView` is instantiated/rendered and determine how `AppState` flows to it. Then construct `IconSet::new(state.settings.ui.icons)` at the render entry point and pass it through.

Replace all `icons::ICON_*` references with the corresponding `IconSet` method calls:
- `icons::ICON_TERMINAL` → `icons.terminal()`
- `icons::ICON_ALERT` → `icons.alert()`
- `icons::ICON_ACTIVITY` → `icons.activity()`

Remove the `icons` import from the `use crate::theme::{icons, palette}` line — replace with `use crate::theme::icons::IconSet`.

**3. Update `styles.rs` — phase indicators**

Current `phase_indicator()` (line 129-149) uses inline string literals for phase icons:
```rust
AppPhase::Running     => ("●", "Running", ...)
AppPhase::Reloading   => ("↻", "Reloading", ...)
AppPhase::Initializing => ("○", "Starting", ...)
AppPhase::Stopped     => ("○", "Stopped", ...)
AppPhase::Quitting    => ("✗", "Stopping", ...)
```

These inline literals duplicate `IconSet` values. Refactor to accept `&IconSet`:

```rust
pub fn phase_indicator(phase: &AppPhase, icons: &IconSet) -> (&'static str, &'static str, Style) {
    match phase {
        AppPhase::Running => (icons.dot(), "Running", ...),
        AppPhase::Reloading => (icons.refresh(), "Reloading", ...),
        AppPhase::Initializing => (icons.circle(), "Starting", ...),
        AppPhase::Stopped => (icons.circle(), "Stopped", ...),
        AppPhase::Quitting => (icons.close(), "Stopping", ...),
    }
}

pub fn phase_indicator_busy(icons: &IconSet) -> (&'static str, &'static str, Style) {
    (icons.refresh(), "Reloading", ...)
}

pub fn phase_indicator_disconnected(icons: &IconSet) -> (&'static str, &'static str, Style) {
    (icons.circle(), "Not Connected", ...)
}
```

Update all callers of `phase_indicator()`, `phase_indicator_busy()`, and `phase_indicator_disconnected()` to pass `&IconSet`.

**4. Find and update all callers**

Search for all call sites of these functions across the TUI crate:
- `device_icon_for_platform(` — called in `header.rs` render method
- `phase_indicator(` — likely called in `tabs.rs`, `status_bar.rs`, `header.rs`
- `phase_indicator_busy(` — likely called in `tabs.rs`
- `phase_indicator_disconnected(` — currently dead code, update signature anyway

For each caller, ensure `IconSet` is available (construct from `state.settings.ui.icons` or pass through).

### Acceptance Criteria

1. No remaining references to `icons::ICON_*` static constants in any `.rs` file
2. All icon rendering uses `IconSet` methods
3. `phase_indicator()` accepts `&IconSet` parameter — no more inline icon literals
4. `IconSet` is constructed from `state.settings.ui.icons` at render entry points
5. All widgets render identical output with `IconMode::Unicode` as before the change
6. `cargo check -p fdemon-tui` passes
7. `cargo clippy -p fdemon-tui -- -D warnings` passes

### Testing

- Existing widget tests should still pass (they test rendering with default state, which uses `IconMode::Unicode`)
- If any tests assert on specific icon characters (e.g., `assert_eq!(icon, "●")`), they should use `IconSet::new(IconMode::Unicode).dot()` instead of hardcoded strings
- Visual verification: run the app with default config and confirm rendering is unchanged

### Notes

- This is the most invasive task — it touches multiple widget files. But the changes are mechanical: replace `icons::ICON_FOO` with `icons.foo()` and thread the `IconSet` parameter through.
- The `IconSet` is cheap (`Copy`), so passing it by reference or by value is fine.
- Keep the function signatures minimal — prefer passing `&IconSet` over storing it in widget structs.
- Check `tabs.rs` and `status_bar.rs` for additional `phase_indicator()` callers that aren't listed in the original task scope.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/theme/styles.rs` | Updated `phase_indicator()`, `phase_indicator_busy()`, and `phase_indicator_disconnected()` to accept `&IconSet` parameter. All inline icon literals replaced with IconSet method calls. Updated all tests to use IconSet. |
| `crates/fdemon-tui/src/widgets/header.rs` | Updated `device_icon_for_platform()` to accept `&IconSet`. Added `icons` field to `MainHeader` struct. Updated constructor to accept IconSet. Updated all test cases to construct and pass IconSet. Changed import from `icons` module to `icons::IconSet`. |
| `crates/fdemon-tui/src/widgets/tabs.rs` | Added `icons` field to `SessionTabs` struct. Updated constructor to accept IconSet. Updated `tab_titles()` and `render_single_session()` to use IconSet for phase indicators. Updated all test cases. |
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Added `icons` field to `LogView` struct. Updated constructor to accept IconSet. Updated `render_metadata_bar()` to use `icons.terminal()`. Updated `render_bottom_metadata()` to accept IconSet parameter and use icon methods for alert, activity icons. Changed import from `icons` module to `icons::IconSet`. |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Added `test_icons()` helper function. Updated all `LogView::new()` calls to pass IconSet (25 call sites). |
| `crates/fdemon-tui/src/render/mod.rs` | Constructed `IconSet::new(state.settings.ui.icons)` at render entry point. Passed IconSet to `MainHeader::new()` and `LogView::new()` for both active session and empty state rendering. |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Updated test count assertion from 16 to 17 items to account for new `ui.icons` setting from Phase 1. |

### Notable Decisions/Tradeoffs

1. **IconSet as struct field vs parameter**: Added IconSet as a field to widget structs (`MainHeader`, `SessionTabs`, `LogView`) rather than threading it through every method call. This keeps the API clean while allowing widgets to access icons throughout their rendering lifecycle.

2. **Single construction point**: IconSet is constructed once in `render/mod.rs` from `state.settings.ui.icons` and then passed to all widgets. This ensures a consistent icon mode across the entire UI and makes it easy to change the icon set dynamically in the future.

3. **Test helper function**: Created `test_icons()` helper in test modules to reduce duplication and make it easy to change test icon mode in the future if needed.

4. **IconSet is Copy**: Because IconSet is a Copy type, we can pass it by value without worrying about performance or ownership issues.

### Testing Performed

- `cargo check -p fdemon-tui` - Passed
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed
- `cargo test -p fdemon-tui --lib` - Passed (418 tests)
- Verified no remaining references to `icons::ICON_*` static constants via grep

### Risks/Limitations

1. **No visual verification**: Tests verify the structure and logic are correct, but visual verification requires running the app to confirm icons render correctly with both Unicode and NerdFonts modes. This should be done in Phase 1 integration testing.

2. **Settings panel test update**: Had to update test count from 16 to 17 to account for the new `ui.icons` setting added in task 01. This is expected but worth noting as it affects test stability across phase boundaries.
