# Task 01: Exact frame-line measurement in calculate_entry_display_rows

Fixes https://github.com/edTheGuy00/fdemon/issues/73. Full context: ../BUG.md, ../research/RESEARCH.md (read both first).

Repo: /home/ed/Dev/personal/fdemon-pro/fdemon, branch `fix/73-wrap-scroll-bounds` (already checked out).
Build/tests: `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo test -p fdemon-tui` (plus `cargo fmt --all` and `cargo clippy -p fdemon-tui` before finishing).

## Problem

`calculate_entry_display_rows` (`crates/fdemon-tui/src/widgets/log_view/mod.rs:752-768`) measures the message line exactly (grapheme packing via `wrapped_row_count_widths`) but counts every stack-frame line as 1 row. The render loop measures the same lines exactly (`line_wrapped_row_count` at mod.rs:1785/1850), so in wrap mode `total_lines`/`max_offset`/`units_skipped` drift whenever a frame line wraps.

## Fix (Phase 1 of BUG.md — follow it exactly)

1. Add a private helper on `LogView` that yields a frame line's per-cluster cell widths as a chained, allocation-free iterator, mirroring `format_stack_frame_line_with_links` (mod.rs:559-654) EXACTLY. Do NOT mirror the test-only `format_stack_frame`/`format_stack_frame_line` (mod.rs:489-553, dead code). Shapes:
   - Async gap (`frame.is_async_gap`): `INDENT(4)` + `<asynchronous suspension>` (25 ASCII chars) → `repeat_n(1, 29)`.
   - Normal frame: `repeat_n(1, 4 + 1 + max(3, digits(frame_number)))` (INDENT + `#` + `{:<3}` min-width number — verify the exact prefix against the formatter, including the trailing space after the function name and `(`)
     ⧺ `grapheme_cell_widths(&frame.function)` (use the actual field name from the formatter)
     ⧺ `repeat_n(1, /* " (" */ 2 + if badge { 3 } else { 0 })`
     ⧺ `grapheme_cell_widths(frame.short_path())`
     ⧺ `repeat_n(1, 1 + digits(line) + if frame.column > 0 { 1 + digits(column) } else { 0 } + 1)`.
     IMPORTANT: derive every constant from the formatter's actual spans — the numbers above are from research; if the formatter disagrees, the formatter wins. The invariant test is the arbiter.
   - Badge presence: same lookup the formatter uses at mod.rs:625-627 (`self.link_highlight_state` → `DetectedLink` for `(entry_index, frame_index)`). `calculate_entry_display_rows` currently doesn't receive the entry index — extend its signature (or the helper's) as needed; callers are mod.rs:1628 and mod.rs:1673 where the index is in scope.
   - Collapsed indicator line: mirror `format_collapsed_indicator` (mod.rs:657-679): `INDENT(4)` + `▶ ` (2 cells) + the exact text it renders (`N more frame(s)...` — check singular/plural and digits(N)).
2. In `calculate_entry_display_rows`, replace the `frame_lines` 1-row-each count with, per frame line the entry actually renders (expanded: all frames; collapsed: first `max_collapsed_frames`; plus indicator line if truncated — same rules as `calculate_entry_lines` mod.rs:692-708): `wrapped_row_count_widths(<frame widths iterator>, visible_width)`.
3. Message-line path: unchanged.
4. Doc comment on the helper: note the segment-chaining assumption (grapheme clusters never span segment joins because every join lands on ASCII — space/paren/colon/digits).
5. No changes to `crates/fdemon-app` — the shared helpers (`grapheme_cell_widths`, `wrapped_row_count_widths`) are already public and sufficient.

## Tests (in crates/fdemon-tui/src/widgets/log_view/tests.rs, follow existing test style there)

1. **Invariant property test** (the acceptance criterion): for a set of constructed entries — expanded trace, collapsed trace (> max_collapsed_frames), async-gap frames, long function names and long short_paths, CJK/emoji in message AND in path, frame_number ≥ 1000, column present and absent — assert `calculate_entry_display_rows(entry, w)` == sum of `line_wrapped_row_count(&line, w)` over the exact formatted lines the render loop would produce for that entry (message via `format_entry`, frames via `format_stack_frame_line_with_links`, indicator via `format_collapsed_indicator` — respecting expand/collapse), for widths at least {10, 20, 40, 80}. Build entries with the existing test fixtures (`make_entry`/`logs_from`/stack-trace fixtures already in tests.rs).
2. **#73 regression test**: an entry with one ~90-char frame line at width 40 — previously estimate=2 vs rendered=3; now equal (assert the exact number too).
3. **Scroll-bounds test**: render such entries in a small viewport via `render_with_regions`, scroll to `max_offset` (e.g. via `LogViewState::scroll_to_bottom` + `update_content_size` flow the render performs) and assert the LAST logical line's text is actually present in the rendered buffer (no unreachable tail).
4. **Link-badge test**: with an active `LinkHighlightState` granting a badge to a frame whose line length sits exactly at the wrap boundary (line fits width without badge, overflows with it), assert estimate == rendered row sum while link mode is active.

## Constraints

- Allocation-free in the estimate path: no `Line`, `String`, or `Vec` construction inside `calculate_entry_display_rows`/the helper (iterator chains only). Tests may allocate freely.
- Hot path: this runs over the whole filtered buffer per frame (mod.rs:1628) — keep it lean.
- `cargo fmt --all`, `cargo clippy -p fdemon-tui` (zero new warnings), `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo test -p fdemon-tui` and `cargo test -p fdemon-app` all green.
- Commit source only when done: `git add --all -- . ':!workflow/plans/'` then a single commit `fix(tui): measure wrapped stack-frame lines exactly in scroll-bounds estimate (#73)`.

## Acceptance criteria

- [x] Invariant test passes across all listed entry shapes and widths
- [x] #73 regression + scroll-bounds + link-badge tests pass
- [x] Full fdemon-tui + fdemon-app test suites green; fmt/clippy clean
- [x] No allocations added to the estimate path; no fdemon-app changes
- [x] Single commit on `fix/73-wrap-scroll-bounds`, workflow/plans excluded

---

## Completion Summary

**Status:** Done
**Branch:** `fix/73-wrap-scroll-bounds`

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Added `digits()` free fn (allocation-free digit-count), `FrameLineWidths<A, B>` enum (non-boxed iterator unification for the async-gap vs normal-frame branches), `LogView::frame_line_widths()` (mirrors `format_stack_frame_line_with_links` exactly as a chained width iterator, including the same `link_highlight_state` badge lookup), `LogView::collapsed_indicator_widths()` (mirrors `format_collapsed_indicator` exactly). Rewrote `calculate_entry_display_rows` to take a new `entry_index: usize` parameter and sum exact `wrapped_row_count_widths` over every frame line + indicator line the entry actually renders (respecting expand/collapse), replacing the old `logical_lines.saturating_sub(1)` (1-row-per-frame-line) approximation. Updated both call sites (`mod.rs` total_lines computation and the render loop) to pass `idx` as `entry_index`. |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Added `rendered_row_count()` (ground-truth helper: sums `line_wrapped_row_count` over the exact lines `format_entry`/`format_stack_frame_line_with_links`/`format_collapsed_indicator` would produce) and `wrap_invariant_trace()` fixture, plus 4 new tests: `test_calculate_entry_display_rows_matches_render_expanded_and_collapsed` (invariant, 3 view configs × widths {10,20,40,80}), `test_calculate_entry_display_rows_wrapping_frame_line_issue_73` (regression, asserts exact `3`), `test_scroll_bounds_last_wrapped_frame_row_reachable_issue_73` (renders via `render_with_regions`, asserts the wrapped frame line's tail is present in the buffer at the auto-scrolled bottom), `test_calculate_entry_display_rows_matches_render_with_link_badge_at_wrap_boundary` (badge pushes a line from 1→2 rows; estimate matches). |

### Notable Decisions/Tradeoffs

1. **`FrameLineWidths<A, B>` enum instead of `Box<dyn Iterator>`**: the async-gap and normal-frame branches of `frame_line_widths` produce structurally different iterator chain types. A manual 2-variant enum with a hand-written `Iterator` impl unifies them into one `impl Iterator` return type with zero heap allocation, satisfying the task's allocation-free constraint (an `either`/`itertools` crate dependency was not present in the workspace, so this was added locally rather than pulling in a new dependency).
2. **Collapsed-indicator's `"▶ "` measured via `grapheme_cell_widths`, not assumed ASCII-width-1**: unlike the rest of the indicator text (pure ASCII digits + literal), the arrow glyph is non-ASCII, so its width is measured the same way the renderer measures it rather than hardcoded — avoids depending on an unverified assumption about `unicode-width`'s classification of U+25B6.
3. **`entry_index` threaded into `calculate_entry_display_rows`**: required so the frame-line width helper can look up `link_highlight_state` the same way the formatter does (`(entry_index, frame_index)` lookup) — badges are real text that changes wrap geometry. Both call sites already had `idx` in scope, so this was a signature-only change with no new state needed.

### Testing Performed

- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo fmt --all -- --check` — Passed (no diff)
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo clippy -p fdemon-tui --all-targets -- -D warnings` — Passed (zero warnings)
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo check --workspace --all-targets` — Passed
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo test -p fdemon-tui` — Passed (1605 passed, 1 ignored, 0 failed; 7 doctests passed)
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo test -p fdemon-app` — Passed (3370 passed, 5 ignored, 0 failed; 2 doctests passed, 3 ignored)

### Risks/Limitations

1. **Hot-path cost**: as noted in BUG.md, the estimate now walks stack-frame text (grapheme measurement) per trace-bearing entry per frame, versus the prior O(1) count. This is bounded by the sparsity of trace-bearing entries in real buffers and is allocation-free; if profiling ever shows this hot, the deferred per-entry row-count cache (BUG.md "Further Considerations") is the documented remedy.
2. **Message-line badges remain unmeasured**: link badges inserted into *message*-line text (not stack-frame lines) are not accounted for by the message-line estimate — a narrower, pre-existing, link-mode-only inaccuracy explicitly called out as out-of-scope in BUG.md and folded into the deferred cache follow-up.
