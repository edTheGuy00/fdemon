# Plan: Flutter SDK Management

## TL;DR

Add a comprehensive Flutter SDK locator and version management system to fdemon — shipped as a single feature. A fresh `flutter_sdk/` module in `fdemon-daemon` replaces the hardcoded `Command::new("flutter")` with a multi-strategy SDK discovery chain that detects FVM, Puro, asdf, mise, proto, and manual installations. A new Flutter Version panel (opened with `V`, following the New Session Dialog design pattern) provides TUI-based SDK visibility and management. For version pinning, fdemon reads and writes `.fvmrc` for ecosystem compatibility, and uses the FVM cache (`~/fvm/versions/`) for managed SDK storage. Managed installation is a low-priority fallback — fdemon assumes Flutter is already installed via some tool.

---

## Background

### Current State
fdemon spawns Flutter via `Command::new("flutter")` in three independent call sites (`process.rs`, `devices.rs`, `emulators.rs`), relying entirely on OS `PATH` resolution. There is no Flutter SDK path configuration anywhere in the codebase.

### Problem (Issue #9)
Users who install Flutter through version managers (Puro, FVM) get `FlutterNotFound` errors because:
1. On Windows, version managers use `.bat` wrapper scripts that Rust's `Command` cannot resolve directly
2. Version managers may not place `flutter` on the system PATH, instead using shims or per-project symlinks
3. fdemon has no awareness of per-project Flutter version pinning (`.fvmrc`, `.puro.json`, `.tool-versions`)

### PR #19 (Reference)
PR #19 adds a `flutter_locator.rs` module to `fdemon-daemon` with a `FlutterExecutable` enum (`Direct`/`WindowsBatch` variants) and detection via `FLUTTER_ROOT` env var + PATH search. We will **not build on PR #19** — instead we create a fresh `flutter_sdk/` directory module to accommodate the expanded scope (10 detection strategies, version management, TUI panel).

### Design Decisions (Resolved)

| Decision | Resolution | Rationale |
|----------|-----------|-----------|
| Shipping | Both phases ship together as one feature | Single cohesive release |
| Module structure | Fresh `flutter_sdk/` directory module | PR #19's flat file doesn't scale to 10+ strategies + types + management |
| Version pinning format | `.fvmrc` (FVM-compatible JSON) | Ecosystem compatibility — FVM has highest adoption |
| SDK cache location | `~/fvm/versions/` (shared with FVM) | Avoids duplicate downloads for FVM users |
| Managed installation priority | Low — fallback only when no SDK found | fdemon assumes users have Flutter installed |
| TUI panel | New Flutter Version panel via `V` key | Full-screen overlay following New Session Dialog pattern |
| Detection logging | `debug` level for full chain | Enables troubleshooting without noise |
| Architecture approach | Option A (detect-only, native file parsing) + Option C (embedded management as fallback) | No hard dependency on FVM CLI |

---

## Affected Modules

### Existing Files (Modified)
- `crates/fdemon-core/src/error.rs` — New error variants for SDK detection failures
- `crates/fdemon-daemon/src/process.rs` — Use locator instead of `Command::new("flutter")`
- `crates/fdemon-daemon/src/devices.rs` — Use locator instead of `Command::new("flutter")`
- `crates/fdemon-daemon/src/emulators.rs` — Use locator instead of `Command::new("flutter")`
- `crates/fdemon-daemon/src/lib.rs` — Re-export `flutter_sdk` module
- `crates/fdemon-daemon/src/tool_availability.rs` — Add Flutter SDK check at startup
- `crates/fdemon-app/src/config/types.rs` — Add `flutter_sdk_path` to `Settings`
- `crates/fdemon-app/src/message.rs` — New `Message` variants for SDK status + Flutter Version panel
- `crates/fdemon-app/src/state.rs` — `UiMode::FlutterVersion`, `FlutterVersionState`, SDK resolution state
- `crates/fdemon-app/src/handler/keys.rs` — `V` key binding + `handle_key_flutter_version()`
- `crates/fdemon-app/src/handler/update.rs` — Wire `FlutterVersion*` message variants
- `crates/fdemon-app/src/handler/mod.rs` — New `UpdateAction` variants for SDK operations
- `crates/fdemon-tui/src/render/mod.rs` — `UiMode::FlutterVersion` render branch
- `crates/fdemon-tui/src/widgets/mod.rs` — Re-export Flutter Version panel widget

### New Files — SDK Locator (`fdemon-daemon`)
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs` — **NEW** Module root
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs` — **NEW** Multi-strategy SDK discovery
- `crates/fdemon-daemon/src/flutter_sdk/version_managers.rs` — **NEW** FVM/Puro/asdf/mise/proto detection
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` — **NEW** `FlutterSdk`, `SdkSource`, `FlutterExecutable`
- `crates/fdemon-daemon/src/flutter_sdk/installer.rs` — **NEW** SDK installation (low priority fallback)
- `crates/fdemon-daemon/src/flutter_sdk/channel.rs` — **NEW** Channel/version info extraction

### New Files — Flutter Version Panel (`fdemon-app`)
- `crates/fdemon-app/src/flutter_version/mod.rs` — **NEW** Module root, state re-exports
- `crates/fdemon-app/src/flutter_version/state.rs` — **NEW** `FlutterVersionState`, sub-states
- `crates/fdemon-app/src/flutter_version/types.rs` — **NEW** Panel-specific types
- `crates/fdemon-app/src/handler/flutter_version/mod.rs` — **NEW** Handler root
- `crates/fdemon-app/src/handler/flutter_version/navigation.rs` — **NEW** Pane/field navigation
- `crates/fdemon-app/src/handler/flutter_version/actions.rs` — **NEW** SDK switch/install actions

### New Files — Flutter Version Panel Widget (`fdemon-tui`)
- `crates/fdemon-tui/src/widgets/flutter_version_panel/mod.rs` — **NEW** Widget root, layout dispatch
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs` — **NEW** Current SDK info pane
- `crates/fdemon-tui/src/widgets/flutter_version_panel/version_list.rs` — **NEW** Installed versions list
- `crates/fdemon-tui/src/widgets/flutter_version_panel/channel_selector.rs` — **NEW** Channel switching UI

---

## Development Phases

### Phase 1: Multi-Strategy SDK Locator

**Goal**: Replace `Command::new("flutter")` with a robust, multi-strategy SDK discovery system that works with all major version managers out of the box.

#### Detection Chain (Priority Order)

The locator walks this chain, returning the first valid SDK path. Each step is logged at `debug` level with the strategy name and result:

```
 Priority  Source              Config File              SDK Path Resolution
 ────────  ──────              ───────────              ───────────────────
 1.        Explicit config     config.toml              User-specified path
 2.        FLUTTER_ROOT        env var                  $FLUTTER_ROOT
 3.        FVM (modern)        .fvmrc                   ~/fvm/versions/<ver>/
 4.        FVM (legacy)        .fvm/fvm_config.json     resolve .fvm/flutter_sdk symlink
 5.        Puro                .puro.json               ~/.puro/envs/<env>/flutter/
 6.        asdf                .tool-versions           ~/.asdf/installs/flutter/<ver>/
 7.        mise                .mise.toml               ~/.local/share/mise/installs/flutter/<ver>/
 8.        proto               .prototools              ~/.proto/tools/flutter/<ver>/
 9.        flutter_wrapper     flutterw + .flutter/     .flutter/ (project-local)
 10.       System PATH         which/where flutter      resolve symlinks to real path
```

Each candidate is **validated** by checking:
- `<path>/bin/flutter` (or `flutter.bat` on Windows) exists
- `<path>/VERSION` file is readable
- `<path>/bin/cache/dart-sdk/` exists (confirms a complete SDK)

#### Key Types

```rust
/// How the Flutter SDK was discovered
#[derive(Debug, Clone, PartialEq)]
pub enum SdkSource {
    ExplicitConfig,           // config.toml flutter_sdk_path
    EnvironmentVariable,      // FLUTTER_ROOT
    Fvm { version: String },  // .fvmrc or .fvm/fvm_config.json
    Puro { env: String },     // .puro.json
    Asdf { version: String }, // .tool-versions
    Mise { version: String }, // .mise.toml
    Proto { version: String },// .prototools
    FlutterWrapper,           // flutterw
    SystemPath,               // PATH lookup
}

/// Resolved Flutter SDK with metadata
#[derive(Debug, Clone)]
pub struct FlutterSdk {
    /// Root directory of the Flutter SDK
    pub root: PathBuf,
    /// Path to the flutter executable (bin/flutter or bin/flutter.bat)
    pub executable: FlutterExecutable,
    /// How this SDK was discovered
    pub source: SdkSource,
    /// Flutter version string (from VERSION file)
    pub version: String,
    /// Current channel (from git branch, if detectable)
    pub channel: Option<String>,
}

/// How to invoke the flutter binary
#[derive(Debug, Clone)]
pub enum FlutterExecutable {
    /// Unix shell script or Windows .exe — invoke directly
    Direct(PathBuf),
    /// Windows .bat file — requires cmd /c wrapper
    WindowsBatch(PathBuf),
}
```

#### Steps

1. **Create `flutter_sdk/` module in `fdemon-daemon`**
   - `types.rs` — `FlutterSdk`, `SdkSource`, `FlutterExecutable`, validation helpers
   - `locator.rs` — `find_flutter_sdk(project_path, explicit_path: Option<&Path>) -> Result<FlutterSdk>` with the detection chain
   - `version_managers.rs` — Per-tool detection: `detect_fvm()`, `detect_puro()`, `detect_asdf()`, `detect_mise()`, `detect_proto()`, `detect_flutter_wrapper()`
   - `channel.rs` — Extract channel info from SDK's git state or VERSION file

2. **Version manager config parsing (native, no CLI invocations)**
   - FVM: Parse `.fvmrc` (JSON) for `flutter` field. Resolve cache path via `FVM_CACHE_PATH` env var, falling back to `~/fvm/versions/`. Also check `.fvm/flutter_sdk` symlink via `fs::canonicalize()`.
   - Puro: Parse `.puro.json` (JSON) for `env` field. SDK at `~/.puro/envs/<env>/flutter/`.
   - asdf: Parse `.tool-versions` (line format: `flutter <version>`). SDK at `~/.asdf/installs/flutter/<version>/`.
   - mise: Parse `.mise.toml` (TOML) `[tools]` section. SDK at `~/.local/share/mise/installs/flutter/<version>/`.
   - proto: Parse `.prototools` (TOML) for `flutter` key. SDK at `~/.proto/tools/flutter/<version>/`.

3. **Directory tree walk for config files**
   - Walk from `project_path` upward to filesystem root (like rustup's `rust-toolchain.toml` search)
   - Stop at the first config file found per tool
   - Respects monorepo layouts where `.fvmrc` may be at the workspace root

4. **Update all call sites**
   - `process.rs` — `spawn_internal()` accepts `&FlutterSdk` instead of hardcoding `"flutter"`
   - `devices.rs` — `discover_devices()` uses resolved SDK
   - `emulators.rs` — `discover_emulators()` and `run_flutter_emulator_launch()` use resolved SDK

5. **Add `flutter_sdk_path` to `Settings`**
   - New optional field in `config.toml` under `[flutter]` section: `sdk_path = "/path/to/flutter"`
   - Highest priority in the detection chain (explicit user override)

6. **Update `ToolAvailability`**
   - Add Flutter SDK check at startup
   - Cache the resolved `FlutterSdk` for reuse across sessions
   - Show which version manager was detected in the status display

7. **Debug logging for detection chain**
   - Each strategy logs at `debug!` level: strategy name, config file found/not found, path tried, validation result
   - Final resolution logs at `info!` level: "Flutter SDK resolved via {source}: {version} at {path}"

**Milestone**: fdemon works out of the box for users with FVM, Puro, asdf, mise, proto, flutter_wrapper, or manual Flutter installations. The resolved SDK source is visible in the UI.

---

### Phase 2: Flutter Version Panel (TUI)

**Goal**: Provide a dedicated TUI panel for viewing and managing Flutter SDK versions, following the New Session Dialog design pattern.

#### Panel Design (New Session Dialog Pattern)

The Flutter Version panel is a **centered popup overlay** (80% width, 70% height) with rounded border and drop shadow, rendered on top of the current view. It follows the same architectural pattern as the New Session Dialog:

- **Own `UiMode`**: `UiMode::FlutterVersion` — dedicated key routing and render branch
- **Centered overlay**: Uses `modal_overlay::dim_background()` + `centered_rect()` + shadow
- **Two-pane layout** (horizontal when width >= 70, stacked when narrower):

```
┌──────────────────────────────────────────────────────┐
│  Flutter SDK                          [Esc] Close    │  ← header
│  Manage Flutter SDK versions and channels.           │  ← subtitle
├──────────────────────────────────────────────────────┤
│                    │                                 │
│  Current SDK       │  Available Versions             │
│  (40% width)       │  (60% width)                    │
│                    │                                 │
│  VERSION           │  Installed                      │
│  3.19.0            │  ● 3.19.0 (stable) ← active    │
│                    │    3.16.0                        │
│  CHANNEL           │    3.22.0-beta                   │
│  stable            │                                 │
│                    │  Channels                       │
│  SOURCE            │    stable ← current             │
│  FVM (.fvmrc)      │    beta                         │
│                    │    main                          │
│  SDK PATH          │                                 │
│  ~/fvm/versions/   │  [Enter] Switch  [i] Install    │
│  3.19.0/           │  [d] Remove      [u] Update     │
│                    │                                 │
├──────────────────────────────────────────────────────┤
│  [Tab] Pane  [↑↓] Navigate  [Enter] Switch  [Esc]   │  ← footer
└──────────────────────────────────────────────────────┘
```

**Left pane — Current SDK Info**: Read-only display of the resolved SDK: version, channel, source (which version manager detected it), SDK root path, Dart SDK version.

**Right pane — Available Versions**: Scrollable list of installed SDK versions (from FVM cache at `~/fvm/versions/`) and available channels. Active version is highlighted. Supports switching, installing new versions, removing unused versions, and updating.

#### State Structure (follows NewSessionDialogState pattern)

```rust
pub struct FlutterVersionState {
    pub sdk_info: SdkInfoState,              // left pane — current SDK details
    pub version_list: VersionListState,      // right pane — installed versions
    pub focused_pane: FlutterVersionPane,    // which pane has focus
    pub visible: bool,
}

pub enum FlutterVersionPane {
    SdkInfo,
    VersionList,
}

pub struct SdkInfoState {
    pub resolved_sdk: Option<FlutterSdk>,    // from Phase 1 locator
}

pub struct VersionListState {
    pub installed_versions: Vec<InstalledSdk>, // scanned from ~/fvm/versions/
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub loading: bool,
}

pub struct InstalledSdk {
    pub version: String,
    pub channel: Option<String>,
    pub path: PathBuf,
    pub is_active: bool,                     // matches current resolved SDK
}
```

#### Handler Decomposition (follows handler/new_session/ pattern)

```
crates/fdemon-app/src/handler/flutter_version/
├── mod.rs              — Re-exports, handle_open/close
├── navigation.rs       — Pane switching, list up/down, field navigation
└── actions.rs          — Switch version, install, remove, update (async actions)
```

#### Key Routing

- **Normal mode**: `V` → `Message::ShowFlutterVersion` (opens panel)
- **FlutterVersion mode**:
  - `Esc` → `Message::HideFlutterVersion` (closes panel)
  - `Tab` → Switch pane focus
  - `j`/`Down` → Navigate down in version list
  - `k`/`Up` → Navigate up in version list
  - `Enter` → Switch to selected version (writes `.fvmrc`, re-resolves SDK)
  - `i` → Install new version (if managed installation is available)
  - `d` → Remove selected version
  - `u` → Update selected version/channel
  - `Ctrl+C` → Quit

#### Version Switching Flow

When the user selects a version and presses `Enter`:
1. Write/update `.fvmrc` in the project root: `{ "flutter": "<version>" }`
2. Re-run the SDK locator — FVM detection now picks up the new `.fvmrc`
3. Update `AppState` with the new resolved `FlutterSdk`
4. If a session is running, prompt: "SDK changed. Hot restart required."
5. Show confirmation in the panel

#### Rendering (render/mod.rs integration)

```rust
UiMode::FlutterVersion => {
    // Render the underlying view first (logs, etc.)
    // ...existing render logic for Normal mode...

    // Then overlay the Flutter Version panel
    let panel = widgets::FlutterVersionPanel::new(
        &state.flutter_version_state,
        &icons,
    );
    frame.render_widget(panel, area);
}
```

**Milestone**: Users can view current SDK details, see all installed versions, and switch between them directly from the TUI.

---

### Phase 3: Managed Installation (Low Priority Fallback)

**Goal**: For users without Flutter installed at all, provide a built-in way to install and manage Flutter SDKs. This is the lowest priority — only triggered when the Phase 1 locator finds nothing.

#### SDK Storage

Managed SDKs are stored in the **FVM cache** at `~/fvm/versions/` to be compatible with FVM's layout. If FVM is not installed, fdemon creates this directory structure itself.

```
~/fvm/
├── versions/
│   ├── stable/           # git checkout of stable branch
│   ├── beta/
│   ├── 3.19.0/           # specific version tag
│   └── ...
└── default -> stable     # symlink to default version
```

#### Installation Flow

1. Locator returns `FlutterNotFound` — no SDK detected anywhere
2. TUI shows: "No Flutter SDK found. Press `V` to install."
3. User opens Flutter Version panel → right pane shows "No versions installed"
4. User presses `i` → prompted for channel/version selection
5. fdemon clones `https://github.com/flutter/flutter.git` into `~/fvm/versions/<version>/`
   - Use shallow clone (`--depth 1`) for channel checkouts
   - Use archive downloads from `storage.googleapis.com` for tagged releases when possible
6. Progress bar shown in the panel during download
7. After clone: run `flutter precache` to download engine binaries
8. Write `.fvmrc` in the project root
9. Re-run locator — now finds the SDK via FVM detection

#### Channel/Version Switching

- Switching channels: `git checkout <branch>` in the SDK directory + `flutter precache`
- Switching versions: check if `~/fvm/versions/<version>/` exists; if not, clone it
- Updating: `git pull` in the SDK directory + `flutter upgrade`

#### CLI Commands (optional, low priority)

```
fdemon sdk install [channel|version]    # download/clone a specific Flutter SDK
fdemon sdk list                         # show installed SDKs
fdemon sdk use [channel|version]        # switch active SDK for current project
fdemon sdk remove [channel|version]     # remove a cached SDK
fdemon sdk update                       # update the active SDK
```

**Milestone**: Users with no existing Flutter installation can install and manage Flutter entirely through fdemon, using FVM-compatible storage.

---

### Phase 4: Polish & Integration (Future)

- Auto-detect SDK version drift (project pins 3.19.0 but SDK is 3.22.0)
- Suggest SDK upgrade when new stable release is available
- Integration with launch configurations (different Flutter versions per launch config)
- Multi-SDK support per session (run Session 1 on stable, Session 2 on beta)
- `fdemon sdk prune` to remove unused versions and reclaim disk space

---

## Edge Cases & Risks

### Version Manager Detection
- **Risk:** FVM cache path varies across versions (v2 vs v3) and platforms
- **Mitigation:** Check `FVM_CACHE_PATH` env var first, then try known defaults in order. Validate each candidate path.

### Symlink Resolution
- **Risk:** `.fvm/flutter_sdk` is a relative symlink that may break if the project is moved
- **Mitigation:** Use `fs::canonicalize()` and fall back to cache lookup if symlink is broken

### Windows .bat Files
- **Risk:** `Command::new()` cannot execute `.bat` files directly on Windows
- **Mitigation:** `FlutterExecutable::WindowsBatch` variant wraps execution in `cmd /c`

### Puro .puro.json Not in Git
- **Risk:** Puro auto-gitignores `.puro.json`, so detection only works locally (not in CI)
- **Mitigation:** Also check `PURO_ROOT` env var and Puro's default PATH entries as fallback

### SDK Validation
- **Risk:** A detected path may point to a corrupted or incomplete SDK
- **Mitigation:** Validate `bin/flutter` + `VERSION` file + `bin/cache/dart-sdk/` existence. If validation fails, continue to next detection strategy.

### Multiple Version Managers
- **Risk:** User has both FVM and asdf configured — which wins?
- **Mitigation:** Follow the priority chain strictly. Show which source was selected in the UI. Allow explicit override via `config.toml`.

### .fvmrc Compatibility
- **Risk:** fdemon writes `.fvmrc` but FVM expects specific fields/formatting
- **Mitigation:** Only write the minimal `{ "flutter": "<version>" }` schema. Read additional fields but don't modify them. Test with FVM v2 and v3.

### FVM Cache Sharing
- **Risk:** fdemon and FVM both writing to `~/fvm/versions/` could cause conflicts
- **Mitigation:** fdemon only reads from the cache for detection. For managed installation (Phase 3), fdemon creates new version directories but never modifies existing ones. Use file locking when writing.

### Git Clone Reliability (Phase 3)
- **Risk:** Flutter repo is ~2GB; clone can fail on slow connections
- **Mitigation:** Support resume-able downloads. Use shallow clone (`--depth 1`). Use archive downloads for tagged releases when possible.

### Disk Space (Phase 3)
- **Risk:** Multiple Flutter SDKs consume significant disk space (~2-3GB each)
- **Mitigation:** Show disk usage in version list. Provide removal from the panel UI.

---

## Configuration Additions

### config.toml

```toml
[flutter]
# Explicit SDK path override (highest priority in detection chain)
# sdk_path = "/path/to/flutter"
```

### .fvmrc (per-project, FVM-compatible)

Written by fdemon when user switches versions via the Flutter Version panel:

```json
{
  "flutter": "3.19.0"
}
```

fdemon reads all FVM `.fvmrc` fields but only writes the `flutter` field when creating/updating.

---

## Keyboard Shortcuts Summary

| Key | Mode | Action |
|-----|------|--------|
| `V` | Normal | Open Flutter Version panel |
| `Esc` | FlutterVersion | Close panel |
| `Tab` | FlutterVersion | Switch pane focus (SDK Info ↔ Version List) |
| `j`/`Down` | FlutterVersion | Navigate down in version list |
| `k`/`Up` | FlutterVersion | Navigate up in version list |
| `Enter` | FlutterVersion | Switch to selected version |
| `i` | FlutterVersion | Install new version |
| `d` | FlutterVersion | Remove selected version |
| `u` | FlutterVersion | Update selected version |
| `Ctrl+C` | FlutterVersion | Quit fdemon |

---

## Success Criteria

### Feature Complete When:
- [ ] fdemon detects Flutter installed via FVM (v2 and v3), Puro, asdf, mise, proto, flutter_wrapper, and manual installation
- [ ] `FLUTTER_ROOT` env var is respected as highest-priority auto-detection
- [ ] `config.toml` `flutter.sdk_path` overrides all detection
- [ ] All three call sites (`process.rs`, `devices.rs`, `emulators.rs`) use the locator
- [ ] Windows `.bat` wrapper files are handled correctly
- [ ] Detection chain logged at `debug` level for troubleshooting
- [ ] Directory tree walk finds config files in parent directories (monorepo support)
- [ ] `ToolAvailability` includes Flutter SDK check at startup
- [ ] Flutter Version panel opens with `V` key (centered popup, New Session Dialog style)
- [ ] Panel shows current SDK info: version, channel, source, path
- [ ] Panel lists installed versions from `~/fvm/versions/`
- [ ] Version switching writes `.fvmrc` and re-resolves SDK
- [ ] Managed installation available as fallback when no SDK found (low priority)
- [ ] Comprehensive unit tests for each detection strategy
- [ ] Existing tests pass, no regressions

---

## References

- [Issue #9 — Flutter SDK Not Found with Puro](https://github.com/edTheGuy00/fdemon/issues/9)
- [PR #19 — Flutter SDK Detection](https://github.com/edTheGuy00/fdemon/pull/19)
- [FVM Documentation](https://fvm.app/documentation/getting-started/configuration)
- [FVM API Commands](https://fvm.app/documentation/guides/basic-commands)
- [Puro Manual](https://puro.dev/reference/manual/)
- [asdf Flutter Plugin](https://github.com/asdf-community/asdf-flutter)
- [mise Documentation](https://mise.jdx.dev/)
- [proto Flutter Support](https://moonrepo.dev/proto)
- [Dart-Code SDK Locating](https://dartcode.org/docs/sdk-locating/) — VS Code extension's search order (useful reference)
- [Rustup Overrides](https://rust-lang.github.io/rustup/overrides.html) — Prior art for toolchain resolution chains
