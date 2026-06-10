## Task: Daemon Xcode + CocoaPods detection — `ComponentKind::XcodeTools` + `CocoaPods` + `checks/ios.rs` + macOS-gated preflight wiring

**Objective**: Add a macOS-host-gated full-Xcode + CocoaPods detection probe to the toolchain daemon as
one compiling, test-green unit. Introduce `ComponentKind::XcodeTools` and `ComponentKind::CocoaPods`, a
new `checks/ios.rs` probe (one pass → two `ComponentCheck`s), wire it into `run_preflight` so it runs
**only on macOS** and appends both components (12 total on macOS, 10 elsewhere), and land a **minimal
no-op stub arm** in `fdemon-app`'s `build_steps` so the workspace still compiles. No real app/TUI
behaviour here — the daemon must compile and all daemon tests pass standalone.

**Depends on**: Phase 3 (merged). Foundation task for Phase 4.

**Agent:** implementor

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentKind::XcodeTools` + `CocoaPods` variants +
  `Display` arms + `test_component_kind_display` assertions.
- `crates/fdemon-daemon/src/toolchain/checks/ios.rs` — **NEW** `check_ios` probe + unit tests.
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `mod ios;` + `pub use ios::check_ios;`.
- `crates/fdemon-daemon/src/toolchain/mod.rs` — `run_preflight` macOS-gated probe wiring + components vec
  `extend` + macOS-only presence test.
- `crates/fdemon-app/src/install_wizard/state.rs` — **minimal no-op stub arm only** in the `build_steps`
  component-routing match (route `XcodeTools | CocoaPods` nowhere; leaves stay `Pending` placeholders).
  Task 03 replaces this.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/web.rs` — `check_web` structure (the probe template:
  `HostPlatform::Unknown` early-return, version-probe helper, constants, tests).
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` — `probe_macos_xcode_clt`,
  `probe_macos_cocoapods`, `check_macos_prerequisites` (existing macOS probe patterns; `PROBE_TIMEOUT`
  usage; `MacOsProbeStatus`).
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `PROBE_TIMEOUT`, `strip_and_truncate`,
  `MAX_DETAIL_LEN`.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name/variant.

#### 1. `types.rs` — the enum + Display

- `ComponentKind` (currently 10 variants ending `WebBrowser`) — add `XcodeTools` and `CocoaPods`
  (11th + 12th variants). Keep the existing derives.
- `Display` impl (exhaustive — **compiler-forced**) — add `Self::XcodeTools => write!(f, "Xcode")` and
  `Self::CocoaPods => write!(f, "CocoaPods")`.
- `test_component_kind_display` (manual list, NOT an exhaustive match) — add
  `assert_eq!(ComponentKind::XcodeTools.to_string(), "Xcode");` and the CocoaPods equivalent.

#### 2. `checks/ios.rs` — the probe (NEW)

```rust
/// Detect full Xcode + CocoaPods for Apple-platform Flutter development (iOS/macOS).
/// macOS-only: returns an empty Vec on every other host (the caller host-gates anyway).
/// One probe pass produces two ComponentChecks: XcodeTools and CocoaPods.
pub async fn check_ios(platform: &HostPlatform) -> Vec<ComponentCheck>
```

Behaviour:
- **Non-macOS** (`Linux`/`Windows`) → return `Vec::new()` (the components simply do not exist off-macOS).
- **`HostPlatform::Unknown`** → return two checks, both `ComponentStatus::Unknown` (so the slots are
  consistent if ever rendered). Prefer empty `Vec::new()` only for the truly off-platform hosts; for
  `Unknown` emit `Unknown`-status checks. (Match the `check_web` convention of `Unknown` for `Unknown`.)
- **macOS** — run the probe pass (best-effort, each sub-probe respects `PROBE_TIMEOUT`):
  - **`XcodeTools`**:
    1. `xcode-select -p` → must resolve to a `Contents/Developer` path under a full `Xcode.app`
       (e.g. `/Applications/Xcode.app/Contents/Developer`), **not** `/Library/Developer/CommandLineTools`.
       If it points at CLT only → treat as **Missing** (full Xcode absent).
    2. `xcodebuild -version` → success + a parseable version → enriches `detail`. Failure (e.g.
       license not accepted prints `Agreeing to the Xcode/iOS license requires admin privileges`) →
       `Missing` (or `Partial` if Xcode is present but the license/first-launch is pending — your call;
       prefer `Missing` so guided commands fire, and put the reason in `detail`).
    3. Optionally probe `xcrun simctl help` reachability to enrich `detail`; best-effort.
    - Outcome: `Ok` (full Xcode usable) with version `detail`; `Missing` otherwise, `detail` carrying the
      reason (CLT-only / license pending / not installed).
  - **`CocoaPods`**: `pod --version` → `Ok` with version `detail`; `Missing` on absence/error.
- Use `strip_and_truncate` for any captured process output. Mirror the `check_web` `probe_version`
  helper shape (`tokio::time::timeout(PROBE_TIMEOUT, Command::new(...).output())`).
- Build each `ComponentCheck { kind, status, detail }` inline (no shared constructor — matches the crate
  convention).

> You may reuse the logic in `prerequisites.rs::probe_macos_xcode_clt` / `probe_macos_cocoapods` as a
> reference, but **do not couple to them** — those detect *CLT*, Phase 4 needs *full Xcode*. Write
> independent probes in `ios.rs`. Do not change `prerequisites.rs`.

#### 3. `checks/mod.rs` — wire-up

Add `mod ios;` and `pub use ios::check_ios;` alongside the existing `android` / `web` / `prerequisites`
re-exports.

#### 4. `mod.rs` — `run_preflight`

- **Signature unchanged** — no new parameter (iOS/macOS need no config override; detection is internal).
- Add `checks::check_ios(&platform)` to the existing `tokio::join!` fan-out (capture as `ios_checks:
  Vec<ComponentCheck>`). It is independent of the android-root checks, so it slots in safely beside
  `check_web`.
- After the `components` vec is built (currently 10 entries ending `web_check`), append:
  `components.extend(ios_checks);`. On non-macOS this is a no-op (empty Vec). Update the order comment to
  note the macOS-only trailing `XcodeTools`, `CocoaPods` entries.

#### 5. `mod.rs` tests — macOS presence (NO count-assertion change)

- The existing `assert!(report.components.len() >= 10, ...)` is **already forward-compatible** — leave it.
- In `test_run_preflight_returns_report_without_panicking`, add a macOS-gated presence assertion:
  ```rust
  #[cfg(target_os = "macos")]
  {
      assert!(report.components.iter().any(|c| c.kind == ComponentKind::XcodeTools));
      assert!(report.components.iter().any(|c| c.kind == ComponentKind::CocoaPods));
  }
  ```
- No `run_preflight` test caller signature changes (signature is unchanged).

#### 6. `fdemon-app/install_wizard/state.rs` — minimal no-op stub arm

The `build_steps` component-routing `match check.kind { ... }` is exhaustive and will fail to compile
when the two new variants exist. Add a **no-op** arm so the workspace compiles and existing tests stay
green (the iOS/macOS leaves remain `Pending` placeholders until Task 03):

```rust
// Phase 4 Task 01 stub: routed to the iOS/macOS leaves in Task 03.
ComponentKind::XcodeTools | ComponentKind::CocoaPods => {}
```

Do **not** add buckets, leaf bodies, or guided commands here — that is Task 03's job.

### Acceptance Criteria

1. `cargo build -p fdemon-daemon` and `cargo build --workspace` compile (both exhaustive matches — daemon
   `Display` and app `build_steps` routing — updated).
2. `run_preflight` returns **12** components on macOS (the trailing two are `XcodeTools` then `CocoaPods`)
   and **10** on Linux/Windows; the `>= 10` assertion holds on every host.
3. `check_ios` returns two `Ok`/`Missing` checks on macOS (full-Xcode + CocoaPods), an empty Vec on
   Linux/Windows, and two `Unknown` checks for `HostPlatform::Unknown`. It never panics and respects
   `PROBE_TIMEOUT`.
4. `xcode-select -p` resolving to CLT-only (not a full `Xcode.app`) yields `XcodeTools = Missing`.
5. `cargo test -p fdemon-daemon --lib` green; `cargo test --workspace --lib` green (app stub keeps leaves
   `Pending`); `cargo fmt --all` + `cargo clippy --workspace -- -D warnings` clean.

### Testing

```bash
cargo build --workspace
cargo test -p fdemon-daemon --lib toolchain
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

New tests to add in `checks/ios.rs`:
- `test_check_ios_never_panics` (smoke; runs on the CI host).
- `test_check_ios_non_macos_returns_empty` (`check_ios(&HostPlatform::Linux)` / `Windows` → empty Vec).
- `test_check_ios_unknown_platform_returns_unknown_checks` (`HostPlatform::Unknown` → two `Unknown`).
- `#[cfg(target_os = "macos")] test_check_ios_macos_returns_two_components` (asserts exactly one
  `XcodeTools` + one `CocoaPods`, regardless of install state).
- A parser test for the `xcode-select -p` CLT-vs-Xcode.app discrimination (feed both path shapes to a
  pure helper and assert Missing vs Ok classification) — keeps the path-shape logic unit-testable without
  a real Xcode install.

### Notes

- **Report raw `Missing`, not `Partial`.** The non-blocking cap is the app's job (Task 03). The daemon
  reports ground truth so the doctor list and any strict consumer keep the real status.
- **Do not add fields to `ToolchainReport`** — the two component slots carry the status.
- **Do not modify `prerequisites.rs`.** The CLT/CocoaPods detection there is a separate, legitimate
  signal (command-line tools). Phase 4's `XcodeTools` is full-Xcode and lives only in `ios.rs`.
- **macOS-only sub-probes can't be exercised on Linux CI** — gate the install-state-dependent assertions
  behind `#[cfg(target_os = "macos")]` and keep the cross-host tests (empty Vec off-mac, pure path
  parser) running everywhere.
- This task is the sole owner of the `ComponentKind` enum change; Tasks 02–04 build on the merged result.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/types.rs` | Added `XcodeTools` + `CocoaPods` variants to `ComponentKind`, two `Display` arms, two assertions in `test_component_kind_display` |
| `crates/fdemon-daemon/src/toolchain/checks/ios.rs` | **NEW** — `check_ios` probe, `probe_xcode_tools`, `probe_cocoapods`, `is_full_xcode_path` classifier, 7 unit tests |
| `crates/fdemon-daemon/src/toolchain/checks/mod.rs` | Added `mod ios; pub use ios::check_ios;` |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | Added `check_ios` to `tokio::join!` fan-out; `components.extend(ios_checks)` after base 10; macOS-gated presence assertions in test |
| `crates/fdemon-app/src/install_wizard/state.rs` | No-op stub arm `ComponentKind::XcodeTools \| ComponentKind::CocoaPods => {}` in `build_steps` match |

### Notable Decisions/Tradeoffs

1. **`is_full_xcode_path` handles versioned app bundles**: The original task spec mentioned `Xcode.app/Contents/Developer` but real installations can use names like `Xcode_15.2.app`. Implemented a more robust classifier that checks for `.app/Contents/Developer` with a bundle name starting with `Xcode`, so versioned bundles are correctly identified as full Xcode installs.

2. **`XcodeSelectResult::FullXcode` is unit type**: The path is only needed for the CLT-only diagnostic message; once confirmed as full Xcode, `xcodebuild -version` provides the version detail. Changed to `FullXcode` (no payload) to eliminate the dead_code warning.

3. **Pre-existing test failure**: `test_run_preflight_nonexistent_sdk_path_does_not_panic` was already failing before this task (confirmed by stash/restore). This host has Flutter on PATH that `find_flutter_sdk` resolves even when a nonexistent explicit path is given plus a blocked FVM cache. Not introduced by this task.

### Testing Performed

- `cargo build --workspace` — Passed (no warnings)
- `cargo test -p fdemon-daemon --lib toolchain::checks::ios` — Passed (7/7 tests)
- `cargo test -p fdemon-daemon --lib toolchain` — 416 passed, 1 pre-existing failure (not introduced)
- `cargo test --workspace --lib` — 1194 passed, 1 pre-existing failure (not introduced)
- `cargo fmt --all` — Passed (clean)
- `cargo clippy --workspace -- -D warnings` — Passed (clean)

### Risks/Limitations

1. **Pre-existing test failure**: `test_run_preflight_nonexistent_sdk_path_does_not_panic` fails on this Linux dev host because Flutter is found via PATH strategies even when the explicit path is nonexistent and FVM cache is blocked. This was pre-existing before this task and is not related to the iOS probe changes.

2. **macOS tests not exercised on Linux CI**: The `test_check_ios_macos_returns_two_components` and macOS-gated preflight presence assertions are `#[cfg(target_os = "macos")]` — they run only on macOS. The cross-host tests (empty Vec on Linux/Windows, Unknown checks, path classifier) run everywhere.
