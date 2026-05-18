## Task: Add `DetailsContext`, `parent_of`, `compute_details_context` to `fdemon-core`

**Objective**: Provide pure tree-derived data needed for Phase 3 conditional tab visibility. Adds a `DetailsContext` value type plus helpers that locate a node's parent in a `DiagnosticsNode` tree and derive the `is_flex_layout` predicate from DevTools' `diagnostics_node.dart:487`.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/widget_tree.rs`

**Files Read (Dependencies):**
- `tmp/devtools/packages/devtools_app/lib/src/shared/diagnostics/diagnostics_node.dart` (~line 487, `isFlexLayout` predicate reference)
- `tmp/devtools/packages/devtools_app/lib/src/screens/inspector/widget_properties/properties_view.dart` (~lines 22–131, `DetailsTable` visibility rules)
- `crates/fdemon-core/src/widget_tree.rs` (existing `widget_runtime_type()`, `is_flex()`, `is_flex_layout(parent)` helpers — Phase 1/2 already added these)

### Details

#### Background

Phase 3 hides the Render Object and Flex Explorer tabs per widget type. The visibility rules require:

- **Render Object** — visible iff `!render_properties.is_empty()` (computed by app from VM Service response — no core change needed).
- **Flex Explorer** — visible iff selected widget OR its tree parent is `Row` / `Column` / `Flex`. The "tree parent" lookup is missing from core; `DiagnosticsNode` has no back-link to parent.

This task adds:

1. A `parent_of(root, target_value_id)` DFS that returns the parent node of a target identified by its `value_id`.
2. A `DetailsContext` struct that bundles the precomputed visibility-relevant fields for a selected node.
3. A `compute_details_context(root, target_value_id)` constructor that performs the parent lookup once and returns the `DetailsContext`.

`InspectorState` will cache one `DetailsContext` per open-details session (see task 02). Because selection is frozen while details are open, recomputation only happens on `handle_open_details` — cheap.

#### 1. Locate the `DiagnosticsNode` struct and existing helpers

`crates/fdemon-core/src/widget_tree.rs` already contains (verified by Phase 3 research):

- `widget_runtime_type(&self) -> Option<&str>` at lines 199–211 — strips generics from `description`.
- `is_flex(&self) -> bool` at lines 239–244 — matches `Row`/`Column`/`Flex`.
- `is_flex_layout(&self, parent: Option<&DiagnosticsNode>) -> bool` at lines 254–256 — self-or-parent flex predicate.
- `is_render_object_property(&self) -> bool` at lines 263–265 — `propertyType == "RenderObject"`.

No new predicate methods are needed; this task only adds the parent-lookup helper and the wrapping `DetailsContext` type.

#### 2. Add `parent_of` free function

Add to `crates/fdemon-core/src/widget_tree.rs`, near the other tree-walking helpers (e.g. after `inspector_rows` if present in core; otherwise as a top-level free function in the same file). Doc comment explains the contract:

```rust
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
```

Key invariants:

- Walks `children` (forward-only), never expects a back-link.
- Pure: takes `&'a DiagnosticsNode`, returns `Option<&'a DiagnosticsNode>` borrowed from the same root.
- Iterative-DFS via recursion is acceptable here — Flutter widget trees rarely exceed ~50 deep, no stack overflow risk.

#### 3. Add `DetailsContext` struct

Add to `crates/fdemon-core/src/widget_tree.rs`. Place near `DiagnosticsNode` (e.g. after `is_flex_layout` definition or in a logical "predicate types" block). Use `#[derive(Debug, Clone, Default, PartialEq, Eq)]` because it will be embedded in `InspectorState` (which derives `Debug, Clone`) and compared in unit tests:

```rust
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
```

Notes:
- `Default` yields `DetailsContext { is_flex_layout: false, parent_type: None }`. This is the "no details open" state — harmless because `visible_tabs()` (task 02) is only read when `details_open == true`, and `handle_open_details` (task 03) always overwrites this value before opening.
- `parent_type` is `String` not `&str` because the cached value outlives the tree borrow.

#### 4. Add `compute_details_context` constructor

Add to `crates/fdemon-core/src/widget_tree.rs`:

```rust
/// Compute the [`DetailsContext`] for a selected node.
///
/// Walks `root` to find the node with `value_id == target_value_id` and its
/// parent (if any), then derives the visibility predicates.
///
/// Returns `DetailsContext::default()` if `target_value_id` is empty or if no
/// matching node is found in `root`. (The empty-default case still allows the
/// renderer to dispatch; the Properties tab is always visible.)
pub fn compute_details_context(
    root: &DiagnosticsNode,
    target_value_id: &str,
) -> DetailsContext {
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
```

Notes:
- `find_by_value_id` may already exist elsewhere in the file under a different name (e.g. a private helper for the `expanded` set). Check before duplicating — if a similar helper exists with the same shape, expose it as `pub` and reuse. Otherwise add the small free function above.
- Two passes (parent + find) is acceptable; each is O(N) and trees are small. A single-pass version that captures both during one DFS is a possible optimization — skip unless profiling shows it matters.

### Acceptance Criteria

1. `parent_of(root, target)` returns the parent `DiagnosticsNode` of any non-root node matched by `value_id`; returns `None` for the root itself, missing target, or empty `target_value_id`.
2. `compute_details_context(root, target)` returns `DetailsContext { is_flex_layout: true, .. }` for a `Column` widget at any depth, and `is_flex_layout: true` for ANY widget whose tree parent is a `Column` / `Row` / `Flex`.
3. `compute_details_context` returns `DetailsContext::default()` for unmatched / empty target IDs without panicking.
4. `DetailsContext` derives `Debug`, `Clone`, `Default`, `PartialEq`, `Eq`.
5. New unit tests in `widget_tree.rs` cover the six cases below.
6. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Add to the existing `#[cfg(test)] mod tests` block (or an adjacent block) in `widget_tree.rs`:

```rust
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
```

Adjust field initialization syntax if `DiagnosticsNode` does not derive `Default` — use struct-literal init with explicit `false`/`Vec::new()` for the remaining fields. (Phase 2 follow-up tasks demonstrate the pattern.)

### Notes

- The `parent_type` field on `DetailsContext` is captured during the same DFS for negligible cost. It is currently unused by visibility logic but useful for debug output, future-proofing, and snapshot test assertions.
- `compute_details_context` is intentionally tolerant of bad inputs (missing target, empty ID): the caller (`handle_open_details`) might race against a `root.take()` reset; returning a safe default keeps the open-details path panic-free.
- `parent_of` does NOT honor the `inspector_rows` hideable-chain collapse — chain folding is a rendering concern, not a tree-structure concern. The parent in the underlying `DiagnosticsNode.children` tree is always the natural parent. This is correct: DevTools' `isFlexLayout` also operates on the raw tree.
- Do NOT add `Serialize` / `Deserialize` to `DetailsContext` — it is a derived in-memory value, never persisted or sent over the wire.
- If a future Phase needs to recompute `DetailsContext` on hot-restart / tree refresh, the cheap recourse is to call `compute_details_context(root, details_node_id)` again — no new RPCs required.
