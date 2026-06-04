# Phase 3 — Android Command-Line Tools + JDK — Task Index

## Overview

Phase 3 makes the **Android Tools** wizard step executable and extends the **PATH
Configuration** step to write Android environment variables. Pressing `Enter` on
the Android Tools step downloads the Android command-line tools, relocates them to
`cmdline-tools/latest/`, runs `sdkmanager` to install `platform-tools`,
`platforms;android-<api>`, and `build-tools;<api>.0.0`, then accepts the SDK
licenses non-interactively — all with the live progress bar + streamed log tail
built in Phase 2. The JDK 17 requirement is **guided** (privileged): when a JDK 17
is missing the wizard shows a per-OS copy-paste install command (copyable with `c`)
and refuses to run the Android install until a JDK is present. After install,
preflight re-runs and the Android component checks flip to `Ok`.

This phase also introduces the wizard's **first guided-command UI** — a small,
reusable model (`GuidedCommand`) rendered in the detail pane and copyable via the
`c` key — which Phase 4 (OS prerequisites) will extend for every guided step.

**Total Tasks:** 10
**Estimated Hours:** 32–43 hours

**Platform scope:** full install automation on **Linux, macOS, and Windows**
(cmdline-tools is a cross-platform zip; `sdkmanager.bat` on Windows; `ANDROID_HOME`
written via rc files on POSIX and via PowerShell `[Environment]::SetEnvironmentVariable`
on Windows). The JDK step is guided everywhere.

## Architecture Recap (what already exists from Phases 1–2)

- **Detection (read-only, reuse as-is):** `toolchain/checks/android.rs`
  (`android_sdk_root()`, `check_android_cmdline_tools`, `check_android_platform_tools`,
  `check_android_platform`, `check_android_build_tools`, `check_android_licenses`,
  `sdkmanager_bin_name()`), `toolchain/checks/mod.rs::check_jdk` +
  `parse_jdk_output` (major-version ≥ 17). `run_preflight()` already runs all of
  these and orders the `AndroidTools` components in `build_steps()`. **Phase 3 does
  not change detection — it re-runs preflight after install to flip checks to Ok.**
- **Streaming + download infra (reuse):** `toolchain/download.rs`
  (`download_to_file`, `verify_sha256`, `extract_zip`), `toolchain/process_stream.rs`
  (`run_streaming` — merged stdout/stderr line stream), `InstallEvent`
  (`Log`/`Download`/`Phase`).
- **Phase 2 executor template to mirror:** `actions/mod.rs` `RunWizardStep` arm —
  `FlutterSdk` (streaming install) and `PathConfig` (`spawn_blocking(add_to_path)`).
  The `AndroidTools` / `Prerequisites` / `Doctor` arm is currently a
  "not executable in this version" stub (`actions/mod.rs:1015`) — Phase 3 replaces
  the `AndroidTools` half of it.
- **Completion chain to mirror:** `handler/install_wizard/actions.rs` —
  `handle_step_completed(FlutterSdk)` stashes the path, writes settings, returns
  `PersistSettings` + `InstallWizardRerunPreflight`, which re-runs preflight and
  `ScanInstalledSdks`.
- **Config (already declared, inert):** `ToolchainSettings` already has
  `android_sdk_root`, `android_api_level` (default 36), `cmdline_tools_build`,
  `jdk_path`. **No config task needed** — Phase 3 starts reading them.
- **Clipboard plumbing (reuse for `c`):** `UpdateAction::WriteClipboard { text }`
  applied via `AppState::pending_runner_actions` (already used by log-copy).
- **PATH writer gap:** `path_config.rs::add_to_path` is **PATH-only** with a
  hardcoded fence marker — Phase 3 adds a generalized env-var writer for
  `ANDROID_HOME`.

## Task Dependency Graph

```
Wave 1 (parallel, no deps)
┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐
│ 01 android install │  │ 04 wizard protocol │  │ 05 guided-command  │
│    types + URLs    │  │    (msg/action/key)│  │    state + steps   │
└─────────┬──────────┘  └─────────┬──────────┘  └─────────┬──────────┘
          │                       │                        │
   (mod.rs chain)                 │            ┌───────────┴───────────┐
          ▼                       │            ▼                       ▼
┌────────────────────┐           │   ┌────────────────────┐  ┌────────────────────┐
│ 02 android_install │           │   │ 07 handlers +      │  │ 08 TUI step_detail │
│    + jdk + license │           │   │    completion+copy │  │    guided render   │
└─────────┬──────────┘           │   └────────────────────┘  └────────────────────┘
          ▼                       │            ▲
┌────────────────────┐           │            │ (04,05)
│ 03 android env     │           │
│    path_config     │           ▼
└─────────┬──────────┘   ┌────────────────────┐
          │              │ 10 CONFIG +        │
          └──────┬───────┤    KEYBINDINGS doc │ (after 04)
                 ▼       └────────────────────┘
       ┌────────────────────┐
       │ 06 RunWizardStep   │ (after 01,02,03,04)
       │    executor        │
       └─────────┬──────────┘
                 ▼
       ┌────────────────────┐
       │ 09 ARCHITECTURE.md │ (doc_maintainer; after 02,03,06,07,08)
       └────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-android-install-types](tasks/01-android-install-types.md) | Not Started | - | 3-4h | `toolchain/types.rs`, `toolchain/mod.rs` |
| 02 | [02-android-installer-and-jdk](tasks/02-android-installer-and-jdk.md) | Not Started | 01 | 7-9h | `toolchain/android_install.rs`, `toolchain/jdk.rs`, `toolchain/process_stream.rs`, `toolchain/mod.rs` |
| 03 | [03-android-env-path-config](tasks/03-android-env-path-config.md) | Not Started | 01 | 4-5h | `toolchain/path_config.rs`, `toolchain/mod.rs` |
| 04 | [04-wizard-protocol-additions](tasks/04-wizard-protocol-additions.md) | Not Started | - | 3-4h | `message.rs`, `handler/mod.rs`, `handler/keys.rs` |
| 05 | [05-guided-command-state](tasks/05-guided-command-state.md) | Not Started | - | 3-4h | `install_wizard/types.rs`, `install_wizard/state.rs`, `install_wizard/mod.rs` |
| 06 | [06-run-android-step-executor](tasks/06-run-android-step-executor.md) | Not Started | 01, 02, 03, 04 | 4-5h | `actions/mod.rs` |
| 07 | [07-wizard-handlers-android-and-copy](tasks/07-wizard-handlers-android-and-copy.md) | Not Started | 04, 05 | 4-5h | `handler/install_wizard/actions.rs`, `handler/update.rs` |
| 08 | [08-tui-guided-step-detail](tasks/08-tui-guided-step-detail.md) | Not Started | 05 | 3-4h | `widgets/install_wizard/step_detail.rs`, `widgets/install_wizard/mod.rs` |
| 09 | [09-update-architecture-doc](tasks/09-update-architecture-doc.md) | Not Started | 02, 03, 06, 07, 08 | 1-2h | `docs/ARCHITECTURE.md` |
| 10 | [10-update-config-keybindings-docs](tasks/10-update-config-keybindings-docs.md) | Not Started | 04 | 1h | `docs/CONFIGURATION.md`, `docs/KEYBINDINGS.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/types.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs` | `toolchain/checks/android.rs` (component kinds), `toolchain/flutter_install.rs` (types pattern) |
| 02 | `crates/fdemon-daemon/src/toolchain/android_install.rs`, `crates/fdemon-daemon/src/toolchain/jdk.rs`, `crates/fdemon-daemon/src/toolchain/process_stream.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs` | `toolchain/download.rs`, `toolchain/checks/android.rs`, `toolchain/types.rs`, `flutter_install.rs` (install template) |
| 03 | `crates/fdemon-daemon/src/toolchain/path_config.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs` | `toolchain/types.rs` (`HostShell`, `HostPlatform`) |
| 04 | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/handler/keys.rs` | `install_wizard/types.rs` (`WizardStepKind`), `config/types.rs` (`ToolchainSettings`) |
| 05 | `crates/fdemon-app/src/install_wizard/types.rs`, `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/install_wizard/mod.rs` | `toolchain/types.rs` (`HostPlatform`, `ComponentStatus`), `config/types.rs` |
| 06 | `crates/fdemon-app/src/actions/mod.rs` | `toolchain/android_install.rs`, `toolchain/jdk.rs`, `toolchain/path_config.rs`, `handler/mod.rs`, `message.rs` |
| 07 | `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/handler/update.rs` | `message.rs`, `install_wizard/state.rs`, `config/settings.rs`, `handler/mod.rs` |
| 08 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`, `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | `fdemon-app::install_wizard` (`GuidedCommand`, exec state re-exports) |
| 09 | `docs/ARCHITECTURE.md` | task files 02, 03, 06, 07, 08 |
| 10 | `docs/CONFIGURATION.md`, `docs/KEYBINDINGS.md` | task files 04 |

### Overlap Matrix

Wave-peer comparisons (tasks with no dependency edge that may run concurrently):

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 04 + 05 | None | Parallel (worktree) |
| 02 + 03 | `toolchain/mod.rs` | **Sequential (same branch)** — enforced by the 01→02→03 mod.rs chain |
| 01 + 02 + 03 | `toolchain/mod.rs` | **Sequential (same branch)** — linear dep chain |
| 06 + 07 + 08 + 10 | None (distinct files) | Parallel (worktree) |
| 04 + 05 | None | Parallel (worktree) |
| 07 + 10 | None | Parallel (worktree) |

**Key isolation note:** `crates/fdemon-daemon/src/toolchain/mod.rs` is written by
tasks 01, 02, and 03 (each adds `mod x;` + `pub use`). These form a linear
dependency chain (01→02→03), so they run sequentially on the same branch — no
merge conflict on `mod.rs`. Mirrors the Phase 2 `02→03→04` mod.rs chain. No other
write-file overlaps exist between concurrent tasks.

## Suggested Wave Schedule

- **Wave 1 (parallel):** 01, 04, 05
- **Wave 2 (parallel, as deps clear):** 02 (after 01), 07 (after 04/05), 08 (after 05), 10 (after 04)
- **Wave 3:** 03 (after 02 — mod.rs chain)
- **Wave 4:** 06 (after 01/02/03/04)
- **Wave 5:** 09 (after 02/03/06/07/08)

## Success Criteria

Phase 3 is complete when:

- [ ] Pressing `Enter` on the **Android Tools** step (with a JDK 17 present)
      downloads the Android command-line tools, relocates them to
      `cmdline-tools/latest/`, installs `platform-tools`, `platforms;android-<api>`,
      and `build-tools;<api>.0.0` via `sdkmanager`, and accepts the SDK licenses —
      all with the live progress bar + streamed log tail.
- [ ] When a JDK 17 is **missing**, the Android Tools step is gated: the detail
      pane shows a per-OS guided install command (apt/dnf/brew/winget), pressing
      `Enter` does **not** auto-run a privileged install, and `c` copies the
      command to the clipboard.
- [ ] The **PATH Configuration** step writes `ANDROID_HOME` plus
      `$ANDROID_HOME/cmdline-tools/latest/bin` and `$ANDROID_HOME/platform-tools`
      to the correct shell rc file (POSIX) or via PowerShell registry write
      (Windows), idempotently and marker-fenced, with a "restart your terminal"
      hint.
- [ ] After the Android Tools step, the discovered SDK root is persisted to
      `[toolchain] android_sdk_root`, preflight re-runs, and the Android component
      checks (cmdline-tools, platform-tools, platform, build-tools, licenses) flip
      to `Ok` without restarting fdemon.
- [ ] Privileged JDK install is never auto-run with `sudo`/`winget`/`brew` — it is
      shown as a copy-paste command and re-checked with `r`.
- [ ] All new code has unit tests (cmdline-tools URL generation per OS, sdkmanager
      package-name generation, `cmdline-tools/latest` relocation, JDK guided-command
      generation per OS, idempotent `ANDROID_HOME` rc-file writes golden-file,
      guided-command state derivation, copy-command dispatch). Existing tests pass;
      no regressions.
- [ ] `cargo fmt --all`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass.

## Keyboard Shortcuts (Phase 3 additions)

| Key | Mode | Action |
|-----|------|--------|
| `Enter` | InstallWizard | Run / retry the selected step — now also **Android Tools** (gated on JDK 17) |
| `c` | InstallWizard | Copy the selected step's guided command (e.g. the JDK install command) to the clipboard |

## Notes

- **Scope discipline:** Phase 3 makes `WizardStepKind::AndroidTools` executable and
  extends `WizardStepKind::PathConfig` with Android env writes. `Prerequisites`
  remains read-only (Phase 4). `Doctor` remains read-only.
- **JDK is guided, not auto:** `sdkmanager` requires a JDK. The Android Tools step
  is **gated** in `handle_run_selected_step` (task 07): if `check_jdk` is not `Ok`,
  the handler refuses to dispatch the install and surfaces the guided command
  instead. This honors the PLAN "Hybrid" decision (no privileged auto-run).
- **cmdline-tools has no published SHA / stable URL:** the download URL embeds a
  build number (`commandlinetools-<os>-<build>_latest.zip`). Phase 3 ships a known
  default build number (config key `cmdline_tools_build` overrides it) and relies
  on HTTPS/TLS integrity — there is no manifest sha256 to verify against (unlike the
  Flutter archive). Fail with a clear message if the URL 404s. See task 01/02.
- **`cmdline-tools/latest` relocation is mandatory:** the zip extracts to
  `cmdline-tools/`, but `sdkmanager` only works from `cmdline-tools/latest/`. The
  installer performs the relocation unconditionally (task 02).
- **License acceptance needs stdin:** `sdkmanager --licenses` reads interactive
  y/n prompts. `run_streaming` does not feed stdin, so task 02 adds a small
  `run_streaming_with_input` variant (pipe a stream of `y\n`) and/or runs
  `flutter doctor --android-licenses` when Flutter is present.
- **Atomic + cleanup:** download to a temp dir under the SDK root, extract, then
  move into `cmdline-tools/latest/`; clean up temp on failure (mirror Phase 2's
  atomic install pattern).
- **TEA purity:** all I/O happens in `actions/mod.rs` spawned tasks; handlers stay
  pure and communicate via `Message` variants. The guided command is **derived
  purely** from the preflight report + `HostPlatform` in `build_steps()` (no async
  message needed to display it).
- **Windows specifics:** `sdkmanager.bat`, `ANDROID_HOME` via PowerShell
  `[Environment]::SetEnvironmentVariable(..., 'User')` (avoids `setx` 1024-char
  truncation), default SDK root `%LOCALAPPDATA%\Android\Sdk`. Reuse the Phase 2
  Windows PATH-write pattern (`FDEMON_NEW_PATH` out-of-band env var to avoid
  injection).
