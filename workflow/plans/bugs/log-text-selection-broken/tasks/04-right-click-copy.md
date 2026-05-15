# Task 04 — Right-click on log row copies line; right-click elsewhere toasts

**Agent:** implementor
**Wave:** 2
**Depends on:** Task 02 (clipboard trait), Task 03 (`CopyLogEntryToClipboard` message)
**Files written:** `crates/fdemon-app/src/handler/mouse.rs`

---

## Goal

In `handler/mouse.rs`, extend the `Normal`-mode (and any other mode that displays log rows) right-click branch:

- If the click coordinates fall on a registered log-row region, emit `Message::CopyLogEntryToClipboard { entry_id }` (where `entry_id` comes from the region's stored row→entry mapping established in Phase 4 of the mouse-support feature — `MouseAction::Emit(Message::ClickLogRow { entry_id, .. })`).
- Otherwise, push a dedup-by-text `ToastLevel::Info` toast: `"Right-click copies log lines; nothing to copy here."` Do not emit a follow-up message.

Right-click in modes that don't show logs (Settings, DevTools sub-panels, dialogs) gets the same fallback toast.

## Background

The mouse-support feature registers each visible log row as a clickable region whose `MouseAction::Emit` is `Message::ClickLogRow { entry_id, frame_index }` (Phase 4 Drift A, BUG.md context). For right-click, we want to **reuse the same hit-test** but emit a different message. Two implementation shapes:

- **Option A (preferred)** — extend `MouseRegionEntry` to optionally store an `on_right_click: MouseAction`. Logging-row regions pass `Some(MouseAction::Emit(Message::CopyLogEntryToClipboard { entry_id }))`. Hit-test branches on the button.
- **Option B** — keep the current `MouseRegionEntry` shape; in `handler::mouse`, hit-test the registry and if the resulting `Message` is `ClickLogRow { entry_id, .. }` and the button is Right, *rewrite* it to `CopyLogEntryToClipboard { entry_id }`.

Option A is cleaner but requires touching `mouse_regions.rs` and every widget that registers a log-row region. Option B is a one-file change in `handler/mouse.rs`. **Choose Option B** for this fix — it minimizes write-overlap with Phase 4 code and keeps the registry's public shape stable.

## Implementation

1. In `handler/mouse.rs`, locate the click-dispatch branch that handles `MouseInput::Click` (currently button-aware only for `Left` + `Middle`).
2. Add a `Right` arm that runs the same hit-test as `Left` *with the same z-ordering*, then:

   - If the resulting `MouseAction::Emit(Message::ClickLogRow { entry_id, .. })` matches → return `Some(Message::CopyLogEntryToClipboard { entry_id })`.
   - Otherwise → call `state.push_toast(ToastLevel::Info, "Right-click copies log lines; nothing to copy here.")` and return `None`.

3. **Dedup**: before pushing the toast, scan `state.toasts` for one with the same text already present and skip if found — prevents stacking on rapid right-clicks.

4. Right-click in `UiMode::Settings`, `DevTools`, `NewSessionDialog`, etc.: same fallback toast path. The mode-dispatcher already routes by `UiMode`; add the right-click arm in the **shared** dispatcher path (above the mode switch) so it covers every mode uniformly.

## Tests

- `test_right_click_on_log_row_emits_copy_message` — set up `AppState` with one log row's region registered, simulate `MouseInput::Click { button: Right, ... }` over it, assert `Message::CopyLogEntryToClipboard { entry_id }` with the correct id.
- `test_right_click_off_log_row_pushes_toast` — click outside any log region, assert one toast added with the expected text.
- `test_right_click_in_settings_mode_pushes_toast` — same fallback applies when not in `Normal` mode.
- `test_right_click_dedup` — two consecutive right-clicks off log rows, only one toast in `state.toasts`.

## Acceptance Criteria

- [ ] Four new unit tests pass.
- [ ] Existing left-click / middle-click / scroll tests continue to pass unchanged.
- [ ] No changes to `mouse_regions.rs` or widget registration sites.
- [ ] Toast text is exactly `"Right-click copies log lines; nothing to copy here."` (the doc/test assertions both reference this string — keep them in sync).

## Notes for Reviewer

- The clipboard write itself happens in Task 06's update-handler arm — this task only emits the message. That preserves "handler returns message" purity (Task 04 has no `Clipboard` dependency at all).
- If the click coordinates land on a *non-log* clickable region (e.g., a header bracket), the current behavior is "ignore Right button on that region." Right-click on header brackets still emits the toast (the hit-test does not match `ClickLogRow`, so it falls through). Confirmed acceptable per the Q2 resolution.
