## Task: Implement LaunchButton Focus Visual Feedback

**Objective**: Add visual focus indication to `LaunchButton` so users can see when the Launch button is selected via keyboard navigation. Currently `is_focused` is stored but never used in `render()`, making the button the only field widget without focus feedback.

**Depends on**: None

**Review Reference**: REVIEW.md #2 (Critical), ACTION_ITEMS.md #2

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs:374-406`: Update `LaunchButton::render()` to branch on `is_focused`

### Details

**Root cause**: The `render()` method only branches on `is_enabled`:
```rust
let (bg, fg, border) = if self.is_enabled {
    (palette::GRADIENT_BLUE, palette::TEXT_BRIGHT, palette::GRADIENT_BLUE)
} else {
    (palette::SURFACE, palette::TEXT_MUTED, palette::BORDER_DIM)
};
```

The `is_focused` field (line 349) and `focused()` setter (lines 363-366) exist and are properly called by all three callers:
- `LaunchContext::render()` at line 794-798
- `LaunchContextWithDevice::render_full()` at line 870-875
- `LaunchContextWithDevice::render_compact()` at line 921-926

**Fix approach**: Add focus-based border styling, following the pattern used by `DropdownField` (line 78-82) and `ActionField` (line 302-306):

```rust
let (bg, fg, border) = if !self.is_enabled {
    (palette::SURFACE, palette::TEXT_MUTED, palette::BORDER_DIM)
} else if self.is_focused {
    (palette::GRADIENT_BLUE, palette::TEXT_BRIGHT, palette::BORDER_ACTIVE)
} else {
    (palette::GRADIENT_BLUE, palette::TEXT_BRIGHT, palette::GRADIENT_BLUE)
};
```

This gives three states:
- **Disabled**: dim background (`SURFACE`), muted text, dim border — greyed out
- **Enabled + Focused**: blue background, bright text, cyan active border (`BORDER_ACTIVE` = Rgb(88, 166, 255)) — visually distinct from unfocused
- **Enabled + Unfocused**: blue background, bright text, blue border (same as background) — current behavior

The key visual difference is the border: focused gets `BORDER_ACTIVE` (bright cyan) while unfocused uses `GRADIENT_BLUE` (matching background, so border blends in). This is consistent with how `DropdownField` and `ActionField` use `styles::border_active()` vs `styles::border_inactive()`.

### Acceptance Criteria

1. LaunchButton shows a visually distinct border when focused via keyboard navigation
2. Unfocused enabled state remains unchanged (border blends with background)
3. Disabled state remains unchanged (greyed out)
4. Focus styling is consistent with DropdownField and ActionField patterns
5. `cargo check -p fdemon-tui` passes

### Testing

- Add unit test verifying border color differs between focused and unfocused states (defer comprehensive tests to Task 06)
- Verify existing `test_launch_button_renders()` and `test_launch_button_disabled_text()` still pass

### Notes

- This is a minimal change — only the color tuple logic in `render()` needs updating.
- The `BORDER_ACTIVE` constant is `Color::Rgb(88, 166, 255)` (bright cyan), providing clear visual contrast against the `GRADIENT_BLUE` background `Color::Rgb(37, 99, 235)`.
- All callers already correctly compute and pass focus state. No caller changes needed.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Updated `LaunchButton::render()` to branch on `is_focused` (lines 376-390), added unit test `test_launch_button_focus_border()` (lines 617-665) |

### Notable Decisions/Tradeoffs

1. **Focus State Priority**: Reordered the conditional to check `!is_enabled` first, then `is_focused`, then the default enabled state. This ensures disabled state always takes precedence over focus state, which is consistent with other field widgets.

2. **Border Color Choice**: Used `palette::BORDER_ACTIVE` (bright cyan) for focused state instead of `styles::border_active()` to maintain direct color control in the button widget, consistent with the existing pattern where the button manages colors directly rather than using style helpers.

3. **Test Implementation**: Used `buffer.cell((x, y))` instead of deprecated `buffer.get(x, y)` to avoid deprecation warnings and follow ratatui best practices.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed (1.17s)
- `cargo test -p fdemon-tui --lib` - Passed (429 tests, all passed)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed (0.78s, no warnings)
- `cargo test -p fdemon-tui --lib -- test_launch_button` - Passed (3 tests: `test_launch_button_renders`, `test_launch_button_disabled_text`, `test_launch_button_focus_border`)

### Risks/Limitations

None. This is a minimal, low-risk change that:
- Only modifies the internal rendering logic of `LaunchButton`
- Maintains backward compatibility with all existing callers
- All existing tests continue to pass
- New test verifies the focus border behavior works correctly
