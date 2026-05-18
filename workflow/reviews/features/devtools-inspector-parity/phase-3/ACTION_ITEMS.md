# Action Items: DevTools Inspector Parity — Phase 3

**Review Date:** 2026-05-18
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2 (M1, M2)

## Critical Issues

None.

## Major Issues (Must Fix Before Merge to main)

### 1. Add depth caps to `parent_of_recursive` and `find_by_value_id`
- **Source:** architecture_enforcer (WARNING), code_quality_inspector (MAJOR), security_reviewer (HIGH)
- **File:** `crates/fdemon-core/src/widget_tree.rs:739-752, 782-795`
- **Problem:** Both functions are unbounded recursive DFS. Every other tree walker in this file applies `MAX_TREE_WALK_DEPTH` (constant + doc comment at lines 27-30 explicitly call it "defence-in-depth"). Adversarial or pathologically-deep `DiagnosticsNode` trees from the VM Service can trigger stack overflow during `handle_open_details`.
- **Required Action:**
  1. Add `depth: usize` parameter to both private helpers.
  2. Guard with `if depth > MAX_TREE_WALK_DEPTH { return None; }` at function entry.
  3. Pass `depth + 1` to all recursive calls.
  4. Public entry points (`parent_of`, `find_by_value_id`, `compute_details_context`) start the counter at `0` — call sites unchanged.
  5. Add two depth-cap unit tests modeled on `walk_node_returns_early_at_max_depth`.
- **Acceptance:** New tests pass; `cargo test -p fdemon-core` clean; no behavior change for trees ≤ `MAX_TREE_WALK_DEPTH` deep.

### 2. Fix "single walk" claim — either fuse the two DFS passes or correct the docs
- **Source:** code_quality_inspector (MAJOR), logic_reasoning_checker (warning), risks_tradeoffs (LOW)
- **Files:**
  - `crates/fdemon-core/src/widget_tree.rs:755-757` (`compute_details_context` doc)
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:690` (inline comment)
  - `docs/ARCHITECTURE.md:986` (architecture description)
- **Problem:** Doc/comments claim a single DFS; implementation calls `parent_of` then `find_by_value_id` (two O(N) passes).
- **Required Action (preferred):** Fuse into a single DFS that returns `(found_node, parent_of_found)`. The matching node's parent can be captured during the same traversal that locates the node. Update the doc to accurately describe a single pass.
- **Alternative Action:** If fusing is deferred, update all three locations to say "two depth-first passes" or remove the walk-count claim entirely.
- **Acceptance:** Existing 12 `widget_tree` tests + 6 `inspector` tests still pass; doc-walk count matches implementation.

## Minor Issues (Should Fix Before Merge — Small, Low-Risk)

### 3. Add `clamp_details_tab()` to `handle_inspector_properties_fetch_timeout`
- **Source:** code_quality_inspector, logic_reasoning_checker, risks_tradeoffs (MEDIUM)
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:494-524`
- **Problem:** Asymmetric with `fetch_failed` (which clamps at line 484) and `fetched` (which clamps at line 438). Benign today (timeout does not mutate `render_properties`) but invariant-breaking.
- **Action:** Add `inspector.clamp_details_tab();` before the `UpdateResult::none()` return, with a comment noting it preserves the "all settlement paths clamp" invariant. Add a unit test mirroring `handle_inspector_properties_fetched_clamps_active_tab_*`.

### 4. Add per-field `///` doc comments to `DetailsContext` public fields
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-core/src/widget_tree.rs:716-719`
- **Problem:** Public fields lack `///` (CODE_STANDARDS requirement).
- **Action:** Move/copy the field-semantics prose from the struct-level doc onto each field.

### 5. Add the missing 2-tab backward cycling test
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs` (test module)
- **Problem:** Task 03 acceptance criterion #6 requires 2-tab Properties↔RenderObject wrap in both directions; backward case is not tested.
- **Action:** Add a test that starts at `Properties` with `visible = [Properties, RenderObject]`, calls `handle_cycle_tab(state, false)` twice, and asserts `RenderObject → Properties`.

### 6. Add a debug-assert (or one-shot `tracing::warn!`) on the renderer fallback path
- **Source:** risks_tradeoffs (MEDIUM)
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs:142-154`
- **Problem:** Silent fallback masks handler-side clamp bugs (issue #3 above is exactly the bug class this hides).
- **Action:** Add `debug_assert!(visible.contains(&state.details_tab), "active tab not in visible_tabs — handler missed a clamp call");`. Renderer remains pure (assert is read-only).

## Suggestions / Nits (Defer to Follow-up if Desired)

### 7. Normalize `DetailsContext` import in the handler
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:700`
- Use the root re-export `fdemon_core::DetailsContext` instead of the sub-module path.

### 8. Drop unused `DetailsContext::parent_type` field — OR justify and consume it
- **File:** `crates/fdemon-core/src/widget_tree.rs:716-719`
- YAGNI: never read by visibility logic; every test fixture carries boilerplate.

### 9. Sanitize `DiagnosticsNode::object_id` at the serde boundary
- **File:** `crates/fdemon-core/src/widget_tree.rs:94`
- Pre-existing inconsistency (not Phase 3 regression). Apply `#[serde(default, deserialize_with = "deserialize_sanitized_option_string")]` for parity with `value_id`, `name`, etc.

### 10. Create explicit Phase 3 follow-up plan
- Create `workflow/plans/features/devtools-inspector-parity/phase-3-followup/TASKS.md` capturing:
  - Split `details/mod.rs` (758 lines, 51% over guideline)
  - Split `flex_explorer_tab.rs` (still flagged from P2-followup m1)
  - Decide `parent_type`'s fate (drop or consume)
  - Remove `DetailsTab::next`/`prev` if confirmed unused in production paths
  - Sanitize `object_id`
- Prevents the cumulative deferral cascade that has already spanned 3 phases.

## Re-review Checklist

After addressing M1 and M2 (and ideally m1/m2):

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] New depth-cap tests added for `parent_of` / `find_by_value_id`
- [ ] Either single-walk implementation OR accurate "two passes" doc in all 3 locations
- [ ] `handle_inspector_properties_fetch_timeout` clamps + has a test
- [ ] `DetailsContext` fields have `///` comments
- [ ] 2-tab backward cycle test exists
- [ ] Renderer fallback has `debug_assert!`
- [ ] Re-run `security_reviewer` and `code_quality_inspector` agents → expect ✅
