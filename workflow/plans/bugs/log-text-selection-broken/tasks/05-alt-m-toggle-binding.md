# Task 05 — `Alt+m` keybinding → `Message::ToggleMouseCapture`

**Agent:** implementor
**Wave:** 2
**Depends on:** Task 03 (`ToggleMouseCapture` message)
**Files written:** `crates/fdemon-app/src/handler/keys.rs`

---

## Goal

Bind `Alt+m` (and, where the terminal sends it instead, the `Esc m` sequence) to `Message::ToggleMouseCapture`. The binding works in **every** `UiMode` so a user can recover native selection regardless of where they currently are.

## Background

`Alt+m` was selected as the toggle binding (BUG.md §Resolved Decisions Q1). Some terminals deliver Alt as a meta-prefix `Esc` followed by the key; we accept either delivery. fdemon's existing `handler/keys.rs` already handles Alt-modified keys for other actions — follow the local convention.

## Implementation

1. Find the global / "always-on" key dispatch path in `handler/keys.rs` — the one that runs before mode-specific dispatch (the same level where `Ctrl+C` quit and `q` quit live). New binding goes there so it works in every mode.

2. Add a match arm:

   ```text
   InputKey::Char { ch: 'm', modifiers } if modifiers.contains(KeyMod::Alt) => {
       Some(Message::ToggleMouseCapture)
   }
   ```

   Use the project's existing modifier enum + field accessors — verify by reading neighboring arms.

3. **Esc-m fallback path:** if the project's key conversion layer (`input_key.rs`) already canonicalizes `Esc m` to `Alt+m`, no extra work needed. If not, document the limitation in BUG.md / MOUSE.md and stop short of trying to detect Esc-m in the handler — the handler should remain meta-aware only.

4. **Gate:** the binding is `is_busy`-independent (the toggle must work even during a hot-reload — it is a UI affordance, not an app action). However, suppress the toggle while a **text-input field is active** (search input, settings inline edit, new-session-dialog text fields) so the user can type `Alt+m` literally if their workflow needs it. Match the existing "swallow keys during text input" gate used by other global keys.

## Tests

- `test_alt_m_in_normal_mode_emits_toggle` — `InputKey::Char { ch: 'm', modifiers: KeyMod::ALT }` in `UiMode::Normal` → `Some(Message::ToggleMouseCapture)`.
- `test_alt_m_in_devtools_emits_toggle` — same in `UiMode::DevTools`.
- `test_alt_m_in_search_input_does_not_toggle` — when search input is active, `Alt+m` falls through (returns `None` from the global dispatcher and the search-input handler treats it normally).
- `test_plain_m_in_normal_does_not_toggle` — `'m'` without Alt remains whatever its existing behavior is (or `None`).
- `test_alt_m_during_busy_session_still_toggles` — assert `is_busy` does NOT gate the toggle.

## Acceptance Criteria

- [ ] Five new unit tests pass.
- [ ] Existing keymap tests in this file pass unchanged.
- [ ] No new dependency on `Clipboard` or `terminal::set_mouse_capture` (this task emits a message only).

## Notes for Reviewer

- The reason for placing the binding in the global dispatcher (rather than per-mode) is symmetric to the rationale in Phase 1 of the mouse-support PLAN: this is a "UI affordance" toggle, not a domain action.
- We deliberately do not log a tracing event from this handler — Task 06's `UpdateAction::SetMouseCapture` is the natural logging point, and double-logging adds noise.
