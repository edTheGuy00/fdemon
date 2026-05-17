## Task: Tree Rendering Correctness Fixes

**Objective**: Fix three correctness bugs in the inspector tree renderer (the `branch_x = 0` sentinel collision, the guideline `│` off-by-one, and the chain-count vs chain-unfold mismatch) plus bundled cleanups in the surrounding `widget_tree.rs` module.

**Depends on**: —

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/widget_tree.rs` — chain-folding tick math, count/emit parity, dead `is_member` parameter, depth cap, move-instead-of-clone.
- `crates/fdemon-core/src/lib.rs` — demote `count_visible_chain_subordinates` re-export from `pub` to `pub(crate)`.
- `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs` — `Option<u16>` sentinel for `branch_x`.
- `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs` — strengthened column assertions on guideline + branch-tick tests.

**Files Read (Dependencies):**
- None.

### Review Items Resolved

- **C3** — `branch_x = 0` sentinel collides with valid x=0
- **C4** — Guideline `│` off-by-one (tick depth math)
- **M4** — Chain count badge mismatches chain-expanded length
- **m5** — `walk_node` `is_member` parameter is dead code; `RowGroup::Member` arm is unreachable
- **m8** — `count_visible_chain_subordinates` should be `pub(crate)`, not `pub`
- **m9** — `walk_node` recursion has no explicit depth cap
- **m10** — `group: group.clone()` avoidable move-by-clone

### Details

#### C3 — `branch_x = 0` sentinel (tree_panel.rs:226-238)

Current:
```rust
let branch_x = match tree_inner.x.checked_add(branch_col) {
    Some(x) if x < tree_inner.right() => x,
    _ => 0,
};
// ...
if branch_x > 0 && branch_x < tree_inner.right() { /* draw tick */ }
```

The `0` sentinel collides with a legitimate `tree_inner.x == 0 && branch_col == 0` case. Replace with `Option<u16>`:

```rust
let branch_x: Option<u16> = tree_inner.x.checked_add(branch_col)
    .filter(|&x| x < tree_inner.right());
if let Some(bx) = branch_x {
    // draw `├─` or `└─` glyphs at column bx
}
```

Strengthen the test: add or modify a test that renders a 2-child tree into a buffer with `tree_inner.x == 0` (build the buffer with no border) and asserts the branch tick char at exact column 0.

#### C4 — Guideline off-by-one (widget_tree.rs:419-421 + tree_panel.rs:205-218)

The algorithm pushes `open_ticks.push(depth)` at widget_tree.rs:420 for a non-last node at depth N. The renderer then iterates `for d in 0..row.depth` and draws `│` at `glyph_col(d)` when `d` is in ticks. This places the guideline at `glyph_col(N)` — the same column as the branch tick — so it's overwritten.

**Pick the algorithmic fix** (more semantic: a tick stores the column of the parent's branch, not the child's):

```rust
// widget_tree.rs:420
if !is_last_sibling_at_this_level {
    open_ticks.push(depth.saturating_sub(1));
}
```

Strengthen the existing `tree_renders_guidelines_for_nonlast_sibling_ancestors` test (tests.rs:776-831) to assert the `│` is at the **exact column** of the parent's branch tick — not just that `row.contains('│')` somewhere.

#### M4 — Chain count vs unfold mismatch (widget_tree.rs:475-541 vs 586-627)

`count_visible_chain_subordinates` (line 619-621) stops counting when it hits an unexpanded node. `emit_chain_members` (line 490) does NOT honour the `expanded` set — it walks as long as the chain shape holds. Result: badge says "+1 more"; unfold reveals three.

Fix: make `emit_chain_members` honour the `expanded` set the same way. Wherever the counter breaks, the emitter breaks. Extract the shared walk into a single helper consumed by both, or duplicate the loop guard verbatim — the implementor's call. Document the shared semantics in a `///` comment.

Add a **property-style test** (table-driven if `proptest` isn't already a dev-dep — check `Cargo.toml`):

```rust
#[test]
fn count_and_emit_agree_for_random_trees() {
    for tree_shape in sample_chain_shapes() {
        for expanded_set in sample_expanded_sets(&tree_shape) {
            let counted = count_visible_chain_subordinates(&tree_shape.leader, &expanded_set);
            let emitted = collect_emitted_members(&tree_shape.leader, &expanded_set);
            assert_eq!(counted, emitted.len(),
                "count vs emit mismatch for shape {:?} with expanded {:?}",
                tree_shape, expanded_set);
        }
    }
}
```

#### m5 — Dead `is_member` parameter (widget_tree.rs:361-365)

Every call site passes `is_member: false`. The `RowGroup::Member` arm in `walk_node` is unreachable. Remove the parameter and the arm. Update all call sites.

#### m8 — Demote `count_visible_chain_subordinates` re-export (fdemon-core/lib.rs:96-100)

Verify no consumer outside `fdemon-core` calls it (`rg "count_visible_chain_subordinates" crates/fdemon-{daemon,app,tui}` should return nothing). Then change the re-export from `pub use` to `pub(crate) use`, or remove the re-export entirely and mark the function itself `pub(crate)` in widget_tree.rs.

#### m9 — Depth cap on recursion (widget_tree.rs:133-142, 352-466)

Add `if depth > MAX_DEPTH { return; }` guards at the top of `walk_node` and `visible_node_count`. Choose `MAX_DEPTH = 512` (matches security_reviewer's suggestion; well above realistic Flutter trees). Add a module-level constant with a doc comment explaining the threshold and the implicit serde JSON recursion limit (128) as the first line of defence:

```rust
/// Maximum recursion depth for widget-tree walkers. Trees deeper than this
/// are truncated to prevent stack exhaustion on malformed or adversarial
/// VM Service responses. serde_json's default recursion limit (128) is the
/// first line of defence; this cap is a defence-in-depth fallback.
const MAX_TREE_WALK_DEPTH: usize = 512;
```

#### m10 — Move instead of clone (widget_tree.rs:397, 403)

```rust
rows.push(InspectorRow {
    node,
    depth,
    ticks: open_ticks.clone(),       // keep — open_ticks lives on
    line_to_parent,
    group: group.clone(),            // ← avoid: move instead
});
```

Use `group` directly (move into the struct literal). `group` is bound just above the push and unused after. The `ticks.clone()` IS necessary (the parent `open_ticks` continues mutating).

### Acceptance Criteria

1. `branch_x` in `tree_panel.rs:226-238` uses `Option<u16>`; no sentinel `0`.
2. Existing branch-tick test enhanced to assert exact column at `tree_inner.x == 0`.
3. `open_ticks.push(depth - 1)` (or equivalent — implementor may instead change the renderer loop, with a justification comment) so guideline column matches parent's branch tick.
4. `tree_renders_guidelines_for_nonlast_sibling_ancestors` test asserts exact column.
5. `count_visible_chain_subordinates` and `emit_chain_members` agree on what counts as a chain subordinate (both honour `expanded`).
6. New table-driven or property test verifies count/emit agreement for ≥4 distinct (shape, expanded-set) combinations.
7. `walk_node`'s `is_member` parameter removed; `RowGroup::Member` arm removed.
8. `MAX_TREE_WALK_DEPTH` constant added with doc comment; `walk_node` and `visible_node_count` both return early when `depth > MAX_TREE_WALK_DEPTH`.
9. `count_visible_chain_subordinates` is `pub(crate)` (or its re-export is removed from `fdemon-core/lib.rs`).
10. `group.clone()` in the row-push at widget_tree.rs:397-403 replaced with a move.
11. `cargo test -p fdemon-core` passes. New tests count toward the green-gate.
12. `cargo test -p fdemon-tui` passes — strengthened tests are green.
13. `cargo clippy --workspace --all-targets -- -D warnings` passes.

### Testing

Beyond the existing tests:

```rust
// fdemon-core/widget_tree.rs tests module
#[test]
fn count_visible_chain_subordinates_and_emit_chain_members_agree() { /* property test */ }

#[test]
fn walk_node_returns_early_at_max_depth() {
    // Build a synthetic 600-deep chain (mock or programmatic).
    let deep = make_deep_chain(600);
    let rows = build_inspector_rows(&InspectorRowBuilderInputs { /* ... */ });
    // Assertion: row count is bounded by MAX_TREE_WALK_DEPTH + a small constant.
    assert!(rows.len() <= MAX_TREE_WALK_DEPTH + 8);
}

// fdemon-tui/widgets/devtools/inspector/tests.rs
#[test]
fn tree_renders_branch_tick_at_column_zero_when_tree_inner_x_is_zero() {
    let buf = render_tree_into_buffer_borderless(/* two-child tree */);
    assert_eq!(buf.cell(0, 1).symbol(), "├");  // depth-1 child's branch tick
}

#[test]
fn tree_guideline_column_matches_parent_branch_tick_column() {
    let buf = render_tree_into_buffer(/* 3-level tree, non-last root child has a grandchild */);
    let parent_branch_col = /* column where ├ was drawn for the parent */;
    let grandchild_row = 2; // y-coordinate
    assert_eq!(buf.cell(parent_branch_col, grandchild_row).symbol(), "│");
}
```

### Notes

- Task is large; **may be split** into 04a (correctness: C3 + C4 + M4) and 04b (cleanups: m5 + m8 + m9 + m10) at the implementor's discretion. If split, both halves are scoped to the same files and must run sequentially (same write-file set).
- The task touches `tests.rs` in `fdemon-tui`. Task 09 also touches `tests.rs`. Both tasks run sequentially per the wave plan (04 in W1, 09 in W5 with 04 as dependency).
- Worktree note: parallel-safe with tasks 01, 02, 03 within W1 (no shared write files with any of them).

---

## Completion Summary

**Status:** Not Started
**Branch:** —

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
