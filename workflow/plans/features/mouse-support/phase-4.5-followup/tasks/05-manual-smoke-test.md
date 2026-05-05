# Task 05: Manual Smoke Test (Live Flutter Session, macOS)

## Goal

Run the Phase 4 manual smoke test on a live Flutter session on macOS and record the results in this task's Completion Summary. The smoke test exercises the full mouse-only walk-through across all five Phase-4 click surfaces, which automated tests cannot fully cover (rendering quirks, real timing, real device discovery).

**Agent:** `implementor` — but the implementor's job here is to *run* the binary and *record* what happened, not to write code. The implementor should treat this as an interactive validation step: launch fdemon, perform each step, observe the result, record success/failure with any anomalies. If a step fails, do not modify code from this task — record the failure and the orchestrator/reviewer will route a follow-up task.

## Background

Phase 4's success criteria explicitly required a manual end-to-end mouse-only walk-through. Task 10 of Phase 4 documented the test in its completion summary but did not execute it (no live Flutter device was available at the time). The review (`workflow/reviews/features/mouse-support-phase-4/REVIEW.md` Major #7) flagged this as blocking phase merge.

## Files

**Modify:**
- `workflow/plans/features/mouse-support/phase-4.5-followup/tasks/05-manual-smoke-test.md` (this file — Completion Summary section only)

**Read:**
- `workflow/plans/features/mouse-support/phase-4-log-view-devtools-clicks/TASKS.md` (Manual Smoke Test section)
- `docs/DEVELOPMENT.md` (running the binary)

## Plan

1. **Pre-flight check.** Verify a Flutter project is available locally (e.g., a sample counter app). Confirm `cargo run` from a Flutter project directory works (or `cargo run -- /path/to/flutter/project`).

2. **Build the binary** at the current `feat/mouse-support` head:
   ```bash
   cargo build
   ```

3. **Launch fdemon** against a live Flutter project (e.g., `examples/counter` if one exists, or a user-provided path).

4. **Run each step below**, recording in the Completion Summary:
   - The exact behavior observed (success / failure / unexpected detail)
   - Any anomalies (visual glitches, latency, crashes, regressions in non-mouse paths)

### Smoke Test Steps

| # | Step | Expected Result |
|---|------|-----------------|
| 1 | Click anywhere in the log area | No scroll. No crash. `last_log_click` updated (verifiable via the next step). |
| 2 | Within 400ms, click the same log row again (entry must have a stack trace) | Stack trace expands. |
| 3 | Click the same row a third time within 400ms | Stack trace collapses again (double-click on the now-expanded row). |
| 4 | Click on a different log row within 400ms of step 3 | Single-click only — no further toggle (the stamp targets a different entry_id). |
| 5 | Press `d` to open DevTools | DevTools panel renders with default active panel (Inspector). |
| 6 | Click `[p] Performance` in the sub-tab bar | Performance panel becomes active. |
| 7 | Click `[i] Inspector` | Inspector panel becomes active. |
| 8 | With a tree loaded in Inspector, click a child row | Row becomes selected; layout panel updates within ~500ms. |
| 9 | Click the leading `▶` glyph on a collapsible Inspector node | Node expands/collapses (toggles); selection follows. |
| 10 | Click `[p] Performance` again; with frames recorded, click a bar in the middle of the chart | That frame is highlighted with `▔`; detail panel shows its timing. |
| 11 | Click `[n] Network`; with requests recorded, click a row | Details appear in side panel (or below in narrow mode). |
| 12 | Click `[h] Headers` in the detail tab bar | Detail panel switches to Headers tab. |
| 13 | While in Network panel, type `/` (or the filter activation key) to enter filter input mode; then click the table area | Click is suppressed (filter input absorbs it). Filter input remains active. |
| 14 | While filter input is active, click `[i] Inspector` in the sub-tab bar | **CONTEXT-DEPENDENT.** Per the Phase 4.5 design decision, this should switch to Inspector AND exit filter input mode. (Note: Task 08 in this phase implements this carve-out; if T08 has not yet merged when running this test, expect the click to be suppressed.) |
| 15 | Quit fdemon | Clean exit. No terminal corruption. |

5. **Record results** in the Completion Summary with format:

   ```
   ### Smoke Test Results

   | # | Step | Result | Notes |
   |---|------|--------|-------|
   | 1 | Click in log area | PASS | |
   | 2 | Double-click expands stack trace | PASS | |
   | ... | | | |
   ```

6. **If any step fails**, do NOT modify code from this task. Record the failure with:
   - Exact step number
   - Observed behavior
   - Any error in the fdemon log (`tail -f` the file in `$TMPDIR`)
   - Any panic / crash details

   The reviewer will pick up failures and decide whether to add a fix task to Phase 4.5 or open a follow-up phase.

## Acceptance Criteria

- [ ] All 15 smoke test steps executed against a live Flutter session.
- [ ] Results recorded in the Completion Summary table format above.
- [ ] Any failures or anomalies clearly described, with reproduction details.
- [ ] No code modifications committed by this task.

## Notes

- **No code changes** — this task is pure validation. If the test surfaces issues, the implementor's commit only updates this task file's Completion Summary; source files are not touched.
- **Step 14** depends on whether Task 08 has merged. If running this task in parallel with T08 (in worktrees), expect the pre-T08 behavior (suppressed). Note this in the result.
- **Edge cases the test does not cover** — these are deferred to the reviewer's discretion: middle-click on regions (no production feature uses it), right-click (Phase 4 explicitly returns None), terminal resize during click sequence, very large log buffers (>10k entries) with click registration overhead.
- **Headless environment** — if no live Flutter project is available, mark this task as Blocked in the completion summary and note the reason. The orchestrator will not auto-skip this task; the user decides whether to defer or proceed.
