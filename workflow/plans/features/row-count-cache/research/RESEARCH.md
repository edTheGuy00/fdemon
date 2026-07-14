# Research: per-entry row-count cache (issue #75)

Two targeted codebase researchers (2026-07-14, branch `feat/75-row-count-cache` @ eeb87633, stacked on PR #74). Supplements the verified #73 sweep (`../../bugs/wrap-scroll-bounds-drift/research/RESEARCH.md`), which already covers the call graph, frame-line shapes, and eviction/level-mutation sites.

## Q1 — What can change an entry's rendered char content between frames?

- **No level icon exists in the line text** — `entry.level` affects styling only (`format_entry`, mod.rs:407-458; `_level_style` unused in span text). ⇒ **The `add_log` level retro-mutation is irrelevant to row counts** and drops out of the cache key.
- Message char content is immutable post-ingestion; search highlighting splits spans but changes **style only, not chars** (`format_message_with_highlights`).
- `estimate_prefix_width` (mod.rs:841-858) is **exact** vs `format_entry` for all levels (all-ASCII prefix).
- `show_timestamps`/`show_source`: builder methods exist but the render path never calls them — **hardcoded true** (render/mod.rs:256+; config field `UiSettings::show_timestamps` exists, unwired). Treat as constants; if ever wired, they become global-key inputs (document in REVIEW_FOCUS entry).
- `is_entry_expanded`: O(1) HashSet lookup (`CollapseState`, collapse.rs:23-31; Session field session.rs:107), available where `calculate_entry_display_rows` runs.
- Eviction pruning signal: `logs.front().map(|e| e.id)` — ids are process-global monotonic (`LOG_ENTRY_COUNTER`), so `id < front_id` ⇔ evicted.

⇒ Per-entry variability reduces to exactly two inputs: **expanded state** and **link badges**.

## Q2 — Link badge semantics (`LinkHighlightState`)

- `DetectedLink.entry_index` is a **RAW VecDeque index** (scan_viewport, hyperlinks.rs:285-380 stores the raw `idx` from `filtered_indices`), matching the render loop's `idx` — consistent with #74's frame badge lookup.
- Links list: **max 35** (`MAX_LINK_SHORTCUTS`), **viewport-scoped** — only visible entries ever carry badges.
- Rebuilt ONLY on: link-mode entry (`EnterLinkMode`, update.rs:953-981) and every scroll (`rescan_links_if_active`, scroll.rs:128-155). NOT on filter change (**pre-existing staleness bug** — file separately) and NOT on new-log arrival. Cleared on exit (`deactivate()` clears `links`).
- Message badge: 3 chars `[c]` spliced **mid-line** before the matched `display_text` (insert_link_badge_into_spans, mod.rs:362-404 — per-span text search, so a search-highlight span split can suppress the match).
- Ownership: `Session.link_highlight_state` (session.rs:110), passed to LogView by reference only when active (render/mod.rs:266-269).

## Design consequence (validated)

- **Non-linked entries**: rows depend only on (message text, trace shape, expanded, width, wrap_mode) → cacheable with per-entry key `expanded: bool` and global key `(width, wrap_mode)`. No handler wiring needed anywhere — everything is checkable at lookup time.
- **Linked entries** (≤35, always visible, only while link mode active): measure **exactly from formatted lines** (`format_entry` + `format_stack_frame_line_with_links` + `line_wrapped_row_count`) instead of badge-width injection math — bounded allocations, byte-exact with the renderer by construction (including the search-split badge-suppression corner that injection math would get wrong).
- Pruning: opportunistic `retain(id >= front_id)` when the map outgrows a threshold; stale ids are never queried, so this is memory hygiene, not correctness.
