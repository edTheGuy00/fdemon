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

**Status:** Not Started
**Branch:** <fill in>

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed
