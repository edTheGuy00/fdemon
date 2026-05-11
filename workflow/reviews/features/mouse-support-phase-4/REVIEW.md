# Code Review: Mouse Support Phase 4 — Log View & DevTools Clicks

**Review Date:** 2026-05-05
**Branch:** `feat/mouse-support`
**Diff Range:** `1cd3e93..HEAD` (11 commits)
**Scope:** 26 files, 3,335 insertions / 59 deletions
**Task Plan:** `workflow/plans/features/mouse-support/phase-4-log-view-devtools-clicks/`
**Verdict:** ⚠️ **NEEDS WORK**

---

## Summary

Phase 4 extends the per-frame mouse region registry to cover the log view, DevTools sub-tab bar, Inspector tree, Performance frame chart, and Network table+detail-tabs. The architecture is preserved (no layer violations, TEA pattern intact, `MouseRegionGuard` RAII applied throughout) and security posture is sound. However, the implementation accumulates real correctness, maintainability, and test-coverage gaps that should be addressed before phase merge.

The single most important finding is a **critical wrap-mode misalignment bug** in log-view click registration — when the user has scrolled into the middle of a multi-row entry (`wrap_intra_offset > 0`), click regions are positioned in `all_lines` space rather than screen space, so a click on the visible row of entry B can resolve to entry A's click region. There is no test exercising wrap-mode + non-zero offset, which is why this slipped through.

Beyond that critical issue, multiple agents independently flagged: duplicated layout-fetch logic in inspector handlers (the task plan explicitly promised extraction; it did not happen), `render_with_regions` sister functions duplicating `Widget::render` bodies in 3 panels (only `log_view` correctly factors via `render_inner`), and significant test gaps including no wrap-mode click tests, no 80×24 baseline tests for performance/network (the spec-mandated viewport), and no manual smoke test against a live Flutter device.

---

## Verdict Aggregation

| Reviewer | Verdict |
|----------|---------|
| `architecture_enforcer` | ✅ PASS (2 warnings, 1 suggestion) |
| `code_quality_inspector` | ⚠️ NEEDS WORK (2 major, 6 minor, 3 nit) |
| `logic_reasoning_checker` | ⚠️ WARNINGS (1 critical, 4 warnings) |
| `risks_tradeoffs_analyzer` | ⚠️ CONCERNS (12 issues, 3 blocking) |
| `security_reviewer` | ✅ PASS (1 medium, 2 low) |

Multiple agents returned ⚠️ CONCERNS plus a critical correctness bug → overall **NEEDS WORK**.

---

## Critical Findings (Must Fix Before Merge)

### 1. Wrap-mode log row click region misalignment
- **Source:** `logic_reasoning_checker`
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:1449-1485` (in conjunction with 1173, 1212-1219, 1427-1434)
- **Problem:** `RowAction.rel_y` is accumulated in `all_lines` space (starts at 0 for the first entry's first wrapped row), but the rendered `Paragraph` uses `.scroll((wrap_intra_offset, 0))`. When `state.offset` falls in the middle of an entry's wrapped rows, click regions are placed at the wrong screen Y. Concrete repro: two entries A (3 wrapped rows) and B (2 wrapped rows) with `wrap_intra_offset = 2` → clicking visible row of B at screen y=1 resolves to A's click region, returning `ClickLogRow { entry_id: A.id, ... }`.
- **Required Action:** Subtract `wrap_intra_offset` from `rel_y` (saturating, drop rows entirely scrolled off) and clip `height` against the top edge as well as the bottom edge, before registering. Add a regression test asserting click regions correspond to the visible row when `wrap_intra_offset > 0`.

---

## Major Findings (Should Fix)

### 2. Duplicated layout-fetch logic in inspector handlers
- **Source:** `code_quality_inspector` + `architecture_enforcer`
- **Files:** `crates/fdemon-app/src/handler/devtools/inspector.rs:174-204` (`handle_inspector_navigate`) and `:389-417` (`handle_inspector_select_row`)
- **Problem:** Phase 2 of both functions (debounce check, cache-hit check, set `layout_loading`/`pending_node_id`/`layout_last_fetch_time`, dispatch `FetchLayoutData`) is pasted verbatim. The implementation comment even acknowledges this: `// same logic as handle_inspector_navigate`. Task 04's notes explicitly stated this would be "shared with `handle_inspector_navigate` via a small private helper" — that helper was never extracted. Future debounce/cache changes must be made in two places without compiler enforcement.
- **Recommended Action:** Extract a private `fn maybe_fetch_layout(inspector: &mut InspectorState) -> Option<String>` and call from both sites.

### 3. `render_with_regions` duplicates `Widget::render` body in 3 panels
- **Source:** `code_quality_inspector` + `architecture_enforcer`
- **Files:**
  - `crates/fdemon-tui/src/widgets/devtools/mod.rs:386-406`
  - `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:295-388`
  - `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs:461-468`
- **Problem:** Each panel's `render_with_regions` reproduces the full background-fill loop, disconnected/state guards, and layout logic from `Widget::render`. `log_view`, `network/request_table`, and `network/request_details` correctly factor via a private `render_inner`/`render_impl` taking `Option<&mut MouseCtx<'_>>`. The 3 panels above did not adopt this pattern, so `Widget::render` and `render_with_regions` may silently drift when one is updated but not the other. Compounded by the fact that no test asserts both paths produce byte-identical buffers.
- **Recommended Action:** Refactor each affected panel to share an internal `render_impl(area, buf, ctx: Option<&mut MouseCtx<'_>>)`. Add a per-widget test that asserts `Widget::render` and `render_with_regions(... None)` produce identical buffers.

### 4. Lint suppression anti-pattern in inspector handlers
- **Source:** `code_quality_inspector`
- **Files:** `crates/fdemon-app/src/handler/devtools/inspector.rs:169` and `:385`
- **Problem:** `let _ = (old_index, new_index); // suppress unused warning` is used to silence rustc on values that genuinely don't escape the inner scope. Per project standards, suppressing legitimate compiler signals via underscore-binding is an anti-pattern. The values are intermediate steps to compute `selection_changed`; that is the only value that needs to surface.
- **Recommended Action:** Return only `selection_changed` from the inner scope; remove the `let _ = ...` lines.

### 5. Wrap-mode click regions have no test coverage
- **Source:** `logic_reasoning_checker` + `risks_tradeoffs_analyzer`
- **Files:** `crates/fdemon-tui/src/widgets/log_view/tests.rs`
- **Problem:** All click-region tests run in `wrap_mode = false`. The implementor's completion summary acknowledged this gap. The wrap-mode misalignment bug (Critical Finding #1) directly results from this test gap.
- **Recommended Action:** Add tests for `wrap_mode = true` covering: (a) zero offset baseline, (b) non-zero `state.offset` causing `wrap_intra_offset > 0`, (c) bottom-clip on a multi-row entry crossing `content_area.bottom()`.

### 6. 80×24 baseline coverage missing for performance + network
- **Source:** `risks_tradeoffs_analyzer`
- **Files:** `crates/fdemon-tui/src/render/tests.rs`
- **Problem:** Phase 4 success criteria specified registry snapshot tests at 80×24. Performance test was inflated to 80×40 (chart needs ≥7 inner rows) and network test to 160×30 (5 tabs need ≥71 inner cols). Result: there is no test asserting that at the spec-mandated 80×24, the compact-mode no-region path is exercised for these panels.
- **Recommended Action:** Add one test per affected widget at 80×24 asserting `regions.iter().filter(|e| /* perf bar | network row */).count() == 0` (or `regions.is_empty()` if appropriate).

### 7. Manual smoke test deferred
- **Source:** `risks_tradeoffs_analyzer`
- **Problem:** Phase 4 success criteria explicitly required end-to-end click-flow verification on macOS against a live Flutter project. Task 10 documented but did not execute the walkthrough.
- **Recommended Action:** Run the smoke test (log click + double-click → stack trace, DevTools tab click, Inspector row+glyph click, Performance frame click, Network row + detail-tab click). Block phase merge to `main` until performed.

---

## Minor Findings

### 8. `last_log_click` not cleared on session switch (cross-session entry_id collision)
- **Source:** `architecture_enforcer`
- **Files:** `crates/fdemon-app/src/state.rs:1059`, session-switch handlers in `handler/update.rs`
- **Recommended:** Clear `last_log_click` in `SelectSessionByIndex`/`NextSession`/`PreviousSession`/`CloseCurrentSession`, OR add `session_id` to `LogClickStamp` and compare it.

### 9. `handle_inspector_toggle_node` calls `visible_nodes()` twice
- **Source:** `architecture_enforcer` + `logic_reasoning_checker`
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:430-461`
- **Recommended:** Hoist `value_id` and `has_children` into the same borrow scope as the bounds-check before delegating to `handle_inspector_select_row`.

### 10. `MouseAction::Emit(Box<Message>)` allocates per region per frame
- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-app/src/mouse_regions.rs:97`
- **Problem:** ~200 log rows × 20 fps ≈ 4,000 small allocations/sec at peak. Module docstring claims "registry hot path is allocation-free at steady state" — no longer accurate.
- **Recommended:** Update the docstring; benchmark before adding optimization. Defer fix until measured pain.

### 11. Network filter-input gate suppresses sub-tab bar clicks
- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs:43-52`
- **Problem:** When user is typing in the network filter, clicks on the `[i]/[p]/[n]` sub-tab bar are also suppressed — mouse-only user is trapped in the textbox.
- **Recommended:** Carve out the sub-tab bar rect; allow clicks there to escape filter input + switch panel.

### 12. Single click visually inert (discoverability)
- **Source:** `risks_tradeoffs_analyzer`
- **Recommended:** Document in `docs/MOUSE.md`. Future: scroll-to-clicked-row or flash highlight.

### 13. Glyph-after-row push order is order-coupled
- **Source:** `risks_tradeoffs_analyzer` + `architecture_enforcer`
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs:127-149`
- **Problem:** Last-pushed-wins relies on registration order. Future refactor that reorders pushes would silently break glyph priority. Current test `glyph_region_wins_over_row_region_at_glyph_cell` exercises the contract via hit-test, which is good — but the underlying invariant should be made more explicit (e.g., a doc comment on `MouseRegionsBuilder::click` describing the same-z last-pushed-wins rule).

### 14. PLAN.md ↔ implementation drift
- **Source:** `risks_tradeoffs_analyzer`
- **Files:** `workflow/plans/features/mouse-support/PLAN.md` vs `crates/fdemon-app/src/handler/log_view.rs`
- **Drift items:**
  - PLAN specified `EmitWithCoord` for log-view rows; implementation uses per-row `Emit`.
  - PLAN specified "within 400ms, within 1 cell of previous click" for double-click; implementation drops the position constraint and uses `entry_id` only.
- **Recommended:** Update PLAN.md or add inline notes documenting the deviations.

### 15. `>= 12` instead of `== 12` in log-view region test
- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-tui/src/render/tests.rs:377`
- **Recommended:** Tighten to `assert_eq!(click_log_rows, 12, ...)`. Companion tests for inspector/perf/network all use exact equality.

### 16. Missing middle-click test in DevTools
- **Source:** `code_quality_inspector` (also flagged at validation time)
- **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs` `press_tests`
- **Recommended:** Add `middle_click_on_recorded_region_returns_middle_action` test.

### 17. `DOUBLE_CLICK_WINDOW` boundary semantics inconsistency
- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-app/src/handler/log_view.rs:15` (constant) vs `:94` (comparison)
- **Recommended:** Clarify whether 400ms is inclusive (`<=`) or exclusive (`<`); update either the comment or operator.

### 18. Glyph X coordinate silent-discard at extreme depths
- **Source:** `security_reviewer` (medium)
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs:138-149`
- **Problem:** `saturating_mul` saturates to `u16::MAX` at extreme depths; `glyph_x < tree_inner.right()` guard then silently skips registration. Glyph click on deep nodes produces row-select instead of toggle.
- **Recommended:** Use `checked_add` with a debug log; document the silent fallback.

### 19. `MouseRegionGuard::deref/deref_mut` use undocumented `expect()`
- **Source:** `security_reviewer` (low)
- **File:** `crates/fdemon-app/src/mouse_regions.rs:311-325`
- **Recommended:** Add `// SAFETY:` comments explaining why `regions` is always `Some` from construction until `drop()`.

### 20. Stale registry pre-size constant + docstring (32 entries)
- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-app/src/mouse_regions.rs:146`
- **Recommended:** Update `MouseRegions::with_capacity()` docstring to reflect Phase 4 reality (registry now grows to viewport height).

### 21. `hit_test` O(N) contract undocumented
- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-app/src/mouse_regions.rs:162`
- **Recommended:** Add docstring noting O(N) per click; flag for revisit if Phase 5 expands the surface.

### 22. `MouseAction::as_emit()` was added out-of-scope
- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-app/src/mouse_regions.rs`
- **Note:** Additive helper added by task 07; benign but signals scope-creep tolerance.

### 23. `_frame_index` parameter never used in handler
- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-app/src/handler/log_view.rs:89`
- **Recommended:** Convert the rationale into a `// TODO(phase-5): use frame_index to open link for stack-frame double-click` comment, or destructure `frame_index` only at the `update()` callsite.

### 24. `LABEL_COL_WIDTH` const placement between `use` blocks
- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs:14-15`

### 25. `matches!(button, MouseButton::Right)` could be direct equality
- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs:38-40`

### 26. Triple `if mouse_ctx.is_some()` guards in log_view::render_inner
- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:1241, 1296, 1348`
- **Recommended:** Extract a closure to centralize the row-action push.

---

## Architecture Compliance

✅ All layer boundaries respected. `fdemon-app` has no `ratatui` dependency; `fdemon-tui` depends on `fdemon-app` and `fdemon-core` only. The TEA pattern is preserved: all 4 new `Message` variants route through `handler::update`, handlers remain pure, the documented `Cell<MouseRegions>` view-write exception is the only TEA carve-out and is correctly RAII-guarded by `MouseRegionGuard`.

## Security Posture

✅ Low-risk UI plumbing. All click coordinates pass through the registry (no raw input reaches business logic), all index-based operations have explicit bounds checks, `saturating_duration_since` defends against monotonic-clock edge cases, and no new external input surfaces are introduced. Two minor hardening notes (#18, #19) are defense-in-depth.

## Documentation Freshness

⚠️ Stale items:
- `docs/MOUSE.md` (if it exists; otherwise candidate for creation) should document: per-row entry registration model, double-click semantics with no position constraint, single-click-is-inert deliberate choice, filter-input click-suppression in Network panel.
- `crates/fdemon-app/src/mouse_regions.rs` module docstring claims "allocation-free at steady state" — no longer accurate.
- `MouseRegions::with_capacity()` docstring says "starts at 32" — Phase 4 widens the working set.
- PLAN.md drift on `EmitWithCoord` / double-click position constraint.

`docs/ARCHITECTURE.md` already covers the `MouseRegionGuard` RAII type from Phase 3.5; no new module-level additions in Phase 4 require ARCHITECTURE.md updates.

---

## Quality Metrics

| Metric | Score | Notes |
|--------|-------|-------|
| Architecture | 5/5 | Layer boundaries clean; TEA preserved; RAII applied |
| Security | 5/5 | No new attack surface; defensive arithmetic throughout |
| Logic Correctness | 3/5 | Wrap-mode click misalignment is a real defect |
| Code Quality | 3/5 | Duplicated logic in 4 places; lint suppression anti-pattern |
| Test Coverage | 3/5 | 15 new tests cover happy paths; wrap-mode + 80×24 + manual smoke missing |
| Maintainability | 3/5 | Sister-function pattern doubles render API surface; will grow |

---

## Recommendation

**Block merge to `main` until critical and major findings are resolved.** Specifically:

1. Fix wrap-mode click misalignment (Critical #1) + add regression test (Major #5)
2. Extract `maybe_fetch_layout` helper (Major #2)
3. Refactor 3 sister functions to share `render_inner` (Major #3)
4. Remove `let _ = (old_index, new_index)` suppression (Major #4)
5. Add 80×24 baseline tests for perf + network (Major #6)
6. Run manual smoke test on live Flutter session (Major #7)

Minor findings can be tracked as follow-up tickets and addressed before Phase 5 starts.

See `ACTION_ITEMS.md` for a checklist-formatted version.
