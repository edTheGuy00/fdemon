## Task: Wire Up IconMode from Settings

**Objective**: Replace all 8 hardcoded `IconSet::new(IconMode::Unicode)` calls in the settings panel with `IconSet::new(self.settings.ui.icons)` so the panel respects the user's configured icon mode.

**Depends on**: None

**Severity**: Major (NerdFonts users see degraded icons)

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Replace 8 `IconSet::new()` calls
- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Remove `#[allow(dead_code)]` from `settings` field (line 42)
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs`: Verify tests still work (they create their own `Settings` with defaults)

### Details

#### Background

The codebase pattern is for `render/mod.rs` to create `IconSet::new(state.settings.ui.icons)` and pass it to widgets. However, `SettingsPanel` is unique — it creates its own `IconSet` instances internally rather than receiving one from the caller. The simplest fix is to read from `self.settings.ui.icons` since the widget already holds a `&Settings` reference.

#### The `settings` field

The `SettingsPanel` struct (line 40-43) already has:
```rust
pub struct SettingsPanel<'a> {
    #[allow(dead_code)] // Used in future tasks for rendering tab content
    settings: &'a Settings,
    project_path: &'a Path,
    title: &'a str,
}
```

The field exists but is marked dead_code. This task activates it.

#### All 8 Call Sites to Fix

| Line | Location | Context |
|------|----------|---------|
| 118 | `render_header()` | Header settings icon |
| 202 | `render_content()` | Icons passed to all tab renderers |
| 225 | `render_footer()` | Footer shortcut hints |
| 570 | `render_user_prefs_info()` | User prefs info banner icon |
| 795 | `render_launch_empty_state()` | Launch empty state icon |
| 986 | `render_vscode_info()` | VSCode info banner icon |
| 1032 | `render_vscode_not_found()` | VSCode not-found empty state icon |
| 1118 | `render_vscode_empty()` | VSCode empty state icon |

For each:
```rust
// BEFORE:
let icons = IconSet::new(IconMode::Unicode);

// AFTER:
let icons = IconSet::new(self.settings.ui.icons);
```

#### Remove dead_code annotation

```rust
// BEFORE (line 41-42):
#[allow(dead_code)] // Used in future tasks for rendering tab content
settings: &'a Settings,

// AFTER:
settings: &'a Settings,
```

#### Remove unused import (if applicable)

After replacing all `IconMode::Unicode` references, check if `IconMode` is still imported directly. It may still be needed as `self.settings.ui.icons` returns `IconMode`.

### Acceptance Criteria

1. All 8 `IconSet::new()` calls in the settings panel read from `self.settings.ui.icons`
2. No remaining `IconMode::Unicode` hardcoded in the settings panel (production code only — test code is fine)
3. `#[allow(dead_code)]` removed from the `settings` field
4. `cargo clippy --workspace -- -D warnings` passes clean
5. `cargo test -p fdemon-tui` passes

### Testing

Existing tests create `Settings::default()` which uses `IconMode::Unicode` by default, so they will continue to work. No test changes should be needed — the behavior is the same for default settings.

If desired, add a test that creates settings with `IconMode::NerdFonts` and verifies the NerdFont glyphs appear:

```rust
#[test]
fn test_settings_panel_uses_nerd_fonts_when_configured() {
    let mut settings = Settings::default();
    settings.ui.icons = IconMode::NerdFonts;
    // Render panel and check for NerdFont icon characters
}
```

### Notes

- This follows the established pattern used by `MainHeader`, `LogView`, and `NewSessionDialog`, all of which receive `IconSet` created from settings.
- Test code should continue to hardcode `IconMode::Unicode` for deterministic output — only production rendering code should read from settings.
- The `ConnectedDeviceList` and `BootableDeviceList` in `new_session_dialog/device_list.rs` have the same hardcoding issue but are out of scope for this task (they belong to a different widget).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Replaced 8 hardcoded `IconSet::new(IconMode::Unicode)` calls with `IconSet::new(self.settings.ui.icons)` on lines 116, 201, 224, 569, 794, 985, 1031, and 1117. Removed `#[allow(dead_code)]` annotation from `settings` field on line 41-42. Removed unused `IconMode` import on line 21. |

### Notable Decisions/Tradeoffs

1. **Removed IconMode import**: After replacing all `IconMode::Unicode` references with `self.settings.ui.icons`, the explicit `use fdemon_app::config::IconMode` import was no longer needed. The compiler can infer the type from the settings field.
2. **No test changes required**: Existing tests use `Settings::default()` which defaults to `IconMode::Unicode`, so behavior remains unchanged for existing tests.

### Testing Performed

- `cargo test -p fdemon-tui` - Passed (446 tests)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed with no warnings

### Risks/Limitations

None. This change activates the previously unused `settings` field and makes the settings panel respect the user's configured icon mode (Unicode vs NerdFonts).
