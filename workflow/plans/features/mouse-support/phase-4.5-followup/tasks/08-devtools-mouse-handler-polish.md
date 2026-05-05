# Task 08: DevTools Mouse Handler Polish

## Goal

Three focused changes in `crates/fdemon-app/src/handler/mouse/devtools.rs`:
1. Carve out the DevTools sub-tab bar from the network filter-input gate so a click on `[i]/[p]/[n]` while filter input is active switches the panel AND exits filter input mode (Minor #11).
2. Add a unit test for middle-click behavior to close the test gap from Phase 4 task 05's CONCERN finding (Minor #16).
3. Replace `matches!(button, MouseButton::Right)` with direct equality `button == MouseButton::Right` (Minor #25).

## Background

- **Filter-gate trap**: When `network.filter_input_active = true` and `active_panel == Network`, `handle_press` currently absorbs *all* clicks — including clicks on the sub-tab bar. A mouse-only user typing in the filter is trapped: clicking `[i] Inspector` does nothing. The user must press Esc on the keyboard to escape. This violates "mouse fully usable" UX principle.

- **Middle-click test gap**: Phase 4 task 05's validator returned CONCERN because no unit test exercises `MouseButton::Middle` resolving an `on_middle` action. The production code at `devtools.rs:61` correctly maps `MouseButton::Middle => entry.on_middle.as_ref()`, but the contract is undocumented in tests.

- **`matches!` style**: `if matches!(button, MouseButton::Right) { return None; }` is equivalent to `if button == MouseButton::Right { return None; }` (since `MouseButton` derives `PartialEq`). The latter is more direct.

## Files

**Modify:**
- `crates/fdemon-app/src/handler/mouse/devtools.rs`

**Read (reference):**
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — sub-tab bar `[i]/[p]/[n]` rect registration (Phase 4 task 02 wired this)
- `crates/fdemon-app/src/handler/keys.rs` — keyboard-side filter-input handling for parity reference

## Plan

1. **Replace the `matches!` wrapper** with direct equality:
   ```rust
   if button == MouseButton::Right {
       return None;
   }
   ```
   No behavior change; idiomatic improvement.

2. **Carve out sub-tab clicks from the filter-input gate.** Currently:
   ```rust
   if state.devtools_view_state.active_panel == DevToolsPanel::Network {
       let filter_active = /* ... */;
       if filter_active {
           return None;
       }
   }
   // ... hit_test ...
   ```

   Refactor to consult the registry first, then check whether the matched action is a sub-tab switch (which should always pass through), or a Network-internal action (which should be gated):

   ```rust
   // Hit-test first to learn what message would fire.
   let guard = state.mouse_regions.take_guard();
   let hit = guard.hit_test(x, y, button);
   let message = hit.and_then(|entry| entry.action.resolve(x, y, button))?;

   // If we're in Network panel with filter input active, suppress all messages
   // EXCEPT a SwitchDevToolsPanel — clicks on the sub-tab bar always escape the
   // filter and switch panel (the act of switching panels invalidates the filter
   // context, so we also exit filter input mode below).
   if state.devtools_view_state.active_panel == DevToolsPanel::Network {
       let filter_active = state.devtools_view_state.network.filter_input_active;
       if filter_active && !matches!(*message, Message::SwitchDevToolsPanel(_)) {
           return None;
       }
       if filter_active && matches!(*message, Message::SwitchDevToolsPanel(_)) {
           // Caller's update() handler for SwitchDevToolsPanel will exit filter input
           // mode, OR we emit a chained message here. Prefer chaining via UpdateResult
           // in the SwitchDevToolsPanel handler — this dispatcher only emits one
           // message per click.
       }
   }

   Some(message)
   ```

   The cleanest factoring depends on the existing dispatcher shape. Two approaches:

   **Approach A:** This dispatcher emits the `SwitchDevToolsPanel` message; the `update()` arm for `SwitchDevToolsPanel` checks if `network.filter_input_active` was true and clears it as part of the panel switch. Single-message dispatch; cleanup happens at update-time.

   **Approach B:** This dispatcher emits a chained message (e.g., `UpdateResult::message(SwitchDevToolsPanel(_))` plus a follow-up `ExitNetworkFilterInput`). More mechanical but explicit.

   **Prefer Approach A.** It keeps the dispatcher's contract simple ("returns the message that the click resolves to") and centralizes the cleanup in the message handler. Modify `Message::SwitchDevToolsPanel`'s `update()` arm in `handler/update.rs` (or wherever `handle_switch_devtools_panel` lives) to clear `network.filter_input_active = false` if the message switches AWAY from Network.

   **Note:** Modifying the SwitchDevToolsPanel handler would touch a file outside this task's declared scope. To keep scope clean, do the carve-out in `handle_press` only — emit `SwitchDevToolsPanel` past the filter gate, but ALSO clear `network.filter_input_active` directly in the dispatcher before returning the message:

   ```rust
   if state.devtools_view_state.active_panel == DevToolsPanel::Network {
       let filter_active = state.devtools_view_state.network.filter_input_active;
       if filter_active {
           if matches!(*message, Message::SwitchDevToolsPanel(_)) {
               // Sub-tab click escapes the filter. Clear filter input mode as part
               // of the click action. (The SwitchDevToolsPanel update handler
               // doesn't know about the click context, so we mutate here.)
               state.devtools_view_state.network.filter_input_active = false;
           } else {
               return None; // Suppress non-tab clicks while filter is active.
           }
       }
   }

   Some(message)
   ```

   This keeps all logic in `devtools.rs::handle_press`. The `take_guard()` already has `&mut MouseRegions`; the dispatcher receives `&mut AppState`, so writing to `network.filter_input_active` is in-scope.

3. **Add the middle-click test.** In the existing `press_tests` module:

   ```rust
   #[test]
   fn middle_click_on_recorded_region_returns_middle_action() {
       let mut state = AppState::new(/* fixture */);
       state.devtools_view_state.active_panel = DevToolsPanel::Inspector;

       // Register a click_left_middle region that emits different messages
       // for left vs middle.
       {
           let mut regions = state.mouse_regions.take_guard();
           let mut builder = regions.builder();
           builder.click_left_middle(
               MouseRect::new(10, 5, 5, 1),
               MouseAction::emit(Message::DevToolsInspectorSelectRow { index: 3 }),
               MouseAction::emit(Message::DevToolsInspectorToggleNode { index: 3 }),
               0,
           );
       }

       let result = handle_press(&mut state, 12, 5, MouseButton::Middle, /* mods */);
       assert!(matches!(
           result,
           Some(msg) if matches!(*msg, Message::DevToolsInspectorToggleNode { index: 3 })
       ));
   }
   ```

   Adapt to the existing test fixture style. The exact `click_left_middle` API may have a different name; check `mouse_regions.rs::MouseRegionsBuilder` for the multi-button registration method.

4. **Add a sub-tab-bar carve-out test:**

   ```rust
   #[test]
   fn network_filter_active_sub_tab_click_switches_panel_and_clears_filter() {
       let mut state = AppState::new(/* fixture */);
       state.devtools_view_state.active_panel = DevToolsPanel::Network;
       state.devtools_view_state.network.filter_input_active = true;

       // Register a SwitchDevToolsPanel(Inspector) region.
       {
           let mut regions = state.mouse_regions.take_guard();
           let mut builder = regions.builder();
           builder.click(
               MouseRect::new(0, 0, 14, 1), // sub-tab bar area
               MouseAction::emit(Message::SwitchDevToolsPanel(DevToolsPanel::Inspector)),
               0,
           );
       }

       let result = handle_press(&mut state, 7, 0, MouseButton::Left, /* mods */);
       assert!(matches!(
           result,
           Some(msg) if matches!(*msg, Message::SwitchDevToolsPanel(DevToolsPanel::Inspector))
       ));
       assert!(!state.devtools_view_state.network.filter_input_active,
           "sub-tab click while filter active must clear filter_input_active");
   }
   ```

## Acceptance Criteria

- [ ] `matches!(button, MouseButton::Right)` replaced with `button == MouseButton::Right`.
- [ ] Sub-tab `SwitchDevToolsPanel` clicks while network filter is active are NOT suppressed; non-tab clicks ARE suppressed.
- [ ] Sub-tab carve-out also clears `network.filter_input_active` so the user is no longer trapped.
- [ ] New test `middle_click_on_recorded_region_returns_middle_action` passes.
- [ ] New test `network_filter_active_sub_tab_click_switches_panel_and_clears_filter` passes.
- [ ] All existing tests in `press_tests` still pass.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.

## Notes

- **Do not touch** `handler/update.rs` or `widgets/devtools/mod.rs` in this task. If you find that the cleanest factoring requires modifying the `SwitchDevToolsPanel` handler in `update.rs`, prefer the in-dispatcher mutation approach instead (mutating `network.filter_input_active` directly in `handle_press`). Recording this decision in the Completion Summary is fine.
- The middle-click test exists primarily to lock in the contract that `MouseButton::Middle` resolves to the `on_middle` field of a `click_left_middle` registration. The Phase 4 production code already has this behavior; the test makes it discoverable and regression-proof.
- If the dispatcher's signature uses `&AppState` rather than `&mut AppState`, the in-dispatcher mutation approach won't work. In that case, fall back to chaining a `Message::ExitNetworkFilterInput` (or similar) — but verify the dispatcher's mutability first by reading the current `mod.rs::handle_press` signature.
