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
//! - [`InspectorRow`] — A flattened row in the inspector tree view, with rendering metadata
//! - [`RowGroup`] — Group-folding marker for hideable-chain rows

use std::collections::HashSet;

use serde::de::{self, Deserializer};
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    /// Location ID for source mapping (Flutter sends this as an integer)
    #[serde(default, deserialize_with = "deserialize_string_or_int")]
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

    /// Runtime type of the associated render-object property, if any.
    ///
    /// Flutter serializes this as `"propertyType"` in the diagnostic JSON.
    /// Used to distinguish render-object property nodes (e.g. `propertyType ==
    /// "RenderObject"`) from regular widget property nodes.
    #[serde(default, rename = "propertyType")]
    pub property_type: Option<String>,
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

    /// Count visible nodes in this subtree for display purposes.
    ///
    /// Returns the number of nodes that would be shown in a tree view.
    /// Hidden nodes (level = `"hidden"` or `"off"`) and their entire subtrees
    /// are excluded — visible children of a hidden parent are NOT counted.
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

    /// Returns the widget's runtime type, stripping any generic type arguments.
    ///
    /// For example, `"BlocProvider<AppBloc>"` returns `"BlocProvider"` and
    /// `"Container"` returns `"Container"`.
    ///
    /// The implementation uses `self.description` as the source, trimming
    /// everything from the first `<` onward, then stripping whitespace. This
    /// mirrors DevTools' `widgetRuntimeType` getter
    /// (`diagnostics_node.dart:588`).
    ///
    /// Returns `None` when the description is empty after stripping.
    pub fn widget_runtime_type(&self) -> Option<&str> {
        let raw = &self.description;
        let trimmed = if let Some(pos) = raw.find('<') {
            raw[..pos].trim()
        } else {
            raw.trim()
        };
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// Whether this node should always be shown in the inspector tree, even
    /// when implementation-widget hiding is active.
    ///
    /// Mirrors DevTools' `_alwaysVisible` predicate
    /// (`diagnostics_node.dart:664–672`):
    ///
    /// - `parent_child_count == 0` — root node (no parent).
    /// - `self.created_by_local_project` — user-authored widget.
    /// - `self.children.len() > 1` — has more than one child.
    /// - `parent_child_count > 1` — has siblings (parent has >1 child).
    ///
    /// `parent_child_count` should be the number of children on the node's
    /// parent (pass `0` for the root). The caller (state layer) is responsible
    /// for supplying this, because `DiagnosticsNode` does not hold a back-link
    /// to its parent.
    pub fn is_always_visible(&self, parent_child_count: usize) -> bool {
        let is_root = parent_child_count == 0;
        let has_more_than_one_child = self.children.len() > 1;
        let has_siblings = parent_child_count > 1;
        is_root || self.created_by_local_project || has_more_than_one_child || has_siblings
    }

    /// Whether this node represents a flex container (`Row`, `Column`, or
    /// `Flex`).
    ///
    /// Mirrors DevTools' `isFlex` getter (`diagnostics_node.dart:102`).
    pub fn is_flex(&self) -> bool {
        matches!(
            self.widget_runtime_type(),
            Some("Row") | Some("Column") | Some("Flex")
        )
    }

    /// Whether this node participates in a flex layout — either because it
    /// *is* a flex container or because its parent is one.
    ///
    /// `parent` should be `None` for the root node, or `Some(parent_node)`
    /// otherwise. The caller (state layer) supplies the parent reference
    /// because `DiagnosticsNode` does not hold a parent back-link.
    ///
    /// Mirrors DevTools' `isFlexLayout` getter (`diagnostics_node.dart:487`).
    pub fn is_flex_layout(&self, parent: Option<&DiagnosticsNode>) -> bool {
        self.is_flex() || parent.is_some_and(|p| p.is_flex())
    }

    /// Whether this node is a render-object property node.
    ///
    /// True when `property_type == "RenderObject"`. DevTools uses this to
    /// distinguish render-object property nodes from regular widget properties
    /// when building the layout explorer.
    pub fn is_render_object_property(&self) -> bool {
        self.property_type.as_deref() == Some("RenderObject")
    }
}

// ============================================================================
// InspectorRow / RowGroup
// ============================================================================

/// Group-folding marker for a row in the inspector tree, used when
/// implementation-widget hiding is active.
///
/// When "Hide implementation widgets" is enabled, consecutive
/// single-child non-local-project nodes are folded into a *hideable chain*.
/// The first node in the chain becomes the `Leader`; subsequent nodes become
/// `Member` rows (or are suppressed entirely when collapsed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowGroup {
    /// Standalone row — not part of a hideable chain.
    None,
    /// Chain leader when the chain is **collapsed**.
    ///
    /// The leader row is shown with a `+ N more widgets` badge; the
    /// `hidden_count` subordinate rows are suppressed from the list entirely.
    LeaderCollapsed {
        /// Number of subordinate rows hidden (does **not** include the leader
        /// itself, matching DevTools' "N more widgets" badge text).
        hidden_count: usize,
    },
    /// Chain leader when the chain is **expanded**.
    ///
    /// The leader renders normally; all subordinate rows follow immediately
    /// below as [`RowGroup::Member`] rows.
    LeaderExpanded,
    /// A subordinate row of an expanded chain leader.
    Member,
}

/// A single row in the inspector tree view, carrying both the node reference
/// and all metadata required by the renderer.
#[derive(Debug, Clone)]
pub struct InspectorRow<'a> {
    /// The diagnostics node this row represents.
    pub node: &'a DiagnosticsNode,
    /// Indentation depth of this row (root = 0).
    pub depth: usize,
    /// Ancestor depths at which a vertical connector line should be drawn
    /// through this row.
    ///
    /// A depth `d` is included when the ancestor at depth `d` still has
    /// sibling nodes that will be rendered *below* this row (i.e. the
    /// ancestor is **not** the last child of its own parent).
    pub ticks: Vec<usize>,
    /// `true` when this row is **not** the last child of its parent.
    ///
    /// `true` → use `├─` branch; `false` → use `└─` branch (last child).
    pub line_to_parent: bool,
    /// Group-folding marker for this row.
    pub group: RowGroup,
}

/// Inputs for [`build_inspector_rows`].
pub struct InspectorRowBuilderInputs<'a> {
    /// Root node of the subtree to flatten.
    pub root: &'a DiagnosticsNode,
    /// Set of `value_id`s whose subtrees are currently **expanded** in the
    /// regular tree expand/collapse sense.
    pub expanded: &'a HashSet<String>,
    /// Set of `value_id`s of *group leaders* whose chains are currently
    /// **expanded** (showing `Member` rows).  When a leader's `value_id` is
    /// absent from this set, the leader renders as
    /// [`RowGroup::LeaderCollapsed`].
    pub expanded_groups: &'a HashSet<String>,
    /// When `true`, consecutive single-child non-local-project nodes are
    /// folded into hideable chains (mirrors DevTools' "Hide implementation
    /// widgets" toggle).  When `false`, every visible node renders as a
    /// standalone [`RowGroup::None`] row.
    pub hide_implementation: bool,
}

/// Flatten a `DiagnosticsNode` subtree into a list of [`InspectorRow`]s
/// suitable for line-by-line rendering.
///
/// The algorithm is a pre-order depth-first walk that:
///
/// 1. Respects `inputs.expanded` for regular tree expand/collapse.
/// 2. When `inputs.hide_implementation` is `true`, folds consecutive
///    single-child non-local-project nodes into *hideable chains* — the
///    chain leader emits a single [`RowGroup::LeaderCollapsed`] or
///    [`RowGroup::LeaderExpanded`] row; subordinates either follow as
///    [`RowGroup::Member`] rows or are suppressed.
/// 3. Computes `ticks` (vertical connector depths) and `line_to_parent`
///    (branch style) for each row.
///
/// The output order is deterministic for the same inputs.
pub fn build_inspector_rows<'a>(inputs: InspectorRowBuilderInputs<'a>) -> Vec<InspectorRow<'a>> {
    let mut rows: Vec<InspectorRow<'a>> = Vec::new();

    // Walk the tree; `open_ticks` tracks ancestor depths that still have
    // remaining siblings to be emitted below the current position.
    walk_node(
        inputs.root,
        0,
        false,
        &mut Vec::new(),
        inputs.expanded,
        inputs.expanded_groups,
        inputs.hide_implementation,
        0,     // parent_child_count: root has no parent → 0
        false, // is_member: root is never a chain member
        &mut rows,
    );

    rows
}

/// Recursive worker for [`build_inspector_rows`].
///
/// # Parameters
/// - `node` — current node being visited.
/// - `depth` — current indentation depth (root = 0).
/// - `line_to_parent` — branch style (`true` = not-last child = `├─`).
/// - `open_ticks` — mutable stack of ancestor depths that still have pending
///   siblings; updated as we enter/leave each child list.
/// - `expanded` — expanded node id set.
/// - `expanded_groups` — expanded group leader id set.
/// - `hide_implementation` — whether chain-folding is active.
/// - `parent_child_count` — number of children on the node's parent (0 for root).
/// - `is_member` — whether this node is being emitted as a `Member` row of an
///   already-open chain (avoids re-checking group membership for subordinates
///   the caller has already decided to emit).
/// - `rows` — accumulator.
#[allow(clippy::too_many_arguments)]
fn walk_node<'a>(
    node: &'a DiagnosticsNode,
    depth: usize,
    line_to_parent: bool,
    open_ticks: &mut Vec<usize>,
    expanded: &HashSet<String>,
    expanded_groups: &HashSet<String>,
    hide_implementation: bool,
    parent_child_count: usize,
    is_member: bool,
    rows: &mut Vec<InspectorRow<'a>>,
) {
    // Determine the RowGroup for this node.
    let group = if is_member {
        RowGroup::Member
    } else if hide_implementation && !node.is_always_visible(parent_child_count) {
        // This node is an implementation node.  Check whether it belongs to a
        // chain (parent is always-visible-with-this-as-only-child OR parent is
        // also an implementation node) — but in the recursive walk we model this
        // at the *parent* side when emitting children, so here we treat every
        // implementation node that is not already tagged as a member as a
        // potential chain leader.
        let subordinate_count =
            count_visible_chain_subordinates(node, expanded, hide_implementation);
        if subordinate_count > 0 {
            // This is a leader node.
            let leader_id = node.value_id.as_deref().unwrap_or("");
            if !leader_id.is_empty() && expanded_groups.contains(leader_id) {
                RowGroup::LeaderExpanded
            } else {
                RowGroup::LeaderCollapsed {
                    hidden_count: subordinate_count,
                }
            }
        } else {
            // Lone implementation node (no subordinates) — render standalone.
            RowGroup::None
        }
    } else {
        RowGroup::None
    };

    // Snapshot ticks BEFORE any push for this node's own non-last status.
    // Ticks record ancestors that are non-last; this node's own non-last
    // status only matters for *its descendants*, not for itself.
    let ticks = open_ticks.clone();
    rows.push(InspectorRow {
        node,
        depth,
        ticks,
        line_to_parent,
        group: group.clone(),
    });

    // Decide whether to recurse into children.
    let should_expand = node
        .value_id
        .as_deref()
        .is_none_or(|id| expanded.contains(id));

    if !should_expand || node.children.is_empty() {
        return;
    }

    // If this node is not the last child of its parent, push its own depth so
    // that all descendants see a `│` at this column (indicating siblings of
    // this node come after the subtree).
    if line_to_parent {
        open_ticks.push(depth);
    }

    match &group {
        RowGroup::LeaderCollapsed { .. } => {
            // Subordinates are suppressed — do not recurse.
        }
        RowGroup::LeaderExpanded => {
            // Emit all subordinates as Member rows.
            emit_chain_members(
                node,
                depth,
                open_ticks,
                expanded,
                expanded_groups,
                hide_implementation,
                rows,
            );
        }
        RowGroup::None | RowGroup::Member => {
            // Normal recursive descent.
            let children = &node.children;
            let child_count = children.len();

            for (i, child) in children.iter().enumerate() {
                let is_last = i == child_count - 1;
                let child_line_to_parent = !is_last;

                walk_node(
                    child,
                    depth + 1,
                    child_line_to_parent,
                    open_ticks,
                    expanded,
                    expanded_groups,
                    hide_implementation,
                    child_count,
                    false,
                    rows,
                );
            }
        }
    }

    if line_to_parent {
        open_ticks.pop();
    }
}

/// Emit the subordinate nodes of a chain whose leader is `leader_node` as
/// [`RowGroup::Member`] rows.
///
/// The chain continues as long as each node is an implementation node (not
/// always-visible, single child, no siblings).  The walk stops when it
/// encounters an always-visible node or a node with >1 child.
fn emit_chain_members<'a>(
    leader_node: &'a DiagnosticsNode,
    leader_depth: usize,
    open_ticks: &mut Vec<usize>,
    expanded: &HashSet<String>,
    expanded_groups: &HashSet<String>,
    hide_implementation: bool,
    rows: &mut Vec<InspectorRow<'a>>,
) {
    let mut current = leader_node;
    let mut depth = leader_depth;

    loop {
        // The chain consists of single-child nodes — if there are no children
        // or >1 children the chain cannot continue.
        if current.children.len() != 1 {
            break;
        }

        let child = &current.children[0];
        let parent_child_count = 1; // single child — no siblings

        if child.is_always_visible(parent_child_count) {
            // Chain interrupted by an always-visible node — render it normally
            // via a regular walk_node call (standalone, not a member).
            let ticks = open_ticks.clone();
            // Single child → last child → line_to_parent = false.
            rows.push(InspectorRow {
                node: child,
                depth: depth + 1,
                ticks,
                line_to_parent: false,
                group: RowGroup::None,
            });
            // Continue descending from this always-visible node normally.
            let should_expand = child
                .value_id
                .as_deref()
                .is_none_or(|id| expanded.contains(id));
            if should_expand {
                walk_children(
                    child,
                    depth + 1,
                    open_ticks,
                    expanded,
                    expanded_groups,
                    hide_implementation,
                    rows,
                );
            }
            break;
        }

        // Still an implementation node — emit as Member.
        let ticks = open_ticks.clone();
        rows.push(InspectorRow {
            node: child,
            depth: depth + 1,
            ticks,
            line_to_parent: false,
            group: RowGroup::Member,
        });

        current = child;
        depth += 1;
    }
}

/// Walk all children of `node` and emit their rows (used after an
/// always-visible node that interrupted a chain, to continue descending).
#[allow(clippy::too_many_arguments)]
fn walk_children<'a>(
    node: &'a DiagnosticsNode,
    depth: usize,
    open_ticks: &mut Vec<usize>,
    expanded: &HashSet<String>,
    expanded_groups: &HashSet<String>,
    hide_implementation: bool,
    rows: &mut Vec<InspectorRow<'a>>,
) {
    let children = &node.children;
    let child_count = children.len();

    for (i, child) in children.iter().enumerate() {
        let is_last = i == child_count - 1;
        let child_line_to_parent = !is_last;

        walk_node(
            child,
            depth + 1,
            child_line_to_parent,
            open_ticks,
            expanded,
            expanded_groups,
            hide_implementation,
            child_count,
            false,
            rows,
        );
    }
}

/// Count the number of subordinate nodes that would be hidden when `node` is
/// treated as a chain leader.
///
/// A "chain" is a path of single-child nodes that are all implementation
/// nodes (i.e. `!is_always_visible(...)`).  The count excludes the leader
/// itself, matching DevTools' "N more widgets" badge semantics.
///
/// Returns `0` when `node` has no children or when the first child would
/// break the chain (is always-visible or has siblings).
pub fn count_visible_chain_subordinates(
    node: &DiagnosticsNode,
    expanded: &HashSet<String>,
    hide_implementation: bool,
) -> usize {
    if !hide_implementation {
        return 0;
    }

    let mut count = 0;
    let mut current = node;

    loop {
        if current.children.len() != 1 {
            break;
        }

        let child = &current.children[0];
        let parent_child_count = 1;

        if child.is_always_visible(parent_child_count) {
            // Chain ends here — the always-visible node is not a subordinate.
            break;
        }

        count += 1;

        // If this child is not expanded we stop descending (its subtree is
        // not visible anyway).
        let should_expand = child
            .value_id
            .as_deref()
            .is_none_or(|id| expanded.contains(id));
        if !should_expand || child.children.is_empty() {
            break;
        }

        current = child;
    }

    count
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
// EdgeInsets
// ============================================================================

/// Edge insets representing padding or margin on four sides.
///
/// Parsed from Flutter's diagnostic property format:
/// `"EdgeInsets(8.0, 0.0, 8.0, 0.0)"` or named constructors.
///
/// # Equality
///
/// `PartialEq` is derived for convenience (primarily test assertions).
/// For production comparisons involving computed layout values, be aware
/// that floating-point arithmetic can produce imprecise results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeInsets {
    /// Top inset in logical pixels
    pub top: f64,
    /// Right inset in logical pixels
    pub right: f64,
    /// Bottom inset in logical pixels
    pub bottom: f64,
    /// Left inset in logical pixels
    pub left: f64,
}

impl EdgeInsets {
    /// Create an `EdgeInsets` with all sides set to zero.
    pub fn zero() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }

    /// Returns `true` when all four sides are exactly `0.0`.
    pub fn is_zero(&self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }

    /// Parse from Flutter's `EdgeInsets` string format.
    ///
    /// Supported formats:
    /// - `"EdgeInsets(8.0, 0.0, 8.0, 0.0)"` — (top, right, bottom, left)
    /// - `"EdgeInsets.all(8.0)"` — uniform on all sides
    /// - `"EdgeInsets.zero"` — all zeros
    ///
    /// Returns `None` if the string cannot be parsed or is in an unrecognised
    /// format. The parser is intentionally lenient — unknown variants become
    /// `None` rather than an error.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();

        // Handle "EdgeInsets.zero"
        if s == "EdgeInsets.zero" {
            return Some(Self::zero());
        }

        // Handle "EdgeInsets.all(N)"
        if let Some(inner) = s
            .strip_prefix("EdgeInsets.all(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            let v = inner.trim().parse::<f64>().ok()?;
            return Some(Self {
                top: v,
                right: v,
                bottom: v,
                left: v,
            });
        }

        // Handle "EdgeInsets(T, R, B, L)"
        if let Some(inner) = s
            .strip_prefix("EdgeInsets(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() == 4 {
                let top = parts[0].trim().parse::<f64>().ok()?;
                let right = parts[1].trim().parse::<f64>().ok()?;
                let bottom = parts[2].trim().parse::<f64>().ok()?;
                let left = parts[3].trim().parse::<f64>().ok()?;
                return Some(Self {
                    top,
                    right,
                    bottom,
                    left,
                });
            }
        }

        None
    }
}

// ============================================================================
// LayoutInfo
// ============================================================================

/// Layout and rendering properties for a widget, from the Layout Explorer extension.
///
/// Populated by calls to `ext.flutter.inspector.getLayoutExplorerNode`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    /// Padding applied inside this widget's box
    pub padding: Option<EdgeInsets>,

    /// Margin applied outside this widget's box
    pub margin: Option<EdgeInsets>,
}

// ============================================================================
// BoxConstraints
// ============================================================================

/// Box constraints describing minimum and maximum width/height.
///
/// # Equality
///
/// `PartialEq` is derived for convenience (primarily test assertions).
/// For production comparisons involving computed values, use the
/// epsilon-based methods ([`BoxConstraints::is_tight_width`], [`BoxConstraints::is_tight_height`]) instead
/// of direct `==`, as floating-point arithmetic can produce imprecise results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

        let (min_width, max_width) = parse_constraint_part(w_part, 'w')?;
        let (min_height, max_height) = parse_constraint_part(h_part, 'h')?;

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
///
/// # Equality
///
/// `PartialEq` is derived for convenience (primarily test assertions).
/// For production comparisons involving computed layout values, prefer
/// epsilon-based comparisons over direct `==`, as floating-point arithmetic
/// can produce imprecise results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
// Serde helpers
// ============================================================================

/// Deserialize a value that may be a string or an integer into `Option<String>`.
///
/// Flutter's VM Service inspector extensions serialize some fields (e.g., `locationId`)
/// as integers, while the Dart `toJsonMap()` method sometimes uses strings for the
/// same fields. This helper accepts either type.
fn deserialize_string_or_int<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Visitor;

    struct StringOrInt;

    impl<'de> Visitor<'de> for StringOrInt {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, integer, or null")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(Some(v.to_string()))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v.to_string()))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v.to_string()))
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_any(StringOrInt)
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
            property_type: None,
        }
    }

    /// Build a simple chain:  root → impl1 → impl2 → … → impl_n (all
    /// implementation nodes, single-child, non-local-project).
    fn make_chain(descriptions: &[&str]) -> DiagnosticsNode {
        assert!(!descriptions.is_empty());
        let mut nodes: Vec<DiagnosticsNode> = descriptions
            .iter()
            .enumerate()
            .map(|(i, &desc)| {
                let mut n = make_test_node(desc);
                n.value_id = Some(format!("id-{i}"));
                n
            })
            .collect();

        // Wire up: each node's child is the next.
        for i in (0..nodes.len() - 1).rev() {
            let child = nodes.remove(i + 1);
            nodes[i].children = vec![child];
            nodes[i].has_children = true;
        }
        nodes.remove(0)
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
    fn test_diagnostics_node_location_id_as_integer() {
        // Flutter sends locationId as an integer, not a string
        let json = r#"{"description": "Widget", "locationId": 9}"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.location_id.as_deref(), Some("9"));
    }

    #[test]
    fn test_diagnostics_node_location_id_as_string() {
        let json = r#"{"description": "Widget", "locationId": "42"}"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.location_id.as_deref(), Some("42"));
    }

    #[test]
    fn test_diagnostics_node_location_id_null() {
        let json = r#"{"description": "Widget", "locationId": null}"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert!(node.location_id.is_none());
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

    // ------------------------------------------------------------------
    // EdgeInsets tests
    // ------------------------------------------------------------------

    #[test]
    fn test_edge_insets_parse_trbl() {
        let ei = EdgeInsets::parse("EdgeInsets(8.0, 16.0, 8.0, 16.0)").unwrap();
        assert_eq!(
            ei,
            EdgeInsets {
                top: 8.0,
                right: 16.0,
                bottom: 8.0,
                left: 16.0
            }
        );
    }

    #[test]
    fn test_edge_insets_parse_all() {
        let ei = EdgeInsets::parse("EdgeInsets.all(8.0)").unwrap();
        assert_eq!(
            ei,
            EdgeInsets {
                top: 8.0,
                right: 8.0,
                bottom: 8.0,
                left: 8.0
            }
        );
    }

    #[test]
    fn test_edge_insets_parse_zero() {
        let ei = EdgeInsets::parse("EdgeInsets.zero").unwrap();
        assert!(ei.is_zero());
    }

    #[test]
    fn test_edge_insets_parse_invalid_returns_none() {
        assert!(EdgeInsets::parse("not an edge insets").is_none());
        assert!(EdgeInsets::parse("").is_none());
    }

    #[test]
    fn test_edge_insets_parse_missing_suffix_returns_none() {
        // Malformed: missing closing paren
        assert!(EdgeInsets::parse("EdgeInsets(8.0, 0.0, 8.0, 0.0").is_none());
    }

    #[test]
    fn test_edge_insets_parse_wrong_component_count_returns_none() {
        // Only 3 components instead of 4
        assert!(EdgeInsets::parse("EdgeInsets(8.0, 0.0, 8.0)").is_none());
    }

    #[test]
    fn test_edge_insets_zero_constructor() {
        let ei = EdgeInsets::zero();
        assert!(ei.is_zero());
        assert_eq!(ei.top, 0.0);
        assert_eq!(ei.right, 0.0);
        assert_eq!(ei.bottom, 0.0);
        assert_eq!(ei.left, 0.0);
    }

    #[test]
    fn test_edge_insets_is_zero_false_when_nonzero() {
        let ei = EdgeInsets {
            top: 1.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        };
        assert!(!ei.is_zero());
    }

    #[test]
    fn test_layout_info_default_has_no_padding_or_margin() {
        let info = LayoutInfo::default();
        assert!(info.padding.is_none());
        assert!(info.margin.is_none());
    }

    #[test]
    fn test_edge_insets_serialize_deserialize_roundtrip() {
        let ei = EdgeInsets {
            top: 4.0,
            right: 8.0,
            bottom: 4.0,
            left: 8.0,
        };
        let json = serde_json::to_string(&ei).unwrap();
        let restored: EdgeInsets = serde_json::from_str(&json).unwrap();
        assert_eq!(ei, restored);
    }

    // -----------------------------------------------------------------------
    // widget_runtime_type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_widget_runtime_type_strips_generics() {
        let node = make_test_node("BlocProvider<AppBloc>");
        assert_eq!(node.widget_runtime_type(), Some("BlocProvider"));
    }

    #[test]
    fn test_widget_runtime_type_no_generics() {
        let node = make_test_node("Container");
        assert_eq!(node.widget_runtime_type(), Some("Container"));
    }

    #[test]
    fn test_widget_runtime_type_empty_description_returns_none() {
        let node = make_test_node("");
        assert_eq!(node.widget_runtime_type(), None);
    }

    #[test]
    fn test_widget_runtime_type_only_generics_returns_none() {
        // Edge case: description starts with '<' — nothing before it.
        let node = make_test_node("<Foo>");
        assert_eq!(node.widget_runtime_type(), None);
    }

    // -----------------------------------------------------------------------
    // is_always_visible tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_always_visible_root_node() {
        let node = make_test_node("Root");
        // parent_child_count == 0 → root → always visible.
        assert!(node.is_always_visible(0));
    }

    #[test]
    fn test_is_always_visible_local_project() {
        let mut node = make_test_node("MyWidget");
        node.created_by_local_project = true;
        // Local-project node is always visible regardless of siblings/children.
        assert!(node.is_always_visible(1));
    }

    #[test]
    fn test_is_always_visible_multi_child() {
        let mut node = make_test_node("Column");
        node.children = vec![make_test_node("A"), make_test_node("B")];
        // Node has 2 children → always visible.
        assert!(node.is_always_visible(1));
    }

    #[test]
    fn test_is_always_visible_has_siblings() {
        let node = make_test_node("SiblingWidget");
        // parent_child_count == 2 → has siblings → always visible.
        assert!(node.is_always_visible(2));
    }

    #[test]
    fn test_is_always_visible_false_for_lone_impl_node() {
        // Single child, non-local, no siblings → not always visible.
        let mut node = make_test_node("Padding");
        node.children = vec![make_test_node("Center")];
        // parent_child_count == 1 (only child), non-local, 1 child.
        assert!(!node.is_always_visible(1));
    }

    // -----------------------------------------------------------------------
    // is_flex / is_flex_layout tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_flex_row() {
        let node = make_test_node("Row");
        assert!(node.is_flex());
    }

    #[test]
    fn test_is_flex_column() {
        let node = make_test_node("Column");
        assert!(node.is_flex());
    }

    #[test]
    fn test_is_flex_flex() {
        let node = make_test_node("Flex");
        assert!(node.is_flex());
    }

    #[test]
    fn test_is_flex_false_for_container() {
        let node = make_test_node("Container");
        assert!(!node.is_flex());
    }

    #[test]
    fn test_is_flex_layout_self_is_row() {
        let node = make_test_node("Row");
        assert!(node.is_flex_layout(None));
    }

    #[test]
    fn test_is_flex_layout_parent_is_column() {
        let parent = make_test_node("Column");
        let child = make_test_node("Container");
        // Child is not flex itself, but parent is.
        assert!(child.is_flex_layout(Some(&parent)));
    }

    #[test]
    fn test_is_flex_layout_false_when_neither() {
        let parent = make_test_node("Scaffold");
        let child = make_test_node("Container");
        assert!(!child.is_flex_layout(Some(&parent)));
    }

    #[test]
    fn test_is_flex_layout_no_parent_non_flex() {
        let node = make_test_node("Container");
        assert!(!node.is_flex_layout(None));
    }

    // -----------------------------------------------------------------------
    // is_render_object_property tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_render_object_property_true() {
        let mut node = make_test_node("size");
        node.property_type = Some("RenderObject".to_string());
        assert!(node.is_render_object_property());
    }

    #[test]
    fn test_is_render_object_property_false_when_none() {
        let node = make_test_node("padding");
        assert!(!node.is_render_object_property());
    }

    #[test]
    fn test_is_render_object_property_false_for_other_type() {
        let mut node = make_test_node("color");
        node.property_type = Some("Color".to_string());
        assert!(!node.is_render_object_property());
    }

    // -----------------------------------------------------------------------
    // property_type deserialization test
    // -----------------------------------------------------------------------

    #[test]
    fn test_diagnostics_node_deserialize_property_type() {
        let json = r#"{"description": "size", "propertyType": "RenderObject"}"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.property_type.as_deref(), Some("RenderObject"));
        assert!(node.is_render_object_property());
    }

    // -----------------------------------------------------------------------
    // build_inspector_rows tests
    // -----------------------------------------------------------------------

    fn empty_set() -> HashSet<String> {
        HashSet::new()
    }

    fn set_of(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_build_rows_single_root_returns_one_row() {
        let root = make_test_node("MaterialApp");
        let expanded = empty_set();
        let expanded_groups = empty_set();
        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &root,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: false,
        });
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[0].group, RowGroup::None);
    }

    #[test]
    fn test_build_rows_chain_folds_when_hide_implementation() {
        // Chain: root (non-local, single-child) → impl1 → impl2 → impl3
        // (all implementation nodes, single children, non-local).
        // root has no parent → always visible (parent_child_count == 0).
        // impl1 is a single child of root and root IS always-visible, so
        // impl1 is an implementation node → becomes leader.
        let chain = make_chain(&["Root", "Padding", "Center", "SizedBox"]);
        let expanded = set_of(&["id-0", "id-1", "id-2"]);
        let expanded_groups = empty_set();

        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &chain,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: true,
        });

        // Root is always visible (parent_child_count == 0) → standalone.
        // Padding is the sole child of Root, non-local → leader.
        assert_eq!(rows.len(), 2, "expect root + leader only");
        assert_eq!(rows[0].group, RowGroup::None);
        // The leader should be collapsed with 2 subordinates (Center, SizedBox).
        assert_eq!(rows[1].group, RowGroup::LeaderCollapsed { hidden_count: 2 });
    }

    #[test]
    fn test_build_rows_chain_all_visible_when_hide_implementation_false() {
        let chain = make_chain(&["Root", "Padding", "Center", "SizedBox"]);
        let expanded = set_of(&["id-0", "id-1", "id-2"]);
        let expanded_groups = empty_set();

        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &chain,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: false,
        });

        // All 4 nodes visible.
        assert_eq!(rows.len(), 4);
        for row in &rows {
            assert_eq!(row.group, RowGroup::None);
        }
    }

    #[test]
    fn test_build_rows_chain_leader_expanded_emits_members() {
        let chain = make_chain(&["Root", "Padding", "Center", "SizedBox"]);
        let expanded = set_of(&["id-0", "id-1", "id-2"]);
        // The leader is "id-1" (Padding).
        let expanded_groups = set_of(&["id-1"]);

        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &chain,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: true,
        });

        // Root (standalone) + LeaderExpanded + 2 Member rows.
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].group, RowGroup::None);
        assert_eq!(rows[1].group, RowGroup::LeaderExpanded);
        assert_eq!(rows[2].group, RowGroup::Member);
        assert_eq!(rows[3].group, RowGroup::Member);
    }

    #[test]
    fn test_build_rows_multi_child_interrupts_chain() {
        // Root → child1 (non-local, 2 children) → grandchild_a, grandchild_b.
        // child1 has 2 children → always visible → chain never forms.
        let mut root = make_test_node("Root");
        root.value_id = Some("root".to_string());
        let mut child1 = make_test_node("Column");
        child1.value_id = Some("col".to_string());
        child1.children = vec![make_test_node("A"), make_test_node("B")];
        child1.has_children = true;
        root.children = vec![child1];
        root.has_children = true;

        let expanded = set_of(&["root", "col"]);
        let expanded_groups = empty_set();

        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &root,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: true,
        });

        // root + Column + A + B — Column has 2 children so it's always-visible.
        assert_eq!(rows.len(), 4);
        for row in &rows {
            assert_eq!(row.group, RowGroup::None);
        }
    }

    #[test]
    fn test_build_rows_ticks_computed_correctly() {
        // Build a two-branch tree:
        // root
        // ├─ child_a (not last)
        // │  └─ grandchild_a (last child of child_a)
        // └─ child_b (last child of root)
        //    └─ grandchild_b (last child of child_b)
        //
        // Expected ticks:
        // root:          depth=0, ticks=[]
        // child_a:       depth=1, ticks=[]      (depth-0 pushed, line_to_parent=true)
        // grandchild_a:  depth=2, ticks=[0]     (depth-0 open because child_b follows)
        // child_b:       depth=1, ticks=[]      (last child of root)
        // grandchild_b:  depth=2, ticks=[]      (last child of child_b)

        let mut root = make_test_node("Root");
        root.value_id = Some("root".to_string());

        let mut child_a = make_test_node("ChildA");
        child_a.value_id = Some("ca".to_string());
        let mut grandchild_a = make_test_node("GrandA");
        grandchild_a.value_id = Some("gca".to_string());
        child_a.children = vec![grandchild_a];
        child_a.has_children = true;

        let mut child_b = make_test_node("ChildB");
        child_b.value_id = Some("cb".to_string());
        let mut grandchild_b = make_test_node("GrandB");
        grandchild_b.value_id = Some("gcb".to_string());
        child_b.children = vec![grandchild_b];
        child_b.has_children = true;

        root.children = vec![child_a, child_b];
        root.has_children = true;

        let expanded = set_of(&["root", "ca", "cb"]);
        let expanded_groups = empty_set();

        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &root,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: false,
        });

        assert_eq!(rows.len(), 5);

        // root
        assert_eq!(rows[0].depth, 0);
        assert!(rows[0].ticks.is_empty());

        // child_a (not last → line_to_parent true); its own ticks are empty
        // because no ancestor of child_a is a non-last sibling.
        assert_eq!(rows[1].depth, 1);
        assert!(rows[1].line_to_parent);
        assert!(rows[1].ticks.is_empty());

        // grandchild_a: ancestor child_a at depth 1 is not last (child_b follows),
        // so a │ connector is needed at column 1.
        assert_eq!(rows[2].depth, 2);
        assert!(!rows[2].line_to_parent, "last child of child_a");
        assert!(
            rows[2].ticks.contains(&1),
            "depth-1 tick expected because child_a (depth 1) has sibling child_b"
        );

        // child_b (last child of root → line_to_parent false)
        assert_eq!(rows[3].depth, 1);
        assert!(!rows[3].line_to_parent);
        assert!(rows[3].ticks.is_empty());

        // grandchild_b (last child of child_b)
        assert_eq!(rows[4].depth, 2);
        assert!(!rows[4].line_to_parent);
        assert!(rows[4].ticks.is_empty());
    }

    #[test]
    fn test_build_rows_local_project_node_breaks_chain() {
        // Chain: root → impl1 → local_widget → impl2
        // local_widget is created_by_local_project → always visible → chain breaks.
        let mut root = make_test_node("Root");
        root.value_id = Some("root".to_string());

        let mut impl1 = make_test_node("Padding");
        impl1.value_id = Some("impl1".to_string());

        let mut local_widget = make_test_node("MyWidget");
        local_widget.value_id = Some("local".to_string());
        local_widget.created_by_local_project = true;

        let mut impl2 = make_test_node("Center");
        impl2.value_id = Some("impl2".to_string());

        local_widget.children = vec![impl2];
        local_widget.has_children = true;
        impl1.children = vec![local_widget];
        impl1.has_children = true;
        root.children = vec![impl1];
        root.has_children = true;

        let expanded = set_of(&["root", "impl1", "local"]);
        let expanded_groups = empty_set();

        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &root,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: true,
        });

        // root (standalone), impl1 (leader, but local breaks the chain so
        // it sees 0 subordinates → standalone), local_widget (always-visible →
        // standalone), impl2 (lone impl child of local → either standalone or
        // leader).  The exact chain behaviour depends on parent_child_count;
        // local_widget has only impl2 as child (non-local, single-child) →
        // impl2 could be a leader.  But it has no children → count = 0 →
        // standalone.
        //
        // Key assertion: local_widget must appear as a standalone row (not
        // hidden inside a chain).
        let local_row = rows
            .iter()
            .find(|r| r.node.value_id.as_deref() == Some("local"));
        assert!(local_row.is_some(), "local_widget must appear in rows");
        assert_eq!(
            local_row.unwrap().group,
            RowGroup::None,
            "local-project node must be standalone"
        );
    }

    #[test]
    fn test_build_rows_last_child_line_to_parent_false() {
        // root with two children → last child has line_to_parent == false.
        let mut root = make_test_node("Root");
        root.value_id = Some("root".to_string());
        let mut a = make_test_node("A");
        a.value_id = Some("a".to_string());
        let mut b = make_test_node("B");
        b.value_id = Some("b".to_string());
        root.children = vec![a, b];
        root.has_children = true;

        let expanded = set_of(&["root"]);
        let expanded_groups = empty_set();

        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &root,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: false,
        });

        assert_eq!(rows.len(), 3);
        assert!(rows[1].line_to_parent, "first child: not last → true");
        assert!(!rows[2].line_to_parent, "second child: last → false");
    }
}
