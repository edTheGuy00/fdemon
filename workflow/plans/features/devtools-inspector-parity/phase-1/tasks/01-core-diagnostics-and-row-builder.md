## Task: Core diagnostics helpers + inspector row builder

**Objective**: Add the domain primitives required by every other Phase 1 task — the `_alwaysVisible` / `is_flex` / `is_flex_layout` predicates on `DiagnosticsNode`, a `property_type` field for distinguishing render-object properties, and a brand-new `InspectorRow` / `RowGroup` row-builder algorithm (with vertical-guideline tick computation and hideable-group chain folding).

**Depends on**: None

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/widget_tree.rs`: Add helpers, types, row-builder, tests.

**Files Read (Dependencies):**
- `tmp/devtools/packages/devtools_app/lib/src/shared/diagnostics/diagnostics_node.dart` (DevTools reference for `_alwaysVisible`, `isFlex`, `isFlexLayout`, `inHideableGroup`).

### Details

This task adds pure-domain code with **zero internal dependencies** — `fdemon-core` has none, by design. Everything is unit-tested in the same file.

#### 1. New field on `DiagnosticsNode`

Add a `property_type: Option<String>` field deserialized from `propertyType` in the JSON. DevTools uses this to distinguish render-object property nodes (those with `propertyType == "RenderObject"`) from regular widget properties.

```rust
pub struct DiagnosticsNode {
    // …existing fields…
    #[serde(default, rename = "propertyType")]
    pub property_type: Option<String>,
}
```

#### 2. Helpers on `DiagnosticsNode`

Add these `pub` methods, each with `///` docs:

- `fn widget_runtime_type(&self) -> Option<&str>` — Returns the widget's runtime type without generic arguments. Implementation: take `self.description.as_str()` and strip from the first `<` if present; trim. Returns the same string when no `<` present. This matches DevTools' `widgetRuntimeType` getter (diagnostics_node.dart).
- `fn is_always_visible(&self, parent_child_count: usize) -> bool` — Mirrors DevTools `_alwaysVisible`. True when: `parent_child_count == 0` (root, no parent) OR `self.created_by_local_project` OR `self.children.len() > 1` OR `parent_child_count > 1` (has siblings). `parent_child_count` is the number of children on the node's parent (so siblings include self; predicate matches `(node.parent?.childrenNow ?? []).length > 1`).
- `fn is_flex(&self) -> bool` — True when `widget_runtime_type()` is one of `"Row" | "Column" | "Flex"`.
- `fn is_flex_layout(&self, parent: Option<&DiagnosticsNode>) -> bool` — True when `self.is_flex() || parent.map_or(false, |p| p.is_flex())`. Parent reference is supplied by the caller (state layer) because `DiagnosticsNode` does not back-link to its parent.
- `fn is_render_object_property(&self) -> bool` — True when `self.property_type.as_deref() == Some("RenderObject")`.

#### 3. New types `InspectorRow` / `RowGroup`

```rust
/// A single row in the inspector tree view, with rendering metadata.
#[derive(Debug, Clone)]
pub struct InspectorRow<'a> {
    pub node: &'a DiagnosticsNode,
    pub depth: usize,
    /// Depth values where a vertical guideline should be drawn through this row.
    /// A depth `d` is included if some ancestor at depth `d` still has more
    /// siblings to render below this row.
    pub ticks: Vec<usize>,
    /// True if this row is not the first child of its parent (used to pick
    /// between the `├─` and `└─` branch ticks; `false` means "last child"
    /// which uses `└─`).
    pub line_to_parent: bool,
    /// Group-folding marker for this row.
    pub group: RowGroup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowGroup {
    /// Standalone row, not part of a chain.
    None,
    /// Group leader (first hideable node in a chain) when the chain is
    /// collapsed. Renders as `+ N more widgets`. The subordinates are
    /// suppressed from the row list entirely.
    LeaderCollapsed { hidden_count: usize },
    /// Group leader when the chain is expanded. Renders normally; all
    /// subordinates follow as `Member` rows directly below.
    LeaderExpanded,
    /// A subordinate row of an expanded chain leader.
    Member,
}
```

#### 4. Row-builder algorithm

Add a free function (so callers can pass their own expand/collapse state without coupling `DiagnosticsNode` to `InspectorState`):

```rust
pub struct InspectorRowBuilderInputs<'a> {
    pub root: &'a DiagnosticsNode,
    /// Expanded value_id set (regular tree expand/collapse).
    pub expanded: &'a HashSet<String>,
    /// Expanded *group leader* value_id set — when a leader id is in this set,
    /// the chain is rendered expanded (Member rows shown); otherwise the
    /// leader renders as `LeaderCollapsed`.
    pub expanded_groups: &'a HashSet<String>,
    /// When false, chain-folding is disabled entirely and every visible node
    /// renders standalone. Mirrors DevTools' "Hide implementation widgets"
    /// toggle.
    pub hide_implementation: bool,
}

pub fn build_inspector_rows<'a>(inputs: InspectorRowBuilderInputs<'a>) -> Vec<InspectorRow<'a>>;
```

Algorithm (recursive walk + post-pass for ticks):

1. Walk `root` pre-order honoring `inputs.expanded` for regular nodes.
2. While walking, track a stack of `(node, remaining_siblings_after_this_node)` for each ancestor; that drives `ticks`.
3. **Chain detection** (only when `inputs.hide_implementation == true`):
   - Define a node as "implementation" if `!node.is_always_visible(parent_child_count)`.
   - When a node is implementation AND its parent is either always-visible-with-this-as-only-child OR another implementation node, it joins a chain.
   - The first implementation node in a chain becomes the leader. Subsequent implementation descendants attach as subordinates UNTIL the chain ends (next descendant is always-visible OR has >1 child OR has siblings).
   - If `inputs.expanded_groups` contains the leader's `value_id`, emit `LeaderExpanded` followed by `Member` rows for each subordinate. Otherwise emit a single `LeaderCollapsed { hidden_count = subordinates.len() }`.
4. `line_to_parent` is set per child: false for the LAST child of a parent (uses `└─`), true otherwise (uses `├─`).
5. After the walk, fix up `ticks` for each row: include depth `d < row.depth` if an ancestor at depth `d` is **not** the last sibling.

Provide a small helper `count_visible_chain_subordinates(node, expanded)` that walks the would-be chain to size the leader badge.

#### 5. Tests (target ≥ 15 new unit tests)

In the same file's `#[cfg(test)] mod tests` block, cover:

- `widget_runtime_type` strips generics (`"BlocProvider<AppBloc>"` → `"BlocProvider"`; `"Container"` → `"Container"`).
- `is_always_visible` permutations (root / local-project / multi-child / sibling).
- `is_flex` / `is_flex_layout` (Row / Column / Flex / non-flex / parent-is-flex).
- `build_inspector_rows`:
  - Empty tree (single root) returns one row, depth 0.
  - Long single-child non-local chain folds into 1 leader row (`LeaderCollapsed { hidden_count: N }`).
  - Same chain with `hide_implementation == false` renders every node.
  - Same chain with the leader id in `expanded_groups` emits `LeaderExpanded` + `Member` rows.
  - Multi-child branch interrupts chain folding (siblings keep nodes "always visible").
  - Tick computation: a deeply nested grid where some ancestors are last-siblings and some are not — confirm `ticks` contains only non-last-sibling ancestor depths.
  - Local-project node mid-chain breaks the chain (local nodes always render standalone, the chain restarts after).
  - Branch tick: last child of a parent has `line_to_parent == false`; non-last children have `line_to_parent == true`.

#### 6. Module organization

Keep everything in `widget_tree.rs` for now — the file is ~500 lines today and the additions push it to roughly 800–900 lines. If it exceeds the 500-line threshold from `docs/CODE_STANDARDS.md` after the additions, **defer splitting to a follow-up cleanup task**, not this one — splitting also requires touching every `use` site downstream and would balloon the task's blast radius.

### Acceptance Criteria

1. `cargo test -p fdemon-core` passes with at least 15 new tests added.
2. The new helpers + types are `pub` and documented with `///` doc comments.
3. The `_alwaysVisible` behavior exactly matches the DevTools predicate documented at `tmp/devtools/.../diagnostics_node.dart:664–672` (port the logic; the file may be referenced but not copied).
4. `build_inspector_rows` is deterministic: same inputs → same outputs (no `HashMap` iteration in the output order).
5. `cargo clippy -p fdemon-core --all-targets -- -D warnings` passes.

### Testing

Snapshot-style unit tests built with hand-rolled `DiagnosticsNode` fixtures (a small `fn make_node(...)` helper at the top of the test module). No external test framework needed — plain `#[test]` + `assert_eq!`.

### Notes

- Do not add a parent back-link to `DiagnosticsNode` — keep the type immutable and pass parent context through `InspectorRowBuilderInputs` or row-builder local state. DevTools maintains the parent ref on its mutable `RemoteDiagnosticsNode`; we deliberately keep our type plain.
- The `hidden_count` in `LeaderCollapsed` should be the count of subordinates only (not including the leader itself), matching DevTools' "N more widgets" text where the user sees the leader plus the count.
- Resist the temptation to add `to_glyph()` or rendering helpers here — `fdemon-core` is rendering-agnostic. Type-icon mapping lives in `fdemon-tui` (task 07).

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/widget_tree.rs` | Added `property_type` field to `DiagnosticsNode`; added `widget_runtime_type`, `is_always_visible`, `is_flex`, `is_flex_layout`, `is_render_object_property` methods; added `InspectorRow`, `RowGroup`, `InspectorRowBuilderInputs` types; added `build_inspector_rows` and `count_visible_chain_subordinates` free functions; added 29 new unit tests; updated `make_test_node` helper. |
| `crates/fdemon-core/src/lib.rs` | Re-exported `build_inspector_rows`, `count_visible_chain_subordinates`, `InspectorRow`, `InspectorRowBuilderInputs`, `RowGroup` from crate root. |

### Notable Decisions/Tradeoffs

1. **Tick computation approach**: Ticks are pushed by each non-last node into `open_ticks` AFTER emitting its own row and BEFORE recursing into children. This ensures a node's own row does not include a tick for its own non-last status — only its descendants see that tick. The alternative (pushing from parent before calling walk_node for child) incorrectly inflates the child's own ticks.

2. **Chain detection in `walk_node` vs pre-pass**: Chain detection happens inline during the walk rather than a separate pre-pass. This keeps the code simpler at the cost of calling `count_visible_chain_subordinates` for each implementation node encountered. Since widget trees are shallow in practice this is fine.

3. **IIFE avoided**: After resolving the tick logic, the match-with-no-early-return can be written cleanly without an IIFE closure.

4. **`widget_runtime_type` uses `description`**: The task spec says to use `self.description` (stripping `<…>`) rather than a separate `widgetRuntimeType` JSON field. DevTools has both; we follow the task spec.

5. **File size**: The file grew from ~500 lines to ~1650 lines. As the task notes, splitting is deferred to a follow-up cleanup task to avoid blowing up the blast radius.

### Testing Performed

- `cargo fmt --all -- --check` — PASS
- `cargo check --workspace --all-targets` — PASS
- `cargo test -p fdemon-core` — PASS (412 tests, 29 new)
- `cargo clippy -p fdemon-core --all-targets -- -D warnings` — PASS

### Risks/Limitations

1. **Chain-folding algorithm is single-pass**: The `count_visible_chain_subordinates` helper walks the chain twice (once to count, once to emit). For very deep single-child chains this doubles traversal, but Flutter trees are shallow enough that this is not a concern in practice.
