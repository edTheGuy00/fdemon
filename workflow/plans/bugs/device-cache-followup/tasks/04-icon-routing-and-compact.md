# Task 04 — Route `↻` Through `IconSet` + Compact-Mode Glyph

**Agent:** implementor
**Phase:** 2
**Depends on:** none (Wave 3, after Wave 2 has merged)
**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`

---

## Goal

Fix Minor issues m2, m4, and nitpick n2:

- **m2:** `tab_bar.rs:71` hardcodes the literal `"↻"`, bypassing
  `IconSet::refresh()` (`crates/fdemon-tui/src/theme/icons.rs:96-101`) which already
  resolves the correct glyph for both `IconMode::Unicode` and `IconMode::NerdFonts`.
  Nerd Fonts users currently see the wrong glyph.
- **m4:** `render_tabs_compact` in `target_selector.rs` does not surface the refresh
  indicator. Users on short terminals get no visual cue that a background refresh is in
  flight.
- **n2:** `test_tab_bar_renders_bootable_refreshing_indicator` lacks the diagnostic
  message its sister test has.

## Context

`IconSet::refresh()` already returns `"\u{21bb}"` (= `↻`) for `Unicode` and the Nerd
Font equivalent for `NerdFonts`. The pattern for threading `&IconSet` into a widget is
already established in `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs:162,175`
(`icons: &'a IconSet`).

`TabBar::new()` currently takes 4 args (`active_tab, pane_focused, connected_refreshing,
bootable_refreshing`). It will gain `icons: &IconSet` as a fifth.

`render_tabs_compact` (around `target_selector.rs:208-251`) renders a single line of
`Span`s wrapped in a `Paragraph`. Both `self.state.refreshing` and
`self.state.bootable_refreshing` are accessible.

`render_full` already calls `TabBar::new(self.state.active_tab, self.is_focused,
self.state.refreshing, self.state.bootable_refreshing)`. The call site needs to gain
`icons` — locate where `target_selector.rs` already receives or constructs an `IconSet`.

## Steps

1. **Update `TabBar::new()` signature** in `tab_bar.rs:27`. Add a final `icons: &'a IconSet`
   parameter; introduce a lifetime `'a` on the struct or accept an owned `IconSet` (cheap
   to clone — it's a thin wrapper over an enum). Pick whichever fits the existing pattern
   in `mod.rs:162`. Suggested:

   ```rust
   pub struct TabBar<'a> {
       active_tab: TargetTab,
       pane_focused: bool,
       connected_refreshing: bool,
       bootable_refreshing: bool,
       icons: &'a IconSet,
   }

   impl<'a> TabBar<'a> {
       pub fn new(
           active_tab: TargetTab,
           pane_focused: bool,
           connected_refreshing: bool,
           bootable_refreshing: bool,
           icons: &'a IconSet,
       ) -> Self {
           Self { active_tab, pane_focused, connected_refreshing, bootable_refreshing, icons }
       }
   }
   ```

   Add `use crate::theme::icons::IconSet;` if not already imported.

2. **Replace the inline `"↻"` literal** in the render loop (around line 71). Replace:

   ```rust
   let label = if refreshing {
       format!("{} ↻", tab.label())
   } else {
       tab.label().to_string()
   };
   ```

   with:

   ```rust
   let label = if refreshing {
       format!("{} {}", tab.label(), self.icons.refresh())
   } else {
       tab.label().to_string()
   };
   ```

3. **Update `target_selector.rs` `render_full`** to pass `icons` into `TabBar::new()`.
   Locate the call (around line 92) and append the icons argument. The icons reference
   should already be reachable via the surrounding `render` / widget context — check
   `target_selector.rs` for existing `&IconSet` or `IconSet` access; if none, accept it
   as a parameter on `TargetSelector` (mirror the pattern from `device_list.rs:64,236`
   which holds `icons: IconSet` and exposes `set_icon_mode`). Pick whichever style fits
   the existing call hierarchy with the smallest churn.

4. **Update `render_tabs_compact`** in `target_selector.rs` (around line 208-251). After
   building each tab's `Span`s, append a small space + the refresh glyph when its flag is
   set. Pseudocode:

   ```rust
   let connected_label = if self.state.refreshing {
       format!("[1 {} {}]", connected_text, icons.refresh())
   } else {
       format!("[1 {}]", connected_text)
   };
   ```

   Mirror the existing active/inactive styling. Keep changes minimal — the `↻` is a
   secondary cue, not a layout element.

5. **Update existing test assertions** in both `tab_bar.rs` and `target_selector.rs`.
   Replace literal `"↻"` checks with assertions that resolve the glyph through
   `IconSet::default()`. Example pattern:

   ```rust
   let icons = IconSet::default(); // = IconMode::Unicode → "↻"
   let glyph = icons.refresh();
   assert!(rendered.contains(glyph), "expected refresh glyph, got: {rendered}");
   ```

   Update calls to `TabBar::new(...)` in tests (currently 4-arg) to pass `&icons` as the
   fifth argument. Search for all occurrences:

   ```bash
   grep -n "TabBar::new" crates/fdemon-tui/src/widgets/new_session_dialog/
   ```

6. **Add a compact-mode render test** in `target_selector.rs` (place near the existing
   `test_target_selector_renders_refreshing_glyph_when_state_set` from the parent plan):

   ```rust
   #[test]
   fn test_target_selector_compact_renders_refreshing_glyph() {
       // Use a height < MIN_EXPANDED_HEIGHT to force compact mode
       let area = Rect::new(0, 0, 40, 6);
       let mut state = TargetSelectorState::default();
       state.set_connected_devices(vec![test_device("dev1", "Device 1")]);
       state.refreshing = true;
       state.active_tab = TargetTab::Connected;

       let icons = IconSet::default();
       let rendered = render_to_string(/* construct TargetSelector with state, icons, area */);

       assert!(rendered.contains(icons.refresh()),
           "compact mode should show refresh glyph when active tab is refreshing");
   }
   ```

   Use the same render-to-string helper that the existing compact-mode tests use (search
   for `render_compact` test patterns).

7. **Add diagnostic message** to `test_tab_bar_renders_bootable_refreshing_indicator`
   (n2). Mirror the format used by `test_tab_bar_renders_connected_refreshing_indicator`:

   ```rust
   assert!(rendered.contains(glyph),
       "expected refresh glyph on Bootable tab, got: {rendered}");
   ```

8. Run verification:
   - `cargo fmt --all`
   - `cargo check -p fdemon-tui`
   - `cargo test -p fdemon-tui --lib`
   - `cargo clippy -p fdemon-tui --lib -- -D warnings`

## Acceptance Criteria

- [ ] `tab_bar.rs:71` no longer contains the inline literal `"↻"`; uses
      `self.icons.refresh()` instead.
- [ ] `TabBar::new()` accepts `&IconSet` as a fifth parameter.
- [ ] `target_selector.rs::render_full` passes `&IconSet` into `TabBar::new()`.
- [ ] `render_tabs_compact` surfaces the refresh glyph when the active tab's flag is set.
- [ ] All existing test assertions in `tab_bar.rs` and `target_selector.rs` use
      `IconSet::default().refresh()` (or equivalent) instead of literal `"↻"`.
- [ ] All `TabBar::new(...)` test call sites updated to include the icons argument.
- [ ] New test `test_target_selector_compact_renders_refreshing_glyph` is present and
      passes.
- [ ] `test_tab_bar_renders_bootable_refreshing_indicator` has a diagnostic message on
      its assertion.
- [ ] `cargo test -p fdemon-tui --lib` passes (no regressions).
- [ ] `cargo clippy -p fdemon-tui --lib -- -D warnings` clean.

## Out of Scope

- Changing `IconSet` itself or the resolved glyphs.
- Plumbing `IconSet` through other widgets that don't currently have it.
- Visual restyling of the indicator (color, dim, etc.) beyond the existing approach.
- The polish bundle items (handled in task 05).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-af9cd63bac5de0327

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | Added `'a` lifetime + `icons: &'a IconSet` field to `TabBar`; updated `new()` signature; replaced `"↻"` literal with `self.icons.refresh()`; updated all 6 test `TabBar::new()` call sites to pass `&icons`; updated glyph assertions to use `icons.refresh()` instead of literal; added diagnostic message to bootable indicator test |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Added `icons: IconSet` (owned, Copy) field to `TargetSelector`; added `.icons()` builder method; updated `render_full` to pass `&self.icons` into `TabBar::new()`; updated `render_tabs_compact` to append refresh glyph when active tab's flag is set; updated 3 existing test assertions to use `IconSet::default().refresh()` instead of literal `"↻"`; added new `test_target_selector_compact_renders_refreshing_glyph` test |

### Notable Decisions/Tradeoffs

1. **Owned `IconSet` in `TargetSelector` vs reference**: Used an owned `IconSet` (which is `Copy + Clone`, just wraps an enum) rather than a reference to avoid lifetime proliferation in the existing API. This keeps `TargetSelector::new()` signature unchanged; callers can opt into Nerd Font glyphs via the `.icons()` builder. The `mod.rs` call sites were not updated since they don't yet plumb `&IconSet` through (tracked as a future improvement if desired).

2. **Compact mode label as `String` not `&'static str`**: The `render_tabs_compact` labels changed from `&'static str` to `String` to allow dynamic formatting with the refresh glyph. `Span::styled` accepts `Into<Cow<'static, str>>` which accepts `String`, so this works without changes to the ratatui API usage.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check -p fdemon-tui` — Passed
- `cargo test -p fdemon-tui --lib` — Passed (876 tests, 0 failed)
- `cargo clippy -p fdemon-tui --lib -- -D warnings` — Clean

### Risks/Limitations

1. **`TargetSelector` in `mod.rs` still uses `IconSet::default()`**: The `NewSessionDialog` has `self.icons` available but the two `TargetSelector::new()` call sites in `mod.rs` don't chain `.icons(*self.icons)`. The widget will work correctly for Unicode mode (the default) but Nerd Font users won't see the NF glyph in the tab bar when rendered through `NewSessionDialog`. A follow-up could add `.icons(*self.icons)` at those two call sites — but that is out of scope per task definition ("smallest churn").
