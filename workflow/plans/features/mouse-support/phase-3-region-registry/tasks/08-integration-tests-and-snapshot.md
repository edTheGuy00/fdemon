## Task: End-to-end Integration Tests and Render-Level Snapshots

**Objective**: Drive `Message::Mouse(Press { ... })` through the full TEA loop to confirm header/tab clicks produce the expected effects (HotReload, session selection, session close, dialog open). Lock in render-time region snapshots that catch span-text drift in `header.rs` and `tabs.rs`.

**Depends on**: 05, 06, 07

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/tests.rs`: End-to-end click tests (`Message::Mouse(Press)` → `update()` → assert state mutation).
- `crates/fdemon-tui/src/render/tests.rs`: Snapshot test on the populated registry contents at 80×24, 120×24, and with 1, 3, 9 sessions. Replace the placeholder "regions empty" test from Task 04.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs`: For type imports.
- `crates/fdemon-app/src/handler/mouse/normal.rs`: To match the busy-gate behavior.
- `crates/fdemon-tui/src/widgets/header.rs`, `widgets/tabs.rs`: Source of truth for what regions should appear.

### Details

#### End-to-end handler tests

These tests live in `handler/tests.rs` and exercise the full `update(state, Message::Mouse(...))` path. The goal is to confirm that the registry built by render → click coords → `Message::HotReload` (etc.) → `update()` produces the expected state change.

The trick is populating the registry without running the TUI render path (since `handler/tests.rs` lives in `fdemon-app` and cannot depend on `fdemon-tui`). Solution: populate the registry manually in the test setup, mirroring what render would produce.

```rust
#[cfg(test)]
mod mouse_phase3_tests {
    use super::*;
    use crate::input_mouse::{KeyModSet, MouseButton, MouseInput};
    use crate::mouse_regions::{MouseAction, MouseRect};

    fn populate_header_shortcuts(state: &mut AppState) {
        // Mirror what widgets/header.rs registers: 6 bracketed shortcuts.
        let mut regions = state.mouse_regions.take();
        let mut b = regions.builder();
        b.click(MouseRect::new(10, 0, 2, 1), MouseAction::Emit(Message::HotReload));
        b.click(MouseRect::new(15, 0, 2, 1), MouseAction::Emit(Message::HotRestart));
        b.click(MouseRect::new(20, 0, 2, 1), MouseAction::Emit(Message::CloseCurrentSession));
        b.click(MouseRect::new(25, 0, 2, 1), MouseAction::Emit(Message::EnterDevToolsMode));
        b.click(MouseRect::new(30, 0, 2, 1), MouseAction::Emit(Message::ToggleDap));
        b.click(MouseRect::new(35, 0, 2, 1), MouseAction::Emit(Message::RequestQuit));
        state.mouse_regions.set(regions);
    }

    fn make_left_press(x: u16, y: u16) -> Message {
        Message::Mouse(MouseInput::Press {
            x, y,
            button: MouseButton::Left,
            modifiers: KeyModSet::NONE,
        })
    }

    fn make_middle_press(x: u16, y: u16) -> Message {
        Message::Mouse(MouseInput::Press {
            x, y,
            button: MouseButton::Middle,
            modifiers: KeyModSet::NONE,
        })
    }

    #[test]
    fn click_on_q_emits_request_quit_and_quits_when_no_running_sessions() {
        let mut state = AppState::new();
        state.settings.behavior.confirm_quit = false;
        populate_header_shortcuts(&mut state);

        let result = update(&mut state, make_left_press(35, 0));

        // The mouse handler returned RequestQuit; update() processed it as a
        // follow-up message via UpdateResult::message(...), which the engine
        // would loop back into update(). Simulate that loop here.
        if let Some(follow_up) = result.message {
            update(&mut state, follow_up);
        }

        assert!(state.should_quit(), "click on [q] should quit");
    }

    #[test]
    fn click_on_r_when_busy_is_no_op() {
        let mut state = AppState::new();
        let id = state
            .session_manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();
        state.session_manager.get_mut(id).unwrap().session.mark_started("app".into());
        state.session_manager.get_mut(id).unwrap().session.start_reload();
        assert!(state.session_manager.any_session_busy(), "precondition");
        populate_header_shortcuts(&mut state);

        let result = update(&mut state, make_left_press(10, 0));

        // No follow-up message expected (busy gate returns None inside the
        // mouse handler, so update() returns UpdateResult::none()).
        assert!(result.message.is_none(), "busy gate blocks reload click");
        assert!(result.action.is_none());
    }

    #[test]
    fn click_outside_any_region_is_no_op() {
        let mut state = AppState::new();
        populate_header_shortcuts(&mut state);

        let result = update(&mut state, make_left_press(200, 200));

        assert!(result.message.is_none());
        assert!(result.action.is_none());
    }

    #[test]
    fn middle_click_on_tab_closes_that_session() {
        let mut state = AppState::new();
        let id1 = state
            .session_manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();
        let id2 = state
            .session_manager
            .create_session(&test_device("d2", "Pixel"))
            .unwrap();
        let id3 = state
            .session_manager
            .create_session(&test_device("d3", "Web"))
            .unwrap();
        state.session_manager.select_by_id(id2);

        // Manually populate the tab registry: three tabs at known coords.
        let mut regions = state.mouse_regions.take();
        let mut b = regions.builder();
        b.click_left_middle(
            MouseRect::new(0, 0, 14, 1),
            MouseAction::Emit(Message::SelectSessionByIndex(0)),
            MouseAction::Emit(Message::CloseSessionAt(0)),
        );
        b.click_left_middle(
            MouseRect::new(17, 0, 14, 1),
            MouseAction::Emit(Message::SelectSessionByIndex(1)),
            MouseAction::Emit(Message::CloseSessionAt(1)),
        );
        b.click_left_middle(
            MouseRect::new(34, 0, 14, 1),
            MouseAction::Emit(Message::SelectSessionByIndex(2)),
            MouseAction::Emit(Message::CloseSessionAt(2)),
        );
        state.mouse_regions.set(regions);

        // Middle-click tab 0 (iPhone). Expect Pixel/Web to remain.
        let result = update(&mut state, make_middle_press(0, 0));
        if let Some(follow_up) = result.message {
            update(&mut state, follow_up);
        }

        assert_eq!(state.session_manager.len(), 2);
        assert!(state.session_manager.get(id1).is_none(), "iPhone closed");
        assert!(state.session_manager.get(id2).is_some(), "Pixel preserved");
        assert!(state.session_manager.get(id3).is_some(), "Web preserved");
        assert_eq!(state.session_manager.selected_id(), Some(id2), "selection follows id");
    }

    #[test]
    fn left_click_on_device_pill_opens_new_session_dialog() {
        let mut state = AppState::new();
        state
            .session_manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();

        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(60, 0, 20, 1),
            MouseAction::Emit(Message::OpenNewSessionDialog),
        );
        state.mouse_regions.set(regions);

        let result = update(&mut state, make_left_press(65, 0));
        if let Some(follow_up) = result.message {
            update(&mut state, follow_up);
        }

        assert!(state.is_new_session_dialog_visible());
    }
}
```

#### Render-level snapshot test

In `crates/fdemon-tui/src/render/tests.rs`, replace the placeholder test from Task 04 with a real snapshot:

```rust
#[test]
fn view_populates_header_shortcut_regions_at_120x24() {
    use fdemon_app::{AppState, MouseAction, Message};
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();

    // Collect just the bracketed-shortcut regions (width 2, on title row).
    let title_y = 1; // Inside the glass-block border at y=1.
    let shortcut_msgs: Vec<String> = regions
        .iter()
        .filter(|e| e.rect.width == 2 && e.rect.y == title_y)
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(m)) => Some(format!("{:?}", m)),
            _ => None,
        })
        .collect();

    // Expected: HotReload, HotRestart, CloseCurrentSession, EnterDevToolsMode,
    // ToggleDap, RequestQuit — in that left-to-right order.
    assert_eq!(shortcut_msgs.len(), 6, "exactly six shortcut regions");
    assert!(shortcut_msgs[0].contains("HotReload"));
    assert!(shortcut_msgs[1].contains("HotRestart"));
    assert!(shortcut_msgs[2].contains("CloseCurrentSession"));
    assert!(shortcut_msgs[3].contains("EnterDevToolsMode"));
    assert!(shortcut_msgs[4].contains("ToggleDap"));
    assert!(shortcut_msgs[5].contains("RequestQuit"));
}

#[test]
fn view_populates_tab_regions_with_three_sessions() {
    use fdemon_app::{AppState, MouseAction, Message};
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    for (id_str, name) in [("d1", "iPhone"), ("d2", "Pixel"), ("d3", "Web")] {
        state
            .session_manager
            .create_session(&crate::test_utils::test_device(id_str, name))
            .unwrap();
    }

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let tab_regions: Vec<_> = regions
        .iter()
        .filter(|e| matches!(
            e.on_left,
            Some(MouseAction::Emit(Message::SelectSessionByIndex(_)))
        ))
        .collect();
    assert_eq!(tab_regions.len(), 3, "three tabs → three regions");
    for entry in &tab_regions {
        assert!(matches!(
            entry.on_middle,
            Some(MouseAction::Emit(Message::CloseSessionAt(_)))
        ), "middle-click bound");
    }
}

#[test]
fn view_records_no_header_shortcuts_in_settings_mode() {
    // Settings mode renders the SettingsPanel as a full-screen overlay.
    // The header is NOT rendered, so no header-shortcut regions exist.
    use fdemon_app::{AppState, UiMode};
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    state.show_settings();
    assert_eq!(state.ui_mode, UiMode::Settings);

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    // Note: the *header* is in fact rendered in Settings mode (it's part of
    // the always-on layout — see render/mod.rs). Adjust this assertion based
    // on what the implementor finds. If the header is rendered, this test
    // becomes "Settings mode has its header regions but no panel-internal
    // regions yet" — Phase 5 will wire panel clicks.
    //
    // Conservative assertion: registry is non-empty if header rendered;
    // empty if it did not. Verify by inspection during implementation.
    let _ = regions; // placeholder — adjust assertion per finding
}
```

The third test is intentionally exploratory — when implementing, the implementor should observe whether the header renders in Settings mode (looking at `render/mod.rs::view`). If it does, document the resulting regions; if it does not, assert empty. The point is to lock in the observed behavior.

### Acceptance Criteria

1. End-to-end click tests in `handler/tests.rs` cover:
   - `[q]` click → `RequestQuit` → quits when no running sessions.
   - `[r]` click while busy → no-op (busy gate).
   - Click outside any region → no-op.
   - Middle-click on tab → `CloseSessionAt(idx)` → that session is removed.
   - Click on single-session device pill → opens New Session dialog.
2. Render-level snapshot tests in `render/tests.rs` confirm:
   - Six bracketed-shortcut regions appear in left-to-right `r R x d D q` order at 120×24.
   - Three tab regions appear with three sessions, with both `Left` and `Middle` actions bound.
3. The placeholder test from Task 04 (`test_view_leaves_mouse_regions_empty_when_no_widget_records`) is removed or rewritten.
4. `cargo test --workspace` passes.
5. `cargo clippy --workspace --all-targets -- -D warnings` passes.

### Notes

- The handler tests deliberately populate the registry manually rather than running the TUI render code, because `fdemon-app` does not depend on `fdemon-tui`. The render-level snapshot tests (in `fdemon-tui::render::tests`) cover the actual register-during-render path.
- `UpdateResult::message` is the canonical way `update()` chains a follow-up message. The engine re-enters `update()` with the follow-up; tests must do the same one-step recursion to observe the final state.
- The Settings-mode test is a **probe** — it documents whatever behavior the implementation produces. Phase 5 will wire panel-internal regions; Phase 3 only needs to confirm the registry behaves sanely outside of Normal mode (does not panic, does not record obviously-wrong regions).
- Add a brief `// TODO(phase-5): tag-filter overlay precedence` note next to any test that will need updating when Phase 5's modal layer arrives. This documents the deferred work without blocking Phase 3.
