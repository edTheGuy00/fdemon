# Phase 5 — Polish, Parity & Integration — Task Index

## Overview

Phase 5 closes out the toolchain-bootstrap feature. A pre-breakdown code audit
(7-agent research workflow, 2026-06-05) found that **most of the PLAN's Phase 5
bullets were already delivered** by the Phase 2-hardening / Phase 3-followup /
Phase 4-followup waves:

- **Re-check loop & per-step retry** — already shipped: `r` re-runs preflight
  (idempotent-guarded, `actions.rs:51`); a failed step retries on `Enter`
  (`handle_step_failed` keeps `log_tail`; `handle_run_selected_step` re-dispatches
  `RunWizardStep`). Only **display polish** remains.
- **Cross-platform PATH parity** — already shipped: fish writes `fish_add_path`
  into `config.fish`; **Windows already uses `[Environment]::SetEnvironmentVariable`
  (registry), deliberately bypassing `setx` and its 1024-char limit**
  (`path_config.rs:577-778`). 45+ tests. Only the `WM_SETTINGCHANGE` broadcast is
  missing.
- **Tests** (manifest / doctor-text / cmdline-tools relocation / rc-writes /
  per-OS commands / step ordering) — **5 of 6 fully covered**; only narrow
  sub-cases remain, folded into the owning tasks below.

What **genuinely remains** is therefore: (1) download safety (disk + network
preflight, abortable downloads), (2) the **launch-dialog handback** (the largest
functional gap — the wizard never re-triggers device discovery after a successful
install), (3) the optional `fdemon doctor` CLI, and (4) small UX/test polish.

**Scope decisions (resolved with requester, 2026-06-05):**

- **Resumable downloads (HTTP Range): DEFERRED** — kept out of scope (was already
  out-of-scope in Phase 2). Abort + disk/network preflight make large downloads
  robust enough; resume is a best-effort nicety. Listed as a Future Enhancement.
- **CLI surface: `fdemon doctor` only** — `fdemon setup` (a headless, TUI-decoupled
  install runner) is deferred to a future phase.
- **Handback trigger: as soon as `flutter_executable()` resolves** (Flutter
  installed + on PATH), even if Android tools remain missing — matches the PLAN
  goal "fdemon can launch sessions." Not gated on all-5-steps-Ok.
- **Fish PATH writer: keep `config.fish`** — the working, tested writer stays;
  `conf.d` noted only as a future improvement (avoids a migration of existing
  fenced blocks).

**Total Tasks:** 8
**Estimated Hours:** 22–32 hours

**Platform scope:** detection + install everywhere; download-safety and PATH
changes exercised on Linux/macOS/Windows per the existing per-OS code paths.

## Architecture Recap (what already exists from Phases 1–4)

- **Download pipeline** (`toolchain/download.rs`): `download_to_file(url, dest,
  on_progress)` — streaming GET, connect/idle timeouts, 3-attempt retry,
  `.part`-file staging, SHA-256 verify; `extract_zip` / `extract_tar_xz`
  (traversal-safe, bounded-RAM XZ via mpsc). **No** disk/network preflight, **no**
  cancellation parameter.
- **Install orchestrators** (`flutter_install.rs`, `android_install.rs`): RAII
  lockfile, channel validation, manifest resolution — spawned fire-and-forget from
  `actions/mod.rs:~838` with the `JoinHandle` **dropped** (no abort path).
- **Wizard re-check/retry** (`handler/install_wizard/actions.rs`): `r` →
  `RunToolchainPreflight`; `handle_step_failed` sets `StepExecStatus::Failed`
  **without** clearing `log_tail`; `Enter` re-runs via `begin_step` + `RunWizardStep`.
- **Wizard open/close** (`navigation.rs`, `runner.rs:296-298`): opened when
  `flutter_executable().is_none()` at startup or via `Shift+I`; closed only by
  `Esc`/`HideInstallWizard` → `UiMode::Normal`, returning `UpdateResult::none()`
  (no device-discovery follow-up).
- **PATH writers** (`path_config.rs`): bash/zsh/fish + Windows-registry, all
  marker-fenced/idempotent/atomic; `PowerShell`/`Cmd`/`Unknown` → config error.
- **Preflight CLI primitive** (`toolchain/mod.rs:78`): `run_preflight(project_path,
  explicit_sdk_path) -> ToolchainReport` (never fails); `ToolchainReport` fields are
  all `pub`; `ComponentStatus` has `Ok/Partial/Missing/Error/Unknown` (no `Display`).

## Task Dependency Graph

```
Wave 1 (parallel)         Wave 2      Wave 3        Wave 4 (parallel)      Wave 5
┌──────────────────┐      ┌───────┐   ┌─────────┐   ┌──────────────────┐   ┌────────┐
│ 01 disk+net      │─────▶│ 02    │──▶│ 03      │─┬▶│ 04 launch        │   │ 08     │
│    preflight     │      │ abort │   │ abort + │ │ │    handback      │──▶│ docs   │
│  (download path) │      │ daemon│   │ retry   │ │ └──────────────────┘   │(doc_   │
└──────────────────┘      └───────┘   │ UX (app)│ │ ┌──────────────────┐   │ maint.)│
┌──────────────────┐                  └─────────┘ └▶│ 06 TUI polish    │   └────────┘
│ 05 fdemon doctor │                                │ (badge + hint)   │   (after
│     CLI          │────────────────────────────────┴──────────────────┘    01-07)
└──────────────────┘
┌──────────────────┐
│ 07 win broadcast │
│  + PATH err tests│
└──────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-disk-network-preflight](tasks/01-disk-network-preflight.md) | Done | - | 4-6h | `toolchain/download.rs`, `flutter_install.rs`, `android_install.rs`, `fdemon-daemon/Cargo.toml` |
| 02 | [02-abortable-downloads-daemon](tasks/02-abortable-downloads-daemon.md) | Done | 01 | 3-4h | `toolchain/download.rs`, `flutter_install.rs`, `android_install.rs` |
| 03 | [03-abort-retry-ux-app](tasks/03-abort-retry-ux-app.md) | Done | 02 | 4-5h | `actions/mod.rs`, `install_wizard/state.rs`, `message.rs`, `handler/install_wizard/actions.rs`, `handler/keys.rs`, `handler/update.rs` |
| 04 | [04-launch-dialog-handback](tasks/04-launch-dialog-handback.md) | Done | 03 | 3-4h | `install_wizard/state.rs`, `handler/install_wizard/actions.rs`, `handler/install_wizard/navigation.rs`, `state.rs` |
| 05 | [05-fdemon-doctor-cli](tasks/05-fdemon-doctor-cli.md) | Done | - | 3h | `src/main.rs`, `src/doctor.rs`, `toolchain/types.rs`, `Cargo.toml` |
| 06 | [06-tui-wizard-polish](tasks/06-tui-wizard-polish.md) | Done | 03 | 2h | `widgets/install_wizard/step_list.rs`, `step_detail.rs` |
| 07 | [07-windows-broadcast-path-tests](tasks/07-windows-broadcast-path-tests.md) | Done | - | 1.5-2h | `toolchain/path_config.rs` |
| 08 | [08-update-docs](tasks/08-update-docs.md) | Done | 01,02,03,04,05,06,07 | 1.5-2h | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/download.rs`, `.../flutter_install.rs`, `.../android_install.rs`, `crates/fdemon-daemon/Cargo.toml` | `toolchain/mod.rs`, root `Cargo.toml` (workspace deps) |
| 02 | `crates/fdemon-daemon/src/toolchain/download.rs`, `.../flutter_install.rs`, `.../android_install.rs` | `fdemon-core/error.rs` (new `Cancelled` variant), `toolchain/process_stream.rs` |
| 03 | `crates/fdemon-app/src/actions/mod.rs`, `.../install_wizard/state.rs`, `.../message.rs`, `.../handler/install_wizard/actions.rs`, `.../handler/keys.rs`, `.../handler/update.rs` | `toolchain` cancel API (task 02), `install_wizard/types.rs` |
| 04 | `crates/fdemon-app/src/install_wizard/state.rs`, `.../handler/install_wizard/actions.rs`, `.../handler/install_wizard/navigation.rs`, `crates/fdemon-app/src/state.rs` | `runner.rs` (startup hook), `handler/mod.rs` (`UpdateAction::DiscoverDevices`) |
| 05 | `src/main.rs`, `src/doctor.rs`, `crates/fdemon-daemon/src/toolchain/types.rs`, `Cargo.toml` (root, binary deps) | `toolchain/mod.rs` (`run_preflight`), `headless/runner.rs` (dispatch pattern) |
| 06 | `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs`, `.../step_detail.rs` | `fdemon-app::install_wizard` (`StepExecStatus`, execution state) |
| 07 | `crates/fdemon-daemon/src/toolchain/path_config.rs` | `toolchain/types.rs` (`HostShell`/`HostPlatform`) |
| 08 | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` | task files 01–07 |

### Overlap Matrix

Wave-peer comparisons (tasks with no dependency edge that may run concurrently):

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 05 + 07 (Wave 1) | None (daemon download path vs binary+types vs `path_config.rs`) | **Parallel (worktree)** — all three are file-disjoint |
| 01 + 02 | `download.rs`, `flutter_install.rs`, `android_install.rs` | **Sequential (same branch)** — enforced by 01→02 dep edge |
| 03 + 04 | `install_wizard/state.rs`, `handler/install_wizard/actions.rs` | **Sequential (same branch)** — enforced by 03→04 dep edge |
| 04 + 06 (Wave 4) | None (app handlers/state vs TUI widgets) | **Parallel (worktree)** |

**Key isolation note:** Two file clusters are shared write-targets and are protected
by dependency edges, not parallelism — the daemon download trio (`download.rs` +
`flutter_install.rs` + `android_install.rs`, 01→02) and the app wizard pair
(`install_wizard/state.rs` + `handler/install_wizard/actions.rs`, 03→04). Do **not**
run 01∥02 or 03∥04 in separate worktrees. The safe parallel sets are Wave 1
(01∥05∥07) and Wave 4 (04∥06).

## Suggested Wave Schedule

- **Wave 1 (parallel):** 01, 05, 07
- **Wave 2:** 02 (after 01)
- **Wave 3:** 03 (after 02)
- **Wave 4 (parallel):** 04 (after 03), 06 (after 03)
- **Wave 5:** 08 docs (after 01–07)

## Success Criteria

Phase 5 is complete when:

- [ ] Before a large download/extract, fdemon checks free disk space on the install
      filesystem (`fs4::available_space`) and surfaces a clear error (required vs
      available bytes) instead of failing mid-extraction.
- [ ] An offline/captive-portal user gets a fast "no network connectivity" error
      (HEAD probe) instead of the 90s (30s idle × 3) stall.
- [ ] An in-flight install download is **abortable**: the wizard stores the task
      handle, `Esc` while a step is `Running` cancels it via a `CancellationToken`,
      the streaming loop exits via `tokio::select!`, and no orphaned `.part` file is
      left (Drop-guard cleanup).
- [ ] After a managed Flutter install succeeds and `flutter_executable()` resolves,
      the wizard **auto-closes and re-triggers device discovery** (and does so on
      manual close too) — the new-session dialog is populated without restarting
      fdemon. Handback fires as soon as Flutter is live, not only when all steps Ok.
- [ ] `fdemon doctor` runs `run_preflight` from any directory, prints each component
      as `[OK]/[!]/[MISS]` plus the captured `flutter doctor` lines, and exits 0 when
      all components are `Ok`, 1 otherwise. The existing positional-path CLI surface
      (`fdemon /path`, `--headless`, `--dap-*`) is preserved.
- [ ] After a failed step, the detail pane shows a "press Enter to retry / r to
      re-check" prompt and the step-list badge shows a run-failed indicator; while a
      step is running the detail pane shows an "Esc cancels" hint.
- [ ] On Windows, the PATH/`ANDROID_HOME` registry write broadcasts
      `WM_SETTINGCHANGE` so already-open terminals/Explorer pick up the change.
- [ ] Test gaps closed: `resolve_stable` empty-manifest, `fetch_release_manifest`
      404 + malformed-JSON (wiremock), exhaustive 9-`ComponentKind` → `WizardStep`
      routing, `add_to_path(PowerShell/Cmd/Unknown)` error path, `fs4` space probe,
      and `CancellationToken` mid-stream abort.
- [ ] `cargo fmt --all`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; no regressions.

## Keyboard Shortcuts (Phase 5 additions)

| Key | Mode | Action |
|-----|------|--------|
| `Esc` | InstallWizard (step `Running`) | Cancel the in-flight install download/step (when not running, `Esc` still closes the wizard) |

## Notes

- **Esc overload:** `Esc` must cancel a running step when one is in progress and
  otherwise close the wizard. Task 03 must branch on `is_step_running()` in
  `handle_key_install_wizard` so cancel takes precedence over close while running.
- **Abort cleanup:** aborting a `tokio::spawn`ed download mid-stream skips the
  natural `.part` cleanup. Task 02 must add a Drop guard owning the `.part` path so
  cancellation never leaks a partial file. The XZ decode thread terminates naturally
  on `BrokenPipe` once the receiver is dropped — verify and document.
- **SDK re-resolution risk:** `settings.flutter.sdk_path` is written on
  `WizardStepCompleted`, but `flutter_executable()` reads `resolved_sdk` on
  `AppState`. Task 04 must confirm/trigger SDK re-resolution before spawning
  discovery, or the device list will be empty despite a successful install.
- **Double-discovery guard:** auto-close (task 04, in `handle_preflight_completed`)
  and a manual `Esc` could both spawn discovery — guard on whether discovery is
  already in flight (e.g. `target_selector.loading`).
- **No new config:** disk/network preflight, abort, handback, and `fdemon doctor`
  add no `[toolchain]` settings — `CONFIGURATION.md` is **not** touched. (A future
  "skip preflight" toggle is a possible enhancement, not Phase 5.)
- **ARCHITECTURE.md change IS warranted** this phase (unlike Phase 4): a new CLI
  `doctor` subcommand + `src/doctor.rs` module, download cancellation/preflight
  surface, and the wizard→device-discovery handback are data-flow/module changes.
  Task 08 routes these to `doc_maintainer`; `KEYBINDINGS.md` (Esc-cancel) rides
  along.
- **Deferred (Future Enhancements):** resumable downloads (HTTP Range/206 — GCS
  supports it; fall back on 200), fish `conf.d` writer migration, `fdemon setup`
  headless install runner.
- **Test placement:** each functional task adds its own unit tests (no separate
  test-only task); the standalone test-coverage gaps from the audit are folded into
  the task that already owns the file (manifest tests → 01, `resolve_stable` +
  doctor `Display` → 05, component-kind routing → 04, PATH error path → 07).
