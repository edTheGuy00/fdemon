# Research: wrap-mode scroll bounds drift (issue #73)

Research-sweep synthesis (2026-07-09, branch `fix/73-wrap-scroll-bounds` @ 7f66caee). 3/3 questions answered; every load-bearing claim adversarially verified. Corrections from the verify pass are folded in below.

## Q1 — Call graph & frequency of `calculate_entry_display_rows`

- Wrap mode, **every frame**: called over **ALL** `filtered_indices` at `mod.rs:1625-1629` to compute `total_lines`, then again in the render loop at `mod.rs:1673` for every iterated entry (loop `break`s once the viewport is filled, so entries past the visible window are only computed once; entries above/into the window are computed twice).
- Per call it already runs `grapheme_cell_widths(&entry.message)` + `wrapped_row_count_widths` (`mod.rs:759-763`) — O(message length) — then counts stack-frame lines as **1 row each** via `calculate_entry_lines` (`mod.rs:765`, rules at `692-708`). That 1-row assumption is the bug.
- The render loop measures the same visible lines **exactly**: `line_wrapped_row_count` on the fully formatted line — message at `mod.rs:1721`, frame lines at `1785` and `1850`.
- `total_lines` consumers: `state.update_content_size()` (`mod.rs:1638` → `log_view_state.rs:505-510`, drives `max_offset`), scrollbar decision + proportions (`mod.rs:1980`).
- `visible_width = content_area.width` (`mod.rs:1620`), same number everywhere in the frame (`wrap_line_chars` at `1957` recomputes the identical value locally — cosmetic).
- **Hot-path reality**: the whole-buffer per-frame walk already exists for message text. Frame lines add cost only on entries that have stack traces.

## Q2 — Exact rendered shape of a stack-frame line

Production formatter: `format_stack_frame_line_with_links` (`mod.rs:559-654`; the similar `format_stack_frame*` at `489-553` is test-only dead code — do not mirror it).

- **Normal frame**: `INDENT(4 sp, styles.rs:39)` + `#` + `{:<3}` frame number + function name + `" ("` + **optional 3-char link badge `[c]`** + `frame.short_path()` + `:` + line number + optional `:column` (only when `frame.column > 0`) + `)`.
  - Frame-number field is a **minimum** width: 4 prefix chars for numbers 0–999, wider for ≥1000 (`{:<3}` doesn't truncate). Width formula must use `1 + max(3, digits(n))`.
  - The badge is inserted (`mod.rs:625-627`) **only when** `link_highlight_state` is active AND that `(entry_index, frame_index)` has a `DetectedLink`. Badges are real characters — they DO change wrap geometry (a researcher claim to the contrary was refuted).
- **Async-gap frame** (`is_async_gap`): `INDENT` + `<asynchronous suspension>` only (`mod.rs:567-572`).
- **Collapsed indicator line**: `INDENT` + `▶ ` + `N more frame(s)...` (`mod.rs:657-679`), rendered/counted as one logical line when `frame_count > max_collapsed_frames`.
- **Collapse rules** (`mod.rs:692-708`): expanded → 1 + frame_count lines; collapsed → 1 + min(max_collapsed_frames, frame_count) + (1 indicator if truncated).
- Character content depends ONLY on: link badge presence, `frame.column > 0`, `is_async_gap`, expand/collapse state. Search/selection/focus/`is_package_frame` are style-only (`focus_info` is a render OUTPUT, not an input).
- Frame lines carry **no** timestamp/source prefix — `estimate_prefix_width` applies to message lines only (`mod.rs:731-748`).

## Q3 — Caching feasibility (decision: DEFER)

- `LogEntry` is **not fully immutable**: `Session::add_log` retroactively mutates `level` over a range (`session.rs:~362`, Logger block detection). `message`/`stack_trace` are never mutated post-ingestion.
- A per-entry cache would need: a REVIEW_FOCUS.md registry entry (required for any new render-written field; the existing `selection_text` precedent does NOT auto-extend), a global key (width, wrap_mode), per-entry invalidation (expand/collapse via `ToggleStackTrace*`, eviction pruning, level-mutation ranges), and link-mode bypass.
- `LogEntry.id` is a stable unique `u64` (`fdemon-core/types.rs:35,47,63`); `HashMap<u64, _>`/`HashSet<u64>` is idiomatic (`CollapseState`).
- **Conclusion**: cache is viable but is a perf enhancement with real invalidation surface; the exact estimate is affordable without it because traces are sparse. Defer to a follow-up issue.
