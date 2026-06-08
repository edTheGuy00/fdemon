## Task: Windows broadcast — single shared script constant + non-blocking invocation (F21, F22)

**Severity:** LOW (F21 test-quality, F22 liveness)

**Objective**: Make the `WM_SETTINGCHANGE` broadcast tests actually guard the shipped
PowerShell script, and ensure the best-effort broadcast can never block the wizard
thread indefinitely.

**Depends on**: — (only file is `path_config.rs`; file-disjoint from all other tasks)

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs`

### Details & Fixes

**F21 (LOW) — broadcast tests assert on a duplicated literal.** The PowerShell script
lives as a `#[cfg(target_os = "windows")]` local in `broadcast_wm_settingchange`
(`path_config.rs:811-821`), so on Linux CI it is not even compiled. The shape tests
(`:1843-1902`) re-type a byte-identical heredoc into their own `let script` and assert
on **that copy** — any divergence in the shipped script is undetectable on non-Windows
CI.
**Fix:** hoist the script into a module-level, **non**-cfg-gated constant so it compiles
and is referenceable on every platform:
```rust
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const BROADCAST_WM_SETTINGCHANGE_SCRIPT: &str = r#"Add-Type @"..."#;
```
Have `broadcast_wm_settingchange` reference it inside its
`#[cfg(target_os = "windows")]` block, and have both shape tests assert against
`BROADCAST_WM_SETTINGCHANGE_SCRIPT` instead of a re-typed heredoc — so the tests guard
the actually-shipped script on Linux CI.

**F22 (LOW) — broadcast `.output()` has no Rust-side timeout.** The broadcast spawns
PowerShell via `Command::new("powershell")...output()` (`path_config.rs:824-826`);
`.output()` blocks until the process exits. The `5000` ms `SMTO_ABORTIFHUNG` bound
applies only to the in-process `SendMessageTimeout` Win32 call inside the script, not
to the powershell.exe lifetime. If PowerShell itself stalls (e.g. `Add-Type` C#
compilation, AV interception), the wizard thread blocks. The registry write has already
committed before the broadcast, and the broadcast is best-effort (errors ignored).
**Fix:** detach instead of joining — replace `.output()` with
`.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()` and drop the
returned `Child` (deliberately not waiting), so the wizard thread never blocks on
PowerShell. (Closing the redirected stdio handles prevents the child blocking on
inherited pipes.) If best-effort failure logging is still wanted, keep `.spawn()` and
move `wait()` onto a short-lived detached thread with an ~8s watchdog that kills the
child — but never `.output()` on the calling thread.

### Acceptance Criteria

1. The PowerShell broadcast script exists as a single module-level constant referenced
   by both the production code and the tests; the shape tests assert against that
   constant (F21).
2. The broadcast invocation does not block the calling thread on PowerShell process
   exit — it is spawned detached (or watchdog-bounded), with redirected/null stdio
   (F22).
3. The broadcast remains best-effort: a spawn/broadcast failure is swallowed and the
   `PathConfigOutcome::Written` success path is unaffected.
4. Non-Windows builds still compile (the constant is not cfg-gated;
   `cargo check --workspace` passes on Linux).

### Testing

```rust
// path_config.rs test module
// - UPDATE windows_broadcast_script_contains_wm_settingchange_constant and the second
//   shape test to assert on BROADCAST_WM_SETTINGCHANGE_SCRIPT (the shared const),
//   not a re-typed heredoc — so they fail if the shipped script drifts.
// - (Windows-only / cfg-gated) optionally assert the broadcast path spawns detached;
//   the detach itself is hard to unit-test cross-platform, so a doc-comment + the
//   constant assertion is the practical guard.
```

### Notes

- Production runtime behaviour of the script is correct today; F21 is purely
  test-quality (the assertion targets a copy, not the shipped script).
- File-disjoint from every other followup task — safe to run in Wave 1.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/path_config.rs` | Hoisted PowerShell script to module-level `BROADCAST_WM_SETTINGCHANGE_SCRIPT` const (non-cfg-gated, with `#[cfg_attr(not(target_os = "windows"), allow(dead_code))]`); replaced `.output()` with detached `.spawn()` + null stdio; updated both shape tests to assert against the shared const |

### Notable Decisions/Tradeoffs

1. **`allow(dead_code)` on non-Windows**: Used `#[cfg_attr(not(target_os = "windows"), allow(dead_code))]` rather than unconditionally allowing dead_code. The production code path inside `broadcast_wm_settingchange` is `#[cfg(target_os = "windows")]`, so on non-Windows the const is only referenced from tests. The `cfg_attr` suppresses the lint without masking real dead-code issues on other items.

2. **Simple spawn-and-drop vs. watchdog thread**: Chose the simpler spawn-and-drop approach (the task offered a watchdog-thread alternative). The registry write has already committed before the broadcast call, and the broadcast is best-effort; there's no value in monitoring or killing the child, and the added complexity of a watchdog thread is unwarranted for a best-effort operation.

3. **Second test refactor**: The second shape test (`windows_path_set_and_broadcast_both_use_out_of_band_values`) was updated to assert against `BROADCAST_WM_SETTINGCHANGE_SCRIPT` rather than the locally-defined `broadcast_lp_arg` literal, making it guard the shipped script on Linux CI consistent with the first test.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all test suites green)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Windows runtime only**: The spawn-detach behaviour cannot be unit-tested cross-platform. The shape tests (which run on Linux CI) guard the script content; the detach behaviour is validated by code review.
