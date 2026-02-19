## Task: Widget Inspector Panel

**Objective**: Create a TUI widget that displays the Flutter widget tree with expand/collapse navigation, widget details, and on-demand data fetching via the VM Service. This is the centerpiece of DevTools mode — users can browse the widget hierarchy, see properties, and identify layout issues.

**Depends on**: 01-devtools-state-foundation

**Estimated Time**: 6-8 hours

### Scope

- `crates/fdemon-tui/src/widgets/devtools/inspector.rs`: **NEW** — Widget inspector panel widget
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Add `pub mod inspector;` and re-export
- `crates/fdemon-app/src/handler/devtools.rs`: Inspector navigation handlers (if not already done in Task 02)
- `crates/fdemon-app/src/actions.rs`: Widget tree fetch async task implementation (if not done in Task 02)

### Details

#### Data Sources

The widget tree data comes from Phase 2's service extensions:

```rust
// Domain type (fdemon-core/src/widget_tree.rs):
pub struct DiagnosticsNode {
    pub description: String,           // e.g., "MaterialApp"
    pub r#type: Option<String>,        // e.g., "DiagnosticsBlock"
    pub name: Option<String>,          // diagnostic property name
    pub value_id: Option<String>,      // unique ID for expand/collapse tracking
    pub object_id: Option<String>,     // VM object reference for details fetch
    pub children: Vec<DiagnosticsNode>,
    pub properties: Vec<DiagnosticsNode>,
    pub creation_location: Option<CreationLocation>,
    pub created_by_local_project: Option<bool>,
    // ...
}

// Navigation state (from Task 01):
pub struct InspectorState {
    pub root: Option<DiagnosticsNode>,
    pub expanded: HashSet<String>,
    pub selected_index: usize,
    pub loading: bool,
    pub error: Option<String>,
}
```

#### Widget Structure

```rust
/// Widget inspector panel for the DevTools mode.
///
/// Renders the Flutter widget tree as an expandable/collapsible tree view
/// with the selected widget's details shown in a side panel.
pub struct WidgetInspector<'a> {
    inspector_state: &'a InspectorState,
    icons: IconSet,
}

impl<'a> WidgetInspector<'a> {
    pub fn new(inspector_state: &'a InspectorState, icons: IconSet) -> Self {
        Self { inspector_state, icons }
    }
}

impl Widget for WidgetInspector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.inspector_state.loading {
            self.render_loading(area, buf);
        } else if let Some(ref error) = self.inspector_state.error {
            self.render_error(area, buf, error);
        } else if self.inspector_state.root.is_some() {
            self.render_tree(area, buf);
        } else {
            self.render_empty(area, buf);
        }
    }
}
```

#### Layout Design

The inspector uses a horizontal split: tree on the left, details on the right.

```
┌─────────────────────────────────┬──────────────────────────┐
│  WIDGET TREE            [r]efresh│  WIDGET DETAILS          │
│                                 │                          │
│  ▶ MyApp                        │  Type: MaterialApp       │
│  ▼ MaterialApp                  │  Key: null               │
│    ▼ Scaffold                   │                          │
│      ▶ AppBar                   │  Properties:             │
│      ▼ Column                   │    title: "My App"       │
│        ● Text("Hello") ◄────── │    debugShowChecked: false│
│        ▶ ElevatedButton         │                          │
│      ▶ FloatingActionButton     │  Location:               │
│                                 │    lib/main.dart:42      │
│                                 │                          │
│  [↑↓] Navigate  [→] Expand     │                          │
│  [←] Collapse   [r] Refresh    │                          │
└─────────────────────────────────┴──────────────────────────┘
```

For narrow terminals (width < 80), use a single-column layout (tree only, no details panel).

#### 1. Tree Rendering

Render the widget tree as a flat list of visible nodes with indentation:

```rust
fn render_tree(&self, area: Rect, buf: &mut Buffer) {
    let visible = self.inspector_state.visible_nodes();
    let selected = self.inspector_state.selected_index;

    // Horizontal split: tree (60%) | details (40%)
    let (tree_area, details_area) = if area.width >= 80 {
        let chunks = Layout::horizontal([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ]).split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    // Block border for tree area
    let tree_block = Block::bordered()
        .title(" Widget Tree ")
        .title_alignment(Alignment::Left);
    let tree_inner = tree_block.inner(tree_area);
    tree_block.render(tree_area, buf);

    // Render visible nodes
    for (i, (node, depth)) in visible.iter().enumerate() {
        let y = tree_inner.y + i as u16;
        if y >= tree_inner.y + tree_inner.height {
            break; // Off-screen
        }

        let is_selected = i == selected;
        let indent = "  ".repeat(*depth);
        let expand_icon = self.expand_icon(node);
        let name = node.display_name();
        let is_user_code = node.is_user_code();

        let line = format!("{indent}{expand_icon} {name}");
        let style = self.node_style(is_selected, is_user_code);

        buf.set_string(tree_inner.x, y, &line, style);

        // Show source location hint for user code
        if is_selected && is_user_code {
            if let Some(loc) = &node.creation_location {
                let loc_text = format!(" ({}:{})", loc.short_path(), loc.line);
                let remaining_width = tree_inner.width.saturating_sub(line.len() as u16);
                if remaining_width > loc_text.len() as u16 {
                    buf.set_string(
                        tree_inner.x + line.len() as u16,
                        y,
                        &loc_text,
                        Style::default().fg(Color::DarkGray),
                    );
                }
            }
        }
    }

    // Scrollbar if tree is taller than viewport
    if visible.len() > tree_inner.height as usize {
        // Render a simple scrollbar indicator
    }

    // Details panel (if wide enough)
    if let Some(details_area) = details_area {
        self.render_details(details_area, buf, &visible, selected);
    }
}
```

#### 2. Expand/Collapse Icons

```rust
fn expand_icon(&self, node: &DiagnosticsNode) -> &str {
    if node.children.is_empty() {
        "●"  // Leaf node
    } else if let Some(value_id) = &node.value_id {
        if self.inspector_state.is_expanded(value_id) {
            "▼"  // Expanded
        } else {
            "▶"  // Collapsed
        }
    } else {
        "●"  // No ID — treat as leaf
    }
}
```

#### 3. Node Styling

```rust
fn node_style(&self, is_selected: bool, is_user_code: bool) -> Style {
    let base = if is_user_code {
        Style::default().fg(Color::White)  // User code: bright
    } else {
        Style::default().fg(Color::DarkGray)  // Framework code: dimmed
    };

    if is_selected {
        base.bg(Color::DarkGray).add_modifier(Modifier::BOLD)
    } else {
        base
    }
}
```

#### 4. Details Panel

Show properties and metadata for the selected node:

```rust
fn render_details(&self, area: Rect, buf: &mut Buffer, visible: &[(&DiagnosticsNode, usize)], selected: usize) {
    let block = Block::bordered()
        .title(" Details ")
        .title_alignment(Alignment::Left);
    let inner = block.inner(area);
    block.render(area, buf);

    let Some((node, _)) = visible.get(selected) else {
        return;
    };

    let mut y = inner.y;

    // Widget type/description
    let desc = &node.description;
    buf.set_string(inner.x + 1, y, desc, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    y += 2;

    // Properties
    if !node.properties.is_empty() {
        buf.set_string(inner.x + 1, y, "Properties:", Style::default().fg(Color::Yellow));
        y += 1;

        for prop in &node.properties {
            if y >= inner.y + inner.height {
                break;
            }
            let name = prop.name.as_deref().unwrap_or("?");
            let value = &prop.description;
            let line = format!("  {name}: {value}");
            buf.set_string(inner.x + 1, y, &line, Style::default().fg(Color::White));
            y += 1;
        }
    }

    y += 1;

    // Creation location
    if let Some(loc) = &node.creation_location {
        if y < inner.y + inner.height {
            buf.set_string(inner.x + 1, y, "Location:", Style::default().fg(Color::Yellow));
            y += 1;
        }
        if y < inner.y + inner.height {
            let path = format!("  {}:{}", loc.short_path(), loc.line);
            buf.set_string(inner.x + 1, y, &path, Style::default().fg(Color::Blue));
        }
    }
}
```

#### 5. Loading / Error / Empty States

```rust
fn render_loading(&self, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered().title(" Widget Inspector ");
    let inner = block.inner(area);
    block.render(area, buf);

    let text = Paragraph::new("Loading widget tree...")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    let y_offset = inner.height / 2;
    text.render(Rect { y: inner.y + y_offset, height: 1, ..inner }, buf);
}

fn render_error(&self, area: Rect, buf: &mut Buffer, error: &str) {
    let block = Block::bordered().title(" Widget Inspector ");
    let inner = block.inner(area);
    block.render(area, buf);

    let text = Paragraph::new(format!("Error: {error}"))
        .style(Style::default().fg(Color::Red))
        .wrap(Wrap { trim: true });
    text.render(inner, buf);
}

fn render_empty(&self, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered().title(" Widget Inspector ");
    let inner = block.inner(area);
    block.render(area, buf);

    let text = Paragraph::new("Press 'r' to load widget tree")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    let y_offset = inner.height / 2;
    text.render(Rect { y: inner.y + y_offset, height: 1, ..inner }, buf);
}
```

#### 6. Scroll Handling

When the tree is larger than the viewport, implement viewport scrolling:

```rust
fn visible_viewport_range(&self, viewport_height: usize, total_items: usize) -> (usize, usize) {
    let selected = self.inspector_state.selected_index;

    // Keep selected item visible with some context
    let half = viewport_height / 2;
    let start = if selected > half {
        (selected - half).min(total_items.saturating_sub(viewport_height))
    } else {
        0
    };
    let end = (start + viewport_height).min(total_items);

    (start, end)
}
```

### Acceptance Criteria

1. Widget tree renders as an indented list with expand/collapse icons (`▶`/`▼`/`●`)
2. Selected node is highlighted with background color
3. User-code widgets are bright, framework widgets are dimmed
4. Details panel shows description, properties, and creation location
5. Narrow terminals (< 80 cols) show tree only (no details panel)
6. Loading state shows "Loading widget tree..." message
7. Error state shows error message in red
8. Empty state shows "Press 'r' to load widget tree" prompt
9. Viewport scrolling keeps selected node visible
10. Source file location shown for user-code widgets
11. Tree respects `InspectorState.expanded` set for expand/collapse

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::widget_tree::DiagnosticsNode;

    fn make_test_tree() -> DiagnosticsNode {
        DiagnosticsNode {
            description: "MyApp".to_string(),
            value_id: Some("widget-1".to_string()),
            children: vec![
                DiagnosticsNode {
                    description: "MaterialApp".to_string(),
                    value_id: Some("widget-2".to_string()),
                    children: vec![
                        DiagnosticsNode {
                            description: "Scaffold".to_string(),
                            value_id: Some("widget-3".to_string()),
                            children: vec![],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn test_inspector_renders_tree_without_panic() {
        let mut state = InspectorState::new();
        state.root = Some(make_test_tree());
        state.expanded.insert("widget-1".to_string());

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_renders_loading_state() {
        let mut state = InspectorState::new();
        state.loading = true;

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_renders_error_state() {
        let mut state = InspectorState::new();
        state.error = Some("Connection failed".to_string());

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_renders_empty_state() {
        let state = InspectorState::new();
        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_narrow_terminal_no_details() {
        let mut state = InspectorState::new();
        state.root = Some(make_test_tree());
        state.expanded.insert("widget-1".to_string());

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 24)); // < 80 cols
        widget.render(Rect::new(0, 0, 60, 24), &mut buf);
        // Should render without panic, no details panel
    }

    #[test]
    fn test_expand_icon_leaf_node() {
        let state = InspectorState::new();
        let widget = WidgetInspector::new(&state, IconSet::default());
        let leaf = DiagnosticsNode {
            description: "Text".to_string(),
            children: vec![],
            ..Default::default()
        };
        assert_eq!(widget.expand_icon(&leaf), "●");
    }

    #[test]
    fn test_expand_icon_collapsed() {
        let state = InspectorState::new();
        let widget = WidgetInspector::new(&state, IconSet::default());
        let node = DiagnosticsNode {
            description: "Column".to_string(),
            value_id: Some("w1".to_string()),
            children: vec![DiagnosticsNode::default()],
            ..Default::default()
        };
        assert_eq!(widget.expand_icon(&node), "▶");
    }

    #[test]
    fn test_expand_icon_expanded() {
        let mut state = InspectorState::new();
        state.expanded.insert("w1".to_string());
        let widget = WidgetInspector::new(&state, IconSet::default());
        let node = DiagnosticsNode {
            description: "Column".to_string(),
            value_id: Some("w1".to_string()),
            children: vec![DiagnosticsNode::default()],
            ..Default::default()
        };
        assert_eq!(widget.expand_icon(&node), "▼");
    }

    #[test]
    fn test_viewport_scrolling_keeps_selected_visible() {
        let state = InspectorState { selected_index: 50, ..Default::default() };
        let widget = WidgetInspector::new(&state, IconSet::default());
        let (start, end) = widget.visible_viewport_range(20, 100);
        assert!(start <= 50);
        assert!(end > 50);
    }
}
```

### Notes

- **`DiagnosticsNode::Default`**: Check if `DiagnosticsNode` derives `Default`. If not, add it (or create test helper constructors). All fields are `Option` or `Vec` so default is straightforward.
- **`CreationLocation::short_path()`**: Check if this method exists on `CreationLocation`. If not, it needs a helper that strips the `file://` prefix and shows a relative path. The field is `file: String` which is a URI.
- **Async tree fetching**: The actual RPC call for `get_root_widget_tree` is spawned in `actions.rs` (detailed in Task 02). This task focuses on the rendering side. The `InspectorState.loading` flag gates the loading UI.
- **Object group management**: The Phase 2 `ObjectGroupManager` handles reference counting for VM objects. When fetching widget trees, use a named group like `"devtools-inspector"`. Dispose the group when leaving DevTools mode to prevent VM memory leaks.
- **Large trees**: Real Flutter apps can have thousands of widgets. The tree should NOT expand all nodes by default — only the root is auto-expanded on initial fetch. Lazy expansion keeps rendering fast.
- **Properties rendering**: `DiagnosticsNode.properties` is a `Vec<DiagnosticsNode>` where each child represents a property. Use `.name` for the property name and `.description` for the value.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/inspector.rs` | **NEW** — Full `WidgetInspector` widget with tree view, details panel, loading/error/empty states, viewport scrolling |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Added `pub mod inspector;` and `pub use inspector::WidgetInspector;` |
| `crates/fdemon-tui/src/widgets/mod.rs` | Added `WidgetInspector` to devtools re-export |
| `crates/fdemon-core/src/widget_tree.rs` | Added `Default` derive to `DiagnosticsNode` (required for tests using `..Default::default()` struct update syntax) |

### Notable Decisions/Tradeoffs

1. **`short_path()` as module-private function**: The task references `loc.short_path()` but `CreationLocation` has no such method. Implemented as a standalone `fn short_path(file: &str) -> &str` helper in the inspector module, reusing `DiagnosticsNode::source_path()` logic. This avoids adding public API to `fdemon-core` for a TUI-only concern.

2. **Selected-row highlighting via background rect**: Instead of using `buf.set_string` with a `bg` style (which only colours occupied cells), the selected row fills the full row width first then overlays the text. This ensures the highlight spans the full tree panel width, not just the text width.

3. **`DiagnosticsNode::Default`**: `DiagnosticsNode` did not derive `Default`. Added the derive since all fields are `Option`, `Vec`, `bool`, or `String` — all have trivial defaults. This is backwards-compatible and required for the task's test suite `..Default::default()` usage.

4. **Tree panel title via `Span::styled`**: Block `.title()` accepts `Into<Line>`, not `Paragraph`. Used `Span::styled(...)` to set the title with the `ACCENT_DIM` colour, consistent with the performance panel.

5. **Viewport scrolling**: Implemented `visible_viewport_range()` as a public method so tests can assert on scroll behaviour. The algorithm keeps the selected item centred (±half viewport) while clamping to valid bounds.

6. **Scroll indicator**: Added a simple single-cell `█` scroll thumb on the right edge of the tree panel. Keeps things minimal — a full scrollbar widget would consume a column.

### Testing Performed

- `cargo fmt --all` — Passed (no formatting changes needed)
- `cargo check --workspace` — Passed (0 errors)
- `cargo test --lib --workspace` — Passed (1968 unit tests: 823 fdemon-app, 318 fdemon-core, 337 fdemon-daemon, 490 fdemon-tui)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)

New tests added in `inspector.rs`: 22 tests covering all acceptance criteria including rendering states, expand icons, viewport scrolling, helper functions, and content verification.

### Risks/Limitations

1. **No scrollbar widget**: The scroll indicator is a minimal single-cell thumb. A full ratatui `Scrollbar` widget would be more polished but adds complexity. Can be enhanced in a future task.

2. **No keyboard hint footer**: The task ASCII diagram shows `[↑↓] Navigate [→] Expand [←] Collapse [r] Refresh` but this wasn't in the acceptance criteria or detailed spec for the widget itself (those hints would live in the DevTools mode render layer, not inside this widget). The widget is self-contained.

3. **`short_path()` depth**: Always shows last 2 path components (`parent/file.dart`). Paths shallower than 2 components return the full (scheme-stripped) path. Edge case for absolute paths like `/main.dart` returns the full `/main.dart`, which is acceptable.
