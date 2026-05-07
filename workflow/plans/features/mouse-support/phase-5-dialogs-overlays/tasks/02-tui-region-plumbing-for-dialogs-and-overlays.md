## Task: TUI Region Plumbing for Dialogs and Overlays

**Objective**: Introduce a sister `render_with_regions(...)` free function for `NewSessionDialog`, `ConfirmDialog`, `SettingsPanel`, and update the existing `render_tag_filter` and `widgets/log_view::render_with_regions` to optionally accept a `MouseCtx` for badge-rect recording. Update `render::view()` to thread `&mut MouseCtx` from frame setup down into every clickable surface in the modes Phase 5 covers (`NewSessionDialog`, `ConfirmDialog`, `Settings`, `Normal` with tag-filter visible, `LinkHighlight`). All sister functions stub their region-recording bodies for now (delegate to existing `Widget::render`); Tasks 06–10 fill in the actual region pushes.

**Depends on**: None (Wave 1, parallel with Task 01)

**Estimated Time**: 1.75 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/render/mod.rs`: Replace direct `frame.render_widget(dialog, area)` calls in the `UiMode::Startup | NewSessionDialog`, `UiMode::ConfirmDialog`, `UiMode::Settings` arms with calls to the new sister functions, threading `Some(&mut mouse_ctx)`. The `tag_filter_visible` branch in `UiMode::Normal` switches from `widgets::render_tag_filter(frame, ...)` to `widgets::render_tag_filter_with_regions(frame, ..., Some(&mut mouse_ctx))`. The `log_view::render_with_regions` call (already in place from Phase 4) is unchanged signature-wise — but now the renamed/extended function will record link-badge rects when `LinkHighlight` mode is active.
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: Add `pub fn render_with_regions(area: Rect, buf: &mut Buffer, view: NewSessionDialog<'_>, _ctx: Option<&mut MouseCtx<'_>>)` that delegates to `<NewSessionDialog as Widget>::render(view, area, buf)`. Task 09 fills in the body.
- `crates/fdemon-tui/src/widgets/confirm_dialog.rs`: Add `pub fn render_with_regions(area: Rect, buf: &mut Buffer, view: ConfirmDialog<'_>, _ctx: Option<&mut MouseCtx<'_>>)` delegating to `<ConfirmDialog as Widget>::render`. Task 06 fills in the body.
- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Add `pub fn render_with_regions(area: Rect, buf: &mut Buffer, view: SettingsPanel<'_>, state: &mut SettingsViewState, _ctx: Option<&mut MouseCtx<'_>>)` that delegates to `<SettingsPanel as StatefulWidget>::render(view, area, buf, state)`. Task 10 fills in the body.
- `crates/fdemon-tui/src/widgets/tag_filter.rs`: Either add a sister `render_tag_filter_with_regions(...)` free function alongside the existing `render_tag_filter` (preferred, mirrors the widget pattern), OR change the existing function's signature to accept `Option<&mut MouseCtx>` (acceptable because there is only one caller). Task author choice — recommendation: add a sister function so the test paths that call the bare `render_tag_filter` continue to compile.
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: The existing `render_with_regions` already accepts `Option<&mut MouseCtx>`. This task adds a *no-op* placeholder branch in the body that ignores `MouseCtx` when *not* in link-highlight mode (already the case) — Task 08 fills in the badge-rect recording when the active session has `link_highlight_state.is_active() == true`.
- `crates/fdemon-tui/src/widgets/mod.rs`: Re-export the new sister functions if needed (`pub use confirm_dialog::render_with_regions as render_confirm_dialog_with_regions`, etc.) — or leave them callable via the canonical `widgets::confirm_dialog::render_with_regions(...)` path. Match the existing convention from Phase 4.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs` (`MouseAction`, `MouseRect`, `MouseCtx`).
- `crates/fdemon-app/src/message.rs` (Phase-5 message variants from Task 01 — references only; no real region recording in this task).
- Phase 4 `crates/fdemon-tui/src/widgets/devtools/mod.rs::render_with_regions` (template — sister-function pattern with `ctx.as_deref_mut()` forwarding).

### Details

#### Sister-function pattern recap

Phase 3 introduced the pattern. Phase 4 extended it to log view + DevTools panels. Phase 5 finishes the rollout by covering the remaining click-relevant widgets.

The contract for every `render_with_regions`:

1. Same visual output as the canonical `Widget::render` / `StatefulWidget::render`.
2. Accepts `Option<&mut MouseCtx<'_>>`. When `None`, behaviour is identical to the canonical render. When `Some`, the function MAY (in this task) or MUST (in Tasks 06–10) push regions into the builder.
3. The free function takes the widget by value so the existing `Widget` impl stays untouched and existing `term.render_widget(widget, area)` test paths remain green.

#### `render::view()` call-site changes

The current matches arm-by-arm:

```rust
// BEFORE:
UiMode::Startup | UiMode::NewSessionDialog => {
    let dialog = widgets::NewSessionDialog::new(/* ... */);
    frame.render_widget(dialog, area);
}

// AFTER:
UiMode::Startup | UiMode::NewSessionDialog => {
    let dialog = widgets::NewSessionDialog::new(/* ... */)
        .migration_banner(state.show_migration_banner);
    widgets::new_session_dialog::render_with_regions(
        area,
        frame.buffer_mut(),
        dialog,
        Some(&mut mouse_ctx),
    );
}
```

```rust
// BEFORE:
UiMode::ConfirmDialog => {
    if let Some(ref dialog_state) = state.confirm_dialog_state {
        let dialog = widgets::ConfirmDialog::new(dialog_state);
        frame.render_widget(dialog, area);
    }
}

// AFTER:
UiMode::ConfirmDialog => {
    if let Some(ref dialog_state) = state.confirm_dialog_state {
        let dialog = widgets::ConfirmDialog::new(dialog_state);
        widgets::confirm_dialog::render_with_regions(
            area,
            frame.buffer_mut(),
            dialog,
            Some(&mut mouse_ctx),
        );
    }
}
```

```rust
// BEFORE:
UiMode::Settings => {
    let settings_panel = widgets::SettingsPanel::new(&state.settings, &state.project_path);
    frame.render_stateful_widget(settings_panel, area, &mut state.settings_view_state);
}

// AFTER:
UiMode::Settings => {
    let settings_panel = widgets::SettingsPanel::new(&state.settings, &state.project_path);
    widgets::settings_panel::render_with_regions(
        area,
        frame.buffer_mut(),
        settings_panel,
        &mut state.settings_view_state,
        Some(&mut mouse_ctx),
    );
}
```

```rust
// BEFORE (inside UiMode::Normal arm):
if state.tag_filter_visible {
    if let Some(handle) = state.session_manager.selected() {
        widgets::render_tag_filter(
            frame,
            areas.logs,
            &handle.native_tag_state,
            &state.tag_filter_ui,
        );
    }
}

// AFTER:
if state.tag_filter_visible {
    if let Some(handle) = state.session_manager.selected() {
        widgets::render_tag_filter_with_regions(
            frame,
            areas.logs,
            &handle.native_tag_state,
            &state.tag_filter_ui,
            Some(&mut mouse_ctx),
        );
    }
}
```

The `LinkHighlight` arm is special: the link badges are rendered *inside* `widgets::log_view::render_with_regions` (already called earlier in `render::view`), not in the `LinkHighlight` arm itself. This task does not change the `LinkHighlight` arm (the instruction bar is rendered there but has no clickable surface). Task 08 modifies `widgets/log_view/mod.rs::render_with_regions` to record badge rects.

#### Sister-function stubs

Each new `render_with_regions` is a delegate-only stub:

```rust
// crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: NewSessionDialog<'_>,
    _ctx: Option<&mut MouseCtx<'_>>,
) {
    // Phase 5 Task 09 fills in the body.
    <NewSessionDialog as Widget>::render(view, area, buf);
}
```

```rust
// crates/fdemon-tui/src/widgets/confirm_dialog.rs
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: ConfirmDialog<'_>,
    _ctx: Option<&mut MouseCtx<'_>>,
) {
    // Phase 5 Task 06 fills in the body.
    <ConfirmDialog as Widget>::render(view, area, buf);
}
```

```rust
// crates/fdemon-tui/src/widgets/settings_panel/mod.rs
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: SettingsPanel<'_>,
    state: &mut SettingsViewState,
    _ctx: Option<&mut MouseCtx<'_>>,
) {
    // Phase 5 Task 10 fills in the body.
    <SettingsPanel as StatefulWidget>::render(view, area, buf, state);
}
```

```rust
// crates/fdemon-tui/src/widgets/tag_filter.rs
pub fn render_tag_filter_with_regions(
    frame: &mut Frame,
    area: Rect,
    tag_state: &NativeTagState,
    ui_state: &TagFilterUiState,
    _ctx: Option<&mut MouseCtx<'_>>,
) {
    // Phase 5 Task 07 fills in the body.
    render_tag_filter(frame, area, tag_state, ui_state);
}
```

`widgets/log_view/mod.rs::render_with_regions` already exists. This task adds **no body changes** — only an `// EXCEPTION: link-highlight badge regions are recorded in Phase 5 Task 08` comment near where badges are rendered, marking the future region-recording site. Task 08 fills it in.

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — every existing test continues to work because `render_with_regions` delegates to the existing `Widget::render` for the no-region path.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. `widgets::new_session_dialog::render_with_regions`, `widgets::confirm_dialog::render_with_regions`, `widgets::settings_panel::render_with_regions`, `widgets::render_tag_filter_with_regions` (or equivalent) all exist with the signatures specified above.
5. `render::view()` calls the four new functions for the `Startup`/`NewSessionDialog`, `ConfirmDialog`, `Settings`, and `Normal+tag_filter_visible` paths (replacing the previous `frame.render_widget` / `frame.render_stateful_widget` / `widgets::render_tag_filter` calls).
6. The `Widget::render` and `StatefulWidget::render` impls of all touched widgets are unchanged in behaviour — pre-existing tests that render via `widget.render(area, buf)` continue to pass without modification.
7. No regions are pushed in this task. Calling `render_with_regions(..., Some(&mut ctx))` with a fresh `MouseCtx` and then iterating the registry afterward must yield zero new entries (verified by a smoke test).

### Testing

Add a single smoke test inside `crates/fdemon-tui/src/render/tests.rs` (or a new file):

```rust
#[test]
fn phase5_sister_functions_record_no_regions_in_stub_state() {
    // After Task 02 lands but before Tasks 06-10, the new sister functions
    // delegate to existing Widget::render and record zero regions. This test
    // locks that invariant in until Tasks 06-10 land.
    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(1));

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render::view(frame, &mut state);
        })
        .unwrap();

    // After the render, the registry should hold the regions from any
    // already-Phase-3/4-recorded widgets (header, tabs, log view, devtools)
    // plus zero new Phase-5 regions. We assert the count by filtering for
    // Phase-5 message variants — none of them should appear yet.
    let regions = state.mouse_regions.take();
    let phase5_count = regions
        .iter()
        .filter(|e| matches_phase5_message_shape(e))
        .count();
    assert_eq!(phase5_count, 0, "no Phase-5 regions before Tasks 06-10");
    state.mouse_regions.set(regions);
}
```

(`matches_phase5_message_shape` is a small helper in the test module that returns true for `Message::NewSessionDialogSelectDeviceAt | NewSessionDialogFocusField | NewSessionDialogFuzzySelectAt | SettingsClickRow | TagFilterClickRow | SelectLink(_)` once you can extract a `Message` from the entry's `MouseAction`. If extracting the inner `Message` requires a helper that doesn't yet exist, deferring this smoke test to Task 11 is acceptable — note that in the completion summary.)

### Notes

- **Why `render_tag_filter_with_regions` is a *new* free function (vs changing the existing one's signature).** `render_tag_filter` is called from one production site (`render::view`) and several test sites (`tag_filter.rs::tests`). Changing the signature would force each test to thread `None` through. A sister function avoids the test-churn and matches the widget pattern.
- **`MouseCtx::as_deref_mut`.** The pattern from Phase 4 (`ctx.as_deref_mut()` to forward an `Option<&mut MouseCtx>` through nested calls) applies here too. `widgets/new_session_dialog/render_with_regions` will need `ctx.as_deref_mut()` when forwarding to its sub-widget regions in Task 09; Task 02 stubs this out by passing `_ctx` straight to the underlying `Widget::render`.
- **`StatefulWidget` for `SettingsPanel`.** The `SettingsPanel` is a `StatefulWidget` because the row layout depends on `state.selected_index`. The sister function takes the state by `&mut SettingsViewState` to mirror the existing impl.
- **No changes to the `Widget`/`StatefulWidget` impls.** Same as Phase 4 — the impls stay for tests, and the sister function is the click-aware variant.
- **No changes to the `LinkHighlight` `render::view` arm.** The instruction bar at the bottom of the log view has no click target. Badge rects are recorded inside `widgets/log_view/mod.rs::render_with_regions` (Task 08), which is already called earlier in `render::view` regardless of `UiMode`. The active-session check + `link_highlight_state.is_active()` check inside `render_with_regions` gates badge recording to only fire in `LinkHighlight` mode. The `UiMode::LinkHighlight` arm in `render::view` only renders the instruction bar, which is informational.
- **No changes to `widgets/mod.rs`'s public re-exports unless required.** If callers reference `widgets::render_tag_filter_with_regions` (without the module path), a `pub use tag_filter::render_tag_filter_with_regions;` re-export is added; otherwise the canonical `widgets::tag_filter::render_tag_filter_with_regions(...)` path is fine.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a19b2b53dbde29248

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/render/mod.rs` | Replaced `frame.render_widget` / `frame.render_stateful_widget` / `widgets::render_tag_filter` calls with the four new sister-function calls for `NewSessionDialog`, `ConfirmDialog`, `SettingsPanel`, and `render_tag_filter_with_regions` |
| `crates/fdemon-tui/src/render/tests.rs` | Added two Phase-5 smoke tests: `phase5_sister_functions_record_no_regions_in_stub_state` and `phase5_settings_sister_records_no_new_regions` |
| `crates/fdemon-tui/src/widgets/confirm_dialog.rs` | Added `pub fn render_with_regions(...)` stub; made module public in `widgets/mod.rs` |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Added `pub fn render_with_regions(...)` stub (delegates to `<NewSessionDialog as Widget>::render`) |
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Added `pub fn render_with_regions(...)` stub (delegates to `<SettingsPanel as StatefulWidget>::render`) |
| `crates/fdemon-tui/src/widgets/tag_filter.rs` | Added `pub fn render_tag_filter_with_regions(...)` sister function (delegates to `render_tag_filter`) |
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Added `// EXCEPTION: link-highlight badge regions are recorded in Phase 5 Task 08` comment in `render_inner` |
| `crates/fdemon-tui/src/widgets/mod.rs` | Made `confirm_dialog` a public module; re-exported `render_tag_filter_with_regions` |

### Notable Decisions/Tradeoffs

1. **`confirm_dialog` module made public**: The task spec calls `widgets::confirm_dialog::render_with_regions(...)` from `render/mod.rs`. Since the module was private (`mod confirm_dialog;`), it was changed to `pub mod confirm_dialog;` to allow the path-based access from the render layer. This matches the precedent of `pub mod devtools;` / `pub mod log_view;`.

2. **Smoke test deferred partial variant**: The task spec's full `matches_phase5_message_shape` helper requires Phase 5 Task 01 message variants (`NewSessionDialogSelectDeviceAt`, `SettingsClickRow`, `TagFilterClickRow`, etc.) which do not exist yet. The smoke tests instead verify the weaker but still meaningful invariant: no `z_index = 1` regions are pushed in stub state (since Phase 5 region-recording tasks will use z=1 for dialog overlays).

3. **`render_tag_filter_with_regions` re-exported from `widgets::mod`**: Since `render::view` uses the path `widgets::render_tag_filter_with_regions(...)` (matching the existing `render_tag_filter` re-export pattern), the function is re-exported at the top level alongside `render_tag_filter`.

### Testing Performed

- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all 0 new failures; 2 new smoke tests passing)
- `cargo fmt --all -- --check` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Smoke test uses z_index heuristic**: The smoke test for "no Phase 5 regions" checks that `z_index != 1` rather than matching specific Phase 5 message variants. This is because Task 01 hasn't landed yet. Task 11 will add the definitive `matches_phase5_message_shape` helper once all Phase 5 message variants exist.
