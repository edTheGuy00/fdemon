## Task: NewSessionDialog Regions

**Objective**: Fill in `widgets::new_session_dialog::render_with_regions` and thread `MouseCtx` through `tab_bar.rs`, `device_list.rs`, `launch_context.rs`, and `fuzzy_modal.rs` so every clickable surface in the dialog records a region. Also fill in the corresponding handler bodies in `handler/new_session/clicks.rs` (`handle_select_device_at`, `handle_focus_field`, `handle_fuzzy_select_at`). Main-dialog regions register at `z_index = 1`; sub-modal regions (fuzzy modal rows, when the modal is open) register at `z_index = 2`. The dart-defines modal stays keyboard-only in v1 (deferred to Phase 6).

**Depends on**: 01 (Phase-5 messages), 02 (sister `render_with_regions` stub)

**Estimated Time**: 2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: Replace the stub `render_with_regions` with a real implementation that re-implements the layout calculation (mirroring `Widget::render`) and threads `MouseCtx` to the sub-widget regions.
- `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`: Add a sister `render_with_regions(...)` (or change `Widget::render` to accept `Option<&mut MouseCtx>` if simpler given the file's size). Records `[1] Connected` and `[2] Bootable` tab rects → `Message::NewSessionDialogSwitchTab(TargetTab::Connected | TargetTab::Bootable)`.
- `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`: Add a sister `render_with_regions` that records one rect per visible device row → `Message::NewSessionDialogSelectDeviceAt { index }`.
- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`: Add a sister `render_with_regions` that records one rect per launch-context field row → `Message::NewSessionDialogFocusField { field }`. Also records the `Launch` button rect → `Message::NewSessionDialogLaunch`.
- `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs`: Add a sister `render_with_regions` that records one rect per visible fuzzy-result row → `Message::NewSessionDialogFuzzySelectAt { index }`. **z_index = 2** because fuzzy modal layers atop the main dialog (z=1).
- `crates/fdemon-app/src/handler/new_session/clicks.rs`: Replace the stub bodies of `handle_select_device_at`, `handle_focus_field`, `handle_fuzzy_select_at` with real implementations.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog/{state.rs, target_selector.rs, launch_context.rs, fuzzy_modal.rs}` — for the underlying state types and their indexable lists.
- `crates/fdemon-app/src/message.rs` — `NewSessionDialogSwitchTab`, `NewSessionDialogSelectDeviceAt`, `NewSessionDialogFocusField`, `NewSessionDialogLaunch`, `NewSessionDialogFuzzySelectAt`, `NewSessionDialogDeviceSelect`, `NewSessionDialogFieldActivate`, `NewSessionDialogFuzzyConfirm`.
- `crates/fdemon-app/src/mouse_regions.rs` — `MouseRect`, `MouseAction::emit`, `MouseRegionsBuilder::click_at_z`.

### Details

#### Approach: walk the layout, record at each level

The dialog has nested layouts (header / panes / footer; each pane has its own internal layout). The cleanest approach is to:

1. In `widgets/new_session_dialog/mod.rs::render_with_regions`, recompute the layout exactly the way `Widget::render` does (split chunks, etc.).
2. For each chunk that contains a clickable sub-widget, call the sub-widget's `render_with_regions` with the chunk's rect and `ctx.as_deref_mut()`.
3. The sub-widget recomputes its own internal layout and pushes regions for its own clickable items.

Sub-widget hierarchy:
- `NewSessionDialog::render_with_regions` (this task fills in)
  - `TargetSelector::render_with_regions` — calls into:
    - `TabBar::render_with_regions` (records 2 tab rects)
    - `DeviceList::render_with_regions` (records N device-row rects)
  - `LaunchContextWithDevice::render_with_regions` — records 4–5 field rects + 1 launch button rect
  - If `state.is_fuzzy_modal_open()`: `FuzzyModal::render_with_regions` (records M result-row rects at z=2)
  - If `state.is_dart_defines_modal_open()`: **NO recording in v1** — sub-modal click support deferred to Phase 6.

For each region, **always register at z=1 for the main dialog and z=2 for fuzzy-modal results.** Do NOT mix z-values within the main dialog — even the Launch button is z=1.

#### TabBar regions

Existing render path: `TabBar::render` splits the inner area into two halves (`Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])`), then renders each tab as a centered `Paragraph` in its half.

Region recording:

```rust
// In tab_bar.rs's new render_with_regions:
let tabs_split = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
    .split(inner);

if let Some(c) = ctx.as_deref_mut() {
    c.click_at_z(
        MouseRect::from(tabs_split[0]),
        MouseAction::emit(Message::NewSessionDialogSwitchTab(TargetTab::Connected)),
        1,
    );
    c.click_at_z(
        MouseRect::from(tabs_split[1]),
        MouseAction::emit(Message::NewSessionDialogSwitchTab(TargetTab::Bootable)),
        1,
    );
}

// (Render the tabs as before.)
```

Each tab rect spans the full half-area of the tab bar — clicking anywhere in the tab cell selects that tab.

#### DeviceList regions

Devices are rendered as a vertical `List` widget. The widget knows its `selected_index` and `scroll_offset` — same pattern as the tag-filter overlay (Task 07). Compute the visible window, record one rect per visible row at the row's screen coordinates:

```rust
// In device_list.rs's new render_with_regions:
let visible_height = list_chunk.height as usize;
let scroll_offset = compute_scroll_offset(state.selected_index, devices.len(), visible_height);

for screen_row in 0..visible_height {
    let abs_index = scroll_offset + screen_row;
    if abs_index >= devices.len() {
        break;
    }
    let rect = MouseRect::new(list_chunk.x, list_chunk.y + screen_row as u16, list_chunk.width, 1);
    if rect.is_empty() {
        continue;
    }
    if let Some(c) = ctx.as_deref_mut() {
        c.click_at_z(
            rect,
            MouseAction::emit(Message::NewSessionDialogSelectDeviceAt { index: abs_index }),
            1,
        );
    }
}
```

#### LaunchContext field regions

`LaunchContextWithDevice` renders fields stacked vertically. Each field has a known rect:

- `Configuration` field
- `Mode` field
- `Flavor` field
- `Entry Point` field
- `Dart Defines` field (button-style)
- `Launch` button (at the bottom)

For each field, record a region with `Message::NewSessionDialogFocusField { field: <variant> }`. The `Launch` button records with `Message::NewSessionDialogLaunch`. The exact rect math depends on the existing layout — the implementer reads `widgets/new_session_dialog/launch_context.rs::render` and mirrors the per-field chunk layout.

```rust
// In launch_context.rs's new render_with_regions, after computing each field's rect:
if let Some(c) = ctx.as_deref_mut() {
    c.click_at_z(
        config_field_rect,
        MouseAction::emit(Message::NewSessionDialogFocusField {
            field: LaunchContextField::Config,
        }),
        1,
    );
    // ...repeat for Mode, Flavor, EntryPoint, DartDefines...

    c.click_at_z(
        launch_button_rect,
        MouseAction::emit(Message::NewSessionDialogLaunch),
        1,
    );
}
```

If the dialog is in compact mode (different field layout), the rects are different but the region-recording logic is the same — for each rendered field, record its rect.

#### FuzzyModal result regions

The fuzzy modal renders matches as a vertical list. Same pattern as DeviceList:

```rust
// In fuzzy_modal.rs's new render_with_regions:
for screen_row in 0..visible_height {
    let abs_index = scroll_offset + screen_row;
    if abs_index >= state.matches.len() {
        break;
    }
    let rect = MouseRect::new(...);
    if let Some(c) = ctx.as_deref_mut() {
        c.click_at_z(
            rect,
            MouseAction::emit(Message::NewSessionDialogFuzzySelectAt { index: abs_index }),
            2, // sub-modal layer
        );
    }
}
```

#### Handler bodies

In `crates/fdemon-app/src/handler/new_session/clicks.rs`:

```rust
/// Set the selected device on the active tab and emit a follow-up
/// `NewSessionDialogDeviceSelect` to confirm.
pub fn handle_select_device_at(state: &mut AppState, index: usize) -> UpdateResult {
    use crate::new_session_dialog::TargetTab;

    // The active tab determines which device list we update.
    let target = &mut state.new_session_dialog_state.target_selector;

    let device_count = match target.active_tab {
        TargetTab::Connected => target.connected_devices.len(),
        TargetTab::Bootable => target.bootable_devices.len(),
    };
    if device_count == 0 {
        return UpdateResult::none();
    }
    let clamped = index.min(device_count - 1);

    match target.active_tab {
        TargetTab::Connected => {
            target.connected_index = clamped;
        }
        TargetTab::Bootable => {
            target.bootable_index = clamped;
        }
    }

    UpdateResult::message(Message::NewSessionDialogDeviceSelect)
}

/// Set the focused field in the LaunchContext pane and emit a follow-up
/// `NewSessionDialogFieldActivate` for fields that activate-on-Enter.
pub fn handle_focus_field(state: &mut AppState, field: LaunchContextField) -> UpdateResult {
    use crate::new_session_dialog::DialogPane;
    state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
    state.new_session_dialog_state.launch_context.focused_field = field;
    UpdateResult::message(Message::NewSessionDialogFieldActivate)
}

/// Set the selected match in the fuzzy modal and emit a follow-up
/// `NewSessionDialogFuzzyConfirm`.
pub fn handle_fuzzy_select_at(state: &mut AppState, index: usize) -> UpdateResult {
    let Some(modal) = state.new_session_dialog_state.fuzzy_modal.as_mut() else {
        return UpdateResult::none();
    };
    if modal.matches.is_empty() {
        return UpdateResult::none();
    }
    let clamped = index.min(modal.matches.len() - 1);
    modal.selected_index = clamped;
    UpdateResult::message(Message::NewSessionDialogFuzzyConfirm)
}
```

(Field/struct names may differ — the implementer reads `crates/fdemon-app/src/new_session_dialog/{target_selector,launch_context,fuzzy_modal}.rs` and adjusts accordingly. The shape of each handler is "set absolute index → emit chained follow-up".)

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — existing dialog tests pass; new tests below are added.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. With a 2-tab dialog and one connected device, the registry contains: 2 tab regions + 1 device row region + ≥4 launch-context field regions + 1 launch button region (≥ 8 total, all z=1).
5. With the fuzzy modal open and 5 matches, an additional 5 region (z=2) entries appear.
6. Each region's `MouseAction` matches the spec above (TabBar → `NewSessionDialogSwitchTab(...)`; DeviceList → `NewSessionDialogSelectDeviceAt { index }`; LaunchContext fields → `NewSessionDialogFocusField { field }`; Launch button → `NewSessionDialogLaunch`; FuzzyModal rows → `NewSessionDialogFuzzySelectAt { index }`).
7. Visual output (rendered cells) is byte-identical to the pre-task render. Verified by extending an existing test or adding a new pixel-parity test.
8. `handle_select_device_at` sets the correct index field on the active-tab device list and emits `Message::NewSessionDialogDeviceSelect` as a follow-up.
9. `handle_focus_field` sets `launch_context.focused_field = field` and emits `Message::NewSessionDialogFieldActivate` as a follow-up.
10. `handle_fuzzy_select_at` sets `fuzzy_modal.selected_index = index` and emits `Message::NewSessionDialogFuzzyConfirm` as a follow-up.

### Testing

Add unit tests in:

- `widgets/new_session_dialog/tab_bar.rs::tests` — registry has exactly 2 tab regions at z=1 with the right messages.
- `widgets/new_session_dialog/device_list.rs::tests` — registry has N regions for N visible devices; absolute indices preserved across scroll.
- `widgets/new_session_dialog/launch_context.rs::tests` — registry has the expected field count + launch button.
- `widgets/new_session_dialog/fuzzy_modal.rs::tests` — registry has M result-row regions at z=2.
- `widgets/new_session_dialog/mod.rs::tests` — integration test: render the full dialog, count total regions, verify z-distribution (≥ 8 at z=1; M at z=2 if fuzzy modal open).
- `handler/new_session/clicks.rs::tests` — unit tests for the three new handler bodies.

Example for `tab_bar.rs::tests`:

```rust
#[test]
fn render_with_regions_records_two_tab_regions_at_z1() {
    use fdemon_app::{message::Message, mouse_regions::MouseRegions, MouseCtx};
    use crate::theme::icons::IconSet;

    let icons = IconSet::default();
    let tab_bar = TabBar::new(TargetTab::Connected, true, false, false, &icons);

    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 40, 3), &mut buf, tab_bar, Some(&mut ctx));
    }

    assert_eq!(regions.len(), 2);
    let connected_present = regions.iter().any(|e| matches!(
        extract_action(e),
        Some(Message::NewSessionDialogSwitchTab(TargetTab::Connected))
    ));
    let bootable_present = regions.iter().any(|e| matches!(
        extract_action(e),
        Some(Message::NewSessionDialogSwitchTab(TargetTab::Bootable))
    ));
    assert!(connected_present);
    assert!(bootable_present);
    for entry in regions.iter() {
        assert_eq!(entry.z_index, 1);
    }
}
```

Example for `handler/new_session/clicks.rs::tests`:

```rust
#[test]
fn handle_select_device_at_sets_index_and_emits_select() {
    let mut state = AppState::new();
    state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;
    state.new_session_dialog_state.target_selector.set_connected_devices(vec![
        test_device("a"), test_device("b"), test_device("c"),
    ]);
    let result = handle_select_device_at(&mut state, 1);
    assert_eq!(state.new_session_dialog_state.target_selector.connected_index, 1);
    assert!(matches!(result.message, Some(Message::NewSessionDialogDeviceSelect)));
}

#[test]
fn handle_focus_field_sets_focused_pane_and_field_and_emits_activate() {
    let mut state = AppState::new();
    let result = handle_focus_field(&mut state, LaunchContextField::Mode);
    assert_eq!(state.new_session_dialog_state.focused_pane, DialogPane::LaunchContext);
    assert_eq!(state.new_session_dialog_state.launch_context.focused_field, LaunchContextField::Mode);
    assert!(matches!(result.message, Some(Message::NewSessionDialogFieldActivate)));
}

#[test]
fn handle_fuzzy_select_at_sets_index_and_emits_confirm() {
    let mut state = AppState::new();
    state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
        FuzzyModalType::Flavor,
        vec!["dev".into(), "prod".into()],
    ));
    let result = handle_fuzzy_select_at(&mut state, 1);
    let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
    assert_eq!(modal.selected_index, 1);
    assert!(matches!(result.message, Some(Message::NewSessionDialogFuzzyConfirm)));
}
```

### Notes

- **Why TabBar regions cover the full half-area, not just the label text.** The tab bar renders each tab's label centered in its half. Recording only the label cells would leave the rest of the tab visually inert — confusing UX. Full-half rects mirror what users expect from any GUI tab control.
- **Why DeviceList rects span the full row width.** Same reason — clicking anywhere on a row should select it.
- **Why the Launch button uses the existing `Message::NewSessionDialogLaunch`.** That variant already exists and is the keyboard handler's target. Click parity with keyboard is a v1 invariant.
- **Why the dart-defines modal is deferred.** It's a full-screen sub-modal with its own internal pane layout (List vs Edit). Adding click regions for it is a Phase 6 polish task — keeping Phase 5 focused on the most-used surfaces.
- **Why fuzzy-modal rows register at z=2.** The fuzzy modal overlays the main dialog. A click on a fuzzy result row must NOT also fire the device-row click underneath. z=2 ensures the fuzzy modal wins.
- **Why we re-implement the layout instead of delegating to `Widget::render`.** Computing per-row rects requires the layout chunks. Delegating would render correctly but leave us blind to the rects. Re-implementing the layout costs ~30 lines of duplicate code and is the standard pattern for `render_with_regions`.
- **Why field-clicking emits `Focus` then `Activate` (chained), not just `FocusField`.** Most users clicking a field expect it to *activate* (open the picker, start editing). Chaining `Activate` after `Focus` matches the keyboard sequence (arrow-key to field, then Enter). Fields that don't activate on Enter (only the Mode field's Left/Right cycler) gracefully no-op the activate follow-up.
- **Why `NewSessionDialogSelectDeviceAt` chains `NewSessionDialogDeviceSelect` instead of being a self-contained "set + select" arm.** The existing `NewSessionDialogDeviceSelect` arm already handles all the side effects (connection probing, error display, etc.) — duplicating that logic in `handle_select_device_at` would be drift-prone. Chaining keeps the click flow trivially equivalent to keyboard "arrow N times then Enter".
- **`MouseRect::from(rect)` helper.** If a `From<ratatui::layout::Rect> for MouseRect` impl already exists (likely from Phase 3), use it. If not, the conversion is `MouseRect::new(rect.x, rect.y, rect.width, rect.height)`.
