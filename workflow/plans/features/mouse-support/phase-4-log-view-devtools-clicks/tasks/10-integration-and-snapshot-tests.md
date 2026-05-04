## Task: Phase-4 Integration & Snapshot Tests

**Objective**: Lock in Phase 4 behaviour with cross-cutting integration and snapshot tests that exercise the full message → registry → click → handler flow. Catches drift between the rect math in widgets and the index math in handlers; ensures modal precedence (`z_index = 0` everywhere in this phase) is respected.

**Depends on**: Tasks 03, 04, 05, 06, 07, 08, 09 (all the production behaviour must already be in place)

**Estimated Time**: 1.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/tests.rs`: Add cross-cutting tests that drive `update(&mut state, Message::ClickLogRow { ... })` etc. through the dispatch chain and assert the expected state mutations and follow-up messages.
- `crates/fdemon-tui/src/render/tests.rs`: Add snapshot tests on registry contents under representative viewport sizes for each Phase-4 panel:
  - Log view 80×24 with 12 mixed message + stack-frame rows
  - DevTools tab bar 80×24 (3 sub-tab regions)
  - Inspector tree 80×24 with 5 nodes (10 regions: 5 row + 5 glyph)
  - Performance frame chart 80×24 with 8 frames (8 regions)
  - Network table 80×24 with 10 rows + 5 detail-tab regions when selected

**Files Read (Dependencies):**
- All Phase-4 production files (read-only — assertions reference public APIs)

### Details

#### Cross-cutting integration test (in `handler/tests.rs`)

```rust
#[test]
fn click_log_row_then_double_click_emits_toggle_stack_trace_for_entry() {
    use crate::handler::update::update;
    use crate::message::Message;

    let mut state = setup_state_with_one_session_and_one_entry_with_stack();
    let entry_id = first_entry_id(&state);

    // Single click → records stamp, no follow-up.
    let r1 = update(&mut state, Message::ClickLogRow { entry_id, frame_index: None });
    assert!(r1.message.is_none());
    assert!(state.last_log_click.is_some());

    // Second click within window → produces follow-up.
    let r2 = update(&mut state, Message::ClickLogRow { entry_id, frame_index: None });
    let follow_up = r2.message.expect("expected follow-up message");
    assert!(matches!(
        follow_up,
        Message::ToggleStackTraceForEntry { entry_id: e } if e == entry_id
    ));

    // Dispatch the follow-up and verify the stack trace toggled.
    let stack_was_collapsed = was_collapsed(&state, entry_id);
    update(&mut state, follow_up);
    assert_ne!(stack_was_collapsed, was_collapsed(&state, entry_id));
}

#[test]
fn click_devtools_inspector_tab_switches_panel() {
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::state::DevToolsPanel;

    let mut state = setup_state_in_devtools_mode_with_session();
    state.devtools_view_state.active_panel = DevToolsPanel::Inspector;

    update(&mut state, Message::SwitchDevToolsPanel(DevToolsPanel::Network));
    assert_eq!(state.devtools_view_state.active_panel, DevToolsPanel::Network);
}

#[test]
fn click_inspector_select_row_dispatches_layout_fetch() {
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::handler::UpdateAction;

    let mut state = setup_state_with_inspector_tree_5_nodes();
    let r = update(&mut state, Message::DevToolsInspectorSelectRow { index: 2 });
    assert_eq!(state.devtools_view_state.inspector.selected_index, 2);
    assert!(matches!(r.action, Some(UpdateAction::FetchLayoutData { .. })));
}

#[test]
fn click_performance_frame_sets_selected_frame() {
    use crate::handler::update::update;
    use crate::message::Message;

    let (mut state, _session_id) = setup_state_in_performance_panel_with_8_frames();
    update(&mut state, Message::SelectPerformanceFrame { index: Some(3) });
    assert_eq!(
        state.session_manager.selected().unwrap().session.performance.selected_frame,
        Some(3)
    );
}

#[test]
fn click_network_select_request_sets_selected_index() {
    use crate::handler::update::update;
    use crate::message::Message;

    let mut state = setup_state_in_network_panel_with_5_requests();
    update(&mut state, Message::NetworkSelectRequest { index: Some(2) });
    assert_eq!(
        state.session_manager.selected().unwrap().session.network.selected_index,
        Some(2)
    );
}
```

(Adapt setup helpers to existing patterns in `handler/tests.rs`; many of the helpers may already exist.)

#### Registry snapshot tests (in `render/tests.rs`)

Mirror the structure of the existing `view_renders_expected_header_regions` test from Phase 3:

```rust
#[test]
fn view_renders_expected_log_view_regions_at_80x24() {
    use fdemon_app::message::Message;
    let mut state = build_state_with_one_session_and_logs(/*entries=*/ 12);
    let mut term = make_test_terminal(80, 24);
    term.draw(|frame| crate::render::view(frame, &mut state)).unwrap();

    // After view(), the registry is back in the Cell.
    let regions = state.mouse_regions.take();
    let click_log_rows = regions
        .iter()
        .filter(|e| matches!(
            e.on_left.as_ref().and_then(|a| a.as_emit()),
            Some(Message::ClickLogRow { .. })
        ))
        .count();
    assert!(
        click_log_rows >= 12,
        "expected ≥ 12 row regions for 12 visible entries, got {click_log_rows}"
    );
}

#[test]
fn view_renders_expected_devtools_tab_regions() {
    use fdemon_app::message::Message;
    let mut state = build_state_in_devtools_mode_inspector_panel();
    let mut term = make_test_terminal(80, 24);
    term.draw(|frame| crate::render::view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let tab_regions: Vec<_> = regions
        .iter()
        .filter(|e| matches!(
            e.on_left.as_ref().and_then(|a| a.as_emit()),
            Some(Message::SwitchDevToolsPanel(_))
        ))
        .collect();
    assert_eq!(tab_regions.len(), 3);
    // Order matches [i] Inspector / [p] Performance / [n] Network.
}

#[test]
fn view_renders_expected_inspector_tree_regions() {
    let mut state = build_state_in_devtools_mode_with_inspector_tree(/*nodes=*/ 5);
    let mut term = make_test_terminal(80, 24);
    term.draw(|frame| crate::render::view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let select_regions = regions.iter().filter(|e| matches!(
        e.on_left.as_ref().and_then(|a| a.as_emit()),
        Some(fdemon_app::message::Message::DevToolsInspectorSelectRow { .. })
    )).count();
    let toggle_regions = regions.iter().filter(|e| matches!(
        e.on_left.as_ref().and_then(|a| a.as_emit()),
        Some(fdemon_app::message::Message::DevToolsInspectorToggleNode { .. })
    )).count();
    assert_eq!(select_regions, 5);
    assert_eq!(toggle_regions, 5);
}

#[test]
fn view_renders_expected_performance_frame_regions() {
    let mut state = build_state_in_devtools_mode_performance_panel(/*frames=*/ 8);
    let mut term = make_test_terminal(80, 24);
    term.draw(|frame| crate::render::view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let frame_regions = regions.iter().filter(|e| matches!(
        e.on_left.as_ref().and_then(|a| a.as_emit()),
        Some(fdemon_app::message::Message::SelectPerformanceFrame { index: Some(_) })
    )).count();
    assert_eq!(frame_regions, 8);
}

#[test]
fn view_renders_expected_network_regions_with_selection() {
    let mut state = build_state_in_devtools_mode_network_panel_with_selection(/*requests=*/ 10);
    let mut term = make_test_terminal(120, 30); // wide layout
    term.draw(|frame| crate::render::view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let row_regions = regions.iter().filter(|e| matches!(
        e.on_left.as_ref().and_then(|a| a.as_emit()),
        Some(fdemon_app::message::Message::NetworkSelectRequest { .. })
    )).count();
    let detail_tab_regions = regions.iter().filter(|e| matches!(
        e.on_left.as_ref().and_then(|a| a.as_emit()),
        Some(fdemon_app::message::Message::NetworkSwitchDetailTab(_))
    )).count();
    assert!(row_regions >= 10);
    assert_eq!(detail_tab_regions, 5);
}

#[test]
fn phase_4_records_no_z1_regions() {
    // Phase 4 must not register any z_index = 1 regions — that level is
    // reserved for Phase 5 dialogs/overlays.
    let mut state = build_state_in_devtools_mode_inspector_panel();
    let mut term = make_test_terminal(80, 24);
    term.draw(|frame| crate::render::view(frame, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    for entry in regions.iter() {
        assert_eq!(entry.z_index, 0, "Phase 4 must not use z_index=1");
    }
}
```

Adapt iterator API (`.iter()`, `.z_index`, etc.) to the actual `MouseRegions` public surface defined in Phase 3.

### Acceptance Criteria

1. ≥ 5 cross-cutting integration tests in `handler/tests.rs` covering the `Message::*` → state mutation flow for every Phase-4 click message.
2. ≥ 6 snapshot tests in `render/tests.rs` covering registry contents for every Phase-4 panel under typical viewport sizes (80×24 and at least one wide-layout 120×30 case for Network).
3. A `phase_4_records_no_z1_regions` test asserting all Phase-4 regions are at `z_index = 0`.
4. A double-click integration test that drives the full `ClickLogRow → ToggleStackTraceForEntry` chain via `update()` with explicit `Instant` planting (or two consecutive calls with no sleep — the 400 ms window is generous enough that test execution stays within it).
5. `cargo test --workspace` passes; new tests grow the suite by ≥ 11.
6. `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.

### Manual Smoke Test (recorded in completion summary)

Execute the full mouse-only walk-through and record the result in the task's Completion Summary. Each step must succeed without keyboard intervention:

1. Start Flutter Demon → terminal in `Normal` mode → click anywhere in the log area → no scroll, no crash.
2. Tail at least one log entry with a stack trace → click that entry's message line → wait < 400 ms → click again → stack trace expands.
3. Click the entry's stack-trace line again twice in quick succession → stack trace collapses back.
4. Click `[d]` in the header → DevTools mode opens → click `[p] Performance` → Performance panel becomes active.
5. With at least 5 frames recorded, click a bar in the middle of the chart → that frame is highlighted with `▔`; detail panel shows its timing.
6. Click `[i] Inspector` → Inspector panel becomes active → click a tree row → row becomes selected → layout panel updates.
7. Click the `▶` glyph next to a parent node → node expands → click the now-`▼` glyph → node collapses.
8. Click `[n] Network` → Network panel becomes active → with ≥ 1 request, click a row → details appear in side panel → click `[h] Headers` → detail panel switches to headers tab → click `[t] Timing` → switches to timing tab.
9. Type `f` to enter filter mode → click anywhere in the table area → no row selection occurs (filter-active gate).
10. Press `Esc` to exit filter mode → click a row again → row selection works.

### Notes

- **Why integration tests live in `handler/tests.rs` rather than each handler module.** The whole Phase-4 click flow crosses module boundaries (input → message → state → follow-up message → state). Per-module tests verified pieces; the cross-cutting tests verify the chain. Mirrors the Phase 3 pattern.
- **Why snapshot tests count regions instead of pixel-diffing.** The pixel output is already covered by the existing rendering tests. Phase-4 snapshot tests focus on the *registry*, which is the new artefact and the one most likely to drift when widget copy or layout changes.
- **Test-helper consolidation.** Several of the proposed helpers (`build_state_in_devtools_mode_*`, `make_test_terminal`, `build_state_with_one_session_and_logs`) likely exist already — search before adding. If a helper doesn't exist, add it to `render/tests.rs` (TUI side) or to a `mod test_helpers` (handler side) so it's reusable.
- **Manual smoke test is mandatory.** The PLAN.md success criteria for Phase 4 explicitly require an end-to-end mouse-only walk-through. The completion summary must include a tick-box list of the 10 steps above with pass / fail / N-A markings.
- **Why no automated browser-driven test.** Terminal-mouse end-to-end testing requires a PTY and a synthetic input source; that infrastructure doesn't exist in this codebase. The manual smoke test is the de-facto end-to-end check.
- **Don't add tests that depend on real timing.** Some doubles-click tests will assert based on `state.last_log_click.is_some()`, not on actual elapsed time. Where the 400 ms window must be exercised, plant an explicit `Instant` slightly older than the window before calling `handle_click_log_row`.
- **z_index = 0 invariant.** Add a snapshot test that iterates every entry and asserts `z_index == 0`. This guards against accidental Phase 5 overlap (e.g., a Phase 5 dialog regression that re-uses Phase 4 widget code).
