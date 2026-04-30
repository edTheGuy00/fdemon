# Task Index — Address review findings from `cache-auto-launch-gate`

Plan: [BUG.md](./BUG.md)

Resolves: 4 critical (C1–C4) + 2 major (M1, M2) findings from [`workflow/reviews/bugs/cache-auto-launch-gate/REVIEW.md`](../../../reviews/bugs/cache-auto-launch-gate/REVIEW.md).

---

## Tasks

| # | Task | File | Agent | Depends on | Resolves |
|---|------|------|-------|------------|----------|
| 01 | Extract migration nudge helper to `fdemon-app::config`; `OnceLock`-gate emission; diverge headless message text; add sibling-bug header comment | [tasks/01-migration-helper-and-headless-text.md](./tasks/01-migration-helper-and-headless-text.md) | implementor | — | C1, C2, sibling-comment-half-of-C4 |
| 02 | Convert `bare_flutter_run` and `find_auto_launch_target` to `Option<AutoLaunchSuccess>`; remove `expect()` panic; update both call sites; add `None`-branch unit test | [tasks/02-eliminate-bare-flutter-run-panic.md](./tasks/02-eliminate-bare-flutter-run-panic.md) | implementor | — | C3 |
| 03 | Settings Panel description hint + sibling-bug `TASKS.md` SUPERSEDED note | [tasks/03-settings-hint-and-sibling-supersede.md](./tasks/03-settings-hint-and-sibling-supersede.md) | implementor | — | M1, sibling-TASKS-half-of-C4 |
| 04 | Promote helper emit `info!` → `warn!`; add TUI banner above New Session dialog gated on helper's `bool` return | [tasks/04-migration-nudge-visibility.md](./tasks/04-migration-nudge-visibility.md) | implementor | 01 | M2 |

---

## Wave Plan

- **Wave 1 (parallel via worktree):** Tasks 01 and 03. Both write disjoint file sets; safe to run concurrently in isolated worktrees.
- **Wave 2 (parallel via worktree):** Tasks 02 and 04. Both write disjoint file sets *from each other* (see overlap matrix below). Task 04 depends on Task 01's helper having merged. Task 02 has no dependency but is grouped here to minimize sequential waves.

> **Note:** Task 02 has no dependency on Task 01, but is placed in Wave 2 to keep Task 01's headless-runner edits isolated (avoids worktree conflict with Task 02's call-site change in the same file).

---

## File Overlap Analysis

### Files Modified (Write)

| Task | Files Modified (Write) | Files Read (dependency) |
|------|------------------------|--------------------------|
| 01 | `crates/fdemon-app/src/config/mod.rs` (new helper `pub fn emit_migration_nudge(mode: NudgeMode) -> bool` + `OnceLock` guard + new `pub enum NudgeMode`) · `crates/fdemon-tui/src/startup.rs` (replace inline `info!` with helper call; capture `bool` result for future banner state) · `src/headless/runner.rs` (replace inline `info!` with helper call AND add header comment near `headless_auto_start` documenting absorbed sibling-bug Task 03 wiring) | `crates/fdemon-app/src/config/settings.rs` (read `OnceLock` pattern from `check_deprecated_auto_start`) |
| 02 | `crates/fdemon-app/src/spawn.rs` (`bare_flutter_run` → `Option<AutoLaunchSuccess>`; `find_auto_launch_target` → `Option<AutoLaunchSuccess>`; update doc comment to remove `# Panics` need; update `spawn_auto_launch` call site at `spawn.rs:~190`; remove the stale `.expect("...line 137")` entirely) · `src/headless/runner.rs` (update `find_auto_launch_target` call site at `runner.rs:305-306` to handle `Option` — `let Some(AutoLaunchSuccess { device, config }) = ... else { /* log + early-return */ }`) | — |
| 03 | `crates/fdemon-app/src/settings_items.rs` (update `behavior.auto_launch` row's `.description(...)` string to include "takes effect on next fdemon launch") · `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` (mark Task 03 row as `SUPERSEDED — wiring absorbed by cache-auto-launch-gate Task 04 on 2026-04-29; close as resolved-by-absorption when reviewed`) | — |
| 04 | `crates/fdemon-app/src/config/mod.rs` (change `tracing::info!` → `tracing::warn!` inside the helper from Task 01) · `crates/fdemon-app/src/state.rs` (add `pub show_migration_banner: bool` field on `AppState`; default `false`) · `crates/fdemon-tui/src/startup.rs` (set `state.show_migration_banner` from helper's `bool` return) · `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` (or wherever the dialog is rendered — render a one-line banner above the dialog when `state.show_migration_banner == true`; clear flag on dialog dismissal) | `crates/fdemon-app/src/config/mod.rs` (helper's signature from Task 01) |

### Overlap Matrix

|        | 01 | 02 | 03 | 04 |
|--------|----|----|----|----|
| **01** | —  | **shared write: `src/headless/runner.rs`** | none | **shared write: `crates/fdemon-app/src/config/mod.rs`, `crates/fdemon-tui/src/startup.rs`** |
| **02** | shared write `headless/runner.rs` | — | none | none |
| **03** | none | none | — | none |
| **04** | shared write `config/mod.rs`, `tui/startup.rs` | none | none | — |

### Strategy Per Pair

- **01 ↔ 02:** Both write `src/headless/runner.rs`. Task 01 modifies the migration log block (lines ~268–281) and adds a header comment. Task 02 modifies the `find_auto_launch_target` call site (lines ~303–306). Different hunks but same file → **sequential (different waves).** Solution: Task 01 in Wave 1, Task 02 in Wave 2 (after Task 01 merges).
- **01 ↔ 03:** Disjoint write sets. **Parallel (worktree).** Both can run in Wave 1.
- **01 ↔ 04:** Both write `crates/fdemon-app/src/config/mod.rs` (Task 01 creates helper, Task 04 changes log level inside the helper) AND `crates/fdemon-tui/src/startup.rs` (Task 01 calls helper, Task 04 captures `bool` for banner). Sequential dependency: Task 04 depends on Task 01 having merged. **Sequential (different waves).** Solution: Task 04 in Wave 2, after Task 01 merges.
- **02 ↔ 03:** Disjoint write sets. **Parallel (worktree).** Could be parallel — but Task 03 runs in Wave 1 with Task 01.
- **02 ↔ 04:** Disjoint write sets (`spawn.rs` + `headless/runner.rs` vs. `config/mod.rs` + `state.rs` + `tui/startup.rs` + `tui/widgets/...`). **Parallel (worktree).** Both can run in Wave 2.
- **03 ↔ 04:** Disjoint write sets. **Parallel (worktree).** Could be Wave 1 or Wave 2; placing 03 in Wave 1 keeps Wave 2 lighter.

### Recommended Merge Order

```
01 ──┐
     ├── 02 ──┐
     │       ├── (release)
     │   04 ─┤
03 ──────────┘
```

**Wave 1 (parallel):** 01, 03
**Wave 2 (parallel, after Task 01 merges):** 02, 04

---

## Documentation Updates

None required:

- No new modules or layer crossings → `docs/ARCHITECTURE.md` unchanged.
- No new build steps or dependencies → `docs/DEVELOPMENT.md` unchanged.
- No new patterns established (the helper extraction is a local refactor; the `OnceLock` pattern is already documented by precedent) → `docs/CODE_STANDARDS.md` unchanged.
- The TUI banner is a small UX addition, not a new widget pattern.
- `docs/CONFIGURATION.md` was rewritten by the parent plan (Task 05) and remains accurate; the helper text changes do not invalidate it.

---

## Verification (run once after all four tasks merge)

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Manual Smoke Tests

See [BUG.md §Verification](./BUG.md#verification) for the full manual smoke matrix. Summary:

1. **C1** — Migration `warn!` line appears once per process invocation, not multiple times within a single process.
2. **C2** — Headless log line uses the headless-specific text (no `[behavior] auto_launch` remediation reference).
3. **C3** — `cargo check` confirms both call sites of `find_auto_launch_target` handle the `Option` return type. No `expect()` reachable from `pub` API.
4. **C4** — Header comment present in `headless/runner.rs`; sibling `TASKS.md` shows SUPERSEDED on Task 03.
5. **M1** — Settings Panel description includes "takes effect on next fdemon launch".
6. **M2** — Migration log emitted at WARN level. New Session dialog shows the migration banner on first appearance after upgrade.

---

## Risks & Mitigations

- **R1 — Banner clear-trigger ambiguity:** Banner state needs to clear at some point so it doesn't persist forever after the user dismisses the dialog. *Mitigation:* Task 04 acceptance criteria require *some* clear path; implementor's choice of trigger (dialog close, ui_mode change away from Startup, or explicit user action). Document the choice in the task's Completion Summary.
- **R2 — `Option<AutoLaunchSuccess>` None-branch behavior:** Both callers must produce a sensible early-return when `None` is returned. *Mitigation:* Task 02 specifies the behavior: log error + emit headless event (headless) / log error + abort spawn (TUI). Both branches already exist for the empty-devices case; reuse them.
- **R3 — `OnceLock` test isolation:** Process-level `OnceLock` survives across tests in the same binary. *Mitigation:* Task 01 places the `OnceLock` inside the helper function (function-static). Tests can verify the helper's `bool` return without needing to reset the lock. If a test specifically wants to assert log emission count, it uses a different test harness or asserts at the `bool`-return level.
- **R4 — Cross-task helper signature drift:** Task 04 needs Task 01's helper to return `bool`. *Mitigation:* Task 01's acceptance criteria explicitly require the `bool` return; Task 04 references it. If the helper signature changes during Task 01 implementation, Task 04 will need a small update.
