# Task 04 — Thread `IconSet` from `NewSessionDialog` to `TargetSelector` (F5)

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`

---

## Goal

Fix Major finding F5 from PR #37's Copilot review: `TargetSelector::new()` defaults
its internal `icons` field to `IconSet::default()` (Unicode). The two
`TargetSelector::new()` call sites in `widgets/new_session_dialog/mod.rs` do not
chain `.icons(*self.icons)`, so the `NewSessionDialog`'s configured `IconSet` (which
the user set to Nerd Fonts via `.fdemon/config.toml`) is dropped. Result: Nerd Fonts
users see the Unicode `↻` glyph in tab labels even though `TabBar` correctly
resolves the Nerd Fonts glyph from a properly-configured `IconSet`.

## Context

`NewSessionDialog` already holds an `&IconSet` (see
`widgets/new_session_dialog/mod.rs:158-182`):

```rust
pub struct NewSessionDialog<'a> {
    state: &'a NewSessionDialogState,
    tool_availability: &'a ToolAvailability,
    icons: &'a IconSet,
}
```

`TargetSelector` exposes a builder method for the icon set (see
`widgets/new_session_dialog/target_selector.rs:49-56`):

```rust
/// Set the icon set for this widget (builder pattern).
///
/// Callers that have a configured `IconSet` (e.g. from `NewSessionDialog`)
/// should pass it here to ensure Nerd Font glyphs are used when configured.
pub fn icons(mut self, icons: IconSet) -> Self {
    self.icons = icons;
    self
}
```

The two un-chained call sites in `mod.rs`:

- **Line 329 (horizontal layout):**

  ```rust
  let target_selector = TargetSelector::new(
      &self.state.target_selector,
      self.tool_availability,
      target_focused,
  );
  target_selector.render(chunks[0], buf);
  ```

- **Line 551 (vertical layout):**

  ```rust
  let target_selector = TargetSelector::new(
      &self.state.target_selector,
      self.tool_availability,
      target_focused,
  )
  .compact(target_compact);
  target_selector.render(chunks[2], buf);
  ```

`IconSet` is `Copy` (verified by inspecting `theme/icons.rs` — confirm before
chaining; if it's not `Copy`, use `.clone()` instead of `*`). The fix is to chain
`.icons(*self.icons)` (or `.icons(self.icons.clone())`) to both call sites.

## Steps

1. **Inspect `theme/icons.rs`** to confirm `IconSet` is `Copy`. If yes, chain
   `.icons(*self.icons)`. If no, chain `.icons(self.icons.clone())` (acceptable
   given it's tiny — a few enum-variant-like fields).

2. Open `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` and locate the
   horizontal-layout call (around line 329). Add the `.icons(...)` chain:

   ```rust
   let target_selector = TargetSelector::new(
       &self.state.target_selector,
       self.tool_availability,
       target_focused,
   )
   .icons(*self.icons);  // or .clone() if IconSet is not Copy
   target_selector.render(chunks[0], buf);
   ```

3. Locate the vertical-layout call (around line 551) and add the chain there too:

   ```rust
   let target_selector = TargetSelector::new(
       &self.state.target_selector,
       self.tool_availability,
       target_focused,
   )
   .icons(*self.icons)
   .compact(target_compact);
   target_selector.render(chunks[2], buf);
   ```

   The order of `.icons()` and `.compact()` is irrelevant since both return `Self`,
   but keep `.icons()` first for consistency with the horizontal layout.

4. **Add a lock-in render test.** The test must construct a `NewSessionDialog`
   with a non-default `IconSet` (one that produces a *different* refresh glyph
   from `IconSet::default()`) and assert the rendered output contains the
   non-default glyph. This proves the icon set flows through to the tab bar.

   Pseudocode (the implementor should consult `theme/icons.rs` for the actual
   constructor — likely something like `IconSet::nerd_fonts()` or a struct literal
   `IconSet { mode: IconMode::NerdFonts, .. }`):

   ```rust
   #[test]
   fn test_new_session_dialog_threads_iconset_to_target_selector() {
       use crate::theme::icons::{IconMode, IconSet};

       // Build a non-default icon set whose refresh glyph differs from the
       // default.
       let nerd_icons = IconSet { mode: IconMode::NerdFonts /* + any other fields */ };
       assert_ne!(
           nerd_icons.refresh(),
           IconSet::default().refresh(),
           "test setup error: nerd_icons.refresh() must differ from default"
       );

       let mut state = NewSessionDialogState::default();
       state.target_selector.refreshing = true;  // ensure the glyph is rendered

       let tool_availability = ToolAvailability::default();
       let dialog = NewSessionDialog::new(&state, &tool_availability, &nerd_icons);

       // Render into a buffer sized for full (horizontal) layout
       let area = Rect::new(0, 0, 120, 30);
       let mut buf = Buffer::empty(area);
       dialog.render(area, &mut buf);

       let rendered = buffer_to_string(&buf);  // use existing helper if present
       assert!(
           rendered.contains(nerd_icons.refresh()),
           "expected NerdFonts refresh glyph in rendered tabs, got: {rendered}"
       );
       assert!(
           !rendered.contains(IconSet::default().refresh()),
           "default Unicode glyph must NOT appear when NerdFonts is configured, got: {rendered}"
       );
   }
   ```

   Notes for the implementor:
   - Inspect existing tests in `mod.rs` and `target_selector.rs` for the
     buffer-to-string helper and `Rect`/`Buffer` setup pattern.
   - If `IconMode::NerdFonts.refresh()` happens to equal Unicode (i.e. they share
     a refresh glyph), pick a different glyph (e.g. `IconSet { mode: IconMode::Ascii }`
     if that exists and produces a distinct refresh value) and assert on that.
     The point is: the test must distinguish "default" from "configured."
   - If no non-default `IconSet` constructor produces a distinct refresh glyph,
     STOP and report — the test as designed cannot prove the bug is fixed and
     the fix becomes unverifiable from the TUI layer. (This would be unusual:
     `theme/icons.rs:96` is documented to expose distinct Nerd Font and Unicode
     refresh glyphs.)

5. Add a similar test for the **vertical (compact) layout** if practical: render
   into a buffer that triggers the vertical layout, configure
   `bootable_refreshing = true`, assert the configured glyph appears. If the
   compact-layout test is too plumbing-heavy, a single horizontal-layout test is
   acceptable as a lock-in (the compact path uses the same threading and the
   compile-time chain is verified by the source change itself).

6. Run verification:
   - `cargo fmt --all`
   - `cargo check -p fdemon-tui`
   - `cargo test -p fdemon-tui --lib`
   - `cargo clippy -p fdemon-tui --lib -- -D warnings`

## Acceptance Criteria

- [ ] Both `TargetSelector::new()` call sites in
      `widgets/new_session_dialog/mod.rs` (horizontal at ~line 329, vertical at
      ~line 551) chain `.icons(*self.icons)` (or `.clone()`).
- [ ] New lock-in test
      `test_new_session_dialog_threads_iconset_to_target_selector` (or
      similarly-named) constructs a `NewSessionDialog` with a non-default
      `IconSet` and asserts the configured (non-Unicode) refresh glyph appears in
      the rendered output.
- [ ] The lock-in test also asserts the default Unicode glyph does **not** appear
      when a non-default `IconSet` is configured (proves the threading is real,
      not a fallback).
- [ ] `cargo test -p fdemon-tui --lib` passes.
- [ ] `cargo clippy -p fdemon-tui --lib -- -D warnings` clean.

## Out of Scope

- Making `IconSet` a required constructor argument on `TargetSelector::new()`
  (per BUG.md, this is rejected to minimize test churn in `target_selector.rs`).
- Modifying `TargetSelector` itself or its `.icons()` builder.
- Threading `IconSet` to other child widgets (`LaunchContextWithDevice` already
  receives `self.icons` per `mod.rs:346` and `mod.rs:570` — verify, don't touch).
- Rewriting the `IconSet` type or its constructors.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a96dd1e525b9e7ff5

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Added `.icons(*self.icons)` chain to both `TargetSelector::new()` call sites (horizontal at line ~329 and vertical at line ~551); added two lock-in render tests |

### Notable Decisions/Tradeoffs

1. **IconSet is Copy**: Confirmed `IconSet` derives `Copy` (line 13 of `theme/icons.rs`), so used `*self.icons` (dereference copy) instead of `.clone()`.

2. **Vertical test ordering**: `set_bootable_devices()` resets `bootable_refreshing` to `false` (line 254 of `target_selector_state.rs`), so the test must set `bootable_refreshing = true` AFTER calling `set_bootable_devices()` — not before. Initial test had the order reversed, causing the glyph to not appear.

3. **Two lock-in tests**: Added both horizontal and vertical tests as the task specified both paths. The vertical test uses `bootable_refreshing = true` with `active_tab = Bootable` to distinguish the NerdFonts glyph from Unicode in the compact tab bar rendered by `render_tabs_compact`.

4. **LaunchContextWithDevice verified untouched**: Both call sites at ~line 346 and ~line 570 already chain `self.icons` to `LaunchContextWithDevice::new()` — confirmed out of scope, not modified.

### Testing Performed

- `cargo fmt --all` - Passed (auto-formatted)
- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui --lib` - Passed (878 tests, 2 new)
- `cargo clippy -p fdemon-tui --lib -- -D warnings` - Passed (clean)

### Risks/Limitations

1. **NerdFonts glyph in test terminal**: The NerdFonts glyph `\u{f021}` is from the Private Use Area. The test correctly checks for its presence using `rendered.contains(nerd_refresh)` which works regardless of terminal font support since it operates on the raw string data in the Ratatui `TestBackend` buffer.
