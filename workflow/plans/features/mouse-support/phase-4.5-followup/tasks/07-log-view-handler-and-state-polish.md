# Task 07: Log View Handler & State Polish

## Goal

Three focused fixes in the log-view click flow:
1. Clear `last_log_click` on session switch to prevent cross-session entry_id collisions (Minor #8).
2. Resolve the `DOUBLE_CLICK_WINDOW` boundary inconsistency between docstring ("within 400ms" implying exclusive) and `<=` operator (inclusive) (Minor #17).
3. Convert `_frame_index` parameter rationale into a `// TODO(phase-5):` comment so the deferred-use intent is explicit and trackable (Minor #23).

## Background

- **Cross-session collision**: `AppState::last_log_click` stores `entry_id` from the previous click for double-click detection. If the user switches sessions between clicks, the `entry_id` from session A could collide with an entry in session B (ids are issued per-session from a shared atomic counter — collisions are theoretically possible). This would trigger a spurious `ToggleStackTraceForEntry` on what the user intended as a single click on the new session's row.
- **Boundary inconsistency**: `DOUBLE_CLICK_WINDOW = 400ms` with comparison `now.saturating_duration_since(prev.at) <= DOUBLE_CLICK_WINDOW` means a click at exactly 400ms is still treated as a double-click. The docstring says "within 400ms" which most readers interpret as exclusive. Hairline distinction in practice but should be consistent.
- **`_frame_index` underscore**: Phase 4 task 03 accepts `frame_index: Option<usize>` in the click handler signature but ignores it (`_frame_index`). The doc comment explains why ("included for future use"). Per code-quality review, the leading underscore signals "intentionally unused" and the rationale should be a `TODO` comment so future implementers know the trigger.

## Files

**Modify:**
- `crates/fdemon-app/src/handler/log_view.rs` — boundary clarification, TODO conversion
- `crates/fdemon-app/src/handler/update.rs` — clear `last_log_click` on session-switch arms
- `crates/fdemon-app/src/state.rs` — (optionally) tighten `LogClickStamp` to include a `session_id` field if simpler than clearing in update arms

**Read (reference):**
- `crates/fdemon-app/src/handler/session.rs` — session-switch handler patterns
- `crates/fdemon-app/src/message.rs` — session-switch message variants

## Plan

1. **Decide the cross-session approach.** Two options:

   **Option A (preferred — simpler):** Clear `state.last_log_click = None` in the `update()` arms for `Message::SelectSessionByIndex`, `Message::NextSession`, `Message::PreviousSession`, and `Message::CloseCurrentSession`. The `LogClickStamp` struct stays unchanged.

   **Option B:** Add `session_id: Uuid` (or whatever session-id type is in use) to `LogClickStamp`; compare it in `handle_click_log_row` and treat a mismatch as a fresh first click.

   Prefer Option A: single-line additions in the existing session-switch handlers, no struct change, easier to reason about.

2. **Implement Option A in `handler/update.rs`.** Find each session-switch arm and add:
   ```rust
   Message::SelectSessionByIndex(idx) => {
       state.last_log_click = None;
       // ... existing handler dispatch ...
   }
   ```
   Repeat for `NextSession`, `PreviousSession`, `CloseCurrentSession`. (The exact arm names may differ; grep for `session_manager.select` or similar to find them.)

3. **Resolve the `DOUBLE_CLICK_WINDOW` boundary.** In `crates/fdemon-app/src/handler/log_view.rs`:
   - Decide which is correct. Recommendation: **keep `<=`** (inclusive boundary; matches GNOME/macOS implementations more closely) and update the docstring:
     ```rust
     /// Window within which two consecutive clicks on the same row count as a
     /// double click — *inclusive* of the boundary value (i.e., a click at
     /// exactly 400ms after the previous click is still treated as a double-click).
     /// 400ms matches the GNOME / KDE / macOS double-click defaults.
     const DOUBLE_CLICK_WINDOW: std::time::Duration = std::time::Duration::from_millis(400);
     ```
   - Alternatively, change to `<` and document as exclusive. Either is acceptable as long as comment and operator are consistent.

4. **Convert `_frame_index` rationale into a TODO comment.** Currently the function signature is:
   ```rust
   pub fn handle_click_log_row(
       state: &mut AppState,
       entry_id: u64,
       _frame_index: Option<usize>,
   ) -> UpdateResult {
   ```
   With a doc comment explaining why the parameter is unused. Refactor to:
   ```rust
   pub fn handle_click_log_row(
       state: &mut AppState,
       entry_id: u64,
       frame_index: Option<usize>,
   ) -> UpdateResult {
       // TODO(phase-5): use `frame_index` to dispatch a stack-frame-specific click
       // (e.g., open the source location for the clicked frame instead of toggling
       // the parent entry's stack trace). For now, single-row click behavior is
       // identical regardless of which line within an entry was clicked.
       let _ = frame_index;
       // ... existing body ...
   }
   ```
   This makes the deferred-use intent grep-able (`TODO(phase-5)`) and removes the underscore-prefix anti-pattern. The `let _ = frame_index;` is acceptable here because the comment explains why; the parameter is explicit (not silently ignored via `_frame_index`).

5. **Verify session-switch tests.** If the existing test suite exercises session switching followed by a click, the new clearing logic should be covered. If not, add one test:
   ```rust
   #[test]
   fn click_after_session_switch_does_not_double_click() {
       // 1. Single-click row entry_id = 7 in session A → last_log_click = Some(...)
       // 2. Switch to session B
       // 3. Single-click row entry_id = 7 in session B (collision)
       // Assert: result is single-click semantics (no follow-up message), last_log_click reset to Some(B's stamp).
   }
   ```

## Acceptance Criteria

- [ ] `state.last_log_click` is reset to `None` on every session-switch message handler arm in `handler/update.rs`.
- [ ] `DOUBLE_CLICK_WINDOW` docstring and comparison operator are consistent (both inclusive or both exclusive).
- [ ] `_frame_index` underscore replaced with `frame_index` + `TODO(phase-5):` comment + explicit `let _ = frame_index;` discard line.
- [ ] At least one new test verifies cross-session click behavior (or an existing test extended to cover it).
- [ ] All existing tests pass.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.

## Notes

- **Do not touch** any other file. The other Phase 4.5 tasks each have their own scope. In particular, `handler/devtools/inspector.rs` is owned by Task 02 and `handler/mouse/devtools.rs` is owned by Task 08.
- If you discover during implementation that Option B (struct field) is in fact simpler (e.g., the session-switch arms are scattered across many messages), it's acceptable to switch — but record the decision in the Completion Summary.
- The `let _ = frame_index;` is intentionally explicit. An alternative is `#[allow(unused_variables)]` on the parameter, but `let _ = ...` is more visible at the body level.
