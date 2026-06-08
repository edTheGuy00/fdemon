## Task: Direct unit tests for `all_components_ok()` and `is_bootstrap()` (Finding 3)

**Objective**: Add co-located, state-level unit tests for the two `pub` predicates introduced by
the original `WizardOrigin` fix, satisfying the `docs/CODE_STANDARDS.md` rule that all new public
functions have direct tests. They are currently only covered indirectly via render and handler
tests.

**Depends on**: None functionally — but shares `install_wizard/state.rs` with task 02, so it runs
**sequentially on the same branch** as task 02 (see TASKS.md File Overlap Analysis), not in a
parallel worktree.

**Agent:** implementor

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**

- `crates/fdemon-app/src/install_wizard/state.rs` — add tests to the existing `#[cfg(test)] mod
  tests` block. Reuse the module's existing report/check helpers (search for how other tests build
  a `ToolchainReport` / `ComponentCheck`; mirror that style rather than introducing new helpers).

**Files Read (Dependencies):**

- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentStatus` variants (`Ok`, `Missing`,
  `Unknown`, …) and `ComponentKind`.

### Details

Add tests covering every branch of each predicate:

`all_components_ok()`:
- no report applied → `false`
- report with empty `components` → `false`
- report where every component is `Ok` → `true`
- report with any `Missing` (or other non-Ok) component → `false`
- report with any `Unknown` component → `false` (documents the intentional stricter-than-`rollup_status` behaviour)

`is_bootstrap()`:
- `opening(WizardOrigin::Bootstrap)` → `true`
- `opening(WizardOrigin::UserInvoked)` → `false`

### Acceptance Criteria

1. Direct tests exist for both predicates, covering the branches above, with descriptive names
   (`all_components_ok_returns_false_when_any_component_is_unknown`, etc.).
2. Tests reuse existing in-module report-building helpers where available.
3. `cargo test -p fdemon-app` passes; `cargo clippy --workspace` and `cargo fmt --all` clean.

### Notes

- Test-only change; no production code is modified.
- If task 02 already added some of these (it adds `observed_unhealthy` tests in the same block),
  avoid duplication — this task owns the `all_components_ok()` / `is_bootstrap()` coverage
  specifically.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Added 7 unit tests in existing `#[cfg(test)] mod tests` block covering all branches of `all_components_ok()` and `is_bootstrap()` |

### Notable Decisions/Tradeoffs

1. **Test placement**: Tests appended after the existing `observed_unhealthy` section (task 02 coverage) near end of the test block, under clear section comments. No duplication with task 02 tests.
2. **Helper reuse**: Used existing `make_report()` and `make_check()` helpers throughout — no new helpers introduced.
3. **Unknown status note**: The `all_components_ok_returns_false_when_any_component_is_unknown` test includes a doc comment explaining the intentional stricter-than-`rollup_status` behaviour, per task spec.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (2929 unit tests in fdemon-app)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
