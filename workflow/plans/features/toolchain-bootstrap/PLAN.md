# Plan: Toolchain Bootstrap

## TL;DR

When fdemon launches on a machine where Flutter (and its supporting toolchain) is missing or
incomplete, replace the bare "Flutter SDK not found" error with a **guided install wizard** that
diagnoses what is missing and walks the user through installing it. The wizard runs a
`flutter doctor`-style preflight, then presents ordered steps: **(1) OS prerequisites**,
**(2) Android command-line tools + JDK**, **(3) PATH/env configuration**, **(4) a managed Flutter
SDK**, and **(5) an embedded `flutter doctor` verification view**.

The wizard is **hybrid**: it auto-runs steps that are safe and self-contained (download/extract
the Flutter SDK, download Android cmdline-tools, run `sdkmanager`, write shell rc files with
confirmation), and for steps that require `sudo` or a GUI (`apt install`, `xcode-select --install`)
it shows the exact copy-paste command and re-checks when the user reports done. The managed Flutter
SDK is installed via **git clone by default with an archive-download fallback** when git is
unavailable. Detection and the doctor view are cross-platform; full install automation targets
**Linux, macOS, and Windows**.

This feature supersedes Phase 3 of the older
[`flutter-sdk-management`](../flutter-sdk-management/PLAN.md) plan (Phases 1–2 of which are already
shipped — the SDK locator and the Flutter Version panel).

---

## Background

### Current State

- **SDK detection already works** via `find_flutter_sdk()` (`crates/fdemon-daemon/src/flutter_sdk/locator.rs:45`),
  a 12-strategy chain. On total failure it returns `Err(Error::FlutterNotFound)`
  (`crates/fdemon-core/src/error.rs:37`), classified **fatal**.
- **`Engine::new()`** (`crates/fdemon-app/src/engine.rs:201`) calls the locator synchronously at
  startup; failure is non-fatal there — it logs a warning and sets `resolved_sdk = None`.
- **The only UI reaction to a missing SDK** is a red string. `runner.rs::dispatch_startup_action()`
  (`crates/fdemon-tui/src/runner.rs:~300`) checks `engine.state.flutter_executable()`; when `None`
  it sends `Message::DeviceDiscoveryFailed { error: "Flutter SDK not found…" }`, which the
  new-session dialog's target selector renders as centered red text
  (`crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs:137`). There is no wizard,
  no install action, no diagnostics screen.
- **`ToolAvailability`** (`crates/fdemon-daemon/src/tool_availability.rs:23`) already checks `adb`,
  `xcrun simctl`, the Android `emulator` binary, macOS `log`, and `idevicesyslog`. It carries
  `flutter_sdk: bool` + `flutter_sdk_source: Option<String>` (set by `Engine::new()`), but these
  are **not used to gate any UI**.
- **The Flutter Version panel** (`UiMode::FlutterVersion`, `crates/fdemon-app/src/flutter_version/`)
  is fully built and provides the architectural template for the new wizard: snapshotted SDK state,
  a two-pane overlay, async completion messages (`FlutterVersionSwitchCompleted`,
  `FlutterVersionProbeCompleted`), and the handler/widget decomposition pattern.

### Problem

On a fresh machine a user sees only "Flutter SDK not found" with no path forward. fdemon knows a lot
about the toolchain (it already probes adb/xcrun/emulator) but does nothing actionable with it.

### Key Research Findings (drive the design)

| Finding | Source | Design impact |
|---------|--------|---------------|
| **`flutter doctor` has NO `--machine`/JSON mode** (verified from `doctor_validator.dart`). | Flutter source | The "doctor view" must run **our own structured checks** to drive wizard steps, and optionally capture `flutter doctor -v` **text** (parsing `[✓] [!] [✗] [☠]` prefixes) for display once Flutter exists. |
| `flutter --version --machine` and `flutter devices --machine` **do** emit JSON. | Flutter source | Reuse `flutter --version --machine` (already used by the Flutter Version probe) to confirm a working SDK. |
| Flutter release manifest: `https://storage.googleapis.com/flutter_infra_release/releases/releases_<os>.json` with `base_url`, `current_release.stable` (hash), and `releases[]` entries (`version`, `channel`, `archive`, `sha256`, `dart_sdk_arch`). | Live manifest | Archive-download path resolves the URL + sha256 from the manifest; select by `channel == "stable"` and arch. |
| Flutter SDK install: git clone (`git clone -b stable --depth 1 …`) keeps `flutter upgrade`/`channel` working; archive (`.tar.xz` Linux / `.zip` mac+win) is faster but breaks `flutter upgrade` and may warn in doctor. | Flutter docs / issue #162096 | Default to git clone; fall back to archive when git is absent. |
| Android cmdline-tools zip extracts to `cmdline-tools/`, but `sdkmanager` requires it relocated to `cmdline-tools/latest/`. | developer.android.com | The installer must perform the `latest/` relocation unconditionally. |
| `sdkmanager` packages: `platform-tools`, `platforms;android-36`, `build-tools;36.0.0`, `cmdline-tools;latest`; licenses via `yes \| sdkmanager --licenses` or `flutter doctor --android-licenses`. JDK 17 required. | Android docs | Step 2 installs these + detects/points at a JDK 17. |
| cmdline-tools has **no stable URL** without a build number (`commandlinetools-<os>-<build>_latest.zip`); the number must be scraped from the studio page or hardcoded with a fallback. | developer.android.com | Treat the build number as config with a sane default + override. |
| A process **cannot** modify its parent shell's env; PATH writes only affect new shells. | OS semantics | PATH step writes rc files (`.bashrc`/`.zshenv`/`.zprofile`/fish/`setx`) and instructs the user to restart their terminal; fdemon then re-checks on next launch. |
| macOS `xcode-select --install` opens a GUI dialog. Headless path exists via `softwareupdate` but is fragile. Rosetta via `softwareupdate --install-rosetta --agree-to-license`. | macOS docs | These are **guided** (show command), not auto-run. |
| Rust crates: `reqwest` (streaming), `zip`, `tar`, `xz2`/`lzma-rs`, `sha2`. `fdemon-app` already depends on `reqwest`. | crates.io | Add download/extract deps to `fdemon-daemon`; prefer pure-Rust `lzma-rs` to avoid a liblzma C dependency. |

### Design Decisions (Resolved)

| Decision | Resolution | Rationale |
|----------|-----------|-----------|
| Automation level | **Hybrid** — auto-run safe steps; show copy-paste command + re-check for sudo/GUI steps | Avoids fdemon running `sudo`/GUI installers; keeps the user in control of privileged actions |
| Flutter SDK install | **git clone (default) + archive fallback** | git keeps `flutter upgrade` working; archive needs no git on truly bare machines |
| Platform scope | **All three** (Linux, macOS, Windows) | Requested; detection is cross-platform anyway |
| Doctor data source | **Own structured checks** drive steps; `flutter doctor -v` **text** is embedded for the verification view | `flutter doctor` has no machine output |
| Wizard architecture | New `UiMode::InstallWizard`, modeled on `UiMode::FlutterVersion` | Proven pattern already in the codebase |
| Module placement | Installer/doctor in `fdemon-daemon` (external-tool I/O, like `flutter_sdk/` and `native_logs/`) | Consistent with existing layering |
| SDK / Android install location | Configurable, defaulting to `~/fvm/versions/<ver>/` (Flutter, shared with the existing Flutter Version panel) and `~/.android/sdk` (Android) | Reuses Phase-2 FVM-cache infra; portable headless defaults |

---

## Affected Modules

### New — Toolchain detection & installer (`fdemon-daemon`)

- `crates/fdemon-daemon/src/toolchain/mod.rs` — **NEW** public API: `run_preflight()`, re-exports
- `crates/fdemon-daemon/src/toolchain/types.rs` — **NEW** `ToolchainReport`, `ComponentCheck`,
  `ComponentStatus` (Ok/Partial/Missing/Error/Unknown), `HostPlatform`, `HostShell`, `InstallTarget`
- `crates/fdemon-daemon/src/toolchain/doctor.rs` — **NEW** structured component checks + `flutter doctor -v`
  text capture and `[✓]/[!]/[✗]/[☠]` parser
- `crates/fdemon-daemon/src/toolchain/prerequisites.rs` — **NEW** per-OS prerequisite detection +
  command generation (Linux apt/dnf packages; macOS Xcode CLT/Rosetta/CocoaPods; Windows git)
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs` — **NEW** managed Flutter install:
  releases-manifest fetch, git-clone runner, archive download+verify+extract, `flutter precache`
- `crates/fdemon-daemon/src/toolchain/android.rs` — **NEW** cmdline-tools download + `latest/`
  relocation, `sdkmanager` package install, license acceptance, JDK 17 detection
- `crates/fdemon-daemon/src/toolchain/path_config.rs` — **NEW** shell detection + rc-file/`setx`
  PATH & `ANDROID_HOME` writers (idempotent, marker-fenced)
- `crates/fdemon-daemon/src/toolchain/download.rs` — **NEW** streaming download (progress callback),
  SHA-256 verify, zip / tar.xz extraction helpers
- `crates/fdemon-daemon/src/toolchain/process_stream.rs` — **NEW** run a child process, stream
  stdout/stderr lines back through a callback (for `sdkmanager`/git/`flutter` output)

### New — Wizard state & handlers (`fdemon-app`)

- `crates/fdemon-app/src/install_wizard/mod.rs` — **NEW** re-exports
- `crates/fdemon-app/src/install_wizard/state.rs` — **NEW** `InstallWizardState`, `WizardStep`,
  `WizardStepKind`, `StepStatus`, `WizardPane`
- `crates/fdemon-app/src/install_wizard/types.rs` — **NEW** panel/view types
- `crates/fdemon-app/src/handler/install_wizard/mod.rs` — **NEW** open/close, message routing
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` — **NEW** step/pane navigation, scroll
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — **NEW** trigger step execution; ingest
  progress/log/completion messages

### Modified (`fdemon-app`)

- `crates/fdemon-app/src/state.rs` — `UiMode::InstallWizard`, `install_wizard_state`, helpers
  `show_install_wizard()` / `hide_install_wizard()`
- `crates/fdemon-app/src/message.rs` — wizard message variants (preflight, step lifecycle, progress,
  log, completion)
- `crates/fdemon-app/src/handler/mod.rs` — `UpdateAction` variants (`RunToolchainPreflight`,
  `RunWizardStep`)
- `crates/fdemon-app/src/handler/keys.rs` — key routing for `UiMode::InstallWizard` + an entry-point
  key from Normal/launch dialog
- `crates/fdemon-app/src/handler/update.rs` — wire the new message variants
- `crates/fdemon-app/src/config/types.rs` — `[toolchain]` settings (install dirs, channel,
  android_api_level, cmdline_tools_build, jdk_path)

### Modified (`fdemon-daemon`)

- `crates/fdemon-daemon/src/lib.rs` — re-export `toolchain`
- `crates/fdemon-daemon/src/tool_availability.rs` — surface JDK 17 + cmdline-tools/`sdkmanager`
  presence (the wizard needs finer granularity than the current adb/emulator checks)
- `crates/fdemon-daemon/Cargo.toml` — add `reqwest`, `zip`, `tar`, `lzma-rs` (or `xz2`), `sha2`

### Modified (`fdemon-tui`)

- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` — **NEW** layout dispatch (step list + detail)
- `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` — **NEW** left pane: ordered steps + status icons
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — **NEW** right pane: per-step detail,
  copy-paste commands, action hints
- `crates/fdemon-tui/src/widgets/install_wizard/progress.rs` — **NEW** download/install progress bar + streamed log tail
- `crates/fdemon-tui/src/widgets/install_wizard/doctor_view.rs` — **NEW** embedded `flutter doctor` output
- `crates/fdemon-tui/src/render/mod.rs` — `UiMode::InstallWizard` render branch
- `crates/fdemon-tui/src/runner.rs` — startup hook: on missing/incomplete toolchain, run preflight
  and open the wizard instead of (or alongside) the bare error
- `crates/fdemon-tui/src/widgets/mod.rs` — re-export the wizard widget
- **Modal precedence**: add `InstallWizard` to the modal list in `docs/CODE_STANDARDS.md`’s mouse
  suppression policy and the render-time `MouseCtx` gating.

### Docs (route to `doc_maintainer`)

- `docs/ARCHITECTURE.md` — new `toolchain/` subsystem; new `UiMode::InstallWizard`
- `docs/CODE_STANDARDS.md` — add `InstallWizard` to the modal-precedence list
- `docs/CONFIGURATION.md` / `docs/KEYBINDINGS.md` — `[toolchain]` config + wizard keys (implementor-editable)

---

## Development Phases

Phases are ordered to ship value incrementally. **Phase 1 alone replaces the dead-end error with a
real diagnostics screen.** Each later phase adds one installable step.

> **Step ordering note.** The user-facing step numbering (prerequisites → Android tools → PATH →
> Flutter SDK → doctor) is preserved in the UI, but the **install dependency order** is:
> prerequisites (need `git`/`unzip`/`curl`) → Flutter SDK → Android tools (need a JDK) →
> PATH/env → doctor verification. The wizard sorts steps by dependency and skips ones already
> satisfied by preflight.

### Phase 1 — Toolchain Preflight & Doctor View (read-only)

**Goal:** Diagnose the toolchain and show it. No installation yet.

1. `toolchain/types.rs` + `toolchain/doctor.rs`: implement `run_preflight(project_path) ->
   ToolchainReport`. Structured checks: Flutter SDK (reuse `find_flutter_sdk`), `git`, JDK (`java -version`),
   Android cmdline-tools/`sdkmanager`, `adb`, platform/build-tools, Android licenses, and per-OS
   prerequisites. When Flutter exists, also capture `flutter doctor -v` text and parse status prefixes.
2. `UiMode::InstallWizard` + `InstallWizardState` (read-only first): steps populated from the report,
   each with a `StepStatus` derived from the checks.
3. TUI: step-list + step-detail panes + the embedded doctor view. Show, per component, Ok/Partial/Missing.
4. Startup hook (`runner.rs`): when `flutter_executable().is_none()` (or preflight finds a broken
   toolchain), open `UiMode::InstallWizard` instead of only emitting `DeviceDiscoveryFailed`. Add an
   entry-point key from Normal mode and a prompt in the launch dialog ("Press I to set up Flutter").
5. Async wiring: `UpdateAction::RunToolchainPreflight` → spawn task → `Message::ToolchainPreflightCompleted { report }`.

**Milestone:** A fresh machine shows a structured "what's installed / what's missing" screen with the
real `flutter doctor` output when available, instead of a one-line error.

### Phase 2 — Managed Flutter SDK install + PATH configuration

**Goal:** Install Flutter itself and put it on PATH. (Highest leverage — gets `flutter` working.)

1. `toolchain/download.rs`: streaming download with progress callback, SHA-256 verify, zip + tar.xz extract.
2. `toolchain/flutter_install.rs`: fetch the releases manifest, resolve stable archive+sha for the host
   arch; **git clone (default)** into `~/fvm/versions/<ver>/` with **archive fallback** when git is
   absent; run `flutter precache`. Stream progress.
3. `toolchain/path_config.rs`: detect shell; write an idempotent, marker-fenced PATH export for
   `<flutter>/bin` to the right rc file (`.bashrc`/`.zshenv`/`.zprofile`/fish `fish_add_path`/Windows
   `setx`+registry). Confirm before writing. Print a "restart your terminal" instruction.
4. On completion, also write `[flutter] sdk_path` to `.fdemon/config.toml` so fdemon itself resolves
   the new SDK immediately (no restart needed for fdemon, only for the user's shell).
5. Async wiring: `UpdateAction::RunWizardStep { kind: FlutterSdk | PathConfig }` →
   `Message::WizardDownloadProgress { kind, received, total }`, `WizardStepLog { kind, line }`,
   `WizardStepCompleted { kind, result }`. Re-run preflight after completion.
6. Reuse/extend the Flutter Version panel: a freshly installed version appears in the FVM-cache list.

**Milestone:** On a machine with no Flutter, the wizard downloads/clones a managed SDK, configures
PATH, and `flutter --version --machine` succeeds — fdemon can launch sessions.

### Phase 3 — Android command-line tools + JDK

**Goal:** Install the Android toolchain headlessly.

1. JDK 17 detection; if missing, **guided** install (show `apt`/`brew`/winget command) — JDK install
   is privileged, so it is not auto-run. `flutter config --jdk-dir=<path>` after.
2. `toolchain/android.rs`: download `commandlinetools-<os>-<build>_latest.zip` (build number from
   config with a default), extract, **relocate to `cmdline-tools/latest/`**, then run `sdkmanager`
   for `platform-tools`, `platforms;android-<api>`, `build-tools;<api>.0.0`, `cmdline-tools;latest`.
3. Accept licenses non-interactively (`yes | sdkmanager --licenses`, then `flutter doctor
   --android-licenses`). Stream output.
4. PATH/env step extends to write `ANDROID_HOME` + `$ANDROID_HOME/{cmdline-tools/latest/bin,platform-tools}`.
5. Re-run preflight; Android toolchain check flips to Ok.

**Milestone:** Android toolchain installs unattended (except the privileged JDK step) and
`flutter doctor` reports the Android toolchain as ready.

### Phase 4 — OS prerequisites (guided + safe-auto)

**Goal:** Handle the platform-specific prerequisites.

- **Linux:** detect package manager; show the canonical Flutter package command
  (`clang cmake ninja-build pkg-config libgtk-3-dev curl git unzip xz-utils zip libglu1-mesa` etc.).
  `sudo` → **guided** (show command, re-check). Detect already-present tools to trim the list.
- **macOS:** Xcode CLT (`xcode-select -p` check → `xcode-select --install` guided), Rosetta on Apple
  Silicon (`softwareupdate --install-rosetta` guided), CocoaPods (`brew install cocoapods` guided).
- **Windows:** detect `git`; guide to Git for Windows. PowerShell present by default.

**Milestone:** The prerequisites step lists exactly what is missing per-OS with copy-paste commands
and re-checks live as the user completes each.

### Phase 5 — Polish, parity & integration

- Re-check loop UX: a "Re-run checks" key; per-step retry on failure with the captured log.
- Cross-platform PATH parity (fish, Windows `setx` 1024-char limit → registry write).
- Disk-space + network preflight before large downloads; resumable/abortable downloads.
- Launch-dialog integration: once the toolchain is healthy, the wizard hands back to device discovery.
- CLI surface (optional): `fdemon doctor`, `fdemon setup`.
- Tests: manifest parsing, doctor-text parsing, `cmdline-tools/latest` relocation, idempotent
  rc-file writes (golden-file), per-OS prerequisite command generation, step dependency ordering.

---

## Edge Cases & Risks

- **`flutter doctor` text parsing is locale/version-fragile.** Mitigation: parsing is for *display
  only*; wizard gating uses our own structured checks. Parse defensively (prefix detection, never panic).
- **cmdline-tools build number drifts.** Mitigation: config key `cmdline_tools_build` with a known
  default + a documented "find the current number" note; fail with a clear message if the URL 404s.
- **Large downloads (Flutter ~1GB+ with precache; SDK clone history large).** Mitigation: `--depth 1`
  clone, streaming with progress, abortable, disk-space preflight, SHA-256 verify for archives.
- **`xz2` pulls a C liblzma dependency.** Mitigation: prefer pure-Rust `lzma-rs` for `.tar.xz`; if
  perf matters, gate `xz2` behind a feature.
- **PATH writes can't affect the running shell, and can be applied twice.** Mitigation: marker-fenced,
  idempotent edits; never duplicate; always print the restart instruction; fdemon writes
  `[flutter] sdk_path` so it works without a restart for itself.
- **Privileged steps (sudo/GUI) cannot be reliably automated across distros/macOS versions.**
  Mitigation: these are guided-only by design (the Hybrid decision).
- **git absent on a truly bare machine** (needed for the default clone path). Mitigation: archive
  fallback; and `git` is itself a Phase-4 prerequisite that the wizard can guide first.
- **`fdemon-daemon` gaining network + archive deps** enlarges its surface and build time. Mitigation:
  keep all of it inside `toolchain/`; consider a cargo feature if build-time becomes a concern.
- **Partial/aborted installs** leaving half-written SDK dirs. Mitigation: download to a temp dir,
  extract, then atomically move into place; clean up temp on failure.
- **Windows `.bat`/`setx` quirks** (1024-char PATH truncation). Mitigation: registry write via
  PowerShell `[Environment]::SetEnvironmentVariable`.

---

## Configuration Additions

### `.fdemon/config.toml`

```toml
[toolchain]
# Where a managed Flutter SDK is installed (default: ~/fvm/versions/<version>)
# flutter_install_dir = "~/fvm/versions"
# Channel for managed installs
channel = "stable"
# Install method preference: "git" (default) or "archive"
flutter_install_method = "git"
# Android SDK root (default: ~/.android/sdk; falls back to ANDROID_HOME if set)
# android_sdk_root = "~/.android/sdk"
# Android API level for platforms/build-tools (default: latest known)
android_api_level = 36
# cmdline-tools build number for the download URL (override if the default 404s)
# cmdline_tools_build = "14742923"
# Explicit JDK 17 directory (passed to `flutter config --jdk-dir`)
# jdk_path = "/usr/lib/jvm/java-17-openjdk"
```

(Existing `[flutter] sdk_path` is written automatically after a managed install.)

---

## Keyboard Shortcuts (proposed)

| Key | Mode | Action |
|-----|------|--------|
| `I` | Normal / launch dialog | Open the Install Wizard |
| `Esc` | InstallWizard | Close wizard |
| `Tab` | InstallWizard | Switch pane (step list ↔ detail) |
| `j`/`k` `↑`/`↓` | InstallWizard | Navigate steps / scroll detail |
| `Enter` | InstallWizard | Run / retry the selected step |
| `r` | InstallWizard | Re-run preflight checks |
| `c` | InstallWizard | Copy the selected guided command to clipboard |
| `Ctrl+C` | InstallWizard | Quit fdemon |

---

## Success Criteria

- [ ] On a machine with no Flutter, fdemon opens a structured diagnostics wizard (not a bare error).
- [ ] Preflight reports Flutter, git, JDK, Android cmdline-tools/sdkmanager, adb, platforms/build-tools,
      licenses, and per-OS prerequisites with Ok/Partial/Missing status.
- [ ] When Flutter is present, `flutter doctor -v` output is embedded and rendered in the wizard.
- [ ] The wizard installs a managed Flutter SDK (git clone default, archive fallback) and configures PATH.
- [ ] After the Flutter step, `[flutter] sdk_path` is written and fdemon resolves the SDK without restart.
- [ ] The wizard installs Android cmdline-tools (with the `cmdline-tools/latest/` relocation),
      sdkmanager packages, and accepts licenses; Android toolchain check turns Ok.
- [ ] Privileged/GUI steps (apt, JDK, xcode-select, Rosetta, CocoaPods, git) are shown as copy-paste
      commands and re-checked live — never auto-run with sudo.
- [ ] PATH/env writes are idempotent, marker-fenced, shell-aware, and accompanied by a restart hint.
- [ ] Works on Linux, macOS, and Windows (detection everywhere; install automation per the per-OS rules).
- [ ] Comprehensive unit tests (manifest parse, doctor-text parse, relocation, idempotent rc writes,
      command generation, step ordering). Existing tests pass; no regressions.

---

## References

- [Install Flutter manually](https://docs.flutter.dev/install/manual)
- [Flutter add to PATH](https://docs.flutter.dev/install/add-to-path)
- Releases manifest: `https://storage.googleapis.com/flutter_infra_release/releases/releases_{linux,macos,windows}.json`
- [Android command-line tools](https://developer.android.com/tools) · [sdkmanager](https://developer.android.com/tools/sdkmanager) · [env variables](https://developer.android.com/tools/variables)
- [Flutter Linux setup](https://docs.flutter.dev/platform-integration/linux/setup) · [Android setup](https://docs.flutter.dev/platform-integration/android/setup) · [iOS/macOS setup](https://docs.flutter.dev/platform-integration/ios/setup)
- [git-clone vs archive install (flutter#162096)](https://github.com/flutter/flutter/issues/162096)
- Superseded: [`flutter-sdk-management/PLAN.md`](../flutter-sdk-management/PLAN.md) (Phases 1–2 shipped; Phase 3 replaced by this plan)
- Reference pattern in-repo: `UiMode::FlutterVersion` (`crates/fdemon-app/src/flutter_version/`, `crates/fdemon-tui/src/widgets/flutter_version_panel/`)
