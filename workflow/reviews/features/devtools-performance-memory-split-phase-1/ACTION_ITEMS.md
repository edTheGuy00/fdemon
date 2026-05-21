# Action Items: DevTools Performance/Memory Tab Split — Phase 1

**Review Date:** 2026-05-18
**Verdict:** ⚠️ NEEDS WORK
**Critical:** 3 (1 already fixed in-flight) | **Major:** 5 | **Minor:** 12

## Critical Issues (Must Fix)

### C1. Extend alloc-unpause guard to cover Memory default panel

- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs`
- **Line:** 1920 (handler for `Message::VmServicePerformanceMonitoringStarted`)
- **Problem:** When `default_panel = "memory"`, the lazy-start cold path leaves `alloc_pause_tx` paused forever — `handle_enter_devtools_mode` can't unpause it (sender doesn't exist yet) and the guard in `VmServicePerformanceMonitoringStarted` only matches `DevToolsPanel::Performance`. Memory tab never sees fresh allocation data.
- **Required Action:** Replace
  ```rust
  if state.devtools_view_state.active_panel == DevToolsPanel::Performance {
  ```
  with
  ```rust
  if matches!(
      state.devtools_view_state.active_panel,
      DevToolsPanel::Performance | DevToolsPanel::Memory,
  ) {
  ```
- **Acceptance:** Add `test_lazy_start_memory_default_unpauses_alloc` mirroring `test_monitoring_started_handler_adjusts_alloc_for_performance_panel` but with `active_panel = Memory`. Assert `alloc_pause_tx.borrow() == false` after the message dispatches.

### C2. Add `handle_memory_scroll` for mouse-wheel on Memory tab

- **Source:** architecture_enforcer, logic_reasoning_checker, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs`
- **Line:** 99 (Memory arm of `handle_scroll`)
- **Problem:** Mouse-wheel on Memory tab dispatches `Message::PerfScroll*`, silently mutating `session.performance.frame_chart_scroll_offset` instead of the Memory chart or alloc table.
- **Required Action:**
  1. Add a `handle_memory_scroll(dir: ScrollDir, mods: KeyModifiers) -> Option<Message>` that emits `Message::MemScrollUp/Down` for plain wheel, `Message::MemPageUp/Down` for Shift+wheel — mirror `handle_performance_scroll`.
  2. Change the `DevToolsPanel::Memory` arm from `handle_performance_scroll(dir, mods)` to `handle_memory_scroll(dir, mods)`.
  3. Remove the stale "until T03 introduces dedicated memory scroll logic" comment.
- **Acceptance:**
  - Add `test_memory_panel_mouse_wheel_emits_mem_scroll` for each direction × modifier combination.
  - Add a regression assertion that `session.performance.frame_chart_scroll_offset` is unchanged after Memory wheel events.

### C3. `performance.monitoring_active` never set to `true` — ✅ FIXED IN-FLIGHT

- **Source:** Manual smoke test by the user during review
- **File:** `crates/fdemon-app/src/handler/update.rs` (handler for `Message::VmServicePerformanceMonitoringStarted`)
- **Status:** Already addressed (uncommitted). The handler now sets both `performance.monitoring_active = true` and `memory.monitoring_active = true` when the polling task is registered. `test_performance_monitoring_started_stores_shutdown_tx` extended to assert both flags flip.
- **Required Action:** Commit the fix and the test update along with C1 and C2.

## Major Issues (Should Fix)

### M1. Memory panel needs a disconnected-state render path

- **Source:** risks_tradeoffs_analyzer, architecture_enforcer
- **Files:**
  - `crates/fdemon-tui/src/widgets/devtools/mod.rs:162` — `_vm_connected` is discarded
  - `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs:130-208` — no `!vm_connected` guard
- **Required Action:** Thread `vm_connected` and `connection_status` into `MemoryPanel::new`. Render a disconnected view mirroring `widgets/devtools/performance/mod.rs:131` (consider extracting a shared `render_disconnected` helper).
- **Acceptance:** New widget test `memory_panel_renders_disconnected_state_when_vm_unavailable`.

### M2. Update stale module docstrings

- **Source:** code_quality_inspector, architecture_enforcer
- **Files:**
  - `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:1-22` — still shows dual-section "Frame Timing (~45%) / Memory (~55%)" diagram.
  - `crates/fdemon-app/src/handler/devtools/performance.rs:1-6` — still mentions "allocation profile updates and rich memory samples" (moved to `memory.rs`).
- **Required Action:** Rewrite both `//!` headers to reflect the post-T03 single-section reality. Cross-link `super::memory` from the handler doc.
- **Acceptance:** Manual read; no test impact.

### M3. Register new `MemoryState` Cell fields in `docs/REVIEW_FOCUS.md`

- **Source:** code_quality_inspector, logic_reasoning_checker
- **File:** `docs/REVIEW_FOCUS.md:29-34` ("Current usage" of approved Cell exceptions)
- **Required Action:** Add bullets for `MemoryState::memory_chart_visible_width` and `MemoryState::alloc_table_visible_height` matching the existing entry style (purpose, who writes, who reads, default).
- **Acceptance:** `docs/REVIEW_FOCUS.md` lists all five Cell render-hint fields (the existing three + the two new).

### M4. Tame the `PerfSection::DetailsTab` keypress trap

- **Source:** risks_tradeoffs_analyzer, logic_reasoning_checker
- **Files:** `crates/fdemon-app/src/session/performance.rs:16-38`, `crates/fdemon-app/src/handler/devtools/performance.rs:111, 150, 180, 203`
- **Problem:** Tab on Performance panel moves focus to `DetailsTab` → no visible change → all scroll keys silently no-op. Footer hint omits Tab.
- **Required Action:** Choose one:
  - **Option A (YAGNI):** Make `PerfSection::next()` return `FrameChart` when there's nothing else to cycle to — effectively Tab no-ops until Phase 2 attaches content. Keep the variant for forward compatibility.
  - **Option B (visible stub):** Render a small "Details (Phase 2)" placeholder pane when `focused_section == DetailsTab` so the focus change is visible. Update the footer hint to advertise Tab.
- **Acceptance:** Verify by hand that Tab on Performance no longer puts the panel into a state where arrow keys appear broken. Add a test for the chosen behavior.

### M5. Revert allocation-table instance counts to comma-separated formatting

- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs:54-65` (`format_number`) and its call site in `table.rs:213`
- **Problem:** "1.2K instances" loses ~3 decimal digits of precision vs. "1,234 instances". Allocation profiling needs exact counts to spot small leak deltas. The `INSTANCES_WIDTH` column has space for full numbers.
- **Required Action:** Use a comma-separated formatter for the instances column. Keep K/M/G for byte-size columns (already handled by `MemoryUsage::format_bytes`).
- **Acceptance:** A test asserting "12345" renders as "12,345" not "12.3K".

## Minor Issues (Consider Fixing)

| # | Source | Fix |
|---|--------|-----|
| m1 | code_quality_inspector | Rewrite stale "(no longer needed here, but kept for format_number)" comment in `widgets/devtools/memory/mod.rs:54` as a proper doc-comment on `format_number`. |
| m2 | code_quality_inspector | Add `// Phase 2: ...` comments next to `#[allow(dead_code)]` on `MemoryPanel.focused` and `chart_focused`, or drop the fields and re-add when needed. |
| m3 | code_quality_inspector | Replace manual `Rect { y: area.y + chart_height, ... }` arithmetic in `widgets/devtools/memory/mod.rs:158-165` with `Layout::vertical([Constraint::Length(chart_height), Constraint::Min(0)])` to comply with `CODE_STANDARDS.md` Principle 2. |
| m4 | code_quality_inspector | Update EXCEPTION annotations on `session/memory.rs:91,97` and `session/performance.rs:73` to cite `docs/REVIEW_FOCUS.md` alongside `docs/CODE_STANDARDS.md`. |
| m5 | code_quality_inspector | Expand `test_tab_bar_shows_all_panels` in `widgets/devtools/mod.rs:523-527` to assert all four panel labels (`Inspector`, `Performance`, `Memory`, `Network`). Drop the negative `Layout` assertion. |
| m6 | risks_tradeoffs_analyzer | Audit Performance tab footer (`widgets/devtools/mod.rs:373-375`) — advertised keys should match `docs/KEYBINDINGS.md`. Add `Tab` and `j/k` or document why they're omitted. |
| m7 | logic_reasoning_checker, risks_tradeoffs_analyzer | Add `test_switch_performance_to_memory_does_not_pause_alloc` and the symmetric reverse — assert `alloc_pause_tx.borrow() == false` across both transitions. Protects the refactor's central correctness claim. |
| m8 | logic_reasoning_checker | Add `test_enter_devtools_with_memory_default_sends_unpause` — symmetric to the existing Performance variant. |
| m9 | risks_tradeoffs_analyzer | Extract `clamp_chart_scroll` and `ScrollDir` into `handler/devtools/scroll_helpers.rs` (or `handler/devtools/mod.rs` private helpers). Currently duplicated between `performance.rs` and `memory.rs`. Track as a follow-up task if not done here. |
| m10 | risks_tradeoffs_analyzer | Document the `performance.monitoring_active ⇔ memory.monitoring_active` co-set invariant in code comments on both fields, OR consolidate into `Session::monitoring_active`. They share lifecycle. |
| m11 | risks_tradeoffs_analyzer | Rename `PerfSection::DetailsTab` to `PerfSection::Details` or `PerfSection::DetailsPane` to disambiguate from `state::DetailsTab` (Inspector). |
| m12 | architecture_enforcer | Add `// TODO(phase-2): use vm_connected for disconnected-state` next to `_vm_connected` capture at `widgets/devtools/mod.rs:162` (subsumed by M1 if M1 is taken). |

## Re-review Checklist

Before re-running the reviewer skill:

- [ ] C1 fixed in `update.rs:1920` with regression test
- [ ] C2 fixed in `mouse/devtools.rs:99` with regression test
- [ ] C3 fix committed (already applied to working tree)
- [ ] M1 disconnected-state UI added to Memory panel
- [ ] M2 stale docstrings rewritten in `performance/mod.rs` and `handler/devtools/performance.rs`
- [ ] M3 `REVIEW_FOCUS.md` entries added for the two new Cell fields
- [ ] M4 `DetailsTab` keypress trap resolved (Option A or B chosen)
- [ ] M5 instance-count formatting reverted to comma separators
- [ ] Verification suite green:
  ```bash
  cargo fmt --all -- --check && \
    cargo check --workspace --all-targets && \
    cargo test --workspace && \
    cargo clippy --workspace --all-targets -- -D warnings
  ```
- [ ] Manual smoke per `TASKS.md` "Phase-Wide Acceptance Test Plan" — particularly steps 4 (Memory tab Tab/j/k/s flow) and 6 (`m` then `Esc` → Logs).
