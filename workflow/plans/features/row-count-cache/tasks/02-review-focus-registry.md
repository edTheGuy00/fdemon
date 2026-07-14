# Task 02: REVIEW_FOCUS.md registry entry for the row-count cache

Depends on task 01 (read its committed diff first: `git log --oneline -3`, `git show` the cache commit). Context: ../PLAN.md Phase 2, ../research/RESEARCH.md.

Repo: /home/ed/Dev/personal/fdemon-pro/fdemon, branch `feat/75-row-count-cache`.

## Change

Edit `docs/REVIEW_FOCUS.md` ONLY. In the approved render-hint exception's "Current usage" list (where the `LogViewState` drag-selection geometry and `selection_text` entries from PR #72 live), add one entry for:

- `LogViewState::row_cache` + `LogViewState::row_cache_key` — per-entry wrapped display-row-count cache, written by the log-view renderer during `render_inner` (wrap mode). Cover, matching the format and depth of the neighboring entries:
  - Shape: plain fields on `&mut LogViewState` via StatefulWidget (same shape as `SettingsViewState::scroll_offset` / the selection fields; not `Cell`).
  - Keying rationale (the reviewer-relevant invariant): NO handler-side invalidation exists by design — coherence comes from lookup-time keys: global `(content width, wrap_mode)` clears the map; per-entry `expanded` compared on every hit; entries with active links bypass the cache entirely and are measured from formatted lines (links are viewport-scoped, ≤35).
  - Why that is sound: rendered char content depends only on immutable entry text + expanded + badges (level and search affect style only; timestamps/source hardcoded true).
  - Future-key note: if `show_timestamps`/`show_source` are ever wired to config (`UiSettings::show_timestamps` exists, unwired), they MUST join the global key.
  - Safe defaults: empty map / `None` (no render yet → all misses, correct).
  - Pruning: opportunistic retain of ids ≥ front entry id (memory hygiene, not correctness).

Keep it to one list entry consistent in length/tone with the existing ones. Do not restructure the document, do not touch any other section or file.

## Acceptance criteria

- [ ] Single new entry in the "Current usage" list, format-consistent with neighbors
- [ ] States the lookup-time-keying invariant, the link bypass, the future config-key note, safe defaults
- [ ] No other files or sections changed
- [ ] Commit: `docs(review-focus): register LogViewState row-count cache render-hint fields (#75)` (exclude workflow/plans)
