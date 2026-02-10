## Task: Update Settings Panel Tests

**Objective**: Fix all broken tests caused by the Phase 4 visual redesign and add new test coverage for the redesigned components — group headers with icons, accent bar selection, info banners, empty states, and footer hints.

**Depends on**: 03-redesign-settings-header, 04-redesign-settings-content, 05-redesign-special-views, 06-redesign-settings-footer

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` — Fix broken tests, add new tests
- `crates/fdemon-tui/src/theme/icons.rs` — Verify new icon tests (from task 01)

### Details

#### Expected Test Breakage

The 754-line test file (`tests.rs`) will have widespread breakage from the visual redesign. Here's a breakdown by test category:

##### Widget Rendering Tests (lines 8-155) — Most will break

| Test | Why it breaks | Fix approach |
|------|--------------|-------------|
| `test_settings_panel_renders` | Header layout changed (5 lines), new styles | Update expected buffer content |
| `test_settings_panel_shows_active_tab` | Tab styling changed (pill-style, uppercase) | Update expected tab rendering |
| `test_settings_panel_dirty_indicator` | Footer redesigned | Update expected footer text |
| `test_render_shows_all_tabs` | Tab labels now uppercase | Update expected labels |
| `test_tab_icons` | Tab icon format may have changed | Update expected format |

##### Data Model Tests (lines 166-200) — Should NOT break

These test `settings_items.rs` data generation, not rendering. They should pass unchanged:
- `test_project_settings_items_count`
- `test_project_settings_sections`
- `test_setting_is_modified`

##### Tab Rendering Tests (lines 213-321) — Will break

| Test | Why it breaks | Fix approach |
|------|--------------|-------------|
| `test_render_project_tab` | Section headers now icon+uppercase | Update expected format |
| `test_render_launch_tab_empty` | Empty state redesigned (icon container) | Update expected rendering |
| `test_render_launch_tab_with_configs` | Config headers may change | Update expected format |

##### Style Tests (lines 328-388) — Some will break

| Test | Why it breaks | Fix approach |
|------|--------------|-------------|
| `test_value_style_*` | Unchanged — value styles preserved | No fix needed |
| Style function tests referencing `section_header_style` | Returns `ACCENT_DIM` not `STATUS_YELLOW` | Update expected color |

##### User Preferences Tests (lines 393-426) — Should NOT break

Data model tests, not rendering.

##### VSCode Config Tests (lines 432-470) — Should NOT break

Data model tests, not rendering.

##### Editor/Editing Tests (lines 476-754) — Mostly should NOT break

These test state transitions, not visual rendering. Some may break if they check rendered buffer content.

#### New Tests to Add

**1. Group Header Tests**

```rust
#[test]
fn test_section_header_renders_icon_and_uppercase() {
    // Render a project tab section header
    // Verify: icon glyph present + spaced uppercase text + ACCENT_DIM color
}

#[test]
fn test_section_header_icon_mapping() {
    // Verify each section name maps to correct icon:
    // "Behavior" → zap, "Watcher" → eye, "UI" → monitor,
    // "DevTools" → cpu, "Editor" → code
}
```

**2. Selected Row Tests**

```rust
#[test]
fn test_selected_row_has_accent_bar() {
    // Render a setting row with is_selected=true
    // Verify: '▎' character at column 0 with ACCENT fg
}

#[test]
fn test_selected_row_has_tinted_background() {
    // Render a setting row with is_selected=true
    // Verify: cells have SELECTED_ROW_BG background
}

#[test]
fn test_unselected_row_has_no_accent_bar() {
    // Render a setting row with is_selected=false
    // Verify: no '▎' character, no background tint
}
```

**3. Info Banner Tests**

```rust
#[test]
fn test_user_prefs_info_banner_glass_style() {
    // Render user prefs info banner
    // Verify: rounded border, ACCENT_DIM border color, info icon present
}

#[test]
fn test_user_prefs_info_banner_content() {
    // Verify: "Local Settings Active" title + path subtitle
}
```

**4. Empty State Tests**

```rust
#[test]
fn test_launch_empty_state_centered() {
    // Render launch empty state in a 60x20 area
    // Verify: icon container is horizontally centered
    // Verify: title text present
}

#[test]
fn test_launch_empty_state_small_terminal() {
    // Render launch empty state in a 40x10 area
    // Verify: degrades gracefully (may skip icon container)
}
```

**5. Footer Tests**

```rust
#[test]
fn test_footer_normal_mode_shows_4_hints() {
    // Render footer in normal mode
    // Verify: "Tab:", "j/k:", "Enter:", "Ctrl+S:" all present
}

#[test]
fn test_footer_ctrl_s_emphasized() {
    // Render footer in normal mode
    // Verify: "Ctrl+S:" uses ACCENT color
}

#[test]
fn test_footer_editing_mode_shows_confirm_cancel() {
    // Render footer in editing mode
    // Verify: "Enter:" + "Confirm" and "Esc:" + "Cancel" present
}

#[test]
fn test_footer_dirty_shows_asterisk() {
    // Render footer with dirty=true
    // Verify: "Save Changes*" or similar dirty indicator
}
```

**6. Tab Bar Tests**

```rust
#[test]
fn test_tab_bar_pill_style_active() {
    // Render header with active tab
    // Verify: active tab has ACCENT bg
}

#[test]
fn test_tab_labels_uppercase() {
    // Render header
    // Verify: tab labels contain "PROJECT", "USER", "LAUNCH", "VSCODE"
}

#[test]
fn test_header_shows_settings_icon_and_title() {
    // Render header
    // Verify: settings icon glyph present + "System Settings" text
}
```

**7. Icon Tests (in icons.rs)**

```rust
#[test]
fn test_new_settings_icons_exist() {
    let icons = IconSet::new(IconMode::Unicode);
    // Verify each new method returns a non-empty string
    assert!(!icons.zap().is_empty());
    assert!(!icons.eye().is_empty());
    assert!(!icons.code().is_empty());
    assert!(!icons.user().is_empty());
    assert!(!icons.keyboard().is_empty());
    assert!(!icons.save().is_empty());
}
```

#### Fix Strategy

1. **Read each failing test** to understand what it asserts
2. **Run `cargo test -p fdemon-tui` first** to see which tests fail
3. **Fix rendering assertion tests** by updating expected buffer content/strings
4. **Verify data model tests pass unchanged** — these are safety checks
5. **Add new tests** for redesigned components
6. **Run full suite** to confirm no regressions

### Acceptance Criteria

1. All existing 754 lines of tests either pass or are updated to match new rendering
2. Data model tests (items count, sections, readonly) pass without changes
3. New tests added for: group headers, accent bar selection, info banners, empty states, footer hints
4. At least 5 new test functions added
5. `cargo test -p fdemon-tui` passes with 0 failures
6. `cargo test --workspace` passes with 0 failures
7. `cargo clippy --workspace` passes with no warnings
8. `cargo fmt --all` passes

### Testing

Run the full verification pipeline:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

### Notes

- **Test-last approach**: This task is intentionally last. Running tests during tasks 03-06 would show failures for in-progress changes. Wait until all rendering tasks are complete before fixing tests.
- **Buffer assertion tests**: Many tests create a `Buffer` and check specific cell contents. These are fragile to visual changes. When updating them, focus on semantic correctness (right text, right colors) rather than exact character positions.
- **Consider test helpers**: If multiple tests need to render the settings panel, consider extracting a helper function that creates a test state and renders to a buffer.
- **Style tests**: Tests that check `section_header_style()` returns `STATUS_YELLOW` need updating to check for `ACCENT_DIM`. Tests that check `label_style(false)` returns default need updating to check for `TEXT_SECONDARY`.
- **Snapshot testing**: If updating buffer assertions becomes too tedious, consider adding `insta` snapshot tests for complex rendering. However, this is optional polish — standard assertion tests are fine.

---

## Completion Summary

**Status:** Not Started
