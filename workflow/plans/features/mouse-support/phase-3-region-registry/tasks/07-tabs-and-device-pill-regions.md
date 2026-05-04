## Task: Record Session Tab and Device Pill Regions

**Objective**: In `widgets/tabs.rs`, record one click region per visible session tab (left-click → `SelectSessionByIndex(i)`, middle-click → `CloseSessionAt(i)`). In single-session mode, additionally record the device pill rect (left-click → `OpenNewSessionDialog`). Multi-session device pill remains unwired (the Phase 3 plan defers it).

**Depends on**: 02 (for `Message::CloseSessionAt`), 04 (for `MouseCtx`)

**Estimated Time**: 1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/tabs.rs`: Add a `render_session_tabs` free function that mirrors the architectural choice from Task 06 (Option B — free function with explicit `Option<&mut MouseCtx>`). Record per-tab and device-pill rects. Add snapshot tests.
- `crates/fdemon-tui/src/widgets/header.rs`: At the `SessionTabs::new(...).render(...)` call site (line 86-89), switch to `widgets::tabs::render_session_tabs(tabs_area, buf, &session_manager, icons, ctx_opt)`. Pass through the `MouseCtx` if Header was given one.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs` (Task 01): `MouseRect`, `MouseAction`.
- `crates/fdemon-app/src/message.rs`: `SelectSessionByIndex`, `CloseSessionAt` (Task 02), `OpenNewSessionDialog`.
- `crates/fdemon-tui/src/render/mod.rs` (Task 04): `MouseCtx`.
- `crates/fdemon-app/src/session_manager.rs`: `len`, `iter`, `selected_index`.

### Details

#### Two render paths

`SessionTabs::render` already branches on session count:
- 1 session → `render_single_session(area, buf)` shows the device pill.
- 2+ sessions → `render_tabs(area, buf)` shows the ratatui `Tabs` widget.

Both paths need region recording. Use the same Option B pattern as Task 06: introduce a free function `render_session_tabs(area, buf, session_manager, icons, ctx)` and have the existing `Widget::render` impl delegate with `ctx = None`.

#### Single-session device pill

The single-session header renders inside `tabs.rs::render_single_session` (lines 56-84). The visible content is `<icon> <name>` rendered into a padded area (`area.x + 1, area.y, area.width - 2, area.height`).

Register the entire content rect (after the 1-cell left padding) as a left-click region → `Message::OpenNewSessionDialog`. The PLAN.md spec: "the device pill is also clickable → `Message::OpenNewSessionDialog` (so the user can quickly switch). Out of scope: device pill click in multi-session mode (no obvious action)."

```rust
fn render_single_session(
    area: Rect,
    buf: &mut Buffer,
    session_manager: &SessionManager,
    icons: IconSet,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    if let Some(handle) = session_manager.selected() {
        // ...existing rendering body...

        // After rendering, register the click region for the padded area.
        if let Some(ctx) = ctx {
            // Use the actual content extent (icon + name + padding) — not the
            // full padded area, to avoid swallowing distant clicks. A simple
            // overestimate is the padded_area; the user "clicks the pill"
            // means the visible text plus its surrounding cell or two.
            let click_rect = MouseRect::new(
                padded_area.x,
                padded_area.y,
                padded_area.width,
                padded_area.height.max(1),
            );
            ctx.click(click_rect, MouseAction::Emit(Message::OpenNewSessionDialog));
        }
    }
}
```

#### Multi-session tabs

The `Tabs` widget renders titles separated by ` │ `. Each title is `<padding> <icon> <name> <padding>` — variable width. We need per-tab rects, which means we need to compute each tab's `(start_x, width)` ourselves (the ratatui `Tabs` widget does not expose this).

Approach: compute the tab widths from the same `tab_titles()` method, accumulate across the divider, push regions as we go:

```rust
fn render_session_tabs(
    area: Rect,
    buf: &mut Buffer,
    session_manager: &SessionManager,
    icons: IconSet,
    mut ctx: Option<&mut MouseCtx<'_>>,
) {
    if session_manager.is_empty() { return; }

    if session_manager.len() == 1 {
        render_single_session(area, buf, session_manager, icons, ctx.as_deref_mut());
        return;
    }

    // Multi-session — render ratatui Tabs and register one region per tab.
    let titles = build_tab_titles(session_manager, icons);
    let selected = session_manager.selected_index();
    let tabs = Tabs::new(titles.clone())
        .select(selected)
        .highlight_style(crate::theme::styles::focused_selected())
        .divider("│");

    let padded_area = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };
    tabs.render(padded_area, buf);

    if let Some(ctx) = ctx {
        // Compute per-tab widths and register regions.
        // Tabs renders as: <title0> │ <title1> │ <title2>
        // Each title's display width is title.width() (ratatui's Line::width).
        const DIVIDER_WIDTH: u16 = 3; // " │ " is 3 cells in the default style
        let mut cursor_x = padded_area.x;
        for (idx, title) in titles.iter().enumerate() {
            let w = title.width() as u16;
            // Skip if this tab is past the right edge.
            if cursor_x + w > padded_area.x + padded_area.width {
                break;
            }
            ctx.click_left_middle(
                MouseRect::new(cursor_x, padded_area.y, w, padded_area.height.max(1)),
                MouseAction::Emit(Message::SelectSessionByIndex(idx)),
                MouseAction::Emit(Message::CloseSessionAt(idx)),
            );
            cursor_x = cursor_x.saturating_add(w + DIVIDER_WIDTH);
        }
    }
}
```

**Important**: `DIVIDER_WIDTH = 3` assumes the divider renders as `" │ "` (space-pipe-space). Verify by looking at the ratatui `Tabs` source or reading the rendered buffer. If the divider is just `"│"` (1 cell), use `DIVIDER_WIDTH = 1`. Add a unit test that pre-measures the rendered buffer to lock in the right number — see Testing.

#### Header re-wiring

In `header.rs::render` (line 86-89), the multi-session branch currently does:

```rust
let tabs = SessionTabs::new(session_manager, self.icons);
tabs.render(tabs_area, buf);
```

Change to call the new free function:

```rust
crate::widgets::tabs::render_session_tabs(
    tabs_area,
    buf,
    session_manager,
    self.icons,
    ctx.as_deref_mut(),
);
```

This requires `header.rs::render` (or the new `render_main_header` from Task 06) to have access to the same `ctx`. Thread it through.

The single-session branch in `header.rs` (line 95-97 — `render_title_row(inner, buf, true)`) is owned by Task 06 (header bracketed shortcuts). The device pill in single-session header is a separate render path inside `render_title_row` (lines 190-209). Decide ownership:

- **Option A**: Task 06 records the device pill (since the single-session pill is rendered inside `render_title_row`).
- **Option B**: Task 07 records the device pill (since "the pill" is conceptually a tab UI element).

**Decision**: Option A — Task 06 records the single-session device pill because it lives in `header.rs`. **Cross-reference this in Task 06**: add a one-line "also register the device-pill rect → `OpenNewSessionDialog` if `show_device && device_name.is_some()`" to Task 06's acceptance criteria.

This task (Task 07) records the multi-session tab regions only.

**Implementor note**: when picking up Task 07, double-check that Task 06's `render_main_header` correctly handles the device pill registration. If not, decide between (a) bouncing back to Task 06 or (b) handling it here as a one-line addition for completeness. Communicate the decision in the completion summary.

### Acceptance Criteria

1. `widgets::tabs::render_session_tabs(area, buf, session_manager, icons, ctx)` exists and is called from `header.rs` for the multi-session branch.
2. The `Widget for SessionTabs` impl continues to work for tests via `term.render_widget(tabs, area)` — it delegates to `render_session_tabs(... , None)`.
3. Multi-session: one region per tab, ordered left-to-right, with width matching the rendered tab title.
4. Each multi-session tab region binds:
   - Left-click → `Message::SelectSessionByIndex(idx)`
   - Middle-click → `Message::CloseSessionAt(idx)`
5. Single-session: the existing `render_single_session` path emits one click region covering the rendered pill area, bound to `Message::OpenNewSessionDialog`. (See Note above — coordinate with Task 06 if the pill is in fact owned by `header.rs::render_title_row`.)
6. No regions registered when `session_manager.is_empty()`.
7. `cargo test --workspace` passes.
8. `cargo clippy --workspace --all-targets -- -D warnings` passes.

### Testing

Add to `widgets/tabs.rs::tests`:

```rust
#[test]
fn multi_session_records_one_region_per_tab() {
    use fdemon_app::{AppState, MouseAction, Message};
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    state
        .session_manager
        .create_session(&test_device("d2", "Pixel"))
        .unwrap();
    state
        .session_manager
        .create_session(&test_device("d3", "Web"))
        .unwrap();

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let tab_regions: Vec<_> = regions
        .iter()
        .filter(|e| matches!(
            e.on_left,
            Some(MouseAction::Emit(Message::SelectSessionByIndex(_)))
        ))
        .collect();
    assert_eq!(tab_regions.len(), 3, "one region per session");

    // Each region also has a middle-click binding.
    for entry in &tab_regions {
        assert!(matches!(
            entry.on_middle,
            Some(MouseAction::Emit(Message::CloseSessionAt(_)))
        ), "middle-click → CloseSessionAt");
    }

    // Indices should be 0, 1, 2 in order.
    let mut indices: Vec<usize> = tab_regions
        .iter()
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(Message::SelectSessionByIndex(i))) => Some(*i),
            _ => None,
        })
        .collect();
    indices.sort();
    assert_eq!(indices, vec![0, 1, 2]);
}

#[test]
fn nine_sessions_record_nine_tab_regions() {
    use fdemon_app::{AppState, MouseAction, Message};
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(160, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    for i in 0..9 {
        state
            .session_manager
            .create_session(&test_device(&format!("d{}", i), &format!("Dev {}", i)))
            .unwrap();
    }

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let count = regions
        .iter()
        .filter(|e| matches!(
            e.on_left,
            Some(MouseAction::Emit(Message::SelectSessionByIndex(_)))
        ))
        .count();
    assert_eq!(count, 9);
}

#[test]
fn divider_width_matches_rendered_buffer() {
    // Sanity-test the DIVIDER_WIDTH constant by measuring the rendered buffer.
    // The Tabs widget renders titles separated by " │ " — verify by reading
    // the cells between two known tab title positions.
    use fdemon_app::AppState;
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    state
        .session_manager
        .create_session(&test_device("d2", "Pixel"))
        .unwrap();
    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let buffer = terminal.backend().buffer();
    let line: String = (0..120)
        .map(|x| buffer.cell((x, 4)).map(|c| c.symbol().to_string()).unwrap_or_default())
        .collect();
    // The divider character is │; the cells around it are spaces.
    assert!(line.contains(" │ "), "divider must be ` │ ` (3 cells); got: {:?}", line);
}
```

### Notes

- The `DIVIDER_WIDTH` constant is the most fragile piece of this task. The `divider_width_matches_rendered_buffer` test pins it. If the divider style ever changes (e.g., we switch to a thicker glyph), update both the constant and the test.
- Multi-session tab regions deliberately span the full tab-title area (icon + name + padding) — clicking anywhere on the tab selects/closes that session. This matches user expectation from terminal browsers (Alacritty, kitty) where the entire tab cell is clickable.
- If the tab list overflows the visible area (unlikely at 9 sessions × ~14 chars/tab + dividers = ~135 chars; well within 160-col tests), break out of the registration loop early and do NOT push regions for tabs that are clipped off-screen.
- The existing `truncate_name` helper at `tabs.rs:124` keeps tab titles ≤ 12 chars + decoration. Worst case: 9 tabs × (1 padding + 1 icon + 1 space + 12 name + 1 padding) + 8 dividers × 3 = 144 + 24 = 168 chars. Above 168 cols all fit; below, the late tabs clip. Tests at 120 cols may show only 5-6 tabs — that is by design.
- Right-click on tabs is intentionally unbound (PLAN.md "Out of scope (v1)"). Phase 3 does not register a `Right` action for any tab.
