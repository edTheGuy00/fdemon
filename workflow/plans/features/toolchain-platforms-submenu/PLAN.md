# Plan: Install-Wizard Step Reorder + Platforms Submenu

## TL;DR

Two improvements to the already-shipped [`toolchain-bootstrap`](../toolchain-bootstrap/PLAN.md)
install wizard:

1. **Reorder the wizard steps** so **PATH Configuration is the last step before Flutter Doctor**.
   Today the order is `Prerequisites → Android Tools → PATH → Flutter SDK → Doctor`, which forces
   the user through PATH Config (where they hit a dead-end `"Install Flutter first"` message) *before*
   they ever install Flutter. New order:
   **`Prerequisites → Platforms → Flutter SDK → PATH Configuration → Flutter Doctor`**.

2. **Replace the single Android-only "Android Tools" step with an expandable "Platforms" submenu**
   containing **Android, iOS, macOS, Web, Windows**. The existing Android tooling moves under the
   **Android** leaf (unchanged behaviour). The other leaves add **detection + guided-command**
   support per platform (none are auto-installable the way Android is). **Web** supports a
   **configurable custom browser** (not just Chrome) via `CHROME_EXECUTABLE`. iOS/macOS leaves are
   **macOS-host-only**; the Windows leaf is **Windows-host-only**; Android and Web render on all hosts.

This closes the gap where `flutter doctor` reports errors for Web (no Chrome), Windows (no Visual
Studio), and iOS/macOS (no Xcode) but the wizard only ever offered Android.

3. **Add a Flutter SDK version picker.** Today the Flutter SDK step installs **latest stable** only.
   Add a navigable picker that fetches the Flutter releases manifest and lets the user choose **any
   version or channel** to install — stable releases (newest-first), beta, and `master`/`main`
   (git-only). Modelled on **fvm**: the releases manifest drives the list; installs use **git clone**
   (`-b <channel>` for channels, clone + `git reset --hard <tag>` for pinned versions), with the
   existing archive path as a fallback. Pinned versions install into `~/fvm/versions/<version>` so
   multiple versions coexist (consistent with the existing Flutter Version panel + FVM cache).

---

## Background

### Current State (verified against live code)

- **Steps are a flat `Vec<WizardStep>`** built once per preflight by `build_steps()`
  (`crates/fdemon-app/src/install_wizard/state.rs:864`). The order is a single `vec![]` literal
  (`state.rs:936`): `Prerequisites[0] → AndroidTools[1] → PathConfig[2] → FlutterSdk[3] → Doctor[4]`.
- **`WizardStepKind`** (`crates/fdemon-app/src/install_wizard/types.rs:45`) is a flat 5-variant `Copy`
  enum. **No submenu / tree / nesting concept exists.** Navigation is a single `selected_index: usize`
  walked by `handle_up`/`handle_down` (`handler/install_wizard/navigation.rs`), clamped to
  `steps.len()` — fully data-driven, so it adapts to a changed step count automatically.
- **The dead-end message** is at `handler/install_wizard/actions.rs:451`: the `PathConfig` arm of
  `handle_run_selected_step` returns `status_message = "Install Flutter first"` when no Flutter
  `bin/` dir is resolvable (`installed_sdk_path` / `settings.flutter.sdk_path` / `resolved_sdk` all
  `None`). A soft tip at `actions.rs:420` says `"Tip: run Android Tools first…"`.
- **Only Android is detectable/installable.** `ComponentKind` (daemon
  `crates/fdemon-daemon/src/toolchain/types.rs:83`) has 9 variants: `FlutterSdk, Git, Jdk,
  AndroidCmdlineTools, AndroidPlatformTools, AndroidPlatform, AndroidBuildTools, AndroidLicenses,
  Prerequisites`. `run_preflight` asserts `report.components.len() == 9` (`mod.rs:253`). There are
  **no** components or checks for iOS (Xcode/CocoaPods), macOS (Xcode), Web (browser), or Windows (VS).
- **Android is the only automatable platform** — `android_install.rs` does a managed
  cmdline-tools download + `sdkmanager` install. Everything else (Xcode, Visual Studio, browsers)
  is privileged/GUI/App-Store and must be **guided-only** (show a copy-paste command + re-check).
- **The completion chain already decouples PATH from visual order**: on `FlutterSdk` success,
  `handle_step_completed` stashes `installed_sdk_path` and fires
  `InstallWizardAutoConfigurePath{FlutterSdk}` → `begin_step(PathConfig)` → `RunWizardStep{PathConfig}`,
  regardless of where PathConfig sits in the list (`actions.rs:587`). So the reorder is **purely a
  display change** plus test/index fixups.
- **Config:** `ToolchainSettings` (`config/types.rs:173`) holds the toolchain knobs. The separate
  DevTools `browser` field (`types.rs:490`) is unrelated (it opens the DevTools UI) — a new
  `web_browser_executable` field will not collide.

### Problem

The wizard's step order creates an awkward navigation loop (PATH → "install Flutter first" → go back
down → Flutter → go back up → PATH), and the wizard only knows how to set up Android even though
`flutter doctor` flags Web/Windows/iOS/macOS as well.

### Key Research Findings (drive the design)

| Finding | Source | Design impact |
|---------|--------|---------------|
| `flutter doctor` groups output by platform section: *Flutter*, *Android toolchain*, *Xcode — develop for iOS and macOS*, *Chrome — develop for the web*, *Visual Studio — develop Windows apps*, *Linux toolchain*, *Connected devices*. | flutter CLI | Mirror these as Platforms leaves; iOS+macOS share one Xcode section. |
| The web browser is selected via the **`CHROME_EXECUTABLE`** env var; any Chromium-based browser (Edge, Brave, Chromium) works. `flutter doctor` reports *"Chrome — develop for the web"* / *"Cannot find Chrome"*. | docs.flutter.dev | Web leaf detects `CHROME_EXECUTABLE` → default Chrome paths → Edge (Windows); guided command sets `CHROME_EXECUTABLE`; new `web_browser_executable` config field feeds it. |
| Windows desktop needs **Visual Studio with the "Desktop development with C++" workload** (MSVC, C++ CMake tools, Windows SDK). Detected via `vswhere.exe -requires VC.Tools.x86.x64 …`. Installable non-interactively via `winget`/`choco`. | docs.flutter.dev | Windows leaf: `vswhere` probe + winget/choco guided command. Windows-host-only. |
| iOS/macOS need **full Xcode** (not just CLT) + **CocoaPods**; Xcode is an App Store / `xcodes` install (GUI). Post-install: `xcode-select -s`, `xcodebuild -runFirstLaunch`, `xcodebuild -license accept`. | docs.flutter.dev | iOS+macOS share one Xcode/CocoaPods probe; all guided-only. macOS-host-only. |
| Android tooling is already fully automatable and shipped. | in-repo | Android leaf reuses the existing install pipeline verbatim. |
| **fvm** lists installable versions from the releases manifest (single GET, full list, newest-first, channels `stable`/`beta`/deprecated `dev`); `master`/`main` are **git-only** (no archive/sha256). fvm installs *everything* via **git clone** (`-b <channel>`; clone + `git reset --hard <tag>` for pins). macOS has dual-arch entries (filter by `dart_sdk_arch`); Linux/Windows x64-only. | fvm source + live manifest | Mirror fvm: manifest drives the picker list, git-clone is the primary install path, archive is fallback. Filter by host arch; flag `master`/`main` as git-only. |
| **fdemon already** fetches the *full* manifest (`fetch_release_manifest`, `flutter_install.rs:414`), resolves any channel (`resolve_channel_release`), and threads `FlutterStepParams.channel` end-to-end. The Flutter Version panel lists *installed* versions but has **no "available" picker**. `version_dir_name` is always set to the channel name. | in-repo | The version-picker phase is mostly **UI + version-tag threading**, not new download/install plumbing. |

### Design Decisions (Resolved with user)

| Decision | Resolution | Rationale |
|----------|-----------|-----------|
| Step order | `Prerequisites → Platforms → Flutter SDK → PATH → Doctor` | PATH last (the ask); platform/OS prep grouped before Flutter. |
| Submenu model | **Flat `Vec` + expandable parent row** (`WizardStepKind::Platforms` parent + per-platform leaf kinds + `platforms_expanded` flag + per-step `indent`), **not** a nested `sub_steps` tree or a third `WizardPane`. | Lowest blast radius: `step_list` renders one row per vec entry; all lifecycle code keys off `WizardStepKind`; navigation is already `steps.len()`-driven. A tree/extra-pane would need a second cursor, 3-way Tab, and a parallel render loop. |
| Submenu UX | Single **Platforms** row; `Enter` expands/collapses to reveal indented per-platform child rows. | Compact list; user choice. |
| Scope | **All five platforms, phased** (5 incremental phases). | User choice; each phase ships independently. |
| Automation level | Android = managed auto-install (existing); iOS/macOS/Web/Windows = **detect + guided-only**. | Xcode/VS/browsers are privileged/GUI/App-Store; cannot be auto-run safely. |
| Host gating | iOS & macOS leaves only on macOS hosts; Windows leaf only on Windows hosts; Android + Web on all hosts. | `flutter doctor` gates the same way; avoids dead leaves. (Linux desktop is out of scope — its prerequisites are already covered by the `Prerequisites` step.) |
| Web optionality | A missing web browser rolls up as **Partial/Warning**, never **Missing** — it must not block the "toolchain healthy" handback. | Web is optional for most users; blocking would regress the bootstrap flow. |

---

## Affected Modules

### Modified — Wizard state & types (`fdemon-app`)

- `install_wizard/types.rs` — extend `WizardStepKind`: add `Platforms` (parent) + `PlatformAndroid`,
  `PlatformIos`, `PlatformMacos`, `PlatformWeb`, `PlatformWindows` (leaves); **remove** `AndroidTools`
  (mapped to `PlatformAndroid`).
- `install_wizard/state.rs` — add `platforms_expanded: bool` to `InstallWizardState`; add `indent: u8`
  (or `parent: Option<WizardStepKind>`) to `WizardStep`; rewrite `build_steps()` for the new order +
  parent/leaf projection (collapsed = parent only; expanded = parent + host-applicable leaves);
  per-platform guided-command builders; update doc-comments.
- `handler/install_wizard/navigation.rs` — Enter-on-parent toggles `platforms_expanded`; Esc collapses
  an expanded group before closing; `selected_command_index` reset unchanged.
- `handler/install_wizard/actions.rs` — rename `AndroidTools` → `PlatformAndroid` in
  `handle_run_selected_step` / `handle_step_completed` / `handle_auto_configure_path`; add guided-only
  arms for iOS/macOS/Web/Windows (must **not** hit the `WizardStepFailed` catch-all); add a no-op
  Platforms-parent arm; reword the `actions.rs:420` Android-first tip; **keep** the
  `actions.rs:451` "Install Flutter first" gate (still reachable via manual nav).
- `handler/keys.rs` — Enter routing already covers run/toggle via `RunSelectedStep` +
  a new toggle message; add expand/collapse keys if desired (`l`/`h` or `→`/`←`).
- `message.rs` — add `InstallWizardToggleExpand` (no payload); extend any `kind`-carrying variants for
  the new leaf kinds.
- `handler/mod.rs` — `UpdateAction::RunWizardStep.kind` accepts the new leaf kinds; add optional
  `web: Option<WebStepParams>` only if Web ever writes env (otherwise guided-only needs no params).
- `config/types.rs` — add `ToolchainSettings.web_browser_executable: Option<String>`
  (`#[serde(default)]`); optional `platforms_enabled: Option<Vec<String>>` (defer).

### Modified — Flutter version picker (`fdemon-app` + `fdemon-tui`)

- `install_wizard/state.rs` — `InstallWizardState` gains a `version_picker: VersionPickerState`
  (fetched/grouped manifest releases, channel-tab focus, `selected_index`, `loading`/`error`,
  chosen `selected_release: Option<FlutterRelease>`), `Cell` render-hint for height.
- `handler/install_wizard/` — **NEW** `version_picker.rs`: open/close, manifest-fetch lifecycle,
  navigation (j/k, channel tab), confirm-selection. The `FlutterSdk` step Enter (or `v`) opens the
  picker; confirming sets the chosen version and runs the install.
- `handler/install_wizard/actions.rs` — the `FlutterSdk` arm sources the version/channel from the
  picker selection (falling back to `settings.toolchain.channel`); thread the chosen `version_tag`.
- `handler/mod.rs` — `UpdateAction::FetchFlutterReleaseManifest`; extend `FlutterStepParams` with
  `version_tag: Option<String>` (the exact manifest version, e.g. `"3.24.0"`).
- `actions/mod.rs` — the `RunWizardStep` FlutterSdk executor sets `FlutterInstallTarget.version_dir_name`
  to the `version_tag` when pinned (so it lands in `~/fvm/versions/<version>`), passes the version as
  the git `-b` ref / archive match.
- `message.rs` — `FlutterManifestFetched`/`FlutterManifestFetchFailed`, picker nav/confirm/cancel.
- `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` — **NEW** overlay/picker widget:
  channel tabs (Stable / Beta / Master·git-only), navigable list, active-stable default cursor,
  per-entry version + date + arch, "git-only" badge for master/main. Reuses the `VersionListState`
  scroll pattern from `flutter_version_panel`.
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs` — add a `resolve_version_release` helper
  (match a manifest entry by exact version + arch) alongside the existing `resolve_channel_release`.

### Modified — Toolchain detection (`fdemon-daemon`)

- `toolchain/types.rs` — add `ComponentKind` variants: `XcodeTools`, `CocoaPods` (iOS/macOS),
  `WebBrowser`, `VisualStudioCpp`; extend the `Display` impl.
- `toolchain/mod.rs` — register the new (host-gated) checks in `run_preflight`; update the
  `components.len() == 9` assertion (now variable by host).
- `toolchain/checks/` — **NEW** `ios.rs` (Xcode/CocoaPods, macOS-gated), `web.rs` (browser, cross-host
  via `CHROME_EXECUTABLE` + default paths), `windows.rs` (`vswhere`, Windows-gated). `android.rs`
  unchanged.

### Modified — Wizard TUI (`fdemon-tui`)

- `widgets/install_wizard/step_list.rs` — render `indent` prefix on leaf rows; expand/collapse caret
  (`▸`/`▾`) on the Platforms parent; extend run-failed badge to leaf kinds.
- `widgets/install_wizard/step_detail.rs` — add Platforms-parent + 5 leaf arms to `step_caption()`,
  `is_executable()`, `action_hint_text()`, `render_action_hint()`; remove `AndroidTools`.
- `widgets/install_wizard/mod.rs` — make `VERTICAL_STEP_LIST_HEIGHT` dynamic from the visible step
  count (collapsed vs expanded); update the footer hint string to mention `[Enter] expand`.

### Docs (route to `doc_maintainer` for core docs; implementor for the rest)

- `docs/ARCHITECTURE.md` — Platforms submenu in the `install_wizard` description; new daemon checks.
- `docs/CONFIGURATION.md` / `docs/KEYBINDINGS.md` — `web_browser_executable`; expand/collapse keys.
- `website/src/pages/docs/toolchain.rs` — rewrite "five ordered steps" prose, numbered table, and
  ASCII art for the new order + Platforms submenu.

---

## Development Phases

> Each phase keeps the daemon component-count assertion and the affected test fixtures green before
> merge. Phases 3–5 are independent and can land in any order after Phase 2.

### Phase 1 — Reorder PATH after Flutter SDK (no new types)

**Goal:** Fix the awkward navigation immediately.
1. Reorder the `vec![]` in `build_steps()` to `Prerequisites → AndroidTools → FlutterSdk → PathConfig
   → Doctor` (interim — `AndroidTools` becomes `Platforms` in Phase 2). Update the `build_steps()`
   and `installed_sdk_path` doc-comments.
2. Reword the `actions.rs:420` "run Android Tools first" tip; keep the `actions.rs:451` gate.
3. Renumber all index-based tests (PathConfig 2→3, FlutterSdk 3→2) across `state.rs`, `actions.rs`,
   `navigation.rs`, `step_list.rs`, `step_detail.rs`.
4. Update `website/src/pages/docs/toolchain.rs` step order.

**Milestone:** Wizard order is `Prerequisites → … → Flutter SDK → PATH → Doctor`; no dead-end nav.

### Phase 2 — Platforms parent + expand/collapse + Android leaf

**Goal:** Working one-platform submenu.
1. `WizardStepKind`: add `Platforms` + `PlatformAndroid`; map old `AndroidTools` behaviour to
   `PlatformAndroid`. Add `indent` to `WizardStep` and `platforms_expanded` to state.
2. `build_steps()`: emit the Platforms parent (rolled-up status from the Android component bucket);
   when expanded, emit the `PlatformAndroid` leaf (and placeholder rows for other host-applicable
   platforms, status `Pending`, filled in later phases).
3. `InstallWizardToggleExpand` message + navigation (Enter-on-parent toggles; Esc collapses first).
4. Rename `AndroidTools` everywhere (handlers, executor, TUI, tests); keep the JDK gate on the
   Android leaf's component bucket.
5. TUI: indent + caret rendering; dynamic step-list height; footer hint.

**Milestone:** "Platforms" expands to reveal Android (fully functional) + placeholder leaves.

### Phase 3 — Web leaf + `web_browser_executable` (cross-host, detect + guided)

**Goal:** Web platform setup with a configurable browser.
1. `ComponentKind::WebBrowser` + `checks/web.rs`: detect `CHROME_EXECUTABLE` → default Chrome paths →
   Edge (Windows). Roll up missing as **Partial/Warning** (non-blocking).
2. `ToolchainSettings.web_browser_executable` config field.
3. Web leaf guided commands: download Chrome **or** `export CHROME_EXECUTABLE="<value>"` (per-OS;
   uses the configured value when set).
4. (Optional, may defer to Phase 6) propagate `CHROME_EXECUTABLE` into the session launch env so
   `flutter run -d chrome` honours it at runtime.

**Milestone:** Web leaf shows browser status + a copy-paste `CHROME_EXECUTABLE` command.

### Phase 4 — iOS + macOS leaves (macOS-host-only, shared Xcode/CocoaPods)

**Goal:** Apple-platform diagnostics + guided setup.
1. `ComponentKind::XcodeTools` + `CocoaPods`; `checks/ios.rs` (one shared probe: `xcode-select -p`,
   `xcodebuild -version`, EULA, `simctl`, `pod --version`), host-gated to macOS.
2. Display the shared status under both the iOS and macOS leaves.
3. Guided commands: Xcode (App Store / `xcodes`), `xcode-select -s … && xcodebuild -runFirstLaunch`,
   `xcodebuild -license accept`, `xcodebuild -downloadPlatform iOS` (iOS only), `brew install cocoapods`.

**Milestone:** On macOS, iOS & macOS leaves report Xcode/CocoaPods status with guided commands.

### Phase 5 — Windows leaf (Windows-host-only)

**Goal:** Windows desktop diagnostics + guided setup.
1. `ComponentKind::VisualStudioCpp`; `checks/windows.rs` using `vswhere.exe` (requires
   `VC.Tools.x86.x64`, `VC.CMake.Project`), host-gated to Windows.
2. Guided commands: `winget install Microsoft.VisualStudio.2022.BuildTools` with the
   `Microsoft.VisualStudio.Workload.NativeDesktop` override; `choco` alternative; "modify existing VS
   install" hint when VS is present but the C++ workload is missing.

**Milestone:** On Windows, the Windows leaf reports the VS C++ workload status with guided commands.

### Phase 6 — Flutter SDK version picker (fvm-style)

**Goal:** Let the user install *any* Flutter version/channel, not just latest stable. Independent of
Phases 2–5; touches only the Flutter SDK step. (Daemon download/install plumbing already exists — this
phase is mostly UI + version-tag threading.)
1. `UpdateAction::FetchFlutterReleaseManifest` → calls the existing `fetch_release_manifest(host)` →
   `Message::FlutterManifestFetched(manifest)` / `…FetchFailed`. Group releases by channel
   (`stable`/`beta`; append synthetic `master`/`main` git-only rows); filter by host arch on macOS.
2. `VersionPickerState` on `InstallWizardState` + `handler/install_wizard/version_picker.rs`:
   open/close, loading/error, channel-tab focus, `j/k` navigation, default cursor on the **active
   stable** release, confirm → store `selected_release`.
3. TUI `version_picker.rs` overlay: channel tabs, list with version + release date + arch, "git-only"
   badge for master/main, footer hints. Reuse `flutter_version_panel` scroll patterns.
4. Thread the selection through install: extend `FlutterStepParams` with `version_tag: Option<String>`;
   set `FlutterInstallTarget.version_dir_name` to the pinned version (so it lands in
   `~/fvm/versions/<version>`); use git clone `-b <ref>` (channel or tag) with archive fallback; add
   `resolve_version_release` for exact-version archive matching. `master`/`main` force the git path.
5. Entry point: on the Flutter SDK step, `Enter` opens the picker when no version is chosen yet (or a
   dedicated `v` key); the soft default remains `settings.toolchain.channel`. After install, the newly
   installed version appears in the existing Flutter Version panel + FVM cache.

**Milestone:** From the wizard's Flutter SDK step, the user browses versions grouped by channel and
installs a chosen stable/beta/pinned/master build into its own `~/fvm/versions/<version>` dir.

### Phase 7 (optional) — Runtime propagation & polish

- Propagate `CHROME_EXECUTABLE` into `SpawnSession`/launch env.
- Honour `platforms_enabled` (hide platforms the user disabled via `flutter config`).
- Optional Linux-desktop leaf for host symmetry.
- Reuse the version picker in the standalone Flutter Version panel (add an "Available" tab beside the
  existing "Installed" list) so versions can be browsed/installed outside the bootstrap flow.

---

## Edge Cases & Risks

- **Hardcoded step count/index assumptions.** Many tests assert `steps.len() == 5` and use literal
  `selected_index` values (PathConfig=2, FlutterSdk=3, AndroidTools=1). All must be renumbered; the
  expandable list makes count dynamic, so prefer `.find(|s| s.kind == …)` over index literals where
  feasible. (Full list enumerated in research; carried into the task breakdown.)
- **Daemon `components.len() == 9` assertion** (`mod.rs:253`) becomes host-variable — update to assert
  per-host expectations rather than a fixed count.
- **Exhaustive `match` on `WizardStepKind`.** Adding variants forces new arms in `handle_run_selected_step`,
  the `actions/mod.rs` executor (avoid the `WizardStepFailed` catch-all for guided leaves),
  `step_detail.rs` (4 match sites), and `step_caption`. Compiler will flag missing arms — lean on it.
- **iOS/macOS shared Xcode probe** must run once and display under two leaves without double-counting
  in the rollup.
- **Web must not block handback** — verify `all_components_ok()` / `flutter_now_live()` semantics treat
  a missing browser as non-fatal.
- **Host gating correctness** — leaves must be absent (not just disabled) off-platform so navigation
  and rollups don't account for impossible components.
- **Expand/collapse + selected_index** — collapsing while a leaf is selected must clamp
  `selected_index` back onto a visible row.
- **Version picker `version_dir_name`** — today it is always the channel name; a pinned install must
  use the version string or it overwrites `~/fvm/versions/stable`. `master`/`main` have no
  archive/sha256 — force the git path and never attempt archive verification for them.
- **Manifest fetch is network + ~300 KB JSON** — fetch lazily when the picker opens (not at startup);
  show loading/error states; arch-filter on macOS (`dart_sdk_arch`); Linux/Windows are x64-only.
- **Picker selection vs `settings.toolchain.channel`** — a per-install picker choice overrides the
  config default for that run only; document the precedence to avoid surprise.

---

## Configuration Additions

```toml
[toolchain]
# Custom web browser for `flutter run -d chrome` / web doctor check.
# Any Chromium-based browser (Chrome, Edge, Brave, Chromium). Sets CHROME_EXECUTABLE.
# web_browser_executable = "/opt/brave.com/brave/brave-browser"

# (Optional, deferred) Platforms to show/enable in the wizard, mirroring `flutter config`.
# platforms_enabled = ["android", "web"]
```

---

## Keyboard Shortcuts (additions)

| Key | Mode | Action |
|-----|------|--------|
| `Enter` | InstallWizard (Platforms parent selected) | Expand / collapse the submenu |
| `Enter` | InstallWizard (platform leaf selected) | Run / show guided commands for that platform |
| `→`/`l`, `←`/`h` | InstallWizard | Expand / collapse Platforms (optional alias) |
| `Esc` | InstallWizard (expanded) | Collapse the submenu (before closing the wizard) |
| `v` | InstallWizard (Flutter SDK step) | Open the Flutter version picker |
| `j`/`k`, `Tab` | Version picker | Navigate versions / switch channel tab |
| `Enter` | Version picker | Confirm the selected version and install |
| `Esc` | Version picker | Close the picker without installing |

(All existing wizard keys — `Tab`, `j`/`k`, `r`, `c`, `[`/`]`, `Ctrl+C` — are unchanged.)

---

## Success Criteria

- [ ] Wizard step order is `Prerequisites → Platforms → Flutter SDK → PATH Configuration → Flutter Doctor`.
- [ ] Navigating to PATH no longer dead-ends on "Install Flutter first" in the normal top-to-bottom flow.
- [ ] "Platforms" is an expandable row; `Enter` reveals indented per-platform leaves.
- [ ] Android leaf retains the existing managed install + JDK gate, unchanged.
- [ ] Web leaf detects the browser (`CHROME_EXECUTABLE` → defaults → Edge) and shows a guided
      `CHROME_EXECUTABLE` command; `web_browser_executable` config feeds it; missing browser is non-blocking.
- [ ] iOS & macOS leaves appear only on macOS, share one Xcode/CocoaPods probe, and show guided commands.
- [ ] Windows leaf appears only on Windows and reports the VS C++ workload via `vswhere` + guided commands.
- [ ] Host-inapplicable leaves are absent; rollups and navigation account only for visible rows.
- [ ] The Flutter SDK step opens a version picker listing versions grouped by channel (stable/beta +
      git-only master/main), defaulting to the active stable release; arch-filtered on macOS.
- [ ] Selecting a pinned version installs it into `~/fvm/versions/<version>` (not the channel name)
      and it appears in the Flutter Version panel / FVM cache; `master`/`main` install via git.
- [ ] Existing tests updated (no hardcoded 5-step/index regressions); new per-platform checks and
      manifest grouping / version-tag threading unit-tested.

---

## References

- Supersedes nothing; **extends** [`toolchain-bootstrap/PLAN.md`](../toolchain-bootstrap/PLAN.md) (shipped).
- [Flutter web: custom browser / CHROME_EXECUTABLE](https://docs.flutter.dev/platform-integration/web/building)
- [Flutter Windows setup (Visual Studio C++ workload)](https://docs.flutter.dev/platform-integration/windows/setup)
- [Flutter iOS/macOS setup (Xcode, CocoaPods)](https://docs.flutter.dev/platform-integration/ios/setup)
- In-repo reference: `WizardStepKind` / `build_steps()` (`crates/fdemon-app/src/install_wizard/`).
- In-repo reference: `fetch_release_manifest` / `resolve_channel_release` / `install_flutter`
  (`crates/fdemon-daemon/src/toolchain/flutter_install.rs`); Flutter Version panel
  (`crates/fdemon-app/src/flutter_version/`, `crates/fdemon-tui/src/widgets/flutter_version_panel/`).
- [FVM (Flutter Version Management)](https://fvm.app) · [fvm source](https://github.com/leoafarias/fvm) — version listing + git-clone install model mirrored here.
- Flutter releases manifest: `https://storage.googleapis.com/flutter_infra_release/releases/releases_{linux,macos,windows}.json`
- Research synthesis (this planning session): 9-agent workflow `toolchain-wizard-refactor-research` + fvm/version-infra research.
