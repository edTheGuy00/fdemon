# TASKS — Version-Check Banner Copilot Review Follow-ups

**Plan**: [PLAN.md](PLAN.md)
**Branch**: `feat/version-check-banner` (these tasks land directly on the
PR-49 branch).

## Wave 1 — Parallel

Both tasks touch disjoint files (`crates/fdemon-app/src/version_check.rs`
vs `crates/fdemon-tui/src/runner.rs`) and can run in parallel worktrees.

| # | Task | Agent | Status | Files modified |
|---|------|-------|--------|----------------|
| [01](tasks/01-normalize-tag-string.md) | Normalize the public tag string in `check_for_newer_release` and align doc comments on `fetch_latest_tag` / `check_for_newer_release`. | `implementor` | ✅ Done | `crates/fdemon-app/src/version_check.rs` |
| [02](tasks/02-runner-gate-timeout-zero.md) | Treat `version_check_timeout_secs = 0` as fully disabled at both TUI spawn sites. | `implementor` | ✅ Done | `crates/fdemon-tui/src/runner.rs`, `crates/fdemon-app/src/config/types.rs` |

---

## File Overlap Analysis

### Files Modified (Write) — per task

| Task | Files (Write) | Files (Read-only) |
|------|---------------|-------------------|
| 01 | `crates/fdemon-app/src/version_check.rs` | `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` (render-site context) |
| 02 | `crates/fdemon-tui/src/runner.rs`, *optionally* `crates/fdemon-app/src/config/types.rs` | `crates/fdemon-app/src/spawn.rs` (spawn signature context) |

### Overlap matrix

| | Task 01 | Task 02 |
|---|---|---|
| **Task 01** | — | No shared write files → **Parallel (worktree)** |
| **Task 02** | No shared write files → **Parallel (worktree)** | — |

Result: both tasks are safe to dispatch in isolated worktrees.

---

## Acceptance (overall)

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace`
- [x] PR #49 review resolved on all four Copilot comments (1, 2, 3, 4).
- [x] The "digit-and-dot only" contract on `check_for_newer_release` is now actually true.
- [x] Setting `version_check_timeout_secs = 0` in `.fdemon/config.toml` results in `spawn_version_check` not being called (verifiable by a trace-log or by running with `lsof -i` and confirming no `api.github.com` lookup).
