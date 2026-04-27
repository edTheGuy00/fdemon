## Task: Restore `-D warnings` on the CI clippy step

**Objective**: Re-enable `-D warnings` on the workspace clippy invocation in `.github/workflows/ci.yml` and confirm the workspace-wide lint is green on all three OS runners.

**Depends on**: 01-fix-fdemon-core, 02-fix-fdemon-daemon, 03-fix-fdemon-dap, 04-fix-fdemon-tui, 05-fix-fdemon-app, 06-fix-integration-tests

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `.github/workflows/ci.yml` — change the clippy step's command back to `cargo clippy --workspace --all-targets -- -D warnings`, and remove the temporary `# NOTE: -D warnings is temporarily dropped …` comment block (currently lines 53–56 of the workflow file).

**Files Read (Dependencies):**
- `workflow/plans/bugs/clippy-rust-191-cleanup/tasks/01-fix-fdemon-core.md` … `06-fix-integration-tests.md` — to confirm all upstream tasks have shipped.

### Procedure

1. Confirm prerequisites locally before editing CI:
   ```bash
   cargo clippy --workspace --all-targets -- -D warnings
   ```
   This must exit 0 on the current branch. If it does not, the upstream task fixes are incomplete — do **not** flip the CI flag yet; instead, file the residual warnings against the relevant per-crate task.
2. Edit `.github/workflows/ci.yml`. The current state (post-Windows-spawn-fix) is:
   ```yaml
   - name: cargo clippy
     # NOTE: `-D warnings` is temporarily dropped while pre-existing Rust 1.91 lints
     # are cleaned up workspace-wide. Tracked at:
     # workflow/plans/bugs/clippy-rust-191-cleanup/
     # Restore -D warnings once that cleanup ships.
     run: cargo clippy --workspace --all-targets
   ```
   Replace it with:
   ```yaml
   - name: cargo clippy
     run: cargo clippy --workspace --all-targets -- -D warnings
   ```
3. Re-run the full local quality gate from `docs/DEVELOPMENT.md`:
   ```bash
   cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
   ```
   All four steps must pass.
4. Open a PR (or merge into the existing branch) and confirm CI green on all three OS runners (`ubuntu-latest`, `macos-latest`, `windows-latest`).

### Acceptance Criteria

1. `.github/workflows/ci.yml` clippy step runs `cargo clippy --workspace --all-targets -- -D warnings`.
2. The temporary explanatory comment block on the clippy step is removed.
3. Local quality-gate command (above) exits 0 end-to-end.
4. CI is green on all three runners.
5. No source-code changes are made by this task — only the workflow file.

### Notes

- If CI fails after the flag flip, the failure mode is almost certainly platform-specific lints (e.g., `cfg(target_os = "windows")` paths that local `cargo clippy` skipped). In that case, leave the CI flag flipped, file a follow-up task for the platform-specific warnings, and triage rather than reverting — reverting just delays the fix.
- After the bug is fully resolved, consider archiving this plan to `workflow/reviews/bugs/clippy-rust-191-cleanup/` per repo convention (this is a manual step outside this task's scope).

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy --workspace --all-targets -- -D warnings` (local) — _tbd_
- CI green on `ubuntu-latest` / `macos-latest` / `windows-latest` — _tbd_

### Risks/Limitations

_tbd_
