## Task: Fix HTTP Health Check Buffer Robustness

**Objective**: Replace the single raw `read()` call with a 256-byte buffer in `try_http_get` with `BufReader::read_line()` to reliably read the complete HTTP status line regardless of TCP segmentation.

**Depends on**: None

**Severity**: Major

### Scope

- `crates/fdemon-app/src/actions/ready_check.rs`: Modify `try_http_get` (lines 129-155)

### Details

#### The Problem

`try_http_get` (line 141) does a single `stream.read(&mut buf)` with a 256-byte buffer. On slow or loaded servers, the first TCP segment may deliver fewer bytes than the complete status line (e.g., just `"HTTP/"` or even 1 byte), causing the status code parsing to fail and misclassifying a healthy 2xx response as a failure. This forces unnecessary retry cycles.

#### The Fix

Replace the raw read with `BufReader::read_line()`:

```rust
async fn try_http_get(addr: &str, host: &str, path: &str) -> std::io::Result<bool> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let mut stream = TcpStream::connect(addr).await?;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );
    stream.write_all(request.as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).await?;

    // Parse "HTTP/1.x 2xx ..."
    if let Some(code_str) = status_line.split_whitespace().nth(1) {
        if let Ok(code) = code_str.parse::<u16>() {
            return Ok((200..300).contains(&code));
        }
    }

    Ok(false)
}
```

### Acceptance Criteria

1. `try_http_get` reads the complete status line via `BufReader::read_line()`
2. No raw `read()` with fixed-size buffer remains
3. Existing HTTP check tests still pass
4. The outer `tokio::time::timeout` in `run_http_check` still caps overall attempt duration (prevents `read_line` from blocking indefinitely on a server that sends data without a newline)

### Testing

The existing `test_http_check_success`, `test_http_check_non_200_retries`, and `test_http_check_connection_refused` tests exercise `try_http_get` indirectly through `run_ready_check`. These should continue to pass.

### Notes

- `BufReader::read_line()` reads until `\n` or EOF — HTTP/1.x status lines are terminated with `\r\n`, so this correctly captures the full line
- The outer `tokio::time::timeout(remaining, try_http_get(...))` in `run_http_check` (line 102) already protects against indefinite blocking, so no additional timeout is needed inside `try_http_get`
- Import changes: replace `AsyncReadExt` with `AsyncBufReadExt`, add `BufReader`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/ready_check.rs` | Replaced raw `stream.read(&mut buf)` with `BufReader::read_line()`; updated imports from `AsyncReadExt` to `AsyncBufReadExt` + `BufReader`; added doc comment noting outer timeout provides protection; added three new HTTP check tests: `test_http_check_success`, `test_http_check_non_200_retries`, `test_http_check_connection_refused` |

### Notable Decisions/Tradeoffs

1. **Added missing HTTP tests**: The task notes referenced three HTTP check tests that did not yet exist in the file. They were added alongside the implementation change so there is concrete coverage of `try_http_get` via `run_ready_check`. Each test spins up a real `tokio::net::TcpListener` to exercise the actual TCP path.
2. **No internal timeout added**: The outer `tokio::time::timeout(remaining, try_http_get(...))` in `run_http_check` already bounds `read_line` if a server sends data without a newline, so no additional timeout was introduced inside `try_http_get`.

### Testing Performed

- `cargo test -p fdemon-app ready_check` - Passed (44 tests, 0 failed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt -p fdemon-app && cargo check -p fdemon-app` - Passed

### Risks/Limitations

1. **EOF with no newline**: `read_line` reads until `\n` or EOF. If a server closes the connection before sending `\r\n`, the accumulated partial line is still parsed — this is safe because `split_whitespace().nth(1)` returns `None` on a truncated line, causing `try_http_get` to return `Ok(false)` and trigger a retry, which is the correct behaviour.
