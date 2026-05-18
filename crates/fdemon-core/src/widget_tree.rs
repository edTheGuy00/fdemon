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

/// Maximum recursion depth for widget-tree walkers. Trees deeper than this
/// are truncated to prevent stack exhaustion on malformed or adversarial
/// VM Service responses. serde_json's default recursion limit (128) is the
/// first line of defence; this cap is a defence-in-depth fallback.
pub(crate) const MAX_TREE_WALK_DEPTH: usize = 512;

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
    #[serde(deserialize_with = "deserialize_sanitized_string")]
    pub description: String,

    /// Runtime type as string
    ///
    /// Sanitized at deserialize time to strip ANSI escape sequences.
    #[serde(
        default,
        rename = "type",
        deserialize_with = "deserialize_sanitized_option_string"
    )]
    pub node_type: Option<String>,

    /// Property name (for property nodes)
    ///
    /// Rendered directly to the terminal buffer in `properties_tab.rs` and
    /// `render_object_tab.rs`, so ANSI sequences in this field would corrupt
    /// terminal state. Sanitized at deserialize time (M4).
    #[serde(default, deserialize_with = "deserialize_sanitized_option_string")]
    pub name: Option<String>,

    /// Diagnostic level: "info", "debug", "warning", "error", "hidden", "off"
    ///
    /// Sanitized at deserialize time to strip ANSI escape sequences. Sanitizing
    /// does not break `filter_and_sort_by_level` because that function matches
    /// against clean literal strings; a clean Flutter response will never
    /// contain ANSI bytes in this field.
    #[serde(default, deserialize_with = "deserialize_sanitized_option_string")]
    pub level: Option<String>,

    /// Whether this node has children
    #[serde(default)]
    pub has_children: bool,

    /// Tree display style: "dense", "sparse", etc.
    ///
    /// Sanitized at deserialize time to strip ANSI escape sequences.
    #[serde(default, deserialize_with = "deserialize_sanitized_option_string")]
    pub style: Option<String>,

    /// VM Service object ID for this node's value — used as `arg` in subsequent calls
    ///
    /// Sanitized at deserialize time to strip ANSI escape sequences (defense-in-depth).
    #[serde(default, deserialize_with = "deserialize_sanitized_option_string")]
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
    ///
    /// Sanitized at deserialize time via [`deserialize_sanitized_option_string`] to
    /// strip any ANSI escape sequences that may appear in VM Service output.
    #[serde(
        default,
        rename = "propertyType",
        deserialize_with = "deserialize_sanitized_option_string"
    )]
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
    /// Trees deeper than [`MAX_TREE_WALK_DEPTH`] are truncated to prevent stack
    /// exhaustion on malformed VM Service responses.
    pub fn visible_node_count(&self) -> usize {
        self.visible_node_count_inner(0)
    }

    fn visible_node_count_inner(&self, depth: usize) -> usize {
        if depth > MAX_TREE_WALK_DEPTH {
            return 0;
        }
        if !self.is_visible() {
            return 0;
        }
        1 + self
            .children
            .iter()
            .map(|c| c.visible_node_count_inner(depth + 1))
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
        0, // parent_child_count: root has no parent → 0
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
    rows: &mut Vec<InspectorRow<'a>>,
) {
    // Defence-in-depth: truncate pathologically deep trees (e.g. malformed VM
    // Service responses).  serde_json's recursion limit (128) is the first line
    // of defence; this cap prevents stack exhaustion in the tree walker itself.
    if depth > MAX_TREE_WALK_DEPTH {
        return;
    }

    // Determine the RowGroup for this node.
    let group = if hide_implementation && !node.is_always_visible(parent_child_count) {
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
        group, // move — `group` is not used after this point
    });

    // Decide whether to recurse into children.
    let should_expand = node
        .value_id
        .as_deref()
        .is_none_or(|id| expanded.contains(id));

    if !should_expand || node.children.is_empty() {
        return;
    }

    // If this node is not the last child of its parent, push `depth - 1` (the
    // column of this node's own branch tick) so that all descendants see a `│`
    // at the correct column — i.e. aligned with the `├─` or `└─` tick drawn
    // for *this* node, not at this node's glyph column.
    //
    // Using `depth` (before this fix) placed the guideline at `glyph_col(depth)`
    // which is the same column as this node's *icon*, overwriting it.  The correct
    // column is `glyph_col(depth.saturating_sub(1))` — the parent depth.
    if line_to_parent {
        open_ticks.push(depth.saturating_sub(1));
    }

    // Re-read the group from the last pushed row (we moved it above).
    let group_ref = &rows.last().expect("just pushed").group;

    match group_ref {
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
/// always-visible, single child, no siblings) **and** the node is expanded
/// in the regular tree expand/collapse sense.
///
/// This walk guard mirrors [`count_visible_chain_subordinates`] exactly: both
/// functions stop when `!should_expand || child.children.is_empty()` so that
/// the count badge and the number of emitted Member rows always agree.
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

        // Mirror the counter's stop condition: if the child is not expanded
        // (or has no children) we stop here — the counter would stop too, so
        // the number of emitted members stays equal to `hidden_count`.
        let should_expand = child
            .value_id
            .as_deref()
            .is_none_or(|id| expanded.contains(id));
        if !should_expand || child.children.is_empty() {
            break;
        }

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
pub(crate) fn count_visible_chain_subordinates(
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
// DetailsContext
// ============================================================================

/// Per-open-details cached predicates derived from a `DiagnosticsNode` tree.
///
/// Populated by [`compute_details_context`] when the user opens the Inspector
/// Details view. Cached on `InspectorState::details_context` to avoid re-walking
/// the tree on every render. Cleared / overwritten by every open/close cycle.
///
/// Field semantics:
///
/// - `is_flex_layout`: mirrors DevTools' `isFlexLayout` predicate
///   (`diagnostics_node.dart:487`). True if the selected widget is `Row`,
///   `Column`, or `Flex`, OR if its tree parent is one of those. Used to gate
///   the Flex Explorer tab in the Details view.
///
/// - `parent_type`: the parent's `widget_runtime_type()` value, or `None` if
///   the selected node is the root (has no parent). Surfaced for diagnostics
///   / future debugging; not currently consumed by visibility logic but cheap
///   to capture during the same DFS.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailsContext {
    pub is_flex_layout: bool,
    pub parent_type: Option<String>,
}

/// Find the parent of the node whose `value_id == target_value_id` in `root`'s subtree.
///
/// Returns `None` if `root` itself matches (root has no parent), if no node in
/// `root` matches, or if `target_value_id` is empty.
///
/// Performs a single depth-first walk over `root.children` (and recursively).
/// Complexity: O(N) in tree size. Safe to call on every `handle_open_details`
/// because the result is cached on `InspectorState::details_context`.
pub fn parent_of<'a>(
    root: &'a DiagnosticsNode,
    target_value_id: &str,
) -> Option<&'a DiagnosticsNode> {
    if target_value_id.is_empty() {
        return None;
    }
    parent_of_recursive(root, target_value_id)
}

fn parent_of_recursive<'a>(
    parent: &'a DiagnosticsNode,
    target_value_id: &str,
) -> Option<&'a DiagnosticsNode> {
    for child in &parent.children {
        if child.value_id.as_deref() == Some(target_value_id) {
            return Some(parent);
        }
        if let Some(found) = parent_of_recursive(child, target_value_id) {
            return Some(found);
        }
    }
    None
}

/// Compute the [`DetailsContext`] for a selected node.
///
/// Walks `root` to find the node with `value_id == target_value_id` and its
/// parent (if any), then derives the visibility predicates.
///
/// Returns `DetailsContext::default()` if `target_value_id` is empty or if no
/// matching node is found in `root`. (The empty-default case still allows the
/// renderer to dispatch; the Properties tab is always visible.)
pub fn compute_details_context(root: &DiagnosticsNode, target_value_id: &str) -> DetailsContext {
    if target_value_id.is_empty() {
        return DetailsContext::default();
    }

    let parent = parent_of(root, target_value_id);
    let selected = find_by_value_id(root, target_value_id);

    let Some(selected_node) = selected else {
        return DetailsContext::default();
    };

    DetailsContext {
        is_flex_layout: selected_node.is_flex_layout(parent),
        parent_type: parent
            .and_then(|p| p.widget_runtime_type())
            .map(|s| s.to_string()),
    }
}

fn find_by_value_id<'a>(
    root: &'a DiagnosticsNode,
    target_value_id: &str,
) -> Option<&'a DiagnosticsNode> {
    if root.value_id.as_deref() == Some(target_value_id) {
        return Some(root);
    }
    for child in &root.children {
        if let Some(found) = find_by_value_id(child, target_value_id) {
            return Some(found);
        }
    }
    None
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
    #[serde(deserialize_with = "deserialize_sanitized_string")]
    pub file: String,

    /// Line number (1-based)
    pub line: u32,

    /// Column number (1-based)
    pub column: u32,

    /// Widget class name at this creation site
    #[serde(deserialize_with = "deserialize_sanitized_option_string", default)]
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
// Flex layout enums
// ============================================================================

/// `FlexFit` for a `Flexible`/`Expanded` child.
///
/// Mirrors Flutter's `FlexFit` enum:
/// - `Tight` — the child is forced to fill the available main-axis space.
/// - `Loose` — the child takes its intrinsic main-axis size up to the available space.
///
/// Source: `tmp/devtools/.../diagnostics_node.dart:78–81`. The JSON field is a
/// string (`"tight"` / `"loose"`). Default per DevTools when missing or
/// unrecognized is `Loose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlexFit {
    /// The child is forced to fill the available main-axis space.
    Tight,
    /// The child takes its intrinsic main-axis size up to the available space.
    #[default]
    Loose,
}

/// Flex container's primary direction.
///
/// Source: `tmp/devtools/.../inspector_data_models.dart:466`. Parsed from the
/// `direction` property in `renderObject.properties` (`"horizontal"` /
/// `"vertical"`). Default per DevTools: `Vertical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    /// Layout children along the horizontal axis (e.g., `Row`).
    Horizontal,
    /// Layout children along the vertical axis (e.g., `Column`).
    #[default]
    Vertical,
}

/// Flex container's main-axis alignment.
///
/// Source: `tmp/devtools/.../inspector_data_models.dart:467–469`.
/// Field name in `renderObject.properties`: `mainAxisAlignment`. Default: `Start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MainAxisAlignment {
    /// Place children at the start of the main axis.
    #[default]
    Start,
    /// Place children at the end of the main axis.
    End,
    /// Center children along the main axis.
    Center,
    /// Distribute children evenly, with the first at start and last at end.
    SpaceBetween,
    /// Distribute children evenly, with half-interval gaps at start and end.
    SpaceAround,
    /// Distribute children evenly, with equal gaps everywhere.
    SpaceEvenly,
}

/// Flex container's cross-axis alignment.
///
/// Source: `tmp/devtools/.../inspector_data_models.dart:470–472`.
/// Field name in `renderObject.properties`: `crossAxisAlignment`. Default: `Center`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CrossAxisAlignment {
    /// Align children at the start of the cross axis.
    Start,
    /// Align children at the end of the cross axis.
    End,
    /// Center children along the cross axis.
    #[default]
    Center,
    /// Stretch children to fill the cross axis.
    Stretch,
    /// Align children along their text baseline.
    Baseline,
}

/// Flex container's main-axis size policy.
///
/// Source: `tmp/devtools/.../inspector_data_models.dart:473`.
/// Field name in `renderObject.properties`: `mainAxisSize`. Default: `Max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MainAxisSize {
    /// The container shrinks to fit its children along the main axis.
    Min,
    /// The container expands to fill the available space along the main axis.
    #[default]
    Max,
}

// ============================================================================
// FlexChild
// ============================================================================

/// One child of a `Row`/`Column`/`Flex` container, as surfaced by
/// `ext.flutter.inspector.getLayoutExplorerNode`.
///
/// Source: `tmp/devtools/.../diagnostics_node.dart:139–154` describes the
/// per-child JSON shape:
/// - `size: { width, height }`
/// - `constraints: { minWidth, maxWidth, minHeight, maxHeight }`
/// - `parentData: { offsetX, offsetY }`
/// - `flexFactor: int | null`
/// - `flexFit: "tight" | "loose" | null`
///
/// Phase 2's `extract_layout_info` (task 04) walks `node.children` and emits
/// one `FlexChild` per child (whether or not it has flex; a child with
/// `flex_factor: None` is a fixed-size child).
///
/// Note: This struct is populated manually by `extract_layout_info` from raw JSON,
/// not via serde derive on the whole struct.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlexChild {
    /// `valueId` of the child node (for cross-reference back into the widget
    /// tree). Optional because some children may not carry a valueId.
    pub id: Option<String>,
    /// Description string (widget runtime type, e.g. `"Container"`,
    /// `"Padding"`). Sanitized at extract time.
    pub name: String,
    /// Child's measured size, if available.
    pub size: Option<WidgetSize>,
    /// Constraints the parent passed to this child.
    pub constraints: Option<BoxConstraints>,
    /// `flexFactor` from `parentData`. `None` means non-flex (fixed-size).
    pub flex_factor: Option<u32>,
    /// `flexFit`. `None` when the child is not a `Flexible`/`Expanded`.
    pub flex_fit: Option<FlexFit>,
    /// Pixel offset of the child within the parent (from `parentData`).
    /// Stored as `(offset_x, offset_y)` in logical pixels.
    pub parent_offset: Option<(f64, f64)>,
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
    #[serde(deserialize_with = "deserialize_sanitized_option_string", default)]
    pub flex_fit: Option<String>,

    /// Widget description (e.g., "Column", "SizedBox")
    #[serde(deserialize_with = "deserialize_sanitized_option_string", default)]
    pub description: Option<String>,

    /// Padding applied inside this widget's box
    pub padding: Option<EdgeInsets>,

    /// Margin applied outside this widget's box
    pub margin: Option<EdgeInsets>,

    /// Flex container's primary axis (only meaningful for `Row`/`Column`/`Flex`).
    /// `None` for non-flex widgets.
    pub direction: Option<Axis>,

    /// Main-axis alignment (only meaningful for flex containers).
    pub main_axis_alignment: Option<MainAxisAlignment>,

    /// Cross-axis alignment (only meaningful for flex containers).
    pub cross_axis_alignment: Option<CrossAxisAlignment>,

    /// Main-axis size policy (only meaningful for flex containers).
    pub main_axis_size: Option<MainAxisSize>,

    /// Children of the flex container, in render order. Empty for non-flex
    /// widgets or when the layout response did not include children.
    #[serde(skip)]
    pub children: Vec<FlexChild>,
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

/// Deserialize a `String` field, stripping any ANSI escape sequences.
///
/// Applied to string fields on [`DiagnosticsNode`] and [`CreationLocation`]
/// that are rendered directly to the terminal. Stripping at the deserialize
/// boundary prevents ANSI bytes from the Dart VM Service from leaking through
/// to Ratatui's buffer.
///
/// Uses [`crate::ansi::strip_ansi_codes`], which additionally removes
/// backslash-prefixed box-drawing characters and trailing backslashes from
/// Flutter's `--machine` mode output.
fn deserialize_sanitized_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: String = serde::Deserialize::deserialize(deserializer)?;
    Ok(crate::ansi::strip_ansi_codes(&raw))
}

/// Deserialize an `Option<String>` field, stripping any ANSI escape sequences.
///
/// `None` and JSON `null` are preserved as `None`. When a string value is
/// present, [`crate::ansi::strip_ansi_codes`] is applied before returning.
fn deserialize_sanitized_option_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(raw.map(|s| crate::ansi::strip_ansi_codes(&s)))
}

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
        // Expected ticks (after C4 fix — push depth.saturating_sub(1)):
        //
        // When child_a (depth=1, line_to_parent=true) is visited, we push
        // `1.saturating_sub(1) = 0` to open_ticks.  This records depth 0
        // as the column where the guideline │ should be drawn for descendants
        // of child_a — which is the column of child_a's own branch tick
        // (glyph_col(0) = 0), matching where └─ or ├─ was drawn for child_a.
        //
        // root:          depth=0, ticks=[]
        // child_a:       depth=1, ticks=[]      (no ancestor with pending siblings)
        // grandchild_a:  depth=2, ticks=[0]     (tick at depth-0 because child_b follows root)
        // child_b:       depth=1, ticks=[]      (last child of root)
        // grandchild_b:  depth=2, ticks=[]      (no pending siblings anywhere in ancestry)

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

        // grandchild_a: child_a (depth 1, non-last) pushed depth.saturating_sub(1)=0
        // to open_ticks, so grandchild_a sees a tick at depth 0.
        // The renderer draws │ at glyph_col(0) = 0 — aligned with child_a's ├ tick.
        assert_eq!(rows[2].depth, 2);
        assert!(!rows[2].line_to_parent, "last child of child_a");
        assert!(
            rows[2].ticks.contains(&0),
            "depth-0 tick expected (C4 fix): child_a pushed 0 not 1 to open_ticks"
        );
        assert!(
            !rows[2].ticks.contains(&1),
            "depth-1 tick must NOT be present after C4 fix"
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

    // -----------------------------------------------------------------------
    // M4: count/emit agreement tests
    // -----------------------------------------------------------------------

    /// Collect the number of Member rows emitted for a chain leader when the
    /// group is expanded, to compare against the `hidden_count` badge.
    fn count_emitted_members(
        root: &DiagnosticsNode,
        expanded: &HashSet<String>,
        expanded_groups: &HashSet<String>,
    ) -> (usize, usize) {
        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root,
            expanded,
            expanded_groups,
            hide_implementation: true,
        });

        // Find the hidden_count from the collapsed leader (before expansion).
        let no_groups = empty_set();
        let collapsed_rows = build_inspector_rows(InspectorRowBuilderInputs {
            root,
            expanded,
            expanded_groups: &no_groups,
            hide_implementation: true,
        });

        let badge = collapsed_rows.iter().find_map(|r| {
            if let RowGroup::LeaderCollapsed { hidden_count } = r.group {
                Some(hidden_count)
            } else {
                None
            }
        });

        let member_count = rows.iter().filter(|r| r.group == RowGroup::Member).count();

        (badge.unwrap_or(0), member_count)
    }

    /// Table of (chain_length, expanded_ids) test cases for count/emit parity.
    ///
    /// Each entry verifies that the badge count (`hidden_count`) equals the
    /// number of `Member` rows when the leader's group is expanded.
    #[test]
    fn count_and_emit_agree_for_various_chain_shapes() {
        // Case 1: Chain of 4 nodes, all expanded.
        // Root (always-visible) → impl1 (leader) → impl2 → impl3
        // badge = 2, emitted members = 2.
        {
            let chain = make_chain(&["Root", "Impl1", "Impl2", "Impl3"]);
            let expanded = set_of(&["id-0", "id-1", "id-2"]);
            let expanded_groups = set_of(&["id-1"]); // leader = id-1 (Impl1)
            let (badge, members) = count_emitted_members(&chain, &expanded, &expanded_groups);
            assert_eq!(
                badge, members,
                "[case 1] badge={badge} members={members}: chain length 4, all expanded"
            );
        }

        // Case 2: Chain of 5 nodes, all expanded.
        // Root → impl1 (leader) → impl2 → impl3 → impl4
        // badge = 3, emitted members = 3.
        {
            let chain = make_chain(&["Root", "Impl1", "Impl2", "Impl3", "Impl4"]);
            let expanded = set_of(&["id-0", "id-1", "id-2", "id-3"]);
            let expanded_groups = set_of(&["id-1"]);
            let (badge, members) = count_emitted_members(&chain, &expanded, &expanded_groups);
            assert_eq!(
                badge, members,
                "[case 2] badge={badge} members={members}: chain length 5, all expanded"
            );
        }

        // Case 3: Chain of 4 nodes, middle node NOT expanded.
        // Root → impl1 (leader) → impl2 [NOT expanded] → impl3 (unreachable).
        // Both counter and emitter should stop at impl2 because its subtree is
        // collapsed.
        {
            let chain = make_chain(&["Root", "Impl1", "Impl2", "Impl3"]);
            // id-2 (Impl2) is NOT in expanded — its children are hidden.
            let expanded = set_of(&["id-0", "id-1"]); // id-2 omitted
            let expanded_groups = set_of(&["id-1"]);
            let (badge, members) = count_emitted_members(&chain, &expanded, &expanded_groups);
            assert_eq!(
                badge, members,
                "[case 3] badge={badge} members={members}: chain of 4 with collapsed middle node"
            );
        }

        // Case 4: Chain of 2 nodes only (leader + 1 member).
        // Root → impl1 (leader) → impl2 (single subordinate).
        {
            let chain = make_chain(&["Root", "Impl1", "Impl2"]);
            let expanded = set_of(&["id-0", "id-1"]);
            let expanded_groups = set_of(&["id-1"]);
            let (badge, members) = count_emitted_members(&chain, &expanded, &expanded_groups);
            assert_eq!(
                badge, members,
                "[case 4] badge={badge} members={members}: chain of 3 (root + leader + 1 member)"
            );
        }

        // Case 5: Collapsed-state only — badge must equal the count from
        // count_visible_chain_subordinates.
        {
            let chain = make_chain(&["Root", "Impl1", "Impl2", "Impl3"]);
            let expanded = set_of(&["id-0", "id-1", "id-2"]);
            let expected_count =
                count_visible_chain_subordinates(&chain.children[0], &expanded, true);
            let no_groups = empty_set();
            let collapsed_rows = build_inspector_rows(InspectorRowBuilderInputs {
                root: &chain,
                expanded: &expanded,
                expanded_groups: &no_groups,
                hide_implementation: true,
            });
            let badge = collapsed_rows.iter().find_map(|r| {
                if let RowGroup::LeaderCollapsed { hidden_count } = r.group {
                    Some(hidden_count)
                } else {
                    None
                }
            });
            assert_eq!(
                badge,
                Some(expected_count),
                "[case 5] badge must equal count_visible_chain_subordinates"
            );
        }
    }

    // -----------------------------------------------------------------------
    // m9: depth cap tests
    // -----------------------------------------------------------------------

    /// Build a linear chain of `n` single-child nodes (all implementation
    /// nodes so they don't trigger chain-folding when hide_implementation is false).
    fn make_deep_chain(n: usize) -> DiagnosticsNode {
        let mut nodes: Vec<DiagnosticsNode> = (0..n)
            .map(|i| {
                let mut node = make_test_node(&format!("Node{i}"));
                node.value_id = Some(format!("deep-{i}"));
                node
            })
            .collect();
        // Wire: each node's only child is the next one.
        for i in (0..nodes.len() - 1).rev() {
            let child = nodes.remove(i + 1);
            nodes[i].children = vec![child];
            nodes[i].has_children = true;
        }
        nodes.remove(0)
    }

    #[test]
    fn walk_node_returns_early_at_max_depth() {
        // Build a chain of MAX_TREE_WALK_DEPTH + 100 nodes.
        let deep = make_deep_chain(MAX_TREE_WALK_DEPTH + 100);

        // Expand all nodes so the walker would recurse the full depth if uncapped.
        let expanded: HashSet<String> = (0..(MAX_TREE_WALK_DEPTH + 100))
            .map(|i| format!("deep-{i}"))
            .collect();
        let expanded_groups = empty_set();

        let rows = build_inspector_rows(InspectorRowBuilderInputs {
            root: &deep,
            expanded: &expanded,
            expanded_groups: &expanded_groups,
            hide_implementation: false,
        });

        // The walker should stop at MAX_TREE_WALK_DEPTH+1 (depth 0..=MAX_TREE_WALK_DEPTH).
        // Give a small slack (+8) for any slight variance in boundary handling.
        assert!(
            rows.len() <= MAX_TREE_WALK_DEPTH + 8,
            "expected at most {} rows for a deep chain (got {})",
            MAX_TREE_WALK_DEPTH + 8,
            rows.len()
        );
    }

    #[test]
    fn visible_node_count_truncated_at_max_depth() {
        // A chain deeper than MAX_TREE_WALK_DEPTH should be truncated.
        let deep = make_deep_chain(MAX_TREE_WALK_DEPTH + 100);
        let count = deep.visible_node_count();
        // Should be capped around MAX_TREE_WALK_DEPTH + 1.
        assert!(
            count <= MAX_TREE_WALK_DEPTH + 2,
            "visible_node_count should be capped at max depth (got {count})"
        );
    }

    // -----------------------------------------------------------------------
    // ANSI sanitisation at deserialize boundary (M7)
    // -----------------------------------------------------------------------

    #[test]
    fn deserialize_strips_ansi_escape_from_description() {
        // JSON \u001b is the ESC byte (0x1B). serde_json decodes this before
        // calling our deserializer, so the sanitizer receives a string with a
        // real ESC byte and strips it.  We use a raw string literal so the
        // \u001b is passed verbatim to serde_json as the JSON-level escape.
        let json = r#"{"description": "\u001b[31mContainerRED\u001b[0m"}"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.description, "ContainerRED");
    }

    #[test]
    fn deserialize_strips_ansi_from_creation_location_file() {
        // Embed a CSI sequence in the file path (adversarial VM Service output).
        let json =
            r#"{"file": "file:///path/to/\u001b[32mmain\u001b[0m.dart", "line": 10, "column": 5}"#;
        let loc: CreationLocation = serde_json::from_str(json).unwrap();
        assert_eq!(loc.file, "file:///path/to/main.dart");
    }

    #[test]
    fn deserialize_strips_ansi_from_creation_location_name() {
        let json = r#"{"file": "file:///main.dart", "line": 1, "column": 1, "name": "My\u001b[1mWidget\u001b[0m"}"#;
        let loc: CreationLocation = serde_json::from_str(json).unwrap();
        assert_eq!(loc.name.as_deref(), Some("MyWidget"));
    }

    #[test]
    fn deserialize_creation_location_name_none_preserved() {
        // When name is absent, it should remain None (not default to empty string).
        let json = r#"{"file": "file:///main.dart", "line": 1, "column": 1}"#;
        let loc: CreationLocation = serde_json::from_str(json).unwrap();
        assert!(loc.name.is_none());
    }

    #[test]
    fn deserialize_preserves_unicode_box_drawing_in_description() {
        // Box-drawing characters (U+2502, U+251C, U+2500) must survive stripping.
        let json = r#"{"description": "\u2502 Column \u251c\u2500 child"}"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.description, "\u{2502} Column \u{251c}\u{2500} child");
    }

    #[test]
    fn deserialize_strips_caret_notation_from_description() {
        // Flutter --machine mode encodes ESC as the two-char sequence ^[ (caret + bracket),
        // which is valid JSON text (no control characters) and must also be stripped.
        let json = r#"{"description": "^[[31mContainer^[[0m"}"#;
        let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.description, "Container");
    }

    #[test]
    fn deserialize_strips_ansi_from_layout_info_description() {
        let json = r#"{"description": "\u001b[33mColumn\u001b[0m"}"#;
        let info: LayoutInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.description.as_deref(), Some("Column"));
    }

    #[test]
    fn deserialize_strips_ansi_from_layout_info_flex_fit() {
        // LayoutInfo uses snake_case field names (no rename_all = "camelCase").
        let json = r#"{"flex_fit": "\u001b[32mtight\u001b[0m"}"#;
        let info: LayoutInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.flex_fit.as_deref(), Some("tight"));
    }
    // -----------------------------------------------------------------------
    // Flex enums
    // -----------------------------------------------------------------------

    #[test]
    fn flex_fit_deserializes_tight_and_loose() {
        let tight: FlexFit = serde_json::from_str("\"tight\"").unwrap();
        let loose: FlexFit = serde_json::from_str("\"loose\"").unwrap();
        assert_eq!(tight, FlexFit::Tight);
        assert_eq!(loose, FlexFit::Loose);
    }

    #[test]
    fn flex_fit_default_is_loose() {
        assert_eq!(FlexFit::default(), FlexFit::Loose);
    }

    #[test]
    fn main_axis_alignment_deserializes_space_between() {
        let v: MainAxisAlignment = serde_json::from_str("\"spaceBetween\"").unwrap();
        assert_eq!(v, MainAxisAlignment::SpaceBetween);
    }

    #[test]
    fn cross_axis_alignment_deserializes_stretch_and_baseline() {
        let stretch: CrossAxisAlignment = serde_json::from_str("\"stretch\"").unwrap();
        let baseline: CrossAxisAlignment = serde_json::from_str("\"baseline\"").unwrap();
        assert_eq!(stretch, CrossAxisAlignment::Stretch);
        assert_eq!(baseline, CrossAxisAlignment::Baseline);
    }

    #[test]
    fn axis_default_is_vertical() {
        assert_eq!(Axis::default(), Axis::Vertical);
    }

    #[test]
    fn axis_horizontal_deserializes() {
        let v: Axis = serde_json::from_str("\"horizontal\"").unwrap();
        assert_eq!(v, Axis::Horizontal);
    }

    #[test]
    fn cross_axis_alignment_default_is_center() {
        assert_eq!(CrossAxisAlignment::default(), CrossAxisAlignment::Center);
    }

    #[test]
    fn main_axis_size_default_is_max() {
        assert_eq!(MainAxisSize::default(), MainAxisSize::Max);
    }

    // -----------------------------------------------------------------------
    // FlexChild struct
    // -----------------------------------------------------------------------

    #[test]
    fn flex_child_default_is_empty() {
        let c = FlexChild::default();
        assert!(c.id.is_none());
        assert_eq!(c.name, "");
        assert!(c.flex_factor.is_none());
    }

    #[test]
    fn flex_child_clone_and_eq() {
        let c = FlexChild {
            id: Some("id-1".to_string()),
            name: "Container".to_string(),
            size: Some(WidgetSize {
                width: 100.0,
                height: 50.0,
            }),
            constraints: None,
            flex_factor: Some(2),
            flex_fit: Some(FlexFit::Tight),
            parent_offset: Some((10.0, 20.0)),
        };
        let c2 = c.clone();
        assert_eq!(c, c2);
    }

    #[test]
    fn layout_info_new_fields_default_to_none() {
        // Existing LayoutInfo tests remain valid: new fields default to None/empty.
        let info = LayoutInfo::default();
        assert!(info.direction.is_none());
        assert!(info.main_axis_alignment.is_none());
        assert!(info.cross_axis_alignment.is_none());
        assert!(info.main_axis_size.is_none());
        assert!(info.children.is_empty());
    }

    #[test]
    fn layout_info_deserializes_without_new_fields() {
        // A JSON blob that lacks the new flex fields must still deserialize cleanly.
        let json = r#"{"description": "Column"}"#;
        let info: LayoutInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.description.as_deref(), Some("Column"));
        assert!(info.direction.is_none());
        assert!(info.children.is_empty());
    }

    // -----------------------------------------------------------------------
    // property_type sanitization (Phase 2)
    // -----------------------------------------------------------------------

    #[test]
    fn property_type_strips_ansi_codes() {
        let json = serde_json::json!({
            "description": "Color",
            "propertyType": "\u{001b}[31mRenderObject\u{001b}[0m"
        });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert_eq!(node.property_type.as_deref(), Some("RenderObject"));
    }

    #[test]
    fn property_type_passes_through_clean_strings() {
        let json = serde_json::json!({
            "description": "Color",
            "propertyType": "RenderObject"
        });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert_eq!(node.property_type.as_deref(), Some("RenderObject"));
    }

    #[test]
    fn property_type_absent_is_none() {
        let json = serde_json::json!({ "description": "Text" });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert!(node.property_type.is_none());
    }

    // -----------------------------------------------------------------------
    // name / level / node_type / style / value_id sanitization (M4 + m9)
    // -----------------------------------------------------------------------

    #[test]
    fn diagnostics_node_name_strips_ansi_codes() {
        let json = serde_json::json!({
            "description": "Container",
            "name": "\u{001b}[31mwidget_name\u{001b}[0m"
        });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert_eq!(node.name.as_deref(), Some("widget_name"));
    }

    #[test]
    fn diagnostics_node_name_passes_clean_strings() {
        let json = serde_json::json!({
            "description": "Container",
            "name": "padding"
        });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert_eq!(node.name.as_deref(), Some("padding"));
    }

    #[test]
    fn diagnostics_node_level_strips_ansi_codes() {
        let json = serde_json::json!({
            "description": "Container",
            "level": "\u{001b}[33mfine\u{001b}[0m"
        });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert_eq!(node.level.as_deref(), Some("fine"));
        // Verify the level filter still works after sanitization
        assert!(matches!(node.level.as_deref(), Some("fine")));
    }

    #[test]
    fn diagnostics_node_value_id_strips_ansi_codes() {
        let json = serde_json::json!({
            "description": "Container",
            "valueId": "\u{001b}[36mobjects/42\u{001b}[0m"
        });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert_eq!(node.value_id.as_deref(), Some("objects/42"));
    }

    #[test]
    fn diagnostics_node_node_type_strips_ansi_codes() {
        let json = serde_json::json!({
            "description": "Container",
            "type": "\u{001b}[32mWidgetProperty\u{001b}[0m"
        });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert_eq!(node.node_type.as_deref(), Some("WidgetProperty"));
    }

    #[test]
    fn diagnostics_node_style_strips_ansi_codes() {
        let json = serde_json::json!({
            "description": "Container",
            "style": "\u{001b}[35mdense\u{001b}[0m"
        });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert_eq!(node.style.as_deref(), Some("dense"));
    }

    #[test]
    fn diagnostics_node_name_absent_is_none() {
        let json = serde_json::json!({ "description": "Text" });
        let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
        assert!(node.name.is_none());
    }

    // =========================================================================
    // DetailsContext / parent_of / compute_details_context tests
    // =========================================================================

    #[test]
    fn parent_of_returns_none_for_root_match() {
        let root = DiagnosticsNode {
            description: "MyApp".into(),
            value_id: Some("root-id".into()),
            children: vec![],
            ..Default::default()
        };
        assert!(parent_of(&root, "root-id").is_none());
    }

    #[test]
    fn parent_of_returns_immediate_parent() {
        let child = DiagnosticsNode {
            description: "Container".into(),
            value_id: Some("child-id".into()),
            ..Default::default()
        };
        let root = DiagnosticsNode {
            description: "Column".into(),
            value_id: Some("root-id".into()),
            children: vec![child],
            ..Default::default()
        };
        let parent = parent_of(&root, "child-id").unwrap();
        assert_eq!(parent.widget_runtime_type(), Some("Column"));
    }

    #[test]
    fn parent_of_returns_none_for_missing_target() {
        let root = DiagnosticsNode {
            description: "MyApp".into(),
            value_id: Some("root-id".into()),
            children: vec![],
            ..Default::default()
        };
        assert!(parent_of(&root, "nonexistent").is_none());
    }

    #[test]
    fn parent_of_returns_none_for_empty_target_id() {
        let root = DiagnosticsNode {
            description: "MyApp".into(),
            value_id: Some("root-id".into()),
            children: vec![],
            ..Default::default()
        };
        assert!(parent_of(&root, "").is_none());
    }

    #[test]
    fn compute_details_context_flex_widget_is_flex_layout() {
        let root = DiagnosticsNode {
            description: "Column".into(),
            value_id: Some("col-id".into()),
            ..Default::default()
        };
        let ctx = compute_details_context(&root, "col-id");
        assert!(ctx.is_flex_layout);
        assert_eq!(ctx.parent_type, None); // root has no parent
    }

    #[test]
    fn compute_details_context_child_of_flex_is_flex_layout() {
        let child = DiagnosticsNode {
            description: "Container".into(),
            value_id: Some("c-id".into()),
            ..Default::default()
        };
        let root = DiagnosticsNode {
            description: "Column".into(),
            value_id: Some("col-id".into()),
            children: vec![child],
            ..Default::default()
        };
        let ctx = compute_details_context(&root, "c-id");
        assert!(ctx.is_flex_layout);
        assert_eq!(ctx.parent_type.as_deref(), Some("Column"));
    }

    #[test]
    fn compute_details_context_non_flex_widget_is_not_flex_layout() {
        let child = DiagnosticsNode {
            description: "Container".into(),
            value_id: Some("c-id".into()),
            ..Default::default()
        };
        let root = DiagnosticsNode {
            description: "Padding".into(),
            value_id: Some("p-id".into()),
            children: vec![child],
            ..Default::default()
        };
        let ctx = compute_details_context(&root, "c-id");
        assert!(!ctx.is_flex_layout);
        assert_eq!(ctx.parent_type.as_deref(), Some("Padding"));
    }

    #[test]
    fn compute_details_context_unmatched_target_returns_default() {
        let root = DiagnosticsNode {
            description: "MyApp".into(),
            value_id: Some("root-id".into()),
            ..Default::default()
        };
        let ctx = compute_details_context(&root, "missing");
        assert_eq!(ctx, DetailsContext::default());
    }

    #[test]
    fn compute_details_context_empty_target_returns_default() {
        let root = DiagnosticsNode {
            description: "MyApp".into(),
            value_id: Some("root-id".into()),
            ..Default::default()
        };
        let ctx = compute_details_context(&root, "");
        assert_eq!(ctx, DetailsContext::default());
    }

    #[test]
    fn compute_details_context_row_widget_is_flex_layout() {
        let root = DiagnosticsNode {
            description: "Row".into(),
            value_id: Some("row-id".into()),
            ..Default::default()
        };
        let ctx = compute_details_context(&root, "row-id");
        assert!(ctx.is_flex_layout);
        assert_eq!(ctx.parent_type, None);
    }

    #[test]
    fn compute_details_context_child_of_row_is_flex_layout() {
        let child = DiagnosticsNode {
            description: "Text".into(),
            value_id: Some("text-id".into()),
            ..Default::default()
        };
        let root = DiagnosticsNode {
            description: "Row".into(),
            value_id: Some("row-id".into()),
            children: vec![child],
            ..Default::default()
        };
        let ctx = compute_details_context(&root, "text-id");
        assert!(ctx.is_flex_layout);
        assert_eq!(ctx.parent_type.as_deref(), Some("Row"));
    }

    #[test]
    fn parent_of_finds_deeply_nested_node() {
        let grandchild = DiagnosticsNode {
            description: "Leaf".into(),
            value_id: Some("leaf-id".into()),
            ..Default::default()
        };
        let child = DiagnosticsNode {
            description: "Middle".into(),
            value_id: Some("mid-id".into()),
            children: vec![grandchild],
            ..Default::default()
        };
        let root = DiagnosticsNode {
            description: "Root".into(),
            value_id: Some("root-id".into()),
            children: vec![child],
            ..Default::default()
        };
        let parent = parent_of(&root, "leaf-id").unwrap();
        assert_eq!(parent.widget_runtime_type(), Some("Middle"));
    }
}
