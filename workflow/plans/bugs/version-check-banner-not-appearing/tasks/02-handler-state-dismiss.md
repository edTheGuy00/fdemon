# Task 02 — Store notice unconditionally + dismiss on first keypress

**Agent:** implementor
**Depends on:** —
**Estimated:** 1–2h
**Fixes:** Defect #3a (handler drops the notice outside the dialog)

## Objective

Always record a discovered newer-version notice in `AppState`, regardless of `ui_mode`, and clear
it on the user's first keypress while on a non-dialog screen. (Task 03 adds the render site so the
stored notice actually surfaces for auto-launch users.)

## Files (Write)

- `crates/fdemon-app/src/handler/update.rs`
- `crates/fdemon-app/src/state.rs`

## Background

- `update.rs:384-394`: `Message::NewVersionAvailable` only sets `startup_notice` when
  `is_new_session_dialog_visible()` (matches `NewSessionDialog | Startup` — `state.rs:1755-1756`),
  otherwise drops it. Auto-launch users (`Startup → Loading → Normal`) never satisfy the gate.
- `Message::Key(key)` is handled at `update.rs:55`.
- `startup_notice` is cleared in `hide_new_session_dialog()` (`state.rs:1698`) — keep that.

## Steps

1. **Remove the visibility gate** (`update.rs:384-394`):
   ```rust
   Message::NewVersionAvailable { latest } => {
       // Store unconditionally; the render layer decides where/whether to show it.
       state.startup_notice = Some(StartupNotice::NewVersionAvailable { latest });
       UpdateResult::none()
   }
   ```

2. **Dismiss on first keypress in non-dialog modes.** In the `Message::Key(key)` arm
   (`update.rs:55`), before/after existing dispatch, clear the notice when the user interacts
   outside the startup dialog (where it lives in the dialog chrome and is cleared via
   `hide_new_session_dialog`). Implement a small helper on `AppState`, e.g.:
   ```rust
   // state.rs
   /// Clears the startup notice once the user interacts on a non-dialog screen.
   /// No-op when the New Session Dialog is visible (the dialog owns the notice's
   /// lifecycle and clears it on dismiss).
   pub fn dismiss_startup_notice_on_interaction(&mut self) {
       if self.startup_notice.is_some() && !self.is_new_session_dialog_visible() {
           self.startup_notice = None;
       }
   }
   ```
   Call it at the top of the `Message::Key` handling for `Normal`/`Loading` (i.e. when not in the
   dialog). Keep it minimal — a single call so the banner disappears on the first key the user
   presses after an auto-launch. Do not interfere with dialog key handling.

3. Verify `StartupNotice` import/visibility is already in scope in `update.rs` (it is, since the
   arm already constructs it).

## Tests

- `new_version_available_sets_notice_in_normal_mode` — set `ui_mode = Normal`, dispatch
  `Message::NewVersionAvailable { latest: "0.5.7".into() }`, assert `state.startup_notice ==
  Some(StartupNotice::NewVersionAvailable { latest: "0.5.7" })`.
- `new_version_available_sets_notice_in_loading_mode` — same for `UiMode::Loading`.
- `keypress_clears_notice_in_normal_mode` — with a notice set and `ui_mode = Normal`, dispatch a
  `Message::Key(..)` and assert `startup_notice` becomes `None`.
- `keypress_does_not_clear_notice_in_dialog` — with a notice set and `ui_mode = NewSessionDialog`,
  a keypress does NOT clear it (dialog dismiss path still owns clearing).
- Existing `hide_new_session_dialog_clears_startup_notice` test stays green.

## Acceptance criteria

- [ ] Notice is stored for `NewVersionAvailable` in any `ui_mode`.
- [ ] First keypress on `Normal`/`Loading` clears the notice; dialog keypresses do not.
- [ ] `cargo test -p fdemon-app` green; `cargo clippy -p fdemon-app` clean.

## Out of scope

- Do not add the render site here (Task 03).
- Do not modify the `StartupNotice` enum definition unless strictly required; if you do, note it
  for Task 03 (which reads the type).

---

## Completion Summary

**Status:** Done
**Branch:** fix/version-check-banner-not-appearing

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/update.rs` | Removed visibility gate from `Message::NewVersionAvailable`; stores notice unconditionally. Added `state.dismiss_startup_notice_on_interaction()` call at top of `Message::Key` arm. Replaced old `new_version_available_dropped_when_dialog_not_visible` test with four new tests covering Normal mode set, Loading mode set, keypress-clears-in-Normal, keypress-does-not-clear-in-dialog. |
| `crates/fdemon-app/src/state.rs` | Added `dismiss_startup_notice_on_interaction(&mut self)` helper method. Added three unit tests for the helper covering Normal (clears), NewSessionDialog (no-op), and no-notice (no-op). |

### Notable Decisions/Tradeoffs

1. **Call site for dismiss**: The dismiss call is placed at the very top of the `Message::Key` arm in `update.rs`, before `handle_key` is dispatched. This means the banner is cleared on the first keypress regardless of which key was pressed or what mode-specific action it triggers. This is the minimal, correct behavior per the task spec.
2. **No changes to `StartupNotice` enum**: The enum definition was left untouched — Task 03 can consume it as-is.
3. **Old test renamed**: `new_version_available_dropped_when_dialog_not_visible` was replaced with `new_version_available_sets_notice_in_normal_mode` since the behavior it was testing is now the opposite.

### Testing Performed

- `cargo test -p fdemon-app` — Passed (2935 tests)
- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Render site not yet wired**: This task stores the notice unconditionally but Task 03 must add the render site for auto-launch users (`UiMode::Normal`/`UiMode::Loading`) to actually see the banner. Without Task 03 the fix is latent.
