# Action Items: Phase 4 — Jump-to-Latest Log Affordance

**Review Date:** 2026-05-29
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3 (MAJOR)

## Critical / Major Issues (Must Fix)

### 1. Pill mouse-click is shadowed by the log-row click region
- **Source:** logic_reasoning_checker (N6) + orchestrator verification
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs` (pill region push from `:1643` → `:1870`; row region `:1712`)
- **Problem:** `hit_test` (`mouse_regions.rs:197`) picks the last-pushed entry at equal `z_index`. The pill region (z=0) is pushed before the per-row `ClickLogRow` regions (z=0), so a click on the pill resolves to `ClickLogRow`, not `ScrollToBottom`. AC 02-6 is functionally unmet.
- **Required Action:** Register the pill region with `ctx.click_at_z(rect, MouseAction::emit(Message::ScrollToBottom), 1)` (or push it after the row loop).
- **Acceptance:** A test calls `regions.hit_test(pill_x, pill_y, MouseButton::Left)` and asserts it returns the `ScrollToBottom` action (not `ClickLogRow`), with a log present on the pill's row.

### 2. `clear_logs` does not reset `unseen_log_count`
- **Source:** architecture_enforcer, code_quality_inspector + orchestrator verification
- **File:** `crates/fdemon-app/src/session/session.rs:403-410`
- **Problem:** After `clear_logs` while scrolled up, the pill renders `↓ N new` over an empty buffer.
- **Required Action:** Add `self.unseen_log_count = 0;` in `clear_logs`.
- **Acceptance:** New test: set `auto_scroll = false`, `add_log` ×N, `clear_logs()`, assert `unseen_log_count == 0` and `logs.is_empty()`.

### 3. `handle_page_down` missing false→true transition guard
- **Source:** all five agents
- **File:** `crates/fdemon-app/src/handler/scroll.rs:62-68`
- **Problem:** Paging to the natural bottom flips `auto_scroll` true but never calls `mark_tail_followed()`, leaving a stale counter that resurfaces a wrong count on the next scroll-up. Deviates from task 01 line 99's explicit directive.
- **Required Action:** Capture `was_following` before `page_down()`; call `mark_tail_followed()` on the false→true transition. Consider a shared `clear_pill_if_reengaged(handle, was_following)` helper reused by `handle_scroll_down`, `handle_page_down`, and any `handler/mouse/` wheel-down path.
- **Acceptance:** New test `handle_page_down_clears_unseen_count_on_natural_follow`.

## Minor Issues (Should Fix)

### 4. Counter ignores active log filter
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/session/session.rs:376-381` (vs. filter `:711`)
- **Decision required:** Either gate the increment on `self.filter_state.matches(&entry)` (preferred), or expand the field doc comment to record filter divergence as an accepted limitation. Track on issue #31.

### 5. Missing `///` doc on `LogView::unseen_log_count` builder
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:181` — add a doc comment matching sibling builders.

### 6. Raw `u16` arithmetic in pill coordinates
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:1857-1858` — use `saturating_add`/`saturating_sub` to match surrounding style.

### 7. `\u{...}` escapes instead of literal glyphs
- **File:** pill constants in `mod.rs` — use `↓` and `·` literals.

## Minor / Nitpicks (Consider)

- Add a pill + scrollbar co-render layout test.
- Add narrow-terminal boundary tests at `width == pill_width` and `pill_width + 1`.
- Consider `unicode-width` for `pill_width` if already a workspace dependency.
- Drop the trivial `make_logs` / `default_icons` test wrappers or fold into existing helpers.
- Consider consolidating the new inline `session.rs` test module with the existing `session/tests.rs`.

## Re-review Checklist

- [ ] M1 resolved — pill hit-test resolves to `ScrollToBottom` (test asserts win, not just existence)
- [ ] M2 resolved — `clear_logs` zeroes the counter (tested)
- [ ] M3 resolved — `handle_page_down` clears on natural follow (tested)
- [ ] m1 decided — filter semantics fixed or documented
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
