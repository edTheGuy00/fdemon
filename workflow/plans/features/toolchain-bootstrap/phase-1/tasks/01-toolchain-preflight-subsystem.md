## Task: Toolchain Preflight Subsystem (read-only diagnostics)

**Objective**: Add a new `toolchain/` module to `fdemon-daemon` that runs a structured,
read-only diagnosis of the Flutter toolchain and returns a `ToolchainReport`. Reuses the existing
SDK locator and version probe; adds probes for git, JDK, adb, Android cmdline-tools/`sdkmanager`,
platforms/build-tools, Android licenses, and per-OS prerequisites; captures and parses
`flutter doctor -v` text when Flutter exists. **No install, download, or network code.**

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 8-10 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/mod.rs` (NEW) — public API `run_preflight()`, module
  declarations (`mod types; mod checks; mod doctor;`), re-exports.
- `crates/fdemon-daemon/src/toolchain/types.rs` (NEW) — report data types.
- `crates/fdemon-daemon/src/toolchain/checks.rs` (NEW) — structured component probes.
- `crates/fdemon-daemon/src/toolchain/doctor.rs` (NEW) — `flutter doctor -v` capture + parser.
- `crates/fdemon-daemon/src/lib.rs` — add `pub mod toolchain;` and re-export public types.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs` — `find_flutter_sdk(project_path, explicit_path)`.
- `crates/fdemon-daemon/src/flutter_sdk/version_probe.rs` — `probe_flutter_version(&FlutterExecutable)`.
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` — `FlutterSdk`, `SdkSource`, `FlutterExecutable`,
  `FlutterVersionInfo`.
- `crates/fdemon-daemon/src/tool_availability.rs` — established `tokio::process::Command` probe idiom.
- `crates/fdemon-core/src/error.rs` — `Error::FlutterNotFound`, `Error::FlutterSdkInvalid`.

### Module Structure

| File | Contents |
|------|----------|
| `types.rs` | `ToolchainReport`, `ComponentCheck`, `ComponentStatus`, `ComponentKind`, `HostPlatform`, `HostShell`, `DoctorLine`, `DoctorMarker`. Plus `HostPlatform::detect()` / `HostShell::detect()`. |
| `checks.rs` | One `async fn check_*` per component returning `ComponentCheck`; an `android_sdk_root()` resolver; a per-OS `check_prerequisites()`. |
| `doctor.rs` | `capture_flutter_doctor(&FlutterExecutable) -> Option<String>` (async) + `parse_doctor_output(&str) -> Vec<DoctorLine>` (pure). |
| `mod.rs` | `run_preflight(project_path, explicit_sdk_path) -> ToolchainReport` orchestration. |

### Details

**`types.rs` — report types** (all `Debug, Clone`; `serde::Serialize` optional for headless reuse):

```rust
pub struct ToolchainReport {
    pub platform: HostPlatform,
    pub shell: HostShell,
    pub components: Vec<ComponentCheck>,
    /// Parsed `flutter doctor -v` lines; None when Flutter is absent or capture failed.
    pub doctor: Option<Vec<DoctorLine>>,
}

pub struct ComponentCheck {
    pub kind: ComponentKind,
    pub status: ComponentStatus,
    /// Human-readable detail: version found, resolved path, or why it is missing.
    pub detail: String,
}

pub enum ComponentStatus { Ok, Partial, Missing, Error, Unknown }

pub enum ComponentKind {
    FlutterSdk, Git, Jdk, AndroidCmdlineTools, AndroidPlatformTools,
    AndroidPlatform, AndroidBuildTools, AndroidLicenses, Prerequisites,
}

pub enum HostPlatform { Linux, MacOs, Windows, Unknown }
pub enum HostShell { Bash, Zsh, Fish, PowerShell, Cmd, Unknown }

pub struct DoctorLine { pub marker: DoctorMarker, pub text: String, pub indent: usize }
pub enum DoctorMarker { Ok, Warning, Error, Dead, None } // [✓] [!] [✗] [☠] / continuation
```

- `HostPlatform::detect()` uses `cfg!(target_os = ...)`.
- `HostShell::detect()` reads `$SHELL` basename on Unix (`bash`/`zsh`/`fish`); Windows → `PowerShell`.

**`checks.rs` — structured probes** (mirror `tool_availability.rs` idiom):

- `check_flutter(project_path, explicit) -> ComponentCheck`: call `find_flutter_sdk`. On `Ok(sdk)`
  → `Ok`, detail = `"{version} ({source})"`. On `Err(FlutterNotFound)` → `Missing`. On
  `Err(FlutterSdkInvalid{path,reason})` → `Partial`, detail includes the reason.
- `check_git() -> ComponentCheck`: `Command::new("git").arg("--version")` → `.output()` with a
  short `timeout`. Present + parse version → `Ok`; absent → `Missing`.
- `check_jdk() -> ComponentCheck`: `java -version` (note: writes to **stderr**). Parse the major
  version; `>= 17` → `Ok`, present but `< 17` → `Partial` (detail names the version), absent →
  `Missing`.
- `android_sdk_root() -> Option<PathBuf>`: `$ANDROID_HOME` → `$ANDROID_SDK_ROOT` → platform default
  (`~/Android/Sdk` Linux, `~/Library/Android/sdk` macOS, `%LOCALAPPDATA%/Android/Sdk` Windows).
  Use `dirs` for home/local-data. Return the first that exists.
- `check_android_cmdline_tools(root) -> ComponentCheck`: presence of
  `<root>/cmdline-tools/latest/bin/sdkmanager(.bat)`. Found → `Ok`; a `cmdline-tools/` that is
  **not** relocated under `latest/` → `Partial` (detail notes the missing `latest/`); none →
  `Missing`.
- `check_android_platform_tools(root) -> ComponentCheck`: `adb` — prefer
  `<root>/platform-tools/adb`, else `Command::new("adb").arg("version")`.
- `check_android_platform(root)` / `check_android_build_tools(root)`: scan
  `<root>/platforms/` and `<root>/build-tools/` for at least one entry → `Ok`/`Missing`.
- `check_android_licenses(root) -> ComponentCheck`: presence of
  `<root>/licenses/android-sdk-license`. Found → `Ok`, else `Missing`/`Unknown` (no SDK root).
- `check_prerequisites(platform) -> ComponentCheck`: lightweight per-OS detection only (NO command
  generation — that is Phase 4). Linux: probe for `cmake`, `ninja`, `pkg-config`, `clang`, `curl`,
  `unzip`, `xz`/`xz-utils` on `PATH` (use `which::which`); report `Ok` when all present, `Partial`
  with the missing list otherwise. macOS: `xcode-select -p` success → `Ok`. Windows: `git` presence
  → `Ok`. Keep this minimal and defensive.

**`doctor.rs` — capture + parse:**

```rust
/// Run `flutter doctor -v` and return its raw stdout text. Display-only; never gates status.
pub async fn capture_flutter_doctor(exe: &FlutterExecutable) -> Option<String>;

/// Parse doctor text into marker-prefixed lines. Pure, total, never panics.
pub fn parse_doctor_output(text: &str) -> Vec<DoctorLine>;
```

- `capture_flutter_doctor`: `exe.command()` + `args(["doctor", "-v"])`, `Stdio::piped()` stdout,
  wrap in `tokio::time::timeout` (e.g. 60s — doctor is slow). Return `None` on timeout/spawn error.
- `parse_doctor_output`: detect the leading marker by scanning the first non-space run for
  `[✓]`/`[!]`/`[✗]`/`[☠]` (also accept ASCII fallbacks `[√]`, plain `!`); compute `indent` from
  leading whitespace; lines without a marker are `DoctorMarker::None` continuation lines. Strip
  ANSI first (reuse `flutter_sdk::diagnostics::strip_ansi` if accessible, else a local helper).

**`mod.rs` — orchestration:**

```rust
pub async fn run_preflight(project_path: &Path, explicit_sdk_path: Option<&Path>) -> ToolchainReport;
```

- Detect platform + shell.
- Run `check_flutter` first; if it produced a usable `FlutterExecutable`, also run
  `capture_flutter_doctor` → `parse_doctor_output`.
- Run the remaining checks concurrently (`tokio::join!`).
- Assemble `components` in user-facing order; return the report. `run_preflight` itself never
  returns `Err` — failures are encoded as component statuses.

**`lib.rs`:** add `pub mod toolchain;` in the `pub mod` block and
`pub use toolchain::{run_preflight, ToolchainReport, ComponentCheck, ComponentStatus, ComponentKind, HostPlatform, HostShell, DoctorLine, DoctorMarker};`.

### Acceptance Criteria

1. `run_preflight()` is callable as `fdemon_daemon::toolchain::run_preflight(...)` and as
   `fdemon_daemon::run_preflight(...)`, returns a populated `ToolchainReport`, and never panics or
   returns `Err`.
2. With a working Flutter on PATH, the `FlutterSdk` component is `Ok` and `report.doctor` is
   `Some(non-empty)`.
3. With no Flutter, the `FlutterSdk` component is `Missing` and `report.doctor` is `None`; all other
   checks still run and return their own statuses.
4. JDK below 17 yields `Partial` (not `Missing`); a `cmdline-tools/` dir without `latest/` yields
   `Partial`.
5. `parse_doctor_output` correctly classifies `[✓]/[!]/[✗]/[☠]` lines and treats unmarked lines as
   continuation; it returns `[]` (not a panic) for empty/garbage input.

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_doctor_classifies_all_markers() { /* ✓ ! ✗ ☠ + continuation */ }

    #[test]
    fn test_parse_doctor_empty_returns_empty_vec() { assert!(parse_doctor_output("").is_empty()); }

    #[test]
    fn test_parse_doctor_ignores_ansi_color_codes() { /* embed \x1b[..m */ }

    #[test]
    fn test_host_platform_detect_matches_cfg() { /* current platform */ }

    #[tokio::test]
    async fn test_check_git_present_or_missing_never_panics() { let _ = check_git().await; }
}
```

- Cover `parse_doctor_output` thoroughly (it is pure and the most fragile piece).
- For process-spawning checks, assert they return a `ComponentCheck` without panicking (environment
  may or may not have the tool); avoid asserting a specific status that depends on the CI host.

### Notes

- Do **not** add any new crate dependencies. `tokio` (full), `serde_json`, `which`, `dirs`,
  `tracing`, `regex` are already available in `fdemon-daemon`.
- `java -version` prints to **stderr** — capture stderr, not stdout, for the JDK probe.
- Keep each file under ~500 lines (CODE_STANDARDS). If `checks.rs` grows large, the Android checks
  may be split into `checks/android.rs`, but a single `checks.rs` is acceptable for Phase 1.
- Remediation/command-generation text is intentionally **deferred to Phase 4** — `ComponentCheck`
  has no `remediation` field in Phase 1 (`detail` only).
