## Task: Layout Explorer Panel

**Objective**: Create a TUI widget that visualizes the box constraints, size, and flex properties of the selected widget from the Inspector panel. This gives developers an ASCII representation of Flutter's layout model without needing to switch to the browser-based DevTools.

**Depends on**: 01-devtools-state-foundation, 04-widget-inspector-panel (shares tree selection state and uses same `VmRequestHandle` pattern)

**Estimated Time**: 5-7 hours

### Scope

- `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs`: **NEW** — Layout explorer panel widget
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Add `pub mod layout_explorer;` and re-export
- `crates/fdemon-app/src/handler/devtools.rs`: Add layout data fetch/receive handlers
- `crates/fdemon-app/src/actions.rs`: Add `FetchLayoutData` async task implementation

### Details

#### Data Sources

Layout data comes from Phase 2's layout extension wrappers:

```rust
// Domain type (fdemon-core/src/widget_tree.rs):
pub struct LayoutInfo {
    pub constraints: Option<BoxConstraints>,
    pub size: Option<WidgetSize>,
    pub flex_factor: Option<f64>,
    pub flex_fit: Option<String>,  // "tight" or "loose"
    pub description: Option<String>,
}

pub struct BoxConstraints {
    pub min_width: f64,
    pub max_width: f64,
    pub min_height: f64,
    pub max_height: f64,
}

pub struct WidgetSize {
    pub width: f64,
    pub height: f64,
}

// State (from Task 01):
pub struct LayoutExplorerState {
    pub layout: Option<LayoutInfo>,
    pub loading: bool,
    pub error: Option<String>,
}
```

#### Widget Structure

```rust
/// Layout explorer panel for the DevTools mode.
///
/// Renders an ASCII visualization of the selected widget's box constraints,
/// actual size, and flex properties.
pub struct LayoutExplorer<'a> {
    layout_state: &'a LayoutExplorerState,
    /// The currently selected widget name (from inspector).
    selected_widget_name: Option<&'a str>,
    icons: IconSet,
}

impl<'a> LayoutExplorer<'a> {
    pub fn new(
        layout_state: &'a LayoutExplorerState,
        selected_widget_name: Option<&'a str>,
        icons: IconSet,
    ) -> Self {
        Self { layout_state, selected_widget_name, icons }
    }
}

impl Widget for LayoutExplorer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title(" Layout Explorer ");
        let inner = block.inner(area);
        block.render(area, buf);

        if self.layout_state.loading {
            self.render_loading(inner, buf);
        } else if let Some(ref error) = self.layout_state.error {
            self.render_error(inner, buf, error);
        } else if let Some(ref layout) = self.layout_state.layout {
            self.render_layout(inner, buf, layout);
        } else {
            self.render_no_selection(inner, buf);
        }
    }
}
```

#### Layout Visualization Design

The layout explorer renders an ASCII box model visualization:

```
┌─── Layout Explorer ───────────────────────────────────────┐
│                                                           │
│  Widget: Scaffold                                         │
│                                                           │
│  ┌─── Constraints ────────────────────────────────────┐   │
│  │  min: 0.0 × 0.0                                   │   │
│  │  max: 414.0 × 896.0                               │   │
│  └────────────────────────────────────────────────────┘   │
│                                                           │
│  ┌─── Actual Size ────────────────────────────────────┐   │
│  │                                                    │   │
│  │              414.0 × 896.0                         │   │
│  │                                                    │   │
│  │    ┌──────────────────────────┐                    │   │
│  │    │      (proportional)      │                    │   │
│  │    │        box model         │                    │   │
│  │    └──────────────────────────┘                    │   │
│  │                                                    │   │
│  └────────────────────────────────────────────────────┘   │
│                                                           │
│  Flex: factor=1.0  fit=tight                              │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

#### 1. Constraints Section

```rust
fn render_constraints(&self, area: Rect, buf: &mut Buffer, constraints: &BoxConstraints) {
    let block = Block::bordered()
        .title(" Constraints ")
        .border_style(Style::default().fg(Color::Blue));
    let inner = block.inner(area);
    block.render(area, buf);

    let min_text = format!(
        "  min: {:.1} x {:.1}",
        constraints.min_width, constraints.min_height
    );
    let max_text = format!(
        "  max: {} x {}",
        format_constraint_value(constraints.max_width),
        format_constraint_value(constraints.max_height),
    );

    buf.set_string(inner.x, inner.y, &min_text, Style::default().fg(Color::Cyan));
    buf.set_string(inner.x, inner.y + 1, &max_text, Style::default().fg(Color::Cyan));

    // Indicate if tightly constrained
    if constraints.min_width == constraints.max_width
        && constraints.min_height == constraints.max_height
    {
        if inner.height > 2 {
            buf.set_string(
                inner.x + 2,
                inner.y + 2,
                "(tight)",
                Style::default().fg(Color::Yellow),
            );
        }
    }
}

fn format_constraint_value(value: f64) -> String {
    if value == f64::INFINITY || value >= 1e10 {
        "Inf".to_string()
    } else {
        format!("{:.1}", value)
    }
}
```

#### 2. Size Box Visualization

Render a proportional box representing the widget's actual size:

```rust
fn render_size_box(&self, area: Rect, buf: &mut Buffer, size: &WidgetSize) {
    let block = Block::bordered()
        .title(" Size ")
        .border_style(Style::default().fg(Color::Green));
    let inner = block.inner(area);
    block.render(area, buf);

    // Size label
    let size_text = format!("{:.1} x {:.1}", size.width, size.height);
    let x = inner.x + (inner.width.saturating_sub(size_text.len() as u16)) / 2;
    buf.set_string(x, inner.y, &size_text, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));

    // Proportional inner box
    if inner.height > 4 && inner.width > 10 {
        let max_dim = size.width.max(size.height);
        if max_dim > 0.0 {
            let box_w = ((size.width / max_dim) * (inner.width as f64 - 4.0))
                .clamp(3.0, (inner.width - 4) as f64) as u16;
            let box_h = ((size.height / max_dim) * (inner.height as f64 - 4.0))
                .clamp(1.0, (inner.height - 4) as f64) as u16;

            let box_x = inner.x + (inner.width.saturating_sub(box_w)) / 2;
            let box_y = inner.y + 2;

            // Draw proportional box
            let box_rect = Rect::new(box_x, box_y, box_w, box_h);
            let inner_box = Block::bordered()
                .border_style(Style::default().fg(Color::DarkGray));
            inner_box.render(box_rect, buf);
        }
    }
}
```

#### 3. Flex Properties

```rust
fn render_flex_properties(&self, area: Rect, buf: &mut Buffer, layout: &LayoutInfo) {
    let mut parts = Vec::new();

    if let Some(factor) = layout.flex_factor {
        parts.push(format!("flex: {:.1}", factor));
    }

    if let Some(ref fit) = layout.flex_fit {
        parts.push(format!("fit: {}", fit));
    }

    if let Some(ref desc) = layout.description {
        parts.push(desc.clone());
    }

    if parts.is_empty() {
        parts.push("(no flex properties)".to_string());
    }

    let text = parts.join("  ");
    buf.set_string(area.x + 1, area.y, &text, Style::default().fg(Color::Magenta));
}
```

#### 4. No Selection State

```rust
fn render_no_selection(&self, area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::from("No widget selected."),
        Line::from(""),
        Line::from("Switch to the Inspector panel (press 'i'),"),
        Line::from("select a widget, then return here."),
    ];

    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    let y_offset = area.height.saturating_sub(4) / 2;
    paragraph.render(
        Rect { y: area.y + y_offset, height: 4, ..area },
        buf,
    );
}
```

#### 5. Auto-Fetch on Panel Switch

When the user switches to the Layout panel, auto-fetch layout data for the currently selected widget in the Inspector:

In `handler/devtools.rs` `handle_switch_panel()`, add:

```rust
DevToolsPanel::Layout => {
    // Get the selected widget's object_id from the inspector
    let selected_node_id = {
        let visible = state.devtools_view_state.inspector.visible_nodes();
        visible.get(state.devtools_view_state.inspector.selected_index)
            .and_then(|(node, _)| node.object_id.clone())
    };

    if let (Some(node_id), Some(handle)) = (selected_node_id, state.session_manager.active_session()) {
        let session_id = handle.session.id;
        state.devtools_view_state.layout_explorer.loading = true;
        return UpdateResult::action(UpdateAction::FetchLayoutData { session_id, node_id });
    }
}
```

#### 6. Layout Data Fetch Action (`actions.rs`)

```rust
UpdateAction::FetchLayoutData { session_id, node_id } => {
    if let Some(handle) = session_manager.get(&session_id) {
        if let Some(vm_handle) = &handle.vm_request_handle {
            let vm_handle = vm_handle.clone();
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let isolate_id = vm_handle.main_isolate_id().await;
                match isolate_id {
                    Some(isolate_id) => {
                        match fdemon_daemon::vm_service::extensions::layout::get_layout_explorer_node(
                            &vm_handle, &isolate_id, &node_id, "devtools-layout"
                        ).await {
                            Ok(layout) => {
                                let _ = msg_tx.send(Message::LayoutDataFetched {
                                    session_id,
                                    layout: Box::new(layout),
                                });
                            }
                            Err(e) => {
                                let _ = msg_tx.send(Message::LayoutDataFetchFailed {
                                    session_id,
                                    error: e.to_string(),
                                });
                            }
                        }
                    }
                    None => {
                        let _ = msg_tx.send(Message::LayoutDataFetchFailed {
                            session_id,
                            error: "No isolate ID available".to_string(),
                        });
                    }
                }
            });
        }
    }
}
```

#### 7. Layout Data Handlers (`handler/devtools.rs`)

```rust
pub fn handle_layout_data_fetched(
    state: &mut AppState,
    session_id: uuid::Uuid,
    layout: Box<LayoutInfo>,
) -> UpdateResult {
    if state.session_manager.active_session()
        .map(|h| h.session.id) == Some(session_id)
    {
        state.devtools_view_state.layout_explorer.layout = Some(*layout);
        state.devtools_view_state.layout_explorer.loading = false;
        state.devtools_view_state.layout_explorer.error = None;
    }
    UpdateResult::none()
}

pub fn handle_layout_data_fetch_failed(
    state: &mut AppState,
    session_id: uuid::Uuid,
    error: String,
) -> UpdateResult {
    if state.session_manager.active_session()
        .map(|h| h.session.id) == Some(session_id)
    {
        state.devtools_view_state.layout_explorer.loading = false;
        state.devtools_view_state.layout_explorer.error = Some(error);
    }
    UpdateResult::none()
}
```

### Acceptance Criteria

1. Layout explorer renders constraint box showing min/max width and height
2. Infinity constraint values display as "Inf" (not scientific notation)
3. Tightly constrained widgets show "(tight)" indicator
4. Actual size section shows width x height with proportional ASCII box
5. Flex properties section shows flex factor and fit when available
6. No-selection state shows helpful instruction to select a widget in Inspector
7. Loading state shows "Loading layout data..." message
8. Error state shows error message in red
9. Auto-fetches layout data when switching to Layout panel from Inspector
10. Layout fetch uses `get_layout_explorer_node` Phase 2 extension wrapper
11. Widget handles various terminal sizes without panics (min 40x10)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::widget_tree::{BoxConstraints, LayoutInfo, WidgetSize};

    fn make_test_layout() -> LayoutInfo {
        LayoutInfo {
            constraints: Some(BoxConstraints {
                min_width: 0.0,
                max_width: 414.0,
                min_height: 0.0,
                max_height: 896.0,
            }),
            size: Some(WidgetSize {
                width: 414.0,
                height: 896.0,
            }),
            flex_factor: Some(1.0),
            flex_fit: Some("tight".to_string()),
            description: None,
        }
    }

    #[test]
    fn test_layout_explorer_renders_with_data() {
        let mut state = LayoutExplorerState::default();
        state.layout = Some(make_test_layout());
        let widget = LayoutExplorer::new(&state, Some("Scaffold"), IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_layout_explorer_no_selection() {
        let state = LayoutExplorerState::default();
        let widget = LayoutExplorer::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_layout_explorer_loading() {
        let mut state = LayoutExplorerState::default();
        state.loading = true;
        let widget = LayoutExplorer::new(&state, Some("Column"), IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_format_infinity_constraint() {
        assert_eq!(format_constraint_value(f64::INFINITY), "Inf");
        assert_eq!(format_constraint_value(414.0), "414.0");
        assert_eq!(format_constraint_value(0.0), "0.0");
    }

    #[test]
    fn test_tight_constraints_detected() {
        let constraints = BoxConstraints {
            min_width: 100.0, max_width: 100.0,
            min_height: 50.0, max_height: 50.0,
        };
        assert_eq!(constraints.min_width, constraints.max_width);
        assert_eq!(constraints.min_height, constraints.max_height);
    }

    #[test]
    fn test_layout_explorer_small_terminal() {
        let mut state = LayoutExplorerState::default();
        state.layout = Some(make_test_layout());
        let widget = LayoutExplorer::new(&state, Some("Scaffold"), IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
    }
}
```

### Notes

- **Shared tree selection**: The Layout Explorer reads the selected widget from `InspectorState`. When the user selects a widget in the Inspector and switches to Layout, the layout for that widget is auto-fetched.
- **Object group management**: Use a different group name (`"devtools-layout"`) than the Inspector (`"devtools-inspector"`) to avoid interference. Dispose the group on panel switch or mode exit.
- **`get_layout_explorer_node` parameters**: Check the exact signature in `crates/fdemon-daemon/src/vm_service/extensions/layout.rs`. It may take `(client, isolate_id, node_id, group)` or similar.
- **`BoxConstraints::parse`** already handles both raw and prefixed formats including "Infinity" (documented in `widget_tree.rs`). The rendering side just needs to check for `f64::INFINITY`.
- **Proportional box rendering**: The ASCII box inside the Size section scales proportionally to the widget's aspect ratio, clamped to the available terminal space. This gives a visual sense of the widget's shape.
