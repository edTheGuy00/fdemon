# Review: wrap-scroll-bounds-drift (issue #73)

**Verdict:** ✅ APPROVED_WITH_CONCERNS (round 0, terminal)
**Diff range:** `7f66caee..fe151802` · **Reviewed HEAD:** fe151802
**Run:** review-diff workflow (6 dimensions, adversarial refuter panel) — 0 confirmed Critical/Major, 11 Minors (5 of them positive verification notes)

## Per-dimension

| Dimension | Verdict | Confirmed | Minors |
|-----------|---------|-----------|--------|
| bugfix | PASS | 0 | 4 |
| architecture | PASS | 0 | 0 |
| quality | PASS | 0 | 2 |
| logic | PASS | 0 | 3 |
| risks | PASS | 0 | 2 |
| security | PASS | 0 | 0 |

Reviewers hand-verified the width-iterator mirror against `format_stack_frame_line_with_links` span-by-span (prefix/badge/column/≥1000-frame/indicator singular-plural all exact; badge width 3 safe because shortcuts are single-width ASCII), confirmed collapse/expand parity with the render loop, and confirmed scope discipline (2 files, single commit, no fdemon-app changes).

## Minors — disposition

1. **Render loop counted collapsed indicator as 1 row while total_lines counts it wrapped** — **FIXED INLINE post-review** (commit `517b5837`): render loop now measures the indicator with `line_wrapped_row_count`; added `test_click_regions_below_wrapped_collapsed_indicator_stay_aligned`, verified red pre-fix / green post-fix (the observable symptom was click-region misalignment below a wrapped indicator, not tail clipping).
2. **Hardcoded constants** (`4` for INDENT ×3, `25` for async-gap text) instead of deriving from `styles::INDENT.len()` / literal `.len()` — DEFERRED (bounded: invariant test is the arbiter, catches drift in CI).
3. **`calculate_entry_display_rows` at 60 lines** (>50 guideline) — DEFERRED (well-segmented; optional shared collapse-visibility helper).
4. **Message-line link badge still unmeasured** (pre-existing, link-mode-only, transient) — DEFERRED; must be tracked in the per-entry-cache follow-up issue.
5. **Hot-path cost not empirically bounded** — DEFERRED; escape hatch is the cache follow-up (recommend filing that issue now).
6. **Segment-chaining grapheme assumption** (combining-mark-leading path segments) — ACCEPTED design trade-off, documented in the helper.
7–11. Positive verification notes (formatter mirror exact, collapse parity, scope, test coverage) — no action.

## Deferred items → follow-up issue (to file)

Per-entry row-count cache (perf: eliminates the whole-buffer per-frame walk), + message-line badge measurement, + constant derivation, + optional collapse-helper extraction.
