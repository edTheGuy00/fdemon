# Task 06 — Target Selector Passes Refreshing Flags into TabBar

**Agent:** implementor
**Phase:** 2
**Depends on:** 02 (refreshing flags), 05 (TabBar accepts flags)
**Files Modified (Write):** `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`

**Files Read:**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`
- `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`

---

## Goal

Update the `TargetSelector` widget so it passes
`self.state.refreshing` and `self.state.bootable_refreshing` into the new four-arg
`TabBar::new()` signature. This is the final wire-up — once merged, the indicator
shows up end-to-end when the dialog is opened with cached data.

## Steps

1. Open `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`.

2. **Locate every `TabBar::new(...)` call** in this file (grep — there are at least
   one or two: the main render and possibly compact-render paths). The main one is
   around line 92:

   ```rust
   let tab_bar = TabBar::new(self.state.active_tab, self.is_focused);
   tab_bar.render(chunks[0], buf);
   ```

3. **Update each call site** to pass the two new flags:

   ```rust
   let tab_bar = TabBar::new(
       self.state.active_tab,
       self.is_focused,
       self.state.refreshing,
       self.state.bootable_refreshing,
   );
   tab_bar.render(chunks[0], buf);
   ```

4. **Audit other call sites** in this file. Ensure every `TabBar::new(...)` call is
   updated. Run a quick grep before finishing:

   ```
   grep -n "TabBar::new" crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs
   ```

5. **Update existing widget tests** in this file if any of them rely on a specific
   tab-bar structure (most tests in this file render via `TargetSelector::render`, so
   they exercise the wired-up behaviour automatically — no change needed beyond a
   compile fix if a test directly constructs a `TabBar`).

6. **Add an integration-style render test** that proves the indicator surfaces from
   state through the widget:

   ```rust
   #[test]
   fn test_target_selector_renders_refreshing_glyph_when_state_set() {
       use ratatui::{backend::TestBackend, Terminal};

       let mut state = TargetSelectorState::default();
       state.set_connected_devices(vec![/* one device, see existing helpers */]);
       state.refreshing = true;

       let backend = TestBackend::new(60, 6);
       let mut terminal = Terminal::new(backend).unwrap();
       terminal
           .draw(|f| {
               let selector = TargetSelector::new(&state, true, &Default::default());
               selector.render(f.area(), f.buffer_mut());
           })
           .unwrap();
       let rendered: String = terminal
           .backend()
           .buffer()
           .content()
           .iter()
           .map(|cell| cell.symbol())
           .collect::<Vec<_>>()
           .join("");
       assert!(
           rendered.contains("↻"),
           "expected refresh glyph in target selector, got: {rendered}"
       );
   }
   ```

   Adjust the constructor / test-helper imports based on the actual signatures in this
   file (use existing tests like `test_set_connected_devices` for reference patterns).

## Acceptance Criteria

- [ ] All `TabBar::new(...)` call sites in `target_selector.rs` pass four arguments,
      including `self.state.refreshing` and `self.state.bootable_refreshing`.
- [ ] `cargo build --workspace` succeeds.
- [ ] Existing `target_selector` tests still pass.
- [ ] New integration-style render test passes.
- [ ] `cargo test -p fdemon-tui --lib` passes.

## Out of Scope

- Modifying the `TabBar` widget itself (task 05).
- Adding new state fields (task 02).
