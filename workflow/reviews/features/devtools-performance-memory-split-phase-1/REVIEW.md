# Review: DevTools Performance/Memory Tab Split — Phase 1

**Review Date:** 2026-05-18
**Branch:** `feat/devtools-inspector-parity`
**Diff Base:** `0b6d9b5..HEAD` (5 merge commits + 1 in-flight fix)
**Verdict:** ⚠️ **NEEDS WORK**

## Scope

Phase 1 of `devtools-performance-memory-split`: 5 tasks splitting the monolithic
Performance DevTools tab into separate **Performance** (frame chart) and
**Memory** (chart + allocation table) panels. Pure refactor — no new RPCs or
VM Service work.

| Task | Status |
|------|--------|
| 01 add-memory-panel-placeholder | Merged |
| 02 extract-memory-state | Merged |
| 03 extract-memory-handlers-and-widgets | Merged |
| 04 update-keybindings-doc | Merged |
| 05 update-architecture-doc | Merged |

27 files changed, 1,924 insertions, 2,468 deletions. All 2,369 `fdemon-app`
lib tests pass; clippy clean.

## Reviewer Verdicts

| Agent | Verdict | Key Findings |
|-------|---------|--------------|
| architecture_enforcer | ⚠️ Warning | Mouse-wheel routing bug; stale module docstring |
| code_quality_inspector | ⚠️ NEEDS WORK | Stale doc comments (×2 modules), missing `REVIEW_FOCUS.md` registration, manual `Rect` arithmetic |
| logic_reasoning_checker | ⚠️ CONCERNS | Lazy-start alloc-unpause guard misses Memory default; mouse-wheel state-routing mismatch |
| risks_tradeoffs_analyzer | ⚠️ Concerns | Mouse-wheel bug; no disconnected UI on Memory panel; K/M/G precision loss; `DetailsTab` "keypress trap" |
| security_reviewer | ✅ PASS | 0 findings — no new panic points, no `unsafe`, RingBuffer access is safe |

Three independent reviewers flagged the **mouse-wheel routing bug** in
`handler/mouse/devtools.rs:99`, making it the strongest signal in the review.

## In-Flight Fix (Already Applied)

During the review, the user observed a live regression: **the Performance
tab was permanently stuck on "performance monitoring starting..."**.

**Root cause:** Pre-T02, `Message::VmServiceMemorySnapshot` set
`performance.monitoring_active = true` as a side effect. T02 moved both the
`memory_history.push()` and the `monitoring_active = true` writes from
`PerformanceState` → `MemoryState`, but nothing else writes
`performance.monitoring_active = true`, so the flag stayed `false` forever
and the Performance widget never left its "starting..." placeholder.

**Fix applied (uncommitted):**
`crates/fdemon-app/src/handler/update.rs` — `VmServicePerformanceMonitoringStarted`
now sets both `performance.monitoring_active` and `memory.monitoring_active`
to `true` when the polling task is registered. Regression test added in
`crates/fdemon-app/src/handler/tests.rs::test_performance_monitoring_started_stores_shutdown_tx`.

This bug was not independently flagged by any of the 5 reviewers — the
user surfaced it via manual smoke test.

## Critical Findings (Must Fix Before Merge)

### C1. Alloc-unpause guard misses Memory default (lazy-start cold path)

**Source:** logic_reasoning_checker (also confirmed by risks_tradeoffs_analyzer)
**File:** `crates/fdemon-app/src/handler/update.rs:1920`

The `VmServicePerformanceMonitoringStarted` handler guards alloc unpause as:

```rust
if state.devtools_view_state.active_panel == DevToolsPanel::Performance {
    if let Some(ref tx) = handle.alloc_pause_tx {
        let _ = tx.send(false);
    }
}
```

But the documented contract (`docs/ARCHITECTURE.md:908`) and the warm-path
handlers (`handle_enter_devtools_mode`, `handle_switch_panel`) all treat
**Performance and Memory as equivalent unpausers**. A user with
`default_panel = "memory"` opens DevTools → `handle_enter_devtools_mode`
queues `StartPerformanceMonitoring` (alloc_pause_tx is still `None`, can't
unpause) → monitoring task starts → `VmServicePerformanceMonitoringStarted`
arrives, but the guard skips because active panel is `Memory`. Alloc polling
stays paused indefinitely.

**Required fix:** Change guard to
`matches!(active_panel, DevToolsPanel::Performance | DevToolsPanel::Memory)`.

### C2. Mouse wheel on Memory tab routes to Performance state

**Source:** architecture_enforcer, logic_reasoning_checker, risks_tradeoffs_analyzer
**File:** `crates/fdemon-app/src/handler/mouse/devtools.rs:99`

```rust
// Memory panel uses the same scroll behaviour as Performance (row scroll,
// Shift for page step) until T03 introduces dedicated memory scroll logic.
DevToolsPanel::Memory => handle_performance_scroll(dir, mods),
```

The comment promises T03 will replace this. T03 added all the `Mem*` message
variants and handlers but never updated the mouse routing. Result: mouse
wheel on the Memory panel silently mutates `session.performance.frame_chart_scroll_offset`
— Memory chart and allocation table don't respond to wheel events at all.

**Required fix:** Add `handle_memory_scroll` emitting
`MemScrollUp/Down`/`MemPageUp/Down`, wire `DevToolsPanel::Memory` to it.

### C3. `performance.monitoring_active` never flipped true (FIXED IN-FLIGHT)

Already addressed before the review consolidated. See **In-Flight Fix** above.
Regression test added. No further action — confirm the fix is committed.

## Major Findings (Should Fix Before Merge)

### M1. Memory panel has no disconnected-state UI

**Source:** risks_tradeoffs_analyzer, architecture_enforcer
**Files:** `crates/fdemon-tui/src/widgets/devtools/mod.rs:162`,
`crates/fdemon-tui/src/widgets/devtools/memory/mod.rs:130-208`

The Performance panel renders a tailored "VM not connected / reconnecting"
view at `widgets/devtools/performance/mod.rs:131`. The Memory panel has
no such guard — it falls through to `"No memory data"` regardless of cause
(no allocations yet vs. VM down vs. monitoring failed). The
`vm_connected` flag in `widgets/devtools/mod.rs:162` is captured as
`_vm_connected` and discarded.

**Required fix:** Thread `vm_connected` and `connection_status` into
`MemoryPanel::new`, render disconnected state mirroring Performance pattern.

### M2. Stale module docstrings

**Source:** code_quality_inspector, architecture_enforcer
**Files:**
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:1-22` — still shows the dual-section "Frame Timing (~45%) / Memory (~55%)" ASCII diagram that no longer exists.
- `crates/fdemon-app/src/handler/devtools/performance.rs:1-6` — still mentions handling "allocation profile updates and rich memory samples" which moved to `memory.rs` in T03.

**Required fix:** Rewrite both `//!` headers to describe current single-section
behavior. Cross-link `memory.rs` from `performance.rs`.

### M3. `docs/REVIEW_FOCUS.md` missing new Cell-render-hint registrations

**Source:** code_quality_inspector, logic_reasoning_checker
**File:** `docs/REVIEW_FOCUS.md:34`

`REVIEW_FOCUS.md` explicitly requires: "New `Cell`-based render-hint fields
require explicit review and documentation here." The plan claimed no new
exceptions are introduced because the pattern is unchanged, but the *fields*
themselves moved to a new struct (`MemoryState`) and need to appear in the
"Current usage" bullet list so future reviewers can find them.

**Required fix:** Add entries for `MemoryState::memory_chart_visible_width`
and `MemoryState::alloc_table_visible_height`.

### M4. `PerfSection::DetailsTab` is a "keypress trap" Phase 2 anchor

**Source:** risks_tradeoffs_analyzer, logic_reasoning_checker
**Files:** `crates/fdemon-app/src/session/performance.rs:16-38`,
`crates/fdemon-app/src/handler/devtools/performance.rs:111, 150, 180, 203`

Pressing Tab on the Performance panel moves focus to `DetailsTab` — but the
renderer does not visualize this focus change and all subsequent
j/k/PgUp/PgDn/Home/End silently no-op. Users get the impression scrolling
broke. The footer hint omits Tab entirely.

**Required fix:** Either make `PerfSection::next()` return `FrameChart` when
`DetailsTab` has no content (effectively disabling Tab until Phase 2 lands
real content), or render a visible "Details (Phase 2)" placeholder so users
see what's happening.

### M5. `format_number` precision loss for instance counts

**Source:** risks_tradeoffs_analyzer
**File:** `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs:54-65`

T03 changed allocation-table instance counts from "1,234" to "1.2K" via
K/M/G suffixes. For a profiling tool where small leak deltas matter
(12,345 → 12,398 = 53 new String instances), this loses ~3 decimal digits
of precision. The `INSTANCES_WIDTH` column easily fits a comma-separated
number.

**Required fix:** Revert instance-count column to comma separators. Keep
K/M/G for byte-size columns only.

## Minor Findings

| # | Source | Issue | Location |
|---|--------|-------|----------|
| m1 | code_quality_inspector | Stale "no longer needed here" comment on `format_number` | `widgets/devtools/memory/mod.rs:54` |
| m2 | code_quality_inspector, architecture_enforcer | `#[allow(dead_code)]` on `MemoryPanel.focused`/`chart_focused` with no rationale | `widgets/devtools/memory/mod.rs:97,101` |
| m3 | code_quality_inspector | Manual `Rect` arithmetic in `render_impl` violates Layout-system principle | `widgets/devtools/memory/mod.rs:158-165` |
| m4 | code_quality_inspector | EXCEPTION annotations cite `CODE_STANDARDS.md` only — should also link `REVIEW_FOCUS.md` | `session/memory.rs:91,97`, `session/performance.rs:73` |
| m5 | code_quality_inspector | `test_tab_bar_shows_all_panels` doesn't check Memory or Network | `widgets/devtools/mod.rs:523-527` |
| m6 | risks_tradeoffs_analyzer | Performance footer omits Tab and j/k bindings while advertising arrows only | `widgets/devtools/mod.rs:373-375` |
| m7 | logic_reasoning_checker, risks_tradeoffs_analyzer | No test for `Perf↔Memory` switch leaving `alloc_pause_tx` untouched | Missing |
| m8 | logic_reasoning_checker | No test for `default_panel = "memory"` cold-path alloc unpause | Missing |
| m9 | risks_tradeoffs_analyzer | `clamp_chart_scroll`/`ScrollDir` duplicated between `performance.rs` and `memory.rs` | `handler/devtools/{performance,memory}.rs` |
| m10 | risks_tradeoffs_analyzer | `monitoring_active` split across two states but always co-set — coupling-by-convention | `session/{performance,memory}.rs` |
| m11 | risks_tradeoffs_analyzer | `PerfSection::DetailsTab` name collides with `state::DetailsTab` (Inspector) | `session/performance.rs` |
| m12 | architecture_enforcer | `_vm_connected` discarded without `TODO(phase-2)` comment | `widgets/devtools/mod.rs:162` |

## Documentation Freshness

| Doc | Updated? | Notes |
|-----|----------|-------|
| `docs/ARCHITECTURE.md` | ✅ Updated by T05 | Validated PASS |
| `docs/KEYBINDINGS.md` | ✅ Updated by T04 | Validated PASS — but Performance tab **footer** keymap drifted from doc keymap (see m6) |
| `docs/REVIEW_FOCUS.md` | ❌ Stale | Missing Memory Cell-field entries (M3) |
| `docs/CODE_STANDARDS.md` | n/a | No new patterns |
| `docs/DEVELOPMENT.md` | n/a | No build changes |

## Strengths

- Clean layer boundaries throughout — `fdemon-tui` reads `fdemon-app::session::memory`, `fdemon-app` depends only on `fdemon-core`. No layer inversions.
- Naming consistency across the four parallel DevTools panels (`MemoryPanel` widget / `MemoryState` state / `MemorySection` enum / `DevToolsPanel::Memory`).
- The `leaving_alloc_panel` refactor in `handle_switch_panel` elegantly handles the shared-sender coalescing.
- All 2,369 `fdemon-app` lib tests pass; clippy is clean with `-D warnings`.
- Security review: 0 findings (no `unsafe`, no new panic points, RingBuffer access is provably safe).
- Memory regression test `test_memory_panel_allocation_table_full_height_at_20_rows` directly proves the headline goal (alloc table is no longer cropped on short-wide terminals).

## Recommendation

**Address all 3 Critical and 5 Major findings before considering Phase 1
shippable.** C3 is already fixed; C1 is a one-line change adjacent to C3;
C2 requires a small new handler function. M1, M2, M3 are all small focused
patches. M4 and M5 are debatable as "fix vs. defer to Phase 2" — but both
materially affect user experience and both are simple. Plus the 12 minor
findings (especially m7, m8 — missing tests for the refactor's central
correctness claim).

Estimated rework: 3–5 hours including new tests.

## Files

- Critical/Major fixes — see [ACTION_ITEMS.md](ACTION_ITEMS.md)
- Original reviewer transcripts — orchestrator agent outputs (see session history)
