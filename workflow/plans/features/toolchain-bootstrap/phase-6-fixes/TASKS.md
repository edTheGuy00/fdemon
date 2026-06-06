# Phase 6 Fixes — Install-Wizard UX & Accuracy — Task Index

## Overview

These tasks fix four user-reported defects in the **toolchain-bootstrap install
wizard** discovered after the feature shipped (Phases 1–5 complete on
`feat/toolchain-bootstrap`). All four were traced to precise root causes by a
multi-agent codebase research pass and re-verified against source:

1. **Detail pane is too cramped** — on smaller terminals the right-hand detail
   pane cannot show the full package list or the copy-paste command, and the
   bottom area wastes vertical space. Root cause: the panel is fixed at **70%
   height** / left pane **35% width**, the header reserves **3 rows but only
   writes 2**, and **no text wrapping exists anywhere** — every line renders into
   a 1-row `Rect` and is clipped at the right edge.
2. **Conflicting per-OS install commands** — Prerequisites correctly shows
   `sudo pacman -S …` on Arch, but the Android/JDK step shows `sudo apt install …`
   on the *same* machine. Root cause: `jdk_guided_command(platform)` takes only
   `HostPlatform` and hardcodes apt for all Linux, never consulting the
   pre-computed `report.linux_package_manager` that the Prerequisites path uses.
3. **Already-installed packages still listed** — `curl`/`git` appear in the
   prerequisites install command even when present. Root cause: the daemon
   *does* probe each tool and encodes only-missing ones in `ComponentCheck.detail`,
   but the Linux branch of `prerequisites_guided_commands` emits a **static full
   package list**, ignoring `parse_missing_prereq_keys` (which macOS/Windows
   already use to filter).
4. **`ANDROID_HOME` / adb not on PATH** — after install, `adb` and friends are not
   available in the user's terminal. Root cause: `add_android_env` writes
   `cmdline-tools/latest/bin` + `platform-tools` but **omits `emulator/`**, and is
   only invoked when the wizard's own AndroidTools step ran — it never falls back
   to `resolve_android_sdk_root_path` (`$ANDROID_HOME` / platform default), so an
   out-of-band Android SDK gets `ANDROID_HOME` silently skipped.

### Design decisions (resolved with the user)

- **Bug 1 — pragmatic wrap + resize.** Enlarge the panel (height 70 → 85%, left
  pane 35 → 28%, header 3 → 2 rows) **and** add real line-wrapping to the
  guided-command / component / doctor lines (per-item `line_count` to advance `y`),
  keeping the existing line-offset scroll model.
- **Bug 3 — add daemon probes.** Extend `check_linux_prerequisites` to probe the
  currently-unprobed packages (GLU via `pkg-config`, `libstdc++`) so the filtered
  install command lists **only** genuinely-missing packages — nothing
  already-installed is ever shown.

## Defect → Task Map

| Defect | Area | Task |
|--------|------|------|
| Detail pane cramped (h/v), long lines clipped, bottom bar oversized, no wrapping | tui-layout | 01 |
| Android/JDK guided command hardcodes apt on non-Debian Linux | app-guided-cmd | 02 |
| Prerequisites command lists already-installed packages | app-guided-cmd + daemon-probe | 02 |
| `ANDROID_HOME` skipped + `emulator/` missing from PATH + no out-of-band fallback | daemon-path + app-actions | 03 |
| ARCHITECTURE reflects the wrapping, per-OS command, probe, and PATH changes | docs | 04 |

## Task Dependency Graph

```
All three implementation tasks are file-disjoint → safe to run in parallel worktrees.

  ┌───────────────────────────┐  ┌───────────────────────────┐  ┌───────────────────────────┐
  │ 01 detail-pane layout +   │  │ 02 OS-accurate + filtered │  │ 03 ANDROID_HOME / PATH    │
  │    wrapping (TUI only)    │  │    guided commands        │  │    fallback + emulator    │
  │                           │  │    (app state + daemon    │  │    (daemon path_config +  │
  │                           │  │     prereq probe)         │  │     app actions)          │
  └───────────────────────────┘  └───────────────────────────┘  └───────────────────────────┘
                    └──────────────────────┬──────────────────────┘
                                           ▼
                          ┌───────────────────────────┐
                          │ 04 docs (after 01–03)     │
                          │    doc_maintainer         │
                          └───────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Agent | Est. Hours | Modules |
|---|------|--------|------------|-------|------------|---------|
| 01 | [01-detail-pane-layout-wrapping](tasks/01-detail-pane-layout-wrapping.md) | ✅ Done | - | implementor | 4–6h | `widgets/install_wizard/{mod,step_detail,doctor_view}.rs` |
| 02 | [02-os-accurate-filtered-commands](tasks/02-os-accurate-filtered-commands.md) | ✅ Done (concern) | - | implementor | 4–6h | `install_wizard/{state,types}.rs`, `toolchain/checks/prerequisites.rs` |
| 03 | [03-android-home-path-fallback](tasks/03-android-home-path-fallback.md) | ✅ Done (concern) | - | implementor | 3–4h | `toolchain/path_config.rs`, `actions/mod.rs`, `handler/install_wizard/actions.rs` |
| 04 | [04-update-docs](tasks/04-update-docs.md) | ✅ Done | 01,02,03 | doc_maintainer | 1–1.5h | `docs/ARCHITECTURE.md` |

**Validation notes:**
- Task 01 — PASS, all 5 acceptance criteria met.
- Task 02 — CONCERN (non-breaking): `types.rs` test fixtures (~lines 179–186) still hardcode the old `sudo apt install openjdk-17-jdk` string rather than the new per-manager output. Assertions are loose (`contains("17")`/`is_some()`) so tests pass; functional criteria all met. Apt arm's JDK note mentions pacman as the alternative hint (cosmetic).
- Task 03 — CONCERN (non-breaking): stale comment at `path_config.rs:776` ("Prepend the two bin dirs" — now three); `test_pathconfig_hints_when_android_sdk_root_absent` defensively gates its hint assertion behind `dispatched_sdk_root.is_none()` to stay robust on CI machines with an SDK at the platform default. Production code correct; all criteria met.
- Integrated quality gate (combined 01+02+03): `cargo fmt --check`, `cargo check`, `cargo clippy -D warnings`, `cargo test --workspace` all PASS (0 failures).

**Total Tasks:** 4
**Estimated Hours:** 12–17.5 hours

## File Overlap Analysis

### Files Modified (Write) per task

| Task | Files Modified (Write) |
|------|------------------------|
| 01 | `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`, `crates/fdemon-tui/src/widgets/install_wizard/doctor_view.rs` |
| 02 | `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/install_wizard/types.rs`, `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` |
| 03 | `crates/fdemon-daemon/src/toolchain/path_config.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs` |
| 04 | `docs/ARCHITECTURE.md` |

### Files Read (Dependencies, read-only — no conflict)

- 01 reads: `install_wizard/state.rs` (for `WizardStep`/`GuidedCommand`/`detail_scroll` shape) — read-only.
- 02 reads: `toolchain/types.rs` (`LinuxPackageManager`, `ComponentCheck`), `toolchain/checks/mod.rs` (`parse_missing_prereq_keys`, `PREREQ_KEY_*`) — read-only.
- 03 reads: `toolchain/checks/android.rs` (`resolve_android_sdk_root_path`) — read-only.

### Overlap Matrix (wave-peer tasks 01, 02, 03)

| Pair | Shared write files | Strategy |
|------|--------------------|----------|
| 01 ↔ 02 | none (TUI widgets vs app state + daemon checks) | **Parallel (worktree)** |
| 01 ↔ 03 | none (TUI widgets vs daemon path_config + app actions) | **Parallel (worktree)** |
| 02 ↔ 03 | none (app state + daemon checks vs daemon path_config + app actions) | **Parallel (worktree)** |

All three implementation tasks have disjoint write-sets, so they run concurrently
in isolated worktrees with zero merge conflict. Task 04 (docs) runs after all three
land (it documents their combined behaviour) and is the only task that writes
`docs/ARCHITECTURE.md`.

> **Note on test-only fixtures:** the TUI test helpers and doc-comment in
> `step_detail.rs` (`make_state_android_jdk_missing`, line ~1265) embed the literal
> `sudo apt install openjdk-17-jdk`. These are independent constructors — they do
> **not** call the production `jdk_guided_command`, so Task 02's signature change
> does not break them. Task 01 owns `step_detail.rs`, so it refreshes those
> fixtures/doc-comment to the new per-manager example as a minor cleanup; this is
> cosmetic and not load-bearing for either task.

## Suggested Wave Schedule

- **Wave 1 (parallel worktrees):** 01, 02, 03
- **Wave 2:** 04 docs (after 01–03 merge to the integration branch)

## Success Criteria

Phase 6 fixes are complete when:

- [ ] **Bug 1:** On an 80×24 terminal the detail pane shows the full guided
      command(s) and package list — long lines **wrap** instead of clipping; the
      panel uses ~85% height and the header occupies 2 rows; `cargo test`
      rendering snapshots confirm wrapped content is present and the bottom
      separator/footer is unchanged at 1 row each.
- [ ] **Bug 2:** On a machine where `report.linux_package_manager == Pacman`, the
      AndroidTools/JDK guided command is `sudo pacman -S jdk17-openjdk` (not apt);
      dnf/yum/zypper/apt each yield their correct JDK package; macOS/Windows
      unchanged. `jdk_guided_command` takes `&ToolchainReport` and dispatches on
      `report.linux_package_manager`.
- [ ] **Bug 3:** The daemon probes GLU and libstdc++ in addition to the existing
      tools; `prerequisites_guided_commands` (Linux) calls `parse_missing_prereq_keys`
      and emits an install command containing **only** the missing packages. With
      `curl` + `git` present they no longer appear in the command; when *all*
      prerequisites are present no command is shown.
- [ ] **Bug 4:** `add_android_env` writes `ANDROID_HOME` plus
      `cmdline-tools/latest/bin`, `platform-tools`, **and `emulator/`** on bash/zsh/
      fish/Windows. The PathConfig executor falls back to
      `resolve_android_sdk_root_path(None)` (filtered by `is_dir()`) when
      `settings.toolchain.android_sdk_root` is `None`, so an out-of-band SDK
      (`$ANDROID_HOME` / platform default) still gets `ANDROID_HOME` written.
- [ ] `cargo fmt --all --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, and `cargo clippy --workspace --all-targets
      -- -D warnings` all pass; no regressions.

## Notes

- **No new config keys, no new keybindings.** These are correctness/UX fixes to
  existing surfaces. `CONFIGURATION.md` / `KEYBINDINGS.md` are not touched.
  `ARCHITECTURE.md` gets the wrapping, per-manager-command, GLU/libstdc++ probe,
  and `emulator/`+fallback notes (Task 04, routed to `doc_maintainer`).
- **Bugs 2 and 3 share `state.rs`** and are both about the same
  `*_guided_commands` family, so they are deliberately combined into Task 02 to
  avoid a same-file serial dependency and to keep the per-OS command logic in one
  reviewable change.
- **`tools/bin/` is intentionally excluded** from the Android PATH block — it was
  deprecated in Android SDK 26.0.0 (2017) and superseded by cmdline-tools.
