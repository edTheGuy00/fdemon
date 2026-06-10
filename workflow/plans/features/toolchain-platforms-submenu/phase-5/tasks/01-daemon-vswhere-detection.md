## Task: Daemon Visual Studio C++ detection — `ComponentKind::VisualStudioCpp` + `checks/windows.rs` + Windows-gated preflight wiring

**Objective**: Add a Windows-host-gated Visual Studio "Desktop development with C++" detection probe to
the toolchain daemon as one compiling, test-green unit. Introduce `ComponentKind::VisualStudioCpp`, a
new `checks/windows.rs` probe (two-gate `vswhere.exe` query → one `ComponentCheck`), wire it into
`run_preflight` so it runs **only on Windows**, and land a **minimal no-op stub arm** in `fdemon-app`'s
`build_steps` so the workspace still compiles. No real app/TUI behaviour here — the daemon must compile
and all daemon tests pass standalone.

**Depends on**: Phase 4 (merged). Foundation task for Phase 5.

**Agent:** implementor

**Complexity:** medium

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentKind::VisualStudioCpp` variant + `Display`
  arm + `test_component_kind_display` assertion.
- `crates/fdemon-daemon/src/toolchain/checks/windows.rs` — **NEW** `check_windows` probe + unit tests.
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `mod windows;` + `pub use windows::check_windows;`.
- `crates/fdemon-daemon/src/toolchain/mod.rs` — `run_preflight` Windows-gated probe wiring + components
  vec `extend` + Windows-only presence test.
- `crates/fdemon-app/src/install_wizard/state.rs` — **minimal no-op stub arm only** in the `build_steps`
  component-routing match (route `VisualStudioCpp` nowhere; the leaf stays a `Pending` placeholder).
  Task 03 replaces this.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/ios.rs` — the probe template: host-gating match shape,
  `tokio::time::timeout(PROBE_TIMEOUT, …)` + `.kill_on_drop(true)` invocation pattern (post
  Phase-4-followup hardening), pure classifier (`classify_xcode_gates`) extracted for cross-host tests.
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `PROBE_TIMEOUT`, `strip_and_truncate`,
  `MAX_DETAIL_LEN`.
- `crates/fdemon-daemon/src/toolchain/mod.rs` — `winget_available` pre-computation (~line 178; reused
  later by the app, no change here), `tokio::join!` fan-out, `components.extend(ios_checks)` site.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name/variant.

#### 1. `types.rs` — the enum + Display

- `ComponentKind` (currently 12 variants ending `CocoaPods`) — add `VisualStudioCpp` (13th). Keep the
  existing derives.
- `Display` impl (exhaustive — **compiler-forced**) — add
  `Self::VisualStudioCpp => write!(f, "Visual Studio (C++ workload)")`.
- `test_component_kind_display` (manual list, NOT an exhaustive match) — add
  `assert_eq!(ComponentKind::VisualStudioCpp.to_string(), "Visual Studio (C++ workload)");`.

#### 2. `checks/windows.rs` — the probe (NEW)

```rust
/// Detect Visual Studio with the "Desktop development with C++" workload for
/// Windows-desktop Flutter development. Windows-only: returns an empty Vec on
/// Linux/macOS, one Unknown-status check for HostPlatform::Unknown.
/// One probe pass produces one ComponentCheck: VisualStudioCpp.
pub async fn check_windows(platform: &HostPlatform) -> Vec<ComponentCheck>
```

Behaviour (mirror `check_ios`'s host-gating shape):
- **`Linux` / `MacOs`** → `Vec::new()` (the component simply does not exist off-Windows).
- **`HostPlatform::Unknown`** → one check, `ComponentStatus::Unknown` (the `check_ios` convention).
- **`Windows`** → run the vswhere probe:
  1. **Resolve `vswhere.exe`**: `%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe`
     (read the `ProgramFiles(x86)` env var — its fixed, Microsoft-documented location), falling back to
     `which::which("vswhere")`. Not found → `Missing`, detail
     `"Visual Studio not found (vswhere.exe not present)"`.
  2. **Gate 1 — any instance**: `vswhere -products * -latest -format json -utf8`
     (`-products *` is required so Build Tools SKUs count).
  3. **Gate 2 — C++ workload**: same args plus
     `-requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 Microsoft.VisualStudio.Component.VC.CMake.Project`
     (AND semantics — both components required, per the Flutter Windows-setup requirements).
  4. **Classify via a pure helper** (see below).
- Each subprocess call: `tokio::time::timeout(PROBE_TIMEOUT, Command::new(…).kill_on_drop(true)…)`,
  stdin `Stdio::null()`, output run through `strip_and_truncate`. Timeout/spawn-error → treat the gate
  as a miss and carry the reason into `detail`; **never panic**.

**Pure classifier** (the `classify_xcode_gates` pattern — all decision logic testable on Linux CI):

```rust
fn classify_vswhere_gates(gate1_json: &str, gate2_json: &str) -> ComponentCheck
```

- Parse both with `serde_json` into minimal structs (only `displayName`, `installationVersion`,
  `installationPath` are read). Empty array / parse failure → that gate is a miss.
- **Gate 2 hit** → `Ok`, detail `"<displayName> <installationVersion>"` (e.g.
  `Visual Studio Build Tools 2022 17.9.x`).
- **Gate 1 hit, gate 2 miss** → `Missing`, detail **must begin with the stable prefix
  `"Visual Studio found"`**:
  `Visual Studio found (<displayName>), but the 'Desktop development with C++' workload is missing`.
  > **Cross-crate contract:** `windows_guided_commands` in
  > `fdemon-app/src/install_wizard/state.rs` (Task 03) branches on this exact prefix to emit the
  > "modify the existing install" guidance. Add a comment at the prefix constant pointing there, and
  > define the prefix as a `pub(crate)` (or documented) `const` so the test can assert it verbatim.
- **Both miss** → `Missing`, detail `"Visual Studio not found"`.
- Apply `strip_and_truncate` to any text interpolated from vswhere output.

#### 3. `checks/mod.rs` — wire-up

Add `mod windows;` and `pub use windows::check_windows;` alongside the existing
`android` / `ios` / `web` / `prerequisites` re-exports.

#### 4. `mod.rs` — `run_preflight`

- **Signature unchanged** — no new parameter (detection is internal; no config override).
- Add `checks::check_windows(&platform)` to the existing `tokio::join!` fan-out (capture as
  `windows_checks: Vec<ComponentCheck>`).
- After `components.extend(ios_checks);` append `components.extend(windows_checks);`. On non-Windows
  this is a no-op. Update the trailing-order comment (macOS: …`XcodeTools`, `CocoaPods`; Windows:
  …`VisualStudioCpp`).

#### 5. `mod.rs` tests — Windows presence (NO count-assertion change)

- The existing `assert!(report.components.len() >= 10, …)` is **already forward-compatible** — leave it.
- In `test_run_preflight_returns_report_without_panicking`, add a Windows-gated presence assertion,
  mirroring the macOS block:
  ```rust
  #[cfg(target_os = "windows")]
  {
      assert!(report.components.iter().any(|c| c.kind == ComponentKind::VisualStudioCpp));
  }
  ```

#### 6. `fdemon-app/install_wizard/state.rs` — minimal no-op stub arm

The `build_steps` component-routing `match check.kind { … }` is exhaustive (verified — no catch-all)
and will fail to compile when the new variant exists. Add a **no-op** arm so the workspace compiles and
existing tests stay green (the Windows leaf remains a `Pending` placeholder until Task 03):

```rust
// Phase 5 Task 01 stub: routed to the Windows leaf in Task 03.
ComponentKind::VisualStudioCpp => {}
```

Do **not** add a bucket, leaf body, or guided commands here — that is Task 03's job.

### Acceptance Criteria

1. `cargo build -p fdemon-daemon` and `cargo build --workspace` compile (both exhaustive matches —
   daemon `Display` and app `build_steps` routing — updated).
2. `run_preflight` appends `VisualStudioCpp` **only on Windows**; Linux/macOS counts are unchanged; the
   `>= 10` assertion holds on every host.
3. `check_windows` returns one `Ok`/`Missing` check on Windows, an empty Vec on Linux/macOS, and one
   `Unknown` check for `HostPlatform::Unknown`. It never panics and respects `PROBE_TIMEOUT` with
   `.kill_on_drop(true)`.
4. `classify_vswhere_gates` is pure and fully covered by fixture-JSON tests that run on Linux CI:
   gate-2 hit → `Ok` with name+version detail; gate-1-only → `Missing` with the `"Visual Studio found"`
   prefix; both-miss / malformed JSON / empty array → `Missing` `"Visual Studio not found"`.
5. The `"Visual Studio found"` prefix is a named constant with a comment referencing the Task 03
   consumer, and a test asserts the classifier's output starts with it.
6. `cargo test -p fdemon-daemon --lib` green; `cargo test --workspace --lib` green (app stub keeps the
   leaf `Pending`); `cargo fmt --all` + `cargo clippy --workspace -- -D warnings` clean.

### Testing

```bash
cargo build --workspace
cargo test -p fdemon-daemon --lib toolchain
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

New tests to add in `checks/windows.rs`:
- `test_check_windows_never_panics` (smoke; runs on the CI host).
- `test_check_windows_non_windows_returns_empty` (`Linux` / `MacOs` → empty Vec).
- `test_check_windows_unknown_platform_returns_unknown_check` (`Unknown` → one `Unknown`-status check).
- `classify_vswhere_gates` fixture tests: realistic vswhere JSON for a full VS Community with the
  workload, Build Tools with the workload, VS without the workload (gate 1 only), empty array `[]`,
  malformed JSON, and over-long `displayName` (assert `strip_and_truncate` capping).
- A prefix-contract test: gate-1-only classification detail starts with the named prefix constant.
- `#[cfg(target_os = "windows")] test_check_windows_windows_returns_one_component` (asserts exactly one
  `VisualStudioCpp`, regardless of install state).

### Notes

- **Report raw `Missing`, not `Partial`.** The non-blocking cap is the app's job (Task 03). The daemon
  reports ground truth.
- **Do not add fields to `ToolchainReport`** — the single component slot plus the detail-prefix
  contract carry everything the app needs. `winget_available` already exists on the report.
- **`-products *` matters** — without it vswhere omits Build Tools SKUs, the exact SKU the guided
  command installs, producing a false `Missing` right after the user follows our own guidance.
- **Dev host is Linux** — the live vswhere path cannot be exercised here; all decision logic must sit
  in the pure classifier. The `#[cfg(target_os = "windows")]` tests only run on a real Windows host/CI.
- This task is the sole owner of the `ComponentKind` enum change; Tasks 02–04 build on the merged result.
