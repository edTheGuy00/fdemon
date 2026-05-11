# Action Items: Mouse Support Phase 4

**Review Date:** 2026-05-05
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 1 critical + 6 major

---

## Critical Issues (Must Fix)

### 1. Wrap-mode log row click region misalignment
- **Source:** `logic_reasoning_checker`
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs`
- **Lines:** 1449–1485 (registration loop), 1173 (`row_actions` init), 1212–1219 / 1427–1434 (`rel_y_cursor` accumulation)
- **Problem:** When `state.offset` lands inside a multi-row entry, `wrap_intra_offset > 0` is passed to `Paragraph::scroll`, but `RowAction.rel_y` remains in `all_lines` space. Click regions land at wrong screen rows; clicking visible row of entry B can resolve to entry A's region.
- **Required Action:**
  - Subtract `wrap_intra_offset` from each `RowAction.rel_y` before registering (saturating; skip rows fully scrolled off).
  - Clip `height` against the top edge for partially-scrolled rows in addition to the existing bottom-edge clip.
- **Acceptance:** New regression test in `crates/fdemon-tui/src/widgets/log_view/tests.rs` exercising `wrap_mode = true` + `state.offset` mid-entry, asserting click region order matches visible row order.

---

## Major Issues (Should Fix Before Merge)

### 2. Duplicated layout-fetch logic in inspector handlers
- **Source:** `code_quality_inspector`, `architecture_enforcer`
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs`
- **Lines:** 174–204 (`handle_inspector_navigate`) and 389–417 (`handle_inspector_select_row`)
- **Required Action:** Extract a private `fn maybe_fetch_layout(inspector: &mut InspectorState) -> Option<String>` helper. Both handlers call it. Task plan promised this in Task 04's notes ("via a small private helper extracted in Task 04").
- **Acceptance:** No duplicated debounce/cache-hit/state-mutation block remains. Existing tests still pass.

### 3. `render_with_regions` duplicates `Widget::render` body in 3 panels
- **Source:** `code_quality_inspector`, `architecture_enforcer`
- **Files:**
  - `crates/fdemon-tui/src/widgets/devtools/mod.rs:386-406`
  - `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:295-388`
  - `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs:461-468`
- **Required Action:** Refactor each panel to share an internal `render_impl(area, buf, ctx: Option<&mut MouseCtx<'_>>)` (the pattern `log_view`, `network/request_table`, `network/request_details` already use). `Widget::render` and `render_with_regions` become thin wrappers.
- **Acceptance:** Per-panel test asserts `Widget::render` and `render_with_regions(... None)` produce byte-identical buffers.

### 4. Lint suppression anti-pattern in inspector handlers
- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:169` and `:385`
- **Problem:** `let _ = (old_index, new_index); // suppress unused warning`
- **Required Action:** Return only `selection_changed` from the inner scope; remove the `let _ = ...` lines. Update destructuring.
- **Acceptance:** No `let _ = ...` suppression remains. `cargo clippy --workspace --all-targets -- -D warnings` still passes.

### 5. Wrap-mode click region tests missing
- **Source:** `logic_reasoning_checker`, `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-tui/src/widgets/log_view/tests.rs`
- **Required Action:** Add tests covering:
  - `wrap_mode = true`, zero offset baseline (regions match visible rows 1:1)
  - `wrap_mode = true`, `state.offset` causing `wrap_intra_offset > 0` (regions match visible rows after top-clip — locks in fix for Critical #1)
  - `wrap_mode = true`, multi-row entry crossing `content_area.bottom()` (height correctly clipped)
- **Acceptance:** All three tests pass after Critical #1 is fixed.

### 6. 80×24 baseline tests missing for performance + network
- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-tui/src/render/tests.rs`
- **Required Action:** Add one test per affected widget at the spec-mandated 80×24 viewport asserting compact-mode produces no relevant click regions:
  - Performance: `regions.iter().filter(|e| /* perf bar */).count() == 0`
  - Network: similar assertion for the no-detail-tab regions case
- **Acceptance:** Tests pass. Existing 80×40 / 160×30 tests stay for positive-region assertions.

### 7. Manual smoke test required
- **Source:** `risks_tradeoffs_analyzer`
- **Required Action:** Run the end-to-end mouse-only walkthrough on macOS against a live Flutter project:
  - [ ] Click log entry → no scroll, no crash
  - [ ] Double-click same entry within 400ms → stack trace toggles (if entry has stack frames)
  - [ ] Click `[p] Performance` sub-tab → Performance panel becomes active
  - [ ] Inspector tree row click → row selected; layout panel updates within ~500ms
  - [ ] Inspector glyph click → node toggles expand/collapse
  - [ ] Performance frame bar click → frame highlighted with `▔`; detail panel shows timing
  - [ ] Network row click → details appear; click `[h] Headers` → detail tab switches
- **Acceptance:** All steps pass; record results in Phase 4 completion summary.

---

## Minor Issues (Track as Follow-up)

### 8. Clear `last_log_click` on session switch
- **File:** `crates/fdemon-app/src/state.rs:1059`, `handler/update.rs` session-switch arms
- **Action:** Reset stamp in `SelectSessionByIndex`/`NextSession`/`PreviousSession`/`CloseCurrentSession`.

### 9. Hoist `visible_nodes()` lookup in `handle_inspector_toggle_node`
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:430-461`
- **Action:** Capture `value_id` and `has_children` in the same borrow scope as the bounds-check.

### 10. Update `MouseAction::Emit` allocation docstring
- **File:** `crates/fdemon-app/src/mouse_regions.rs`
- **Action:** Module docstring "registry hot path is allocation-free at steady state" no longer accurate. Update or add benchmark.

### 11. Carve out network sub-tab bar from filter-input gate
- **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs:43-52`
- **Action:** Allow clicks on `[i]/[p]/[n]` rect to bypass `filter_input_active` gate (mouse-only user is otherwise trapped).

### 12. Document single-click-inert behavior
- **File:** `docs/MOUSE.md` (create if needed)
- **Action:** Note that single click only updates `last_log_click`; double-click toggles stack trace.

### 13. Strengthen glyph-after-row push order invariant
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs:127-149`
- **Action:** Either add a doc comment on `MouseRegionsBuilder::click` describing the same-z last-pushed-wins rule, or add an assertion-based test that catches reversed push order.

### 14. Update PLAN.md ↔ implementation drift
- **File:** `workflow/plans/features/mouse-support/PLAN.md`
- **Items:**
  - Per-row `Emit` (vs originally-planned `EmitWithCoord`)
  - Double-click without position constraint (vs originally-planned "within 1 cell")

### 15. Tighten log-view region count assertion
- **File:** `crates/fdemon-tui/src/render/tests.rs:377`
- **Action:** Change `assert!(click_log_rows >= 12, ...)` to `assert_eq!(click_log_rows, 12, ...)`.

### 16. Add middle-click test for DevTools
- **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs` `press_tests`
- **Action:** Add `middle_click_on_recorded_region_returns_middle_action` test.

### 17. Clarify `DOUBLE_CLICK_WINDOW` boundary semantics
- **File:** `crates/fdemon-app/src/handler/log_view.rs:15` vs `:94`
- **Action:** Pick inclusive (`<=`) or exclusive (`<`); align comment and operator.

### 18. Document or harden glyph X-coordinate overflow
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs:138-149`
- **Action:** Use `checked_add` with a debug log, or document the silent fallback for extreme depths.

### 19. Document `MouseRegionGuard::deref/deref_mut` `expect()` invariants
- **File:** `crates/fdemon-app/src/mouse_regions.rs:311-325`
- **Action:** Add `// SAFETY:` comments explaining the always-Some invariant.

### 20. Update `MouseRegions::with_capacity()` docstring
- **File:** `crates/fdemon-app/src/mouse_regions.rs:146`
- **Action:** Reflect Phase 4 sizing reality (viewport-bounded, not 32).

### 21. Document `hit_test` O(N) contract
- **File:** `crates/fdemon-app/src/mouse_regions.rs:162`
- **Action:** Add docstring; flag for revisit if Phase 5 expands the surface.

### 22. Audit `MouseAction::as_emit()` placement
- **File:** `crates/fdemon-app/src/mouse_regions.rs`
- **Action:** Decide whether to keep on `MouseAction` or move to a test-only `mouse_regions::testing` module.

### 23. Convert `_frame_index` parameter rationale into TODO
- **File:** `crates/fdemon-app/src/handler/log_view.rs:89`
- **Action:** Replace the underscore-prefix-with-doc pattern with `// TODO(phase-5): ...` or move destructuring to the `update()` callsite.

### 24. Move `LABEL_COL_WIDTH` const after `use` blocks
- **File:** `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs:14-15`

### 25. Replace `matches!(button, MouseButton::Right)` with direct equality
- **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs:38-40`

### 26. Extract row-action push into a closure in `log_view::render_inner`
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:1241, 1296, 1348`

---

## Re-review Checklist

After addressing Critical and Major issues:

- [ ] Critical #1 (wrap-mode misalignment) resolved with regression test
- [ ] Major #2 (`maybe_fetch_layout` extracted)
- [ ] Major #3 (3 sister functions refactored to shared `render_impl`)
- [ ] Major #4 (`let _ = ...` suppression removed)
- [ ] Major #5 (3 wrap-mode tests added)
- [ ] Major #6 (80×24 baseline tests for perf + network)
- [ ] Major #7 (manual smoke test performed and recorded)
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes (no new failures)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] Re-review confirms all Critical and Major findings resolved
