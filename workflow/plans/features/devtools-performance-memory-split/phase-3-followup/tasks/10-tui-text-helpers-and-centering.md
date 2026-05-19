# 10 — TUI Text Helpers Extraction + Placeholder Centering

**Wave:** 3
**Depends On:** —
**Agent:** implementor
**Estimated Hours:** 1–2h
**Addresses:** M9, L4, L5

## Context

Three related TUI cleanups in `crates/fdemon-tui/src/widgets/devtools/performance/details/`:

- **M9.** `truncate_with_ellipsis`, `pad_right`, and `pad_left` are byte-identical private helpers in both `rebuild_stats_tab.rs:303–333` and `timeline_events_tab.rs:301–327`. A near-identical implementation also exists in `widgets/new_session_dialog/mod.rs:65` (public) — consolidating with that one is out of scope (a separate refactor), but the two sibling-tab duplicates should be merged.
- **L4.** Both tab files use manual `Rect` arithmetic for placeholder centering — `let y_offset = area.height.saturating_sub(line_count) / 2; let centered = Rect { y: area.y + y_offset, ... }`. This is the exact anti-pattern named in CODE_STANDARDS Principle 2 ("Anti-pattern: manual position outside layout system"). Use `Layout::vertical` with `Constraint::Min(0)` absorbers.
- **L5.** `line_count = 3u16` in the placeholder rendering is an undocumented magic number. CODE_STANDARDS Principle 4 requires named constants with derivation comments.

## Acceptance Criteria

1. **M9 resolved.** New module `crates/fdemon-tui/src/widgets/devtools/performance/details/text_helpers.rs`:
   - `pub(super) fn truncate_with_ellipsis(s: &str, max_width: usize) -> String`
   - `pub(super) fn pad_right(s: &str, width: usize) -> String`
   - `pub(super) fn pad_left(s: &str, width: usize) -> String`
   - Module-level `//!` doc comment explaining its purpose ("shared TUI text helpers for performance details tab rendering").
   - Unit tests in an `#[cfg(test)] mod tests` block covering: empty input, exact-fit, longer-than-max truncation, Unicode (e.g., emoji width), wide grapheme clusters.
2. **M9 wiring.** Both tab files (`rebuild_stats_tab.rs`, `timeline_events_tab.rs`):
   - Remove the local private implementations of the three helpers.
   - Add `use super::text_helpers::{truncate_with_ellipsis, pad_right, pad_left};` (or equivalent).
   - All existing call sites continue to compile and behave identically.
3. **M9 module declaration.** `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` adds `pub(super) mod text_helpers;`.
4. **L4 resolved.** In both tab files, the placeholder centering:
   - Replaces the manual `Rect` construction with `Layout::vertical([Constraint::Min(0), Constraint::Length(PLACEHOLDER_LINE_COUNT), Constraint::Min(0)]).split(area)`.
   - Renders the placeholder text into the middle chunk.
   - The top and bottom `Min(0)` absorbers handle small-area cases gracefully (clip silently when space runs out).
5. **L5 resolved.** Named constant in each tab file (or in `text_helpers.rs` if shared):
   ```rust
   /// Total line count for the disabled/empty placeholder block.
   /// Derived from: header line + spacer + hint line = 3.
   const PLACEHOLDER_LINE_COUNT: u16 = 3;
   ```
   Used wherever the `3u16` magic number appeared.
6. **Tests.** Existing rendering tests in both tab files continue to pass after the helper extraction. New `text_helpers` tests pass.
7. `cargo fmt --all -- --check && cargo check -p fdemon-tui && cargo test -p fdemon-tui && cargo clippy -p fdemon-tui --all-targets -- -D warnings` all pass.
8. **Manual visual check.** Render both tabs with disabled/empty state at small terminal sizes (e.g., 80×8, 80×20, 200×30) — text remains centered, no overflow, no panic.

## Files Modified (Write)

- `crates/fdemon-tui/src/widgets/devtools/performance/details/text_helpers.rs` (NEW) — three helpers + tests.
- `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` — `pub(super) mod text_helpers;` declaration.
- `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` — remove local helpers, import from `text_helpers`, replace `Rect` arithmetic with `Layout::vertical`, name the line-count constant.
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` — same as above.

## Files Read (Dependencies)

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs:65` — read-only: confirm the third near-identical implementation. **Do NOT touch it** — consolidating with this one is a separate refactor outside scope.
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` — read-only: confirm the existing Phase-2 tab does not use these helpers (and so doesn't need updating).

## Approach Hints

- For unicode width: use `unicode-width` if already in the dependency tree (it's a common Ratatui ecosystem dep). Otherwise stick to character-count semantics and document the limitation.
- The `Constraint::Min(0)` absorber pattern is idiomatic Ratatui — see CODE_STANDARDS Principle 2 for the canonical example.
- If `PLACEHOLDER_LINE_COUNT` differs between the two tabs (e.g., one has 3 lines, the other has 5), keep the constants local to each file. If both happen to be the same number AND the layout shape is identical, extracting the constant to `text_helpers.rs` is fine.
- The existing call sites in both tab files use the helpers in the table-row formatting code (e.g., `(file:line  Name  Count)` for rebuild stats; `(thread, name, dur, ts)` for timeline). Verify call signatures match exactly — the helpers should be drop-in replacements.

## Out of Scope

- Consolidating with `widgets/new_session_dialog/mod.rs`'s text helpers. Separate refactor; touches a different module tree and would require widening visibility from `pub` to a different boundary.
- Adding new helpers (e.g., `pad_center`).
- Refactoring the placeholder content text itself.
- Changing the placeholder appearance beyond centering.
- Adding visual tests (snapshot tests) — the existing unit-test patterns suffice.
