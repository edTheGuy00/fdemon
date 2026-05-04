## Task: Log-View Click Handlers (Single + Double Click)

**Objective**: Fill in the bodies of `handle_click_log_row` and `handle_toggle_stack_trace_for_entry` in `handler/log_view.rs`. Single-click records `AppState::last_log_click`. A second click on the same `entry_id` within 400 ms emits a follow-up `Message::ToggleStackTraceForEntry { entry_id }` via `UpdateResult::message`. The follow-up handler delegates to `Session::toggle_stack_trace`.

**Depends on**: Task 01 (the `Message` variants, the `LogClickStamp` type, and stub functions must already exist)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/log_view.rs`: Replace the two stub function bodies added in Task 01 with real implementations. Add a `#[cfg(test)] mod tests` (or extend the existing one) covering the single/double-click semantics.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/update.rs::UpdateResult` (for `UpdateResult::none()`, `UpdateResult::message(...)`)
- `crates/fdemon-app/src/state.rs::LogClickStamp`
- `crates/fdemon-app/src/session/session.rs::toggle_stack_trace` (the existing helper used by the keyboard `c` handler)
- `crates/fdemon-app/src/handler/keys.rs:255` (for the existing `Message::ToggleStackTrace` emission pattern, kept unchanged)

### Details

#### `handle_click_log_row`

```rust
/// Handle a click on a single log-view row.
///
/// Tracks consecutive clicks in `state.last_log_click`. When the same
/// `entry_id` is clicked twice within [`DOUBLE_CLICK_WINDOW`], emits a
/// follow-up [`Message::ToggleStackTraceForEntry`] and clears the stamp so
/// a *third* click within the window does not chain another toggle.
///
/// `frame_index` is currently informational — Phase 4 v1 does not act on
/// stack-frame double-click (the natural action would be "open the link"
/// but that overlaps with the existing `LinkHighlight` mode). The field is
/// included in the message so future work can act on it without another
/// `Message` variant.
pub fn handle_click_log_row(
    state: &mut AppState,
    entry_id: u64,
    _frame_index: Option<usize>,
) -> UpdateResult {
    let now = std::time::Instant::now();

    let is_double = state.last_log_click.is_some_and(|prev| {
        prev.entry_id == entry_id
            && now.saturating_duration_since(prev.at) <= DOUBLE_CLICK_WINDOW
    });

    if is_double {
        // Consume the stamp so a third click within the window doesn't chain.
        state.last_log_click = None;
        return UpdateResult::message(Message::ToggleStackTraceForEntry { entry_id });
    }

    state.last_log_click = Some(LogClickStamp {
        entry_id,
        at: now,
    });
    UpdateResult::none()
}

/// Window within which two consecutive clicks on the same row count as a
/// double click. 400 ms matches GNOME / KDE / macOS default double-click
/// thresholds and is short enough that an accidental re-click doesn't
/// trigger an unwanted stack-trace toggle.
const DOUBLE_CLICK_WINDOW: std::time::Duration = std::time::Duration::from_millis(400);
```

#### `handle_toggle_stack_trace_for_entry`

```rust
/// Toggle stack trace expansion for the explicit `entry_id`.
///
/// Distinct from [`Message::ToggleStackTrace`], which targets the
/// scroll-focused entry — that handler already exists at
/// `handler/update.rs:682` and stays unchanged. This sibling handler is
/// emitted only by [`handle_click_log_row`] on double-click.
pub fn handle_toggle_stack_trace_for_entry(
    state: &mut AppState,
    entry_id: u64,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        let default_collapsed = state.settings.ui.stack_trace_collapsed;
        handle
            .session
            .toggle_stack_trace(entry_id, default_collapsed);
    }
    UpdateResult::none()
}
```

#### Borrow ordering

Reading `state.settings.ui.stack_trace_collapsed` after `selected_mut()` requires care — `selected_mut` borrows `state.session_manager` mutably, but `state.settings` is a sibling field, so the disjoint-borrow rules allow it. Mirror the exact pattern from the existing `Message::ToggleStackTrace` arm (`handler/update.rs:682-690`):

```rust
if let Some(handle) = state.session_manager.selected_mut() {
    let default_collapsed = state.settings.ui.stack_trace_collapsed;
    handle.session.toggle_stack_trace(entry_id, default_collapsed);
}
```

If the borrow checker complains, lift `default_collapsed` out before `selected_mut`.

### Acceptance Criteria

1. `handle_click_log_row` updates `state.last_log_click` to `Some(LogClickStamp { entry_id, at: <now> })` on a single click on a row that does not match the previous stamp.
2. A second click on the same `entry_id` within 400 ms returns `UpdateResult::message(ToggleStackTraceForEntry { entry_id })` and clears `state.last_log_click`.
3. A second click on a *different* `entry_id` within 400 ms is treated as a fresh single click — `last_log_click` is overwritten with the new entry, no follow-up emitted.
4. A second click on the same `entry_id` *outside* 400 ms is treated as a fresh single click — same overwrite, no follow-up.
5. `handle_toggle_stack_trace_for_entry` invokes `handle.session.toggle_stack_trace(entry_id, settings.ui.stack_trace_collapsed)` for the currently selected session. No-op when no session is selected.
6. `frame_index` is accepted but unused in v1. The argument name is `_frame_index` (or `frame_index` with a `let _ = frame_index;` line) to avoid clippy warnings.
7. `cargo test --workspace` passes including ≥ 4 new tests below. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo check` all pass.

### Testing

Add to `handler/log_view.rs` tests (or create `#[cfg(test)] mod tests` if absent):

```rust
#[cfg(test)]
mod click_handler_tests {
    use super::*;
    use crate::handler::update::update;
    use crate::message::Message;

    fn fresh_state() -> AppState {
        // Assumes existing `AppState::new()` and a helper to create one
        // session (mirroring patterns already in this file or
        // handler/devtools/inspector.rs).
        AppState::new()
    }

    #[test]
    fn single_click_records_stamp_and_emits_no_followup() {
        let mut state = fresh_state();
        let result = handle_click_log_row(&mut state, /*entry_id=*/ 42, None);
        assert!(result.message.is_none(), "single click does not emit follow-up");
        assert!(state.last_log_click.is_some(), "stamp recorded");
        assert_eq!(state.last_log_click.unwrap().entry_id, 42);
    }

    #[test]
    fn second_click_same_entry_within_window_emits_toggle() {
        let mut state = fresh_state();
        let _ = handle_click_log_row(&mut state, 42, None);
        let result = handle_click_log_row(&mut state, 42, None);
        assert!(matches!(
            result.message,
            Some(Message::ToggleStackTraceForEntry { entry_id: 42 })
        ));
        assert!(state.last_log_click.is_none(), "stamp consumed by double-click");
    }

    #[test]
    fn second_click_different_entry_is_treated_as_fresh_single() {
        let mut state = fresh_state();
        let _ = handle_click_log_row(&mut state, 42, None);
        let result = handle_click_log_row(&mut state, 43, None);
        assert!(result.message.is_none());
        assert_eq!(state.last_log_click.unwrap().entry_id, 43);
    }

    #[test]
    fn third_click_within_window_does_not_chain_double() {
        // A → B → A pattern: third click on A should NOT immediately re-toggle,
        // because the stamp was cleared by the A → A double-click consumption.
        let mut state = fresh_state();
        let _ = handle_click_log_row(&mut state, 42, None);
        let _ = handle_click_log_row(&mut state, 42, None); // double-click → clears stamp
        let result = handle_click_log_row(&mut state, 42, None);
        assert!(result.message.is_none(), "third click is a fresh single click");
    }

    #[test]
    fn second_click_after_window_is_treated_as_fresh_single() {
        let mut state = fresh_state();
        // Manually plant a stamp older than the window.
        state.last_log_click = Some(LogClickStamp {
            entry_id: 42,
            at: std::time::Instant::now() - std::time::Duration::from_millis(500),
        });
        let result = handle_click_log_row(&mut state, 42, None);
        assert!(result.message.is_none(), "outside window → no double-click");
    }

    #[test]
    fn toggle_stack_trace_for_entry_no_op_without_session() {
        let mut state = AppState::new();
        let result = handle_toggle_stack_trace_for_entry(&mut state, 42);
        assert!(result.message.is_none());
    }
}
```

### Notes

- **Why `saturating_duration_since`.** `Instant::duration_since` panics if the argument is later than `self`. `saturating_duration_since` returns `Duration::ZERO` instead, which always lies inside the 400 ms window — that is the correct behaviour: a click whose timestamp slightly drifted ahead is still a same-window click.
- **400 ms threshold.** GNOME (default 400 ms), KDE (400 ms), macOS (~500 ms by default). 400 ms is the lowest common denominator and matches the value the PLAN.md sketched (line 275). Documented as a `const` so future tuning is one-line.
- **Why clear the stamp on double-click consumption.** Otherwise a third click within the window would re-trigger the toggle, which feels like a stutter to the user. Clearing on consumption gives a clean "click → single-click record; click → double, expand; click → single-click record again" cadence.
- **Why `frame_index` is unused in v1.** Clicking a stack-frame line could plausibly: (a) open the link via the existing link-highlight extraction, (b) toggle just that frame, (c) do nothing. Each choice has UX implications and overlaps with link mode. Defer to a future enhancement; carry the field through the message so the deferred work doesn't need a new variant.
- **No new test for the `update.rs` dispatch arm.** Task 01 wired the arm to delegate; the integration test in Task 10 exercises it end-to-end via `update(&mut state, Message::ClickLogRow { ... })`.
- **Multi-session safety.** `state.last_log_click` is a single `Option`, so switching sessions and clicking again may double-click against a stamp from the previous session. This is acceptable: the stamp's `entry_id` is checked, and entry IDs are unique across the global ID space (`LogEntry::id` is monotonic per process). A click on entry 42 in session A followed by entry 42 in session B is exceedingly unlikely; even if it happens, expanding a stack trace in session B is the user's intent, not a bug. If it becomes a real problem, scope `last_log_click` per session in a future patch.
