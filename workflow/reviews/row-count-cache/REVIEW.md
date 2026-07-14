# Review: row-count-cache (issue #75)

**Verdict:** ✅ APPROVED_WITH_CONCERNS (round 0, terminal)
**Diff range:** `eeb87633..671ddb37` · **Reviewed HEAD:** 671ddb37 (+ inline polish 261609fd, doc addendum d14be711)
**Run:** review-diff workflow (5 dimensions, adversarial refuter panel) — 0 confirmed Critical/Major, 16 Minors (several duplicates/positive notes)

## Per-dimension

| Dimension | Verdict | Confirmed | Minors |
|-----------|---------|-----------|--------|
| architecture | PASS | 0 | 2 |
| quality | PASS | 0 | 4 |
| logic | PASS | 0 | 4 |
| risks | CONCERNS (minors only) | 0 | 5 |
| security | PASS | 0 | 1 |

## Minors — disposition

**Fixed inline post-review (commit 261609fd):**
1. Empty-buffer cache staleness (dedup of 3 findings): the reviewer flagged the `unwrap_or(0)` prune fallback as a no-op; the actual hole was one level up — the `logs.is_empty()` early return skips the pruning pass entirely. Fix: clear `row_cache` in that early return (filter-matches-nothing path deliberately keeps the cache — ids still live). Regression test `test_row_cache_cleared_when_buffer_empties` (failed against both the original code AND the reviewer's suggested fix location; green now). The prune-pass fallback also replaced with a match (defensive).
2. Stale "registry entry pending (task 02)" doc comments ×3 → reworded to point at the landed REVIEW_FOCUS.md entry.
3. `row_cache_key` second element hardcoded `true` → writes `self.wrap_mode`.
4. Bare `4` in `collapsed_indicator_widths` → `FRAME_INDENT_WIDTH`.
5. `debug_assert` at the u16 rows insert (silent-saturation landmine now fails loudly in test builds).

**Documented (doc addendum d14be711):**
6. `max_collapsed_frames` is a hidden row-count input — compile-time constant 3 in production; registry entry now lists it as assumed-constant (must join the global key if ever wired to config).
7. Pruning bound is filtered-view-relative, not buffer-relative → noted in the registry entry, with the empty-buffer clear behavior.

**Accepted, no action:**
8. Render-only cache shape (no handler reads) — registry entry covers this variant.
9. u16 saturation unreachable at real widths (debug_assert added anyway).
10. Pre-existing link `entry_index` staleness correctly carried forward, not worsened — separate issue to file (with the rescan-on-filter gap).
11. `log_view/mod.rs` at 2.6k lines (pre-existing; `row_cache.rs` split candidate next touch). REVIEW_FOCUS.md over its size cap (pre-existing; schedule docs compaction).

## Verification highlights from the review

Reviewers confirmed: both render_inner call sites share `entry_display_rows_cached`; the linked-entry bypass condition matches the formatters' badge conditions; the cache-miss path is byte-identical to #74's math; the sentinel and message-badge tests are honest (mutation-verified by the implementor); hit path does no grapheme work or allocation.
