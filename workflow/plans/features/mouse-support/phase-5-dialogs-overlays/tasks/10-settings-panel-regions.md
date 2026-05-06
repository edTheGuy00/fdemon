## Task: Settings Panel Regions

**Objective**: Fill in `widgets::settings_panel::render_with_regions` so the four tab headers (`1. PROJECT` / `2. USER` / `3. LAUNCH` / `4. VSCODE`) and each visible setting row become clickable. Tab headers emit `Message::SettingsGotoTab(i)`; setting rows emit `Message::SettingsClickRow { index }` (single-click selects, double-click toggles edit — handled by Task 03's chained-message logic). Settings is a full-screen panel — no overlay competes for these cells, so all regions register at `z_index = 0`. **Sub-modals (dart-defines, extra-args) are deferred to Phase 6 polish.**

**Depends on**: 01 (Phase-5 messages), 02 (sister `render_with_regions` stub)

**Estimated Time**: 1.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Replace the stub `render_with_regions` body with real implementation. Tab headers + per-row registrations. The existing `StatefulWidget::render` impl is **unchanged**.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs::SettingsViewState` — `active_tab`, `selected_index`, `editing`.
- `crates/fdemon-app/src/config/SettingsTab` — `Project`, `UserPrefs`, `LaunchConfig`, `VSCodeConfig`.
- `crates/fdemon-app/src/settings_items.rs` — `project_settings_items`, `user_prefs_items`, `launch_config_items`, `vscode_config_items` (used by the renderer to enumerate rows).
- `crates/fdemon-app/src/mouse_regions.rs` — `MouseRect`, `MouseAction::emit`, `MouseRegionsBuilder::click`.

### Details

#### Tab header regions

In `render_tab_bar`, four tabs are rendered at fixed `tab_width = 12` columns each starting at `area.left()`, with a 1-column gap. The rect for tab `i` is:

```rust
let tab_rect = Rect::new(area.left() + i * (tab_width + gap), area.top(), tab_width, 1);
```

Region recording:

```rust
let tabs = [
    SettingsTab::Project,
    SettingsTab::UserPrefs,
    SettingsTab::LaunchConfig,
    SettingsTab::VSCodeConfig,
];

let mut x = area.left();
for (i, _tab) in tabs.iter().enumerate() {
    if x + tab_width > area.right() {
        break;
    }
    let rect = MouseRect::new(x, area.top(), tab_width, 1);
    if !rect.is_empty() {
        ctx.click(rect, MouseAction::emit(Message::SettingsGotoTab(i)));
    }
    x += tab_width + gap;
}
```

#### Setting row regions

Each tab renders its rows via `render_setting_row` / `render_user_pref_row` / equivalents. The renderer increments `y` per row and skips section header rows (which are not clickable — they're decorative section labels like `B E H A V I O R`).

**Important**: section headers are inserted between groups of items. The `index` in `Message::SettingsClickRow { index }` must be the *flat item index* into the active tab's `Vec<SettingItem>`, not a row-counted index that includes section headers.

The cleanest factoring:

```rust
// In render_with_regions, after the layout calculation:
let items = match state.active_tab {
    SettingsTab::Project => project_settings_items(view.settings),
    SettingsTab::UserPrefs => user_prefs_items(&state.user_prefs, view.settings),
    SettingsTab::LaunchConfig => launch_config_items(/* ... */),
    SettingsTab::VSCodeConfig => vscode_config_items(/* ... */),
};

// Walk the items and record rect per row, mirroring the renderer's section-header skipping logic.
let mut current_section = String::new();
let mut y = inner.y;
for (idx, item) in items.iter().enumerate() {
    if y >= inner.bottom() {
        break;
    }
    if item.section != current_section {
        if !current_section.is_empty() {
            y += 1;
        }
        if y < inner.bottom() {
            // Section header row — NOT clickable.
            y += 1;
        }
        current_section = item.section.clone();
    }
    if y < inner.bottom() {
        let rect = MouseRect::new(inner.x, y, inner.width, 1);
        if !rect.is_empty() {
            ctx.click(rect, MouseAction::emit(Message::SettingsClickRow { index: idx }));
        }
        y += 1;
    }
}
```

Some tabs (UserPrefs, LaunchConfig) render an info banner above the items (`render_user_prefs_info`, `render_launch_config_info` etc.) — those are NOT clickable. The Y-offset calculation must account for the banner's height when computing the first item's `y`.

#### `render_with_regions` signature

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: SettingsPanel<'_>,
    state: &mut SettingsViewState,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Render the panel via the existing StatefulWidget::render to keep visual
    // output byte-identical. Then walk the layout to record regions.
    //
    // ALTERNATIVE (recommended for cleanliness): re-implement the layout
    // calculation here, threading ctx into render_header / render_content /
    // render_footer through new optional ctx parameters.

    // ── Implementation outline ───────────────────────────────────────────

    // 1. Render the panel (delegate to StatefulWidget::render).
    //    OR recompute layout & call self.render_header_with_regions(...) etc.

    // 2. If ctx is Some, register regions:
    //    - Tab headers (4 regions, z=0)
    //    - Visible setting rows for the active tab (N regions, z=0)
    //    - Sub-modal regions: deferred to Phase 6

    // 3. Footer: not clickable in v1.

    // ── Sub-modal note ───────────────────────────────────────────────────
    //
    // When `state.dart_defines_modal.is_some()` or `state.extra_args_modal.is_some()`,
    // we still record the underlying tab+row regions but DO NOT register
    // sub-modal regions. The dispatcher's editing-gate (Task 05) suppresses
    // clicks while editing, but sub-modal mode is NOT `editing == true` — it
    // is a separate state. Phase 6 polish will:
    //   (a) detect sub-modal-open in this function and skip recording the
    //       underlying rows (so clicks underneath don't fire), or
    //   (b) register sub-modal regions at z=2 so they shadow the underlying
    //       rows on the same cells.
    //
    // For v1, we accept that clicks on the visible underlying area while a
    // sub-modal is open do nothing (the sub-modal renders on top, so the
    // click coordinate doesn't actually land on a recorded row in most cases —
    // the sub-modal occupies the central area). The handler dispatcher may
    // also short-circuit clicks when a sub-modal is open. See Notes for
    // discussion.
}
```

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — existing settings-panel tests pass; new tests below are added.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. With Settings open on the Project tab, the registry contains 4 tab regions plus N setting-row regions, where N is the visible item count for the Project tab.
5. Each tab region's `MouseAction` is `Emit(Message::SettingsGotoTab(i))` for `i ∈ {0, 1, 2, 3}`.
6. Each setting-row region's `MouseAction` is `Emit(Message::SettingsClickRow { index })` where `index` is the flat item index into the tab's `Vec<SettingItem>` — section header rows are NOT registered.
7. All Phase-5 settings regions register at `z_index = 0`.
8. Visual output (rendered cells) is byte-identical to the pre-task render. Verified by extending an existing test.
9. When a sub-modal (dart-defines or extra-args) is open, the function STILL registers the underlying tab + row regions (sub-modal click suppression is the dispatcher's responsibility — Task 05's editing gate, plus a Phase 6 follow-up).
10. The header `[Esc] Close` hint is **not** clickable in v1 (clicking it currently triggers `Esc` close in the keyboard handler, but adding a click region would conflict with the hint area — defer to Phase 6).

### Testing

Add unit tests inside `widgets/settings_panel/mod.rs::tests`:

```rust
#[test]
fn render_with_regions_records_four_tab_headers() {
    use fdemon_app::{message::Message, mouse_regions::MouseRegions, MouseCtx};

    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let panel = SettingsPanel::new(&settings, project_path);
    let mut state = SettingsViewState::default();

    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 40));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 100, 40), &mut buf, panel, &mut state, Some(&mut ctx));
    }

    let tab_count = regions.iter().filter(|e| matches!(
        extract_action(e),
        Some(Message::SettingsGotoTab(_))
    )).count();
    assert_eq!(tab_count, 4, "expected 4 tab-header regions");

    // All regions register at z=0 (full-screen panel).
    for entry in regions.iter() {
        assert_eq!(entry.z_index, 0);
    }
}

#[test]
fn render_with_regions_records_one_region_per_visible_setting_row() {
    // Render with the Project tab active. Count SettingsClickRow regions —
    // must equal the number of items returned by project_settings_items().
    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let panel = SettingsPanel::new(&settings, project_path);
    let mut state = SettingsViewState::default();
    state.active_tab = SettingsTab::Project;

    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 60));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 100, 60), &mut buf, panel, &mut state, Some(&mut ctx));
    }

    let row_count = regions.iter().filter(|e| matches!(
        extract_action(e),
        Some(Message::SettingsClickRow { .. })
    )).count();
    let expected = project_settings_items(&settings).len();
    // Allow row_count <= expected because some rows may scroll off-screen.
    assert!(row_count > 0 && row_count <= expected);
}

#[test]
fn render_with_regions_indices_match_item_positions() {
    // Click the third item — expect SettingsClickRow { index: 2 }.
    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let panel = SettingsPanel::new(&settings, project_path);
    let mut state = SettingsViewState::default();

    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 60));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 100, 60), &mut buf, panel, &mut state, Some(&mut ctx));
    }

    // Collect the recorded indices in registration order.
    let indices: Vec<usize> = regions.iter().filter_map(|e| match extract_action(e) {
        Some(Message::SettingsClickRow { index }) => Some(index),
        _ => None,
    }).collect();
    // Indices must be strictly increasing AND start at 0.
    assert!(indices.first() == Some(&0));
    for window in indices.windows(2) {
        assert!(window[0] < window[1]);
    }
}

#[test]
fn render_with_regions_section_headers_are_not_clickable() {
    // Verify that the row count of registered click regions equals the
    // number of items, NOT items + section headers.
    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let panel = SettingsPanel::new(&settings, project_path);
    let mut state = SettingsViewState::default();
    state.active_tab = SettingsTab::Project;

    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 80)); // tall enough to render all rows
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 100, 80), &mut buf, panel, &mut state, Some(&mut ctx));
    }

    let row_count = regions.iter().filter(|e| matches!(
        extract_action(e),
        Some(Message::SettingsClickRow { .. })
    )).count();
    let expected = project_settings_items(&settings).len();
    assert_eq!(row_count, expected, "all items registered, no section-header regions");
}

#[test]
fn render_with_regions_visual_output_unchanged() {
    let settings = Settings::default();
    let project_path = std::path::Path::new("/tmp/test");
    let mut state_a = SettingsViewState::default();
    let mut state_b = SettingsViewState::default();

    let mut buf_widget = Buffer::empty(Rect::new(0, 0, 100, 40));
    let mut buf_with_regions = Buffer::empty(Rect::new(0, 0, 100, 40));

    let panel_a = SettingsPanel::new(&settings, project_path);
    StatefulWidget::render(panel_a, Rect::new(0, 0, 100, 40), &mut buf_widget, &mut state_a);

    let panel_b = SettingsPanel::new(&settings, project_path);
    super::render_with_regions(Rect::new(0, 0, 100, 40), &mut buf_with_regions, panel_b, &mut state_b, None);

    assert_eq!(buf_widget, buf_with_regions);
}
```

### Notes

- **Why `z_index = 0`.** The Settings panel takes the full screen — there is no underlying base UI to compete with. Sub-modals (dart-defines, extra-args) when added in Phase 6 will register at z=1 (they are modals over the panel, but the panel itself is the "base" for that hierarchy).
- **Why section-header rows are not clickable.** They are decorative — `B E H A V I O R`, `W A T C H E R`, etc. They have no message to emit and no useful interactivity.
- **Why the header's `[Esc] Close` hint is not clickable in v1.** Adding a region for it would require careful rect math (the hint is right-aligned in the header) and disambiguation from the `System Settings` title text. Phase 6 polish.
- **Why footer hints are not clickable.** Footer hints (`Tab: Switch tabs`, `j/k: Navigate`, `Enter: Edit`, `Ctrl+S: Save Changes`) are reminders, not interactive elements. Each hint references a keyboard shortcut; mouse interaction with these would be redundant with the actual settings rows + dedicated buttons. Phase 6 may revisit `Save Changes` as a clickable button.
- **Why we don't auto-suppress clicks when a sub-modal is open.** v1 accepts the inconsistency: with a sub-modal open, the visible non-modal area covers the rendered modal background (dimmed cells), and the sub-modal itself doesn't have click regions yet. A click on a still-recorded underlying row would fire `SettingsClickRow`, but the dispatcher's editing-gate (Task 05) returns `None` when `editing == true`. Sub-modal-open is not `editing` — so this gap exists. Phase 6 fixes it by either: (a) skipping underlying registration when a sub-modal is open, or (b) registering sub-modal regions at z=1 to shadow.
- **Why we mirror the renderer's section-skip logic.** The renderer at `render_project_tab` walks `items` and inserts a `y += 1` row before each new section. The region-recording walk must do the same to keep `y` in sync with where rows actually land in the buffer. If the row-y miscounts, click rects fire on the wrong row — confusing and hard to debug. The flat `index` into `items` (which the registry stores) matches what the keyboard handler `SettingsNextItem`/`SettingsPrevItem` operates on, so click → keyboard parity is preserved.
