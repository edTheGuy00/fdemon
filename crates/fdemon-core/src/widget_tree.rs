//! # Widget Tree Domain Types
//!
//! Domain types representing Flutter's diagnostic/widget tree, as returned by the
//! VM Service inspector extensions (e.g., `ext.flutter.inspector.getRootWidget`).
//!
//! These types are shared between the daemon layer (which parses the VM Service JSON)
//! and the TUI layer (which renders the widget tree), which is why they live in
//! `fdemon-core` rather than `fdemon-daemon`.
//!
//! ## Key Types
//!
//! - [`DiagnosticsNode`] — A node in Flutter's diagnostic tree (widgets, render objects, properties)
//! - [`CreationLocation`] — Source code location where a widget was instantiated
//! - [`LayoutInfo`] — Layout and rendering properties from the Layout Explorer extension
//! - [`BoxConstraints`] — Min/max width and height constraints for a widget
//! - [`WidgetSize`] — Actual rendered size of a widget
//! - [`DiagnosticLevel`] — Severity/visibility level for a diagnostic node

use serde::{Deserialize, Serialize};

// ============================================================================
// DiagnosticsNode
// ============================================================================

/// A node in Flutter's diagnostic tree, as returned by the VM Service inspector extensions.
///
/// This is the parsed form of the JSON `DiagnosticsNode` that Flutter serializes
/// via `DiagnosticsNode.toJsonMap()` with inspector-specific additions.
///
/// The JSON fields use camelCase (Flutter convention); serde handles mapping to
/// Rust's snake_case fields via `#[serde(rename_all = "camelCase")]`.
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

    /// Diagnostic level: "info", "debug", "warning", "error", "hidden", "off"
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

impl DiagnosticsNode {
    /// Whether this node should be shown in a summary view.
    ///
    /// Filters out nodes with `hidden` or `off` diagnostic levels.
    pub fn is_visible(&self) -> bool {
        !matches!(self.level.as_deref(), Some("hidden") | Some("off"))
    }

    /// Get the display name: the description string for this node.
    pub fn display_name(&self) -> &str {
        &self.description
    }

    /// Whether this node represents user code (not Flutter framework internals).
    pub fn is_user_code(&self) -> bool {
        self.created_by_local_project
    }

    /// Get the source file path, stripping the `file://` URI prefix if present.
    ///
    /// Returns `None` if no creation location is available.
    pub fn source_path(&self) -> Option<&str> {
        self.creation_location
            .as_ref()
            .map(|loc| loc.file.strip_prefix("file://").unwrap_or(&loc.file))
    }

    /// Count total visible nodes in this subtree (including self).
    ///
    /// Returns 0 if this node is not visible (hidden/off level).
    ///
    /// Note: Flutter widget trees rarely exceed ~100 levels deep, so the
    /// recursive approach is safe in practice.
    pub fn visible_node_count(&self) -> usize {
        if !self.is_visible() {
            return 0;
        }
        1 + self
            .children
            .iter()
            .map(|c| c.visible_node_count())
            .sum::<usize>()
    }
}

// ============================================================================
// CreationLocation
// ============================================================================

/// Source location where a Flutter widget was instantiated.
///
/// Populated when the Flutter inspector's `creationLocationEnabled` mode is
/// active. The `file` field uses the `file://` URI scheme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreationLocation {
    /// File URI (e.g., "file:///path/to/main.dart")
    pub file: String,

    /// Line number (1-based)
    pub line: u32,

    /// Column number (1-based)
    pub column: u32,

    /// Widget class name at this creation site
    pub name: Option<String>,
}

// ============================================================================
// LayoutInfo
// ============================================================================

/// Layout and rendering properties for a widget, from the Layout Explorer extension.
///
/// Populated by calls to `ext.flutter.inspector.getLayoutExplorerNode`.
#[derive(Debug, Clone, Default)]
pub struct LayoutInfo {
    /// Box constraints applied to this widget
    pub constraints: Option<BoxConstraints>,

    /// Actual rendered size
    pub size: Option<WidgetSize>,

    /// Flex factor (for children of Flex widgets: Row, Column, Flex)
    pub flex_factor: Option<f64>,

    /// Flex fit: "tight" or "loose"
    pub flex_fit: Option<String>,

    /// Widget description (e.g., "Column", "SizedBox")
    pub description: Option<String>,
}

// ============================================================================
// BoxConstraints
// ============================================================================

/// Box constraints (min/max width and height) applied to a widget during layout.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxConstraints {
    /// Minimum width in logical pixels
    pub min_width: f64,
    /// Maximum width in logical pixels (may be `f64::INFINITY`)
    pub max_width: f64,
    /// Minimum height in logical pixels
    pub min_height: f64,
    /// Maximum height in logical pixels (may be `f64::INFINITY`)
    pub max_height: f64,
}

impl BoxConstraints {
    /// Parse from a VM Service constraint description string.
    ///
    /// Handles two formats:
    /// - Raw: `"0.0<=w<=414.0, 0.0<=h<=896.0"`
    /// - Prefixed: `"BoxConstraints(0.0<=w<=414.0, 0.0<=h<=Infinity)"`
    ///
    /// The value `"Infinity"` is parsed as [`f64::INFINITY`].
    ///
    /// Returns `None` if the string cannot be parsed.
    pub fn parse(s: &str) -> Option<Self> {
        // Strip optional "BoxConstraints(" prefix and trailing ")"
        let inner = if let Some(stripped) = s.strip_prefix("BoxConstraints(") {
            stripped.strip_suffix(')').unwrap_or(stripped)
        } else {
            s
        };

        // Expected format: "min_w<=w<=max_w, min_h<=h<=max_h"
        let (w_part, h_part) = inner.split_once(',')?;
        let w_part = w_part.trim();
        let h_part = h_part.trim();

        let min_width = parse_constraint_part(w_part, 'w')?;
        let (min_width, max_width) = min_width;

        let min_height = parse_constraint_part(h_part, 'h')?;
        let (min_height, max_height) = min_height;

        Some(Self {
            min_width,
            max_width,
            min_height,
            max_height,
        })
    }

    /// Whether width is tightly constrained (min == max).
    pub fn is_tight_width(&self) -> bool {
        (self.min_width - self.max_width).abs() < f64::EPSILON
    }

    /// Whether height is tightly constrained (min == max).
    pub fn is_tight_height(&self) -> bool {
        (self.min_height - self.max_height).abs() < f64::EPSILON
    }

    /// Whether both dimensions are unconstrained (0 to infinity).
    pub fn is_unconstrained(&self) -> bool {
        self.min_width == 0.0
            && self.max_width.is_infinite()
            && self.min_height == 0.0
            && self.max_height.is_infinite()
    }
}

/// Parse a single axis constraint string like `"0.0<=w<=414.0"` or `"0.0<=h<=Infinity"`.
///
/// Returns `(min, max)` as `f64` values.
fn parse_constraint_part(s: &str, axis: char) -> Option<(f64, f64)> {
    // Format: "<min><=<axis><=<max>"
    // Split on the axis character surrounded by "<=" tokens
    let separator = format!("<={axis}<=");
    let (min_str, max_str) = s.split_once(&separator)?;
    let min = parse_f64(min_str.trim())?;
    let max = parse_f64(max_str.trim())?;
    Some((min, max))
}

/// Parse a float from VM Service notation, treating "Infinity" as [`f64::INFINITY`].
fn parse_f64(s: &str) -> Option<f64> {
    match s {
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        other => other.parse::<f64>().ok(),
    }
}

// ============================================================================
// WidgetSize
// ============================================================================

/// Rendered widget size in logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetSize {
    /// Width in logical pixels
    pub width: f64,
    /// Height in logical pixels
    pub height: f64,
}

// ============================================================================
// DiagnosticLevel
// ============================================================================

/// Diagnostic severity level for a [`DiagnosticsNode`].
///
/// Maps the string `level` field from Flutter's `DiagnosticsNode.toJsonMap()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    /// Node should not be shown (completely suppressed)
    Hidden,
    /// Fine-grained detail (more verbose than debug)
    Fine,
    /// Debug-level information
    Debug,
    /// Normal informational node
    Info,
    /// Something potentially unexpected
    Warning,
    /// An error condition
    Error,
    /// Suppress all output
    Off,
}

impl DiagnosticLevel {
    /// Parse from the string format used in `DiagnosticsNode` JSON.
    ///
    /// Unknown strings default to [`DiagnosticLevel::Info`].
    pub fn parse(s: &str) -> Self {
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

impl std::str::FromStr for DiagnosticLevel {
    type Err = std::convert::Infallible;

    /// Parse from the string format used in `DiagnosticsNode` JSON.
    ///
    /// This is infallible: unknown strings default to [`DiagnosticLevel::Info`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_node(description: &str) -> DiagnosticsNode {
        DiagnosticsNode {
            description: description.to_string(),
            node_type: None,
            name: None,
            level: None,
            has_children: false,
            style: None,
            value_id: None,
            object_id: None,
            creation_location: None,
            location_id: None,
            created_by_local_project: false,
            summary_tree: false,
            children: vec![],
            properties: vec![],
        }
    }

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
    fn test_diagnostics_node_is_visible_off() {
        let mut node = make_test_node("Widget");
        node.level = Some("off".to_string());
        assert!(!node.is_visible());
    }

    #[test]
    fn test_diagnostics_node_visible_counts_children() {
        let mut parent = make_test_node("Parent");
        parent.children = vec![make_test_node("Child1"), make_test_node("Child2")];
        // 1 (parent) + 2 (children) = 3
        assert_eq!(parent.visible_node_count(), 3);
    }

    #[test]
    fn test_diagnostics_node_hidden_returns_zero_count() {
        let mut node = make_test_node("Hidden");
        node.level = Some("hidden".to_string());
        node.children = vec![make_test_node("Child")];
        assert_eq!(node.visible_node_count(), 0);
    }

    #[test]
    fn test_diagnostics_node_display_name() {
        let node = make_test_node("MyWidget");
        assert_eq!(node.display_name(), "MyWidget");
    }

    #[test]
    fn test_diagnostics_node_is_user_code() {
        let mut node = make_test_node("Widget");
        assert!(!node.is_user_code());
        node.created_by_local_project = true;
        assert!(node.is_user_code());
    }

    #[test]
    fn test_diagnostics_node_source_path_strips_prefix() {
        let mut node = make_test_node("Widget");
        node.creation_location = Some(CreationLocation {
            file: "file:///path/to/main.dart".to_string(),
            line: 1,
            column: 1,
            name: None,
        });
        assert_eq!(node.source_path(), Some("/path/to/main.dart"));
    }

    #[test]
    fn test_diagnostics_node_source_path_no_prefix() {
        let mut node = make_test_node("Widget");
        node.creation_location = Some(CreationLocation {
            file: "/path/to/main.dart".to_string(),
            line: 1,
            column: 1,
            name: None,
        });
        assert_eq!(node.source_path(), Some("/path/to/main.dart"));
    }

    #[test]
    fn test_diagnostics_node_source_path_none() {
        let node = make_test_node("Widget");
        assert_eq!(node.source_path(), None);
    }

    #[test]
    fn test_diagnostics_node_unknown_fields_ignored() {
        // Verify that extra/unknown fields in JSON do not cause deserialization failure
        // (we do NOT use deny_unknown_fields)
        let json = r#"{
            "description": "Widget",
            "unknownFutureField": "some value",
            "anotherField": 42
        }"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.description, "Widget");
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
        assert_eq!(c.max_width, 414.0);
        assert_eq!(c.min_height, 0.0);
        assert!(c.max_height.is_infinite());
    }

    #[test]
    fn test_box_constraints_parse_infinity_width() {
        let c = BoxConstraints::parse("0.0<=w<=Infinity, 0.0<=h<=600.0").unwrap();
        assert!(c.max_width.is_infinite());
        assert_eq!(c.max_height, 600.0);
    }

    #[test]
    fn test_box_constraints_parse_invalid_returns_none() {
        assert!(BoxConstraints::parse("not a constraint").is_none());
        assert!(BoxConstraints::parse("").is_none());
    }

    #[test]
    fn test_box_constraints_tight() {
        let c = BoxConstraints {
            min_width: 100.0,
            max_width: 100.0,
            min_height: 50.0,
            max_height: 50.0,
        };
        assert!(c.is_tight_width());
        assert!(c.is_tight_height());
    }

    #[test]
    fn test_box_constraints_not_tight() {
        let c = BoxConstraints {
            min_width: 0.0,
            max_width: 100.0,
            min_height: 0.0,
            max_height: 50.0,
        };
        assert!(!c.is_tight_width());
        assert!(!c.is_tight_height());
    }

    #[test]
    fn test_box_constraints_unconstrained() {
        let c = BoxConstraints {
            min_width: 0.0,
            max_width: f64::INFINITY,
            min_height: 0.0,
            max_height: f64::INFINITY,
        };
        assert!(c.is_unconstrained());
    }

    #[test]
    fn test_box_constraints_not_unconstrained() {
        let c = BoxConstraints {
            min_width: 0.0,
            max_width: 414.0,
            min_height: 0.0,
            max_height: f64::INFINITY,
        };
        assert!(!c.is_unconstrained());
    }

    #[test]
    fn test_creation_location_deserialize() {
        let json =
            r#"{"file": "file:///app/lib/main.dart", "line": 42, "column": 8, "name": "MyWidget"}"#;
        let loc: CreationLocation = serde_json::from_str(json).unwrap();
        assert_eq!(loc.file, "file:///app/lib/main.dart");
        assert_eq!(loc.line, 42);
        assert_eq!(loc.column, 8);
        assert_eq!(loc.name.as_deref(), Some("MyWidget"));
    }

    #[test]
    fn test_creation_location_deserialize_no_name() {
        let json = r#"{"file": "file:///app/lib/main.dart", "line": 1, "column": 1}"#;
        let loc: CreationLocation = serde_json::from_str(json).unwrap();
        assert!(loc.name.is_none());
    }

    #[test]
    fn test_diagnostic_level_from_str() {
        assert_eq!(DiagnosticLevel::parse("hidden"), DiagnosticLevel::Hidden);
        assert_eq!(DiagnosticLevel::parse("fine"), DiagnosticLevel::Fine);
        assert_eq!(DiagnosticLevel::parse("debug"), DiagnosticLevel::Debug);
        assert_eq!(DiagnosticLevel::parse("info"), DiagnosticLevel::Info);
        assert_eq!(DiagnosticLevel::parse("warning"), DiagnosticLevel::Warning);
        assert_eq!(DiagnosticLevel::parse("error"), DiagnosticLevel::Error);
        assert_eq!(DiagnosticLevel::parse("off"), DiagnosticLevel::Off);
        assert_eq!(DiagnosticLevel::parse("unknown"), DiagnosticLevel::Info);
    }

    #[test]
    fn test_diagnostic_level_from_str_trait() {
        use std::str::FromStr;
        assert_eq!(
            DiagnosticLevel::from_str("error").unwrap(),
            DiagnosticLevel::Error
        );
        // Unknown values default to Info (infallible)
        assert_eq!(
            DiagnosticLevel::from_str("unknown").unwrap(),
            DiagnosticLevel::Info
        );
    }

    #[test]
    fn test_layout_info_default() {
        let info = LayoutInfo::default();
        assert!(info.constraints.is_none());
        assert!(info.size.is_none());
        assert!(info.flex_factor.is_none());
        assert!(info.flex_fit.is_none());
        assert!(info.description.is_none());
    }
}
