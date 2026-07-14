# Task 01: Per-entry display-row-count cache + exact linked-entry path

Implements https://github.com/edTheGuy00/fdemon/issues/75. Read ../PLAN.md and ../research/RESEARCH.md FIRST — the design is fully decided there; this file is the operational spec.

Repo: /home/ed/Dev/personal/fdemon-pro/fdemon, branch `feat/75-row-count-cache` (already checked out; stacked on PR #74).
Build/tests: `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo test -p fdemon-tui` / `-p fdemon-app`; gates also include `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` AND `cargo +1.97.0 clippy --workspace --all-targets -- -D warnings` (toolchain installed).

## Changes

### A. Cache storage — `crates/fdemon-app/src/log_view_state.rs`

New plain fields on `LogViewState` (same render-written `&mut` StatefulWidget shape as the existing selection fields — do NOT use Cell):
- `pub row_cache: HashMap<u64, CachedRows>` with `pub struct CachedRows { pub expanded: bool, pub rows: u16 }`
- `pub row_cache_key: Option<(u16, bool)>` — (content width, wrap_mode)
Safe defaults: empty map / None. Doc comments must state: render-written; lookup-time keyed; REVIEW_FOCUS.md registry entry pending (task 02).

### B. Cache consult/populate — `crates/fdemon-tui/src/widgets/log_view/mod.rs`

In `render_inner`, wrap-mode only:
1. At render start (where `visible_width` is known): if `state.row_cache_key != Some((visible_width as u16, wrap_mode))` → `row_cache.clear()`, set key. (Nowrap mode: leave cache untouched — nowrap uses `calculate_entry_lines`, not measured widths.)
2. Replace the two direct `calculate_entry_display_rows` calls (total_lines loop + render loop) with a cached wrapper (helper fn or inline closure; both call sites MUST share it):
   - **Linked entry** (only when `self.link_highlight_state` is `Some` and `links.iter().any(|l| l.entry_index == idx)`): bypass cache — measure exactly by formatting the entry's actual lines (message via `format_entry(entry, idx)` — which inserts the message badge — frames via `format_stack_frame_line_with_links`, indicator via `format_collapsed_indicator`, respecting expand/collapse) and summing `line_wrapped_row_count`. Do NOT insert into cache. (≤35 links, all visible — bounded allocations.)
   - **Otherwise**: `expanded = is_entry_expanded(entry)`; hit iff cache entry exists with matching `expanded` → reuse; miss → compute via the EXISTING exact iterator path (#74's `calculate_entry_display_rows` internals) and insert `{expanded, rows}`.
3. Pruning: after the total_lines loop, if `row_cache.len() > 2 * filtered_indices.len() + 64`, `retain(|id, _| *id >= front_id)` where `front_id = self.logs.front-most id` (logs are ordered; take from the first entry, or pass in). Keep it allocation-free.
4. The cached `rows` MUST be exactly what `calculate_entry_display_rows` returns today — no behavior change for cache misses.

### C. Small alignments (same commit or a second commit, your call)

- `frame_line_widths`: replace the bare `4` INDENT literals with a const derived from `styles::INDENT` (e.g. `styles::INDENT.len()` — INDENT is 4 ASCII spaces so len==width) and `25` with `"<asynchronous suspension>".len()`.
- Extract the collapse-visibility computation (`is_expanded` / `visible_frames` / `has_indicator` / `hidden_count`) into one private helper used by BOTH `calculate_entry_display_rows` and `calculate_entry_lines` (dedupes the rules; trims the 60-line function).

## Tests (crates/fdemon-tui/src/widgets/log_view/tests.rs, existing style; reuse `rendered_row_count` ground truth from #74)

1. **Steady-state hit test**: render twice with identical state; after the second render assert the cache is populated and (via a probe: e.g. mutate a cached `rows` value to a sentinel between renders and assert `total_lines` reflects the sentinel) prove the second render READ the cache rather than recomputing. Then invalidate (width change) and assert the sentinel is gone (recomputed correctly).
2. **Coherence tests** — for each of: width change, wrap toggle, expand/collapse toggle (`CollapseState`), link-mode enter (badge appears) and exit, eviction of cached entries: perform the change, re-render, assert estimate == `rendered_row_count` per entry and correct `total_lines`.
3. **Message-badge exactness** (the deferred #73 drift): message-level link whose badge pushes the message line across a wrap boundary → estimate == rendered while link mode active (this test MUST fail if the linked-entry bypass is removed — verify by temporarily disabling it, then restore).
4. **Pruning test**: with a small `max_logs`-style setup (drive eviction via ids), grow the cache past threshold, render, assert no cached id < front id and live ids survive.
5. All #74 tests stay green (invariant, regression, scroll-bounds, frame-badge-boundary, region-alignment).

## Constraints

- No allocations on the non-linked steady-state path (cache hit = one HashMap get; miss = existing iterator math + one insert).
- Zero behavior change to rendered output — this is measurement/caching only.
- No edits to `docs/REVIEW_FOCUS.md` (task 02, doc_maintainer) or any other core doc.
- Gates: fmt, clippy on BOTH default and +1.97.0 toolchains, full `-p fdemon-tui` and `-p fdemon-app` suites.
- Commit source only: `git add --all -- . ':!workflow/plans/'`; message `perf(tui): cache per-entry display row counts in wrap mode (#75)` (second commit for the alignments OK: `refactor(tui): derive frame-line constants, share collapse-visibility helper`).

## Acceptance criteria

- [x] Steady-state render does zero row recomputation for unchanged unlinked entries (proven by the sentinel probe test)
- [x] All coherence tests pass; message-badge exactness test passes and is load-bearing
- [x] Pruning bounded; no stale ids after threshold pass
- [x] All #74 tests green; both-toolchain clippy + fmt clean; full suites green
- [x] No REVIEW_FOCUS.md/doc edits; commits exclude workflow/plans

---

## Completion Summary

**Status:** Done
**Branch:** `feat/75-row-count-cache`
**Commit:** `59f76f3c` — `perf(tui): cache per-entry display row counts in wrap mode (#75)`

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/log_view_state.rs` | Added `pub struct CachedRows { expanded: bool, rows: u16 }` and two new `LogViewState` fields: `pub row_cache: HashMap<u64, CachedRows>`, `pub row_cache_key: Option<(u16, bool)>`. Safe defaults (`HashMap::new()` / `None`) wired into `LogViewState::new()`. Doc comments state render-written / lookup-time-keyed / REVIEW_FOCUS.md entry pending (task 02), per spec. |
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Added `collapse_visibility()` (shared `is_expanded`/`visible_frames`/`has_indicator`/`hidden_count` computation, now used by `calculate_entry_lines`, `calculate_entry_display_rows`, and the new `measure_entry_exact_rows`). Added `measure_entry_exact_rows()` (exact bypass measurement for linked entries: `format_entry` + `format_stack_frame_line_with_links` + `format_collapsed_indicator`, summed via `line_wrapped_row_count`). Added `entry_display_rows_cached()` — the single cached-lookup helper shared by both `render_inner` call sites (total_lines loop and the render loop's `entry_units`): bypasses the cache for linked entries, otherwise hits on matching `expanded` state or misses into `calculate_entry_display_rows` + insert. Added cache-key check/clear at render start (wrap mode only) and a pruning pass (`retain(id >= front_id)`) after the total_lines loop when `row_cache.len() > 2 * filtered_len + 64`. Section C alignments: `frame_line_widths`'s two bare `4` literals replaced by `FRAME_INDENT_WIDTH = styles::INDENT.len()`, and `25` by `ASYNC_GAP_TEXT_LEN = "<asynchronous suspension>".len()`. |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | 8 new tests (see below) plus 3 shared test helpers: `row_cache_test_view`, `expected_total_lines`, `row_cache_mixed_logs`. |

### New tests (all passing)

- `test_row_cache_sentinel_probe_proves_second_render_reads_cache` — sentinel-probe proof: overwrites a cached `rows` value between two identical renders and asserts `total_lines` reflects the sentinel exactly on the second render (proving the cache is read, not recomputed), then invalidates via width change and asserts both the sentinel is gone and the recomputed total matches a fresh render.
- `test_row_cache_coherent_after_width_change`
- `test_row_cache_coherent_after_wrap_toggle`
- `test_row_cache_coherent_after_expand_collapse_toggle`
- `test_row_cache_coherent_after_link_mode_enter_and_exit`
- `test_row_cache_coherent_after_buffer_eviction`
- `test_row_cache_message_badge_exactness_at_wrap_boundary` — load-bearing (see proof below).
- `test_row_cache_pruning_drops_stale_ids_past_threshold`

### Load-bearing verification (manual, not left in the codebase)

1. **Sentinel probe**: temporarily short-circuited the cache-read branch in `entry_display_rows_cached` (`if false { ... }` around the `state.row_cache.get(&entry.id)` lookup) — `test_row_cache_sentinel_probe_proves_second_render_reads_cache` failed with `left: 6, right: 10004` (the un-sentineled recomputed total vs. the expected sentinel-corrupted total), confirming the test actually detects a cache-bypassed implementation. Reverted; full suite green again.
2. **Message-badge bypass**: temporarily forced `is_linked = false && ...` in `entry_display_rows_cached` — `test_row_cache_message_badge_exactness_at_wrap_boundary` failed with `estimate=1 rendered=2` (the un-bypassed cached estimator missed the badge-induced wrap), confirming the test is load-bearing. Reverted; full suite green again.

### Notable Decisions/Tradeoffs

1. **`total_lines` loop uses a plain `for` loop instead of `.map().sum()`**: `entry_display_rows_cached` needs `&mut state` per call; threading a unique borrow through a `.map` closure across iterations is legal via implicit reborrow at call sites but is less readable than an explicit loop for this mutable-accumulator case. Chose clarity over iterator-chaining purity here.
2. **`CachedRows.rows: u16` uses a saturating (`u16::try_from(...).unwrap_or(u16::MAX)`) cast on insert**, per the spec's literal field type. A single log entry occupying >65535 wrapped terminal rows is not a realistic scenario (documented inline); a fresh cache-miss render in that pathological case would still be numerically correct for that one frame, only a subsequent cache *hit* would read back the saturated value. Not exercised by any test — flagged here as a known, spec-mandated limitation rather than silently deviating from the given field type.
3. **Section C alignment scope**: only `frame_line_widths`'s two bare literals were touched (`4`→`FRAME_INDENT_WIDTH`, `25`→`ASYNC_GAP_TEXT_LEN`), per the task's explicit scoping to that function. `collapsed_indicator_widths` has a similar bare `4` INDENT literal but was left untouched — out of the task's stated scope, not accidental.
4. **Combined both task changes (cache + alignments) into a single commit**: the spec explicitly allowed this ("same commit or a second commit, your call").

### Testing Performed

- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo fmt --all -- --check` — Passed (after running `cargo fmt --all` once to apply 2 formatting fixes)
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo check --workspace --all-targets` — Passed
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo clippy --workspace --all-targets -- -D warnings` (default toolchain) — Passed, 0 warnings
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73-1970 cargo +1.97.0 clippy --workspace --all-targets -- -D warnings` — Passed, 0 warnings
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo test -p fdemon-tui` — Passed, 1614 passed / 0 failed / 1 ignored (was 1606; +8 new)
- `CARGO_TARGET_DIR=/data/cache/target/fdemon-73 cargo test -p fdemon-app` — Passed, 3370 passed / 0 failed / 5 ignored (unchanged — no fdemon-app test-suite changes beyond the new `CachedRows`/field additions, which are covered indirectly by the fdemon-tui render tests)
- All pre-existing #74 tests (invariant, regression, scroll-bounds, frame-badge-boundary, region-alignment, click-region alignment) — still green, unmodified

### Risks/Limitations

1. **`u16` row-count truncation edge case** (see Decision #2 above): purely theoretical at realistic terminal widths and message lengths; no test exercises it since triggering it would require an entry occupying tens of thousands of wrapped rows.
2. **Pre-existing link-mode staleness** (documented in RESEARCH.md/PLAN.md, out of scope for this task): `rescan_links_if_active` doesn't run on filter changes, and raw `entry_index` link indices can drift on `pop_front` eviction while link mode is active. The cache bypass uses the same (possibly stale) links the formatter uses, so estimate == render still holds — this task does not change that pre-existing behavior.
