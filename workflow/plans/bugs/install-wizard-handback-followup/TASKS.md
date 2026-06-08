# Install Wizard Handback Follow-ups — Task Index

## Overview

Three review follow-ups from
[install-wizard-informational-reopen/REVIEW.md](../install-wizard-informational-reopen/REVIEW.md),
plus a doc update. See [BUG.md](BUG.md) for root-cause analysis and the chosen approaches.

- **Finding 1 (MAJOR):** reuse the SDK `run_preflight` already resolves so a Bootstrap install can
  never silently close to Normal without device discovery.
- **Finding 2 (MEDIUM):** tailor the post-install header hint for `UserInvoked` opens (strict
  Option-1 kept).
- **Finding 3 (MINOR):** direct unit tests for `all_components_ok()` / `is_bootstrap()`.

**Total Tasks:** 4
**Estimated Hours:** 6–9.5 hours

## Task Dependency Graph

```
┌────────────────────────────────────┐   ┌──────────────────────────────┐
│ 01-harden-handback-sdk-resolution  │   │ 02-tailor-installed-hint     │
│ (daemon + app executor + binary)   │   │ (app state + tui header)     │
└─────────────────┬──────────────────┘   └───────────────┬──────────────┘
                  │                                       │ (shares state.rs)
                  ▼                                       ▼
┌────────────────────────────────────┐   ┌──────────────────────────────┐
│ 04-doc-architecture (doc_maintainer)│   │ 03-state-predicate-unit-tests│
└────────────────────────────────────┘   └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-harden-handback-sdk-resolution](tasks/01-harden-handback-sdk-resolution.md) | Not Started | - | 3-5h | `fdemon-daemon/toolchain`, `fdemon-app/actions`, `src/doctor.rs`, `fdemon-app/handler/install_wizard` |
| 2 | [02-tailor-installed-hint](tasks/02-tailor-installed-hint.md) | Not Started | - | 2-3h | `fdemon-app/install_wizard/state.rs`, `fdemon-tui/widgets/install_wizard` |
| 3 | [03-state-predicate-unit-tests](tasks/03-state-predicate-unit-tests.md) | Not Started | 2 (file overlap) | 0.5-1h | `fdemon-app/install_wizard/state.rs` |
| 4 | [04-doc-architecture](tasks/04-doc-architecture.md) | Not Started | 1 | 0.5h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-harden-handback-sdk-resolution | `crates/fdemon-daemon/src/toolchain/mod.rs`, `crates/fdemon-daemon/src/lib.rs`, `crates/fdemon-app/src/actions/mod.rs`, `src/doctor.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs` | `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/state.rs` |
| 02-tailor-installed-hint | `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`, `docs/KEYBINDINGS.md` | `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-daemon/src/toolchain/types.rs` |
| 03-state-predicate-unit-tests | `crates/fdemon-app/src/install_wizard/state.rs` | `crates/fdemon-daemon/src/toolchain/types.rs` |
| 04-doc-architecture | `docs/ARCHITECTURE.md` | task 01 files |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 02 + 03 | `crates/fdemon-app/src/install_wizard/state.rs` | **Sequential (same branch)** — 03 depends on 02 |
| 01 + 04 | None | Sequential by dependency (04 → 01) |

> **Note:** Task 01 is a single atomic compile unit — the `run_preflight` return-type change
> ripples to `actions/mod.rs`, `src/doctor.rs`, and daemon tests simultaneously; do not split it.
> Task 02 is likewise atomic across `install_wizard/state.rs` (new `observed_unhealthy` field) and
> the TUI header that reads it.

## Suggested Execution Plan

- **Wave 1:** Task 01 (worktree) ‖ Task 02 (worktree) — no shared write files.
- **Wave 2:** Task 03 (same branch, after 02 merges — shares `state.rs`) ‖ Task 04
  (doc_maintainer, after 01 merges).

## Success Criteria

- [ ] `run_preflight` returns the resolved SDK; executor no longer double-resolves; Bootstrap
      handback fires whenever the post-install report shows `FlutterSdk: Ok`.
- [ ] A `UserInvoked` wizard that installed Flutter shows the "Flutter installed — press <key> to
      start a session" hint; a healthy-throughout wizard still shows "All set — press Esc to return".
- [ ] Direct unit tests cover `all_components_ok()` and `is_bootstrap()`.
- [ ] `cargo test --workspace` green; `cargo clippy --workspace --all-targets -- -D warnings` and
      `cargo fmt --all -- --check` clean.
- [ ] `docs/ARCHITECTURE.md` reflects the `run_preflight` return-type change; `docs/KEYBINDINGS.md`
      reflects the installed hint if touched.

## Notes

- Strict Option-1 (a `UserInvoked` wizard never auto-hands-back) is **not** revisited — Finding 2
  only changes hint text.
- Decisions captured 2026-06-08 via review follow-up: Finding 1 = reuse preflight SDK result;
  Finding 2 = tailor hint text.
