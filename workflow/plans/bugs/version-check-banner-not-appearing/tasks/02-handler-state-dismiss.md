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
