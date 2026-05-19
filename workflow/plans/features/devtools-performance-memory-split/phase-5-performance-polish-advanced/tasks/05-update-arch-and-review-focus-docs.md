# Task 05 — Update Architecture and Review-Focus Docs (Phase 5)

**Status:** Not Started
**Wave:** 3
**Agent:** doc_maintainer
**Estimated Effort:** 2 hours
**Depends On:** T01, T02, T03, T04

## Problem

Phase 5 introduces:

- A manual-viewport state machine (`timeline_viewport_*`, `timeline_follow_latest`) with pan/zoom keybindings.
- A minimap ribbon above the time axis.
- An event-selection cursor (`TimelineEventCursor`) and a modal details popup.
- A search-and-jump UX with `/`, `n`, `N` keys.

These need documentation in `docs/ARCHITECTURE.md` (DevTools Subsystem → Performance Panel) and `docs/REVIEW_FOCUS.md` (approved patterns + deferred-scope notes for Phase 6).

## Files (Write)

- `docs/ARCHITECTURE.md`
- `docs/REVIEW_FOCUS.md`

## Files (Read)

- T01–T04 completion summaries
- Current `docs/ARCHITECTURE.md` and `docs/REVIEW_FOCUS.md` (preserve content boundaries)
- `~/.claude/skills/doc-standards/schemas.md` — content-boundary rules

## Approach Hints

### `docs/ARCHITECTURE.md` additions (Phase 5)

Append the following Phase-5 paragraphs at appropriate insertion points within the existing "DevTools Subsystem → Performance Panel → Timeline Events" section:

**1. Manual-Viewport State Machine**

> Phase 5: the Gantt viewport has two modes — `follow_latest = true` (Phase 4 default; window is the latest `viewport_width_micros` of events) and `follow_latest = false` (pinned to `viewport_start_micros`). Pan (`←`/`→` with no selection) and zoom (`+`/`-`) actions both set `follow_latest = false`. The `End` (or `g`) key resets to live-follow and snaps the width back to `TIMELINE_VIEWPORT_MICROS` (5s default). A "PAUSED" indicator renders in the time-axis row whenever `!follow_latest`.

**2. Minimap Ribbon**

> Phase 5: a 1-row minimap above the time axis compresses the full event history to canvas width, with each column colored by the dominant thread in its time slice. A `[...]` overlay marks the current viewport position. The minimap walks only depth-0 root events for dominance computation to keep cost bounded.

**3. Selection Cursor**

> Phase 5: `PerformanceState::timeline_selected_event: Option<TimelineEventCursor>` identifies the focused event by `(tid, depth, ts)`. Arrow keys traverse the per-thread tree (`←`/`→` for siblings, `↑`/`↓` for parent/child or thread). Selection updates auto-pan the viewport to keep the selected event visible, setting `follow_latest = false` as a side effect.

**4. Details Popup**

> Phase 5: pressing `Enter` on a selected event opens a modal overlay (uses `widgets/modal_overlay` helpers) showing the event's full name, category, thread label, ts, dur, parent chain, and child count. The popup follows the existing modal-precedence rules: while open, base-UI widgets receive `MouseCtx::None` and `Esc` falls through popup → selection → DevTools-exit in that order.

**5. Search-and-Jump**

> Phase 5: `/` opens a search input on the Timeline Events tab. Typed chars append to `timeline_search_query`; `Enter` commits the query and arms `n`/`N` for next/previous match cycling. Search **highlights** matching bars (no filtering); `n`/`N` pans the viewport to center on the next match and updates `timeline_selected_event`. `Esc` clears the query.

### `docs/REVIEW_FOCUS.md` additions

Under "Approved Optimizations" / "Approved Patterns":

**1. Viewport State in `PerformanceState` (not widget-local)**

> Phase 5: pan/zoom state lives in `PerformanceState::timeline_viewport_*`, not as widget-internal mutable state. This preserves unidirectional data flow (TEA) — keybindings dispatch messages, handlers mutate state, renderer is a pure function of state. Reviewers should not refactor this into widget-local fields.

**2. Selection Cursor by `(tid, depth, ts)` not Index**

> Phase 5: `TimelineEventCursor = (tid, depth, ts)` is chosen over index-based paths because tracks mutate (new roots appended, oldest evicted) between batches. The cursor is stable as long as the underlying event survives the ring-buffer eviction policy. When the event ages out, the cursor is cleared with a debug log; reviewers should expect and approve this defensive handling.

**3. Search as Highlight, not Filter**

> Phase 5: search highlights matching bars but does not hide non-matches. This matches DevTools' search-and-jump UX. Reviewers should not propose filter-by-name behavior — that's the role of the existing `T`-key thread filter, which Phase 5 preserves untouched.

**4. `n`/`N` Fallthrough Pattern**

> Phase 5: `n` on the TimelineEvents tab returns `TimelineSearchNextMatch` only when `timeline_search_query.is_some()`; otherwise falls through to the existing top-level `n` → Network handler. Mirrors the Phase 3-followup `R`-key fallthrough for HotRestart. Reviewers should approve this pattern wherever a context-specific binding might conflict with a global one.

**5. Minimap Dominant-Thread Coloring**

> Phase 5: per-column color is the thread with the largest total event-duration in that column's time slice. Reviewers should not propose per-event coloring — the macro view's purpose is thread-balance visibility, not event identification.

**6. Phase 6 Deferred Scope**

> Phase 5 closes the interactive-Gantt scope. Phase 6 will add CPU sampling via `getCpuSamples`, cross-thread async connector lines, per-frame zoom-to-frame coupling, event annotation/pinning, and trace export. Reviewers seeing Phase 5 PRs should not expect these features.

### Content boundary checks

- **`docs/ARCHITECTURE.md`** is for system design only. No code snippets > 4 lines, no rationale paragraphs > 1 sentence per pattern.
- **`docs/REVIEW_FOCUS.md`** is reviewer guidance — what's approved, what to flag, what's deferred.
- Do **not** modify `docs/CODE_STANDARDS.md` — Phase 5 reuses existing TEA + Cell render-hint patterns; no new conventions.
- Do **not** modify `docs/DEVELOPMENT.md` — no new build/test commands.
- Do **not** modify `docs/CONFIGURATION.md` — Phase 5 introduces no new config keys (viewport width, zoom factor, etc. are constants, not user-configurable; could be promoted to config in Phase 6 if requested).

## Acceptance Criteria

1. **ARCHITECTURE.md documents** all five Phase 5 mechanisms: viewport state machine, minimap, selection cursor, details popup, search-and-jump.
2. **REVIEW_FOCUS.md adds** six approved-pattern entries covering: viewport state placement, cursor type choice, search-as-highlight, `n`/`N` fallthrough, minimap dominance, and Phase 6 deferral.
3. **No content boundary violations** — no code blocks > 4 lines, no tutorial content, no edits to off-limits files.
4. **Cross-references valid** — any links to Phase 4 or Phase 5 task files are correct.
5. **Quality gate (light)** — markdown-lint sanity: no broken headers, no malformed tables, no dangling refs.

## Notes

- Doc-only task; do not edit source code.
- Phase 6 deferred-scope language must explicitly enumerate the deferred features (CPU sampling, async lines, frame-zoom coupling, event annotation, trace export) so reviewers know what's intentionally absent.
- If T01–T04 completion summaries flag unexpected architectural changes (e.g., new public API, new crate dep, new VM Service call), document those too — read each task's `## Completion Summary` carefully.
