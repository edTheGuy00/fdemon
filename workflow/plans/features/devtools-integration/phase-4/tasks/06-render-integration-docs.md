## Task: DevTools Render Integration & Documentation

**Objective**: Wire all three DevTools panel widgets into the main render pipeline, add a DevTools sub-tab bar, update the header to show contextual key hints in DevTools mode, and update `KEYBINDINGS.md` to document all changes.

**Depends on**: 02-devtools-handlers-key-reassignment, 03-performance-panel-widget, 04-widget-inspector-panel, 05-layout-explorer-panel

**Estimated Time**: 4-6 hours

### Scope

- `crates/fdemon-tui/src/render/mod.rs`: Add `UiMode::DevTools` match arm with full panel rendering
- `crates/fdemon-tui/src/widgets/header.rs`: Contextual key hints for DevTools mode
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Complete module with `DevToolsView` composite widget
- `crates/fdemon-tui/src/widgets/mod.rs`: Add `pub mod devtools;` export
- `docs/KEYBINDINGS.md`: Update `d` key docs, add DevTools section

### Details

#### 1. Add `UiMode::DevTools` Arm to `render/mod.rs`

At `render/mod.rs:124` in the `match state.ui_mode` block, add:

```rust
UiMode::DevTools => {
    // DevTools replaces the log view area entirely (like Settings)
    let devtools = widgets::devtools::DevToolsView::new(
        &state.devtools_view_state,
        state.session_manager.active_session(),
        icons,
    );
    frame.render_widget(devtools, area);
}
```

**Key decision**: DevTools mode renders over `area` (the full terminal area), NOT `areas.logs`. This is the same pattern used by `UiMode::Settings` (line 225-229). The header and tabs from the base rendering (steps 1-5) are still visible underneath but get overwritten by the full-screen DevTools widget. If we want the header to remain, render DevTools into `areas.logs` instead. **Recommendation**: Render into the full `area` so DevTools has maximum space, but include its own header/tab-bar at the top.

Actually, looking at the Settings pattern more carefully: Settings renders over `area` (full screen). But for DevTools, it makes more sense to keep the app header visible (showing project name, session tabs) and render DevTools only into `areas.logs`. This way the user sees which session they're in.

**Revised approach**: Render DevTools into `areas.logs` (below the header):

```rust
UiMode::DevTools => {
    let devtools = widgets::devtools::DevToolsView::new(
        &state.devtools_view_state,
        state.session_manager.active_session(),
        icons,
    );
    frame.render_widget(devtools, areas.logs);
}
```

#### 2. Create `DevToolsView` Composite Widget

This is the top-level DevTools widget that renders the sub-tab bar and dispatches to the active panel:

```rust
// crates/fdemon-tui/src/widgets/devtools/mod.rs

pub mod inspector;
pub mod layout_explorer;
pub mod performance;

pub use inspector::WidgetInspector;
pub use layout_explorer::LayoutExplorer;
pub use performance::PerformancePanel;

use fdemon_app::session::SessionHandle;
use fdemon_app::state::DevToolsViewState;
use fdemon_core::widget_tree::DiagnosticsNode;

/// Top-level DevTools mode widget.
///
/// Renders a sub-tab bar at the top and dispatches to the active panel below.
pub struct DevToolsView<'a> {
    state: &'a DevToolsViewState,
    session: Option<&'a SessionHandle>,
    icons: IconSet,
}

impl<'a> DevToolsView<'a> {
    pub fn new(
        state: &'a DevToolsViewState,
        session: Option<&'a SessionHandle>,
        icons: IconSet,
    ) -> Self {
        Self { state, session, icons }
    }
}

impl Widget for DevToolsView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Vertical layout: [sub-tab bar (3 lines)] + [panel content (remaining)]
        let chunks = Layout::vertical([
            Constraint::Length(3),  // Sub-tab bar
            Constraint::Min(5),    // Panel content
        ]).split(area);

        // Render sub-tab bar
        self.render_tab_bar(chunks[0], buf);

        // Render active panel
        match self.state.active_panel {
            DevToolsPanel::Inspector => {
                let widget = WidgetInspector::new(&self.state.inspector, self.icons);
                widget.render(chunks[1], buf);
            }
            DevToolsPanel::Layout => {
                let selected_name = self.state.inspector.visible_nodes()
                    .get(self.state.inspector.selected_index)
                    .map(|(node, _)| node.description.as_str());

                let widget = LayoutExplorer::new(
                    &self.state.layout_explorer,
                    selected_name,
                    self.icons,
                );
                widget.render(chunks[1], buf);
            }
            DevToolsPanel::Performance => {
                let (perf, vm_connected) = self.session
                    .map(|s| (&s.session.performance, s.session.vm_connected))
                    .unwrap_or_else(|| {
                        // Fallback — shouldn't happen in practice
                        static DEFAULT_PERF: std::sync::LazyLock<PerformanceState> =
                            std::sync::LazyLock::new(PerformanceState::new);
                        (&*DEFAULT_PERF, false)
                    });

                let widget = PerformancePanel::new(perf, vm_connected, self.icons);
                widget.render(chunks[1], buf);
            }
        }

        // Footer with contextual hints
        self.render_footer(chunks[1], buf);
    }
}
```

#### 3. Sub-Tab Bar Rendering

Follow the `SettingsPanel` tab bar pattern from `settings_panel/mod.rs:148-185`:

```rust
fn render_tab_bar(&self, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered()
        .title(" DevTools ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    block.render(area, buf);

    let tabs = [
        (DevToolsPanel::Inspector, "[i] Inspector"),
        (DevToolsPanel::Layout, "[l] Layout"),
        (DevToolsPanel::Performance, "[p] Performance"),
    ];

    let mut x = inner.x + 1;
    for (panel, label) in &tabs {
        let is_active = self.state.active_panel == *panel;
        let style = if is_active {
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let padded = format!(" {label} ");
        buf.set_string(x, inner.y, &padded, style);
        x += padded.len() as u16 + 1;
    }

    // Right-aligned: overlay status indicators
    let mut indicators = Vec::new();
    if self.state.overlay_repaint_rainbow {
        indicators.push("Rainbow");
    }
    if self.state.overlay_debug_paint {
        indicators.push("DebugPaint");
    }
    if self.state.overlay_performance {
        indicators.push("PerfOverlay");
    }
    if !indicators.is_empty() {
        let indicator_text = indicators.join(" | ");
        let right_x = inner.x + inner.width.saturating_sub(indicator_text.len() as u16 + 1);
        buf.set_string(right_x, inner.y, &indicator_text, Style::default().fg(Color::Yellow));
    }
}
```

#### 4. Footer Hints

Render contextual keybinding hints at the bottom of the panel area:

```rust
fn render_footer(&self, area: Rect, buf: &mut Buffer) {
    // Only render if there's room
    if area.height < 2 {
        return;
    }

    let y = area.y + area.height - 1;

    let hints = match self.state.active_panel {
        DevToolsPanel::Inspector => {
            "[Esc] Logs  [↑↓] Navigate  [→] Expand  [←] Collapse  [r] Refresh  [b] Browser"
        }
        DevToolsPanel::Layout => {
            "[Esc] Logs  [i] Inspector  [b] Browser  [Ctrl+r] Rainbow  [Ctrl+d] DebugPaint"
        }
        DevToolsPanel::Performance => {
            "[Esc] Logs  [i] Inspector  [b] Browser  [Ctrl+p] PerfOverlay"
        }
    };

    buf.set_string(
        area.x + 1,
        y,
        hints,
        Style::default().fg(Color::DarkGray),
    );
}
```

#### 5. Update Header Key Hints (`header.rs`)

The header at `header.rs:158-173` hardcodes `[r] Run  [R] Restart  [x] Stop  [d] Debug  [q] Quit`. Update to be mode-aware:

**Option A (Minimal):** Just change the label to match the new `d` behavior:
- The label `[d] Debug` is already accurate since `d` now enters DevTools mode. No change needed! The existing label accidentally describes the new behavior correctly.

**Option B (Full context switching):** Pass `UiMode` to `MainHeader` and show different hints:
- Normal mode: `[r] Run  [R] Restart  [x] Stop  [d] Debug  [q] Quit`
- DevTools mode: The header continues showing the same hints (since it's above the DevTools panel). The DevTools panel has its own footer hints.

**Recommendation**: Use Option A. The current `[d] Debug` label is already correct for the new behavior. The DevTools panel's own footer provides panel-specific hints. No header changes are needed unless the implementer wants to grey out session-control keys while in DevTools mode.

However, there IS a need to update the header builder to accept `UiMode` so that if the user is in DevTools mode, the `[d]` hint can show as active/highlighted. This is a nice-to-have, not a blocker.

#### 6. Add DevTools Module to Widget Exports

In `crates/fdemon-tui/src/widgets/mod.rs`, add:

```rust
pub mod devtools;
```

#### 7. Update `docs/KEYBINDINGS.md`

##### Replace `d` Key Entries

**Session Management section (line 68):**

Before:
```markdown
| `d` | Start New Session | Alternative binding for starting new session |
```

After:
```markdown
| `d` | DevTools Mode | Enter DevTools mode (Inspector/Layout/Performance panels) |
```

**Startup state section (line 54):**

Before:
```markdown
Press `+` or `d` to open the Startup Dialog and configure your first session.
```

After:
```markdown
Press `+` to open the Startup Dialog and configure your first session.
```

##### Add DevTools Mode Section

Add a new section after the Normal Mode section:

```markdown
## DevTools Mode

Enter DevTools mode by pressing `d` in Normal mode (requires VM Service connection).

### Panel Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Exit DevTools | Return to Normal mode (log view) |
| `i` | Inspector Panel | Switch to Widget Inspector panel |
| `l` | Layout Panel | Switch to Layout Explorer panel |
| `p` | Performance Panel | Switch to Performance monitoring panel |
| `b` | Browser DevTools | Open Flutter DevTools in system browser |
| `q` | Quit | Quit the application |

### Debug Overlays

| Key | Action | Description |
|-----|--------|-------------|
| `Ctrl+r` | Repaint Rainbow | Toggle repaint rainbow overlay on device |
| `Ctrl+p` | Performance Overlay | Toggle performance overlay on device |
| `Ctrl+d` | Debug Paint | Toggle debug paint overlay on device |

### Widget Inspector Navigation

When the Inspector panel is active:

| Key | Action | Description |
|-----|--------|-------------|
| `Up` / `k` | Move Up | Move selection up in widget tree |
| `Down` / `j` | Move Down | Move selection down in widget tree |
| `Enter` / `Right` | Expand | Expand selected tree node |
| `Left` / `h` | Collapse | Collapse selected tree node |
| `r` | Refresh | Refresh widget tree from VM Service |
```

### Acceptance Criteria

1. `UiMode::DevTools` arm in `render/mod.rs` renders the `DevToolsView` composite widget
2. DevTools renders into `areas.logs` (below the header, keeping project name and session tabs visible)
3. Sub-tab bar shows Inspector/Layout/Performance tabs with active tab highlighted
4. Active panel widget renders in the content area below the tab bar
5. Footer shows contextual key hints based on the active panel
6. Debug overlay status indicators shown in sub-tab bar when active
7. `DevToolsView` module correctly imports and dispatches to all three panel widgets
8. `widgets/mod.rs` exports the `devtools` module
9. `KEYBINDINGS.md` updated: `d` key reassigned, DevTools Mode section added with all keybindings
10. Startup section no longer references `d` for opening Startup Dialog
11. No rendering panics in various terminal sizes (80x24, 120x40, 40x10)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_devtools_view_renders_inspector_panel() {
        let state = DevToolsViewState::default();
        assert_eq!(state.active_panel, DevToolsPanel::Inspector);

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_devtools_view_renders_performance_panel() {
        let mut state = DevToolsViewState::default();
        state.active_panel = DevToolsPanel::Performance;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_devtools_view_renders_layout_panel() {
        let mut state = DevToolsViewState::default();
        state.active_panel = DevToolsPanel::Layout;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_tab_bar_highlights_active_panel() {
        let mut state = DevToolsViewState::default();
        state.active_panel = DevToolsPanel::Performance;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);
        // Check that "Performance" tab has active styling
    }

    #[test]
    fn test_overlay_indicators_shown_when_active() {
        let mut state = DevToolsViewState::default();
        state.overlay_repaint_rainbow = true;
        state.overlay_debug_paint = true;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);
        // Check buffer contains "Rainbow" and "DebugPaint"
    }

    #[test]
    fn test_devtools_view_small_terminal() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
        // Should not panic
    }
}

// Full-screen render tests (in render/tests.rs)
#[cfg(test)]
mod render_integration_tests {
    use super::*;

    #[test]
    fn test_full_render_devtools_mode() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::DevTools;
        // Create a TestTerminal, render, and verify no panics
    }
}
```

### Notes

- **Render into `areas.logs` vs `area`**: The recommendation is `areas.logs` to keep the header visible. This preserves project name, session tabs, and the `[d] Debug` hint in the header. DevTools has its own sub-tab bar and footer for panel-specific navigation.
- **`PerformanceState::new()` fallback**: The static `LazyLock` fallback for missing sessions is a safety net. In practice, DevTools mode is only reachable when a session exists (the `d` key handler checks for VM connection).
- **Snapshot tests**: Consider adding `insta::assert_snapshot!` tests for the DevTools panels in `render/tests.rs`, following the existing pattern.
- **Theme consistency**: Use the project's existing color/style constants from `crates/fdemon-tui/src/theme.rs` or `styles.rs` if they exist, rather than hardcoding `Color::` values. Check what's available.
- **The `DevToolsView` is non-stateful** (`Widget`, not `StatefulWidget`). All state is passed in via `&DevToolsViewState` reference. This is simpler than `SettingsPanel`'s `StatefulWidget` approach and sufficient since DevTools state is managed entirely by the TEA handler.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Added `DevToolsView` composite widget with sub-tab bar, panel dispatch, footer hints, overlay status indicators, and 11 unit tests |
| `crates/fdemon-tui/src/render/mod.rs` | Replaced `UiMode::DevTools => {}` stub with actual `DevToolsView` rendering into `areas.logs` |
| `crates/fdemon-tui/src/widgets/mod.rs` | Added `DevToolsView` to re-exports |
| `docs/KEYBINDINGS.md` | Updated `d` key entry (was "Start New Session", now "DevTools Mode"), removed `d` from startup text, added DevTools Mode section with Panel Navigation / Debug Overlays / Widget Inspector Navigation tables, updated Table of Contents |

### Notable Decisions/Tradeoffs

1. **Render into `areas.logs` not `area`**: DevTools renders into the logs area (below the app header), keeping the project name and session tabs visible. This follows the task recommendation and provides context about which session is active.

2. **`selected_name` via `into_iter().nth()`**: The `LayoutExplorer` needs the currently selected widget name from the inspector. Used `.into_iter().nth(selected_index)` on `visible_nodes()` to avoid a `.get()` index that would require copying the Vec.

3. **`LazyLock` fallback for `PerformanceState`**: Used `std::sync::LazyLock<PerformanceState>` for the zero-session fallback in the Performance panel arm, matching the task spec pattern. In practice this is never reached since `d` requires a VM-connected session.

4. **Minor `Color::` usage in tab bar**: The tab bar uses `Color::Cyan` and `Color::Black` directly (as in the task spec) rather than palette constants, since there are no palette equivalents for the active-tab cyan/black style.

5. **Header not modified**: The existing `[d] Debug` label in the header already accurately describes the new behavior. Option A (no change needed) was chosen per the task recommendation.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo test --lib` — Passed (517 unit tests including 11 new `DevToolsView` tests)
- `cargo test --workspace` — 517 lib tests pass; 25 pre-existing e2e failures (timeouts in headless/integration tests, unrelated to this task)

### Risks/Limitations

1. **No snapshot tests added**: The task noted snapshot tests as optional ("consider adding"). The render integration test (`test_full_render_devtools_mode`) was not added since the existing `render/tests.rs` pattern requires `AppState::new()` which doesn't include sessions, and DevTools mode with an empty session manager works but just shows the Inspector panel empty state.

2. **`LayoutExplorer` selected_name clone**: The `selected_name` computation clones a `String` to avoid lifetime issues. This is a tiny allocation per frame but is acceptable for TUI rendering.
