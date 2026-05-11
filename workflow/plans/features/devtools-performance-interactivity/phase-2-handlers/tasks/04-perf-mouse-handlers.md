## Task: Mouse Handlers for Performance Tab Interactivity

**Objective**: Add `Message` routing for mouse-region-emitted actions that reuse the keyboard handlers from task 03.

**Depends on**: 03-perf-keyboard-handlers (handler functions to reuse)

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/update.rs`: Confirm `PerfFocusSection` and `PerfSelectAllocRow { index }` are routed to the handlers from task 03 (no new handlers needed — mouse just emits the same `Message`s).
- `crates/fdemon-app/src/handler/devtools/performance.rs`: Verify the existing handlers cover the mouse cases or extend them if needed (likely no-op).
- (Widget-side region registration is Phase 3 — this task is just confirming the handler API supports both keyboard and mouse-emitted messages.)

**Files Read (Dependencies):**
- Phase 1 + 03 outputs.
- `docs/CODE_STANDARDS.md` "Region Registry Pattern" — for context on what region emission looks like.

### Details

This task is essentially a handler-side audit: the Phase 3 widget changes will register `MouseRegion`s that emit `PerfFocusSection(...)` / `PerfSelectAllocRow { ... }`. Those messages must route correctly through `update.rs` whether emitted by keyboard or mouse.

Verify:
1. `Message::PerfFocusSection(PerfSection::MemoryList)` from a mouse click on the memory list results in `state.focused_section = MemoryList` — same outcome as `Tab` keyboard press.
2. `Message::PerfSelectAllocRow { index: Some(3) }` from a mouse click sets both `alloc_table_selected_row = Some(3)` AND `focused_section = MemoryList`.
3. `Message::PerfSelectAllocRow { index: None }` from a click outside any row clears the selection but does not change focus.

If any of the above doesn't already hold from task 03, extend the handler accordingly.

### Acceptance Criteria

1. The keyboard-emitted and mouse-emitted message paths produce identical state mutations.
2. `Message::PerfSelectAllocRow { index: Some(_) }` always sets `focused_section = MemoryList`.
3. Unit tests cover both message origins (no new code path; just additional assertions).
4. `cargo test --workspace` passes.

### Testing

```rust
#[test]
fn perf_focus_section_via_mouse_or_keyboard_yields_same_state() {
    let mut state_keyboard = AppState::test_default();
    let mut state_mouse = AppState::test_default();
    state_keyboard.add_test_session_in_devtools_performance();
    state_mouse.add_test_session_in_devtools_performance();

    // Keyboard-style dispatch
    update(&mut state_keyboard, Message::PerfFocusSection(PerfSection::MemoryChart));
    // Mouse-style dispatch (same message)
    update(&mut state_mouse, Message::PerfFocusSection(PerfSection::MemoryChart));

    assert_eq!(
        active_perf_state(&state_keyboard).unwrap().focused_section,
        active_perf_state(&state_mouse).unwrap().focused_section
    );
}

#[test]
fn perf_select_alloc_row_with_some_focuses_memory_list() {
    let mut state = AppState::test_default();
    state.add_test_session_in_devtools_performance();
    update(&mut state, Message::PerfSelectAllocRow { index: Some(2) });
    let perf = active_perf_state(&state).unwrap();
    assert_eq!(perf.alloc_table_selected_row, Some(2));
    assert_eq!(perf.focused_section, PerfSection::MemoryList);
}
```

### Notes

- This is a light-weight task; most work happens at the widget layer in Phase 3.
- The mouse region registry pattern (CODE_STANDARDS.md "Region Registry Pattern") guarantees regions are recreated every frame; no persistence concerns at the handler layer.
