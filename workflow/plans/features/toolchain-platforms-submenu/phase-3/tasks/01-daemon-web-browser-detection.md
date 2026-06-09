## Task: Daemon WebBrowser detection — `ComponentKind::WebBrowser` + `checks/web.rs` + preflight wiring

**Objective**: Add a cross-host Web-browser detection probe to the toolchain daemon as one compiling,
test-green unit. Introduce `ComponentKind::WebBrowser`, a new `checks/web.rs` probe, thread a
`web_browser_executable` override into `run_preflight`, append the result to the components vec (always
10 total), and update the count assertion. No app/TUI changes here — the daemon must compile and all
daemon tests pass standalone.

**Depends on**: Phase 2 (merged). Foundation task for Phase 3.

**Agent:** implementor

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentKind::WebBrowser` variant + `Display` arm + Display test.
- `crates/fdemon-daemon/src/toolchain/checks/web.rs` — **NEW** `check_web` probe + unit tests.
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `mod web;` + `pub use web::check_web;`.
- `crates/fdemon-daemon/src/toolchain/mod.rs` — `run_preflight` signature + `tokio::join!` + components vec + count assertion + test callers.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` — `check_prerequisites` (platform-dispatch template).
- `crates/fdemon-daemon/src/toolchain/checks/android.rs` — sync filesystem-probe template.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name/variant.

#### 1. `types.rs` — the enum + Display

- `ComponentKind` (`:83`, currently 9 variants ending `Prerequisites`) — add `WebBrowser` as the 10th
  variant. Keep the existing derives.
- `Display` impl (`:104-117`, exhaustive — **compiler-forced**) — add
  `Self::WebBrowser => write!(f, "Web Browser")`.
- `test_component_kind_display` (`:521`, NOT an exhaustive match — manual list) — add
  `assert_eq!(ComponentKind::WebBrowser.to_string(), "Web Browser");`.

#### 2. `checks/web.rs` — the probe (NEW)

```rust
/// Detect a Chromium-based browser for Flutter web (`flutter run -d chrome`).
/// Probe order: explicit override → CHROME_EXECUTABLE env → per-OS default locations.
pub async fn check_web(platform: &HostPlatform, browser_override: Option<&str>) -> ComponentCheck
```

Probe order:
1. `browser_override` (the configured `web_browser_executable`) — if `Some(path)` and the file exists, `Ok`.
2. `std::env::var("CHROME_EXECUTABLE")` — if set and the file exists, `Ok`.
3. Per-OS defaults, dispatched on `platform` (mirror `check_prerequisites` at `prerequisites.rs:164`):
   - **Linux**: `which::which("google-chrome")` → `"google-chrome-stable"` → `"chromium"` → `"chromium-browser"`.
   - **macOS**: `PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome").is_file()`,
     then Chromium fallback. **Use `PathBuf::is_file`, NOT `which`** — Chrome is not on `PATH` on macOS
     (verified gotcha).
   - **Windows**: `%PROGRAMFILES%\Google\Chrome\Application\chrome.exe`,
     `%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe`, then `which::which("msedge")` (Edge is a valid
     Chromium engine for web).
   - **`HostPlatform::Unknown`** → `ComponentStatus::Unknown` (no display target; do not probe).

Outcome:
- **Found** → `ComponentStatus::Ok`, `detail` = the resolved path. Optionally run `<browser> --version`
  with `PROBE_TIMEOUT` (`checks/mod.rs:65`) and `strip_and_truncate` (`checks/mod.rs:51`) to enrich
  `detail`; a version probe is best-effort — fall back to the bare path on timeout/error.
- **Not found** → `ComponentStatus::Missing`. **Report raw `Missing` here** — the app layer (Task 03)
  caps it to non-blocking `Partial`. The daemon reports ground truth.

Use the established `ComponentCheck` construction shape (see `check_git` at `checks/mod.rs:126` and the
android checks). Keep the function `async` so it slots into the existing `tokio::join!`.

#### 3. `checks/mod.rs` — wire-up

Add `mod web;` and `pub use web::check_web;` alongside the existing `android` / `prerequisites`
re-exports.

#### 4. `mod.rs` — `run_preflight`

- Signature (`:112`) — add a parameter:
  `web_browser_executable: Option<&str>` (place it last, after `override_android_root`).
- The `tokio::join!` fan-out (`:180-186`) — add `checks::check_web(&platform, web_browser_executable)`.
  Web has **no Android-root dependency**, so it is safe inside the `join!` alongside `check_git` /
  `check_jdk` (the synchronous android-root checks above the `join!` are unaffected). Capture as `web_check`.
- The components vec (`:190-200`) — append `web_check` as the **10th** element, after `prereq_check`.
  Update the order comment to list 10 components.

#### 5. `mod.rs` tests — count assertion + callers

- `test_run_preflight_returns_report_without_panicking` (`:245`, assertion at `:253`):
  - `assert_eq!(report.components.len(), 9)` → `10`.
  - Add `assert_eq!(report.components[9].kind, ComponentKind::WebBrowser);` after the existing
    `[8] == Prerequisites` assertion.
  - Update the `:252` "Must always have 9 components" comment → "10".
- Every `run_preflight(...)` test caller (`:249` and others, e.g. `:308`, `:324`) — add the new trailing
  `None` web argument.

### Acceptance Criteria

1. `cargo build -p fdemon-daemon` compiles (both exhaustive matches — the `Display` impl — updated).
2. `run_preflight` always returns **10** components on every host; index `[9]` is `WebBrowser`.
3. `check_web` returns `Ok` (with a path/version `detail`) when a browser is found via override /
   `CHROME_EXECUTABLE` / per-OS default; `Missing` when none found; `Unknown` for `HostPlatform::Unknown`.
4. The probe never panics and respects `PROBE_TIMEOUT` for any spawned version probe.
5. `cargo test -p fdemon-daemon --lib` green; `cargo fmt --all` + `cargo clippy -p fdemon-daemon -- -D warnings` clean.

### Testing

```bash
cargo build -p fdemon-daemon
cargo test -p fdemon-daemon --lib toolchain
cargo test -p fdemon-daemon --lib
cargo fmt --all && cargo clippy -p fdemon-daemon -- -D warnings
```

New tests to add in `checks/web.rs`:
- `test_check_web_never_panics` (smoke; runs on the CI host).
- `test_check_web_respects_browser_override` (point the override at a known-present binary, e.g. the test
  binary path or a tempfile, assert `Ok` with that path in `detail`).
- `test_check_web_respects_chrome_executable_env` (set `CHROME_EXECUTABLE` to a tempfile, assert `Ok`;
  use a guard so the env var is cleared after — env mutation is process-global).
- `test_check_web_unknown_platform_returns_unknown` (`check_web(&HostPlatform::Unknown, None)` → `Unknown`).

### Notes

- **Report raw `Missing`, not `Partial`.** The non-blocking cap is the app's job (Task 03). If the daemon
  pre-capped to `Partial`, the doctor list and any future strict consumer would lose the truth.
- **Do not** add a `chrome_available: bool` field to `ToolchainReport` — the component slot carries the
  status. (The `winget_available` field exists because *no* component represents it; `WebBrowser` is a
  real component, so no extra field is needed.)
- **Env var tests are global** — `CHROME_EXECUTABLE` and any tempfile-based override must restore/clear
  state to avoid cross-test contamination. Prefer a single serialized test or a scope guard.
- This task is the sole owner of the `ComponentKind` enum change; Tasks 02–04 build on the merged result.

---

## Completion Summary

**Status:** Done (validated PASS) · **Branch:** feat/toolchain-platforms-submenu · **Commit:** `0a97b04`

> Reconstructed by the orchestrator (the implementor's original summary was lost when task-file changes
> were discarded during the Wave-2 merge bookkeeping; the code commit is intact and validated).

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/types.rs` | Added `ComponentKind::WebBrowser` (10th variant) + `Display` arm `"Web Browser"` + display-test assertion |
| `crates/fdemon-daemon/src/toolchain/checks/web.rs` (new) | `check_web(platform, browser_override)`: probe order override → `CHROME_EXECUTABLE` → per-OS defaults (Linux `which`; macOS `PathBuf::is_file`; Windows Program Files/LocalAppData + `msedge`); `Ok`/`Missing`/`Unknown`; best-effort `--version` via `PROBE_TIMEOUT`; 4 unit tests (`serial_test` for env mutation) |
| `crates/fdemon-daemon/src/toolchain/checks/mod.rs` | `mod web;` + `pub use web::check_web;` |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | `run_preflight` gains `web_browser_executable: Option<&str>`; `check_web` added to `tokio::join!`; `web_check` appended (10th component); count assertion `9`→`10` + `[9] == WebBrowser`; test callers updated |
| Cross-crate stubs (minimal, to keep workspace compiling) | `fdemon-app`: `handler/mod.rs` (`RunToolchainPreflight.web_browser_executable` field), `actions/mod.rs` (executor passes `.as_deref()`), `handler/install_wizard/{navigation,actions}.rs` (`None` placeholders), `install_wizard/state.rs` (`WebBrowser` routing arm + `Pending`-fallback `web_status`); `src/doctor.rs` (`None` arg). Tasks 02/03 replace the placeholders with real wiring. |

### Testing

- `cargo test -p fdemon-daemon --lib` — 1183 passed, 0 failed
- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` — clean · `cargo fmt --all -- --check` — clean

### Validation

task_validator: **PASS** — all 5 acceptance criteria met; cross-crate stubs confirmed minimal/in-scope; macOS `PathBuf` gotcha honoured; env-var tests serialized.
