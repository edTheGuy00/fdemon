# Task 05 — Bounded log tail (VecDeque) + ANSI-safe rendering

**Agent:** implementor
**Status:** Not Started
**Depends On:** -
**Estimated Hours:** 2-3h
**Modules:** `crates/fdemon-app/src/install_wizard/types.rs`,
`crates/fdemon-app/src/install_wizard/state.rs`,
`crates/fdemon-tui/src/widgets/install_wizard/progress.rs`

## Context

The wizard's streamed log tail uses `Vec<String>` with `Vec::remove(0)` for eviction —
O(n) per line during a `git clone`/`flutter precache`. The streamed lines also originate
from `git`/`flutter` stdout/stderr and may contain ANSI escape sequences, which the
`StepProgress` widget renders directly into the Ratatui buffer.

References: `workflow/reviews/features/toolchain-bootstrap-phase-2/ACTION_ITEMS.md`
(M8, LOW-1, RESULT_SUMMARY_HEIGHT nit) and `REVIEW.md`.

## Findings to Fix

### M8 — O(n) log-tail eviction (MAJOR) — `install_wizard/state.rs` ~line 129-133, field in `types.rs`
`push_step_log` does `self.execution.log_tail.remove(0)` on a `Vec`, shifting up to
`MAX_LOG_TAIL - 1` elements per appended line.

**Fix:**
- Change `StepExecution::log_tail` from `Vec<String>` to
  `std::collections::VecDeque<String>` in `install_wizard/types.rs`.
- Update `push_step_log` to `pop_front()` when at capacity and `push_back()` (O(1)).
- Update `begin_step` (and any other constructor) to initialize `VecDeque::new()`.
- Update the `StepProgress` renderer in `progress.rs` to iterate the `VecDeque`
  (`.iter()` works the same) — fix the log-tail clipping/`rev()`/`take()` accordingly.
- Update any tests that index `log_tail` (use `.iter().nth(..)`/`.front()`/`.back()`).

### LOW-1 — ANSI escapes passed through to the TUI (NITPICK→fix) — `progress.rs` log-tail render
git progress output and flutter logs can contain ANSI/control sequences. Confirm the
log-tail render path strips or neutralizes ANSI before pushing text into the Ratatui
buffer (raw escapes can corrupt cursor/color state).

**Fix:** Run each rendered log line through the existing ANSI strip helper
(`fdemon_core::ansi` / `strip_ansi`, as used elsewhere for Flutter CLI output) before
rendering, or otherwise sanitize control characters. If a shared helper exists, reuse it;
do not reimplement.

### Nitpick — `RESULT_SUMMARY_HEIGHT` constant — `progress.rs` ~line 232-276
The result-summary row uses a bare `Constraint::Length(1)` while sibling rows use named
constants. Add `const RESULT_SUMMARY_HEIGHT: u16 = 1;` (with a one-line doc) and use it,
per CODE_STANDARDS Principle 4.

## Acceptance Criteria

- [ ] `StepExecution::log_tail` is a `VecDeque<String>`; `push_step_log` evicts via
      `pop_front` (O(1)) and the bound test (`MAX_LOG_TAIL + N` pushes → `len ==
      MAX_LOG_TAIL`, oldest dropped) still passes.
- [ ] `StepProgress` renders the `VecDeque` log tail correctly at small and large heights
      (existing progress widget tests pass; clipping unchanged in behavior).
- [ ] Rendered log lines are ANSI-sanitized (a test feeds a line containing an escape
      sequence and asserts the rendered cell text contains no raw escape bytes).
- [ ] `RESULT_SUMMARY_HEIGHT` named constant introduced and used.
- [ ] Only the three listed files are modified. `cargo fmt`/`check --workspace
      --all-targets`/`test --workspace`/`clippy -D warnings` pass.

## Notes

- Shares no write files with Task 04 (which owns `message.rs`/`actions/mod.rs`/
  `handler/*`). This task owns `install_wizard/{types,state}.rs` and the `progress.rs`
  widget — fully parallel-safe.
- Keep `MAX_LOG_TAIL` unchanged; only the data structure changes.
- If `strip_ansi` is not already re-exported where `progress.rs` can reach it, prefer
  consuming it via the existing `fdemon_core` path (TUI may depend on `fdemon-core`).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a8eba0dded3f3b8a8

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/types.rs` | Added `use std::collections::VecDeque`; changed `StepExecution::log_tail` field from `Vec<String>` to `VecDeque<String>` with updated doc comment |
| `crates/fdemon-app/src/install_wizard/state.rs` | Updated `begin_step` to use `VecDeque::new()`; updated `push_step_log` to use `pop_front()`/`push_back()` (O(1)); fixed test `first()` → `front()` and test `push()` → `push_back()` |
| `crates/fdemon-tui/src/widgets/install_wizard/progress.rs` | Added `use fdemon_core::strip_ansi_codes`; added `RESULT_SUMMARY_HEIGHT` constant (used in layout); updated `render_log_tail` to iterate via `.iter().skip(skip)` (VecDeque-compatible) with ANSI stripping; updated all test helpers from `vec![...]` to `VecDeque::from(vec![...])` for `log_tail` fields; added `test_log_tail_ansi_stripped_before_render` test |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | Updated 3 test `StepExecution` constructions from `vec![...]`/`vec![]` to `VecDeque::from(vec![...])` / `VecDeque::new()` to fix compilation (type change propagation) |

### Notable Decisions/Tradeoffs

1. **`step_detail.rs` updated despite "only three files"**: The `Vec→VecDeque` type change in `types.rs` necessarily breaks any code that constructs `StepExecution` with struct-literal `log_tail: vec![...]`. Updating `step_detail.rs` tests was required for `cargo check --workspace` to pass. The task's "only three files" constraint was interpreted as covering the implementation files; test-helper compilation fixes in collateral files are unavoidable.

2. **`VecDeque::from(vec![...])` in tests**: This is idiomatic for constructing a non-empty `VecDeque` from a literal. No helper macro was introduced since it's test-only boilerplate.

3. **ANSI stripping in `render_log_tail` only**: The stripping is applied only at render time (not at push time via `push_step_log`). This keeps the stored data faithful to the original streamed output, which is useful for debugging, while protecting the Ratatui buffer from raw escape sequences.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (all tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **No risk**: `VecDeque` supports the same `len()`, `is_empty()`, `iter()`, `front()`, `push_back()`, `pop_front()`, and `Index<usize>` operations used throughout the codebase. Behavioral equivalence is preserved.
