# Feature Review: DevTools Inspector Parity — Phase 1

**Review Date:** 2026-05-18
**Reviewer:** Code Review Orchestrator (5 agents in parallel)
**Diff Base:** `a3ea91c..e01639f` on `feat/devtools-inspector-parity`
**Task Files Reviewed:** 11 tasks (01–11) under `workflow/plans/features/devtools-inspector-parity/phase-1/tasks/`
**Files Changed:** 22 source/doc files (+4,130 / -147)

---

## Executive Summary

**Overall Verdict:** ⚠️ **NEEDS WORK**

Phase 1 lands the scaffolding for DevTools-style inspector parity: row builder + chain-folding algorithm in `fdemon-core`, ten new `InspectorState` fields, tabbed Details view scaffold, mode-switched rendering, persisted hide-impl toggle, tiered Esc, and new key bindings — all built behind a green workspace test suite (~5,500 tests passing). Architecture, layer boundaries, and TEA contracts are respected.

However, two findings are blocking-for-Phase-2:

1. **The flagship "BlocProvider chain" demo is unreachable.** `expanded_groups` is never mutated by any user action — `Right`, glyph click, and `InspectorNav::Expand` all write to `expanded`, not `expanded_groups`. A folded chain leader has no expand path except toggling Shift+H globally.
2. **Details state leaks across `r` refresh and hot-restart.** `handle_widget_tree_fetched` and `SessionRestartCompleted` clear `expanded` and selection but never touch `details_open`, `details_node_id`, `details_tab`, or `expanded_groups`. Stale `details_node_id` points at a freed Dart object after restart.

A third correctness issue (branch-tick `branch_x = 0` sentinel + a separate `open_ticks.push(depth)` off-by-one) materializes as visibly broken guideline/tick rendering under specific conditions. Plus several MAJOR cleanups deferred from earlier tasks were never completed (`_visible` placeholder param, `get_selected_value_id` duplicate, stale `#[allow(dead_code)]`).

See `ACTION_ITEMS.md` for the prioritized fix list.

---

## Changes Overview

### Task Status

All 11 tasks merged to `feat/devtools-inspector-parity`. Final quality gate (`fmt + check + test + clippy`) is green.

### Files Changed (summary)

```
crates/fdemon-app/src/config/settings.rs                |   8 +
crates/fdemon-app/src/config/types.rs                   |  41 +
crates/fdemon-app/src/handler/devtools/inspector.rs     | 448 +++-
crates/fdemon-app/src/handler/devtools/mod.rs           | 108 ++-
crates/fdemon-app/src/handler/keys.rs                   | 218 ++-
crates/fdemon-app/src/handler/update.rs                 |  17 +-
crates/fdemon-app/src/message.rs                        |  42 +
crates/fdemon-app/src/state.rs                          | 542 ++++-
crates/fdemon-core/src/lib.rs                           |   5 +-
crates/fdemon-core/src/widget_tree.rs                   | 969 +++++++++++
crates/fdemon-tui/src/theme/palette.rs                  |  10 +
crates/fdemon-tui/src/widgets/devtools/inspector/details/{flex_explorer_tab,mod,properties_tab,render_object_tab}.rs (NEW)
crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs |  12 +-
crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs |  24 +-
crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs        | 513 +++
crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs   | 391 +++-
crates/fdemon-tui/src/widgets/devtools/mod.rs           |  73 +-
docs/{ARCHITECTURE,KEYBINDINGS}.md                      |  50 ++/-
22 files changed, 4130 insertions(+), 147 deletions(-)
```

---

## Subagent Review Summaries

### Architecture Enforcer
**Verdict:** ⚠️ CONCERNS

Layer boundaries clean (`fdemon-core` zero deps, `fdemon-tui → fdemon-app → fdemon-daemon → fdemon-core`). TEA pattern respected — no state mutation in render, all events routed through `Message`. New `details/` submodule visibility scoped via `pub(in crate::widgets::devtools::inspector)`.

**Key Findings:**
- `get_selected_value_id` (`handler/devtools/inspector.rs:208`) duplicates the newly-added `InspectorState::selected_value_id()` — task 02 flagged it for task 05 migration; task 05 shipped without doing it.
- `_visible` parameter (`tree_panel.rs:154`) still in signature after task 09 — task 09 closed without removal, no follow-up filed.
- `widget_tree.rs` reached 1,650 lines (>500 standard), follow-up task never created.
- Task 11 completion summary still says "Not Started" despite docs being updated.

### Code Quality Inspector
**Verdict:** ⚠️ NEEDS WORK

**Quality Scores:**
| Metric | Score |
|--------|-------|
| Language Idioms | ⭐⭐⭐⭐ (group.clone, refs re-collect, sentinel bug) |
| Error Handling | ⭐⭐⭐⭐⭐ (no unjustified unwraps, save_settings failure is non-fatal/logged) |
| Testing | ⭐⭐⭐⭐ (340+ new tests, breadth strong; missing leader-click & open-already-open edge cases; 6× duplicated `collect_buf_text`) |
| Documentation | ⭐⭐⭐⭐ (`///` on pubs; stale `#[allow(dead_code)]`; two single-slash doc lines) |
| Maintainability | ⭐⭐⭐ (1650-line file, stale params/duplicates, deferred cleanups not tracked) |

**Major Issues:**
- `branch_x = 0` sentinel in `tree_panel.rs:226-238` collides with legitimate `tree_inner.x == 0` — silently suppresses tick rendering in borderless contexts.
- `get_selected_value_id` duplicate (also flagged by architecture_enforcer).
- Stale `#[allow(dead_code)]` on every function in `details/{render_object_tab,flex_explorer_tab,properties_tab}.rs` and constants in `properties_tab.rs` after task 09 wired them.

### Logic & Reasoning Checker
**Verdict:** ⚠️ CONCERNS (with 2 CRITICAL findings)

**Critical (C1) — Stale details/chain state on refresh & hot-restart:** `handle_widget_tree_fetched` (`inspector.rs:21-78`) clears `selected_index/expanded/layout/last_fetched_node_id` but not `details_open`, `details_node_id`, `details_tab`, `expanded_groups`. `SessionRestartCompleted` (`update.rs:222-244`) only flips `has_ever_rendered_tree`. After `r` or hot-restart the user sees a Details panel pointing at a stale node_id. Zero tests cover this path.

**Critical (C2) — Guideline `│` off-by-one:** `open_ticks.push(depth)` (`widget_tree.rs:419-421`) plus renderer iterating `0..row.depth` causes the guideline column for descendants of a non-last node at depth N to be drawn at `glyph_col(N)` (overwritten by the branch tick) instead of `glyph_col(N-1)`. Existing tests assert `contains('│')` without column checks so the bug passes them.

**Warnings:**
- W1: `count_visible_chain_subordinates` honors `expanded` set, but `emit_chain_members` does not. Badge promises "+1 more", unfold reveals three.
- W3: `handle_open_details` resets `details_tab = Properties` but `handle_close_details` doc comment claims it preserves the value.
- W4: Narrow terminal + details_open → details panel silently not rendered; tree mouse regions also suppressed → user trapped.
- W5: `Message::ExitDevToolsMode` is now misleading (routed through tiered-Esc helper).
- W6: same `branch_x = 0` sentinel issue as code quality.

**Notes:** `walk_node`'s `is_member` parameter is unreachable dead code; KEYBINDINGS.md claims Up/Down are no-ops while handler doesn't actually block them at the keys layer (handler swallows the navigation but the comment in `keys.rs:633-638` is inverted).

### Risks & Tradeoffs Analyzer
**Verdict:** ⚠️ NEEDS WORK (1 BLOCKING for Phase 2)

**Identified Risks:**

| Risk | Severity | Mitigated? |
|------|----------|------------|
| `expanded_groups` never wired to user input — flagship BlocProvider chain unreachable | HIGH | NO — blocks Phase 1 acceptance |
| `inspector_rows()` rebuilt 3–4× per frame; `count_visible_chain_subordinates` is O(n·k) per chain | HIGH | NO — visible on deep chains |
| `_visible` placeholder param survived Phase 1 | HIGH | NO — task 09 should have removed it |
| Synchronous `save_settings()` on every Shift+H | HIGH | NO — risk of TUI stall on slow FS |
| `count_visible_chain_subordinates` is `pub` (locks in algorithmic shape as SemVer) | MEDIUM | NO — demote to `pub(crate)` before Phase 2 |
| Mouse-click on `LeaderCollapsed` glyph mutates wrong `expanded` set silently | MEDIUM | NO |
| `details_tab` reset semantics undefined for Phase 3 conditional visibility | MEDIUM | NO — decide now |
| Branch-tick `branch_x = 0` sentinel | MEDIUM | NO |
| Per-tab `render_centered_text` helper duplicated 2× | LOW | — Phase 2 will delete |

**Critical Finding (verbatim):** "The flagship demo scenario (BlocProvider chain) doesn't work end-to-end. Folding renders correctly, expansion path is unwired. Cannot ship Phase 1 as 'DevTools parity' without this."

### Security Reviewer
**Verdict:** ✅ PASS (with hardening recommendations)

This is a local dev tool; full threat model is the developer's own machine. No critical or high-severity vulnerabilities.

**Security Findings:**

| Finding | Category | Severity |
|---------|----------|----------|
| No ANSI/control-char sanitisation on `DiagnosticsNode.description` / `creation_location.file` before terminal render | Injection (terminal) | MEDIUM |
| `save_settings()` uses fixed temp filename `.config.toml.tmp` (concurrent-write race / readable intermediate) | TOCTOU / Info disclosure | MEDIUM |
| `walk_node` and `visible_node_count` recursion uncapped (stack exhaustion on pathological VM Service trees) | Resource exhaustion | MEDIUM |
| `save_settings()` logs absolute config path at `info!` every toggle | Info disclosure (telemetry future) | LOW |
| Synchronous I/O in handler (TEA purity drift) | Architecture | LOW |

`strip_ansi_codes()` already exists in `fdemon-core/ansi.rs` and is used by the daemon layer — apply it at `DiagnosticsNode` deserialize boundary.

### Documentation Freshness
**Status:** ⚠️ Updates needed

`docs/ARCHITECTURE.md` and `docs/KEYBINDINGS.md` were updated by task 11 — but:
- KEYBINDINGS.md Details-mode table claims `Up/Down/j/k` are **no-op**. Implementation does not block these at the keys layer; the handler returns early, but the `keys.rs` comment at line 633-638 says they "work in both modes." Docs and the source comment disagree.
- Task 11 completion summary block still says `**Status:** Not Started` despite the docs being modified.

---

## Consolidated Findings (deduplicated, severity-ordered)

### 🔴 CRITICAL (must fix before Phase 1 is accepted)

| # | Finding | File(s) | Source |
|---|---------|---------|--------|
| C1 | `expanded_groups` never wired — chain leader cannot be expanded by Right / glyph click / Expand handler | `handler/devtools/inspector.rs:154-160, 459-465` | risks_tradeoffs, logic_reasoning |
| C2 | Stale `details_open`/`details_node_id`/`details_tab`/`expanded_groups` after `r` refresh and hot-restart | `handler/devtools/inspector.rs:21-78`, `handler/update.rs:222-244` | logic_reasoning |
| C3 | Branch-tick `branch_x = 0` sentinel collides with valid x=0 — silently suppresses ticks | `tree_panel.rs:226-238` | code_quality, logic_reasoning, risks_tradeoffs |
| C4 | Guideline `│` off-by-one: `open_ticks.push(depth)` vs renderer math draws line at column where branch tick already lives | `widget_tree.rs:419-421`, `tree_panel.rs:205-218` | logic_reasoning |

### 🟠 MAJOR (should fix before merge)

| # | Finding | File(s) | Source |
|---|---------|---------|--------|
| M1 | `get_selected_value_id` private duplicate of `InspectorState::selected_value_id()` (3 call sites) | `handler/devtools/inspector.rs:65, 208, 225, 284` | architecture, code_quality |
| M2 | `_visible` placeholder param still in `render_tree_panel_inner` signature after task 09 | `tree_panel.rs:154`, `inspector/mod.rs` | architecture, code_quality, risks_tradeoffs |
| M3 | `save_settings()` runs synchronously on every Shift+H — TUI-loop stall risk | `handler/devtools/inspector.rs:585` | risks_tradeoffs, security |
| M4 | Chain count badge mismatch: `count_visible_chain_subordinates` honors `expanded`, `emit_chain_members` does not | `widget_tree.rs:475-541, 586-627` | logic_reasoning |
| M5 | `inspector_rows()` rebuilt 3–4× per frame; per-row `count_visible_chain_subordinates` is O(n·k) | `inspector/mod.rs:156`, `tree_panel.rs:175`, `details/mod.rs:95` | risks_tradeoffs |
| M6 | Stale `#[allow(dead_code)]` annotations across `details/{properties,render_object,flex_explorer}_tab.rs` after task 09 wired them | `details/properties_tab.rs:24/29/40/82`, `render_object_tab.rs:13/18`, `flex_explorer_tab.rs:13/18` | code_quality, risks_tradeoffs |
| M7 | No ANSI/control-char sanitisation on VM Service strings rendered to terminal | `tree_panel.rs:295-328`, `layout_panel.rs:137-158` | security |
| M8 | Mouse click on `LeaderCollapsed` glyph mutates wrong set (`expanded` instead of `expanded_groups`) silently | `handler/devtools/inspector.rs:433-468` | risks_tradeoffs |

### 🟡 MINOR (fix soon)

| # | Finding | File(s) | Source |
|---|---------|---------|--------|
| m1 | `widget_tree.rs` is 1,650 lines (>500 standard); no follow-up split task filed | `widget_tree.rs` | architecture, code_quality |
| m2 | `details_tab` reset on open contradicts `handle_close_details` doc comment | `inspector.rs:492, 530-531` | logic_reasoning |
| m3 | Narrow-terminal details mode silently invisible (panel suppressed, mouse regions also off) | `inspector/mod.rs:212-223` | logic_reasoning |
| m4 | `Message::ExitDevToolsMode` variant name misleading now that it routes through tiered Esc | `message.rs`, `update.rs:1920` | logic_reasoning |
| m5 | `walk_node` `is_member` parameter is dead code; `RowGroup::Member` arm unreachable | `widget_tree.rs:361-365` | logic_reasoning |
| m6 | KEYBINDINGS.md says Up/Down are no-ops in details mode; `keys.rs:633-638` comment says they work in both modes; handler swallows them | `keys.rs:633-638`, `docs/KEYBINDINGS.md` | architecture, logic_reasoning |
| m7 | Settings persistence write uses fixed temp filename `.config.toml.tmp` (TOCTOU / readable intermediate) | `config/settings.rs:536` | security |
| m8 | `count_visible_chain_subordinates` is `pub`; should be `pub(crate)` to avoid SemVer lock-in | `fdemon-core/lib.rs:96-100` | risks_tradeoffs |
| m9 | `walk_node` / `visible_node_count` recursion has no explicit depth cap | `widget_tree.rs:133-142, 352-466` | security |
| m10 | `InspectorRow.group: group.clone()` avoidable move-by-clone | `widget_tree.rs:397, 403` | code_quality |
| m11 | `details/mod.rs:95-98` re-collects `visible` into `refs` for no semantic gain | `details/mod.rs:95-98` | code_quality |
| m12 | `_tab` named like unused binding but actively used | `details/mod.rs:136` | code_quality |
| m13 | Six identical `collect_buf_text` test helpers across `details/*`, `tests.rs`, `layout_panel_tests.rs` | various tests | code_quality |
| m14 | Two single-slash `/` doc comment lines (rustdoc-invisible) | `properties_tab.rs:28`, `tree_panel.rs:152` | code_quality |

### 🔵 NITPICK

| # | Finding | Source |
|---|---------|--------|
| n1 | Task 11 completion summary still says "Not Started" even though docs are updated | architecture |
| n2 | `save_settings()` logs absolute path at `info!` level (telemetry future risk) | security |
| n3 | `unwrap_or("") + !is_empty()` cleaner as `is_some_and(...)` | code_quality |
| n4 | Test missing for `handle_open_details_is_no_op_when_already_open` early-return path | code_quality |
| n5 | `render_centered_text` helper duplicated in render-object + flex-explorer stub tabs | risks_tradeoffs |

---

## Verdict Logic

| Agent | Verdict |
|-------|---------|
| Architecture Enforcer | ⚠️ CONCERNS |
| Code Quality Inspector | ⚠️ NEEDS WORK |
| Logic & Reasoning Checker | ⚠️ CONCERNS (with 2 CRITICAL) |
| Risks & Tradeoffs Analyzer | ⚠️ NEEDS WORK (1 BLOCKING) |
| Security Reviewer | ✅ PASS |

Per the matrix:
- ≥2 agents flagged CONCERNS/NEEDS-WORK
- Risk analyzer called the unwired `expanded_groups` blocking-for-Phase-2
- Logic checker flagged 2 CRITICAL correctness issues

**Final Verdict: ⚠️ NEEDS WORK** — Phase 1 cannot be claimed complete without resolving C1–C4 and ideally M1–M3 before Phase 2 builds on top.

---

## Recommendation

1. **Treat C1 (expanded_groups wiring) as blocking.** Until `InspectorNav::Expand` / `Collapse` and `handle_inspector_toggle_node` branch on `RowGroup` and mutate the right set, the flagship demo from the parent PLAN does not work. Add at minimum 3 wired tests.
2. **Fix C2 (state reset on refresh/restart) before any user observes stale details panels.**
3. **Fix C3/C4 (branch-tick + guideline rendering) — both are visible regressions on common tree shapes.**
4. **Address M1–M3 in the same PR** — these are debt items the plan tracked as "Phase 1 cleanup" that didn't make it: `_visible` removal, `get_selected_value_id` deletion, async settings persistence.
5. **File follow-up tasks for m1 (widget_tree.rs split), m6 (Up/Down doc drift), m7 (temp filename), and m8 (pub→pub(crate))** so they don't get lost between phases.
6. **Re-validate by re-running the same 5 reviewer agents on the fix branch** before merging.

Action items detailed in `ACTION_ITEMS.md`.
