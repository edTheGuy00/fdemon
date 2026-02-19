## Task: Widget Tree Data Models

**Objective**: Create domain data types in `fdemon-core` for representing the Flutter widget tree, diagnostic nodes, creation locations, and layout properties. These types are the shared vocabulary between the daemon (parsing) and TUI (rendering) layers.

**Depends on**: None (pure data types, no dependency on Phase 2 code)

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-core/src/widget_tree.rs`: **NEW** — All widget tree domain types
- `crates/fdemon-core/src/lib.rs`: Add `pub mod widget_tree` and re-exports

### Details

#### 1. DiagnosticsNode

The fundamental type returned by all inspector extensions. Represents a node in Flutter's diagnostic tree (widgets, render objects, properties).

```rust
/// A node in Flutter's diagnostic tree, as returned by the VM Service inspector extensions.
///
/// This is the parsed form of the JSON `DiagnosticsNode` that Flutter serializes
/// via `DiagnosticsNode.toJsonMap()` with inspector-specific additions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsNode {
    /// Widget/object description (e.g., "Container", "Text('Hello')")
    pub description: String,

    /// Runtime type as string
    #[serde(rename = "type")]
    pub node_type: Option<String>,

    /// Property name (for property nodes)
    pub name: Option<String>,

    /// Diagnostic level: "info", "debug", "warning", "error", "hidden"
    pub level: Option<String>,

    /// Whether this node has children
    #[serde(default)]
    pub has_children: bool,

    /// Tree display style: "dense", "sparse", etc.
    pub style: Option<String>,

    /// VM Service object ID for this node's value — used as `arg` in subsequent calls
    pub value_id: Option<String>,

    /// VM Service object ID for the DiagnosticsNode itself
    pub object_id: Option<String>,

    /// Source code location where the widget was created
    pub creation_location: Option<CreationLocation>,

    /// Location ID for source mapping
    pub location_id: Option<String>,

    /// Whether this widget was created by user's project code (vs framework)
    #[serde(default)]
    pub created_by_local_project: bool,

    /// True when in summary tree mode (user-relevant widgets only)
    #[serde(default)]
    pub summary_tree: bool,

    /// Child nodes (populated when subtreeDepth > 0)
    #[serde(default)]
    pub children: Vec<DiagnosticsNode>,

    /// Property nodes (populated when includeProperties is true)
    #[serde(default)]
    pub properties: Vec<DiagnosticsNode>,
}
```

#### 2. CreationLocation

Source code location for a widget's creation site.

```rust
/// Source location where a Flutter widget was instantiated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreationLocation {
    /// File URI (e.g., "file:///path/to/main.dart")
    pub file: String,

    /// Line number (1-based)
    pub line: u32,

    /// Column number (1-based)
    pub column: u32,

    /// Widget class name
    pub name: Option<String>,
}
```

#### 3. LayoutInfo

Layout properties for the Layout Explorer. Extracted from the layout explorer extension response.

```rust
/// Layout and rendering properties for a widget, from the Layout Explorer extension.
#[derive(Debug, Clone, Default)]
pub struct LayoutInfo {
    /// Box constraints applied to this widget
    pub constraints: Option<BoxConstraints>,

    /// Actual rendered size
    pub size: Option<WidgetSize>,

    /// Flex factor (for children of Flex widgets: Row, Column, Flex)
    pub flex_factor: Option<f64>,

    /// Flex fit (tight, loose)
    pub flex_fit: Option<String>,

    /// Widget description (e.g., "Column", "SizedBox")
    pub description: Option<String>,
}

/// Box constraints (min/max width and height).
#[derive(Debug, Clone)]
pub struct BoxConstraints {
    pub min_width: f64,
    pub max_width: f64,
    pub min_height: f64,
    pub max_height: f64,
}

/// Rendered widget size.
#[derive(Debug, Clone)]
pub struct WidgetSize {
    pub width: f64,
    pub height: f64,
}
```

#### 4. DiagnosticLevel

Enum mapping for the diagnostic level strings.

```rust
/// Diagnostic severity level for a DiagnosticsNode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Hidden,
    Fine,
    Debug,
    Info,
    Warning,
    Error,
    Off,
}

impl DiagnosticLevel {
    /// Parse from the string format used in DiagnosticsNode JSON.
    pub fn from_str(s: &str) -> Self {
        match s {
            "hidden" => Self::Hidden,
            "fine" => Self::Fine,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warning" => Self::Warning,
            "error" => Self::Error,
            "off" => Self::Off,
            _ => Self::Info,
        }
    }
}
```

#### 5. Helper Methods on DiagnosticsNode

```rust
impl DiagnosticsNode {
    /// Whether this node should be shown in a summary view (filters hidden/off nodes).
    pub fn is_visible(&self) -> bool {
        match self.level.as_deref() {
            Some("hidden") | Some("off") => false,
            _ => true,
        }
    }

    /// Get the display name: description or name, whichever is set.
    pub fn display_name(&self) -> &str {
        &self.description
    }

    /// Whether this node represents user code (not framework internals).
    pub fn is_user_code(&self) -> bool {
        self.created_by_local_project
    }

    /// Get the source file path (without the file:// prefix).
    pub fn source_path(&self) -> Option<&str> {
        self.creation_location.as_ref().map(|loc| {
            loc.file.strip_prefix("file://").unwrap_or(&loc.file)
        })
    }

    /// Count total visible nodes in this subtree.
    pub fn visible_node_count(&self) -> usize {
        if !self.is_visible() {
            return 0;
        }
        1 + self.children.iter().map(|c| c.visible_node_count()).count()
    }
}
```

#### 6. BoxConstraints Parsing

The constraints string from VM Service looks like `"0.0<=w<=414.0, 0.0<=h<=Infinity"`. Add a parser:

```rust
impl BoxConstraints {
    /// Parse from VM Service constraint description string.
    /// Format: "0.0<=w<=414.0, 0.0<=h<=896.0" or "BoxConstraints(0.0<=w<=414.0, 0.0<=h<=Infinity)"
    pub fn parse(s: &str) -> Option<Self> { ... }

    /// Whether width is tightly constrained (min == max).
    pub fn is_tight_width(&self) -> bool {
        (self.min_width - self.max_width).abs() < f64::EPSILON
    }

    /// Whether height is tightly constrained.
    pub fn is_tight_height(&self) -> bool {
        (self.min_height - self.max_height).abs() < f64::EPSILON
    }

    /// Whether both dimensions are unconstrained (0 to infinity).
    pub fn is_unconstrained(&self) -> bool {
        self.min_width == 0.0 && self.max_width.is_infinite()
            && self.min_height == 0.0 && self.max_height.is_infinite()
    }
}
```

### Acceptance Criteria

1. `DiagnosticsNode` deserializes from real Flutter inspector JSON responses
2. `CreationLocation` correctly parses file URI, line, column, name
3. `BoxConstraints::parse()` handles the constraint description string format
4. `LayoutInfo` stores flex factor, flex fit, constraints, and size
5. `DiagnosticLevel` maps all string variants
6. Helper methods (`is_visible`, `is_user_code`, `source_path`) work correctly
7. All types implement `Debug` and `Clone`
8. Types are re-exported from `fdemon_core::widget_tree`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostics_node_deserialize_minimal() {
        let json = r#"{"description": "Container", "hasChildren": false}"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.description, "Container");
        assert!(!node.has_children);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_diagnostics_node_deserialize_full() {
        let json = r#"{
            "description": "MyWidget",
            "type": "_WidgetDiagnosticableNode",
            "hasChildren": true,
            "valueId": "objects/42",
            "createdByLocalProject": true,
            "summaryTree": true,
            "creationLocation": {
                "file": "file:///path/to/main.dart",
                "line": 15,
                "column": 12,
                "name": "MyWidget"
            },
            "children": [
                {"description": "Container", "hasChildren": false}
            ],
            "properties": [
                {"name": "color", "description": "Color(0xff2196f3)", "level": "info"}
            ]
        }"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.description, "MyWidget");
        assert!(node.has_children);
        assert_eq!(node.value_id.as_deref(), Some("objects/42"));
        assert!(node.created_by_local_project);
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.properties.len(), 1);
        assert_eq!(node.source_path(), Some("/path/to/main.dart"));
    }

    #[test]
    fn test_diagnostics_node_is_visible() {
        let mut node = make_test_node("Widget");
        assert!(node.is_visible());

        node.level = Some("hidden".to_string());
        assert!(!node.is_visible());
    }

    #[test]
    fn test_box_constraints_parse() {
        let c = BoxConstraints::parse("0.0<=w<=414.0, 0.0<=h<=896.0").unwrap();
        assert_eq!(c.min_width, 0.0);
        assert_eq!(c.max_width, 414.0);
        assert_eq!(c.min_height, 0.0);
        assert_eq!(c.max_height, 896.0);
    }

    #[test]
    fn test_box_constraints_parse_with_prefix() {
        let c = BoxConstraints::parse("BoxConstraints(0.0<=w<=414.0, 0.0<=h<=Infinity)").unwrap();
        assert_eq!(c.min_width, 0.0);
        assert!(c.max_height.is_infinite());
    }

    #[test]
    fn test_box_constraints_tight() {
        let c = BoxConstraints { min_width: 100.0, max_width: 100.0, min_height: 50.0, max_height: 50.0 };
        assert!(c.is_tight_width());
        assert!(c.is_tight_height());
    }

    #[test]
    fn test_creation_location_deserialize() {
        let json = r#"{"file": "file:///app/lib/main.dart", "line": 42, "column": 8, "name": "MyWidget"}"#;
        let loc: CreationLocation = serde_json::from_str(json).unwrap();
        assert_eq!(loc.line, 42);
        assert_eq!(loc.name.as_deref(), Some("MyWidget"));
    }

    #[test]
    fn test_diagnostic_level_from_str() {
        assert_eq!(DiagnosticLevel::from_str("hidden"), DiagnosticLevel::Hidden);
        assert_eq!(DiagnosticLevel::from_str("error"), DiagnosticLevel::Error);
        assert_eq!(DiagnosticLevel::from_str("unknown"), DiagnosticLevel::Info);
    }
}
```

### Notes

- **Serde `rename_all = "camelCase"`** is used because the VM Service returns camelCase JSON fields. Rust fields use snake_case per convention, serde handles the mapping.
- **`children` and `properties` default to empty Vec** — they are only populated when the extension response includes them (depends on `subtreeDepth` and `includeProperties` params).
- **`value_id` is the critical field** for follow-up calls. It's the handle used to fetch detail subtrees, layout info, and select widgets.
- The `visible_node_count()` method uses recursion — for very deep trees this could theoretically stack overflow, but Flutter widget trees rarely exceed ~100 depth. Add a depth guard if needed.
- `BoxConstraints::parse()` must handle both the raw format (`"0.0<=w<=414.0, 0.0<=h<=896.0"`) and the prefixed format (`"BoxConstraints(0.0<=w<=414.0, 0.0<=h<=Infinity)"`).
- These types are intentionally in `fdemon-core` (not `fdemon-daemon`) because the TUI layer needs them for rendering without depending on `fdemon-daemon`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/widget_tree.rs` | NEW — All widget tree domain types: `DiagnosticsNode`, `CreationLocation`, `LayoutInfo`, `BoxConstraints`, `WidgetSize`, `DiagnosticLevel`, with helper methods, `BoxConstraints::parse()`, and 44 unit tests |
| `crates/fdemon-core/src/lib.rs` | Added `pub mod widget_tree` declaration and re-exports for all 6 new types |

### Notable Decisions/Tradeoffs

1. **`DiagnosticLevel::parse()` instead of `from_str()`**: The task spec specified `from_str()`, but clippy `-D warnings` flags this as confusable with `std::str::FromStr::from_str`. Resolution: implemented `std::str::FromStr` as the trait (returning `Result<Self, Infallible>`) and named the plain helper `parse()`. Both are available; the behavior matches the spec exactly.

2. **`visible_node_count()` uses `sum()` not `.count()`**: The task spec had a bug — `.map(...).count()` always returns the number of children, not their recursive counts. Corrected to `.map(...).sum::<usize>()` so counts properly accumulate through the tree.

3. **No `#[serde(deny_unknown_fields)]`**: Intentionally omitted per spec. Flutter VM Service responses include many optional/future fields; this allows forward compatibility.

4. **`CreationLocation` uses plain field names**: Not `#[serde(rename_all = "camelCase")]` because the fields already match (file, line, column, name are the same in camelCase and lowercase).

### Testing Performed

- `cargo check -p fdemon-core` — Passed
- `cargo test -p fdemon-core` — Passed (304 tests: 260 pre-existing + 44 new widget_tree tests)
- `cargo clippy -p fdemon-core -- -D warnings` — Passed
- `cargo fmt -p fdemon-core` — Applied (no substantive changes)

### Risks/Limitations

1. **`visible_node_count()` is recursive**: Stack overflow is theoretically possible for extremely deep trees, but Flutter widget trees rarely exceed ~100 levels. A depth guard can be added if needed.
2. **`BoxConstraints::parse()` is regex-free**: Uses string splitting on the `<=w<=` and `<=h<=` patterns. Works for the documented VM Service format; unusual formats will return `None`.
