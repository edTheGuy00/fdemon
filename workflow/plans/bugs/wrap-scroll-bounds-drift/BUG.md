# Bugfix Plan: Wrap-mode scroll bounds drift for wrapped stack-frame lines (issue #73)

## TL;DR

In wrap mode, `calculate_entry_display_rows` counts every stack-frame line as exactly 1 row while the renderer measures the same lines exactly with `line_wrapped_row_count`. When any frame line wraps (long paths/function names on narrow panes), `total_lines`/`max_offset`/`units_skipped` drift from rendered reality — bottom rows become unreachable or the view over-scrolls. Fix: measure frame lines exactly in the estimate, using the same shared grapheme helpers, computed allocation-free from raw `StackFrame` fields (no `Line` construction). Per-entry caching is explicitly deferred.

## Bug Reports

### Bug 1: scroll bounds drift when stack-frame lines wrap

**Symptom:** In wrap mode with long stack-frame lines above the viewport, scrolling misbehaves: the last rows of the log can be unreachable, the scrollbar proportions are wrong, and offset↔row mapping is shifted.

**Expected:** `total_lines` equals the sum of actually rendered row heights, at every width, for expanded and collapsed traces.

**Root Cause Analysis:**
1. `calculate_entry_display_rows` (`crates/fdemon-tui/src/widgets/log_view/mod.rs:752-768`) measures the message line exactly (grapheme packing) but hardcodes frame lines at 1 row each ("Stack frame lines rarely exceed terminal width" — false for Flutter's deep package paths on narrow panes).
2. The render loop measures visible frame lines exactly (`line_wrapped_row_count` at `mod.rs:1785`, `1850`), so estimate and render disagree by (actual_rows − 1) per wrapping frame line. `total_lines` (`mod.rs:1628` → `update_content_size` → `max_offset`) and the render loop's own `units_skipped` math (`mod.rs:1673`) both consume the wrong estimate.

**Affected Files:**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — the fix lives here entirely.
- (read-only) `crates/fdemon-app/src/log_view_state.rs` — shared helpers `grapheme_cell_widths` / `wrapped_row_count_widths` already exist post-#72; no changes expected.

## Affected Modules

- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: replace the 1-row frame assumption with exact, allocation-free per-frame-line measurement; add tests.

## Phases

### Phase 1: exact frame-line measurement in the estimate — Critical

Compute each frame line's wrapped row count from raw `StackFrame` fields using the verified render formula (research/RESEARCH.md Q2), WITHOUT building the styled `Line`:

**Steps:**
1. Add a private helper that yields the per-cluster cell widths of a frame line as chained iterators, mirroring `format_stack_frame_line_with_links` exactly:
   - normal frame: `repeat_n(1, 4 /*INDENT*/ + 1 + max(3, digits(frame_number)))` ⧺ `grapheme_cell_widths(function_name)` ⧺ `repeat_n(1, 2 /*" ("*/ [+ 3 if badge])` ⧺ `grapheme_cell_widths(short_path)` ⧺ `repeat_n(1, 1 + digits(line) [+ 1 + digits(col) if col>0] + 1)`.
   - async-gap frame: `repeat_n(1, 4 + 25)` (`<asynchronous suspension>` is ASCII).
   - badge presence: consult `self.link_highlight_state` for `(entry_index, frame_index)` — same lookup the formatter uses (`mod.rs:625-627`). Cheap; the links list is small and usually `None`.
2. In `calculate_entry_display_rows`, for each frame line the entry renders (expanded: all; collapsed: first `max_collapsed_frames`), sum `wrapped_row_count_widths(<frame widths>, visible_width)` instead of 1. Add the collapsed-indicator line via the same measurement (`INDENT + "▶ " + "N more frame(s)..."` — digits(N) matters).
3. Keep the message-line path unchanged.
4. Segment-chaining caveat: chaining per-segment grapheme widths assumes no cluster spans a segment boundary; every join lands on ASCII (`space`, `(`, `:`, digits), so boundaries hold. Note this in the helper's doc comment.

**Measurable Outcomes:**
- Property test (the issue's acceptance invariant): for entries with expanded/collapsed traces, async gaps, long function names/paths, CJK/emoji in message AND path, frame numbers ≥1000, column present/absent — `calculate_entry_display_rows(entry, w)` == sum of `line_wrapped_row_count` over the exact formatted lines the render loop produces, for several widths (incl. narrow: 10, 20, 40).
- Regression test reproducing #73: a long frame line at width 40 previously estimated 2 rows vs 3 rendered; now equal.
- A scroll-bounds test: with such entries, `max_offset` positions the true last row at the bottom (no unreachable tail, no over-scroll).
- Link-mode test: with an active `LinkHighlightState` granting a badge to a frame whose line sits at the wrap boundary, estimate still equals render.

## Edge Cases & Risks

### Hot path cost
- **Risk:** the estimate runs over the whole filtered buffer per frame (`mod.rs:1628`); adding frame measurement increases that cost.
- **Mitigation:** allocation-free iterator chains only (no `Line`/`String` construction); cost is proportional to trace-bearing entries, which are sparse in real buffers (the message-text walk, which dominates, already exists today). If profiling ever shows this hot, the follow-up cache (below) is the remedy — not approximation.

### Link badges (3 chars of real text)
- **Risk:** badges change wrap geometry; a stale/incorrect badge lookup reintroduces drift exactly while link mode is active.
- **Mitigation:** use the same `link_highlight_state` lookup as the formatter. Note: message-line badges (links in message text) remain unmeasured by the message-line estimate — a narrower, pre-existing, transient (link-mode-only) inaccuracy, out of scope here; folded into the follow-up below.

### Frame numbers ≥1000 / unusual frames
- **Risk:** `{:<3}` is a minimum width; hardcoding 8-char prefixes drifts for ≥1000 frames.
- **Mitigation:** `1 + max(3, digits(n))` in the formula; property test includes ≥1000.

### Refuted/contested research claims (kept out of the plan body)
- "LogEntry is immutable post-ingestion" — REFUTED: `add_log` retroactively mutates `level` over ranges. Irrelevant to this fix (level doesn't change measured text), but disqualifying for naive caching; recorded for the follow-up.
- "Link badges don't affect geometry" — REFUTED; handled above.
- "Frame prefix is fixed 8 chars" — CONTESTED (≥1000); handled above.
- The test-only `format_stack_frame` (`mod.rs:489-553`) is dead code — the helper must mirror `format_stack_frame_line_with_links`, not it.

## Further Considerations

1. **Per-entry row-count cache — DEFERRED.** Would also eliminate the pre-existing whole-buffer message walk per frame. Real invalidation surface (width/wrap-mode key, expand-collapse, eviction pruning, `level` range mutation, link-mode bypass) + a required REVIEW_FOCUS.md registry entry make it a separate perf task. Proposal: file as a new issue after this fix lands.

## Task Dependency Graph

Single task (single file, single module) → sequential in the main loop; no worktree fan-out needed. `review-diff` (changeType: bug) after.
