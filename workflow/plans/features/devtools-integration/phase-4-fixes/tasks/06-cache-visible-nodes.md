## Task: Cache `visible_nodes()` to Avoid Per-Frame Allocation

**Objective**: Replace the per-call `Vec` allocation in `InspectorState::visible_nodes()` with a cached result that is invalidated only when the tree structure or expand/collapse state changes.

**Depends on**: 02-fix-vm-connection, 04-session-switch-reset

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/state.rs`: Add cached visible nodes with dirty flag
- `crates/fdemon-tui/src/widgets/devtools/inspector.rs`: Use cached accessor
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Use cached accessor for selected name lookup
- `crates/fdemon-app/src/handler/devtools.rs`: Use cached accessor in key handlers

### Details

#### Current Problem

`InspectorState::visible_nodes()` (state.rs:118-125) walks the entire widget tree and allocates a `Vec<(&DiagnosticsNode, usize)>` on every call. It's called from:

1. `inspector.rs:119` — inside `render_tree()`, every frame during render
2. `mod.rs:91` — inside `Widget::render()` for Layout panel, every frame
3. `devtools.rs:79, 192` — in key handlers (acceptable, not hot path)

The `mod.rs:91` call is additionally wasteful: it builds the entire visible list just to extract one element via `.nth(selected_index)`.

#### Fix: Cached Visible Nodes

Add a `visible_cache` field to `InspectorState` that stores the flattened visible list. Invalidate it whenever the tree or expand state changes.

**Challenge:** `visible_nodes()` currently returns `Vec<(&DiagnosticsNode, usize)>` — the references borrow from `self.root`. Storing this as a cached field creates a self-referential struct, which Rust doesn't allow with safe references.

**Solution:** Cache `Vec<(usize, usize)>` — indices into a pre-order traversal plus depths — or cache `Vec<(NodeIdentity, usize)>` where `NodeIdentity` lets you look up the node. The simplest approach is to cache the full owned data needed for rendering:

```rust
#[derive(Debug, Clone)]
pub struct VisibleNode {
    pub description: String,
    pub widget_type: Option<String>,
    pub value_id: Option<String>,
    pub object_id: Option<String>,
    pub has_children: bool,
    pub depth: usize,
}

pub struct InspectorState {
    pub root: Option<DiagnosticsNode>,
    pub expanded: HashSet<String>,
    pub selected_index: usize,
    pub loading: bool,
    pub error: Option<String>,
    visible_cache: Option<Vec<VisibleNode>>,
}
```

**Alternative (simpler, recommended):** Keep the existing `visible_nodes()` signature but add a method that rebuilds only when dirty:

```rust
impl InspectorState {
    /// Returns the cached visible nodes, rebuilding if the cache is stale.
    pub fn visible_nodes_cached(&mut self) -> &[VisibleNode] {
        if self.visible_cache.is_none() {
            self.visible_cache = Some(self.build_visible_nodes());
        }
        self.visible_cache.as_ref().unwrap()
    }

    /// Invalidate the cache (called after tree mutation or expand/collapse).
    pub fn invalidate_visible_cache(&mut self) {
        self.visible_cache = None;
    }

    fn build_visible_nodes(&self) -> Vec<VisibleNode> {
        // Same logic as current visible_nodes() but produces owned VisibleNode structs
    }
}
```

**Invalidation points** — call `invalidate_visible_cache()` in:

1. `handle_widget_tree_fetched` (devtools.rs) — when a new tree is loaded
2. `handle_inspector_navigate` (devtools.rs) — when expand/collapse changes the visible set
3. `InspectorState::reset()` (state.rs) — on reset

**TUI rendering:** The `Widget::render()` method receives `&DevToolsViewState` (immutable). To use `visible_nodes_cached(&mut self)`, either:
- (a) Change the TUI to receive `&mut DevToolsViewState` — breaks the pure render contract
- (b) Use interior mutability (`RefCell` or `Cell<Option<...>>`) on the cache
- (c) Keep `visible_nodes()` as-is but also provide `selected_node_description()` — a targeted accessor for `mod.rs:91`

**Recommended approach (c):** Don't change the render signature. Instead:

1. Add `selected_node_description(&self) -> Option<String>` that traverses to the `nth` visible node and returns just its description — O(n) traversal but no allocation.
2. For `inspector.rs:119`, keep calling `visible_nodes()` (it's needed for the full list) but consider passing the pre-built list from the handler layer via a cached field. Or accept the allocation since inspector rendering is only active when the Inspector panel is shown.
3. For `mod.rs:91`, replace with `self.state.inspector.selected_node_description()`.

This is the minimal fix that addresses the most wasteful call site without breaking the render signature or introducing interior mutability.

### Acceptance Criteria

1. `mod.rs:91` no longer calls `visible_nodes()` — uses a targeted accessor instead
2. The targeted accessor does not allocate a `Vec`
3. Inspector rendering still works correctly (tree display, scrolling, expand/collapse)
4. `selected_node_description()` returns `None` when no tree is loaded or index is out of bounds
5. All existing tests pass + new tests for the accessor
6. No performance regression in the handler path

### Testing

```rust
#[test]
fn test_selected_node_description_returns_correct_node() {
    let mut inspector = InspectorState::default();
    inspector.root = Some(make_tree_with_three_nodes());
    inspector.selected_index = 1;

    let desc = inspector.selected_node_description();
    assert_eq!(desc.as_deref(), Some("SecondNode"));
}

#[test]
fn test_selected_node_description_empty_tree() {
    let inspector = InspectorState::default();
    assert!(inspector.selected_node_description().is_none());
}

#[test]
fn test_selected_node_description_index_out_of_bounds() {
    let mut inspector = InspectorState::default();
    inspector.root = Some(make_single_node());
    inspector.selected_index = 99;
    assert!(inspector.selected_node_description().is_none());
}
```

### Notes

- A full caching solution with `visible_cache: Option<Vec<VisibleNode>>` is the ideal long-term fix but requires careful lifetime management or interior mutability in the render path. Deferring to a future optimization pass is acceptable.
- The targeted accessor approach is the pragmatic minimum: it eliminates the most wasteful call site (full Vec build for one element) without architectural changes.
- The `inspector.rs` render path still calls `visible_nodes()`. For very large trees (2000+ nodes), this allocation may cause frame drops. If profiling shows this is a problem, the full caching approach should be implemented.
- `collect_visible()` (state.rs:127-139) is a recursive function. For trees with thousands of levels, stack overflow is theoretically possible. In practice, Flutter widget trees rarely exceed ~100 levels of depth.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `selected_node_description(&self) -> Option<String>` and private `find_nth_description()` helper to `InspectorState`. Added 7 new tests covering empty tree, correct node, third node, index out of bounds, collapsed children, and parity with `visible_nodes()`. |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Replaced `visible_nodes().into_iter().nth(selected_index).map(...)` with `selected_node_description()` in the `DevToolsPanel::Layout` render branch. Eliminates the per-frame `Vec` allocation. |

### Notable Decisions/Tradeoffs

1. **Approach (c) — targeted accessor only**: Followed the task's recommended pragmatic minimum. `selected_node_description()` traverses the tree in pre-order using a countdown (`remaining` counter), stopping at the nth visible node without ever collecting a `Vec`. The `inspector.rs` render path keeps calling `visible_nodes()` since it needs the full list.

2. **Private recursive helper `find_nth_description`**: The traversal logic mirrors `collect_visible()` exactly — same visibility check, same expand-set guard, same pre-order ordering — but short-circuits and returns a `&str` borrow instead of pushing to a Vec. This keeps the two traversals in sync by sharing the same code shape.

3. **Test borrow workaround**: The parity test (`test_selected_node_description_no_allocation_path_matches_visible_nodes`) first collects descriptions into an owned `Vec<String>` to drop the borrow from `visible_nodes()` before mutating `selected_index`. This is test-only and has no production impact.

4. **`devtools.rs` handler uses left unchanged**: The two `visible_nodes()` calls in `handle_switch_panel` and `handle_inspector_navigate` are in handler paths (not the render hot path) and were acceptable per the task spec. They remain unchanged to keep the diff minimal.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (852 tests, 7 new)
- `cargo test -p fdemon-tui` - Passed (518 tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **`inspector.rs` render path still allocates**: The `WidgetInspector::render_tree()` method calls `visible_nodes()` every frame. For very large trees (2000+ nodes) this may cause frame drops. A full caching solution with `visible_cache: Option<Vec<VisibleNode>>` remains as a future optimization.

2. **Borrow-checker prevents caching in render**: The render signature `fn render(self, area: Rect, buf: &mut Buffer)` takes `&DevToolsViewState` immutably. A mutable cache on `InspectorState` would require `RefCell` or a signature change. Both are deferred as noted in the task notes.
