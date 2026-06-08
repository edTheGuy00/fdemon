# Phase 5 Followup — Review Remediation — Task Index

## Overview

These tasks remediate the confirmed findings from a multi-agent adversarial review
of the **Phase 5** implementation (git range `56a2f95..HEAD`). Nine specialised
reviewers fanned out across the eight Phase 5 task areas (download safety,
abort/retry, launch handback, doctor CLI, TUI polish, Windows broadcast,
test-coverage, architecture/concurrency); every raw finding was then independently
re-verified by a second agent prompted to **refute** it.

Result: **26 confirmed** findings (1 of 27 raw findings was rejected — the
"handback dialog renders empty / configs never load" claim was refuted: startup's
`startup_flutter` loads launch configs into the dialog before the wizard ever
opens). Severity breakdown: **1 CRITICAL, 3 HIGH, 10 MEDIUM, 10 LOW, 2 NIT**.

The Phase 5 quality gate itself is **green**: `cargo fmt --all --check`,
`cargo check --workspace --all-targets`, `cargo test --workspace`
(~8954 pass / 0 fail / 116 pre-existing ignores), and
`cargo clippy --workspace --all-targets -- -D warnings` all pass on the reviewed tree.
Every finding below is a latent correctness/UX/test-quality gap, not a build break.

### Headline items

- **F1 (CRITICAL):** the *primary* handback path — managed Flutter install
  succeeds → wizard auto-closes → re-discover devices — leaves `UiMode::Normal`,
  but `DevicesDiscovered` only populates the new-session dialog when the mode is
  `Startup`/`NewSessionDialog`. Discovered devices are **silently dropped**, so the
  headline Phase 5 feature ("the new-session dialog is populated without restarting
  fdemon") does not actually work on the auto-close path. The manual-close path
  (which correctly sets `Startup`) is fine. Existing tests passed because they only
  assert `ui_mode != InstallWizard`.
- **F3/F4/F7/F8 (HIGH/MEDIUM races):** the abort handle (`install_task`) is
  delivered to state **asynchronously** via a `WizardInstallTaskReady` message sent
  from a *separate* `tokio::spawn` than the one emitting step lifecycle events. This
  opens several windows where `Esc` cancels the wrong (already-finished) task while
  the real download keeps running — defeating the core "in-flight install is
  abortable" guarantee — and where a stale token survives into the next step or
  leaks the RAII install lock. The token must be stored **synchronously** at
  `begin_step` and the ready message must be **self-validating** (carry the step
  kind + a run sequence id).
- **F2 (HIGH test):** the single acceptance test for mid-stream cancellation is
  genuinely **flaky** (reproduced 1-in-5 to 2-in-15 failures) — a loopback body can
  arrive in one chunk and the download completes before cancellation is observed.

## Finding → Task Map

| Finding | Sev | Area | Task |
|---------|-----|------|------|
| F1 auto-close handback leaves `UiMode::Normal`; discovered devices dropped | CRITICAL | launch-handback | 01 |
| F10 handback tests assert only `!= InstallWizard`, never devices reach dialog | MEDIUM | launch-handback | 01 |
| F3 Esc during handle-arrival window orphans install task + leaks lock | HIGH | concurrency | 02 |
| F4 late `WizardInstallTaskReady` clobbers a retried run's handle | HIGH | concurrency | 02 |
| F7 ready msg races after terminal msg, re-installs stale token | MEDIUM | abort-retry | 02 |
| F8 stale `install_task` survives into next step; Esc cancels wrong task | MEDIUM | abort-retry | 02 |
| F19 `hide_install_wizard` never clears/cancels `install_task` | LOW | abort-retry | 02 |
| F9 no test for ready-after-terminal ordering / stale-token-across-steps | MEDIUM | abort-retry | 02 |
| F6 user cancel can render as red "Failed" summary (AC#4) | MEDIUM | cancel-ux | 03 |
| F12 cancel races into `WizardStepFailed` → red run-failed badge + retry prompt | MEDIUM | cancel-ux | 03 |
| F11 run-failed badge identical to plain Missing badge (common case) | MEDIUM | tui-polish | 03 |
| F17 cancel reason double-prefixed ("Cancelled: Cancelled: …") | LOW | cancel-ux | 03 |
| F18 actions-layer `is_cancelled()` → "Cancelled:" mapping untested | LOW | cancel-ux | 03 |
| F5 `IDLE_TIMEOUT` applied as total-request deadline, not per-read idle | MEDIUM | download | 04 |
| F14 abort() backstop leaks install temp dir (no RAII cleanup) | MEDIUM | download | 04 |
| F15 disk budget double-counted on same filesystem | LOW | download | 04 |
| F16 connectivity probe passes captive portals (doc/limitation) | LOW | download | 04 |
| F23 `git_install` not cancellable via token (only archive path is) | LOW | download | 04 |
| F26 `tokio-util` `rt` feature unnecessary (only `sync` used) | NIT | cleanup | 04 |
| F2 `cancel_mid_stream` test is flaky (one-chunk body) | HIGH | test | 05 |
| F13 manifest 404/malformed tests replicate logic, never call the fn | MEDIUM | test | 05 |
| F20 `{:>4}` width specifier dead — Display ignores formatter width | LOW | doctor | 06 |
| F24 doctor ignores configured `[flutter] sdk_path` (always `None`) | LOW | doctor | 06 |
| F25 a real `./doctor` project can't be launched via `fdemon doctor` | NIT | doctor | 06 |
| F21 Windows broadcast tests assert on a duplicated literal | LOW | windows | 07 |
| F22 broadcast PowerShell `.output()` has no Rust-side timeout | LOW | windows | 07 |
| (docs) ARCHITECTURE/KEYBINDINGS reflect the above behavioural fixes | — | docs | 08 |

## Task Dependency Graph

```
Chain A — app wizard files (handler/install_wizard/actions.rs, state.rs, navigation.rs,
          message.rs, actions/mod.rs, install_wizard/types.rs, +TUI rendering) — SERIAL
  ┌──────────────────────────┐   ┌──────────────────────────┐   ┌──────────────────────────┐
  │ 01 handback auto-close    │──▶│ 02 install-task handle    │──▶│ 03 cancelled-state        │
  │    UiMode fix (CRITICAL)  │   │    lifecycle race (HIGH)  │   │    rendering (MEDIUM)     │
  └──────────────────────────┘   └──────────────────────────┘   └──────────────────────────┘

Chain B — daemon download path (download.rs, flutter_install.rs, android_install.rs) — SERIAL
  ┌──────────────────────────┐   ┌──────────────────────────┐
  │ 04 download robustness    │──▶│ 05 daemon download tests  │
  │    (MEDIUM/LOW/NIT)       │   │    (HIGH/MEDIUM)          │
  └──────────────────────────┘   └──────────────────────────┘

Parallel disjoint (no shared files with A/B or each other)
  ┌──────────────────────────┐   ┌──────────────────────────┐
  │ 06 doctor CLI fixes (LOW) │   │ 07 windows broadcast (LOW)│
  └──────────────────────────┘   └──────────────────────────┘

  ┌──────────────────────────┐
  │ 08 docs (after 01–07)     │
  └──────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Sev (max) | Est. Hours | Modules |
|---|------|--------|------------|-----------|------------|---------|
| 01 | [01-handback-autoclose-uimode](tasks/01-handback-autoclose-uimode.md) | ✅ Done | - | CRITICAL | 1.5-2h | `handler/install_wizard/actions.rs`, `handler/install_wizard/navigation.rs` |
| 02 | [02-install-task-handle-race](tasks/02-install-task-handle-race.md) | ✅ Done | 01 | HIGH | 5-7h | `install_wizard/state.rs`, `handler/install_wizard/actions.rs`, `message.rs`, `actions/mod.rs`, `handler/install_wizard/navigation.rs`, `state.rs` |
| 03 | [03-cancelled-state-rendering](tasks/03-cancelled-state-rendering.md) | ✅ Done | 02 | MEDIUM | 3-4h | `install_wizard/types.rs`, `install_wizard/state.rs`, `handler/install_wizard/actions.rs`, `actions/mod.rs`, `widgets/install_wizard/{progress,step_list,step_detail,mod}.rs` |
| 04 | [04-download-pipeline-robustness](tasks/04-download-pipeline-robustness.md) | ✅ Done | - | MEDIUM | 4-5h | `toolchain/download.rs`, `toolchain/flutter_install.rs`, `toolchain/android_install.rs`, `Cargo.toml` |
| 05 | [05-daemon-download-tests](tasks/05-daemon-download-tests.md) | ✅ Done | 04 | HIGH | 3-4h | `toolchain/download.rs`, `toolchain/flutter_install.rs` |
| 06 | [06-doctor-cli-fixes](tasks/06-doctor-cli-fixes.md) | ✅ Done | - | LOW | 1.5-2h | `src/doctor.rs`, `src/main.rs`, `toolchain/types.rs` |
| 07 | [07-windows-broadcast-hardening](tasks/07-windows-broadcast-hardening.md) | ✅ Done | - | LOW | 1.5-2h | `toolchain/path_config.rs` |
| 08 | [08-update-docs](tasks/08-update-docs.md) | ✅ Done | 01,02,03,04,06 | — | 1-1.5h | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` |

**Total Tasks:** 8
**Estimated Hours:** 21–28 hours

## Suggested Wave Schedule

- **Wave 1 (parallel):** 01 (chain A start), 04 (chain B start), 06, 07
- **Wave 2:** 02 (after 01), 05 (after 04)
- **Wave 3:** 03 (after 02)
- **Wave 4:** 08 docs (after 01–06)

**Isolation note:** Chain A (01→02→03) all touch
`handler/install_wizard/actions.rs` + `install_wizard/state.rs`; Chain B (04→05)
both touch `download.rs`/`flutter_install.rs`. Do **not** run those pairs in
parallel worktrees — serialise them on the same branch. 06 (`src/` + daemon
`types.rs`) and 07 (`path_config.rs`) are file-disjoint from everything and from
each other, so Wave 1's four entry points are safe to parallelise.

## Success Criteria

Phase 5 followup is complete when:

- [ ] After a managed Flutter install succeeds, the auto-close handback leaves
      `ui_mode == Startup` and discovered devices actually populate the new-session
      dialog's target selector (F1) — proven by a test that drives
      `DevicesDiscovered` through `update()` and asserts the selector is non-empty (F10).
- [ ] The abort handle is stored **synchronously** at `begin_step`, so `Esc`
      during the ready-arrival window cancels the real running install and releases
      the install lock (F3); `WizardInstallTaskReady` carries the step kind + a run
      sequence id and is discarded when it does not match the current run (F4/F7);
      `begin_step` and `hide_install_wizard` clear any prior handle (F8/F19);
      tests cover ready-after-terminal and stale-token-across-steps (F9).
- [ ] A user-initiated cancellation never renders as a red "Failed" badge/summary:
      a dedicated `StepExecStatus::Cancelled` state renders neutrally and suppresses
      the run-failed badge + retry-as-failure framing (F6/F12); the run-failed badge
      is visually **distinct** from a plain `Missing` badge (F11); the cancel reason
      is not double-prefixed (F17); the actions-layer cancel mapping is tested (F18).
- [ ] `IDLE_TIMEOUT` is a per-read idle guard (`read_timeout`), not a total-request
      deadline, so legitimate slow large downloads are not aborted (F5); a cancelled
      or aborted install never leaks its extraction temp dir (RAII cleanup, F14);
      the disk budget is not double-counted (F15); the captive-portal limitation is
      documented (F16); `git_install` honours the cancel token (F23); the
      `tokio-util` `rt` feature is dropped (F26).
- [ ] The mid-stream cancellation test is deterministic (F2) and the manifest
      404/malformed tests exercise the real `fetch_release_manifest` path (F13).
- [ ] The doctor report status column aligns (F20); `fdemon doctor` honours
      `[flutter] sdk_path` (F24); the `./doctor` collision is documented (F25).
- [ ] Windows broadcast tests assert against a single shared constant (F21) and the
      broadcast invocation cannot block the wizard thread indefinitely (F22).
- [ ] `cargo fmt --all --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; no regressions.

## Notes

- **Root cause behind F3/F4/F7/F8/F19:** the async, separately-spawned
  `WizardInstallTaskReady` handoff is the common thread. Task 02 is the load-bearing
  fix — store the `CancellationToken` synchronously at `begin_step`, reuse it in
  `handle_action`, and demote `WizardInstallTaskReady` to *upgrading* the stored
  handle's `join` field (guarded by kind + run-seq). This collapses all five
  findings into one coherent redesign rather than five point-patches.
- **F6/F12/F17 are one fix:** introduce `StepExecStatus::Cancelled`. Routing the
  "Cancelled:" terminal branch through `Cancelled` (instead of `Failed`)
  automatically suppresses `failed_execution_kind()` → red badge, lets the renderer
  pick a neutral colour, and removes the brittle `starts_with("Cancelled:")`
  string-coupling. Task 03 also fixes F11 (distinct run-failed glyph/modifier) since
  it touches the same `step_list.rs` rendering.
- **Rejected finding (do NOT action):** "neither handback path loads launch configs,
  dialog is empty." Refuted — `startup_flutter` calls `show_new_session_dialog(configs)`
  with `load_all_configs(project_path)` before the wizard opens, so configs are
  already loaded. F1's real defect is the `UiMode::Normal` transition, not config loading.
- **Latent pre-existing bug disclosed:** F5 (`.timeout()` vs `.read_timeout()`) predates
  Phase 5 (the wiring is unchanged since base `56a2f95`), but Phase 5's cancellation
  rewrite shares that loop and left the misleading "Idle/stall guard" doc in place, so
  it is in-scope to fix here.
- **No new config / no new keybindings:** these are all internal correctness/UX/test
  fixes. `CONFIGURATION.md` is not touched. `KEYBINDINGS.md` only gets a clarifying
  note if Task 03 changes how cancel is surfaced; `ARCHITECTURE.md` gets the handback
  `UiMode::Startup` correction and the RAII temp-dir/cancel-token notes.
