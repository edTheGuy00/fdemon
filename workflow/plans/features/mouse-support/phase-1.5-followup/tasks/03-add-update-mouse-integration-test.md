## Task: Add integration test for `update(state, Message::Mouse(...))`

**Objective**: Add the missing integration test that Phase 1's success criteria explicitly required: `update(state, Message::Mouse(...))` returns `UpdateResult::none()` and does not mutate state. Phase 1's per-mode `handle_mouse` tests cover the inner function but not the outer `update()` routing.

**Depends on**: Task 01 (rename-click-to-press) — uses `MouseInput::Press` from the start.

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/tests.rs`: Add a new test that constructs an `AppState`, builds a `Message::Mouse(MouseInput::Press { ... })` message, calls `update(&mut state, message)`, and asserts both that the result is `UpdateResult::none()` and that key state fields (`ui_mode`, `phase`) are unchanged.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/input_mouse.rs`: For `MouseInput::Press` constructor and `MouseButton`, `KeyModSet` types.
- `crates/fdemon-app/src/message.rs`: For `Message::Mouse` variant.
- `crates/fdemon-app/src/state.rs`: For `AppState::new()` constructor signature and the `UiMode` / `AppPhase` types being asserted on.
- `crates/fdemon-app/src/handler/update.rs`: To confirm the dispatch path being tested (lines 60–66 routing `Message::Mouse` to `handle_mouse`).

### Details

The Phase 1 plan (`workflow/plans/features/mouse-support/phase-1-foundation/TASKS.md` success criteria) lists this test as required:

> `update(state, Message::Mouse(...))` returns `UpdateResult::none()` and does not mutate state

But only the inner `handle_mouse` per-mode tests were added. A regression that wires `Message::Mouse` to a side effect in `update()` would silently pass CI today.

Add a test of approximately this shape (adjust to actual `AppState::new()` signature and the available `UpdateResult` accessors):

```rust
#[test]
fn test_mouse_message_returns_none_result_and_does_not_mutate_state() {
    let mut state = AppState::new(/* match the existing test pattern in this file */);
    let before_mode = state.ui_mode;
    let before_phase = state.phase;

    let input = MouseInput::Press {
        x: 0,
        y: 0,
        button: MouseButton::Left,
        modifiers: KeyModSet::NONE,
    };
    let result = update(&mut state, Message::Mouse(input));

    assert!(result.message.is_none(), "update should not produce a follow-up message");
    assert!(result.action.is_none(), "update should not request a side effect");
    assert_eq!(state.ui_mode, before_mode, "ui_mode must not change");
    assert_eq!(state.phase, before_phase, "phase must not change");
}
```

If `Message` does not derive `PartialEq`, use `is_none()` rather than `assert_eq!(..., None)`.

If `AppState` exposes more fields whose immutability matters (e.g., `selected_session_id`, `logs.len()`), include reasonable additional snapshots. Keep the test focused — assert the contract, not every possible field.

Optionally, parameterise across each `MouseInput` variant (`Press`, `Release`, `Drag`, `Scroll`) and each `MouseButton` to harden against future regressions in the dispatcher. A single representative case is sufficient to satisfy the Phase 1 success criterion.

### Acceptance Criteria

1. `crates/fdemon-app/src/handler/tests.rs` contains a new test asserting that `update(state, Message::Mouse(...))` returns `UpdateResult::none()` and does not mutate `state.ui_mode` or `state.phase`.
2. The test passes: `cargo test -p fdemon-app test_mouse_message_returns_none_result_and_does_not_mutate_state`.
3. The test uses `MouseInput::Press` (the renamed variant from Task 01), not `Click`.
4. `cargo test --workspace` is fully green.

### Testing

```bash
cargo test -p fdemon-app test_mouse_message_returns_none_result_and_does_not_mutate_state
cargo test --workspace
```

### Notes

- Look at the existing test patterns in `handler/tests.rs` for how `AppState` is constructed and how `update()` is called in this file — match the prevailing style.
- The implementation under test is in `handler/update.rs:60-66`; if you find any logic there that does mutate state for `Message::Mouse`, that itself is a bug and the test should fail — flag it rather than weakening the assertion.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/tests.rs` | Added `test_mouse_message_returns_none_result_and_does_not_mutate_state` test at end of file |

### Notable Decisions/Tradeoffs

1. **Pre-existing overlapping test**: `test_update_mouse_message_is_no_op` already existed at end of file and partially covered the contract (no message, no action, phase unchanged) but lacked the `ui_mode` assertion and the exact name required by the Phase 1 success criteria. Rather than rename/modify the existing test (which would risk surprising future readers), the new required test was appended alongside it. Both now pass and provide complementary coverage.
2. **Inline `use` style**: The `use crate::input_mouse::{...}` import was placed inline inside the test function body, matching the exact pattern of the pre-existing `test_update_mouse_message_is_no_op` test directly above it.

### Testing Performed

- `cargo test -p fdemon-app test_mouse_message_returns_none_result_and_does_not_mutate_state` - Passed (1 test)
- `cargo test --workspace` - Passed (all crates green, 0 failures)
