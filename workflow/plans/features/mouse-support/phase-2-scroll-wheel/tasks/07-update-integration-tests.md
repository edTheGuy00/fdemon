## Task: End-to-end scroll integration tests through `update()`

**Objective**: Add integration tests in `crates/fdemon-app/src/handler/tests.rs` that drive `update(state, Message::Mouse(MouseInput::Scroll {...}))` for each `UiMode` (and key sub-states) and assert the resulting `UpdateResult::message`. This catches dispatcher misrouting that per-submodule unit tests can miss — for example, a typo in `mod.rs::handle_scroll` that sends `Settings` to `new_session::handle_scroll`.

**Depends on**: 02-normal-mode-scroll, 03-devtools-mode-scroll, 04-settings-mode-scroll, 05-new-session-dialog-scroll, 06-simple-modes-scroll

**Estimated Time**: 1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/tests.rs` — Add a new section `// ─ Mouse scroll routing through update() ─` with the integration test cases below. Phase 1.5 Task 03 also adds a similar `Message::Mouse` integration test for the no-op case; this task extends that pattern with real routing assertions.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/update.rs` — Confirm `Message::Mouse(input) => super::mouse::handle_mouse(state, input).map(UpdateResult::message).unwrap_or_else(UpdateResult::none)` shape (lines 60-66 today; this task does not modify update.rs).
- `crates/fdemon-app/src/state.rs` — `UiMode` variants, `DevToolsPanel`, `tag_filter_visible`, `settings_view_state`, `new_session_dialog_state`.
- `crates/fdemon-app/src/input_mouse.rs` — `MouseInput::Scroll`, `ScrollDir`, `KeyModSet`.
- `crates/fdemon-app/src/message.rs` — Every scroll/nav `Message` variant returned by the per-mode handlers.

### Details

The goal is a set of `update()`-level assertions, not a duplicate of the per-submodule unit tests. Each test:

1. Constructs an `AppState` with the right `ui_mode` and any sub-state required.
2. Invokes `update(&mut state, Message::Mouse(MouseInput::Scroll { x: 0, y: 0, direction, modifiers }))`.
3. Asserts the `UpdateResult::message` field matches the expected follow-up `Message`.
4. Asserts no `UpdateResult::action` is produced (scroll never spawns a side-effect action).

Suggested helper:

```rust
fn scroll_input(direction: ScrollDir, modifiers: KeyModSet) -> MouseInput {
    MouseInput::Scroll { x: 0, y: 0, direction, modifiers }
}

fn assert_scroll_routes_to(state: &mut AppState, dir: ScrollDir, mods: KeyModSet, expected: Message) {
    let result = update(state, Message::Mouse(scroll_input(dir, mods)));
    match result.message {
        Some(actual) => assert!(
            std::mem::discriminant(&actual) == std::mem::discriminant(&expected),
            "expected {:?}, got {:?}",
            expected,
            actual
        ),
        None => panic!("expected Some({:?}), got None", expected),
    }
    assert!(result.action.is_none(), "scroll must not produce an action");
}

fn assert_scroll_routes_to_nothing(state: &mut AppState, dir: ScrollDir, mods: KeyModSet) {
    let result = update(state, Message::Mouse(scroll_input(dir, mods)));
    assert!(result.message.is_none(), "expected None, got {:?}", result.message);
    assert!(result.action.is_none());
}
```

`std::mem::discriminant` comparison is used because some `Message` variants carry payloads (e.g. `NetworkNavigate(NetworkNav::Up)`); for those, prefer `assert!(matches!(...))` patterns to verify both the variant and the inner enum value. Use the discriminant-only helper for unit-style messages like `ScrollUp`.

### Acceptance Criteria

The new `mod tests` block contains at least 12 distinct test cases covering:

1. `UiMode::Normal` (no tag filter), `Up` no mods → `Message::ScrollUp`.
2. `UiMode::Normal` (no tag filter), `Down` Shift-only → `Message::PageDown`.
3. `UiMode::Normal` (`tag_filter_visible == true`), `Up` no mods → `Message::TagFilterMoveUp`.
4. `UiMode::DevTools`, `Inspector` panel, `Down` no mods → `Message::DevToolsInspectorNavigate(InspectorNav::Down)` (full inner-variant match).
5. `UiMode::DevTools`, `Performance` panel, `Up` Shift-only → `None`.
6. `UiMode::DevTools`, `Network` panel (filter inactive), `Up` no mods → `Message::NetworkNavigate(NetworkNav::Up)`.
7. `UiMode::DevTools`, `Network` panel (filter inactive), `Down` Shift-only → `Message::NetworkNavigate(NetworkNav::PageDown)`.
8. `UiMode::Settings` (no modal, not editing), `Up` no mods → `Message::SettingsPrevItem`.
9. `UiMode::FlutterVersion`, `Down` no mods → `Message::FlutterVersionDown`.
10. `UiMode::LinkHighlight`, `Up` Shift-only → `Message::PageUp`.
11. `UiMode::Startup`, TargetSelector pane, `Down` no mods → `Message::NewSessionDialogDeviceDown`.
12. `UiMode::SearchInput`, any wheel input → no message (sanity: explicit no-op modes route through dispatcher correctly).

Additional tests are welcome (especially modal-precedence cases for Settings dart-defines or NewSessionDialog fuzzy modal) — the per-submodule tests cover those at unit level, but a smoke test through `update()` for at least one modal case strengthens the dispatcher contract.

The test module also asserts that the `Press`, `Release`, and `Drag` variants are still no-ops in every mode (already covered by the existing `mod.rs` tests + Phase 1.5 Task 03 — confirm here that integration paths agree).

### Testing

```bash
cargo test -p fdemon-app handler::tests::mouse_scroll
cargo test -p fdemon-app handler::tests
cargo test --workspace
```

The workspace-wide test run is the load-bearing check — Phase 2 success criteria require it to be green.

### Notes

- **Why integration tests on top of per-submodule unit tests.** The per-submodule tests (Tasks 02–06) verify routing logic inside each submodule. They do NOT verify that `handle_mouse` → `mod.rs::handle_scroll` → submodule wiring is correct. A dispatcher typo (e.g. `UiMode::Settings` accidentally routed to `new_session::handle_scroll`) would pass every unit test and fail integration. This task closes that gap.
- **`std::mem::discriminant` vs `matches!`.** Use whichever reads more clearly per case. For `Message::ScrollUp` (no payload), discriminant is fine. For `NetworkNavigate(NetworkNav::PageDown)`, prefer `matches!(actual, Message::NetworkNavigate(NetworkNav::PageDown))` to verify the inner enum.
- **No new `Message` variants verified.** Confirm `cargo check --workspace` shows zero new variants in `message.rs` between Phase 1.5 HEAD and Phase 2 HEAD. The integration tests reference only existing variants.
- **Why this task depends on every Wave-2 task.** The integration tests assert routing for every mode; if any submodule is still a stub returning `None`, the corresponding test case fails. Run last.
- **`tests.rs` line growth.** This file already contains ~hundreds of tests. Append a clearly-marked section at the end rather than weaving cases into existing test groups, to keep the diff easy to review.
