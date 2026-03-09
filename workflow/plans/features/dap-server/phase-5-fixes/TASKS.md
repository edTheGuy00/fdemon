# Phase 5 Fixes: IDE DAP Auto-Configuration Review Fixes — Task Index

## Overview

Addresses 1 critical, 4 major, and 5 minor issues found during the Phase 5 code review. The critical fix wires the `--dap-config` CLI override through combined mode. Major fixes eliminate code duplication, remove dead code, fix the Emacs merge path regression, and add content comparison before file writes.

**Total Tasks:** 10
**Waves:** 2 (dependency-ordered)

## Task Dependency Graph

```
Wave 1 — Critical + Major (parallel, except 04→05 sequential)
├── 01-thread-cli-override         (Critical — independent)
├── 02-deduplicate-jsonc           (Major — independent)
├── 03-remove-indoc-noop           (Major — independent)
├── 04-fix-emacs-merge-path        (Major — independent)
└── 05-content-comparison          (Major — depends on 04, both touch run_generator)

Wave 2 — Minor cleanups (all parallel)
├── 06-helix-unreachable-to-error
├── 07-idiomatic-or-else
├── 08-restrict-merge-visibility
├── 09-zed-delve-comment
└── 10-serial-env-var-tests
```

## Tasks

| # | Task | Status | Depends On | Wave | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-thread-cli-override](tasks/01-thread-cli-override.md) | Not Started | - | 1 | `src/main.rs`, `src/tui/runner.rs`, `src/headless/runner.rs`, `fdemon-app/engine.rs`, `fdemon-app/state.rs`, `fdemon-app/handler/dap.rs` |
| 2 | [02-deduplicate-jsonc](tasks/02-deduplicate-jsonc.md) | Not Started | - | 1 | `fdemon-app/config/vscode.rs`, `fdemon-app/ide_config/merge.rs` |
| 3 | [03-remove-indoc-noop](tasks/03-remove-indoc-noop.md) | Not Started | - | 1 | `fdemon-app/ide_config/helix.rs` |
| 4 | [04-fix-emacs-merge-path](tasks/04-fix-emacs-merge-path.md) | Not Started | - | 1 | `fdemon-app/ide_config/mod.rs`, `fdemon-app/ide_config/emacs.rs` |
| 5 | [05-content-comparison](tasks/05-content-comparison.md) | Not Started | 4 | 1 | `fdemon-app/ide_config/mod.rs` |
| 6 | [06-helix-unreachable-to-error](tasks/06-helix-unreachable-to-error.md) | Not Started | - | 2 | `fdemon-app/ide_config/helix.rs` |
| 7 | [07-idiomatic-or-else](tasks/07-idiomatic-or-else.md) | Not Started | - | 2 | `fdemon-app/actions/mod.rs` |
| 8 | [08-restrict-merge-visibility](tasks/08-restrict-merge-visibility.md) | Not Started | - | 2 | `fdemon-app/ide_config/mod.rs`, `fdemon-app/ide_config/merge.rs` |
| 9 | [09-zed-delve-comment](tasks/09-zed-delve-comment.md) | Not Started | - | 2 | `fdemon-app/ide_config/zed.rs` |
| 10 | [10-serial-env-var-tests](tasks/10-serial-env-var-tests.md) | Not Started | - | 2 | `fdemon-app/Cargo.toml`, `fdemon-app/config/settings.rs` |

## Success Criteria

Phase 5 fixes are complete when:

- [ ] `fdemon --dap-config neovim` (no `--dap-port`) generates Neovim config when DAP starts
- [ ] `clean_jsonc` exists in one location only
- [ ] `ConfigAction::Skipped` is no longer dead code — produced when content is unchanged
- [ ] Emacs merge path produces absolute paths, not relative placeholders
- [ ] `indoc()` no-op removed from helix.rs
- [ ] No `unreachable!()` in library code
- [ ] Internal merge utilities are `pub(crate)`, not `pub`
- [ ] `cargo fmt --all` — Pass
- [ ] `cargo check --workspace` — Pass
- [ ] `cargo test --workspace` — Pass
- [ ] `cargo clippy --workspace -- -D warnings` — Pass

## Notes

- Tasks 4 and 5 both modify `run_generator()` in `ide_config/mod.rs` — execute sequentially to avoid merge conflicts.
- Task 1 is the only critical fix. All other tasks are quality improvements that could ship independently.
- Task 10 adds a new dev-dependency (`serial_test`) to `fdemon-app/Cargo.toml`.
