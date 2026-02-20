# Bugfix Plan: DevTools v2 Phase 3 Review Fixes

## TL;DR

Post-review fixes for the Phase 3 Performance Tab overhaul. Two critical bugs (allocation table invisible on standard terminals due to layout arithmetic; UTF-8 panic in class name truncation), two major structural issues (ring buffer stale index, file size limits), and a batch of minor cleanup items. Seven tasks across three waves.

---

## Bug 1: Allocation Table Invisible on Standard Terminals (CRITICAL)

### Symptoms

The allocation table below the memory chart never appears on terminals with fewer than 30 rows. On a standard 24-row terminal, the memory section's inner area is only 7 rows, but `show_table` requires 9.

### Root Cause Analysis

Compound layout arithmetic issue across three layers:

1. **55/45 split** (`performance/mod.rs:152-155`): The performance panel splits 55% frame / 45% memory. On an 18-row panel area (24-row terminal), memory gets 9 outer rows.
2. **Borders::ALL** on the memory block consumes 2 rows (top + bottom border), leaving 7 inner rows.
3. **Threshold too high** (`memory_chart.rs:107`): `show_table = area.height >= MIN_CHART_HEIGHT + MIN_TABLE_HEIGHT` = `6 + 3 = 9`. Since `7 < 9`, the table is never rendered.
4. **Footer overlap** (`devtools/mod.rs:261`): The DevTools footer writes to the last row of the panel area, which falls inside the memory block's bottom border.

**Layout trace for 24-row terminal (single session):**

```
Terminal: 24 rows
  Header:                3 rows
  areas.logs:           21 rows
    DevTools sub-tab:    3 rows
    PerformancePanel:   18 rows
      55% frame outer:   9 rows → inner 7
      45% memory outer:  9 rows → inner 7  ← show_table needs >= 9
```

The allocation table first appears at **terminal height 30** (single session) or **32** (multi-session).

### Affected Files

- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — 55/45 split ratio
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs` — `MIN_TABLE_HEIGHT`, `show_table` threshold
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — footer overlap with memory block border

---

## Bug 2: UTF-8 Byte-Slice Panic in Class Name Truncation (CRITICAL)

### Symptoms

Application panics with `byte index N is not a char boundary` when a class name from the VM Service contains multi-byte UTF-8 characters (Cyrillic, CJK, emoji) and exceeds 30 bytes.

### Root Cause Analysis

Three instances of `&string[..N]` byte-slice truncation exist:

1. **`memory_chart.rs:682`** (HIGH risk) — `&class.class_name[..27]` where `class_name` comes from unfiltered VM Service JSON. Dart packages from non-English ecosystems use Unicode identifiers.
2. **`search_input.rs:93`** (MEDIUM risk) — `&error[..27]` on regex error messages, which can contain the user's typed pattern.
3. **`session.rs:519`** (MEDIUM risk) — `&self.name[..14]` on device names, which come from the OS (e.g., Chinese Android device names like `"小米 14 Ultra"`).

### Affected Files

- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:681-684`
- `crates/fdemon-tui/src/widgets/search_input.rs:92-93`
- `crates/fdemon-app/src/session/session.rs:518-519`

---

## Bug 3: `selected_frame` Stale Index on Ring Buffer Wrap (MAJOR)

### Symptoms

After 300 frames accumulate (~5 seconds at 60fps), the frame detail panel silently displays data for the wrong frame. The user selects a frame, and as new frames arrive, the selection drifts to show a different frame without any visual indication.

### Root Cause Analysis

1. `frame_history` is a `RingBuffer<FrameTiming>` with capacity 300 (`DEFAULT_FRAME_HISTORY_SIZE`).
2. `selected_frame: Option<usize>` is a bare positional index.
3. When the buffer is at capacity, `RingBuffer::push()` calls `pop_front()`, shifting all indices down by 1.
4. The `VmServiceFrameTiming` handler (`update.rs:1350-1362`) pushes the new frame but **never adjusts `selected_frame`**.
5. Each new frame while selected makes `selected_frame` point to one frame newer than intended.

### Affected Files

- `crates/fdemon-app/src/handler/update.rs:1350-1362` — push without index adjustment
- `crates/fdemon-app/src/session/performance.rs` — `PerformanceState.selected_frame`
- `crates/fdemon-core/src/performance.rs:329-334` — `RingBuffer::push()` eviction

---

## Bug 4: Frame Navigation Logic Duplicated (MAJOR)

### Symptoms

The prev/next frame index computation exists in two places that must be kept in sync manually:
- `handler/keys.rs:379-406` — inline computation in key handler
- `session/performance.rs:118-148` — `select_prev_frame()` / `select_next_frame()` methods

### Affected Files

- `crates/fdemon-app/src/handler/keys.rs:379-406`
- `crates/fdemon-app/src/session/performance.rs:118-148`
- `crates/fdemon-app/src/handler/devtools/performance.rs:19-27`

---

## Bug 5: Files Exceed 500-Line Limit (MAJOR)

### Symptoms

- `memory_chart.rs` — 711 lines (limit: 500)
- `frame_chart.rs` — 544 lines (limit: 500)

Both files already have partially-started directory module structures (tests extracted, `braille_canvas` extracted for memory_chart) but the main source hasn't been split.

### Affected Files

- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs`
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart.rs`

---

## Bug 6: Minor Issues Batch

Six minor issues from the review:

1. **Duplicate test** — `test_parse_memory_usage` identical to `test_parse_memory_usage_still_works` in `fdemon-daemon/src/vm_service/performance.rs:318-344`
2. **`.map().flatten()` anti-pattern** — `fdemon-app/src/handler/devtools/performance.rs:131-137`, should be `.and_then()`
3. **Dead `allocation_sort` state** — `fdemon-app/src/session/performance.rs:70`, field initialised and tested but never read by any handler or widget
4. **Magic number `7`** — `memory_chart.rs:209`, y-axis width should be a named constant
5. **`DEFAULT_MEMORY_SAMPLE_SIZE` visibility** — `session/performance.rs:19`, is `pub` while siblings are `pub(crate)`, not consumed outside `fdemon-app`
6. **Unnecessary `Arc`** — `actions.rs:618`, wraps `watch::Sender` due to `Message: Clone` constraint; document as known design constraint

---

## Affected Modules

- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — layout split ratio
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs` — threshold, UTF-8, extraction
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart.rs` — extraction
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — footer overlap
- `crates/fdemon-tui/src/widgets/search_input.rs` — UTF-8
- `crates/fdemon-app/src/session/session.rs` — UTF-8
- `crates/fdemon-app/src/session/performance.rs` — dead state, visibility, navigation
- `crates/fdemon-app/src/handler/update.rs` — ring buffer index
- `crates/fdemon-app/src/handler/keys.rs` — duplicated navigation
- `crates/fdemon-app/src/handler/devtools/performance.rs` — anti-pattern
- `crates/fdemon-daemon/src/vm_service/performance.rs` — duplicate test

---

## Task Dependency Graph

```
Wave 1 (parallel — independent bug fixes in different files)
┌──────────────────────────────┐  ┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 01-fix-alloc-table-layout    │  │ 02-fix-utf8-truncation       │  │ 03-fix-selected-frame-wrap   │
│ (fdemon-tui performance/)    │  │ (3 files, 3 crates)          │  │ (fdemon-app handler/)        │
└──────────────┬───────────────┘  └──────────────┬───────────────┘  └──────────────┬───────────────┘
               │                                  │                                 │
Wave 2 (parallel — structural improvements)       │                                 │
               │                                  │                                 │
               │         ┌────────────────────────┘              ┌──────────────────┘
               │         │                                       │
               ▼         ▼                                       ▼
┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 04-extract-memory-chart-mods │  │ 05-dedup-frame-nav-logic     │
│ (fdemon-tui memory_chart/)   │  │ (fdemon-app handler/keys.rs) │
│ depends: 01, 02              │  │ depends: 03                  │
└──────────────────────────────┘  └──────────────────────────────┘
                                  ┌──────────────────────────────┐
                                  │ 06-extract-frame-chart-mods  │
                                  │ (fdemon-tui frame_chart/)    │
                                  │ depends: none                │
                                  └──────────────────────────────┘
                                           │
Wave 3 (solo — cleanup after all above)    │
               ┌───────────────────────────┘
               ▼
┌──────────────────────────────┐
│ 07-minor-fixes-batch         │
│ (workspace-wide)             │
│ depends: 04, 05              │
└──────────────────────────────┘
```

---

## Success Criteria

### All Fixes Complete When:

- [ ] Allocation table visible with at least 2 data rows on a standard 24-row terminal
- [ ] No UTF-8 panic with multi-byte class names, device names, or search patterns
- [ ] `selected_frame` correctly tracks the same logical frame through ring buffer wraps
- [ ] Frame navigation has a single source of truth (no duplicated logic)
- [ ] `memory_chart.rs` and `frame_chart.rs` both under 500 lines
- [ ] All minor issues resolved
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

---

## Milestone Deliverable

A clean, merge-ready Phase 3 implementation with no critical, major, or minor issues outstanding from the review.
