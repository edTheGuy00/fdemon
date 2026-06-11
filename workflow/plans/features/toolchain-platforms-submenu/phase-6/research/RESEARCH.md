# Phase 6 Research — Flutter SDK Version Picker (verified against live code, 2026-06-10)

Synthesized from a 4-agent research workflow (`phase6-version-picker-research`, run `wf_b9e2144f-de1`):
daemon install plumbing, app wizard state/handlers, TUI widgets/overlays, version-panel/FVM conventions.
Line numbers are a snapshot at branch `feat/toolchain-platforms-submenu` @ `f53a9b54` — locate by symbol.

## 1. Daemon install plumbing (`fdemon-daemon/src/toolchain/`)

- `fetch_release_manifest(platform: HostPlatform) -> Result<FlutterReleaseManifest>` —
  **pub**, `flutter_install.rs:414`. No caching; HEAD probe (5s) then GET (15s connect / 60s request)
  via `pub(crate) fetch_release_manifest_from(url)`. URL: `releases_{linux,macos,windows}.json`
  (Unknown host → linux).
- `FlutterReleaseManifest { base_url, current_stable_hash: Option<String>, releases: Vec<FlutterRelease> }`
  (`types.rs:340`). Releases are newest-first.
- **`FlutterRelease` only carries `version`, `channel`, `archive`, `sha256`, `dart_sdk_arch: Option<String>`**
  (`types.rs:321`). The raw JSON's `hash`, `release_date`, `dart_sdk_version` are **NOT deserialized**
  (`RawRelease`, `flutter_install.rs:378`) — the picker's "version + date" row needs `release_date`
  added to `RawRelease`/`FlutterRelease`.
- `resolve_channel_release(manifest, channel, arch)` is **private** (`flutter_install.rs:353`):
  pass 1 exact `dart_sdk_arch` match, pass 2 any-arch fallback. `resolve_stable()` on the manifest type
  ignores `current_stable_hash` and linear-scans `channel == "stable"` — **active stable = first stable
  entry**.
- **`resolve_version_release` does not exist** — confirmed by grep. Must be added.
- `FlutterInstallTarget { method, channel, install_root, version_dir_name }` (`types.rs:380`).
  Constructed **by struct literal in `fdemon-app/src/actions/mod.rs:924-932`** with
  `version_dir_name: params.channel.clone()` — adding a field to this struct breaks that literal
  (cross-crate compile break → Phase-5-style stub line required in the daemon task).
- `install_flutter` flow (`flutter_install.rs:589`): pre-cancel check → `validate_channel` →
  short-circuit if `final_dir/bin/flutter` exists → `LockGuard` → tmp dir + `TempDirGuard` →
  `install_inner`: `use_git = method != Archive && git on PATH`;
  git path = `git clone -b <channel> --depth 1 https://github.com/flutter/flutter.git`;
  archive path = re-fetch manifest → `resolve_channel_release` (**silently falls back to stable when
  the channel is missing** — must NOT happen for a pinned version) → download → sha256 → extract.
  Then atomic rename → `run_precache` (non-fatal) → `read_installed_version(final_dir, channel)`.
- `validate_channel` rejects empty, leading `-`, and anything outside `[A-Za-z0-9._-]` —
  **old Flutter tags like `1.12.13+hotfix.5` contain `+` and would be rejected**. Args go to git via
  `run_streaming` (no shell), so widening to allow `+` is safe.
- `git clone -b <ref>` accepts **tags as well as branches** — a pinned version can clone
  `-b 3.24.0 --depth 1`; no clone + `git reset --hard` needed (deviation from the PLAN's fvm
  description, same outcome, cheaper).
- `InstallEvent`: `Log(String)`, `Download{received, total}`, `Phase(&'static str)` —
  "Cloning"/"Downloading"/"Verifying"/"Extracting"/"Precaching".
- `HostArch::{X64, Arm64, Unknown}` (`types.rs:296`) — compile-time `cfg!` detect,
  `as_manifest_str() -> Option<&str>` ("x64"/"arm64").
- Test conventions: inline `MANIFEST_FIXTURE` JSON const; `wiremock` MockServer for HEAD/GET;
  `serial_test::serial` for `FVM_CACHE_PATH` env tests; pure-helper unit tests for resolution logic.

## 2. App wizard state & handlers (`fdemon-app`)

- `InstallWizardState` fields incl. `selected_command_index`, `execution: StepExecution`,
  `install_task`, `run_seq` (stale-message guard), `platforms_expanded`, `origin`,
  `last_known_visible_height: Cell<usize>`, `handback_done`, `observed_unhealthy`
  (`install_wizard/state.rs:92-188`). **No picker/overlay sub-state exists.**
- `WizardStepKind`: `Prerequisites, Platforms, Platform{Android,Ios,Macos,Web,Windows}, PathConfig,
  FlutterSdk, Doctor` (`types.rs:45`). `WizardStep { kind, title, status, components,
  guided_commands, indent }` lives in `state.rs:65-85`.
- FlutterSdk run arm (`handler/install_wizard/actions.rs:293-328`): channel from
  `settings.toolchain.channel`, method from `settings.toolchain.install_method()`, root from
  `settings.toolchain.flutter_install_dir` → `FlutterStepParams { method, channel, install_root }`
  (`handler/mod.rs:831-841`) → `begin_step(kind)` (bumps `run_seq`, mints `CancellationToken`,
  stores `InstallTaskHandle`) → `UpdateAction::RunWizardStep { kind, run_seq, cancel_token,
  install: Some(params), .. }`.
- **No manifest-fetch action exists.** Async-fetch pattern to copy: `RunToolchainPreflight` executor
  (`actions/mod.rs:800-834`) — `tokio::spawn` → `msg_tx.send(Message::…Completed)`.
  `ProbeFlutterVersion` → `FlutterVersionProbeCompleted` is the same shape.
- Executor FlutterSdk arm (`actions/mod.rs:892-998`): `resolve_install_dir(params.install_root)` →
  `FlutterInstallTarget` literal (**`version_dir_name` hardcoded to `params.channel`**) →
  `install_flutter(...)` with event callback → `WizardStepCompleted { sdk_path }` /
  `WizardStepFailed`; deposits join handle via `WizardInstallTaskReady { run_seq, handle }`.
- Key routing (`handler/keys.rs:433-495`, `handle_key_install_wizard`): Esc → cancel-if-running else
  escape; Enter → `InstallWizardToggleExpand` if on Platforms parent else
  `InstallWizardRunSelectedStep`; j/k, Tab, l/h, r, c, [, ]. **No overlay-within-wizard precedent**;
  the model is the tag-filter intercept in `handle_key_normal` (keys.rs:149): when overlay visible,
  intercept ALL keys first.
- Post-install registration: `handle_step_completed` (FlutterSdk) stashes `installed_sdk_path`, writes
  `settings.flutter.sdk_path`, fires `PersistSettings` + `InstallWizardAutoConfigurePath` → preflight
  rerun → `SdkResolved` → `ScanInstalledSdks` refreshes the Flutter Version panel. No explicit
  registration — the FVM cache scanner picks up any `install_root/<dir>` with `bin/flutter` + version
  file.
- `ToolchainSettings` (config/types.rs:173): `flutter_install_dir`, `channel` (default "stable"),
  `flutter_install_method` ("git" default / "archive"), plus android/jdk/web fields.
  **No version field; none needed — picker choice is per-run.**

## 3. TUI (`fdemon-tui`)

- `InstallWizardPanel` render (`widgets/install_wizard/mod.rs:394-461`): `dim_background` →
  `centered_rect_percent(80, 85)` → min-size check → shadow → clear → rounded Block →
  vertical layout (header 2 / sep / panes Min(5) / sep / footer 1). Content routing: loading →
  spinner; `is_step_running()` → full-width `StepProgress`; else horizontal (≥70 cols) or vertical
  panes. Footer: `"[Tab] switch · [j/k] move · [r] re-run · [Esc] close"` + contextual additions.
- A nested overlay inside the wizard should render **after** the panel body within
  `InstallWizardPanel::render` using `modal_overlay::{centered_rect/centered_rect_percent,
  clear_area}` (confirm_dialog/tag_filter precedent; no second `dim_background`).
- `step_detail.rs`: `step_caption` (FlutterSdk → None today), `is_executable` (FlutterSdk → true),
  `action_hint_text` (FlutterSdk → "▶ Press Enter to install Flutter SDK"), `render_action_hint`
  (1-row bottom anchor). Guided-command list + `compute_guided_window` block-scrolling pattern.
- Canonical list-scroll pattern to reuse: `VersionListState`
  (`fdemon-app/src/flutter_version/state.rs:106-122`: `selected_index`, `scroll_offset`, `loading`,
  `error`, `Cell<usize>` render-hint) + render-time `corrected_scroll` (no state mutation) in
  `flutter_version_panel/version_list.rs:235-265`.
- Widget tests: `Buffer::empty(area)` + `widget.render` + symbol-collect + `contains` assertions;
  render-hint writeback asserted before/after render; per-row scans for layout anchoring.

## 4. Refuted / corrected PLAN assumptions

| PLAN claim | Reality | Impact |
|---|---|---|
| Picker rows show "version + date + arch" | `release_date` not deserialized | Daemon task adds `release_date: Option<String>` to `RawRelease`/`FlutterRelease` (serde default) |
| "add `resolve_version_release` alongside `resolve_channel_release`" | `resolve_channel_release` is private; no version helper | New helper; keep private to `flutter_install.rs` (only `archive_install` needs it — the app picks from its own fetched manifest) |
| "clone + `git reset --hard <tag>` for pinned versions" | `git clone -b <tag> --depth 1` works for tags | Simpler: thread the tag as the `-b` ref |
| Channel-missing manifest fallback acceptable | `archive_install` silently falls back to **stable** | For a pinned `version_tag`, a manifest miss must be a **hard error**, never a stable fallback |
| (implicit) version strings are valid refs | `validate_channel` rejects `+` (old tags `1.12.13+hotfix.5`) | Widen validation for version refs (no-shell arg passing makes `+` safe) |
| `FlutterStepParams` extension is app-local | `FlutterInstallTarget` literal in `actions/mod.rs:924` breaks on new daemon field | Daemon task carries a one-line `version_tag: None` stub in `actions/mod.rs` (Phase-5 stub pattern) |
| — | `current_stable_hash` unused by `resolve_stable()`; first stable entry = active stable | Default cursor = index 0 of the Stable tab; no hash plumbing needed |
