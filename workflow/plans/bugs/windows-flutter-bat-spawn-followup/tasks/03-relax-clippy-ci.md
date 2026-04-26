## Task: Relax CI clippy step + scaffold dedicated workspace-clippy-cleanup bug

**Objective**: Unblock the new CI workflow on day one. The Wave-1 fix added `cargo clippy --workspace --all-targets -- -D warnings` to `.github/workflows/ci.yml`, but Rust 1.91's tightened lints surface ~120 pre-existing errors across 41 files in all 5 crates plus integration tests. Verified via diff against base commit `a455e4f` — these errors pre-date the Windows fix.

This task does two things: (1) drop `-D warnings` so CI passes immediately, and (2) scaffold a separate `workflow/plans/bugs/clippy-rust-191-cleanup/` bug to track the cleanup as discrete future work. Restoring `-D warnings` happens once that cleanup ships.

**Depends on**: nothing — Wave A

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `.github/workflows/ci.yml`: change the clippy step to drop `-D warnings`. Leave `--all-targets` for completeness.
- `workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md` (NEW): scaffold a one-document bug-plan for the cleanup. Include a categorized inventory of error types (counts), affected crates, and a recommended approach.

**Files Read (Dependencies):**
- The current `.github/workflows/ci.yml` (Wave-1 output).
- The actual clippy output (run `cargo clippy --workspace --all-targets 2>&1 | grep -E "^error" | sort | uniq -c` to populate the cleanup BUG.md inventory).

### Details

#### `ci.yml` change

Current clippy step (around line 50):

```yaml
- name: Clippy
  run: cargo clippy --workspace --all-targets -- -D warnings
```

Replacement:

```yaml
- name: Clippy
  # NOTE: `-D warnings` is temporarily dropped while pre-existing Rust 1.91 lints
  # are cleaned up workspace-wide. Tracked at:
  # workflow/plans/bugs/clippy-rust-191-cleanup/
  # Restore -D warnings once that cleanup ships.
  run: cargo clippy --workspace --all-targets
```

#### `workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md` scaffold

Create the file with the following content (size and exact wording can be adjusted at write time, but it must include the inventory and rationale so a future implementor can pick it up cold):

```markdown
# Bugfix Plan: Workspace-Wide Rust 1.91 Clippy Cleanup

## TL;DR

Rust 1.91 tightened several clippy lints, surfacing ~120 errors across 41 files
in all 5 crates plus integration tests. These pre-date the Windows
spawn-failure fix and were temporarily allowed by relaxing `-D warnings` to a
warning gate in `.github/workflows/ci.yml`. This bug tracks the cleanup so
`-D warnings` can be restored.

## Inventory

(Populate by running `cargo clippy --workspace --all-targets 2>&1 | grep -E "^error" | sort | uniq -c | sort -rn`. Expect roughly:)

| Lint | Count | Notes |
|------|-------|-------|
| `clippy::field_reassign_with_default` | ~48 | Convert `let mut x = T::default(); x.foo = ...` to struct literals |
| `clippy::bool_assert_comparison` | ~16 | `assert_eq!(x, true)` → `assert!(x)`; `assert_eq!(x, false)` → `assert!(!x)` |
| `unused_variable`, `unused_mut` | ~24 | Remove or prefix with `_` |
| `clippy::type_complexity` | ~7 | Extract `Arc<Mutex<Vec<(...)>>>` into named `type` aliases |
| `clippy::manual_range_contains` | ~5 | `x >= a && x < b` → `(a..b).contains(&x)` |
| `clippy::assertions_on_constants` | ~5 | `assert!(true)` — remove or fix the test logic |
| Others | ~10 | Mixed (`useless_vec`, `while_let`, `clone_on_copy`, etc.) |

## Affected Crates

- `fdemon-core` (small)
- `fdemon-daemon` (~10 files)
- `fdemon-app` (largest — ~20 files)
- `fdemon-tui` (~7 files)
- `fdemon-dap` (~5 files)
- `tests/` integration tests (a few)

## Strategy

The cleanup is mechanical and parallelizable. Suggested approach:

1. Generate the full clippy output as a single fixture file.
2. Split work by crate so multiple implementors can run in parallel worktrees.
3. For each crate: apply automated fixes via `cargo clippy --fix --workspace --all-targets --allow-dirty` first, then hand-fix the remainder.
4. Verify each crate independently with `cargo clippy -p <crate> --all-targets -- -D warnings`.
5. Final step: restore `-D warnings` to `.github/workflows/ci.yml` and confirm CI green.

## Out of Scope

- Test renames, refactors, or behavior changes — this is a lint cleanup only.
- Changes to public APIs (the type-alias extractions can stay private).

## Notes

- This bug was created as part of `workflow/plans/bugs/windows-flutter-bat-spawn-followup/tasks/03-relax-clippy-ci.md`.
- The Windows spawn-failure fix should not be blocked on this cleanup — the lints pre-date that work.
```

### Acceptance Criteria

1. `.github/workflows/ci.yml`'s clippy step no longer contains `-- -D warnings`. The step retains `--workspace --all-targets`.
2. The clippy step has an inline comment pointing to the new cleanup bug-plan.
3. `workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md` exists, follows the standard BUG.md template, and includes the inventory table populated from actual `cargo clippy` output.
4. Running `cargo clippy --workspace --all-targets` locally on the working branch exits 0 (warnings allowed).
5. The new BUG.md is committed alongside the `ci.yml` change.

### Testing

```bash
# Confirm clippy now exits 0 (warnings allowed)
cargo clippy --workspace --all-targets

# Confirm clippy still produces warnings (so the cleanup work is real)
cargo clippy --workspace --all-targets 2>&1 | grep -c "^warning"

# Validate the new BUG.md is well-formed Markdown
ls workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md
```

### Notes

- Do NOT add `--no-deps` to silence dependency warnings — the issue is project code, not dependencies.
- Do NOT fix any of the actual clippy errors in this task. The whole point is to defer them to the dedicated bug-plan. Touching them here defeats the scope split.
- Pinning the `dtolnay/rust-toolchain` action to a specific Rust version (e.g. `@1.77.2`) would make CI deterministic against future toolchain changes, but that's a separate concern (see Task 08 for action-pinning hygiene). For now, leave it at `@stable`.
- The scaffold BUG.md does not need to be exhaustive — a future planner agent will flesh it out into TASKS.md when the cleanup is scheduled.
