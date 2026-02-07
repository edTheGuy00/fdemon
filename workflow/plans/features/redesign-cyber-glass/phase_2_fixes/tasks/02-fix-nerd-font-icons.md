## Task: Replace Nerd Font Icons with Safe Unicode

**Objective**: Replace all Nerd Font glyphs used in production rendering paths with universally-supported Unicode characters. Nerd Font icons render as `?` or tofu in terminals without Nerd Fonts installed (reported in Zed integrated terminal).

**Depends on**: None

**Review Reference**: REVIEW.md #2 (Critical), ACTION_ITEMS.md #2

### Scope

- `crates/fdemon-tui/src/theme/icons.rs`: Replace `ICON_*` constants with safe Unicode equivalents. Keep Nerd Font values as `NERD_*` constants for future opt-in.
- `crates/fdemon-tui/src/widgets/header.rs:222-231`: `device_icon_for_platform()` uses `icons::ICON_SMARTPHONE`, `ICON_GLOBE`, `ICON_MONITOR`, `ICON_CPU`.
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Lines 647 (`ICON_TERMINAL`), 734 (`ICON_ALERT`), 766 (`ICON_ACTIVITY`), 775 (`ICON_ALERT`), 780 (`ICON_ALERT`).

### Details

**Strategy**: Replace the default `ICON_*` constants with universally-supported Unicode. The `phase_indicator()` function already uses safe characters ("●", "○", "↻", "✗") — follow that pattern.

**Suggested safe Unicode replacements**:

| Constant | Current (Nerd Font) | Replacement (Safe Unicode) | Notes |
|----------|---------------------|---------------------------|-------|
| `ICON_TERMINAL` | `\u{f120}` | `">"` or `"❯"` | Terminal prompt indicator |
| `ICON_SMARTPHONE` | `\u{f3cd}` | `"📱"` or `"[M]"` | Mobile device |
| `ICON_GLOBE` | `\u{f0ac}` | `"🌐"` or `"[W]"` | Web device |
| `ICON_MONITOR` | `\u{f108}` | `"🖥"` or `"[D]"` | Desktop device |
| `ICON_ACTIVITY` | `\u{f0f1}` | `"⏱"` or `"~"` | Uptime/activity |
| `ICON_ALERT` | `\u{f071}` | `"⚠"` or `"!"` | Warning/error count |
| `ICON_CPU` | `\u{f2db}` | `"⚙"` or `"[C]"` | Generic device fallback |
| `ICON_PLAY` | `\u{f04b}` | `"▶"` | Play/running |
| `ICON_STOP` | `\u{f04d}` | `"■"` | Stopped |
| `ICON_REFRESH` | `\u{f021}` | `"↻"` | Reload/refresh |
| `ICON_CHECK` | `\u{f00c}` | `"✓"` | Success |
| `ICON_CLOSE` | `\u{f00d}` | `"✗"` | Close/error |
| `ICON_CHEVRON_R` | `\u{f054}` | `"›"` | Right chevron |
| `ICON_CHEVRON_D` | `\u{f078}` | `"⌄"` | Down chevron |

**Implementation steps**:

1. In `icons.rs`, rename current `ICON_*` constants to `NERD_*` (preserve for future Nerd Font opt-in)
2. Replace `ICON_*` constant values with the safe Unicode equivalents
3. Remove `ASCII_*` constants (the `ICON_*` values are now safe enough to serve as universal defaults)
4. Fix `ICON_TERMINAL` and `ICON_COMMAND` being identical — give `ICON_COMMAND` a distinct value (e.g., `"$"`)
5. Update the module docstring to remove the reference to the nonexistent `icon()` function
6. No changes needed to consuming code — they already reference `icons::ICON_*` which will now be safe Unicode

### Acceptance Criteria

1. All icons render correctly in Ghostty (full Unicode support)
2. All icons render correctly in Zed integrated terminal (basic Unicode support)
3. No `?` or tofu characters visible in any rendering path
4. Nerd Font constants preserved as `NERD_*` for future config-driven opt-in
5. `ICON_TERMINAL` and `ICON_COMMAND` are visually distinct
6. Module docstring accurately reflects current state (no phantom `icon()` reference)
7. `cargo check -p fdemon-tui` passes

### Testing

- Visual inspection in both Ghostty and Zed terminals
- Existing tests that reference icon constants will need updating if they assert on specific glyph values

### Notes

- Prefer single-character Unicode symbols that are widely supported (Unicode 6.0+ / BMP)
- Emoji (📱, 🌐, 🖥) are double-width in most terminals — consider using single-width alternatives to avoid layout issues
- The `phase_indicator()` function in `styles.rs` already uses safe Unicode ("●", "○", "↻", "✗") and works correctly — this validates the approach
