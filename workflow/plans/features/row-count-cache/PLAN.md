# Plan: per-entry display-row-count cache for the log view (issue #75)

## TL;DR

In wrap mode the scroll-bounds pass grapheme-walks the ENTIRE filtered buffer every frame (~20 fps) to compute `total_lines`. Research shows an entry's row count depends on exactly two frame-variable inputs — expanded state and link badges — so a lookup-time-keyed cache needs **zero handler-side invalidation wiring**: global key `(width, wrap_mode)`, per-entry key `expanded`, and exact formatted-line measurement for the ≤35 viewport-scoped linked entries while link mode is active. This also closes the last #73-class drift (message-line badges) and takes the two small deferred alignments.

---

## Background

#74 made `calculate_entry_display_rows` exact but kept it O(filtered buffer) per frame (message + frame-line grapheme walks, mod.rs:1785-1795 loop). Steady-state buffers don't change frame to frame, so ~all of that work is redundant. Deferred from #73's review: this cache, message-line badge measurement, constant derivation, collapse-helper extraction (issue #75).

## Affected Modules

- `crates/fdemon-app/src/log_view_state.rs` — cache storage on `LogViewState` (render-written, same `&mut` StatefulWidget shape as the approved `selection_text` fields).
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — cache consult/populate in the two `calculate_entry_display_rows` call paths; exact formatted-line path for linked entries; constants derivation; collapse-visibility helper.
- `docs/REVIEW_FOCUS.md` — registry entry for the new render-written fields (**doc_maintainer task**; implementor may not edit).

## Development Phases

### Phase 1: cache + exact linked-entry path (single implementation task)

**Goal:** `total_lines` cost becomes O(changed entries) in steady state, exact everywhere including link mode.

#### Steps

1. **Cache storage** on `LogViewState`:
   - `row_cache: HashMap<u64, CachedRows>` where `CachedRows { expanded: bool, rows: u16 }`
   - `row_cache_key: Option<(u16 /*content width*/, bool /*wrap_mode*/)>`
   - Cleared wholesale when the global key mismatches at render start. Safe defaults (empty/None) per the render-hint rules.
2. **Lookup path** (render_inner, both the total_lines loop and the render loop — they share it):
   - Entry has any link (`links.iter().any(|l| l.entry_index == idx)`, only when `link_highlight_state` is active): **bypass cache**, measure exactly by formatting the entry's lines (message via `format_entry`, frames/indicator via the existing formatters) and summing `line_wrapped_row_count`. Bounded: ≤35 links, all visible. Do not write these to the cache.
   - Otherwise: hit if `cached.expanded == is_entry_expanded(entry)` → reuse `rows`; miss → compute via the existing exact iterators (#74) and store.
3. **Pruning:** when `row_cache.len()` exceeds a threshold (e.g. `2 * filtered_len + 64`), `retain(|id, _| *id >= front_id)`. Memory hygiene only — evicted ids are never queried.
4. **Message-line badge exactness** falls out of step 2's bypass (no injection math; per-span badge-suppression corner handled by construction).
5. **Small alignments** (same task): derive the `4`/`25` literals in `frame_line_widths` from `styles::INDENT.len()` / `"<asynchronous suspension>".len()`; extract the collapse-visibility computation (`is_expanded`/`visible_frames`/`has_indicator`) into a helper shared by `calculate_entry_display_rows` and `calculate_entry_lines` (also trims the 60-line function).

**Milestone:** steady-state wrap-mode frames do zero grapheme work on unchanged, unlinked entries; #74's invariant test still green; link-mode estimate now exact for message badges too.

### Phase 2: REVIEW_FOCUS.md registry entry (doc_maintainer)

Document `row_cache` + `row_cache_key` in the "Current usage" list: render-written plain fields on `&mut LogViewState`, numeric-per-entry hints, safe empty defaults, lookup-time keying rationale, and the note that `show_timestamps`/`show_source` (currently hardcoded true) must join the global key if ever wired to config.

## Edge Cases & Risks

### Stale rows on unforeseen content change
- **Risk:** some input outside (expanded, links, width, wrap) changes rendered chars.
- **Mitigation:** research enumerated the inputs — level is style-only, search is style-only, message/trace immutable, timestamps/source hardcoded. The #74 invariant test (estimate == rendered) plus new cache-specific tests are the backstop; a cache-coherence test renders twice with a toggled input and asserts equality.

### Link-mode filter staleness (pre-existing, out of scope)
- `rescan_links_if_active` doesn't run on filter changes, so links (raw indices into the filtered view) can go stale in link mode **today**, badges included. The cache bypass uses the same (possibly stale) links the formatter uses, so estimate == render still holds. File the rescan gap as its own issue; do not fix here.

### Eviction + raw link indices (pre-existing, noted)
- Raw `entry_index` shifts on `pop_front` eviction while link mode is active — badges can attach to wrong entries (formatter and estimate identically). Same bucket as above; out of scope.

## Success Criteria

### Phase 1 Complete When:
- [ ] Steady-state test: second render with unchanged state performs zero recomputation (observable via a test-only counter or by asserting cache hits/len)
- [ ] Cache-coherence tests: width change, wrap toggle, expand/collapse toggle, link-mode enter/exit, eviction — each followed by estimate == rendered rows (reuse #74's `rendered_row_count` ground truth)
- [ ] Link-mode message-badge test: message-level link at a wrap boundary → estimate == rendered (this was the deferred #73-class drift)
- [ ] Pruning test: after eviction past threshold, cache contains no ids < front id
- [ ] All #74 tests (invariant, regression, scroll-bounds, frame-badge, region alignment) still green; fmt/clippy (1.96 and 1.97) clean
- [ ] No allocations added to the non-linked steady-state path

### Phase 2 Complete When:
- [ ] REVIEW_FOCUS.md registry entry present, following the existing entries' format

## Further Considerations

1. **File separately after this lands:** link rescan-on-filter-change gap (+ raw-index eviction shift) — pre-existing link-mode staleness, both formatter and estimate affected equally.

## Task Dependency Graph

T1 (implementation, `medium`) → T2 (doc_maintainer, `low`, depends on T1). Both sequential in the main loop; no file overlap (T1: code files; T2: docs/REVIEW_FOCUS.md only). Branch `feat/75-row-count-cache` is stacked on #74's branch — rebase onto main after #74 merges, before opening the PR.
