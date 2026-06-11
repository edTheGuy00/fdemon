# Phase 5 — Windows leaf (Windows-host-only) — Task Index

## Overview

Graduate the **Windows** platform leaf from its Phase-2 inert placeholder (`StepStatus::Pending`, no
components, no guided commands) into a **live detect + guided-only** step, **Windows-host-only**, that
**never blocks** the toolchain-healthy handback. One new `ComponentKind` variant —
`VisualStudioCpp` — is produced by a new Windows-gated probe (`checks/windows.rs`) that locates
`vswhere.exe` and runs a **two-gate** check: gate 1 detects *any* Visual Studio / Build Tools instance,
gate 2 re-queries with `-requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64
Microsoft.VisualStudio.Component.VC.CMake.Project` to confirm the **Desktop development with C++**
workload. Guided commands surface the `winget` Build Tools install (with the `NativeDesktop` workload
override), a `choco` alternative, and — when VS exists but the C++ workload is missing — a
"modify the existing install" entry. Missing Visual Studio surfaces as **Partial (warning)**, never
**Missing** — non-blocking, exactly like Web and iOS/macOS.

> Windows is **detection + guided-only** — there is NO auto-installer (the VS installer is privileged
> GUI/elevated). The leaf is never executable; `Enter` shows guided commands, `c` copies the selected
> command, `[`/`]` cycle commands, `r` re-checks. **No new keybindings, no new config field, no new
> messages.**

### Decisions resolved by research (verified against source)

1. **One new `ComponentKind` variant: `VisualStudioCpp`** (13th variant, after `CocoaPods`).
   `Display` → `"Visual Studio (C++ workload)"`. Distinct from anything in `Prerequisites` — no
   Windows desktop signal exists anywhere today.
2. **Windows-host-gated at the component-push level**, mirroring `check_ios` exactly
   (`crates/fdemon-daemon/src/toolchain/checks/ios.rs:83`): a new
   `check_windows(&platform) -> Vec<ComponentCheck>` returns **empty off-Windows**, **one
   `Unknown`-status check** for `HostPlatform::Unknown`, and **one real check on Windows**. The
   result is `extend`ed onto `components` in `run_preflight` beside `ios_checks`
   (`toolchain/mod.rs:223`). Count becomes **11 on Linux, 13 on macOS, 11 on Windows**. The
   `components.len() >= 10` assertion is already forward-compatible — no change.
3. **Two-gate vswhere probe with a stable detail contract.** `vswhere.exe` is resolved from
   `%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe` (its fixed, documented
   location) with a PATH fallback. Gate 1 (`-products * -latest -format json -utf8`) detects any VS
   instance; gate 2 adds the `-requires` component IDs. Classification:
   - gate 2 hit → `Ok`, detail = display name + version from the JSON.
   - gate 1 hit, gate 2 miss → `Missing`, detail **begins with the stable prefix
     `"Visual Studio found"`** (e.g. `Visual Studio found (<displayName>), but the 'Desktop
     development with C++' workload is missing`). The app-side guided builder branches on this
     prefix — it is a **cross-crate string contract** owned by Task 01 and consumed by Task 03;
     both files document it.
   - no vswhere / no instance → `Missing`, detail = `Visual Studio not found` (vswhere absence noted).
   JSON parsing lives in a **pure classifier** testable on Linux CI (the `classify_xcode_gates`
   pattern); subprocess calls use `PROBE_TIMEOUT`, `.kill_on_drop(true)`, and `strip_and_truncate`
   (the Phase-4-followup hardening pattern).
4. **Daemon reports raw `Missing`; the app caps it to `Partial`** at the leaf in `build_steps` —
   the same local Missing→Partial cap Web and iOS/macOS use (`state.rs` `web_status`/`ios_status`).
   Empty bucket (off-Windows / legacy report) → `Pending`.
5. **No new config field.** The vswhere path is fixed; `winget_available` is already pre-computed on
   `ToolchainReport` (`toolchain/mod.rs:178`) and drives the winget-vs-choco guided branching, the
   same way `web_browser_guided_commands`'s Windows arm uses it.
6. **`handle_step_completed` needs no Windows arm** — guided-only leaves never complete through that
   path (verified: the chain only handles `FlutterSdk`, `PlatformAndroid`, `PathConfig`).

### Why these task boundaries

- `ComponentKind::VisualStudioCpp` hard-errors at exactly **two** exhaustive `match` sites — the daemon
  `Display` impl (`types.rs`) and `build_steps`'s component-routing match (`state.rs:1206`, verified
  exhaustive with no catch-all). The daemon half (Task 01) is a self-contained compiling unit; it lands
  a **minimal no-op stub arm** in `state.rs` so the workspace compiles. Task 03 replaces that stub with
  real bucketing + the leaf body. (Identical to the Phase 3/4 Task-01 cross-crate-stub approach.)
- **All `handler/install_wizard/actions.rs` edits go in Task 02** (fold `PlatformWindows` into the
  guided-only arm, replace the placeholder test). **All `install_wizard/state.rs` edits go in Task 03**
  (bucket, cap, `windows_guided_commands` builder, leaf body). Tasks 02 and 03 are **write-disjoint**
  and parallelize in separate worktrees after Task 01.
- TUI rendering (Task 04) touches only `step_detail.rs` and depends on Task 03 for meaningful tests.
- Docs split in two write-disjoint tasks: `docs/ARCHITECTURE.md` (Task 05, `doc_maintainer`) and the
  **deferred-since-Phase-2 website rewrite** (`website/src/pages/docs/toolchain.rs`, Task 06,
  implementor) — the Phase-2/3/4 notes explicitly parked the website Platforms prose until the Windows
  leaf carried content, i.e. now.

**Total Tasks:** 6
**Estimated Hours:** 9–13 hours

## Task Dependency Graph

```
                ┌──────────────────────────────────────────────┐
                │ 01-daemon-vswhere-detection                    │   Wave 1
                │  ComponentKind::VisualStudioCpp                │
                │  + checks/windows.rs + run_preflight wiring    │
                │  + state.rs no-op stub arm (keeps ws compiling)│
                └───────────────────┬──────────────────────────┘
                                    │  (compiles + daemon tests green)
              ┌─────────────────────┴──────────────────┐
              ▼                                         ▼            Wave 2 (parallel worktrees)
 ┌──────────────────────────────────┐   ┌────────────────────────────────────┐
 │ 02-app-handler-windows-arm        │   │ 03-app-build-steps-windows-leaf     │
 │ fold PlatformWindows into the     │   │ bucket + Missing→Partial cap +      │
 │ guided-only arm (actions.rs only) │   │ windows_guided_commands + leaf body │
 │                                   │   │ (state.rs only)                     │
 └─────────────────┬─────────────────┘   └──────────────────┬─────────────────┘
                   │                                          │
                   │                                          ▼            Wave 3
                   │                          ┌────────────────────────────────────┐
                   │                          │ 04-tui-windows-caption-and-hint     │
                   │                          │ (step_detail.rs)                    │
                   │                          └──────────────────┬─────────────────┘
                   └──────────────────┬──────────────────────────┘
                                      ▼                                  Wave 4 (parallel worktrees)
            ┌─────────────────────────┴───────────────────────────┐
            ▼                                                      ▼
 ┌──────────────────────────────────┐        ┌────────────────────────────────────┐
 │ 05-update-architecture-docs       │        │ 06-update-website-docs              │
 │ (doc_maintainer) ARCHITECTURE.md  │        │ (implementor) toolchain.rs rewrite  │
 └──────────────────────────────────┘        │ Platforms submenu prose/table/art   │
                                              └────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Complexity | Modules |
|---|------|--------|------------|------------|------------|---------|
| 1 | [01-daemon-vswhere-detection](tasks/01-daemon-vswhere-detection.md) | ✅ Done (validated PASS) | - | 3–4h | medium | `fdemon-daemon/src/toolchain/{types,mod}.rs`, `toolchain/checks/{windows,mod}.rs` (+ minimal `fdemon-app/install_wizard/state.rs` stub arm) |
| 2 | [02-app-handler-windows-arm](tasks/02-app-handler-windows-arm.md) | ✅ Done (validated PASS, merged) | 1 | 1h | low | `fdemon-app/src/handler/install_wizard/actions.rs` |
| 3 | [03-app-build-steps-windows-leaf](tasks/03-app-build-steps-windows-leaf.md) | ✅ Done (validated PASS, merged) | 1 | 2–3h | medium | `fdemon-app/src/install_wizard/state.rs` |
| 4 | [04-tui-windows-caption-and-hint](tasks/04-tui-windows-caption-and-hint.md) | ✅ Done (validated PASS) | 3 | 1h | low | `fdemon-tui/src/widgets/install_wizard/step_detail.rs` |
| 5 | [05-update-architecture-docs](tasks/05-update-architecture-docs.md) | ✅ Done (validated PASS, merged) | 1, 2, 3, 4 | 1h | low | `docs/ARCHITECTURE.md` |
| 6 | [06-update-website-docs](tasks/06-update-website-docs.md) | ✅ Done (validated CONCERN¹, merged) | 1, 2, 3, 4 | 2–3h | medium |

> ¹ Task 06 validator CONCERN (minor, deferred): the ASCII wizard mock in `website/src/pages/docs/toolchain.rs` shows a macOS-host view (4 leaves) without a note that the Windows leaf is host-gated; the iOS `xcodes` note is condensed (omits `xcodes install --latest`). Prose/tables/commands are correct for all five leaves. `website/src/pages/docs/toolchain.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/types.rs`, `crates/fdemon-daemon/src/toolchain/checks/windows.rs` (new), `crates/fdemon-daemon/src/toolchain/checks/mod.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs`, `crates/fdemon-app/src/install_wizard/state.rs` (no-op stub arm only) | `toolchain/checks/ios.rs` (probe + pure-classifier template), `toolchain/checks/web.rs` |
| 02 | `crates/fdemon-app/src/handler/install_wizard/actions.rs` | `install_wizard/state.rs` (reads `selected_step().guided_commands` at runtime), `install_wizard/types.rs` |
| 03 | `crates/fdemon-app/src/install_wizard/state.rs` | `toolchain/types.rs` (`ComponentKind::VisualStudioCpp`), `install_wizard/types.rs` (`GuidedCommand`, `StepStatus`), `web_browser_guided_commands` / `xcode_guided_commands` (analogs) |
| 04 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `install_wizard/{types,state}.rs` |
| 05 | `docs/ARCHITECTURE.md` | task 01–04 files, `~/.claude/skills/doc-standards/schemas.md` |
| 06 | `website/src/pages/docs/toolchain.rs` | task 01–04 files, `install_wizard/state.rs` (leaf titles/captions), Phase 2–4 TASKS.md (submenu semantics) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | none (01 = daemon + state.rs stub; 02 = actions.rs) — 02 depends on 01 | Sequential (01 → 02) |
| 01 + 03 | **`install_wizard/state.rs`** (01 lands a no-op stub arm; 03 rewrites it) — 03 depends on 01 | Sequential (01 → 03) |
| **02 + 03** | **none** (02 = `handler/install_wizard/actions.rs`; 03 = `install_wizard/state.rs`) | **Parallel (worktree)** after 01 |
| 04 vs 02/03 | none | Sequential (after 03) |
| **05 + 06** | **none** (`docs/ARCHITECTURE.md` vs `website/src/pages/docs/toolchain.rs`) | **Parallel (worktree)** after 01–04 |
| 05/06 vs 01–04 | none | Sequential (after 01–04) |

> The one overlap on `state.rs` is **between Task 01 and Task 03**, which are **sequential by
> dependency** (03 replaces 01's stub arm) — no parallel conflict. Wave-2 peers 02 + 03 are
> write-disjoint (`actions.rs` vs `state.rs`); Task 02's guided arm reads
> `selected_step().guided_commands` at **runtime** (no compile dep on Task 03's builder), so it is
> correct whichever merges first. Wave-4 peers 05 + 06 are write-disjoint docs.

## Success Criteria

Phase 5 is complete when:

- [ ] `ComponentKind::VisualStudioCpp` exists with a `Display` arm (`"Visual Studio (C++ workload)"`);
      a new `checks/windows.rs` probe emits it as one `ComponentCheck`.
- [ ] `run_preflight` runs the vswhere probe **only on Windows** (`HostPlatform::Windows`); the
      component is present on Windows, absent on Linux/macOS, `Unknown`-status for
      `HostPlatform::Unknown`. The `>= 10` count assertion still holds on every host; Windows presence
      is asserted under `#[cfg(target_os = "windows")]`.
- [ ] `check_windows` resolves `vswhere.exe` (fixed installer path → PATH fallback), runs the two-gate
      probe (any instance / instance with `VC.Tools.x86.x64` + `VC.CMake.Project`), classifies via a
      pure, Linux-CI-testable helper, respects `PROBE_TIMEOUT` with `.kill_on_drop(true)`, caps detail
      via `strip_and_truncate`, and never panics.
- [ ] The "VS present, C++ workload missing" case yields `Missing` with a detail starting with the
      stable `"Visual Studio found"` prefix (documented at both producer and consumer).
- [ ] `build_steps` routes `VisualStudioCpp` into a `platform_windows_components` bucket; the Windows
      leaf's status is `rollup_status(...)` **capped `Missing → Partial`** (empty → `Pending`); guided
      commands populate when `Partial` and are empty when `Ok`.
- [ ] Guided commands: `winget` Build Tools install with the `NativeDesktop` workload override (when
      `winget_available`), a `choco` alternative, and a "modify the existing Visual Studio install"
      entry when the detail carries the `"Visual Studio found"` prefix.
- [ ] A missing Visual Studio is **non-blocking**: `flutter_now_live()` /
      `close_wizard_and_dispatch_discovery` are unaffected; the Platforms parent rolls up to at most
      `Partial`.
- [ ] `handle_run_selected_step` folds `PlatformWindows` into the existing guided-only arm (mirror
      `PlatformWeb`/`PlatformIos`: `has_guided` guard, return `none()`, never
      `begin_step`/`RunWizardStep`); the "Available in a later phase" placeholder arm and its test are
      gone.
- [ ] TUI: the Windows leaf renders a caption + guided-command block with the `c`-copy hint;
      `render_action_hint` suppresses the "coming soon" hint when guided commands are present (no
      dual-CTA), matching Web/iOS/macOS.
- [ ] Host-inapplicable: on Linux/macOS the Windows leaf is **absent** and its component never exists;
      rollups and navigation account only for visible rows.
- [ ] `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D warnings`
      clean.
- [ ] `docs/ARCHITECTURE.md` documents the live Windows leaf, `checks/windows.rs`, the new
      `ComponentKind` variant, the detail-prefix contract, and the non-blocking semantics; the website
      toolchain page describes the Platforms submenu (all five leaves) instead of the legacy
      "Android Tools" step.

## Phase Review

| Round | Verdict | Review | Reviewed HEAD |
|-------|---------|--------|---------------|
| 0 | APPROVED_WITH_CONCERNS | workflow/reviews/features/toolchain-platforms-submenu-phase-5/REVIEW.md | 4d9eb360 |
| fix (user-requested) | ✅ ALL FINDINGS FIXED — integration verify PASS, 7,370 tests / 0 failures | (commits 3b4ff259, 6e49365, 769fd2d) | 769fd2d |

### Review Findings — RESOLVED 2026-06-10 (user-requested fix round)

- ✅ **M1** `VS_FOUND_PREFIX` now `pub` in `checks/windows.rs`, re-exported `checks/mod.rs` → `toolchain/mod.rs` → `lib.rs`, imported by `state.rs`; duplicated `WINDOWS_VS_FOUND_PREFIX` deleted.
- ✅ **M2** Modify-VS guided command branches on `report.shell`: PowerShell gets `Start-Process "${env:ProgramFiles(x86)}\…"`, cmd keeps `start "" "%ProgramFiles(x86)%\…"`; both covered by tests.
- ✅ **M3** Two vswhere gates now run via `tokio::join!` in `probe_visual_studio_cpp` (worst case capped at one `PROBE_TIMEOUT`).
- ✅ **N1** Added `test_windows_guided_commands_existing_vs_no_winget_gives_modify_and_choco`.
- ✅ **N2** Windows Ok-status TUI test now asserts "coming soon" + "flutter doctor" render.
- ✅ **N3** TUI test helper uses production-reachable `StepStatus::Partial`.
- ✅ **N4/N5** Single `strip_and_truncate` in gate-2 classify branch; test uses `super::super::MAX_DETAIL_LEN`.
- ✅ **N7** Website: host-gating callout added beside the ASCII mock; full xcodes note restored verbatim.
- ✅ **Pre-existing test** `test_run_preflight_nonexistent_sdk_path_does_not_panic` fixed. Root cause: an invalid explicit `sdk_path` intentionally falls through the locator strategies (see `test_explicit_path_invalid_falls_through`), so on hosts with Flutter on PATH the SDK resolves Ok and the old `assert_ne!(status, Ok)` was environment-dependent. Rewritten to assert host-invariant consistency invariants (`flutter_sdk.is_some() ⇔ status == Ok`; `doctor` None when no SDK) — matching the test's "does not panic" intent.

### Still deferred (accepted by design, no action planned)

- **N6** choco guided command emitted unconditionally (no `choco_available` signal); the "Requires Chocolatey." note mitigates.
- **N8** Security LOWs accepted: env-var/PATH binary-planting surface identical to all other tool probes; `from_utf8_lossy` and spawn-error→"not found" conflation are pre-existing probe patterns.

## Keyboard Shortcuts

No new keybindings. The Windows leaf reuses the guided-leaf keys established in Phase 3:

| Key | Mode | Action |
|-----|------|--------|
| `Enter` | InstallWizard (Windows leaf selected) | Show guided-command hint message (guided-only; no install) |
| `c` | InstallWizard (Windows leaf selected) | Copy the selected guided command to clipboard |
| `[` / `]` | InstallWizard (Windows leaf selected) | Cycle the selected guided command |
| `r` | InstallWizard | Re-run preflight (re-check Visual Studio) |

## Notes

- **Windows never blocks handback.** `flutter_now_live` checks only `ComponentKind::FlutterSdk == Ok`;
  `close_wizard_and_dispatch_discovery` gates on `flutter_executable().is_some()`. Do **not** add
  `VisualStudioCpp` to either. `all_components_ok()` intentionally stays strict (Phase 3/4 precedent) —
  a Windows host without VS correctly doesn't show "All set".
- **Cross-crate detail-prefix contract.** The `"Visual Studio found"` prefix is the only channel that
  tells the app-side builder "VS exists but the workload is missing" (`ComponentCheck` has no extra
  field, and adding one for this was rejected as over-reach). Task 01 owns the producer, Task 03 the
  consumer; both must carry a comment pointing at the other side. If the string drifts, the only
  symptom is a slightly less specific guided list — degraded, not broken.
- **Guided commands are display-only copy-paste text** — fdemon never runs them. The VS installer
  needs elevation/GUI; present commands verbatim with notes (the Android JDK guided pattern).
- **Dev host is Linux** — all Windows-real execution paths must be exercised through the pure
  classifier with fixture JSON; `#[cfg(target_os = "windows")]` gates the live-probe assertions, which
  will only run on a real Windows host/CI.
- **Locate by symbol, not line.** Line numbers in the task files are a snapshot and will drift.
- **Runtime propagation is out of scope** (Phase 7). Phase 5 is detection + guided display only.
- Phase 6 (Flutter SDK version picker) is independent of this phase and not covered here.
