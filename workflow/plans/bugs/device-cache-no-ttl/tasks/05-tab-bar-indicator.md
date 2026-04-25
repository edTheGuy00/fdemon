# Task 05 — Tab Bar Refreshing Indicator

**Agent:** implementor
**Phase:** 2
**Depends on:** none
**Files Modified (Write):** `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`

---

## Goal

Make `TabBar` accept two refreshing flags (one per tab) and render a small `↻` glyph
appended to the tab label when its flag is set. The indicator should be visible but
unobtrusive — the cached device list remains the primary UI, with the glyph signalling
that an in-place refresh is in flight.

## Steps

1. Open `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`.

2. **Extend the `TabBar` struct** (around line 16):

   ```rust
   pub struct TabBar {
       active_tab: TargetTab,
       pane_focused: bool,
       /// Refresh-in-flight indicator for the Connected tab.
       connected_refreshing: bool,
       /// Refresh-in-flight indicator for the Bootable tab.
       bootable_refreshing: bool,
   }
   ```

3. **Update `TabBar::new()`** to take the two flags:

   ```rust
   impl TabBar {
       pub fn new(
           active_tab: TargetTab,
           pane_focused: bool,
           connected_refreshing: bool,
           bootable_refreshing: bool,
       ) -> Self {
           Self {
               active_tab,
               pane_focused,
               connected_refreshing,
               bootable_refreshing,
           }
       }
   }
   ```

4. **Update the render loop** (around line 49). For each tab, decide whether to append
   the indicator. Keep the existing centered-paragraph layout — append ` ↻` to the
   label string when refreshing. Use the existing `Style` for the label so the glyph
   inherits color/emphasis from the tab's active/inactive state. (Subtle dim styling
   was considered, but inheriting the tab's style keeps the implementation minimal
   and the glyph readable on both active and inactive tabs.)

   ```rust
   for (i, tab) in [TargetTab::Connected, TargetTab::Bootable]
       .iter()
       .enumerate()
   {
       let is_active = *tab == self.active_tab;
       let refreshing = match tab {
           TargetTab::Connected => self.connected_refreshing,
           TargetTab::Bootable => self.bootable_refreshing,
       };

       let label = if refreshing {
           format!("{} ↻", tab.label())
       } else {
           tab.label().to_string()
       };

       // ... existing style logic, unchanged ...

       let paragraph = Paragraph::new(label)
           .style(style)
           .alignment(Alignment::Center);
       paragraph.render(tabs[i], buf);
   }
   ```

   Note: `tab.label()` currently returns a `&'static str` (e.g. `"1 Connected"`). Use
   `format!()` only when refreshing; otherwise pass the `&str` via `.to_string()` (or
   keep the existing `Paragraph::new(label)` taking `&str` and use a `String` only for
   the refreshing branch — both compile).

5. **Update the existing tab-bar tests** (lines 107-145) — they call
   `TabBar::new(TargetTab::Connected, true)`. Add `false, false` for the two new
   flags:

   ```rust
   let tab_bar = TabBar::new(TargetTab::Connected, true, false, false);
   ```

   Apply the same fix to `test_tab_bar_renders_with_bootable_active` and
   `test_tab_bar_unfocused`.

6. **Add new tests:**

   ```rust
   #[test]
   fn test_tab_bar_renders_connected_refreshing_indicator() {
       let backend = TestBackend::new(40, 3);
       let mut terminal = Terminal::new(backend).unwrap();
       terminal
           .draw(|f| {
               let tab_bar = TabBar::new(TargetTab::Connected, true, true, false);
               f.render_widget(tab_bar, f.area());
           })
           .unwrap();
       let buffer = terminal.backend().buffer();
       let rendered: String = buffer
           .content()
           .iter()
           .map(|cell| cell.symbol())
           .collect::<Vec<_>>()
           .join("");
       assert!(
           rendered.contains("↻"),
           "expected refresh glyph on Connected tab, got: {rendered}"
       );
   }

   #[test]
   fn test_tab_bar_renders_bootable_refreshing_indicator() {
       let backend = TestBackend::new(40, 3);
       let mut terminal = Terminal::new(backend).unwrap();
       terminal
           .draw(|f| {
               let tab_bar = TabBar::new(TargetTab::Bootable, true, false, true);
               f.render_widget(tab_bar, f.area());
           })
           .unwrap();
       let buffer = terminal.backend().buffer();
       let rendered: String = buffer
           .content()
           .iter()
           .map(|cell| cell.symbol())
           .collect::<Vec<_>>()
           .join("");
       assert!(rendered.contains("↻"));
   }

   #[test]
   fn test_tab_bar_no_indicator_when_not_refreshing() {
       let backend = TestBackend::new(40, 3);
       let mut terminal = Terminal::new(backend).unwrap();
       terminal
           .draw(|f| {
               let tab_bar = TabBar::new(TargetTab::Connected, true, false, false);
               f.render_widget(tab_bar, f.area());
           })
           .unwrap();
       let buffer = terminal.backend().buffer();
       let rendered: String = buffer
           .content()
           .iter()
           .map(|cell| cell.symbol())
           .collect::<Vec<_>>()
           .join("");
       assert!(!rendered.contains("↻"));
   }
   ```

## Acceptance Criteria

- [ ] `TabBar::new()` takes four arguments: `active_tab`, `pane_focused`,
      `connected_refreshing`, `bootable_refreshing`.
- [ ] When `connected_refreshing` is true, the Connected tab label ends with ` ↻`.
- [ ] When `bootable_refreshing` is true, the Bootable tab label ends with ` ↻`.
- [ ] When both flags are false, no glyph is rendered (existing label behaviour
      unchanged).
- [ ] Existing tab-bar render tests pass (after their `TabBar::new()` calls are updated
      with the two new false arguments).
- [ ] All three new render tests pass.
- [ ] `cargo test -p fdemon-tui --lib` passes.

## Out of Scope

- Updating callers of `TabBar::new()` outside this file (handled in task 06).
- Setting the flags from the handler side (handled in task 04).
