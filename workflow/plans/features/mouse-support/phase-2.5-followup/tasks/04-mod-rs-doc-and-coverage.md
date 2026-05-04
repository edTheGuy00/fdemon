## Task: Document `mod.rs::handle_scroll` and close test coverage gaps

**Objective**: Add a `///` doc comment to the central `handle_scroll` dispatcher in `crates/fdemon-app/src/handler/mouse/mod.rs`, add positive-assertion tests for `UiMode::Settings` and `UiMode::NewSessionDialog` (currently missing from the dispatcher's local test suite), and add `UiMode::EmulatorSelector` to the `test_scroll_no_op_in_non_scrollable_modes` array. All three changes are in the same file, owned exclusively by this task.

**Depends on**: None

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/mod.rs`:
  1. Add a `///` doc comment to the private `handle_scroll` function (currently undocumented despite being the central dispatcher).
  2. Add `UiMode::EmulatorSelector` to the `test_scroll_no_op_in_non_scrollable_modes` array (currently lists only 4 of the 4 truly non-scrollable modes — wait, EmulatorSelector IS in the dispatcher's no-op match arm but missing from the test).
  3. Add `test_scroll_settings_routes_to_settings_prev_item` (or similarly named) asserting `UiMode::Settings` + Up returns `Some(Message::SettingsPrevItem)` through the dispatcher.
  4. Add `test_scroll_new_session_dialog_routes_to_device_up` asserting `UiMode::NewSessionDialog` + Up returns `Some(Message::NewSessionDialogDeviceUp)` through the dispatcher.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/mouse/settings.rs`: Confirm the message routing target for the Settings positive assertion.
- `crates/fdemon-app/src/handler/mouse/new_session.rs`: Confirm the message routing target for the NewSessionDialog positive assertion (depends on `focused_pane` default — `TargetSelector` per `NewSessionDialogState::new()`).

### Details

#### Sub-task A — Doc comment for `handle_scroll`

Today the function reads:

```rust
fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    match state.ui_mode {
        UiMode::Normal => normal::handle_scroll(state, dir, mods),
        UiMode::DevTools => devtools::handle_scroll(state, dir, mods),
        // ...
    }
}
```

It has no `///` doc comment despite being the central dispatcher. Add:

```rust
/// Route a wheel scroll to the appropriate per-mode handler based on
/// `state.ui_mode`.
///
/// Modes with a real scroll surface (`Normal`, `DevTools`, `Settings`,
/// `Startup`/`NewSessionDialog`, `LinkHighlight`, `FlutterVersion`) delegate
/// to their submodule. Modes with no scrollable surface (`SearchInput`,
/// `ConfirmDialog`, `EmulatorSelector`, `Loading`) return `None`.
///
/// Per-mode handlers differ in modifier handling: `Normal`, `LinkHighlight`,
/// and `DevTools/Network` honor `Shift+wheel` for page-step (via
/// `KeyModSet::is_shift_only`); other modes ignore modifiers entirely.
/// See `docs/MOUSE.md` for the full per-mode reference.
fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
```

#### Sub-task B — Add `EmulatorSelector` to no-op test sweep

The current array reads (around `mod tests` in `mod.rs`):

```rust
for mode in [
    UiMode::ConfirmDialog,
    UiMode::Loading,
    UiMode::SearchInput,
] {
    assert_noop(mode, make_scroll_up());
}
```

Wait — verify the actual current state by reading the file. Per the Phase 2 merge resolution, the array should already contain `EmulatorSelector`. If it does, this sub-task is a no-op. If it does not, add `UiMode::EmulatorSelector` to the array. The dispatcher already routes `EmulatorSelector → None` (`mod.rs:43-47`), so the test is a coverage closure, not a behavior change.

#### Sub-task C — Settings positive assertion

Add a new test in `mod.rs::tests`:

```rust
#[test]
fn test_scroll_settings_routes_to_settings_prev_item() {
    // Settings mode (no modal, not editing) routes scroll-up to SettingsPrevItem
    // via the dispatcher. This catches a typo in the dispatcher's match arm
    // that would otherwise route Settings to a different submodule.
    let state = state_in_mode(UiMode::Settings);
    let msg = handle_mouse(&state, make_scroll_up());
    assert!(
        matches!(msg, Some(Message::SettingsPrevItem)),
        "expected SettingsPrevItem for Settings + scroll-up, got {:?}",
        msg
    );
}
```

#### Sub-task D — NewSessionDialog positive assertion

```rust
#[test]
fn test_scroll_new_session_dialog_routes_to_device_up() {
    // NewSessionDialog mode with default focused_pane (TargetSelector) routes
    // scroll-up to NewSessionDialogDeviceUp via the dispatcher.
    let state = state_in_mode(UiMode::NewSessionDialog);
    let msg = handle_mouse(&state, make_scroll_up());
    assert!(
        matches!(msg, Some(Message::NewSessionDialogDeviceUp)),
        "expected NewSessionDialogDeviceUp for NewSessionDialog + scroll-up, got {:?}",
        msg
    );
}
```

If `AppState::new()` sets a different default `focused_pane`, adjust the test setup to explicitly set `state.new_session_dialog_state.focused_pane = DialogPane::TargetSelector` and import `DialogPane`.

### Acceptance Criteria

1. `mod.rs::handle_scroll` carries a `///` doc comment that explains per-mode dispatch and references `KeyModSet::is_shift_only` and `docs/MOUSE.md`.
2. `test_scroll_no_op_in_non_scrollable_modes` (or the equivalently named test) iterates `EmulatorSelector` alongside `ConfirmDialog`, `Loading`, `SearchInput`. (If `EmulatorSelector` is already in the array, this sub-task is satisfied without change.)
3. A new test `test_scroll_settings_routes_to_settings_prev_item` exists and passes.
4. A new test `test_scroll_new_session_dialog_routes_to_device_up` exists and passes.
5. All four existing positive-assertion tests (`test_scroll_normal_mode_returns_scroll_up`, `test_devtools_scroll_routes_to_inspector_nav`, `test_scroll_produces_message_in_link_highlight_mode`, `test_scroll_produces_message_in_flutter_version_mode`) still pass.
6. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

```bash
cargo test -p fdemon-app handler::mouse::tests
cargo test --workspace
```

Expected: 4 + 2 = 6 positive-assertion tests in `mod.rs::tests`, all passing. Plus the no-op test covering 4 modes.

### Notes

- **This is the dispatcher's exclusive editing window.** Per the Phase 2 lesson, every mode-specific implementor edited `mod.rs` and produced 4 merge conflicts. Phase 2.5 isolates `mod.rs` writes to this task only. The other 5 Phase 2.5 tasks must NOT touch `mod.rs`.
- **Why positive assertions for Settings and NewSessionDialog specifically.** The Phase 2 merge-resolved positive-assertion suite covers Normal, DevTools, LinkHighlight, FlutterVersion. A dispatcher-arm typo for Settings or NewSessionDialog would be caught only by the integration suite at `tests.rs:10203`, not by the local `mod.rs` unit suite. Closing the gap makes `mod.rs::tests` self-sufficient as a dispatcher-correctness suite.
- **The integration suite at `tests.rs::mouse_scroll`** already has `mouse_scroll_settings_plain_up_produces_settings_prev_item` and `mouse_scroll_startup_target_selector_down_produces_device_down`, so a dispatcher misroute would still be caught — but unit tests in `mod.rs` are the first line of defense and should cover all wired modes.
- **Cross-task coordination.** Task 06 also modifies `tests.rs` (the integration test file), so there is no overlap with this task. Task 01 modifies `devtools.rs`, which is unrelated to `mod.rs`.

---

## Completion Summary

**Status:** <!-- Done / Blocked / Failed -->
**Branch:** <!-- current branch name -->

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/mod.rs` | <!-- summary --> |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <!-- Rationale -->

### Testing Performed

- `cargo fmt --all -- --check` — Passed/Failed
- `cargo test -p fdemon-app handler::mouse::tests` — Passed/Failed (X tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed/Failed

### Risks/Limitations

None known.
