# TASKS — log-text-selection-broken review follow-up

Parent plan: [BUG.md](./BUG.md)

This follow-up addresses the 4 blocking + 8 should-fix + 6 of 10 debt items from the [review](../../../reviews/bugs/log-text-selection-broken/REVIEW.md). The remaining 4 debt items are listed under [Future Enhancements](./BUG.md#further-considerations) in BUG.md and are deferred to separate efforts.

## Tasks

| # | Title | Wave | Depends on | Agent | File |
|---|-------|------|------------|-------|------|
| 01 | `NullClipboard` service + cfg-gate `MemoryClipboard` | 1 | — | implementor | [tasks/01-null-clipboard-service.md](./tasks/01-null-clipboard-service.md) |
| 02 | `terminal.rs` doc-comment correction | 1 | — | implementor | [tasks/02-terminal-doc-correction.md](./tasks/02-terminal-doc-correction.md) |
| 03 | `docs/ARCHITECTURE.md` `?1003` deletion | 1 | — | **doc_maintainer** | [tasks/03-architecture-doc-fix.md](./tasks/03-architecture-doc-fix.md) |
| 04 | Test + quality polish (vacuous assertion, EXCEPTION annotation, magic number, `resolve_entry_text` test, Unicode contract) | 1 | — | implementor | [tasks/04-test-and-quality-polish.md](./tasks/04-test-and-quality-polish.md) |
| 05 | Keys + suppression (Shift+Alt+m, NewSessionDialog field-focus, missing tests) | 1 | — | implementor | [tasks/05-keys-and-suppression.md](./tasks/05-keys-and-suppression.md) |
| 06 | `PLAN.md` markdown hyperlink fix | 1 | — | implementor | [tasks/06-plan-md-hyperlink.md](./tasks/06-plan-md-hyperlink.md) |
| 07 | `AppState::pending_runner_actions` visibility hygiene | 1 | — | implementor | [tasks/07-state-visibility.md](./tasks/07-state-visibility.md) |
| 08 | Runner correctness (`try_send` fallback, `NullClipboard` adoption, exhaustive match) | 2 | 01 | implementor | [tasks/08-runner-correctness.md](./tasks/08-runner-correctness.md) |
| 09 | Manual-test matrix execution + parent BUG.md success-criteria check-off | 3 | 01–08 | implementor | [tasks/09-manual-test-matrix.md](./tasks/09-manual-test-matrix.md) |

Waves 1 → 3 must complete in order. Within a wave, all tasks may dispatch in parallel — see overlap matrix below.

---

## File Overlap Analysis

### Files Modified per Task (write set)

| Task | Files Written |
|------|---------------|
| 01 | `crates/fdemon-app/src/services/clipboard.rs`, `crates/fdemon-app/src/services/mod.rs` |
| 02 | `crates/fdemon-tui/src/terminal.rs` |
| 03 | `docs/ARCHITECTURE.md` |
| 04 | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/mouse/mod.rs`, `crates/fdemon-app/src/handler/tests.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs` |
| 05 | `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/tests.rs` |
| 06 | `workflow/plans/features/mouse-support/PLAN.md` |
| 07 | `crates/fdemon-app/src/state.rs` |
| 08 | `crates/fdemon-tui/src/runner.rs` |
| 09 | `workflow/plans/bugs/log-text-selection-broken/BUG.md` (check off success-criteria boxes only) |

### Files Read (read-only deps — no overlap risk)

- Task 08 reads `services/clipboard.rs` (NullClipboard from task 01).
- Tasks 04, 05 read `state.rs`, `message.rs`, `handler/mod.rs` — fine because these are read-only consumers.
- Task 09 reads MOUSE.md and BUG.md but only writes to BUG.md success-criteria checkboxes.

### Overlap Matrix (wave-peers only)

| Pair | Shared write files? | Strategy |
|------|---------------------|----------|
| **Wave 1** | | |
| 01 ↔ 02 | none | Parallel (worktree) |
| 01 ↔ 03 | none | Parallel (worktree) |
| 01 ↔ 04 | none | Parallel (worktree) |
| 01 ↔ 05 | none | Parallel (worktree) |
| 01 ↔ 06 | none | Parallel (worktree) |
| 01 ↔ 07 | none | Parallel (worktree) |
| 02 ↔ 03 | none | Parallel (worktree) |
| 02 ↔ 04 | none | Parallel (worktree) |
| 02 ↔ 05 | none | Parallel (worktree) |
| 02 ↔ 06 | none | Parallel (worktree) |
| 02 ↔ 07 | none | Parallel (worktree) |
| 03 ↔ 04 | none | Parallel (worktree) |
| 03 ↔ 05 | none | Parallel (worktree) |
| 03 ↔ 06 | none | Parallel (worktree) |
| 03 ↔ 07 | none | Parallel (worktree) |
| **04 ↔ 05** | **`crates/fdemon-app/src/handler/tests.rs`** | **Sequential (same branch)** |
| 04 ↔ 06 | none | Parallel (worktree) |
| 04 ↔ 07 | none | Parallel (worktree) |
| 05 ↔ 06 | none | Parallel (worktree) |
| 05 ↔ 07 | none | Parallel (worktree) |
| 06 ↔ 07 | none | Parallel (worktree) |
| **Wave 2** | | |
| (only task 08) | n/a | n/a |
| **Wave 3** | | |
| (only task 09) | n/a | n/a |

**Wave 1 sub-wave structure:** Tasks 01, 02, 03, 06, 07 run as one parallel sub-wave. Task 04 (touches `handler/tests.rs`) and task 05 (also touches `handler/tests.rs`) must run sequentially on the current branch — orchestrator should run task 04 first (it adds new test functions in distinct slots), then task 05.

### Notes on Why Boundaries Land Here

- **Task 04 + Task 05 share `handler/tests.rs`** because both add new test functions to the same `tests` module. They land in different test slots (Task 04 adds focused tests for `resolve_entry_text` near the existing copy-message tests; Task 05 adds suppression tests near the existing Alt+m tests), but the file's append-only nature means a parallel-worktree merge would conflict. Run them sequentially on the current branch.
- **Task 08 depends on Task 01** because the runner fallback paths must reference `NullClipboard` (introduced by Task 01). No other dependencies — Task 08 does not need anything from tasks 02-07.
- **Task 09 depends on all of Tasks 01-08** because manual verification asserts the merged state of all code changes works correctly across terminals.
- **Task 03 is routed to `doc_maintainer`** because `docs/ARCHITECTURE.md` is in the core-docs allow-list managed by `doc_maintainer` only.

---

## Verification (post-merge gate)

After all nine tasks complete, the implementor of Task 09 (or whoever finishes last) runs:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Plus the manual-test matrix per Task 09's instructions.
