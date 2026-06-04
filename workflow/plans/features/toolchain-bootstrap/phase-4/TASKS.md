# Phase 4 — OS Prerequisites (guided + safe-auto) — Task Index

## Overview

Phase 4 makes the **Prerequisites** wizard step actionable as a **guided** step:
it surfaces, per-OS, exactly which platform build prerequisites are missing and
shows copy-paste install commands the user runs manually, then re-checks live with
`r`. There is **no privileged auto-run** — Phase 4 is overwhelmingly a
*guided-command + detection-refinement* effort, not a new-executor effort.

The entire copy / re-check / multi-command-render plumbing already exists from
Phase 3: `r` (`InstallWizardRerunPreflight` → `RunToolchainPreflight` →
`apply_report` rebuilds all steps), `c` (`handle_copy_command` →
`selected_guided_command` → `WriteClipboard`), and the N-command render loop in
`step_detail.rs`. The only genuinely new code is:

1. **Detection refinement** in `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`
   (currently a coarse 7-tool Linux `which`-probe, a single `xcode-select` macOS
   gate, and a git-proxy Windows stub) — add package-manager identification, a GTK
   dev-headers probe, and macOS Rosetta/CocoaPods + Windows winget probes.
2. **Guided-command population** in `crates/fdemon-app/src/install_wizard/state.rs`:
   the `Prerequisites` step's `guided_commands` is hardcoded `Vec::new()`
   (`state.rs:357`); a new `prerequisites_guided_commands()` helper (mirroring the
   existing `jdk_guided_command`, `state.rs:238`) populates it.
3. **Per-command navigation** so macOS can show its 2–3 genuinely-independent
   commands (Xcode CLT, CocoaPods, Rosetta) as individually-copyable entries.
   Today a step structurally holds a `Vec<GuidedCommand>` and the renderer loops
   over all of them, **but** `c` only copies `.first()` (`state.rs:115-117`) and
   only command index 0 shows the `[c] copy` hint (`step_detail.rs:316`). Phase 4
   adds a `selected_command_index` + `[`/`]` navigation keys.

**Total Tasks:** 6
**Estimated Hours:** 19–28 hours

**Platform scope:** detection + guided commands on **Linux, macOS, and Windows**.
All Phase-4 install actions are **guided** (copy-paste + re-check) — nothing is
auto-run with `sudo`/`brew`/`winget`/GUI installers, honoring the PLAN "Hybrid"
decision.

## Scope Decisions (resolved with the requester)

- **macOS shows multiple independently-copyable commands** → per-command navigation
  is in scope (task 04 adds `selected_command_index` + `[`/`]` keys). Without it
  only the first macOS command would be copyable.
- **Linux emits the full canonical package list** whenever anything is missing
  (apt/dnf skip already-installed packages, so this is idempotent and robust) —
  the Linux guided command is *not* dynamically trimmed package-by-package. macOS,
  by contrast, **is** trimmed to the missing items (CLT/CocoaPods/Rosetta), which
  is why detection exposes per-item missing keys (see task 02 contract).
- **Windows Visual Studio "Desktop development with C++" workload detection
  (`vswhere.exe`) is deferred** (out of scope). Phase 4 Windows detection covers
  git + winget only; VS C++ is mentioned as a manual note in the detail text.

## Architecture Recap (what already exists from Phases 1–3)

- **Detection (read-only, refined here):** `toolchain/checks/prerequisites.rs`
  already aggregates `ComponentKind::Prerequisites` (+ `Git`) into **one** check
  per OS via `which`/`xcode-select`. `ComponentCheck` carries only
  `{ kind, status: ComponentStatus, detail: String }` (`types.rs`); Phase 4 keeps
  this shape (no new fields/variants) and makes the missing list machine-parseable.
- **Step assembly (extended here):** `install_wizard/state.rs::build_steps`
  (`state.rs:285-358`) groups checks into the 5 steps and derives the
  `AndroidTools` guided command via `is_jdk_actionable` + `jdk_guided_command`
  (`state.rs:238-272`, `345-349`). The `Prerequisites` step is built with
  `guided_commands: Vec::new()` (`state.rs:357`) — the single line Phase 4 replaces.
- **Guided-command UI (reused):** `GuidedCommand { label, command, note }`
  (`install_wizard/types.rs:11-19`); rendered by `render_guided_commands`
  (`step_detail.rs:255-338`) with a JDK caption gated on `AndroidTools`
  (`step_detail.rs:279-290`) and a `[c] copy` hint on command `i==0`
  (`step_detail.rs:316`).
- **Keys/messages (reused + extended):** `handle_key_install_wizard`
  (`keys.rs:~413-441`) routes `r`→`InstallWizardRerunPreflight`,
  `c`→`InstallWizardCopyCommand`, `j`/`k`→`InstallWizardUp/Down`,
  `Tab`→`InstallWizardSwitchPane`. `handle_copy_command` (`actions.rs:372-378`)
  copies `selected_guided_command()`. Phase 4 adds `[`/`]` for per-command nav.
- **Re-check loop (reused as-is):** pressing `r` re-runs preflight and
  `apply_report` rebuilds every step — no Phase-4 change needed for live re-check.
- **Executor guard (stays unreached):** the `Prerequisites`/`Doctor` arm in
  `actions/mod.rs:1103` and the handler stub in
  `handler/install_wizard/actions.rs:211-215` ("Available in a later phase").
  Phase 4 keeps `Prerequisites` **non-executable** (Enter never dispatches
  `RunWizardStep`) and only relaxes the status message.

## Task Dependency Graph

```
Wave 1        Wave 2        Wave 3        Wave 4         Wave 5 (parallel)
┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐    ┌──────────────────┐
│ 01      │──▶│ 02      │──▶│ 03      │──▶│ 04      │──┬▶│ 05 TUI detail    │
│ linux   │   │ mac/win │   │ guided  │   │ per-cmd │  │ │    (step_detail) │
│ detect  │   │ detect  │   │ commands│   │ nav     │  │ └──────────────────┘
└─────────┘   └─────────┘   └─────────┘   └─────────┘  │ ┌──────────────────┐
 (prereq.rs    (prereq.rs    (state.rs +   (state.rs +  └▶│ 06 KEYBINDINGS   │
  chain)        chain)        actions.rs)   msg/keys/    │ │   (doc_maintainer)│
                                            update/nav)  │ └──────────────────┘
                                                         05 also depends on 03
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-linux-prereq-detection](tasks/01-linux-prereq-detection.md) | Not Started | - | 4-6h | `toolchain/checks/prerequisites.rs` |
| 02 | [02-macos-windows-prereq-detection](tasks/02-macos-windows-prereq-detection.md) | Not Started | 01 | 4-6h | `toolchain/checks/prerequisites.rs` |
| 03 | [03-prerequisites-guided-commands](tasks/03-prerequisites-guided-commands.md) | Not Started | 01, 02 | 4-6h | `install_wizard/state.rs`, `handler/install_wizard/actions.rs` |
| 04 | [04-per-command-navigation](tasks/04-per-command-navigation.md) | Not Started | 03 | 4-6h | `install_wizard/state.rs`, `message.rs`, `handler/keys.rs`, `handler/update.rs`, `handler/install_wizard/navigation.rs` |
| 05 | [05-tui-prereq-detail-render](tasks/05-tui-prereq-detail-render.md) | Not Started | 03, 04 | 2-3h | `widgets/install_wizard/step_detail.rs` |
| 06 | [06-update-keybindings-docs](tasks/06-update-keybindings-docs.md) | Not Started | 04, 05 | 1h | `docs/KEYBINDINGS.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` | `toolchain/types.rs`, `toolchain/checks/mod.rs` |
| 02 | `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` | `toolchain/types.rs` |
| 03 | `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs` | `install_wizard/types.rs`, `toolchain/checks/prerequisites.rs` (missing-key contract) |
| 04 | `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/install_wizard/navigation.rs` | `install_wizard/types.rs` (`GuidedCommand`, `WizardPane`) |
| 05 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `fdemon-app::install_wizard` (`selected_command_index`, `GuidedCommand`, `WizardStepKind`) |
| 06 | `docs/KEYBINDINGS.md` | task files 03, 04 |

### Overlap Matrix

Wave-peer comparisons (tasks with no dependency edge that may run concurrently):

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | `toolchain/checks/prerequisites.rs` | **Sequential (same branch)** — enforced by the 01→02 dep edge |
| 03 + 04 | `install_wizard/state.rs` | **Sequential (same branch)** — enforced by the 03→04 dep edge |
| 05 + 06 | None (`step_detail.rs` vs `KEYBINDINGS.md`) | **Parallel (worktree)** |

**Key isolation note:** Phase 4 is a tightly-coupled vertical slice. Two files are
shared write-targets across tasks and are protected by dependency edges, not
parallelism: `prerequisites.rs` (01→02) and `state.rs` (03→04). The only safe
parallel pair is the final TUI + docs wave (05 ∥ 06). Do **not** run 01∥02 or
03∥04 in separate worktrees — they would conflict on the same file.

## Suggested Wave Schedule

- **Wave 1:** 01
- **Wave 2:** 02 (after 01)
- **Wave 3:** 03 (after 02)
- **Wave 4:** 04 (after 03)
- **Wave 5 (parallel):** 05 (after 03, 04), 06 (after 04, 05)

## Success Criteria

Phase 4 is complete when:

- [ ] On a machine missing OS build prerequisites, the **Prerequisites** step shows
      a per-OS guided install command (Linux: full canonical package list for the
      detected package manager; macOS: only the missing items among Xcode CLT /
      CocoaPods / Rosetta; Windows: Git for Windows) — and shows **no** command when
      every prerequisite is already `Ok`.
- [ ] On Linux, the correct package-manager command is chosen by detecting
      apt-get → dnf → yum → pacman → zypper; an unknown manager falls back to a docs
      URL. The GTK dev-headers presence is probed via `pkg-config --exists gtk+-3.0`
      (which `which` cannot detect).
- [ ] On macOS, Rosetta is probed **only on Apple Silicon** (`ARCH == "aarch64"`)
      via `pgrep oahd`; CocoaPods via `pod --version`; Xcode CLT via `xcode-select -p`.
- [ ] On Windows, git is detected via `which`; when missing and `winget` is present
      the command is `winget install Git.Git`, otherwise the git-scm.com download
      URL is shown. (VS C++ workload detection is explicitly out of scope.)
- [ ] The macOS Prerequisites step lists its missing commands as **individually
      copyable** entries: `[`/`]` move `selected_command_index`, `c` copies the
      selected command, and the `[c]`/highlight follows the selection.
- [ ] Pressing `Enter` on the Prerequisites step does **not** run a privileged
      install; the status message guides the user to run the command and press `r`.
      Pressing `r` re-runs preflight and the Prerequisites status flips to `Ok`
      once the tools are installed — without restarting fdemon.
- [ ] All new code has unit tests (package-manager detection precedence; GTK
      probe mapping; ARCH-gated Rosetta; per-OS command generation; empty when all
      `Ok`; missing-key parse round-trip; per-command navigation clamping + copy of
      the selected index; render of caption + per-OS command + absence of the
      "later phase" hint for Prerequisites-with-commands).
- [ ] `cargo fmt --all`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; no regressions.

## Keyboard Shortcuts (Phase 4 additions)

| Key | Mode | Action |
|-----|------|--------|
| `[` | InstallWizard | Select the previous guided command on the current step |
| `]` | InstallWizard | Select the next guided command on the current step |
| `c` | InstallWizard | Copy the **selected** guided command (now index-aware, not just the first) |

## Notes

- **Scope discipline:** Phase 4 makes `WizardStepKind::Prerequisites` a guided
  (non-executable) step. It does **not** add an `UpdateAction::RunWizardStep`
  executor for prerequisites — the `actions/mod.rs:1103` guard stays unreached.
  `Doctor` remains read-only and keeps its "later phase"/embedded-output behavior.
- **No new config:** `ToolchainSettings` needs no additions for Phase 4 — there is
  nothing to install/manage. `CONFIGURATION.md` is **not** touched.
- **No ARCHITECTURE.md change warranted:** no new module, layer, or data-flow
  change — detection refinement lives in the existing `checks/prerequisites.rs`,
  guided-command derivation in the existing `state.rs`. Only `KEYBINDINGS.md`
  changes (new `[`/`]` keys + index-aware `c`).
- **Missing-key contract (tasks 02 ↔ 03):** to let macOS trim to exactly the
  missing items without a brittle app-side string parse, detection emits a stable,
  documented set of canonical keys into `ComponentCheck.detail`, and the daemon
  exposes `pub fn parse_missing_prereq_keys(detail: &str) -> Vec<&str>` as the
  single source of truth. Task 03 consumes that helper. Both tasks share unit tests
  asserting the round-trip.
- **TEA purity:** all detection I/O stays in the daemon; guided commands are derived
  **purely** in `build_steps()` from the report + `HostPlatform` (no async message);
  per-command navigation is pure state mutation (no `UpdateAction`).
- **Existing tests that must be updated:** the `actions.rs` "Available in a later
  phase" assertion for `Prerequisites` and the `step_detail.rs` "later phase"
  render assertion (~`step_detail.rs:725-738`) break once the step carries guided
  commands — tasks 03 and 05 update them.
