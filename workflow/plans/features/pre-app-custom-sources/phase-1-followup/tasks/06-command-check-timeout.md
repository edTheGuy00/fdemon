## Task: Align `run_command_check` Timeout Pattern with Siblings

**Objective**: Change `run_command_check` to use the `remaining.is_zero()` pattern with `tokio::time::timeout` wrapping the subprocess, matching `run_http_check` and `run_tcp_check`.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/actions/ready_check.rs`: Modify `run_command_check` (lines 228-274)

### Details

#### Current Code (Inconsistent)

`run_command_check` uses `start.elapsed() >= timeout` at the top of the loop and does NOT wrap the subprocess in `tokio::time::timeout`:

```rust
// line 239-241
if start.elapsed() >= timeout {
    return ReadyCheckResult::TimedOut(start.elapsed());
}
// line 244 — no timeout wrapper
match tokio::process::Command::new(command)
    .status()
    .await
```

This means a hung subprocess can block past the configured deadline.

#### Sibling Pattern (HTTP/TCP)

```rust
let remaining = timeout.saturating_sub(start.elapsed());
if remaining.is_zero() {
    return ReadyCheckResult::TimedOut(start.elapsed());
}
match tokio::time::timeout(remaining, /* async op */).await {
    Err(_) => return ReadyCheckResult::TimedOut(start.elapsed()),
    ...
}
```

#### Fix

Apply the same pattern to `run_command_check`:

```rust
loop {
    let remaining = timeout.saturating_sub(start.elapsed());
    if remaining.is_zero() {
        return ReadyCheckResult::TimedOut(start.elapsed());
    }

    match tokio::time::timeout(
        remaining,
        tokio::process::Command::new(command)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status(),
    )
    .await
    {
        Ok(Ok(status)) if status.success() => {
            return ReadyCheckResult::Ready(start.elapsed());
        }
        Ok(Ok(_)) | Ok(Err(_)) => { /* non-zero exit or spawn error — retry */ }
        Err(_) => {
            return ReadyCheckResult::TimedOut(start.elapsed());
        }
    }

    let remaining = timeout.saturating_sub(start.elapsed());
    tokio::time::sleep(interval.min(remaining)).await;
}
```

### Acceptance Criteria

1. `run_command_check` uses `remaining.is_zero()` after `saturating_sub` for timeout guard
2. Subprocess `.status()` is wrapped in `tokio::time::timeout(remaining, ...)`
3. A hung subprocess can no longer block past the configured deadline
4. Existing command check tests pass

### Notes

- This is both a consistency fix and a correctness fix — the current code has a real (if unlikely) deadlock risk if a command hangs indefinitely
- The subprocess timeout wrapper means `Err(_)` from `tokio::time::timeout` must be handled as `TimedOut`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/ready_check.rs` | Replaced `start.elapsed() >= timeout` guard with `remaining.is_zero()` pattern; wrapped subprocess `.status()` call in `tokio::time::timeout(remaining, ...)`; added `tracing::debug!` in retry arm to keep `source_name` used (avoiding clippy unused-variable error) |

### Notable Decisions/Tradeoffs

1. **Debug logging in retry arm**: The task's example pattern used a comment `/* non-zero exit or spawn error — retry */` instead of logging. However, removing the `tracing::debug!` calls that previously existed left `source_name` unused, causing a clippy `-D warnings` failure. Added a single `tracing::debug!` in the combined retry arm (matching the sibling `run_tcp_check` style) to resolve the warning while preserving observability.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo clippy -p fdemon-app --lib -- -D warnings` - Passed
- `cargo test -p fdemon-app ready_check` - Passed (44 tests)

### Risks/Limitations

1. **None**: This is a mechanical consistency fix. The timeout behaviour is now identical to `run_http_check` and `run_tcp_check`.
