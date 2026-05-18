# Phase 2 Follow-up — Review Findings Punch List — Task Index

## Overview

Follow-up fixes for the Phase 2 review findings documented at:
- `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md`
- `workflow/reviews/features/devtools-inspector-parity/phase-2/ACTION_ITEMS.md`

Covers the **3 critical + 5 major findings** that were flagged as blocking the Phase 2 merge to `main`. Minor findings are documented under "Deferred Minor Findings" at the bottom for tracking, to be addressed in a separate cleanup phase before Phase 3 begins.

**Top priority:** the user-reported visual bug in the Flex Explorer vertical main-axis strip (C1) is the highest-priority item — it's the first impression users get when opening the Flex Explorer tab on a `Column` widget.

**Total Tasks:** 5
**Estimated Hours:** 10–15 hours

## Task Dependency Graph

```
                Wave 1 — Fixes (parallel, file-disjoint)
   ┌───────────────────────┐ ┌─────────────────────────┐
   │ 01-flex-explorer-     │ │ 02-handler-stale-guard- │
   │  visual-fix           │ │  unification            │
   │ (flex_explorer_tab    │ │ (handler/devtools/      │
   │  .rs only)            │ │  inspector.rs only)     │
   │ C1 + C3 + M1 + m6/m7  │ │ C2 + M2                 │
   └─────────┬─────────────┘ └────────────┬────────────┘
             │                            │
   ┌─────────┴─────────────┐ ┌────────────┴────────────┐
   │ 03-actions-inspector- │ │ 04-core-diagnostics-    │
   │  hardening            │ │  name-sanitize          │
   │ (actions/inspector/   │ │ (fdemon-core/           │
   │  mod.rs only)         │ │  widget_tree.rs only)   │
   │ M3 + M5               │ │ M4 + m9                 │
   └─────────┬─────────────┘ └────────────┬────────────┘
             │                            │
             └──────────────┬─────────────┘
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
| 01 | [01-flex-explorer-visual-fix](tasks/01-flex-explorer-visual-fix.md) | Not Started | — | 3–4h | `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` |
| 02 | [02-handler-stale-guard-unification](tasks/02-handler-stale-guard-unification.md) | Not Started | — | 2–3h | `crates/fdemon-app/src/handler/devtools/inspector.rs` |
| 03 | [03-actions-inspector-hardening](tasks/03-actions-inspector-hardening.md) | Not Started | — | 2–3h | `crates/fdemon-app/src/actions/inspector/mod.rs` |
| 04 | [04-core-diagnostics-name-sanitize](tasks/04-core-diagnostics-name-sanitize.md) | Not Started | — | 1–2h | `crates/fdemon-core/src/widget_tree.rs` |
| 05 | [05-docs-update](tasks/05-docs-update.md) | Not Started | 01–04 | 1–2h | `docs/ARCHITECTURE.md` |

## Wave Schedule

| Wave | Tasks | Notes |
|------|-------|-------|
| W1 | 01, 02, 03, 04 | All four touch disjoint files. Foundation: visual fix, handler races, action-task hardening, core deserialization. |
| W2 | 05 | Documentation update reflecting the new sanitization fields and the stale-guard pattern change (Agent: doc_maintainer). |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-flex-explorer-visual-fix | `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` | `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` (palette + tab block plumbing); `crates/fdemon-core/src/widget_tree.rs` (axis/alignment enums, `LayoutInfo`); `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md` (C1, C3, M1 specs) |
| 02-handler-stale-guard-unification | `crates/fdemon-app/src/handler/devtools/inspector.rs` | `crates/fdemon-app/src/state.rs` (InspectorState fields, `details_node_id`); `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md` (C2, M2 specs) |
| 03-actions-inspector-hardening | `crates/fdemon-app/src/actions/inspector/mod.rs` | `crates/fdemon-app/src/message.rs` (Message variants); `crates/fdemon-daemon/src/vm_service/extensions/properties.rs` (`parse_properties_response`); `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md` (M3, M5 specs) |
| 04-core-diagnostics-name-sanitize | `crates/fdemon-core/src/widget_tree.rs` | `crates/fdemon-core/src/ansi.rs` (`strip_ansi_codes` via `deserialize_sanitized_option_string`); `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md` (M4, m9 specs) |
| 05-docs-update | `docs/ARCHITECTURE.md` | Tasks 01–04 completion summaries; `~/.claude/skills/doc-standards/schemas.md`; current ARCHITECTURE.md DevTools Subsystem section |

### Overlap Matrix

| Wave | Task Pair | Shared Write Files | Isolation Strategy |
|------|-----------|--------------------|--------------------|
| W1 | 01 + 02 | None | Parallel (worktree) |
| W1 | 01 + 03 | None | Parallel (worktree) |
| W1 | 01 + 04 | None | Parallel (worktree) |
| W1 | 02 + 03 | None | Parallel (worktree) |
| W1 | 02 + 04 | None | Parallel (worktree) |
| W1 | 03 + 04 | None | Parallel (worktree) |

No write-file collisions within Wave 1. All four Wave 1 tasks can be dispatched concurrently in isolated worktrees. Wave 2 (task 05) runs sequentially after Wave 1.

## Cross-Cutting Constraints

1. **Stale-guard unification choice (task 02).** The review proposes two options: (a) cross-check `response.node_id == state.details_node_id`, or (b) clear `pending_properties_node_id` and `properties_loading` in `handle_close_details`. Pick (a) for both properties AND layout handlers — it's the simpler diff, keeps in-flight tasks running to completion (the response is silently dropped), and unifies on `state.details_node_id` as the single source of truth. Option (b) requires care with the layout side too and changes more surface area.

2. **MainAxis label redesign approach (task 01).** Move both main-axis label text and alignment value into the outer block title alongside the cross-axis label. Keep only `▲` / `▼` arrows in the side strip. Rationale: any in-strip text presentation has to deal with `MAIN_AXIS_STRIP_WIDTH = 3` cells, and even widening the strip eats horizontal space from the child boxes which already get small at common terminal widths. Title-bar labels are a single-string change to `Block::title(...)`, are readable at any width, and consistent with how the cross-axis label is already presented.

3. **Per-RPC vs total timeout (task 03).** Choose the "tighten code to match doc" option: wrap the entire async block of `spawn_fetch_inspector_properties` in a single outer `tokio::time::timeout(PROPERTIES_FETCH_TIMEOUT, ...)`. Remove the per-RPC timeouts. This bounds total wall-clock at the documented 10s and matches the doc comment. The alternative (cap sub-fetch count with a constant) is also valid but loses fidelity vs DevTools when widgets do legitimately have multiple render-object properties.

4. **DiagnosticsNode sanitization scope (task 04).** Apply `deserialize_sanitized_option_string` to: `name`, `level`, `node_type`, `style`, `value_id`. Skip `object_id` and `location_id` for now — they're internal opaque tokens, not user-facing strings. Wrap in tests demonstrating ANSI codes are stripped from at least `name` (the M4 critical fix) and one other field (defense-in-depth verification).

5. **No new key bindings.** Phase 2 follow-up introduces no new keys.

6. **All four W1 tasks must independently pass `cargo fmt + check + test + clippy --workspace --all-targets -- -D warnings`.** Wave 3 of Phase 2 originally shipped with 26 latent `field_reassign_with_default` clippy violations; the orchestrator's quality gate caught them post-merge. Implementors must run the full gate locally before reporting completion.

## Success Criteria

Phase 2 follow-up is complete when:

- [ ] User confirms the Flex Explorer MainAxis label is readable (no more "pushed to the far right" complaint) — task 01.
- [ ] "Terminal too small" fallback in flex_explorer_tab.rs `render()` centres in the tab pane, not the full buffer — task 01.
- [ ] `render_flex_viz` no longer has a dead `inspector_state` parameter — task 01.
- [ ] New regression test: open details on A → close → open on B (with A's fetch in flight) → A's response arrives → B's details are NOT mutated — task 02.
- [ ] Properties and layout handlers both stale-guard on `state.details_node_id` (unified key) — task 02.
- [ ] All five `let _ = msg_tx.send(...).await` sites in `spawn_fetch_inspector_properties` replaced with `if let Err(e) = ... { tracing::error!(...) }` — task 03.
- [ ] `spawn_fetch_inspector_properties` worst-case wall-clock bounded by `PROPERTIES_FETCH_TIMEOUT` (10s), regardless of sub-fetch count. Doc comment matches code — task 03.
- [ ] `DiagnosticsNode.name` deserializes with ANSI codes stripped; new test in `widget_tree.rs` verifies this — task 04.
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.
- [ ] `docs/ARCHITECTURE.md` updated to reflect the new sanitization coverage and stale-guard pattern — task 05.

## Deferred Minor Findings

The following minor findings from the Phase 2 review are documented here for tracking but NOT scheduled for this follow-up. They should be addressed in a separate cleanup pass before Phase 3 begins, OR rolled into Phase 3 prep tasks.

| ID | Description | Recommended Owner |
|----|-------------|-------------------|
| m1 | `flex_explorer_tab.rs` (1,077 lines) and `actions/inspector/mod.rs` (907 lines) exceed the 500-line CODE_STANDARDS ceiling. Natural splits exist (`flex_explorer_tab/{strip.rs, child_boxes.rs, mod.rs}`; `actions/inspector/{mod.rs, layout.rs, properties.rs, widget_tree.rs}` — widget_tree already split). | Phase 3 prep |
| m2 | Helper duplication across `details/` siblings: `render_muted_centered`, `truncate_to`, and `render_object_tab`'s private `filtered_and_sorted` (duplicates `details/mod.rs::filter_and_sort_by_level`). Consolidate into `details/mod.rs` as `pub(super)` helpers. | Cleanup pass |
| m3 | `extract_flex_child` in `extensions/layout.rs:178-182` uses `as_u64()` which rejects JSON float `1.0`; align with `extract_layout_info.flex_factor` at `:118-123` which uses `as_f64()`. | Cleanup pass |
| m4 | `extra_actions` consumption divergence — `process.rs` manually chains `result.action.into_iter().chain(result.extra_actions)` instead of using `result.actions()`. Also consider privatizing the field so all multi-action construction goes through `actions_vec()`. | Cleanup pass |
| m5 | Layout cache not cleared on `SessionRestartCompleted` (pre-existing). Move layout-cache invalidation into `reset_details_and_groups()`. | Cleanup pass |
| m6 | Vacuous match in `cross_axis_label` (both `Axis` variants produce `"Cross Axis"`). **Bundled into task 01** since same file. | Task 01 |
| m7 | Wrong constant in `render_horizontal_flex` size guard (`MAIN_AXIS_STRIP_WIDTH.min(3)` used as height). **Bundled into task 01** since same file. | Task 01 |
| m8 | `unwrap()` in test assertions (`render_object_tab.rs:410-412`, `properties_tab.rs:419-420`) → use `.expect("...")`. `_unused: Option<()>` dead test-helper param in `render_object_tab.rs`. | Cleanup pass |
| m9 | Defense-in-depth: sanitize `DiagnosticsNode.level`, `node_type`, `style`, `value_id`. **Bundled into task 04** since same file and same one-line attribute change. | Task 04 |
| m10 | `inspector.render_properties` vec is unbounded (accumulates initial + every sub-fetch). Cap at e.g. 256 with a logged warning. | Cleanup pass |

Items m6, m7, m9 are **bundled into Wave 1 tasks** because they share the same file as a critical/major finding and the cost of doing them together is near-zero. Items m1, m2, m3, m4, m5, m8, m10 remain deferred.

## Notes

- The orchestrator that ran Phase 2 produced two unmerged worktree-style commits (the squash merges into `feat/devtools-inspector-parity`) plus one post-merge clippy fix. The branch passes the full quality gate as of `bbdcc57`. This follow-up should land on the same branch before merging to `main`.
- The review concluded "the implementation is architecturally sound at the layer-boundary level — no `core → *` violations, no `tui → daemon` leaks, TEA renderer purity preserved." These follow-up tasks address quality-of-implementation concerns, NOT architectural debt.
- Task 02's stale-guard unification touches the layout handler too (currently uses `selected_value_id()`), so even though the C2 race is in the properties handler, the fix sweeps both for consistency. This is the right scope — the divergence is a maintenance hazard, not just a properties-handler bug.
