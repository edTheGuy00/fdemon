# Task 08 — Status-bar mouse indicator via `StatusInfo`

**Agent:** implementor
**Wave:** 3
**Depends on:** Task 03 (`mouse_capture_active` field on `AppState`)
**Files written:**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`
- `crates/fdemon-tui/src/render/mod.rs`

---

## Goal

Render a compact `[mouse]` / `[mouse-off]` badge in the existing bottom metadata bar (the `StatusInfo` strip rendered by `LogView`). Reflects `AppState::mouse_capture_active` so the user can see the current state of capture and learn the `Alt+m` toggle exists.

## Background

The status bar at the bottom of the log view is built in `render/mod.rs:201` as a `widgets::StatusInfo` struct (`widgets/log_view/mod.rs:37`), then attached to the `LogView` via `with_status(...)`. Adding a new field on `StatusInfo` plus a render branch in the log-view widget is the minimal change.

## Implementation

1. In `widgets/log_view/mod.rs`, locate `pub struct StatusInfo<'a>` at line 37. Add:

   ```text
   pub mouse_capture_active: bool,
   ```

   Default value when constructing it: read from `state.mouse_capture_active` at the `render/mod.rs:201` build site.

2. In the same widget, find the function that renders the `StatusInfo` strip (search for `impl StatusInfo` or `fn render_status` / similar). Append a `Span` to the right-most cluster:

   ```text
   if status.mouse_capture_active {
       Span::styled("[mouse]",  Style::default().fg(theme.dim_fg))
   } else {
       Span::styled("[mouse-off]", Style::default().fg(theme.warning_fg))
   }
   ```

   Place it adjacent to the existing keymap hints (`[q] Quit ...`) on the right side. Match the existing spacing convention used by neighboring spans.

3. In `render/mod.rs:201`, when building the `StatusInfo`, set `mouse_capture_active: state.mouse_capture_active`.

4. **Width-pressure fallback:** the status bar may be space-constrained on 80-col terminals. Drop the badge gracefully when the remaining width is too tight: prefer the existing keymap hints over the badge. Use the same width-aware truncation pattern the file already employs (look for `width.saturating_sub` or `truncate` in this file).

## Tests

- `test_status_info_renders_mouse_on_badge` — `StatusInfo { mouse_capture_active: true, .. }` produces a span list containing `[mouse]`.
- `test_status_info_renders_mouse_off_badge` — same with false → contains `[mouse-off]`.
- `test_status_info_drops_badge_when_width_too_narrow` — set width to 40 cols, badge is omitted but keymap hints remain.
- Render-snapshot test if the file already has one for `StatusInfo` — extend the existing snapshot to cover the badge.

## Acceptance Criteria

- [ ] Three new unit tests pass.
- [ ] Existing render snapshot tests pass (regenerate where the badge legitimately changes the snapshot).
- [ ] No theme additions in `crate::theme` — reuse `dim_fg` / `warning_fg` (or whatever the existing palette names are; verify).
- [ ] Badge is **always present** at widths ≥ 80 cols when there's room; verified by snapshot.

## Notes for Reviewer

- We deliberately do *not* add a separate "status_bar.rs" widget. The existing `StatusInfo` strip already plays that role.
- `[mouse-off]` uses warning color (yellow/orange) to signal "you're in a non-default state" — discoverable cue that points the user toward `Alt+m`.
- The badge is rendered even in demo mode (where `mouse_capture_active = false` per `AppState`'s init logic); the warning color in demo is a benign cosmetic.
