# Review: DevTools v2 Phase 3 — Performance Tab Overhaul

**Review Date:** 2026-02-21
**Verdict:** NEEDS WORK
**Blocking Issues:** 2

## Summary

Phase 3 implements a complete overhaul of the Performance panel: frame bar chart with half-block vertical resolution, braille time-series memory chart with stacked area layers, GC markers, frame selection with phase breakdown, and an allocation table. The implementation is architecturally sound with clean layer boundaries, comprehensive test coverage (87+ new tests), and good defensive coding throughout the VM Service layer.

However, the review identified the **root cause of the reported allocation table bug** and several issues that need attention before merge.

## Agent Verdicts

| Agent | Verdict | Key Finding |
|-------|---------|-------------|
| Architecture Enforcer | PASS with warnings | Frame navigation logic duplicated between `keys.rs` and `PerformanceState` |
| Code Quality Inspector | NEEDS WORK | Two files over 500-line limit; UTF-8 panic risk in class name truncation; duplicate test |
| Logic & Reasoning Checker | CONCERNS | **Root cause of allocation table bug confirmed** — layout arithmetic |
| Risks & Tradeoffs Analyzer | CONCERNS | `selected_frame` index never invalidated on ring buffer wrap; `allocation_sort` dead state |

## Allocation Table Bug — Root Cause

The reported bug ("allocation table shows no class lists") has been confirmed as a **layout arithmetic issue**, not a data flow problem.

**Proof:**

1. `PerformancePanel::render_content()` splits the area 55%/45% (frame timing / memory)
2. The memory section gets a `Block` with `Borders::ALL`, consuming 2 rows for borders
3. In a standard 24-row terminal, the DevTools content area is ~20 rows
4. Memory section: `floor(20 * 0.45) = 9 rows`, minus 2 for borders = **7 inner rows**
5. `show_table` in `memory_chart.rs:107` requires `area.height >= MIN_CHART_HEIGHT + MIN_TABLE_HEIGHT = 6 + 3 = 9`
6. **7 < 9, so the allocation table is never rendered** on a standard 24-row terminal

Even on a 30-row terminal (inner area = 9 rows), only 1 data row fits after the header and separator.

The data flow itself is correct: polling works, parsing works, the handler stores the profile, and the widget receives it. The table rendering code is sound — it simply never gets called because the area is too small.

## Critical Issues

### 1. Allocation table invisible on standard terminals

- **Source:** Logic & Reasoning Checker, Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:107`
- **Problem:** `show_table = area.height >= 9` threshold is too high for the 45% memory section after block borders
- **Fix options:**
  - (a) Lower `MIN_TABLE_HEIGHT` from 3 to 2 (show header + 1 data row in tighter spaces)
  - (b) Increase memory section proportion from 45% to 50%
  - (c) Remove block borders on the memory section (use a simple title line instead)
  - (d) Combine (a) + (b) for best results

### 2. UTF-8 byte-slice panic in class name truncation

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:681-684`
- **Problem:** `&class.class_name[..27]` is a byte-slice that panics if byte 27 falls within a multi-byte UTF-8 sequence (Cyrillic, CJK, emoji class names)
- **Fix:** Replace with char-based truncation: `class.class_name.chars().take(27).collect::<String>()`

## Major Issues

### 3. `memory_chart.rs` exceeds 500-line limit (710 lines)

- **Source:** Code Quality Inspector
- **Fix:** Extract `render_sample_chart`/`render_history_chart` to `memory_chart/chart_renderer.rs` and `render_allocation_table` to `memory_chart/allocation_table.rs`

### 4. `frame_chart.rs` exceeds 500-line limit (543 lines)

- **Source:** Code Quality Inspector
- **Fix:** Extract pure helper functions (`bar_colors`, `ms_to_half_blocks`, `render_bar`, etc.) to `frame_chart/helpers.rs`

### 5. `selected_frame` index never invalidated on ring buffer wrap

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs:1350-1362`
- **Problem:** When `frame_history` wraps (after 300 frames at 60fps = 5 seconds), `selected_frame` index points to a different frame than the one the user selected. Silent UI data corruption.
- **Fix:** After pushing a new frame in the `VmServiceFrameTiming` handler, check if the buffer has wrapped and either decrement `selected_frame` to track the same logical frame, or clear it to `None`

### 6. Frame navigation logic duplicated in `keys.rs`

- **Source:** Architecture Enforcer
- **Files:** `crates/fdemon-app/src/handler/keys.rs:379-406` duplicates `PerformanceState::select_prev_frame()/select_next_frame()`
- **Fix:** Add `compute_prev_frame_index(&self) -> Option<usize>` / `compute_next_frame_index(&self) -> Option<usize>` pure methods on `PerformanceState` that `keys.rs` calls

## Minor Issues

### 7. Duplicate test in daemon crate

- **File:** `crates/fdemon-daemon/src/vm_service/performance.rs:318-344`
- **Fix:** Delete `test_parse_memory_usage` (identical to `test_parse_memory_usage_still_works`)

### 8. `.map().flatten()` anti-pattern in test code

- **File:** `crates/fdemon-app/src/handler/devtools/performance.rs:131-137`
- **Fix:** Replace with `.and_then()`

### 9. `allocation_sort` is dead state

- **File:** `crates/fdemon-app/src/session/performance.rs:70`
- **Fix:** Either wire to rendering or remove with a TODO comment

### 10. Magic number `7` for Y-axis label width

- **File:** `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:209`
- **Fix:** Add `const Y_AXIS_LABEL_WIDTH: u16 = 7;`

### 11. `DEFAULT_MEMORY_SAMPLE_SIZE` inconsistent visibility

- **File:** `crates/fdemon-app/src/session/performance.rs:19`
- **Fix:** Change `pub` to `pub(crate)` to match adjacent constants

### 12. Hardcoded "60s ago" x-axis label

- **File:** `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:604`
- **Fix:** Add comment documenting the assumption, or derive from actual timestamps

### 13. Unnecessary `Arc` wrapping of `watch::Sender`

- **File:** `crates/fdemon-app/src/actions.rs:618`
- **Fix:** Remove `Arc::new()` if the `Message` variant can accept `watch::Sender<bool>` directly

## Strengths

- Clean layered architecture — all types in correct crates, no dependency violations
- Comprehensive test coverage: 87+ new tests across 4 crates
- Excellent defensive coding in VM Service layer — all RPCs gracefully handle failures
- Correct braille canvas implementation (verified against Unicode standard)
- Good fallback behavior (MemorySample -> MemoryUsage) when rich data unavailable
- Frame chart half-block rendering provides genuine 2x vertical resolution
- TEA pattern followed throughout — state mutations via handlers, rendering is pure

## Reviewed By

- Architecture Enforcer Agent
- Code Quality Inspector Agent
- Logic & Reasoning Checker Agent
- Risks & Tradeoffs Analyzer Agent
