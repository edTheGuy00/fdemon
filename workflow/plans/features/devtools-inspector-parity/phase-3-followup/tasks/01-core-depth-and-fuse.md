## Task: Fuse `parent_of` + `find_by_value_id` into a single depth-bounded DFS, document fields, sanitize `object_id`

**Objective**: Bundle four widget_tree.rs fixes from the Phase 3 review (M1, M2, m3, s3) that all touch `crates/fdemon-core/src/widget_tree.rs`. Replace the two-walk `compute_details_context` with a single fused pre-order DFS that captures `(found_node, parent_of_found)` and respects `MAX_TREE_WALK_DEPTH`. Add `///` doc comments to the public fields of `DetailsContext`. Apply ANSI sanitization to `DiagnosticsNode::object_id` for parity with other string fields.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/widget_tree.rs`

**Files Read (Dependencies):**
- `crates/fdemon-core/src/ansi.rs` — `deserialize_sanitized_option_string` helper (already used by other `DiagnosticsNode` fields)
- `workflow/reviews/features/devtools-inspector-parity/phase-3/ACTION_ITEMS.md` — M1, M2, m3, s3 specs

### Details

#### Background

The Phase 3 review identified four findings on `widget_tree.rs` that can be addressed together because they all touch the same file:

| ID | Severity | Sub-issue |
|----|----------|-----------|
| M1 | HIGH (security) / MAJOR (quality) / WARNING (architecture) | `parent_of_recursive` (line 739) and `find_by_value_id` (line 782) are unbounded recursive DFS — bypass the `MAX_TREE_WALK_DEPTH` defence-in-depth used by every other walker in this file. Stack overflow risk on adversarial trees. |
| M2 | MAJOR | `compute_details_context` performs two separate O(N) DFS passes; doc says "single walk." |
| m3 | MINOR | `DetailsContext::is_flex_layout` and `parent_type` public fields lack `///` doc comments (CODE_STANDARDS requires them on all `pub` items). |
| s3 | MEDIUM (security) | `DiagnosticsNode::object_id` is the only unsanitized `Option<String>` field on the struct — defence-in-depth gap. Phase 2 follow-up deferred it; Phase 3 review reverses that. |

#### 1. Fuse the two DFS walks (M1 + M2)

Current shape (`widget_tree.rs:762-780`):

```rust
pub fn compute_details_context(root: &DiagnosticsNode, target_value_id: &str) -> DetailsContext {
    if target_value_id.is_empty() {
        return DetailsContext::default();
    }
    let parent = parent_of(root, target_value_id);          // DFS pass 1
    let selected = if /* root matches */ {
        Some(root)
    } else {
        find_by_value_id(root, target_value_id)            // DFS pass 2
    };
    let Some(selected_node) = selected else {
        return DetailsContext::default();
    };
    DetailsContext {
        is_flex_layout: selected_node.is_flex_layout(parent),
        parent_type: parent.and_then(|p| p.widget_runtime_type().map(str::to_owned)),
    }
}
```

**New shape** — single fused pre-order DFS with depth guard:

```rust
/// Walks the tree rooted at `root` in a single depth-first pass, returning
/// `(matching_node, parent_of_matching_node)` for the node whose `value_id`
/// equals `target_value_id`. Returns `(None, None)` if the target is not
/// found (or if the target is the root itself — the root has no parent).
///
/// Bounded by `MAX_TREE_WALK_DEPTH` to defend against pathological trees.
fn find_with_parent<'a>(
    root: &'a DiagnosticsNode,
    target_value_id: &str,
) -> (Option<&'a DiagnosticsNode>, Option<&'a DiagnosticsNode>) {
    if target_value_id.is_empty() {
        return (None, None);
    }
    if root.value_id.as_deref() == Some(target_value_id) {
        return (Some(root), None);  // root match — no parent
    }
    find_with_parent_inner(root, target_value_id, 0)
}

fn find_with_parent_inner<'a>(
    parent: &'a DiagnosticsNode,
    target_value_id: &str,
    depth: usize,
) -> (Option<&'a DiagnosticsNode>, Option<&'a DiagnosticsNode>) {
    if depth > MAX_TREE_WALK_DEPTH {
        return (None, None);
    }
    for child in &parent.children {
        if child.value_id.as_deref() == Some(target_value_id) {
            return (Some(child), Some(parent));
        }
        let (found, found_parent) = find_with_parent_inner(child, target_value_id, depth + 1);
        if found.is_some() {
            return (found, found_parent);
        }
    }
    (None, None)
}
```

Then rewrite `compute_details_context`:

```rust
pub fn compute_details_context(
    root: &DiagnosticsNode,
    target_value_id: &str,
) -> DetailsContext {
    let (selected, parent) = find_with_parent(root, target_value_id);
    let Some(selected_node) = selected else {
        return DetailsContext::default();
    };
    DetailsContext {
        is_flex_layout: selected_node.is_flex_layout(parent),
        parent_type: parent.and_then(|p| p.widget_runtime_type().map(str::to_owned)),
    }
}
```

**API decision — keep `parent_of` as a thin shim, remove `find_by_value_id` if unused externally:**

- `parent_of(root, target_value_id)` is **kept** as the public API (it's re-exported from `lib.rs` and is the named tree-query verb). Reimplement it as: `find_with_parent(root, target_value_id).1` — extract just the parent.
- `find_by_value_id` is a private helper (introduced in Phase 3 task 01); grep to confirm it's not referenced outside `widget_tree.rs`. If it's unused externally, delete it. If anything references it (currently nothing does), keep it as a shim: `find_with_parent(root, target_value_id).0`.
- Public re-exports in `lib.rs` are unchanged: `DetailsContext`, `parent_of`, `compute_details_context`.

The `parent_of_recursive` private helper is **deleted** — its logic is now subsumed by `find_with_parent_inner`. Verify no other callers exist before removing.

#### 2. Update doc comments to honestly describe the single walk

Update `compute_details_context` doc (currently lines 755–761) to:

```rust
/// Walks `root` in a single depth-first pass to find the node with
/// `value_id == target_value_id` and its parent (if any), then derives
/// the visibility predicates.
///
/// Returns `DetailsContext::default()` if the target is not found or
/// `target_value_id` is empty. The walk is bounded by
/// `MAX_TREE_WALK_DEPTH`.
```

#### 3. Add per-field `///` doc comments to `DetailsContext` (m3)

Current (lines 715–719):

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailsContext {
    pub is_flex_layout: bool,
    pub parent_type: Option<String>,
}
```

New:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailsContext {
    /// Whether the selected widget participates in a flex layout.
    ///
    /// `true` if the selected widget's `widget_runtime_type` is `Row`,
    /// `Column`, or `Flex`, OR if its tree parent is one of those.
    /// Mirrors DevTools' `isFlexLayout` predicate
    /// (`diagnostics_node.dart:487`). Gates the Flex Explorer tab.
    pub is_flex_layout: bool,

    /// The tree parent's `widget_runtime_type()`, or `None` for the root.
    ///
    /// Captured during the same DFS as `is_flex_layout`; not consumed by
    /// current visibility logic but surfaced for diagnostics and possible
    /// future use.
    pub parent_type: Option<String>,
}
```

#### 4. Sanitize `DiagnosticsNode::object_id` (s3)

Current (line 94, approximately):

```rust
#[serde(default, rename = "objectId")]
pub object_id: Option<String>,
```

New (matches the pattern set by `value_id`, `name`, etc. in phase-2-followup task 04):

```rust
#[serde(
    default,
    rename = "objectId",
    deserialize_with = "deserialize_sanitized_option_string"
)]
pub object_id: Option<String>,
```

### Acceptance Criteria

1. `compute_details_context` performs exactly **one** DFS pass over `root.children` (verified by code inspection — only one recursive walker is invoked).
2. The fused walker respects `MAX_TREE_WALK_DEPTH` — returns `(None, None)` when depth exceeds the cap.
3. `parent_of` public API is preserved (signature and behavior unchanged from caller's perspective).
4. `DetailsContext::is_flex_layout` and `DetailsContext::parent_type` each carry a `///` doc comment.
5. `DiagnosticsNode::object_id` is sanitized at deserialization — ANSI escape sequences are stripped from the JSON input.
6. All existing `widget_tree.rs` tests pass (the 12 new Phase 3 tests + the prior baseline).
7. New unit tests pass:
   - Depth-cap test for the fused walker (analogous to `walk_node_returns_early_at_max_depth`).
   - ANSI-strip test for `object_id` (analogous to existing `diagnostics_node_value_id_strips_ansi_codes`).
8. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Add to the existing `#[cfg(test)] mod tests` block in `widget_tree.rs`, near the existing `DetailsContext` / `parent_of` tests:

```rust
#[test]
fn find_with_parent_returns_none_at_max_depth() {
    // Build a tree exactly MAX_TREE_WALK_DEPTH + 2 levels deep where the
    // target value_id sits at the bottom. The depth guard should prevent
    // the walker from reaching it.
    let mut current = DiagnosticsNode {
        description: "Leaf".into(),
        value_id: Some("deep-target".into()),
        ..Default::default()
    };
    for i in 0..(MAX_TREE_WALK_DEPTH + 2) {
        current = DiagnosticsNode {
            description: format!("Wrapper{}", i),
            children: vec![current],
            ..Default::default()
        };
    }
    let ctx = compute_details_context(&current, "deep-target");
    // Target unreachable → DetailsContext::default()
    assert_eq!(ctx, DetailsContext::default());
}

#[test]
fn compute_details_context_walks_tree_once() {
    // Regression test for the fused-walk fix: a tree where two passes would
    // give a different answer than one would not exist in practice, but we
    // can at least assert the public behavior is preserved after the fuse.
    let root = DiagnosticsNode {
        description: "Column".into(),
        children: vec![DiagnosticsNode {
            description: "Container".into(),
            value_id: Some("child".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let ctx = compute_details_context(&root, "child");
    assert!(ctx.is_flex_layout, "child of Column should be flex layout");
    assert_eq!(ctx.parent_type.as_deref(), Some("Column"));
}
```

For `object_id`:

```rust
#[test]
fn diagnostics_node_object_id_strips_ansi_codes() {
    let json = serde_json::json!({
        "description": "Container",
        "objectId": "\u{001b}[36mobjects/42\u{001b}[0m"
    });
    let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
    assert_eq!(node.object_id.as_deref(), Some("objects/42"));
}
```

### Notes

- **`parent_of` and `find_by_value_id` reachability:** Before deleting `parent_of_recursive` and (possibly) `find_by_value_id`, grep both crates and tests to confirm no external callers. The phase-3 main implementation introduced `find_by_value_id` as a private helper specifically for `compute_details_context`; if grep shows no external uses, delete it. If anything references it, keep it as `find_with_parent(...).0`.
- **API contract preservation:** `parent_of(root, target)` MUST continue to return `None` when `root` itself matches the target (root has no parent in the tree). The existing test `parent_of_returns_none_for_root_match` exercises this; preserve it.
- **`compute_details_context` performance:** Two walks → one walk. Both are O(N); the constant factor halves. Practical impact: imperceptible. The fix is for correctness-vs-doc and code-aesthetic reasons, not performance.
- **No behavior change for clean inputs.** All four sub-fixes are pure hardening — none change observable behavior for non-adversarial / well-formed VM Service responses.
- **`object_id` sanitization rationale:** The phase-2-followup task 04 deferral was based on "internal opaque token, not user-facing." That reasoning is sound today, but `pub` fields can be displayed by future consumers without checking the sanitization status. Applying the same `deserialize_with` attribute as every other `Option<String>` field on this struct removes the inconsistency and is a one-line change.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/widget_tree.rs` | (1) Replaced `parent_of_recursive` + `find_by_value_id` with fused `find_with_parent` / `find_with_parent_inner` DFS (M1+M2); (2) Rewrote `parent_of` as a thin shim over `find_with_parent`; (3) Rewrote `compute_details_context` to use single-pass fused walker; (4) Added per-field `///` doc comments to `DetailsContext` fields (m3); (5) Added `deserialize_with = "deserialize_sanitized_option_string"` to `object_id` field (s3); (6) Added 3 new tests |

### Notable Decisions/Tradeoffs

1. **`find_by_value_id` deleted**: Confirmed via grep that it had no external callers (it was introduced as a private helper exclusively for `compute_details_context`). Deleted rather than kept as a shim to avoid dead-code noise.
2. **`parent_of_recursive` deleted**: Its logic is fully subsumed by `find_with_parent_inner`. No external callers existed (it was always private).
3. **`parent_of` public API preserved**: Reimplemented as `find_with_parent(root, target_value_id).1` — same signature, same behavior, including `None` for root match. Existing tests (`parent_of_returns_none_for_root_match`, etc.) pass unchanged.
4. **`object_id` rename**: The `#[serde(rename_all = "camelCase")]` attribute on `DiagnosticsNode` already maps `object_id` → `objectId`, so no explicit `rename` attribute was needed in the new `#[serde(default, deserialize_with = ...)]` annotation.
5. **Depth parameter in `find_with_parent_inner` starts at 0**: Consistent with the pattern used in every other bounded walker in this file (`visible_node_count_inner`, `walk_node`). The root's direct children are visited at `depth == 0`, so a tree with `MAX_TREE_WALK_DEPTH + 2` wrapper layers has the target just out of reach.

### Testing Performed

- `cargo check -p fdemon-core` - Passed
- `cargo test -p fdemon-core` - Passed (460 tests)
- `cargo test --workspace` - Passed (all crates, no failures)
- `cargo fmt --all -- --check` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- New tests: `find_with_parent_returns_none_at_max_depth`, `compute_details_context_walks_tree_once`, `diagnostics_node_object_id_strips_ansi_codes` — all pass

### Risks/Limitations

1. **Depth-cap behavior at exactly `MAX_TREE_WALK_DEPTH`**: A child at depth exactly `MAX_TREE_WALK_DEPTH` (not `> MAX_TREE_WALK_DEPTH`) is still found. This matches the inclusive-at-cap behavior of other walkers in the file and is intentional.
