## Task: Daemon — harden the iOS/macOS Xcode probe (kill_on_drop + misclassification + real license/first-launch/simctl gates)

**Objective**: Harden `checks/ios.rs` to address three Phase 4 review findings as one compiling, test-green
unit, all within the single file:

- **H1 (HIGH/blocking):** add `.kill_on_drop(true)` to every process spawn so a hung `xcodebuild`/`pod`/`xcrun`
  is killed on `PROBE_TIMEOUT` instead of orphaned.
- **M1 (MAJOR):** stop misclassifying a non-zero `xcode-select -p` exit (no developer tools at all) as
  `CltOnly(empty)` — which renders the false "Only Xcode Command Line Tools found ()". Plus fold **L1**:
  `strip_and_truncate` the genuine CLT path.
- **Md1 (MEDIUM):** *implement* the read-only license, first-launch, and simctl gates the docs already claim,
  so `XcodeTools = Ok` genuinely means *usable* (kills the false-positive Ok from a license-unaccepted Xcode).

**Depends on**: Phase 4 (merged). Daemon-only; compiles and `cargo test -p fdemon-daemon` green standalone.

**Agent:** implementor

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/ios.rs` — `kill_on_drop` on all spawns; fix
  `probe_xcode_select_path` non-zero arm; add `probe_xcode_license` / `probe_xcode_first_launch` /
  `probe_simctl` gates; a pure `classify_xcode_gates(...)` helper; update `probe_xcode_tools` orchestration;
  update the module + fn doc-comments; add unit tests.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `PROBE_TIMEOUT`, `strip_and_truncate`, `MAX_DETAIL_LEN`.
- `crates/fdemon-daemon/src/toolchain/doctor.rs` and `toolchain/process_stream.rs` — the established
  `.kill_on_drop(true)` convention to mirror.
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentStatus` variants (`Ok`, `Missing`, `Error`,
  `Unknown`).

### Details

> Line numbers drift — locate by symbol/variant/test-name.

#### 1. H1 — `kill_on_drop(true)` on every spawn

Every `Command::new(...)` builder in `ios.rs` currently calls `.output()` inside
`tokio::time::timeout(PROBE_TIMEOUT, …)` **without** `.kill_on_drop(true)`. On timeout the `output()` future
is dropped and Tokio **detaches** (does not SIGKILL) the child. Add `.kill_on_drop(true)` to the builder for:
`xcode-select`, `xcodebuild -version`, `pod --version`, **and** the three new gates below. This matches
`doctor.rs:57` and `process_stream.rs:78,190`.

#### 2. M1 (+ L1) — fix the `xcode-select -p` non-zero-exit classification

In `probe_xcode_select_path`, the arm:

```rust
Ok(Ok(_)) => XcodeSelectResult::CltOnly(String::new()),   // BUG
```

fires when `xcode-select -p` runs but exits non-zero — which on macOS means **no active developer tools at
all**, not CLT. It currently produces "Only Xcode Command Line Tools found (). Install full Xcode…".

- Route this arm to a state that yields an **accurate** message. Either reuse `XcodeSelectResult::NotFound`
  (the caller maps it to `Missing` with "xcode-select reports no active developer directory — Xcode or CLT
  not installed"), or add a dedicated `NoActiveTools` variant with that message. Do **not** label it CLT.
- **L1 fold:** where a *genuine* CLT path is returned (`Ok(Ok(output))` success branch →
  `XcodeSelectResult::CltOnly(path)`), wrap the path in `strip_and_truncate(&path)` for consistency with every
  other external-output string in the file.

#### 3. Md1 — implement the license / first-launch / simctl gates

Add three read-only, non-interactive, no-sudo probes, each `PROBE_TIMEOUT`-wrapped + `kill_on_drop`. **Run all
three** — do not short-circuit (a Mac can have a valid license but incomplete first-launch, or vice-versa):

```rust
/// `xcodebuild -license check` → exit 0 = accepted, non-zero = not accepted.
async fn probe_xcode_license() -> GateResult { /* … */ }

/// `xcodebuild -checkFirstLaunchStatus` → exit 0 = components present, non-zero = run -runFirstLaunch.
async fn probe_xcode_first_launch() -> GateResult { /* … */ }

/// `xcrun simctl list devices booted` → exit 0 = simctl reachable. (Filtered output avoids the
/// known `simctl list` hang; still wrap in PROBE_TIMEOUT.)
async fn probe_simctl() -> GateResult { /* … */ }
```

Suggested `GateResult` shape (kept private to the module):

```rust
/// Outcome of one read-only Xcode usability gate.
enum GateResult { Pass, Fail, Unknown }   // Unknown = timed out / spawn error
```

Rework `probe_xcode_tools` so that **after** `FullXcode` + a successful `xcodebuild -version` (capture the
version string for `detail`), it runs the three gates and folds them through a **pure** classifier:

```rust
/// Pure: combine the version detail with the three gate outcomes into a final XcodeTools check.
/// `Ok` iff license + first_launch + simctl all Pass. Otherwise `Missing` with a detail naming the
/// first failed (or Unknown) gate and its remediation. Unit-tested on every host (no real Xcode needed).
fn classify_xcode_gates(
    version_detail: &str,
    license: GateResult,
    first_launch: GateResult,
    simctl: GateResult,
) -> ComponentCheck
```

Classifier rules (see TASKS.md table; sources below):
- all three `Pass` → `XcodeTools = Ok`, `detail = version_detail`.
- license `Fail` → `Missing`, detail e.g. `"<version> — license not accepted; run sudo xcodebuild -license accept"`.
- first_launch `Fail` → `Missing`, detail `"<version> — first-launch incomplete; run sudo xcodebuild -runFirstLaunch"`.
- simctl `Fail` → `Missing`, detail `"<version> — simctl unreachable; run sudo xcodebuild -runFirstLaunch"`.
- any gate `Unknown` (timeout/spawn error) → `Missing` (or `Error` if you prefer) with a "could not verify
  <gate>" detail. Prefer surfacing the most actionable failing gate when several fail; document the precedence
  you pick.

> **Status encoding:** report `ComponentStatus::Missing` for present-but-misconfigured (not a new
> `ComponentStatus` variant). The app's existing `Missing → Partial` cap turns it into a non-blocking
> `Partial` leaf, and the leaf's existing guided command
> (`xcode-select -s … && xcodebuild -runFirstLaunch && xcodebuild -license accept`) already remediates all
> three gates. Distinguishing present-but-broken from absent at the `ComponentStatus` level is a **deferred**
> nitpick — do not add a variant here.

#### 4. Doc-comment update (same file)

Update the `//!` module header and `probe_xcode_tools` doc to describe the **real** five-gate sequence
(`xcode-select -p` → `xcodebuild -version` → `-license check` → `-checkFirstLaunchStatus` →
`simctl list devices booted`) and that `Ok` requires all to pass. Remove the prior wording that implied
license/simctl were merely "inferred". (Core-doc `ARCHITECTURE.md` is Task 03.)

### Acceptance Criteria

1. **H1:** every `Command` builder in `ios.rs` sets `.kill_on_drop(true)`. (Grep: count of `kill_on_drop`
   == count of `Command::new` in the file.)
2. **M1/L1:** a non-zero `xcode-select -p` exit no longer yields a `CltOnly` classification or the
   "Only Xcode Command Line Tools found ()" string; a genuine CLT path is `strip_and_truncate`'d.
3. **Md1:** `XcodeTools = Ok` is returned **only** when full Xcode + `xcodebuild -version` + all three gates
   pass; a license-unaccepted / first-launch-pending / simctl-broken Xcode returns `Missing` with a `detail`
   naming the specific failed gate and its sudo remediation. All three gates run regardless of each other's
   outcome.
4. Each new gate is `PROBE_TIMEOUT`-wrapped and `kill_on_drop`; none panics; non-macOS still returns an empty
   `Vec`; `HostPlatform::Unknown` still returns two `Unknown` checks.
5. `classify_xcode_gates` is a pure function with Linux-runnable unit tests covering: all-pass→Ok,
   each-single-gate-fail→Missing(correct detail), and an Unknown-gate case.
6. `cargo test -p fdemon-daemon --lib toolchain` green; `cargo test --workspace --lib` green; `cargo fmt --all`
   + `cargo clippy --workspace --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-daemon --lib toolchain
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

New / updated tests in `ios.rs`:
- `test_classify_xcode_gates_all_pass_is_ok` — all `Pass` → `Ok`, detail == version.
- `test_classify_xcode_gates_license_fail_is_missing_with_license_detail`.
- `test_classify_xcode_gates_first_launch_fail_is_missing_with_runfirstlaunch_detail`.
- `test_classify_xcode_gates_simctl_fail_is_missing_with_simctl_detail`.
- `test_classify_xcode_gates_unknown_gate_is_non_ok` — a timed-out gate never yields `Ok`.
- Keep `test_is_full_xcode_path_*` and the non-macOS / Unknown-platform tests.
- (macOS-only, `#[cfg(target_os = "macos")]`) the existing presence smoke test still holds — it asserts two
  components + non-empty details, which remains true regardless of gate outcomes.

### Notes

- **Read-only & non-interactive is mandatory.** Use `xcodebuild -license check` (NOT bare `xcodebuild
  -license`, which opens an interactive pager). All gates must avoid sudo and must not mutate state. Keep
  `stdin(Stdio::null())` on every spawn.
- **Run all gates.** Mirror Flutter's own validator, which reports every failing condition in one pass.
- **`xcodebuild -downloadPlatform iOS` is Xcode 16+ only** — do not add it as a gate here; the iOS leaf's
  guided command already surfaces it. (Conditionally gating that guided command on the detected version is a
  deferred nitpick in `state.rs`, not this task.)
- **Exit code 69** on an `xcrun` call specifically indicates an unaccepted license — you may use it to enrich
  the simctl-gate detail, but the dedicated `-license check` gate is the authoritative license signal.
- **Sources (external_researcher, 2026-06-10):** `man xcodebuild` (keith.github.io mirror) — `-license check`,
  `-checkFirstLaunchStatus`, `-runFirstLaunch` semantics; macops.ca "Deploying Xcode" — license plist keys;
  Workbrew Homebrew-Xcode-license post — `-license check` / `-checkFirstLaunchStatus` automation pattern;
  Flutter `xcode.dart` (`isSimctlInstalled` → `xcrun simctl list devices booted`); Flutter issues #141349,
  #16427 — exit-69 license behavior. `-checkFirstLaunchStatus`: exit 0 = up-to-date, non-zero = run
  `-runFirstLaunch`; both read-only, no sudo.
- This task is daemon-only and write-disjoint from Task 02 (`state.rs`); they run in parallel worktrees.

---

## Completion Summary

**Status:** Not Started
