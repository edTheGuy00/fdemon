## Task: Add Header Line Length Limit in Codec

**Objective**: Prevent unbounded memory allocation from oversized DAP header lines by adding a `MAX_HEADER_LINE_LENGTH` guard to `read_message()`.

**Depends on**: None

**Priority**: HIGH (pre-merge)

**Review Source**: REVIEW.md Issue #1 (Risks & Tradeoffs Analyzer)

### Scope

- `crates/fdemon-dap/src/protocol/codec.rs`: Add bounded header line reading

### Background

The `read_message` function at `codec.rs:51-125` reads header lines in a loop using `reader.read_line(&mut line)` (line 60). This call has **no upper bound** on line length — a malicious or malformed client can send a single header line of arbitrary size, growing heap allocation unboundedly. The existing `MAX_MESSAGE_SIZE` constant (10 MB) only guards the **body** allocation (step 3, line 99), not individual header lines.

The `BufReader` default buffer is 8 KB, but `read_line` will continue reading beyond the buffer size until it finds `\n`, allocating as needed.

### Details

#### 1. Add Constant

```rust
/// Maximum allowed length for a single DAP header line (bytes).
/// DAP headers are simple key-value pairs (e.g., "Content-Length: 42\r\n"),
/// so 4 KB is extremely generous.
const MAX_HEADER_LINE_LENGTH: usize = 4096;
```

Place next to the existing `MAX_MESSAGE_SIZE` constant (line 33).

#### 2. Add Length Check After `read_line`

In the header-reading loop (lines 58-79), add a check immediately after `read_line` succeeds:

```rust
let bytes_read = reader.read_line(&mut line).await?;

if bytes_read > MAX_HEADER_LINE_LENGTH {
    return Err(Error::protocol(format!(
        "DAP: header line exceeds maximum allowed length of {} bytes (got {})",
        MAX_HEADER_LINE_LENGTH, bytes_read
    )));
}
```

This approach is simple and uses the existing pattern. The allocation still happens (up to `MAX_HEADER_LINE_LENGTH`), but 4 KB is bounded and safe.

**Alternative (byte-by-byte):** Read byte-by-byte with a counter to avoid any allocation beyond the limit. This is more complex and unnecessary — the 4 KB allocation is negligible, and the check-after-read pattern matches how `MAX_MESSAGE_SIZE` is handled for the body.

### Acceptance Criteria

1. `MAX_HEADER_LINE_LENGTH` constant exists (4096 bytes)
2. `read_message` returns `Err(Error::protocol(...))` when a header line exceeds the limit
3. Existing tests continue to pass (all 14 codec tests)
4. New test: sending a header line >4 KB produces an error (not OOM or silent truncation)
5. `cargo test -p fdemon-dap` passes
6. `cargo clippy -p fdemon-dap -- -D warnings` clean

### Testing

Add to the existing inline `#[cfg(test)] mod tests` block in `codec.rs`:

```rust
#[tokio::test]
async fn test_read_message_oversized_header_line_rejected() {
    // Build a header line that exceeds MAX_HEADER_LINE_LENGTH (4096 bytes)
    let long_header = format!("Content-Length:{}\r\n\r\n", "9".repeat(4090));
    assert!(long_header.len() > 4096);

    let bytes = long_header.as_bytes();
    let mut reader = BufReader::new(bytes);
    let result = read_message(&mut reader).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("header line exceeds maximum"),
        "Expected header-line-too-long error, got: {}",
        err_msg
    );
}
```

### Notes

- The 4 KB limit is deliberately generous — a well-formed `Content-Length: <number>\r\n` header is at most ~30 bytes. The limit exists purely as a safety cap against malicious input.
- This is the only unbounded allocation path in the codec; the body is already guarded by `MAX_MESSAGE_SIZE`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/protocol/codec.rs` | Added `MAX_HEADER_LINE_LENGTH` constant (4096 bytes); added length check after `read_line` in the header loop; added `test_read_message_oversized_header_line_rejected` test |

### Notable Decisions/Tradeoffs

1. **Check-after-read pattern**: Kept consistent with how `MAX_MESSAGE_SIZE` guards the body — check the returned `bytes_read` value immediately after `read_line` succeeds. Up to 4 KB may be allocated before the check triggers, but 4 KB is negligible and avoids the complexity of byte-by-byte reading.

2. **Constant placement**: Placed `MAX_HEADER_LINE_LENGTH` directly below `MAX_MESSAGE_SIZE` so both allocation guards are co-located and visible together.

3. **Test input construction**: Used `"9".repeat(4090)` to produce a header line of exactly 4096 + 2 (`\r\n`) bytes, reliably exceeding the 4096-byte limit with the shortest possible digit string.

### Testing Performed

- `cargo test -p fdemon-dap` — Passed (78 tests: 77 pre-existing + 1 new)
- `cargo clippy -p fdemon-dap -- -D warnings` — Passed (no warnings)
- `cargo fmt -p fdemon-dap -- --check` — Passed (no formatting changes needed)

### Risks/Limitations

1. **Allocation before check**: The check fires after `read_line` completes, meaning up to 4096 bytes are allocated before the error is returned. This is by design (matching the existing body-size guard pattern) and the 4 KB allocation is negligible even under attack conditions.
