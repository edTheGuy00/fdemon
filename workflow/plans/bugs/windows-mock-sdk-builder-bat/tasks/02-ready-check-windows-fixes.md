# Task 02: ready_check.rs Windows fixes (B2 + L1)

**Severity:** BLOCKER + LATENT — bundled because both touch the same file.

**Estimated Time:** 0.5 hours

## Objective

Make every test in `crates/fdemon-app/src/actions/ready_check.rs` Windows-clean. There are two distinct changes, both inside the `#[cfg(test)] mod tests` block:

1. **B2 (BLOCKER):** `test_command_check_succeeds_on_true` and `test_command_check_timeout_on_false` invoke `Command::new("true")` / `Command::new("false")`. These are POSIX shell builtins — neither has a Windows equivalent on a stock GitHub Windows runner (no `true.exe` on PATH). The audit found these are currently masked by Task 01's earlier failure but will surface in the next CI run once 01 lands.
2. **L1 (LATENT):** `test_http_check_non_200_retries` uses the same accept-once-then-drop-socket pattern that broke `test_http_check_success` on Windows. The 503 test happens to still pass today because the assertion is `TimedOut` (which is reached either way), but on Windows the test is exercising the connection-error path instead of the intended 503-retry path. Mirror the drain + shutdown fix already applied to `test_http_check_success`.

**Depends on:** None

## Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/ready_check.rs` — gate two tests with `#[cfg(unix)]`; apply read+shutdown to one mock server

**Files Read (Dependencies):**
- The same file's `test_http_check_success` (post-fix) is the reference for the L1 drain pattern.

## Details

### B2 — gate `true` / `false` tests

For `test_command_check_succeeds_on_true` (currently around line 492) and `test_command_check_timeout_on_false` (currently around line 508), add a `#[cfg(unix)]` attribute on the test function so they don't run on Windows. Both tests verify behaviour against POSIX shell builtins; Windows verification would need cmd.exe-based equivalents (`cmd /c exit 0`), which is out of scope for this batched fix.

Pattern to apply (per test):

```rust
#[tokio::test]
#[cfg(unix)]  // `true` / `false` are POSIX shell builtins, not native on Windows
async fn test_command_check_succeeds_on_true() { ... }
```

(The same for `test_command_check_timeout_on_false`.)

### L1 — drain HTTP request and shutdown socket in `test_http_check_non_200_retries`

The current mock server is at `crates/fdemon-app/src/actions/ready_check.rs` lines 412–420:

```rust
tokio::spawn(async move {
    loop {
        if let Ok((mut sock, _)) = listener.accept().await {
            let _ = sock
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\n\r\n")
                .await;
        }
    }
});
```

Replace it with the same pattern already in `test_http_check_success` (drain the request before responding, then shutdown):

```rust
tokio::spawn(async move {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    loop {
        if let Ok((mut sock, _)) = listener.accept().await {
            // Drain the incoming request before responding. Windows TCP RSTs
            // sockets closed with un-read data, which would race the client's
            // response read.
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let _ = sock
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\n\r\n")
                .await;
            let _ = sock.shutdown().await;
        }
    }
});
```

The `use tokio::io::{AsyncReadExt, AsyncWriteExt}` import inside the spawned async block is fine — `AsyncWriteExt` is already in scope at the test-mod level (`use tokio::io::AsyncWriteExt;` at the top of the test); just add `AsyncReadExt` to that existing use.

## Acceptance Criteria

- [ ] `test_command_check_succeeds_on_true` carries `#[cfg(unix)]`.
- [ ] `test_command_check_timeout_on_false` carries `#[cfg(unix)]`.
- [ ] `test_http_check_non_200_retries`'s spawned mock server reads the request bytes, writes the 503, then `sock.shutdown().await`s before looping.
- [ ] `cargo test -p fdemon-app actions::ready_check` passes locally on macOS.
- [ ] `cargo clippy -p fdemon-app --all-targets -- -D warnings` clean.
- [ ] `cargo fmt --all -- --check` clean.
- [ ] No production code changes.
- [ ] No other tests in `ready_check.rs` are touched.

## Out of Scope

- Rewriting the gated tests with Windows-equivalent commands. That would require new fixtures and is not warranted for B2's coverage scope.
- Adding integration-level coverage for the gated paths on Windows. Out of scope for this batched fix; the production `Command::new(<user-supplied>)` code path is platform-agnostic and already exercised on Unix.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/ready_check.rs` | Added `#[cfg(unix)]` to `test_command_check_succeeds_on_true` and `test_command_check_timeout_on_false`; added request drain + `shutdown()` to `test_http_check_non_200_retries` mock server; added `AsyncReadExt` to the `use tokio::io` import in that test |

### Notable Decisions/Tradeoffs

1. **Import scope**: Added `AsyncReadExt` via a local `use tokio::io::{AsyncReadExt, AsyncWriteExt}` inside the `test_http_check_non_200_retries` test function rather than modifying any module-level import, matching the pattern in `test_http_check_success` (which already has `use tokio::io::{AsyncReadExt, AsyncWriteExt}` inline).
2. **No production code changes**: All changes are confined to the `#[cfg(test)] mod tests` block, exactly as specified.

### Testing Performed

- `cargo test -p fdemon-app actions::ready_check` — Passed (18 tests, 0 failed)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed (no warnings)
- `cargo fmt --all -- --check` — Passed (no formatting issues)

### Risks/Limitations

1. **Windows coverage gap**: The `true`/`false` command tests remain ungated on Windows but this is explicitly out of scope per the task. The production `run_command_check` code path is platform-agnostic.
