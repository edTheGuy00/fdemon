## Task: TUI Region Plumbing & DevTools Sub-Tab Regions

**Objective**: Introduce a sister `render_with_regions(...)` free function for `LogView`, `DevToolsView`, `WidgetInspector`, `PerformancePanel`, and `NetworkMonitor` so widgets can record click rects without changing the existing `Widget` / `StatefulWidget` impls. Update `render::view()` to call the new functions, threading `&mut MouseCtx` from frame setup down into every clickable surface. As the only actual region recording in this task, register `[i] Inspector` / `[p] Performance` / `[n] Network` rects in `widgets/devtools/mod.rs::render_tab_bar` so DevTools sub-tab clicks become live the moment Wave 2 starts.

**Depends on**: None (Wave 1, parallel with Task 01)

**Estimated Time**: 2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/render/mod.rs`: Replace direct `frame.render_widget(devtools, areas.logs)` and `frame.render_stateful_widget(log_view, ..., &mut state)` calls with the new `widgets::log_view::render_with_regions(...)` and `widgets::devtools::render_with_regions(...)` calls. `&mut mouse_ctx` is forwarded as `Option<&mut MouseCtx<'_>>`.
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Add `pub fn render_with_regions(area: Rect, buf: &mut Buffer, state: &mut LogViewState, view: LogView<'_>, _ctx: Option<&mut MouseCtx<'_>>)` that delegates to `<LogView as StatefulWidget>::render(view, area, buf, state)` and ignores `_ctx` for now. Task 06 fills in the body.
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Add `pub fn render_with_regions(area: Rect, buf: &mut Buffer, view: DevToolsView<'_>, ctx: Option<&mut MouseCtx<'_>>)`. The function:
  1. Splits the layout (sub-tab bar + panel content) the same way the existing `Widget::render` impl does.
  2. Calls `view.render_tab_bar(chunks[0], buf)` — modified to accept `Option<&mut MouseCtx>` and register one `MouseAction::Emit(Message::SwitchDevToolsPanel(panel))` per `[i]` / `[p]` / `[n]` rect.
  3. Dispatches to `inspector::render_with_regions(...)`, `performance::render_with_regions(...)`, or `network::render_with_regions(...)` as appropriate, forwarding `ctx`.
  4. Renders the footer (no clicks).
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`: Add `pub fn render_with_regions(area: Rect, buf: &mut Buffer, widget: WidgetInspector<'_>, _ctx: Option<&mut MouseCtx<'_>>)` that delegates to `<WidgetInspector as Widget>::render(widget, area, buf)` for now. Task 07 fills in the body.
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`: Add `pub fn render_with_regions(area: Rect, buf: &mut Buffer, widget: PerformancePanel<'_>, _ctx: Option<&mut MouseCtx<'_>>)` that delegates to `<PerformancePanel as Widget>::render`. Task 08 fills in the body.
- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`: Add `pub fn render_with_regions(area: Rect, buf: &mut Buffer, widget: NetworkMonitor<'_>, _ctx: Option<&mut MouseCtx<'_>>)` that delegates to `<NetworkMonitor as Widget>::render`. Task 09 fills in the body.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs` (for `MouseAction::Emit`, `MouseRect`)
- `crates/fdemon-app/src/message.rs` (for `Message::SwitchDevToolsPanel`, `DevToolsPanel`)
- `crates/fdemon-tui/src/widgets/header.rs` and `crates/fdemon-tui/src/widgets/tabs.rs` (templates — existing sister-function precedent from Phase 3 Tasks 06 / 07)

### Details

#### Sister-function pattern recap

Phase 3 introduced `widgets::header::render_main_header(area, buf, &header, Some(&mut mouse_ctx))` as a free function alongside `MainHeader`'s `Widget` impl. The free function is the click-aware variant; the `Widget` impl stays for tests and other call paths that don't need region recording. Phase 4 generalises that pattern: every widget that has clickable surfaces gains a sister free function whose name is `render_with_regions`.

Existing `Widget::render` / `StatefulWidget::render` impls are NOT modified in this task. The sister function delegates to them when `ctx` is `None` (or when this task lands and Tasks 06–09 haven't yet filled in real region recording).

#### `render::view()` call-site changes

Replace the existing log-view block:

```rust
// Before (current):
frame.render_stateful_widget(log_view, areas.logs, &mut handle.session.log_view_state);

// After (Task 02):
widgets::log_view::render_with_regions(
    areas.logs,
    frame.buffer_mut(),
    &mut handle.session.log_view_state,
    log_view,
    Some(&mut mouse_ctx),
);
```

Replace the existing DevTools block:

```rust
// Before:
let devtools = widgets::devtools::DevToolsView::new(
    &state.devtools_view_state,
    state.session_manager.selected(),
    icons,
);
frame.render_widget(devtools, areas.logs);

// After:
let devtools = widgets::devtools::DevToolsView::new(
    &state.devtools_view_state,
    state.session_manager.selected(),
    icons,
);
widgets::devtools::render_with_regions(
    areas.logs,
    frame.buffer_mut(),
    devtools,
    Some(&mut mouse_ctx),
);
```

The empty-session `frame.render_stateful_widget(log_view, areas.logs, &mut empty_state)` path (when `state.session_manager.selected_mut()` is `None`) should also use the sister function for consistency, even though it has no clickable surfaces.

#### Sub-tab bar regions in `widgets/devtools/mod.rs`

Modify the existing `render_tab_bar` to accept `Option<&mut MouseCtx>`:

```rust
fn render_tab_bar(&self, area: Rect, buf: &mut Buffer, ctx: Option<&mut MouseCtx>) {
    // ... existing block + inner setup ...

    let tabs = [
        (DevToolsPanel::Inspector, "[i] Inspector"),
        (DevToolsPanel::Performance, "[p] Performance"),
        (DevToolsPanel::Network, "[n] Network"),
    ];

    let mut x = inner.x + 1;
    let mut ctx = ctx; // re-bind so we can `as_deref_mut` per iteration

    for (panel, label) in &tabs {
        let padded = format!(" {label} ");
        let needed_width = padded.len() as u16;

        if x + needed_width > inner.right() {
            break;
        }

        // ... existing style + buf.set_string ...
        buf.set_string(x, inner.y, &padded, style);

        // Register a click region covering the padded label cells.
        if let Some(ref mut c) = ctx {
            let rect = MouseRect::new(x, inner.y, needed_width, 1);
            // Skip empty rects (defensive — needed_width > 0 always here).
            if rect.width > 0 && rect.height > 0 {
                c.click(
                    rect,
                    MouseAction::emit(Message::SwitchDevToolsPanel(*panel)),
                );
            }
        }

        x += needed_width + 1;
    }

    // ... existing right-aligned indicator block (no changes) ...
}
```

The signature change cascades: callers of `render_tab_bar` inside `Widget::render` (untouched) pass `None`; the new `render_with_regions` passes `Some(&mut mouse_ctx)`.

To avoid mutating `Widget::render`, restructure: extract a private helper `fn render_tab_bar_inner(&self, area: Rect, buf: &mut Buffer, ctx: Option<&mut MouseCtx>)` and have `Widget::render` call `self.render_tab_bar_inner(chunks[0], buf, None)` — the new `render_with_regions` passes `Some(...)`.

#### `render_with_regions` for `DevToolsView`

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: DevToolsView<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Background fill — same as existing Widget::render.
    let bg_style = Style::default().bg(palette::DEEPEST_BG);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(bg_style).set_char(' ');
            }
        }
    }

    if area.height < DEVTOOLS_MIN_HEIGHT || area.width < DEVTOOLS_MIN_WIDTH {
        // Min-size message — same as existing Widget::render.
        // ...
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(area);

    // Sub-tab bar with click registration.
    let mut ctx = ctx;
    view.render_tab_bar_inner(chunks[0], buf, ctx.as_deref_mut());

    // Panel dispatch — sister functions delegate to existing Widget::render
    // until Tasks 07–09 fill them in.
    match view.state.active_panel {
        DevToolsPanel::Inspector => {
            // Build WidgetInspector — same setup as Widget::render.
            let widget = WidgetInspector::new(
                &view.state.inspector,
                vm_connected,
                &view.state.connection_status,
            );
            inspector::render_with_regions(chunks[1], buf, widget, ctx.as_deref_mut());
        }
        DevToolsPanel::Performance => {
            // ... PerformancePanel build ...
            performance::render_with_regions(chunks[1], buf, widget, ctx.as_deref_mut());
        }
        DevToolsPanel::Network => {
            // ... NetworkMonitor build ...
            network::render_with_regions(chunks[1], buf, widget, ctx.as_deref_mut());
        }
    }

    // Footer — no clicks.
    view.render_footer(chunks[1], buf);
}
```

#### Sub-panel `render_with_regions` stubs

Each of `inspector::render_with_regions`, `performance::render_with_regions`, `network::render_with_regions` is a no-op delegate in this task:

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    widget: WidgetInspector<'_>,
    _ctx: Option<&mut MouseCtx<'_>>,
) {
    // Phase 4 Task 07 fills in the body.
    <WidgetInspector as Widget>::render(widget, area, buf);
}
```

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — every existing test continues to work because `render_with_regions` delegates to the existing `Widget::render` for the no-region path, and the new region-recording in `render_tab_bar_inner` only fires when `ctx` is `Some`.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. `widgets::log_view::render_with_regions`, `widgets::devtools::render_with_regions`, `widgets::devtools::inspector::render_with_regions`, `widgets::devtools::performance::render_with_regions`, `widgets::devtools::network::render_with_regions` all exist with the signatures specified above.
5. `render::view()` calls `widgets::log_view::render_with_regions` and `widgets::devtools::render_with_regions` (replacing the previous `frame.render_stateful_widget` / `frame.render_widget` calls).
6. After this task, opening DevTools mode and clicking `[p] Performance` switches the active panel — verified by a unit test on the sub-tab bar registry.
7. The `[i]` / `[p]` / `[n]` rects each cover exactly the padded `format!(" {label} ")` cells (`needed_width` columns wide, 1 row tall, starting at the running `x` coordinate). No leading-space-only or trailing-space-only fragment is registered.
8. The `Widget::render` and `StatefulWidget::render` impls of all touched widgets are unchanged in behaviour — pre-existing tests that render via `widget.render(area, buf)` continue to pass without modification.

### Testing

Add a unit test inside `widgets/devtools/mod.rs::tests`:

```rust
#[test]
fn devtools_tab_bar_registers_three_click_regions() {
    use fdemon_app::{state::DevToolsViewState, MouseRegions};
    use fdemon_app::message::Message;

    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    let state = DevToolsViewState::default();
    let view = DevToolsView::new(&state, None, IconSet::new(true));

    let mut regions = MouseRegions::default();
    {
        let mut builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 80, 24), &mut buf, view, Some(&mut ctx));
    }

    // Filter to sub-tab regions: each Emit(SwitchDevToolsPanel(...)).
    let switch_panel_count = regions
        .iter() // adjust to actual public iterator API
        .filter(|e| matches!(
            e.on_left.as_ref().and_then(|a| a.as_emit()),
            Some(Message::SwitchDevToolsPanel(_))
        ))
        .count();
    assert_eq!(switch_panel_count, 3, "expected 3 sub-tab regions");
}
```

(Adjust the iteration helper to match the actual `MouseRegions` public API after Phase 3 Task 01.)

### Notes

- **Why a free function instead of a builder method on the widget.** Builder methods (`fn with_mouse_ctx(self, ctx) -> Self`) tie the lifetime of `MouseCtx` to the widget's `'a` lifetime, which the `Widget::render(self, ...)` consumed-self pattern fights. A standalone free function takes the widget by value and the ctx separately — Phase 3's `render_main_header` already validated this pattern.
- **Why `Option<&mut MouseCtx>` everywhere.** Some test paths render widgets without a registry (e.g., the existing `widgets/devtools/mod.rs::tests` that just check pixel layout). Passing `None` keeps those tests working without exposing a separate `render_no_regions` variant.
- **Why register sub-tab regions in this task and not a separate Task 05/06/07.** The sub-tab bar lives entirely in `widgets/devtools/mod.rs`, which Task 02 already touches for plumbing. Splitting registration into a separate task would mean two writes to the same file with no parallelism win. By bundling, Wave 2 can focus on the four panel-internal surfaces in parallel without anyone needing to re-touch `mod.rs`.
- **No changes to `widgets/devtools/inspector/tree_panel.rs`, `widgets/devtools/performance/frame_chart/bars.rs`, or `widgets/devtools/network/request_table.rs` in this task** — those files belong to Tasks 07 / 08 / 09. The sister `render_with_regions` functions delegate to `Widget::render` and never thread `ctx` deeper.
- **`MouseCtx::as_deref_mut`.** The pattern `ctx.as_deref_mut()` lets us pass `Option<&mut MouseCtx>` through multiple layers without losing ownership. If the local `ctx` shadowing reads awkwardly, factor a small helper `fn forward<'a>(ctx: &'a mut Option<&mut MouseCtx<'_>>) -> Option<&'a mut MouseCtx<'_>> { ctx.as_deref_mut() }` — but inline `ctx.as_deref_mut()` is idiomatic.
- **`render_footer` does not get a ctx.** The footer is informational text only; no click targets. Phase 5 may revisit if any footer hint becomes a click target.
