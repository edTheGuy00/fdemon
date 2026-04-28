# Task 03: gate Unix-only `custom.rs` capture tests

**Severity:** BLOCKER (currently latent only because Task 01 fails earlier in the test run)

**Estimated Time:** 0.25 hours

## Objective

Ten tests in `crates/fdemon-daemon/src/native_logs/custom.rs` invoke Unix-only commands (`printf`, `echo`, `yes`, `printenv`) directly via `Command::new(...)`. On Windows these binaries do not exist on the stock GitHub runner — `Command::new("printf")` returns `Error::NotFound`, the spawned capture process never produces events, and the tests time out or assert against empty channels. The single existing `#[cfg(unix)]`-gated test in this file (`test_custom_capture_working_dir`, gated in commit `88e72eb`) is the precedent and pattern to follow.

After Task 01 lands the SDK fixture failures will clear, and these 10 tests become the next visible Windows failure. Bundle them now.

**Depends on:** None

## Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/native_logs/custom.rs` — add `#[cfg(unix)]` to ten test functions in the `#[cfg(test)] mod tests` block.

**Files Read (Dependencies):**
- The same file's `test_custom_capture_working_dir` (already gated — see commit `88e72eb`) is the pattern reference.

## Details

The audit identified these ten tests as Windows-incompatible (each uses one of `printf`, `echo`, `yes`, `printenv` directly via `Command::new`):

| Test | Uses | Approx. line |
|------|------|--------------|
| `test_custom_capture_with_echo_command` | `printf` | 293 |
| `test_custom_capture_process_exit` | `echo` | 318 |
| `test_custom_capture_shutdown` | `yes` | 340 |
| `test_custom_capture_with_env` | `printenv` | 394 |
| `test_custom_capture_tag_filtering_exclude` | `printf` | 424 |
| `test_custom_capture_tag_filtering_include` | `printf` | 467 |
| `test_create_custom_log_capture_returns_box` | `echo` | 538 |
| `test_stdout_ready_pattern_fires_on_match` | `printf` | 617 |
| `test_stdout_ready_pattern_no_match_drops_tx` | `echo` | 647 |
| `test_stdout_ready_pattern_none_no_signal` | `echo` | 673 |

Implementor: line numbers above are approximate (the file has ~1000 lines). Use grep to locate each test by name.

For each, add `#[cfg(unix)]` immediately after the existing `#[tokio::test]` attribute, mirroring the already-gated `test_custom_capture_working_dir`. Example:

```rust
#[tokio::test]
#[cfg(unix)]  // uses POSIX `printf` / `echo` / `yes` / `printenv`, not native on Windows
async fn test_custom_capture_with_echo_command() { ... }
```

The accompanying inline comment is short and explanatory; the existing gated test uses a similar comment style.

**Do not modify any other test in this file** — the audit found the remaining tests (e.g. value-only tests, tests that don't spawn subprocesses) are platform-portable.

## Acceptance Criteria

- [ ] All ten listed tests carry `#[cfg(unix)]` immediately after `#[tokio::test]`.
- [ ] No other test in `custom.rs` is modified.
- [ ] No production code changes.
- [ ] `cargo test -p fdemon-daemon native_logs::custom` passes locally on macOS (all gated tests still run on macOS).
- [ ] `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` clean.
- [ ] `cargo fmt --all -- --check` clean.

## Out of Scope

- Rewriting the gated tests with Windows-equivalent invocations (e.g. PowerShell `Write-Output`). Out of scope for this batched fix.
- Adding equivalent Windows coverage for the production code path. Production `Command::new(<user-supplied>)` is platform-agnostic; the gated tests verify only the wrapper logic, which is exercised identically on Unix.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/native_logs/custom.rs` | Added `#[cfg(unix)]` before `#[tokio::test]` on 10 test functions that invoke Unix-only commands (`printf`, `echo`, `yes`, `printenv`) |

### Notable Decisions/Tradeoffs

1. **Attribute order**: Placed `#[cfg(unix)]` before `#[tokio::test]`, matching the existing codebase convention used by the already-gated tests (`test_custom_capture_working_dir`, `test_custom_capture_stderr_does_not_produce_events`, `test_custom_capture_concurrent_shutdown`). The task's example illustration showed the reverse order, but the file's established pattern takes precedence.

2. **Tests left ungated**: `test_custom_capture_invalid_command`, `test_stdout_ready_logs_still_flow_after_match`, `test_spawn_with_readiness_none_behaves_like_spawn`, and `test_stdout_ready_invalid_regex_drops_tx` were left ungated as directed by the task (not in the audit list of 10).

### Testing Performed

- `cargo test -p fdemon-daemon native_logs::custom` — Passed (17/17 tests, all gated tests run on macOS)
- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` — Passed (no warnings)
- `cargo fmt --all -- --check` — Passed (no formatting changes needed)

### Risks/Limitations

1. **Ungated tests using Unix commands**: `test_stdout_ready_logs_still_flow_after_match` uses `printf` and `test_spawn_with_readiness_none_behaves_like_spawn` / `test_stdout_ready_invalid_regex_drops_tx` use `echo`, but were not in the audit list. If Windows CI is introduced, these may also need gating. This is outside scope of this task.
