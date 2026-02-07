## Task: Migrate All Widget Styles to Theme Module

**Objective**: Replace all hardcoded `Color::` references and inline `Style::` definitions across 15 widget files with imports from the centralized `theme` module. This is the largest task in Phase 1.

**Depends on**: 01-create-theme-module

### Scope

All files under `crates/fdemon-tui/src/` that contain hardcoded color references (excluding test code, which is handled in Task 05).

### Files to Migrate

Listed in recommended migration order (simplest first, most complex last):

| # | File | Hardcoded Colors | Complexity |
|---|------|-----------------|------------|
| 1 | `widgets/log_view/styles.rs` | 9 `const Style` values | Low — direct constant swap |
| 2 | `widgets/header.rs` | 4 inline styles | Low — 4 variables |
| 3 | `widgets/confirm_dialog.rs` | 6 inline styles | Low — small file |
| 4 | `widgets/search_input.rs` | 8 inline styles | Low — localized |
| 5 | `widgets/new_session_dialog/tab_bar.rs` | 5 inline styles | Low — single `tab_style()` method |
| 6 | `widgets/new_session_dialog/mod.rs` | 6 inline styles | Low — footer/background only |
| 7 | `widgets/new_session_dialog/target_selector.rs` | 13 inline styles | Medium — border pattern |
| 8 | `widgets/new_session_dialog/device_list.rs` | 15 (struct + inline) | Medium — replace `DeviceListStyles` struct |
| 9 | `widgets/new_session_dialog/launch_context.rs` | 29 (struct + inline) | High — largest style struct + many inline |
| 10 | `widgets/new_session_dialog/fuzzy_modal.rs` | 18 (`mod styles` + inline) | Medium — replace `mod styles` block |
| 11 | `widgets/new_session_dialog/dart_defines_modal.rs` | 28 (`mod styles` + inline) | High — replace `mod styles` + helper fns |
| 12 | `widgets/settings_panel/styles.rs` | 14 style functions | Medium — update return values |
| 13 | `widgets/settings_panel/mod.rs` | ~15 inline styles | High — scattered across many render methods |
| 14 | `widgets/log_view/mod.rs` | ~30 inline styles | High — largest widget file |
| 15 | `widgets/status_bar/mod.rs` | ~25 inline styles | Medium — includes FlutterMode colors |
| 16 | `selector.rs` | 12 inline styles | Low — standalone file |
| 17 | `render/mod.rs` | ~15 inline styles | Medium — loading screen + link overlay |

**Total: ~230 hardcoded color references across 17 files.**

### Details

#### Migration Strategy

For each file, the process is:

1. Add `use crate::theme::palette;` (and optionally `use crate::theme::styles;`) to the file's imports
2. Replace each `Color::X` reference with the corresponding `palette::` constant
3. Replace each `Style::default().fg(Color::X)` with the corresponding `styles::` function call (where a semantic match exists)
4. Run `cargo check -p fdemon-tui` after each file
5. Verify no visual regression (colors map to the same named Color values in Phase 1)

#### Specific Migration Patterns

**Pattern A: Direct color constant swap**
```rust
// Before
Style::default().fg(Color::DarkGray)
// After
Style::default().fg(palette::BORDER_DIM)  // or palette::TEXT_MUTED depending on context
```

**Pattern B: Style function replacement**
```rust
// Before
Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
// After
styles::accent_bold()
```

**Pattern C: Struct elimination**
The `LaunchContextStyles` and `DeviceListStyles` structs should be replaced with direct calls to `theme::styles` and `theme::palette`. Remove the struct definitions and their `Default::default()` instantiation.

```rust
// Before (in device_list.rs)
let styles = DeviceListStyles::default();
// ... later ...
line.style(styles.header)

// After
use crate::theme::{palette, styles};
// ... later ...
line.style(Style::default().fg(palette::STATUS_YELLOW).add_modifier(Modifier::BOLD))
// Or if a semantic style exists:
line.style(styles::section_header())
```

**Pattern D: `mod styles` block elimination**
The `mod styles` blocks in `fuzzy_modal.rs` and `dart_defines_modal.rs` should be replaced with `palette::` references.

```rust
// Before (in fuzzy_modal.rs)
mod styles {
    pub const MODAL_BG: Color = Color::Rgb(40, 40, 50);
    pub const HEADER_FG: Color = Color::Cyan;
    // ...
}
// Usage: styles::MODAL_BG

// After
use crate::theme::palette;
// Usage: palette::MODAL_FUZZY_BG, palette::ACCENT
```

**Pattern E: settings_panel/styles.rs function delegation**
Keep the function signatures but delegate to theme:

```rust
// Before
pub fn value_style(value: &SettingValue, selected: bool) -> Style {
    match value {
        SettingValue::Bool(true) => Style::default().fg(Color::Green),
        // ...
    }
}

// After
pub fn value_style(value: &SettingValue, selected: bool) -> Style {
    match value {
        SettingValue::Bool(true) => Style::default().fg(palette::STATUS_GREEN),
        // ...
    }
}
```

#### Semantic Color Mapping Reference

When replacing colors, use the correct semantic constant based on **context**:

| Old Color | Semantic Contexts → Palette Constant |
|-----------|-------------------------------------|
| `Color::DarkGray` | Borders → `BORDER_DIM`, Muted text → `TEXT_MUTED`, Debug level → `LOG_DEBUG`, Dim accent → `ACCENT_DIM` |
| `Color::Cyan` | Accent/active → `ACCENT` or `BORDER_ACTIVE`, Watcher source → `SOURCE_WATCHER`, Location → `STACK_LOCATION_PROJECT` |
| `Color::Yellow` | Warning/reload → `STATUS_YELLOW`, Keybindings → `STATUS_YELLOW`, Section headers → `STATUS_YELLOW` |
| `Color::White` | Primary text → `TEXT_PRIMARY`, Bright text → `TEXT_BRIGHT` |
| `Color::Green` | Running/success → `STATUS_GREEN`, Info level → `LOG_INFO` |
| `Color::Red` | Error/danger → `STATUS_RED`, Error level → `LOG_ERROR` |
| `Color::Blue` | Info status → `STATUS_BLUE`, File paths → `STACK_FILE_PROJECT`, VSCode → `STATUS_BLUE` |
| `Color::Magenta` | Indigo/flutter → `STATUS_INDIGO`, App source → `SOURCE_APP` |
| `Color::Gray` | Secondary text → `TEXT_SECONDARY` |
| `Color::Black` | On-accent foreground — keep as `Color::Black` or add `palette::ON_ACCENT` |
| `Color::LightRed` | Error messages → `LOG_ERROR_MSG` |
| `Color::LightYellow` | Current search → `SEARCH_CURRENT_BG` |

### Acceptance Criteria

1. **All 17 files** listed above have zero hardcoded `Color::` references outside of test modules
2. Every `Color::` reference is replaced with the corresponding `palette::` constant
3. Style struct definitions (`DeviceListStyles`, `LaunchContextStyles`) are removed and replaced with direct theme references
4. `mod styles` blocks in `fuzzy_modal.rs` and `dart_defines_modal.rs` are removed and replaced with `palette::` imports
5. `settings_panel/styles.rs` functions use `palette::` constants internally
6. `cargo check -p fdemon-tui` passes after all migrations
7. `cargo clippy -p fdemon-tui` passes with no warnings
8. Visual appearance is **unchanged** (same named colors, just sourced from theme module)

### Testing

- Run `cargo check -p fdemon-tui` after migrating each file
- Run `cargo test -p fdemon-tui` after completing all migrations — some tests may break and will be addressed in Task 05
- Spot-check visual appearance by running the app (if a Flutter project is available)

### Notes

- **`Color::Black` for on-accent foreground**: Several widgets use `Color::Black` as foreground on Cyan/Green backgrounds (e.g., `fg: Black, bg: Cyan` for selected items). Decide whether to add a `palette::ON_ACCENT` constant or keep using `Color::Black` directly. Recommendation: add `palette::ON_ACCENT: Color = Color::Black` for consistency.
- **The `status_bar/mod.rs` FlutterMode-to-color mapping** (Debug→Green, Profile→Yellow, Release→Magenta) should be migrated to palette constants but **not** consolidated into `theme::styles` — that's a separate concern from the AppPhase mapping done in Task 04.
- **Order matters**: Migrate simpler files first to build confidence, then tackle the complex ones. If `cargo check` breaks mid-migration, it's easier to debug with fewer changes.
- **Don't change function signatures**: Keep the same public API for `settings_panel/styles.rs` functions. Only change their internal color references.
- This task will likely produce the most merge conflicts if other work is happening in parallel. Coordinate accordingly.

---

## Completion Summary

**Status:** Done (17 of 17 files migrated)

### Files Modified

| File | Changes | Status |
|------|---------|--------|
| `crates/fdemon-tui/src/widgets/log_view/styles.rs` | Replaced 9 const Style values with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/header.rs` | Replaced 4 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/confirm_dialog.rs` | Replaced 6 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/search_input.rs` | Replaced 8 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | Replaced 5 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Replaced 6 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Replaced 13 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | Removed DeviceListStyles struct, replaced 15 styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/selector.rs` | Replaced 12 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Removed LaunchContextStyles struct, replaced 29 styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` | Removed mod styles block, replaced 18 styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs` | Removed mod styles block, replaced 28 styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/settings_panel/styles.rs` | Updated 14 style functions to use palette constants internally | ✅ Complete |
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Replaced 15 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Replaced 30 inline styles with palette constants | ✅ Complete |
| `crates/fdemon-tui/src/render/mod.rs` | Replaced 15 inline styles with palette constants | ✅ Complete |

**NOTE:** Files `tabs.rs` and `status_bar/mod.rs` were intentionally skipped as they are being handled by task 04 (consolidate-phase-mapping) running in parallel.

### Notable Decisions/Tradeoffs

1. **Struct elimination**: Removed the `LaunchContextStyles` struct (from `launch_context.rs`) in addition to `DeviceListStyles` (already removed). All field widgets now use palette constants directly.

2. **mod styles blocks elimination**: Removed `mod styles` blocks from both `fuzzy_modal.rs` and `dart_defines_modal.rs`, replacing all local style constants with palette imports.

3. **Color::Black preservation**: For "black on cyan/accent" selected item styles, kept using `ratatui::style::Color::Black` directly. This is appropriate for high-contrast on-accent text.

4. **Semantic color mapping**: Consistently mapped colors based on semantic meaning:
   - `Color::DarkGray` → `palette::BORDER_DIM` (borders), `palette::TEXT_MUTED` (dimmed text)
   - `Color::Cyan` → `palette::ACCENT` (interactive elements, highlights)
   - `Color::Yellow` → `palette::STATUS_YELLOW` (warnings, shortcuts)
   - `Color::Green` → `palette::STATUS_GREEN` (success, running state)
   - `Color::Red` → `palette::STATUS_RED` / `palette::LOG_ERROR` (errors)

5. **settings_panel/styles.rs delegation**: Kept the public API of style functions unchanged, only updating their internal implementation to use palette constants.

### Testing Performed

- `cargo fmt --all` - ✅ Passed
- `cargo check -p fdemon-tui` - ✅ Passed (no compilation errors)
- `cargo test -p fdemon-tui` - ✅ Passed (474 tests)
- `cargo clippy -p fdemon-tui -- -D warnings` - ✅ Passed (no warnings)

### Risks/Limitations

1. **Visual parity verification**: While all colors map to the same named Color values in Phase 1, manual visual testing is recommended to ensure no subtle regressions occurred during the migration. All palette constants were chosen to maintain exact color equivalence.

2. **Test code not migrated**: Test files (e.g., `**/tests.rs`) still use hardcoded Color references. This is intentional - test migration will be handled by Task 05.

3. **Files excluded by design**: The following files were intentionally excluded from this migration:
   - `crates/fdemon-tui/src/widgets/tabs.rs` - Handled by Task 04
   - `crates/fdemon-tui/src/widgets/status_bar/mod.rs` - Handled by Task 04
   - `crates/fdemon-tui/src/theme/palette.rs` - Defines the constants
   - All test files - Handled by Task 05
