# Task 06 — Update Architecture and Review-Focus Docs

**Status:** Not Started
**Wave:** 4
**Agent:** doc_maintainer
**Estimated Effort:** 2 hours
**Depends On:** T01, T02, T04, T05

## Problem

Phase 4 introduces:

- A new timeline event tree model (`TimelineTrack`, `TimelineNode`, `pair_be_events`, B/E pairing algorithm) in `fdemon-core`.
- A breaking state-shape change in `PerformanceState` (`timeline_events: VecDeque<…>` → `timeline_tracks: BTreeMap<…>`).
- A new Gantt rendering subdirectory under `widgets/devtools/performance/details/timeline_events/`.
- An immediate-fetch-on-unpause path in `spawn_timeline_polling`.
- New frame-chart selection-within-viewport semantics.

These changes need documentation in `docs/ARCHITECTURE.md` (DevTools Subsystem → Performance Panel section) and new approved-pattern entries in `docs/REVIEW_FOCUS.md`.

## Files (Write)

- `docs/ARCHITECTURE.md`
- `docs/REVIEW_FOCUS.md`

## Files (Read)

- T01–T05 completion summaries (in `workflow/plans/features/devtools-performance-memory-split/phase-4-performance-polish/tasks/`)
- Current `docs/ARCHITECTURE.md` and `docs/REVIEW_FOCUS.md` — preserve existing content boundaries
- `~/.claude/skills/doc-standards/schemas.md` — content-boundary rules

## Approach Hints

### `docs/ARCHITECTURE.md` additions

Locate the existing "DevTools Subsystem → Performance Panel Interactivity" section. Append the following Phase-4 paragraphs at appropriate insertion points:

**1. Timeline Event Tree Model**

> Phase 4: Timeline Events are stored per-thread as trees of `TimelineNode` instances rather than a flat ring buffer. `fdemon-core::timeline::pair_be_events` reconstructs Begin/End pairs into duration nodes, then nests them by interval containment within each thread. `PerformanceState::timeline_tracks: BTreeMap<i64, TimelineTrack>` holds the result, with stable thread ordering by `tid`. The polling task forwards `ThreadMetadata` events (from `ph="M" name="thread_name"`) so `timeline_thread_name_map` can label rows with human-readable names like `"io.flutter.raster"`.

**2. Gantt Renderer**

> Phase 4: The Timeline Events tab renders as a Gantt chart — thread rows with colored event bars across a fixed `TIMELINE_VIEWPORT_MICROS` window (default 5 s, auto-scrolling forward). Color is per-thread with depth-alternation (UI=light/dark blue, Raster=blue/dark gray, Other=magenta). Depth-stacked children render in vertical bands within each thread row, up to `MAX_DEPTH = 5`. Pan, zoom, event-level selection, and minimap are deferred to Phase 5.

**3. Immediate fetch on unpause**

> Phase 4: `spawn_timeline_polling` mirrors the allocation-polling pattern: on `pause_rx.changed -> false`, one `fetch_timeline_chunk` cycle runs immediately before entering the 1-Hz tick loop. Eliminates the ~1 s cold-start placeholder on every Performance-panel-enter.

**4. Frame-chart selection-within-viewport**

> Phase 4: `compute_visible_range` in `frame_chart/bars.rs` now uses `frame_chart_scroll_offset` as the sole viewport authority — the selected frame is no longer anchored to the right edge. `handle_select_performance_frame` only adjusts `scroll_offset` when the selection moves outside the visible viewport. Bar-height clamping (`MIN_BAR_HALF_BLOCKS = 1`) prevents fast frames from disappearing in small terminal heights. Full-column selection overlay replaces the previous single-character `▔` indicator.

### `docs/REVIEW_FOCUS.md` additions

Under the existing "Approved Optimizations" / "Approved Patterns" section, add:

**1. Gantt Depth-Stacked Rendering**

> Phase 4: depth-stacked timeline event rendering follows DevTools' legacy `FlameChart` pattern — depth N child events render at row Y+N within their parent's row band. This is an approved exception to "one widget = one rectangular region" because depth math is bounded by `MAX_DEPTH` and the renderer always honors `Layout::vertical` parent constraints. Reviewers should not flag this.

**2. Thread-Row Scroll**

> Phase 4: `timeline_thread_scroll_offset` measures scroll position in **thread rows**, not event lines. The Gantt has no event-level selection in Phase 4, so the scroll target is the thread row itself. Phase 5 may add event-level selection within rows.

**3. Full-Column Frame-Chart Selection Overlay**

> Phase 4: the frame chart's selected bar is rendered with a full-column overlay (side-marker characters or distinct background color across every chart row), not a single-character tip. This is an approved replacement for the Phase 1 single-`▔` highlight, which research found visually invisible.

**4. Phase 5 deferred**

> Pan/zoom, minimap, and event-level selection in the Timeline Gantt view are deferred to Phase 5. Reviewers seeing PRs touching `timeline_events/` should expect a fixed-viewport rendering in Phase 4 and a configurable viewport in Phase 5.

### Content boundary checks

Follow `doc-standards/schemas.md` strictly:

- **`docs/ARCHITECTURE.md`** is for system design and data flow only. No tutorials, no code snippets longer than 4 lines, no rationale paragraphs.
- **`docs/REVIEW_FOCUS.md`** is for reviewer guidance — what's approved, what to flag. One-sentence rationale per entry.
- Do not modify `docs/CODE_STANDARDS.md` — Phase 4 does not introduce new conventions.
- Do not modify `docs/DEVELOPMENT.md` — no new build commands or test commands.
- Do not modify `docs/CONFIGURATION.md` — `timeline_event_buffer_size` config key is unchanged.

## Acceptance Criteria

1. **ARCHITECTURE.md** documents:
   - `TimelineTrack` / `TimelineNode` per-thread tree model
   - B/E pairing algorithm + nesting by interval containment
   - `timeline_thread_name_map` wiring from metadata events
   - Gantt renderer layout (thread rows, depth bars, color-by-thread-and-depth, fixed 5 s viewport)
   - Immediate-fetch-on-unpause for timeline polling
   - Frame-chart selection-within-viewport semantics
   - Frame-chart bar-height minimum clamp
   - Full-column selection overlay
2. **REVIEW_FOCUS.md** adds approved-pattern entries for:
   - Gantt depth-stacked rendering
   - Thread-row scroll
   - Full-column frame-chart selection overlay
   - Explicit Phase-5 deferred-scope note
3. **No content boundary violations**:
   - No code snippets > 4 lines in ARCHITECTURE.md
   - No tutorial content in REVIEW_FOCUS.md
   - No edits to `docs/CODE_STANDARDS.md`, `docs/DEVELOPMENT.md`, `docs/CONFIGURATION.md`
4. **Cross-references valid** — Any links to Phase 4 task files or PRs are correct.
5. **Quality gate (lightweight)** — `markdown-lint`-style sanity check: no broken headers, no malformed tables.

## Notes

- This is a **doc-only task** routed to `doc_maintainer`. Do not edit source code.
- Phase 5 deferred-scope language must explicitly enumerate: pan/zoom, minimap, event-level selection, search/filter, CPU samples. So reviewers reading Phase 4 PRs immediately know what's intentionally absent.
- If T01–T05 completion summaries flag any unexpected architectural changes (e.g., new public API, new crate dep), incorporate those into ARCHITECTURE.md as well — read each task's `## Completion Summary` block carefully.
