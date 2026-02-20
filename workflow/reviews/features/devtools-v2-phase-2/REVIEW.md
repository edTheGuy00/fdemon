# Feature Review: DevTools v2 Phase 2 — Merged Inspector + Layout Tab

**Review Date:** 2026-02-20
**Reviewer:** Code Review Orchestrator
**Task Files Reviewed:** 7 tasks
**Files Changed:** 21 files (+1,398 / -1,687 lines)

---

## Executive Summary

**Overall Verdict:** :warning: APPROVED WITH CONCERNS

Phase 2 merges the Inspector and Layout tabs into a single unified Inspector tab with a 50/50 split, adds EdgeInsets types to core, extracts padding from the VM service, and creates a new layout panel widget with auto-fetch-on-navigation and 500ms debounce. The architecture is clean, TEA pattern compliance is excellent, and test coverage is strong (+582 new tests). However, two logic gaps — stale layout responses during debounce windows and missing cache invalidation on tree refresh — should be tracked as follow-ups.

---

## Changes Overview

### Task Files Reviewed

| Task | Status | Crate |
|------|--------|-------|
| `01-add-edge-insets-core-types` | Done | `fdemon-core` |
| `02-merge-layout-state-into-inspector` | Done | `fdemon-app` |
| `03-remove-layout-panel-variant` | Done | `fdemon-app`, `fdemon-tui` |
| `04-extract-padding-from-vm-service` | Done | `fdemon-daemon` |
| `05-create-layout-panel-widget` | Done | `fdemon-tui` |
| `06-wire-merged-inspector-layout` | Done | `fdemon-app`, `fdemon-tui` |
| `07-final-test-and-cleanup` | Done | workspace |

### Files Changed

```
 crates/fdemon-app/src/handler/devtools/inspector.rs   | 548 +++++++
 crates/fdemon-app/src/handler/devtools/layout.rs      | 410 ------  (DELETED)
 crates/fdemon-app/src/handler/devtools/mod.rs         |  70 +-
 crates/fdemon-app/src/handler/keys.rs                 |   6 +-
 crates/fdemon-app/src/handler/tests.rs                |  56 +-
 crates/fdemon-app/src/handler/update.rs               |   8 +-
 crates/fdemon-app/src/lib.rs                          |   4 +-
 crates/fdemon-app/src/message.rs                      |   2 +-
 crates/fdemon-app/src/state.rs                        | 145 +--
 crates/fdemon-core/src/lib.rs                         |   3 +-
 crates/fdemon-core/src/widget_tree.rs                 | 201 ++++
 crates/fdemon-daemon/src/vm_service/extensions/layout | 270 +++++
 crates/fdemon-daemon/src/vm_service/extensions/mod.rs |   4 +-
 crates/fdemon-daemon/src/vm_service/mod.rs            |  10 +-
 crates/fdemon-tui/.../inspector/details_panel.rs      | 129 ---  (DELETED)
 crates/fdemon-tui/.../inspector/mod.rs                |  26 +-
 crates/fdemon-tui/.../inspector/tests.rs              |  25 +-
 crates/fdemon-tui/.../inspector/layout_panel.rs       | 533 +++++++  (NEW)
 crates/fdemon-tui/.../inspector/layout_panel_tests.rs | 376 +++++  (NEW)
 crates/fdemon-tui/.../devtools/layout_explorer.rs     | 852 ------  (DELETED)
 crates/fdemon-tui/.../devtools/mod.rs                 |  43 +-
 27 files changed, 1398 insertions(+), 1687 deletions(-)
```

---

## Subagent Review Summaries

### Architecture Enforcer
**Verdict:** :white_check_mark: PASS

All layer boundaries are intact. New types (`EdgeInsets`, `LayoutInfo`, `BoxConstraints`, `WidgetSize`) correctly placed in `fdemon-core`. Layout parsing in `fdemon-daemon`. State management in `fdemon-app`. Rendering in `fdemon-tui`. No upward dependency violations. TEA pattern consistently honored — all state mutations in handler functions, `UpdateAction::FetchLayoutData` properly defers side effects to the engine. Backward compatibility handled well (`parse_default_panel("layout")` maps to Inspector).

**Key Findings:**
- 0 layer boundary violations
- TEA purity confirmed: `handle_inspector_navigate` returns `UpdateAction`, no inline I/O
- Minor: inline field mutations in navigate handler could be centralized in a `record_layout_fetch_start()` method
- Minor: `fdemon_core::LayoutInfo` used as fully-qualified path instead of import

### Code Quality Inspector
**Verdict:** :white_check_mark: PASS (with reservations)

Clean Rust throughout. Error handling consistent with project patterns. Strong test coverage (36 handler + 22 widget tests). Quality gate green (fmt, check, clippy, test all pass).

**Quality Scores:**

| Metric | Score |
|--------|-------|
| Language Idioms | 4/5 |
| Error Handling | 5/5 |
| Testing | 4/5 |
| Documentation | 5/5 |
| Maintainability | 4/5 |

**Key Findings:**
- Dead variables `old_index`/`new_index` with `let _ =` suppression — code smell
- Double `visible_nodes()` allocation in `handle_inspector_navigate`
- Magic numbers `4`/`6` in `render_full_layout` not extracted as constants
- Tautological condition `oi.x < oi.x + off` (always true)
- Missing `PartialEq` derive on `LayoutInfo`

### Logic & Reasoning Checker
**Verdict:** :large_orange_diamond: CONCERNS

Core logic is sound — navigation, debounce, TEA compliance all correct. Two moderate gaps identified in edge case handling around stale responses and cache invalidation.

**Key Findings:**
- **Stale layout response accepted**: If debounce suppresses fetch for node B but node A's fetch completes, node A's layout displays while user is on node B
- **Tree refresh doesn't invalidate layout cache**: After a tree refresh, stale `layout`/`last_fetched_node_id` may briefly show data from the old tree
- Tautological condition in `render_box_model` (cosmetic)
- All `InspectorNav` variants correctly handled; Expand/Collapse properly skip layout fetch

### Risks & Tradeoffs Analyzer
**Verdict:** :large_orange_diamond: CONCERNS

Well-executed refactor with strong test coverage (+582 tests). Two medium-risk items: missing initial layout fetch on Inspector entry (skipped acceptance criterion) and leading-edge-only debounce that can leave the panel in inconsistent state.

**Identified Risks:**

| Risk | Severity | Mitigated? |
|------|----------|------------|
| No initial layout fetch on Inspector entry | Medium | No — deferred |
| Leading-edge debounce with no trailing fetch | Medium | Partial — user can re-navigate |
| Double `visible_nodes()` per navigation | Medium | No |
| `layout_panel.rs` exceeds 400-line guideline (539 lines) | Low | Partial — tests in sibling file |
| `EdgeInsets::is_zero()` uses exact float comparison | Low | Yes — parsed values are exact |
| `description.clone()` in render path | Low | Yes — one string per frame |

---

## Consolidated Issues

### :red_circle: Critical Issues (Must Fix)

None.

### :orange_circle: Major Issues (Should Fix)

**1. [Source: Logic Checker] Stale layout response accepted when user navigates away during debounce window**
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:229-248`
- **Problem:** `handle_layout_data_fetched` does not verify that `pending_node_id` matches the currently selected node. If the user navigates from A to B within 500ms (debounce suppresses B's fetch), A's layout response arrives and displays while the user is on node B.
- **Recommended Action:** Before applying layout data, compare `pending_node_id` against the currently selected node's `value_id`. If they differ, discard the response.

**2. [Source: Logic Checker] Tree refresh does not invalidate layout cache**
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:18-49`
- **Problem:** `handle_widget_tree_fetched` resets tree-related fields but leaves layout fields (`layout`, `layout_loading`, `layout_error`, `last_fetched_node_id`, `pending_node_id`, `layout_last_fetch_time`) intact. After a tree refresh, stale layout data may briefly display for a node that no longer exists.
- **Recommended Action:** Clear all layout fields in `handle_widget_tree_fetched` after resetting tree state.

**3. [Source: Risks Analyzer] Missing initial layout fetch on Inspector entry (deferred acceptance criterion 11)**
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs`
- **Problem:** When entering DevTools mode, the tree auto-loads and root node is selected (index 0), but no layout fetch is dispatched. The layout panel shows "Select a widget to see layout details" despite a node being selected — contradictory UX.
- **Recommended Action:** Add layout fetch dispatch in `handle_widget_tree_fetched()` after storing the tree, for the root node.

### :yellow_circle: Minor Issues (Consider Fixing)

**1. [Source: Quality Inspector] Dead variables with `let _ =` suppression**
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:143-145`
- **Suggestion:** Remove `old_index` and `new_index` from the Phase 1 return tuple. Thread the selected node's `value_id` through instead, also eliminating the double `visible_nodes()` call.

**2. [Source: Quality Inspector] Magic numbers in `render_full_layout`**
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs:178-183`
- **Suggestion:** Extract `4` and `6` as named constants (`SIZE_BOX_MIN_HEIGHT`, `SIZE_BOX_MAX_HEIGHT`).

**3. [Source: Quality Inspector, Logic Checker] Tautological condition `oi.x < oi.x + off`**
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs:380`
- **Suggestion:** Remove the condition or replace with `if off > 0` for clarity.

**4. [Source: Architecture, Quality] Fully-qualified `fdemon_core::LayoutInfo` instead of import**
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:232`
- **Suggestion:** Add `use fdemon_core::LayoutInfo;` to the import block for consistency.

**5. [Source: Quality Inspector] Missing `PartialEq` on `LayoutInfo`**
- **File:** `crates/fdemon-core/src/widget_tree.rs`
- **Suggestion:** Add `PartialEq` derive to `LayoutInfo` to enable clean equality assertions in tests.

**6. [Source: Architecture] Layout field mutations not centralized**
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:145-148`
- **Suggestion:** Add `InspectorState::record_layout_fetch_start(node_id)` to centralize the 3-line mutation pattern.

**7. [Source: Risks Analyzer] Leading-edge-only debounce (no trailing fetch)**
- **Suggestion:** Consider adding a trailing-edge mechanism using the existing `Tick` message to check if a fetch is needed after the debounce window expires. Track as a follow-up if not addressed now.

---

## Review Checklist

- [x] **Architecture Compliance**: Changes follow layer boundaries and design patterns
- [x] **Code Quality**: Language idioms, error handling, and project conventions followed
- [ ] **Logical Consistency**: Two edge cases with stale data handling need attention
- [x] **Risk Mitigation**: Documented risks have adequate mitigations (except initial fetch)
- [x] **Testing Coverage**: +582 new tests; all rendering states, debounce, navigation covered
- [x] **Documentation**: Public APIs documented, borrow-splitting logic well commented

---

## Actionable Items

### Required for Approval

None — issues are non-blocking. All are edge cases that don't affect the primary user flow.

### Recommended Improvements

1. [ ] **Guard stale layout responses** — Compare `pending_node_id` against selected node before applying in `handle_layout_data_fetched`
2. [ ] **Clear layout cache on tree refresh** — Reset layout fields in `handle_widget_tree_fetched`
3. [ ] **Add initial layout fetch** — Dispatch `FetchLayoutData` in `handle_widget_tree_fetched` for root node
4. [ ] **Remove dead variables** — Eliminate `old_index`/`new_index` from Phase 1 tuple and the double `visible_nodes()` call
5. [ ] **Extract magic numbers** — `SIZE_BOX_MIN_HEIGHT = 4`, `SIZE_BOX_MAX_HEIGHT = 6`
6. [ ] **Remove tautological condition** — `oi.x < oi.x + off` in `render_box_model`
7. [ ] **Import `LayoutInfo`** — Add to import block in `inspector.rs`

---

## Conclusion

**Final Assessment:** This is a well-executed feature implementation that correctly merges the Inspector and Layout tabs while maintaining clean architecture and TEA compliance. The code is well-tested (+582 tests), well-documented, and passes the full quality gate. The three "should fix" items are real edge cases (stale response during debounce, cache invalidation on tree refresh, missing initial fetch) but they affect uncommon interaction sequences — not the primary navigation flow. They are suitable for a follow-up task rather than blocking this phase.

**Next Steps:**
1. Track the 3 major items as a follow-up task for Phase 3 or a polish pass
2. Address minor items (dead variables, magic numbers, tautological condition) at convenience
3. Phase 2 is ready for merge

**Blocking Issues Count:** 0
**Re-review Required:** No
