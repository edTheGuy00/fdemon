# Task 04: 80×24 Baseline Tests + Tighten `>=12` Assertion

## Goal

Add spec-mandated 80×24 baseline tests for the Performance and Network panels' compact-mode no-region behavior in `crates/fdemon-tui/src/render/tests.rs`, and tighten the existing log-view region-count assertion from `>=12` to `==12`.

## Background

Two review findings:

1. **Major #6 (80×24 baseline missing).** Phase 4 success criteria specified registry snapshot tests at 80×24 for every panel. Performance and Network were inflated to 80×40 and 160×30 respectively because the chart needs ≥7 inner rows and the network detail panel needs ≥71 cols for 5 tab labels. **Result:** there is no test asserting that at the spec-mandated 80×24 the compact-mode path is exercised and no spurious regions are pushed. A future refactor of the compact-mode path could silently break the "no regions when too small" contract.

2. **Minor #15 (`>=12` assertion).** The log-view region test asserts `assert!(click_log_rows >= 12, ...)`. Companion tests for inspector (==5+5), performance (==8), and network (==5 tabs) all use exact equality. The log-view test should match.

## Files

**Modify:**
- `crates/fdemon-tui/src/render/tests.rs`

**Read (reference):**
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — compact-mode threshold (`MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT`)
- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` — minimum-cols guards

## Plan

1. **Add `performance_compact_mode_at_80x24_records_no_regions`**. Render a `PerformancePanel` view with `vm_connected = true`, `monitoring_active = true`, and at least 8 frames in the buffer at 80×24. Assert that `regions.iter().filter(|e| matches!(e.action, MouseAction::Emit(msg) if matches!(**msg, Message::SelectPerformanceFrame { .. }))).count() == 0`. The compact-mode threshold should kick in at 24 rows (less than `MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT`), so no frame regions are registered.

2. **Add `network_compact_mode_at_80x24_records_no_detail_tab_regions`**. Render a `NetworkMonitor` view at 80×24 with `vm_connected = true`, `extensions_available = Some(true)`, a non-empty request list, and a selected request (so the detail panel would render at 160×30). Assert that detail-tab regions count is 0 (the detail panel collapses or omits the tab bar at 80 cols). The exact predicate depends on the layout — at 80 cols, the panel may render in `render_table_only` mode (no detail panel at all) or render the detail panel without tab regions. Either way, the `NetworkSwitchDetailTab` count must be 0.

   Verify by reading `widgets/devtools/network/mod.rs` and confirming which layout path triggers at 80×24. If the layout is `table_only`, the detail panel is not rendered at all and no tab regions can be pushed. If `narrow_split`, the detail panel renders but the tab bar may be width-clipped.

3. **Tighten the log-view assertion**. Find the existing test (probably named `log_view_records_one_region_per_visible_row` or similar) that uses `assert!(click_log_rows >= 12, ...)`. Confirm that at 80×24 with the test fixture's 12 entries, exactly 12 regions are produced. Change to `assert_eq!(click_log_rows, 12, "expected exactly 12 ClickLogRow regions for 12 visible entries, got {click_log_rows}")`.

   If the count is non-deterministic (e.g., 12 or 13 depending on a height calculation), adjust the fixture to produce a known count and use exact equality. Imprecise assertions hide regressions where duplicate or stale regions accumulate.

## Acceptance Criteria

- [ ] `performance_compact_mode_at_80x24_records_no_regions` test added and passing.
- [ ] `network_compact_mode_at_80x24_records_no_detail_tab_regions` test added and passing.
- [ ] Log-view region-count assertion uses `assert_eq!(... 12 ...)`, not `>=`.
- [ ] All existing render tests still pass.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.

## Notes

- **Do not touch** the per-widget `tests.rs` files in `widgets/log_view/`, `widgets/devtools/performance/`, etc. Those are owned by Tasks 01 and 03 respectively. This task lives entirely in `render/tests.rs`.
- The test for the network panel may need to verify the actual layout path taken at 80×24. If the panel renders the detail panel at 80 cols and the detail-tab bar is clipped to width 0 (so no regions register), that's the no-region condition we want to lock in. If it doesn't render the detail panel at all, even better. Either way, the assertion is "no `NetworkSwitchDetailTab` regions at 80×24".
- The compact-mode threshold for performance is documented in the task plan for Phase 4 task 08. Read that task's notes if the predicate is unclear.
