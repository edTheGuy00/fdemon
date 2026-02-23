## Task: Render Settings Modal Overlays

**Objective**: Add overlay rendering in the settings panel TUI widget so that `DartDefinesModal` and `FuzzyModal` appear as modal overlays when their respective state fields are `Some`.

**Depends on**: 02-settings-modal-state

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Add modal overlay rendering after settings content
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs`: Rendering tests for modal overlays

### Details

#### 1. Add modal overlay rendering to `SettingsPanel`

The `SettingsPanel::render()` method at `settings_panel/mod.rs:68-93` currently calls:
1. `render_header()`
2. `render_content()`
3. `render_footer()`

After these, add modal overlay checks:

```rust
fn render(self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
    // ... existing header/content/footer rendering ...

    // Modal overlays (rendered last to appear on top)
    if let Some(dart_defines_modal) = &state.dart_defines_modal {
        self.render_dart_defines_modal_overlay(area, buf, dart_defines_modal);
    } else if let Some(extra_args_modal) = &state.extra_args_modal {
        self.render_extra_args_modal_overlay(area, buf, extra_args_modal);
    }
}
```

Only one modal is open at a time (enforced by `has_modal_open()` check on open handlers), so `else if` is correct.

#### 2. Render `DartDefinesModal` overlay

```rust
fn render_dart_defines_modal_overlay(
    &self,
    area: Rect,
    buf: &mut Buffer,
    modal_state: &DartDefinesModalState,
) {
    // Dim background behind the modal
    modal_overlay::dim_background(buf, area);

    // Render the DartDefinesModal widget over the full area
    let modal = DartDefinesModal::new(modal_state);
    modal.render(area, buf);
}
```

Import `DartDefinesModal` from `crate::widgets::new_session_dialog::dart_defines_modal`. The widget is already a standalone `Widget` that takes `&DartDefinesModalState` — no changes needed to the widget itself.

The `DartDefinesModal` widget self-computes its position (near full-screen minus margins) and calls `Clear` on its area internally (`dart_defines_modal.rs:603-605`).

#### 3. Render `FuzzyModal` overlay

```rust
fn render_extra_args_modal_overlay(
    &self,
    area: Rect,
    buf: &mut Buffer,
    modal_state: &FuzzyModalState,
) {
    // Dim background behind the modal
    modal_overlay::dim_background(buf, area);

    // Render the FuzzyModal widget
    let modal = FuzzyModal::new(modal_state);
    modal.render(area, buf);
}
```

Import `FuzzyModal` from `crate::widgets::new_session_dialog::fuzzy_modal`. The widget self-positions at the bottom ~50% of the given area.

#### 4. Import required types and modules

Add to the imports at the top of `settings_panel/mod.rs`:

```rust
use crate::widgets::modal_overlay;
use crate::widgets::new_session_dialog::dart_defines_modal::DartDefinesModal;
use crate::widgets::new_session_dialog::fuzzy_modal::FuzzyModal;
use fdemon_app::new_session_dialog::state::{DartDefinesModalState, FuzzyModalState};
```

Check the actual module paths — the `new_session_dialog` module in `fdemon-tui` re-exports some types. Use the actual import paths that work in the TUI crate:
- `modal_overlay` is at `crate::widgets::modal_overlay` (verify with `mod.rs` of widgets)
- `DartDefinesModal` widget is at `crate::widgets::new_session_dialog::dart_defines_modal::DartDefinesModal`
- `FuzzyModal` widget is at `crate::widgets::new_session_dialog::fuzzy_modal::FuzzyModal`

#### 5. Suppress footer key hints when modal is open

The settings footer (`render_footer()`) shows keybinding hints. When a modal is open, the modal's own hints bar should be visible instead. Either:
- Skip `render_footer()` when `state.has_modal_open()` returns true (the modal renders its own hints), OR
- The modal overlay already covers the footer area (since `DartDefinesModal` uses near-full-screen and `FuzzyModal` uses bottom ~50%)

Check which approach works better. The simplest is to let the modal's `Clear` operation overwrite the footer naturally.

### Acceptance Criteria

1. When `state.dart_defines_modal.is_some()`, the `DartDefinesModal` widget renders as an overlay on top of the settings panel
2. When `state.extra_args_modal.is_some()`, the `FuzzyModal` widget renders as an overlay on top of the settings panel
3. Background behind modals is dimmed via `modal_overlay::dim_background`
4. Only one modal can render at a time (verified by `else if` logic)
5. When no modal is open, the settings panel renders normally (no visual changes)
6. `cargo check -p fdemon-tui` compiles
7. `cargo test -p fdemon-tui` passes with new rendering tests
8. `cargo clippy -p fdemon-tui` passes

### Testing

```rust
#[test]
fn test_settings_panel_renders_dart_defines_modal_overlay() {
    let settings = Settings::default();
    let temp = tempdir().unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    // Set up a dart defines modal
    state.dart_defines_modal = Some(DartDefinesModalState::new(vec![
        DartDefine { key: "API_KEY".to_string(), value: "abc123".to_string() },
    ]));

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| {
        let panel = SettingsPanel::new(&settings, temp.path());
        frame.render_stateful_widget(panel, frame.area(), &mut state);
    }).unwrap();

    let content: String = terminal.backend().buffer().content()
        .iter().map(|c| c.symbol()).collect();
    // DartDefinesModal renders "Manage Dart Defines" as its title
    assert!(content.contains("Manage Dart Defines") || content.contains("Dart Defines"));
}

#[test]
fn test_settings_panel_renders_extra_args_modal_overlay() {
    let settings = Settings::default();
    let temp = tempdir().unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::LaunchConfig;
    state.extra_args_modal = Some(FuzzyModalState::new(
        FuzzyModalType::ExtraArgs,
        vec!["--verbose".to_string()],
    ));

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| {
        let panel = SettingsPanel::new(&settings, temp.path());
        frame.render_stateful_widget(panel, frame.area(), &mut state);
    }).unwrap();

    let content: String = terminal.backend().buffer().content()
        .iter().map(|c| c.symbol()).collect();
    // FuzzyModal renders its title from modal_type.title()
    assert!(content.contains("Edit Extra Args"));
}

#[test]
fn test_settings_panel_no_overlay_when_no_modal() {
    // Verify normal rendering when both modal fields are None
    let settings = Settings::default();
    let temp = tempdir().unwrap();
    let mut state = SettingsViewState::new();
    state.active_tab = SettingsTab::Project;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| {
        let panel = SettingsPanel::new(&settings, temp.path());
        frame.render_stateful_widget(panel, frame.area(), &mut state);
    }).unwrap();

    let content: String = terminal.backend().buffer().content()
        .iter().map(|c| c.symbol()).collect();
    assert!(!content.contains("Manage Dart Defines"));
    assert!(!content.contains("Edit Extra Args"));
}
```

### Notes

- The `DartDefinesModal` widget at `new_session_dialog/dart_defines_modal.rs:553-676` already handles its own layout (margin inset, Clear, border). Just pass it the full `area`.
- The `FuzzyModal` widget at `new_session_dialog/fuzzy_modal.rs:35-50` already self-positions at the bottom ~50% of the given area. Just pass it the full `area`.
- Both widgets call `Clear.render(modal_area, buf)` internally, so they properly overwrite the settings content behind them.
- The `modal_overlay::dim_background` function iterates all cells in the area and applies a dim modifier — this creates the visual effect of the background being "behind" the modal.
- Make sure the `DartDefinesModal` and `FuzzyModal` types are accessible from the `settings_panel` module. They are `pub` in the `new_session_dialog` widget module.
- The `DartDefinesModal::new()` takes `&DartDefinesModalState`; `FuzzyModal::new()` takes `&FuzzyModalState` — both are borrowed, matching the `&SettingsViewState` reference in the render method.
