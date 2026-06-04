## Task: Android install — license-acceptance verification (m1) + cleanups (m5, n1, n3)

**Objective**: Make `sdkmanager --licenses` acceptance verifiable instead of blind, and
clean up the surrounding dead code / fragile constructs. All changes are confined to one
file, `android_install.rs`. These findings are grouped because m1's resolution determines
m5's disposition of the `log_lines` accumulator.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Background (verified)

All in `crates/fdemon-daemon/src/toolchain/android_install.rs`:

- **m1 (license verification):** the license block (~222-242) pipes `"y\n"` ×
  `LICENSE_YES_COUNT` (=20) to `sdkmanager --licenses` and trusts only the exit code — no
  check that licenses were actually accepted. The `log_lines` accumulator (~223) was
  *clearly written for exactly this scan* but never wired. The `flutter doctor
  --android-licenses` fallback named in `phase-3/TASKS.md:209` is an "and/or" option, **not
  a requirement**, and is not built.
  → **Recommended approach (lowest risk, fully self-contained): Option A — scan the streamed
  output** for a success marker (e.g. a line matching "All SDK package licenses accepted" /
  "Accepted N of N") and **log a `warn!`** when acceptance cannot be confirmed (do not hard
  fail — exit code remains the primary signal). Wire `log_lines` (or the live stream) into
  the scan.
- **m5 (cleanups):** `log_lines` + its `line.clone()` (~223, ~231) — keep+wire for m1
  rather than delete; and the gratuitous `pkg_refs` + `pkg_refs.clone()` into `install_args`
  (~248-253) — build `install_args` directly from `packages.iter()`.
- **n1 (temp dir):** `.fdemon-android-tmp-{pid}` (~114-122) can reuse a stale dir on PID
  recycling. Add removal of any pre-existing dir before `create_dir_all` (or use a
  collision-resistant suffix). The cleanup at ~127-134 already runs on success and failure.
- **n3 (path join):** `jdk_bin = format!("{}/bin", java_home)` (~204) bypasses `Path::join`
  (produces `//bin` on a trailing slash). Use `Path::new(java_home).join("bin")`.

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/android_install.rs` only.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/process_stream.rs` (`run_streaming_with_input` shape).
- `crates/fdemon-daemon/src/toolchain/types.rs` (`AndroidInstallTarget`/`Outcome` — unchanged).

### Acceptance Criteria

1. (m1) After `sdkmanager --licenses` runs, the streamed output is scanned for a success
   marker; when not found, a `warn!` is logged (and surfaced as an `InstallEvent::Log`
   line if consistent with existing behavior). Exit-code handling is unchanged. `log_lines`
   is either wired into the scan or replaced by scanning the live stream — no dead
   accumulator remains.
2. (m5) `pkg_refs.clone()` is gone; `install_args` is built directly. No gratuitous clones.
3. (n1) A pre-existing temp dir cannot cause a stale-content install (removed before use, or
   uniquely suffixed). Behavior is strictly safer; cleanup still runs on success and failure.
4. (n3) `jdk_bin` uses `Path::join`; no `//bin` on trailing-slash `java_home`.
5. Tests: a unit test using a mock/stub line stream asserts the license scan logs a warning
   when the success marker is absent and does **not** warn when present. (If exercising the
   real `sdkmanager` is infeasible, factor the marker-detection into a pure
   `fn licenses_confirmed(lines: &[String]) -> bool` and test that directly.)
6. `cargo fmt`/`check`/`test`/`clippy -D warnings` pass workspace-wide.

### Notes

- Keep the scope to **Option A (output scan)**. Do **not** implement the
  `flutter doctor --android-licenses` fallback in this task — that is a larger, separately
  scoped change (and Flutter may not be present). If desired later, it becomes its own task.
- No public API / struct changes. `run_streaming_with_input` signature is unchanged.
- The marker string should be defined as a named `const` with a comment noting it depends on
  `sdkmanager` output format (so future drift is greppable).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | m1: added `LICENSES_ACCEPTED_MARKER` const + `licenses_confirmed()` pure fn + scan+warn block after `sdkmanager --licenses`; m5: removed `pkg_refs`/`pkg_refs.clone()`, build `install_args` directly from `packages.iter()`; n1: remove pre-existing stale temp dir before `create_dir_all`; n3: replaced `format!("{}/bin", java_home)` with `Path::new(java_home).join("bin")`; added 4 unit tests for `licenses_confirmed` |

### Notable Decisions/Tradeoffs

1. **`licenses_confirmed` visibility**: Marked `pub(crate)` rather than private so it is accessible to any future sibling test modules without moving it to a separate test helper file. The pure-function factoring is the task-specified approach.
2. **Warn + surface as `InstallEvent::Log`**: When the marker is absent, a `tracing::warn!` is emitted AND an `InstallEvent::Log("[fdemon warn] ...")` line is forwarded so the TUI log tail shows it to the user without requiring a separate event variant.
3. **`log_lines` retained**: As specified in m1, `log_lines` is wired into `licenses_confirmed` rather than deleted — it was clearly intended for this scan.
4. **Path::join for jdk_bin (n3)**: Used `std::path::Path::new(java_home.as_str()).join("bin").to_string_lossy().into_owned()` to correctly handle trailing-slash paths. The `to_string_lossy` step is necessary because the result must be serialized into an environment variable string.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-daemon --lib toolchain::android_install` — 11 passed (includes 4 new `licenses_confirmed` tests)
- `cargo test --workspace --lib` — 1422 passed, 0 failed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Marker string fragility**: `LICENSES_ACCEPTED_MARKER = "All SDK package licenses accepted"` is matched against sdkmanager output which can vary between releases or when licenses are already accepted. The const has a doc comment noting the format dependency and greppability. A missing marker only logs a warning — the exit code remains the primary failure signal.
2. **Already-licensed SDKs**: When all licenses are already accepted, sdkmanager may print a different message ("Reuse the existing license accepted response") and the marker may not appear; this will trigger a harmless warning. Future refinement could also accept the "already accepted" pattern.
