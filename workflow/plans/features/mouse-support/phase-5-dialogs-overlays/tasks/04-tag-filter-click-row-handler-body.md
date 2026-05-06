## Task: Tag-Filter Click-Row Handler Body

**Objective**: Replace the stub `Message::TagFilterClickRow { index } => UpdateResult::none()` arm in `handler/update.rs` with the real implementation: set `tag_filter_ui.selected_index = index` AND toggle the visibility of the tag at that index, in a single arm. No chained follow-up message — single click both navigates and toggles, matching the PLAN.md UX decision.

**Depends on**: 01 (the `Message::TagFilterClickRow` variant and stub arm must already exist)

**Estimated Time**: 0.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/update.rs`: Replace the stub arm body for `Message::TagFilterClickRow { index }` with the real implementation.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs::TagFilterUiState` (for `selected_index`).
- `crates/fdemon-app/src/session/native_tags.rs::NativeTagState` (for `sorted_tags`, `toggle_tag`, `tag_count`).
- `crates/fdemon-app/src/handler/update.rs::Message::TagFilterToggleSelected` (existing arm, ~lines 2456–2472, used as a template — the new arm is essentially "set index, then run the toggle logic").

### Details

The new arm body mirrors the existing `TagFilterToggleSelected` arm but uses the click's absolute `index` instead of `tag_filter_ui.selected_index`:

```rust
// Click on a tag row in the tag filter overlay.
//
// Sets `tag_filter_ui.selected_index = index` (so the selection follows the
// click target — useful if the user keyboard-arrows after the click) AND
// toggles the visibility of the tag at that index.
//
// Single click both navigates and toggles by design. See Phase 5 PLAN.md
// notes for the UX rationale (tag-filter overlay has no useful "selected
// but not toggled" state).
//
// `index` is clamped to the valid tag range. If the index is out of range
// (which shouldn't happen because the widget only registers regions for
// visible rows), the toggle is a no-op.
Message::TagFilterClickRow { index } => {
    let tag_count = state
        .session_manager
        .selected()
        .map(|h| h.native_tag_state.tag_count())
        .unwrap_or(0);

    if tag_count == 0 || index >= tag_count {
        return UpdateResult::none();
    }

    state.tag_filter_ui.selected_index = index;

    if let Some(session_id) = state.session_manager.selected_id() {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            // Collect the tag name at the clicked index before mutably
            // borrowing the session manager. Mirrors the `TagFilterToggleSelected`
            // arm's pattern (~ update.rs:2461).
            let tag_name: Option<String> = handle
                .native_tag_state
                .sorted_tags()
                .get(index)
                .map(|(tag, _)| tag.to_string());
            if let Some(tag) = tag_name {
                handle.native_tag_state.toggle_tag(&tag);
            }
        }
    }
    UpdateResult::none()
}
```

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — the new tests below are added and pass.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. `Message::TagFilterClickRow { index: 2 }` sets `tag_filter_ui.selected_index = 2`.
5. `Message::TagFilterClickRow { index: 2 }` toggles the visibility of the third tag in `sorted_tags()` order.
6. Out-of-range `index` (`>= tag_count`) is a no-op (no panic, no mutation).
7. When no session is selected, the arm is a no-op (matches the `TagFilterToggleSelected` arm's behaviour).

### Testing

Add unit tests inside `handler/tests.rs` (or a new `tag_filter_click_tests` module). The tests follow the pattern established by the existing `TagFilterToggleSelected` tests in `handler/tests.rs`:

```rust
#[test]
fn click_row_sets_selection_and_toggles_visibility() {
    let mut state = AppState::new();
    let id = state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    let handle = state.session_manager.get_mut(id).unwrap();

    // Discover three tags.
    handle.native_tag_state.observe_tag("alpha");
    handle.native_tag_state.observe_tag("beta");
    handle.native_tag_state.observe_tag("gamma");
    assert!(handle.native_tag_state.is_tag_visible("beta"));

    // Click row 1 (sorted: alpha=0, beta=1, gamma=2).
    let result = handler::update::update(&mut state, Message::TagFilterClickRow { index: 1 });

    let handle = state.session_manager.get(id).unwrap();
    assert_eq!(state.tag_filter_ui.selected_index, 1);
    assert!(!handle.native_tag_state.is_tag_visible("beta"), "beta toggled hidden");
    assert!(handle.native_tag_state.is_tag_visible("alpha"), "alpha unchanged");
    assert!(handle.native_tag_state.is_tag_visible("gamma"), "gamma unchanged");
    assert!(result.message.is_none(), "no follow-up message");
}

#[test]
fn click_row_with_out_of_range_index_is_no_op() {
    let mut state = AppState::new();
    let id = state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    let handle = state.session_manager.get_mut(id).unwrap();
    handle.native_tag_state.observe_tag("alpha");

    let initial_selected = state.tag_filter_ui.selected_index;
    let result = handler::update::update(&mut state, Message::TagFilterClickRow { index: 99 });

    assert_eq!(state.tag_filter_ui.selected_index, initial_selected);
    let handle = state.session_manager.get(id).unwrap();
    assert!(handle.native_tag_state.is_tag_visible("alpha"), "alpha unchanged");
    assert!(result.message.is_none());
}

#[test]
fn click_row_with_no_session_is_no_op() {
    let mut state = AppState::new();
    let result = handler::update::update(&mut state, Message::TagFilterClickRow { index: 0 });
    assert!(result.message.is_none());
    // selected_index might still be set from the click since the widget
    // could be rendering a stale registry; however, with `tag_count == 0`
    // we return early before mutating selected_index. Verify:
    assert_eq!(state.tag_filter_ui.selected_index, 0);
}

#[test]
fn click_row_double_toggles_back() {
    // Two clicks on the same row toggle off, then on — proving the
    // "single click is single toggle" semantic.
    let mut state = AppState::new();
    let id = state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    let handle = state.session_manager.get_mut(id).unwrap();
    handle.native_tag_state.observe_tag("alpha");
    assert!(handle.native_tag_state.is_tag_visible("alpha"));

    let _ = handler::update::update(&mut state, Message::TagFilterClickRow { index: 0 });
    assert!(!state.session_manager.get(id).unwrap().native_tag_state.is_tag_visible("alpha"));

    let _ = handler::update::update(&mut state, Message::TagFilterClickRow { index: 0 });
    assert!(state.session_manager.get(id).unwrap().native_tag_state.is_tag_visible("alpha"));
}
```

### Notes

- **Why no double-click logic.** The PLAN.md and Phase 5 TASKS.md notes both call out that the tag-filter overlay's "select-without-toggle" state has no useful UX. A user who clicks a tag wants the visibility to flip. Settings is different — single-click select gives the user a chance to read the description before deciding to edit.
- **Why we set `selected_index` even though the toggle effect is what the user wanted.** Keyboard navigation after a click should resume from the clicked row, not from wherever the keyboard was last. Mirrors how clicking a list row in any IDE moves the focus.
- **Why we don't `UpdateResult::message(Message::TagFilterToggleSelected)` as a follow-up.** It would work, but adds an extra round-trip through the message bus and an extra render frame between the index-set and the toggle. Inline implementation keeps the click visually atomic.
- **Why no test for "select_index defaults to 0 when tag_count == 0 and we click index 0".** That path is the no-op early-return — `selected_index` is *not* updated when `tag_count == 0`, even for `index = 0`. Locked in by the third test above.
