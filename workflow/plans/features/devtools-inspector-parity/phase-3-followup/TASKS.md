# Phase 3 Follow-up — Review Findings Punch List — Task Index

## Overview

Follow-up fixes for the Phase 3 review findings documented at:
- `workflow/reviews/features/devtools-inspector-parity/phase-3/REVIEW.md`
- `workflow/reviews/features/devtools-inspector-parity/phase-3/ACTION_ITEMS.md`

Covers the **2 MAJOR + 4 MINOR + 2 NIT review findings** that need addressing before Phase 3 merges to `main`. Two NIT items (`parent_type` field fate; structural split of `details/mod.rs` + `flex_explorer_tab.rs`) are explicitly deferred — see "Deferred / Out of Scope" at the bottom.

**Top priority:** **M1** (stack-overflow defence-in-depth gap in two new tree walkers) is the highest-severity finding — `security_reviewer` flagged it as HIGH, three reviewers converged on it.

**Total Tasks:** 5
**Estimated Hours:** 7–11 hours

## Task Dependency Graph

```
                Wave 1 — Fixes (parallel, file-disjoint)
   ┌─────────────────────────┐ ┌─────────────────────────┐
   │ 01-core-depth-and-fuse  │ │ 02-handler-clamp-and-   │
   │ (widget_tree.rs)        │ │  tests                  │
   │ M1 + M2 + m3 + s3       │ │ (handler/devtools/      │
   │                         │ │  inspector.rs)          │
   │                         │ │ m1 + m4 + s1            │
   └─────────┬───────────────┘ └────────────┬────────────┘
             │                              │
   ┌─────────┴───────────────┐ ┌────────────┴────────────┐
   │ 03-tui-render-assert    │ │ 04-state-cleanup-dead-  │
   │ (details/mod.rs)        │ │  code                   │
   │ m2                      │ │ (state.rs)              │
   │                         │ │ Remove DetailsTab::next │
   │                         │ │  /prev + their tests    │
   └─────────┬───────────────┘ └────────────┬────────────┘
             └──────────────┬───────────────┘
                            ▼
                Wave 2 — Docs (single, doc_maintainer)
                ┌──────────────────────────┐
                │ 05-docs-update           │
                │ (Agent: doc_maintainer)  │
                │ docs/ARCHITECTURE.md     │
                │ depends: 01–04           │
                └──────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-core-depth-and-fuse](tasks/01-core-depth-and-fuse.md) | Pending | — | 2–3h | `crates/fdemon-core/src/widget_tree.rs` |
| 02 | [02-handler-clamp-and-tests](tasks/02-handler-clamp-and-tests.md) | Pending | — | 2–3h | `crates/fdemon-app/src/handler/devtools/inspector.rs` |
| 03 | [03-tui-render-assert](tasks/03-tui-render-assert.md) | Pending | — | 1h | `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` |
| 04 | [04-state-cleanup-dead-code](tasks/04-state-cleanup-dead-code.md) | Pending | — | 1h | `crates/fdemon-app/src/state.rs` |
| 05 | [05-docs-update](tasks/05-docs-update.md) | Pending | 01–04 | 1–2h | `docs/ARCHITECTURE.md` |

## Wave Schedule

| Wave | Tasks | Notes |
|------|-------|-------|
| W1 | 01, 02, 03, 04 | All four touch disjoint files in different crates — no write overlap. Foundation: core tree-walk hygiene, handler invariant fix, TUI observability, dead-code removal. |
| W2 | 05 | Documentation update reflecting the fused-walk implementation and new sanitization coverage (Agent: doc_maintainer). |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-core-depth-and-fuse | `crates/fdemon-core/src/widget_tree.rs` | `crates/fdemon-core/src/ansi.rs` (`deserialize_sanitized_option_string` for `object_id`); `workflow/reviews/features/devtools-inspector-parity/phase-3/ACTION_ITEMS.md` (M1, M2, m3, s3 specs) |
| 02-handler-clamp-and-tests | `crates/fdemon-app/src/handler/devtools/inspector.rs` | `crates/fdemon-app/src/state.rs` (`InspectorState::clamp_details_tab`, `details_context`, `DetailsTab`); `crates/fdemon-core/src/lib.rs` (root re-export of `DetailsContext`); `workflow/reviews/features/devtools-inspector-parity/phase-3/ACTION_ITEMS.md` (m1, m4, s1 specs) |
| 03-tui-render-assert | `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | `crates/fdemon-app/src/state.rs` (`visible_tabs`, `details_tab`); `workflow/reviews/features/devtools-inspector-parity/phase-3/ACTION_ITEMS.md` (m2 spec) |
| 04-state-cleanup-dead-code | `crates/fdemon-app/src/state.rs` | grep verification that `DetailsTab::next` / `DetailsTab::prev` are referenced only by their own tests (currently lines 2356–2365 of `state.rs`) |
| 05-docs-update | `docs/ARCHITECTURE.md` | Tasks 01–04 completion summaries; `~/.claude/skills/doc-standards/schemas.md`; current ARCHITECTURE.md DevTools Subsystem section (Inspector Details Tab Visibility ~line 978–991) |

### Overlap Matrix

(Pairs evaluated only between tasks in the same wave — i.e. tasks with no dependency between them.)

| Wave | Task Pair | Shared Write Files | Isolation Strategy |
|------|-----------|--------------------|--------------------|
| W1 | 01 + 02 | None — different crates | Parallel (worktree) |
| W1 | 01 + 03 | None — different crates | Parallel (worktree) |
| W1 | 01 + 04 | None — different files in same crate? No: 01 writes `fdemon-core/widget_tree.rs`; 04 writes `fdemon-app/state.rs` | Parallel (worktree) |
| W1 | 02 + 03 | None — different crates | Parallel (worktree) |
| W1 | 02 + 04 | None — same crate (`fdemon-app`), different files (`handler/devtools/inspector.rs` vs `state.rs`) | Parallel (worktree) |
| W1 | 03 + 04 | None — different crates | Parallel (worktree) |

No write-file collisions within Wave 1. All four Wave 1 tasks can be dispatched concurrently in isolated worktrees. Wave 2 (task 05) runs sequentially after Wave 1.

## Cross-Cutting Constraints

1. **M2 implementation choice — fuse, don't paper over.** Task 01 fuses `parent_of` and `find_by_value_id` into a single pre-order DFS that carries the current parent pointer and returns `(found_node, parent_of_found)`. After fusing, the existing public API surface (`parent_of`, `find_by_value_id`, `compute_details_context`) is preserved — `parent_of` and `find_by_value_id` become thin shims around the fused walker, OR `find_by_value_id` is removed if it's not used elsewhere. The `compute_details_context` doc honestly claims "single walk" because the implementation actually does one walk. The inline comment `"Walks the tree once"` in `inspector.rs:690` is then accurate without change. This eliminates the doc-drift root cause rather than masking it.

2. **M1 depth cap — match the existing pattern exactly.** The fused walker takes a `depth: usize` parameter, guards with `if depth > MAX_TREE_WALK_DEPTH { return None; }` at function entry, and passes `depth + 1` to recursive calls. Public entry points (`parent_of`, `compute_details_context`, and `find_by_value_id` if retained) start the counter at `0` — call sites unchanged. Two new unit tests model after the existing `walk_node_returns_early_at_max_depth` / `visible_node_count_truncated_at_max_depth` tests in the same file.

3. **s3 supersedes the phase-2-followup `object_id` deferral.** Phase 2 follow-up task 04 explicitly skipped `object_id` ("internal opaque token, not user-facing"). The Phase 3 `security_reviewer` re-flagged it as the only remaining unsanitized `Option<String>` field on `DiagnosticsNode` — a defence-in-depth gap. Task 01 reverses that decision: apply `#[serde(default, deserialize_with = "deserialize_sanitized_option_string")]` to `object_id` for parity with `name`, `level`, `node_type`, `style`, `value_id`, `property_type`. Cheap, no behavior change for clean inputs.

4. **m1 timeout-clamp symmetry — defensive, not behavior-changing.** Today, `handle_inspector_properties_fetch_timeout` does not mutate `render_properties` or `details_context`, so visible tabs cannot change as a result of a timeout. Adding `clamp_details_tab()` is a no-op in current code. The value is invariant preservation: any future change that clears `render_properties` on timeout will not silently leave a stale active tab. Task 02 adds the call **plus a test** that asserts the no-op behavior today, locking in the invariant.

5. **m2 renderer assertion — `debug_assert!` only, no production warning.** Task 03 uses `debug_assert!` rather than `tracing::warn!`. Rationale: in release builds the renderer fallback works correctly; the value of the assert is catching handler-side regressions in dev/test/CI, not production observability. The renderer remains pure (assertion is read-only).

6. **Task 04 scope — strict.** Remove ONLY `DetailsTab::next`, `DetailsTab::prev`, and their two unit tests (`detailstab_next_cycles_forward` / `detailstab_prev_cycles_backward`, currently `state.rs:2356–2365`). Confirm with grep that no production code (non-test) references these methods before deleting. If any reference is found in a non-test path, abort the task and report — do not delete in that case.

7. **No new key bindings, no user-visible behavior changes.** All five tasks are pure quality / hardening fixes.

## Success Criteria

Phase 3 follow-up is complete when:

- [ ] **M1:** Both new tree walkers respect `MAX_TREE_WALK_DEPTH`; depth-cap unit tests pass.
- [ ] **M2:** `compute_details_context` performs a single DFS pass; all existing 12+ `widget_tree` tests + 6 `inspector` tests pass; doc-comment claim matches reality.
- [ ] **m1:** `handle_inspector_properties_fetch_timeout` calls `clamp_details_tab()`; unit test asserts the invariant.
- [ ] **m2:** `render_details_panel` has `debug_assert!` on the fallback path; renderer remains pure (existing `details_panel_falls_back_to_properties_when_active_tab_hidden` test still passes).
- [ ] **m3:** `DetailsContext::is_flex_layout` and `DetailsContext::parent_type` each carry `///` doc comments.
- [ ] **m4:** New unit test exists for 2-tab backward cycling (Properties → RenderObject → Properties wrap when `visible_tabs() = [Properties, RenderObject]`).
- [ ] **s1:** `crates/fdemon-app/src/handler/devtools/inspector.rs:700` uses `DetailsContext::default()` (short form) with the import added to the existing `use fdemon_core::...` block.
- [ ] **s3:** `DiagnosticsNode::object_id` is sanitized at the serde boundary; tests verify ANSI stripping.
- [ ] Dead code removed: `DetailsTab::next` / `DetailsTab::prev` and their two tests no longer exist; production callers verified absent before removal.
- [ ] `docs/ARCHITECTURE.md` Inspector Details Tab Visibility section accurately describes a single-walk `compute_details_context`; DiagnosticsNode types list reflects `object_id` sanitization.
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.
- [ ] Re-running `security_reviewer` and `code_quality_inspector` on the post-followup diff returns no MAJOR/HIGH findings on the Phase 3 changes.

## Deferred / Out of Scope

The following items from the Phase 3 review are explicitly OUT OF SCOPE for this follow-up:

| ID | Description | Rationale for deferral | Recommended next step |
|----|-------------|------------------------|-----------------------|
| s2 | Drop `DetailsContext::parent_type` (or justify and consume) | Dropping forces fixture-init ripple across `handler/devtools/inspector.rs` and `widgets/devtools/inspector/details/mod.rs` tests, breaking Wave 1 parallelism. Field is harmless; cost of keeping is low. | Decide in Phase 4 — either consume it (e.g., debug overlay) or drop with a single-purpose cleanup task scheduled with no other tree-walk work in flight. |
| Structural | Split `details/mod.rs` (757 lines, 51% over the 500-line guideline) | Large refactor; doesn't block review-merge gate. | Create `phase-3-cleanup` plan covering both file splits below. |
| Structural | Split `flex_explorer_tab.rs` (1192 lines — pre-existing P2-followup m1 deferral) | Same as above. Has been deferred through two prior phases. | Bundle with `details/mod.rs` split in the cleanup plan. |

## Notes

- Phase 3 follow-up ships as a single PR — all 5 tasks merged together, mirroring the phase-2-followup cadence.
- This follow-up is **the gate** for merging the Phase 3 feature branch to `main`. The previously-merged Phase 3 commits + these follow-up commits will be the bundle that lands.
- Re-review (running `security_reviewer` and `code_quality_inspector` again) is the final sign-off step.
