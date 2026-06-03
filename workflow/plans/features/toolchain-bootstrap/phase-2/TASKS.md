# Phase 2 — Managed Flutter SDK Install + PATH Configuration — Task Index

## Overview

Phase 2 turns the read-only Install Wizard (Phase 1) into an actor: it installs a
managed Flutter SDK (git clone by default, archive download fallback), writes the
SDK onto the user's `PATH` via shell-aware rc files, persists `[flutter] sdk_path`
to `.fdemon/config.toml` so fdemon resolves the new SDK without a restart, and
re-runs preflight so the wizard reflects the now-working toolchain.

Only the **Flutter SDK** and **PATH Configuration** wizard steps become
executable in this phase. Android tools (Phase 3) and OS prerequisites (Phase 4)
remain read-only.

**Total Tasks:** 12
**Estimated Hours:** 34–46 hours

## Architecture Recap (what already exists from Phase 1)

- `fdemon-daemon::toolchain` — `run_preflight()`, `types.rs` (`HostShell`,
  `HostPlatform`, `ComponentCheck`, …), `checks.rs`, `doctor.rs`. Read-only.
- `fdemon-app::install_wizard` — `InstallWizardState`, `WizardStep`,
  `WizardStepKind` (`Prerequisites`/`AndroidTools`/`PathConfig`/`FlutterSdk`/`Doctor`),
  `StepStatus`, `WizardPane`, `build_steps()`. Re-exports daemon display types.
- `fdemon-app::handler::install_wizard` — `navigation.rs` (open/close/nav),
  `actions.rs` (`handle_preflight_completed`, `handle_rerun_preflight`).
- `UpdateAction::RunToolchainPreflight` + `Message::ToolchainPreflightCompleted`
  already wired through `actions/mod.rs::handle_action` and the runner.
- Startup hook in `runner.rs` already opens the wizard when
  `flutter_executable().is_none()`.
- Async-install template to copy: `handler/flutter_version/actions.rs` +
  `actions/mod.rs` arms (`SwitchFlutterVersion`, `ScanInstalledSdks`,
  `ProbeFlutterVersion`), `PersistSettings` action, `save_settings()`,
  `resolve_fvm_cache_path()`.

## Task Dependency Graph

```
Wave 1 (parallel, no deps)
┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐
│ 01 daemon deps     │  │ 05 wizard step     │  │ 06 [toolchain]     │  │ 07 wizard exec     │
│    + install types │  │    protocol        │  │    config settings │  │    state           │
└─────────┬──────────┘  └─────────┬──────────┘  └─────────┬──────────┘  └─────────┬──────────┘
          │                       │                        │                       │
          ▼                       │                        │                       ▼
┌────────────────────┐            │                        │             ┌────────────────────┐
│ 02 download +      │            │                        │             │ 10 TUI progress +  │
│    extract +       │            │                        │             │    step detail     │
│    process stream  │            │                        │             └────────────────────┘
└─────────┬──────────┘            │                        │
          ▼                       │                        │
┌────────────────────┐            └──────────┬─────────────┘
│ 03 flutter_install │                       ▼
└─────────┬──────────┘            ┌────────────────────┐   ┌────────────────────┐
          ▼                       │ 09 wizard handlers │   │ 12 CONFIG +        │
┌────────────────────┐           │    + completion    │   │    KEYBINDINGS doc │
│ 04 path_config     │           │    wiring          │   └────────────────────┘
└─────────┬──────────┘           └────────────────────┘
          │
          └────────────┬───────────(05)────────────┐
                       ▼                            │
            ┌────────────────────┐                  │
            │ 08 RunWizardStep   │◄─────────────────┘
            │    executor        │
            └─────────┬──────────┘
                      ▼
            ┌────────────────────┐
            │ 11 ARCHITECTURE.md │ (doc_maintainer; after 02,03,04,08,09,10)
            └────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-daemon-install-deps-and-types](tasks/01-daemon-install-deps-and-types.md) | Not Started | - | 2-3h | root `Cargo.toml`, `fdemon-daemon/Cargo.toml`, `toolchain/types.rs` |
| 02 | [02-download-extract-process-stream](tasks/02-download-extract-process-stream.md) | Not Started | 01 | 5-6h | `toolchain/download.rs`, `toolchain/process_stream.rs`, `toolchain/mod.rs` |
| 03 | [03-managed-flutter-install](tasks/03-managed-flutter-install.md) | Not Started | 02 | 5-7h | `toolchain/flutter_install.rs`, `toolchain/mod.rs` |
| 04 | [04-path-config-writer](tasks/04-path-config-writer.md) | Not Started | 03 | 4-5h | `toolchain/path_config.rs`, `toolchain/mod.rs` |
| 05 | [05-wizard-step-protocol](tasks/05-wizard-step-protocol.md) | Not Started | - | 2-3h | `message.rs`, `handler/mod.rs`, `handler/keys.rs` |
| 06 | [06-toolchain-config-settings](tasks/06-toolchain-config-settings.md) | Not Started | - | 2-3h | `config/types.rs` |
| 07 | [07-wizard-exec-state](tasks/07-wizard-exec-state.md) | Not Started | - | 3-4h | `install_wizard/state.rs`, `install_wizard/types.rs`, `install_wizard/mod.rs` |
| 08 | [08-run-wizard-step-executor](tasks/08-run-wizard-step-executor.md) | Not Started | 03, 04, 05 | 4-5h | `actions/mod.rs` |
| 09 | [09-wizard-handlers-and-completion](tasks/09-wizard-handlers-and-completion.md) | Not Started | 05, 06, 07 | 4-5h | `handler/install_wizard/actions.rs`, `handler/update.rs` |
| 10 | [10-tui-progress-and-detail](tasks/10-tui-progress-and-detail.md) | Not Started | 07 | 4-5h | `widgets/install_wizard/progress.rs`, `step_detail.rs`, `mod.rs` |
| 11 | [11-update-architecture-doc](tasks/11-update-architecture-doc.md) | Not Started | 02,03,04,08,09,10 | 1-2h | `docs/ARCHITECTURE.md` |
| 12 | [12-update-config-keybindings-docs](tasks/12-update-config-keybindings-docs.md) | Not Started | 05, 06 | 1h | `docs/CONFIGURATION.md`, `docs/KEYBINDINGS.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01 | root `Cargo.toml`, `crates/fdemon-daemon/Cargo.toml`, `crates/fdemon-daemon/src/toolchain/types.rs` | `crates/fdemon-app/src/version_check.rs` (reqwest usage pattern) |
| 02 | `crates/fdemon-daemon/src/toolchain/download.rs`, `crates/fdemon-daemon/src/toolchain/process_stream.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs` | `toolchain/types.rs` |
| 03 | `crates/fdemon-daemon/src/toolchain/flutter_install.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs` | `toolchain/download.rs`, `toolchain/process_stream.rs`, `toolchain/types.rs`, `flutter_sdk/cache_scanner.rs` |
| 04 | `crates/fdemon-daemon/src/toolchain/path_config.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs` | `toolchain/types.rs` (`HostShell`) |
| 05 | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/handler/keys.rs` | `install_wizard/types.rs` (`WizardStepKind`) |
| 06 | `crates/fdemon-app/src/config/types.rs` | `crates/fdemon-app/src/config/settings.rs` (serde pattern) |
| 07 | `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/install_wizard/types.rs`, `crates/fdemon-app/src/install_wizard/mod.rs` | - |
| 08 | `crates/fdemon-app/src/actions/mod.rs` | `toolchain/flutter_install.rs`, `toolchain/path_config.rs`, `message.rs`, `handler/mod.rs` |
| 09 | `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/handler/update.rs` | `message.rs`, `install_wizard/state.rs`, `config/settings.rs` |
| 10 | `crates/fdemon-tui/src/widgets/install_wizard/progress.rs`, `step_detail.rs`, `mod.rs` | `fdemon-app::install_wizard` (exec state re-exports) |
| 11 | `docs/ARCHITECTURE.md` | task files 02,03,04,08,09,10 |
| 12 | `docs/CONFIGURATION.md`, `docs/KEYBINDINGS.md` | task files 05,06 |

### Overlap Matrix

Wave-peer comparisons (tasks with no dependency edge between them that may run concurrently):

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 05 + 06 + 07 | None | Parallel (worktree) |
| 02 + 03 + 04 | `toolchain/mod.rs` | **Sequential (same branch)** — enforced by the 01→02→03→04 dependency chain |
| 05 + 06 + 07 + 02 | None | Parallel (worktree) |
| 08 + 09 + 10 | None (distinct files) | Parallel (worktree) |
| 09 + 12 | None | Parallel (worktree) |
| 08 + 11 | `actions/mod.rs` vs `docs/ARCHITECTURE.md` — none | Parallel; 11 depends on 08 anyway |

**Key isolation note:** `crates/fdemon-daemon/src/toolchain/mod.rs` is written by
tasks 02, 03, and 04 (each adds a `mod x;` + `pub use`). These are already a
linear dependency chain (02→03→04), so they run sequentially on the same branch —
no merge conflict on `mod.rs`. No other write-file overlaps exist between
concurrent tasks.

## Suggested Wave Schedule

- **Wave 1 (parallel):** 01, 05, 06, 07
- **Wave 2 (parallel, as deps clear):** 02 (after 01), 09 (after 05/06/07), 10 (after 07), 12 (after 05/06)
- **Wave 3:** 03 (after 02)
- **Wave 4:** 04 (after 03)
- **Wave 5:** 08 (after 03/04/05)
- **Wave 6:** 11 (after 02/03/04/08/09/10)

## Success Criteria

Phase 2 is complete when:

- [ ] On a machine with no Flutter, pressing `Enter` on the **Flutter SDK** step
      downloads/clones a managed SDK into the configured install dir with a live
      progress bar and streamed log tail.
- [ ] git clone is the default; archive download + SHA-256 verify + extract is used
      automatically when `git` is absent (or when `flutter_install_method = "archive"`).
- [ ] `flutter precache` runs after install and its output streams into the wizard.
- [ ] The **PATH Configuration** step writes an idempotent, marker-fenced PATH
      export for `<flutter>/bin` to the correct shell rc file and shows a
      "restart your terminal" hint.
- [ ] After the Flutter step, `[flutter] sdk_path` is written to
      `.fdemon/config.toml` and a re-run of preflight shows Flutter SDK = Ok
      without restarting fdemon.
- [ ] The freshly installed version appears in the Flutter Version panel's
      FVM-cache list (reuses `ScanInstalledSdks`).
- [ ] Step failures surface a clear error and the step can be retried with `Enter`.
- [ ] All new code has unit tests (manifest parse, host-arch resolution,
      SHA-256 verify, zip + tar.xz extraction, idempotent rc-file writes
      golden-file, shell rc-path selection). Existing tests pass; no regressions.
- [ ] `cargo fmt`, `cargo check --workspace --all-targets`, `cargo test --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Keyboard Shortcuts (Phase 2 additions)

| Key | Mode | Action |
|-----|------|--------|
| `Enter` | InstallWizard | Run / retry the selected step (Flutter SDK or PATH Config only this phase) |

## Notes

- **Scope discipline:** Only `WizardStepKind::FlutterSdk` and
  `WizardStepKind::PathConfig` are executable. Pressing `Enter` on
  `Prerequisites`, `AndroidTools`, or `Doctor` is a no-op (or shows
  "available in a later phase").
- **No `sudo`/GUI automation** in Phase 2 — that is Phases 3/4. PATH writes are
  user-file writes only.
- **Pure-Rust xz:** prefer `lzma-rs` for `.tar.xz` to avoid a liblzma C dependency
  (per PLAN.md decision). Gate `xz2` behind a feature only if perf demands it.
- **Atomic installs:** download/clone into a temp dir under the install root, then
  rename into `<install_dir>/<version>` on success; clean up temp on failure.
- **PATH-write confirmation:** pressing `Enter` on the PATH step *is* the
  confirmation. The write is idempotent and marker-fenced, so re-running is safe.
- **TEA purity:** all I/O happens in `actions/mod.rs` spawned tasks; handlers stay
  pure and communicate via `Message` variants (mirrors `flutter_version`).
</content>
</invoke>
