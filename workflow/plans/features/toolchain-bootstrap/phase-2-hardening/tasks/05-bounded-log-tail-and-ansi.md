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
