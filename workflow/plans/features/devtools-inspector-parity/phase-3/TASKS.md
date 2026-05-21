# Phase 3 — Conditional Tab Visibility + Polish — Task Index

## Overview

Phase 3 of the DevTools Inspector parity feature. The Inspector Details view currently shows all three tabs (Widget Properties / Render Object / Flex Explorer) unconditionally for every widget. Phase 3 makes tab visibility data-driven, matching DevTools' `DetailsTable` predicate from `widget_properties/properties_view.dart:22–131`:

- **Widget Properties** — always visible (Phase 3 does not change).
- **Render Object** — visible iff the selected widget's `getProperties` response contained a node with `propertyType == "RenderObject"`. State signal: `!render_properties.is_empty()`.
- **Flex Explorer** — visible iff the selected widget OR its tree parent is `Row` / `Column` / `Flex` (mirrors DevTools' `isFlexLayout` from `diagnostics_node.dart:487`).

`Tab` / `Shift+Tab` skip hidden tabs while cycling. Active tab is clamped to a visible tab whenever the set changes (after `Enter` opens details on a new node, after a properties fetch arrives, after a tree refresh).

Cf. parent plan at `workflow/plans/features/devtools-inspector-parity/PLAN.md` §5.4 and §6 Phase 3.

**Total Tasks:** 5
**Estimated Hours:** 11–17 hours

## Task Dependency Graph

```
                Wave 1 — Core helper
   ┌──────────────────────────────────────────┐
   │ 01-core-details-context                  │
   │ (fdemon-core/widget_tree.rs)             │
   │ parent_of() + compute_details_context()  │
   │ + DetailsContext struct                  │
   └──────────────────┬───────────────────────┘
                      │
                      ▼
                Wave 2 — State surface
   ┌──────────────────────────────────────────┐
   │ 02-app-state-visible-tabs                │
   │ (fdemon-app/state.rs)                    │
   │ details_context field + visible_tabs() + │
   │ clamp_details_tab()                      │
   │ depends: 01                              │
   └──────────────────┬───────────────────────┘
                      │
       ┌──────────────┴──────────────┐
       ▼                             ▼
                Wave 3 — Wiring (parallel, file-disjoint)
   ┌─────────────────────────┐ ┌─────────────────────────┐
   │ 03-app-handler-cycle-   │ │ 04-tui-render-visible-  │
   │  and-context            │ │  tabs                   │
   │ (handler/devtools/      │ │ (details/mod.rs)        │
   │  inspector.rs)          │ │ tab strip uses          │
   │ handle_open_details     │ │ visible_tabs() + active │
   │ populates context;      │ │ tab clamp at render;    │
   │ handle_cycle_tab skips  │ │ updated + new snapshot  │
   │ hidden; clamp on        │ │ tests for 4 widget-type │
   │ properties_fetched      │ │ cases                   │
   │ depends: 02             │ │ depends: 02             │
   └────────────┬────────────┘ └────────────┬────────────┘
                └───────────────┬───────────┘
                                ▼
                Wave 4 — Docs (doc_maintainer)
                ┌──────────────────────────┐
                │ 05-docs-update           │
                │ (Agent: doc_maintainer)  │
                │ docs/ARCHITECTURE.md +   │
                │ docs/KEYBINDINGS.md      │
                │ depends: 01–04           │
                └──────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-core-details-context](tasks/01-core-details-context.md) | Done | — | 2–3h | `crates/fdemon-core/src/widget_tree.rs` |
| 02 | [02-app-state-visible-tabs](tasks/02-app-state-visible-tabs.md) | Done | 01 | 2–3h | `crates/fdemon-app/src/state.rs` |
| 03 | [03-app-handler-cycle-and-context](tasks/03-app-handler-cycle-and-context.md) | Done | 02 | 3–4h | `crates/fdemon-app/src/handler/devtools/inspector.rs` |
| 04 | [04-tui-render-visible-tabs](tasks/04-tui-render-visible-tabs.md) | Done | 02 | 3–5h | `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` |
| 05 | [05-docs-update](tasks/05-docs-update.md) | Done | 01–04 | 1–2h | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` |

## Wave Schedule

| Wave | Tasks | Notes |
|------|-------|-------|
| W1 | 01 | Core: add `DetailsContext` struct, `parent_of()` tree walk, `compute_details_context()` constructor — no app/tui consumers yet. |
| W2 | 02 | State: add `details_context: DetailsContext` field on `InspectorState`, `visible_tabs() -> SmallVec<[DetailsTab; 3]>` method, `clamp_details_tab()` method, `reset()` / `reset_details_and_groups()` clears. Depends on `DetailsContext` type from W1. |
| W3 | 03, 04 | Wiring: handler populates context at open + clamps on fetch (03); TUI strip iterates `visible_tabs()` + dispatch uses clamped tab + snapshot tests (04). File-disjoint — parallel-safe. |
| W4 | 05 | Documentation update reflecting the new `DetailsContext`, visibility rules, and tab-cycling behavior (Agent: doc_maintainer). |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-core-details-context | `crates/fdemon-core/src/widget_tree.rs` | `tmp/devtools/packages/devtools_app/lib/src/shared/diagnostics/diagnostics_node.dart` (`isFlexLayout` reference, ~line 487); `tmp/devtools/packages/devtools_app/lib/src/screens/inspector/widget_properties/properties_view.dart` (`DetailsTable` visibility predicate, ~lines 22–131) |
| 02-app-state-visible-tabs | `crates/fdemon-app/src/state.rs` | `crates/fdemon-core/src/widget_tree.rs` (`DetailsContext`, `parent_of`, `compute_details_context` from task 01) |
| 03-app-handler-cycle-and-context | `crates/fdemon-app/src/handler/devtools/inspector.rs` | `crates/fdemon-app/src/state.rs` (`InspectorState`, `visible_tabs`, `clamp_details_tab`, `details_context` from task 02); `crates/fdemon-core/src/widget_tree.rs` (`compute_details_context` from task 01) |
| 04-tui-render-visible-tabs | `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | `crates/fdemon-app/src/state.rs` (`visible_tabs`, `details_context` from task 02); `crates/fdemon-core/src/widget_tree.rs` (`DetailsContext` from task 01) |
| 05-docs-update | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` | Tasks 01–04 completion summaries; `~/.claude/skills/doc-standards/schemas.md`; current ARCHITECTURE.md DevTools Subsystem section |

### Overlap Matrix

(Pairs evaluated only between tasks in the same wave — i.e. tasks with no dependency between them.)

| Wave | Task Pair | Shared Write Files | Isolation Strategy |
|------|-----------|--------------------|--------------------|
| W3 | 03 + 04 | None — different crates, different files | Parallel (worktree) |

W1, W2, W4 each contain a single task — no overlap possible. Only Wave 3 has two parallel tasks, and they touch disjoint files in different crates (`fdemon-app/src/handler/devtools/inspector.rs` vs `fdemon-tui/src/widgets/devtools/inspector/details/mod.rs`).

## Cross-Cutting Constraints

1. **Visibility predicates are pure state queries.** `InspectorState::visible_tabs()` must never mutate state, never re-walk the tree, and never invoke RPCs. It reads `details_context.is_flex_layout` (computed once at open) and `render_properties.is_empty()` (a vec length). Per CODE_STANDARDS the TUI renderer is pure — `visible_tabs()` is called once per frame and may also be called from `handle_cycle_tab` / `handle_open_details`.

2. **`details_context` is computed at open-details time and frozen.** Because selection is frozen while details are open (parent PLAN §3.1 #1), the only state that can change tab visibility while details are open is `render_properties` arriving from the in-flight `getProperties` fetch. Walking the tree once at open and caching the result avoids per-frame tree walks. The cached `DetailsContext` is cleared by `reset()` and `reset_details_and_groups()`, and overwritten by every successful `handle_open_details` call.

3. **Active tab is clamped on every state transition that may hide it.** Specifically:
   - `handle_open_details` resets `details_tab = Properties` (existing behavior at `handler/devtools/inspector.rs:678`); Properties is always visible so no further clamp needed at open time.
   - `handle_inspector_properties_fetched` may add or remove the Render Object tab from `visible_tabs()`. Call `inspector.clamp_details_tab()` at the end of the handler.
   - `handle_inspector_properties_fetch_failed` may remove the Render Object tab (failed fetch → `render_properties` empty). Call `inspector.clamp_details_tab()` at the end.
   - `handle_close_details` resets state in a way the renderer never sees (`details_open = false`), so no clamp needed.
   - `reset_details_and_groups()` already clears `details_tab` to `Properties` — covered.

4. **Cycling honors the visible-tab list, not the static enum order.** `handle_cycle_tab` must compute `visible_tabs()`, find the current `details_tab`'s position, and advance/retreat within the visible-tab vec (with wrap). If `details_tab` is somehow not in `visible_tabs()` (defensive: should not happen post-clamp), fall back to the first visible tab. `DetailsTab::next()` / `DetailsTab::prev()` are NO LONGER called from `handle_cycle_tab` — leave them as-is for backwards compatibility with existing tests, but cycling now goes through `visible_tabs()`.

5. **Renderer must dispatch using the clamped active tab.** The match in `details/mod.rs:137–148` switches on `inspector_state.details_tab`. Phase 3 must defend against a stale/hidden `details_tab` value: when computing the dispatch target, the renderer should consult `visible_tabs()` and, if `details_tab` is not visible, fall back to dispatching the first visible tab (always `Properties`). The renderer does NOT mutate state — it just chooses what to draw. State mutation happens in handler clamp calls.

6. **Tab strip rendering iterates `visible_tabs()` only.** The `TAB_LABELS` constant in `details/mod.rs:73–77` becomes a label-lookup map keyed by `DetailsTab` rather than a fixed display order. The render loop in `render_tab_strip()` iterates `state.visible_tabs()` and looks up labels per tab. Hidden tabs are not drawn at all (no gap, no placeholder).

7. **`is_flex_layout` predicate uses tree parent, not visual parent.** DevTools' rule (`diagnostics_node.dart:487`): a node `isFlexLayout` if its `widgetRuntimeType in {Row, Column, Flex}` OR its tree parent's `widgetRuntimeType in {Row, Column, Flex}`. The tree parent is the parent in the `DiagnosticsNode.children` tree — not the visual / row-list parent. Task 01's `parent_of(root, value_id)` performs a DFS over `root.children` and returns the parent of the node whose `value_id` matches. Use the `summary tree` (post-Phase-1.5 sanitized) — same tree the inspector currently renders.

8. **`parent_of` handles chain-collapse correctly.** Hideable-chain group collapse only affects rendering (`InspectorRow`), not the underlying `DiagnosticsNode` tree. Task 01's `parent_of` walks the raw `DiagnosticsNode.children` tree and is independent of group state. The selected node's `value_id` always identifies a real `DiagnosticsNode` regardless of how it was reached visually.

9. **No new key bindings.** Phase 3 introduces no new keys. `Tab` / `Shift+Tab` already cycle (Phase 1); their effect is now "skip hidden tabs" within the existing `DevToolsInspectorCycleTab { forward }` message.

10. **Existing tests must not regress.** Two existing tests will need updating because they assert all three tabs unconditionally:
    - `tab_strip_renders_three_labels_in_order` (`details/mod.rs:~277`) — currently asserts `text.contains("Widget properties") && "Render object" && "Flex explorer"` after rendering with default `InspectorState`. Phase 3 default `InspectorState` has no `details_context` → no flex layout → flex tab hidden; no render_properties → render object tab hidden → only Properties tab visible. Update this test's fixture to set `details_context.is_flex_layout = true` and populate `render_properties` so all three tabs become visible, OR split into multiple widget-type-specific tests.
    - `handle_cycle_tab_forward_advances_through_three_tabs_with_wrap` / `..backward..` (`handler/devtools/inspector.rs:~2262`) — currently asserts the strict 3-tab cycle. Update their fixtures to populate `details_context` and `render_properties` so all three tabs are visible, preserving the original assertion. Add NEW tests that exercise 1-tab (Container) and 2-tab (Padding) cycling.

11. **Loading state is allowed to show fewer tabs.** When `handle_open_details` fires and `properties_loading` becomes true, `render_properties` is empty (cleared by open) → Render Object tab is hidden during the in-flight fetch. When the fetch returns, the tab "pops in" if non-empty. This is consistent with DevTools' DetailsTable behavior (the table re-renders on data arrival). Document this in the task 04 acceptance criteria; it is NOT a bug.

12. **Footer hint string left static.** `"[Esc] Close  [Tab] Next Tab  [Shift+Tab] Prev Tab  [r] Refresh  [b] Browser"` remains the details-mode hint at `widgets/devtools/mod.rs:349–350` even when 1 tab is visible. Cosmetic improvement (suppressing the Tab hints when `visible_tabs().len() == 1`) is OUT OF SCOPE for Phase 3 — it can be a follow-up. Rationale: the user can still press Tab and nothing visibly happens (`visible_tabs()` with 1 element wraps to itself), so the hint is technically still correct.

## Success Criteria

Phase 3 is complete when:

- [ ] Selecting a `Container` widget and pressing `Enter` opens Details with **only the Widget properties tab** visible in the strip. Tab cycling (`Tab`, `Shift+Tab`, `Right`, `Left`) is a no-op (1 visible tab, wraps to self).
- [ ] Selecting a `Padding` widget (a render-object widget that is not flex and not the child of a flex) opens Details with **Widget properties + Render object** (2 tabs) — no Flex Explorer tab. Cycling toggles between the two.
- [ ] Selecting a `Column` / `Row` / `Flex` widget opens Details with **all three tabs** visible. Cycling rotates through all three.
- [ ] Selecting a widget whose tree parent is `Column` / `Row` / `Flex` (e.g. a `Container` inside a `Column`) opens Details with **all three tabs** visible (the flex-explorer-by-parent rule from DevTools' `isFlexLayout`).
- [ ] During the in-flight `getProperties` fetch, the Render Object tab is **not** shown (since `render_properties.is_empty() == true`). Once the fetch returns, the tab appears if non-empty.
- [ ] If the user was on the Render Object tab and the active fetch fails (clearing `render_properties` back to empty if applicable, OR keeping the previous value — implementation choice documented), the active tab clamps to a visible tab. Clamping never panics, never leaves `details_tab` pointing at a hidden tab after a clamp call.
- [ ] `InspectorState::visible_tabs()` is a pure read of `details_context` + `render_properties.len()`. No tree walking, no RPC.
- [ ] `compute_details_context(root, value_id)` walks the tree once and returns `DetailsContext { is_flex_layout, parent_type }`. `parent_type` is the parent's `widget_runtime_type()` for debugging / future use; `is_flex_layout` is the canonical predicate.
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.
- [ ] Existing Phase 1 / Phase 2 / Phase 2-followup tests in the inspector code path do not regress, with the two named exceptions (tests 10) updated as documented.
- [ ] New snapshot tests (in `details/mod.rs` test module) exist for the four canonical widget-type cases: Container (1 tab), Padding (2 tabs), Column (3 tabs), Container-child-of-Column (3 tabs).
- [ ] `docs/ARCHITECTURE.md` DevTools Subsystem section is updated to document `DetailsContext`, the `visible_tabs` predicate, and the open-time computation. `docs/KEYBINDINGS.md` Inspector Panel section is updated to note that `Tab` / `Shift+Tab` skip hidden tabs.

## Keyboard Shortcuts (changed by Phase 3)

None added or removed. Behavior of existing keys refined:

| Key | Tree mode | Details mode (Phase 2) | Details mode (Phase 3) |
|---|---|---|---|
| `Tab` | (unbound) | Cycle forward through all 3 tabs | Cycle forward through visible tabs only |
| `Shift+Tab` | (unbound) | Cycle backward through all 3 tabs | Cycle backward through visible tabs only |
| `Right` / `l` | Expand node | Cycle forward through all 3 tabs | Cycle forward through visible tabs only |
| `Left` / `h` | Collapse node | Cycle backward through all 3 tabs | Cycle backward through visible tabs only |

## Deferred / Out of Scope

The following items are explicitly OUT OF SCOPE for Phase 3 (and remain deferred from Phase 2 follow-up):

| ID | Description | Recommended Owner |
|----|-------------|-------------------|
| m1 (P2f) | Split `flex_explorer_tab.rs` (>500 lines) and `actions/inspector/mod.rs` (>500 lines) into submodules | Post-Phase-3 cleanup |
| m2 (P2f) | Consolidate `render_muted_centered`, `truncate_to`, and the duplicated `filtered_and_sorted` in render_object_tab into shared `details/mod.rs` helpers | Post-Phase-3 cleanup |
| m3 (P2f) | `extract_flex_child` should use `as_f64()` instead of `as_u64()` to accept JSON float `1.0` for `flex_factor` | Post-Phase-3 cleanup |
| m4 (P2f) | `extra_actions` consumption divergence in `process.rs` | Post-Phase-3 cleanup |
| m5 (P2f) | Move layout-cache invalidation into `reset_details_and_groups()` so `SessionRestartCompleted` clears it | Post-Phase-3 cleanup |
| m8 (P2f) | Replace `unwrap()` with `.expect()` in test assertions in `render_object_tab.rs` and `properties_tab.rs` | Post-Phase-3 cleanup |
| m10 (P2f) | Cap `inspector.render_properties` vec at 256 with a logged warning to prevent unbounded growth | Post-Phase-3 cleanup |
| — | Dynamic footer-hint suppression when `visible_tabs().len() == 1` | Future polish |
| — | "Hide implementation widgets" settings UI toggle visible to the user (currently only `Shift+H` key) | Future UX |
| — | On-device selection via `setSelectionById` (click in fdemon → highlight in Flutter app) | Future feature |
| — | Inline expandable property values (e.g., `Color` → ARGB picker) | Future feature |

## Notes

- Phase 3 ships as a single PR — all 5 tasks merged together. Cf. Phase 1 / Phase 2 cadence.
- The `DetailsContext` struct lives in `fdemon-core` (not `fdemon-app`) because it's a pure value derived from `DiagnosticsNode` tree data, with no app-state knowledge. Keeping it in core means future consumers (e.g. a future MCP-server module) can derive the same context from a tree snapshot.
- The default-construction problem: when `details_context` is on `InspectorState`, it must have a sensible `Default`. The simplest choice is `DetailsContext::default() = DetailsContext { is_flex_layout: false, parent_type: None }`, which results in `visible_tabs() = [Properties]` when no details are open and no properties have been fetched. This is harmless because `visible_tabs()` is only read when `details_open == true`, and `handle_open_details` always overwrites `details_context` before `details_open` is set.
- The naming `compute_details_context` and `parent_of` is intentional — `compute_` connotes a non-cached one-shot derivation; `parent_of` is a tree query verb. Both are pure functions on the tree.
- Reference DevTools source files (read-only) under `tmp/devtools/packages/devtools_app/lib/src/`. Key references:
  - `diagnostics_node.dart:487` — `isFlexLayout` predicate
  - `widget_properties/properties_view.dart:22–131` — `DetailsTable` tab visibility
- The `last_fetched_properties_node_id` cache (Phase 2 task 03 constraint #2) is unaffected by Phase 3: cache hits skip the re-fetch and `details_context` is still recomputed on every `handle_open_details` (cheap — one DFS over the tree).
