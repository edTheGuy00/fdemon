## Task: Fix single-shot mock server race in `test_http_check_success`

**Objective**: Convert the spawned mock-server task in `test_http_check_success` from a one-shot accept into an accept-loop, matching the pattern already used by `test_http_check_non_200_retries` in the same file. This eliminates the race condition that causes the test to fail on Windows.

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/ready_check.rs`: Edit `test_http_check_success` around lines 376–399.

**Files Read (Dependencies):**
- The sibling `test_http_check_non_200_retries` test (around line 412) in the same file — use as the pattern reference for the accept-loop.

### Details

#### Current shape (broken on Windows)

```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
let port = listener.local_addr().unwrap().port();
tokio::spawn(async move {
    let (mut socket, _) = listener.accept().await.unwrap();
    // read GET, write 200 OK, drop socket
});
let check = ReadyCheck::Http {
    url: format!("http://127.0.0.1:{}/health", port),
    timeout_s: 5,
    interval_ms: 100,
    ...
};
let result = run_ready_check(&check, "test", None).await;
assert!(result.is_ready());
```

#### Why it fails on Windows

`run_http_check` retries every 100ms for up to 5s (50 attempts). On Windows, `TcpStream::connect` can return success at the OS level before the spawned task reaches `accept()` — the SYN is queued by the TCP stack. The first `try_http_get` then writes the GET and tries to read a response, but the server task hasn't called `accept()` yet so the read returns empty/`Connection reset`. `try_http_get` returns `Ok(false)` (status-line parse fails). The single `accept()` is now consumed; subsequent retries get `Connection refused`. The check times out and the assertion fails.

#### Fix: accept-loop

Replace the single-accept body with the same loop shape used by `test_http_check_non_200_retries`:

```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
let port = listener.local_addr().unwrap().port();
tokio::spawn(async move {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else { break };
        // read GET, write 200 OK, drop socket
    }
});
```

Keep the rest of the test unchanged. The `tokio::spawn` task continues accepting until the test completes and the listener is dropped.

Read `test_http_check_non_200_retries` first and copy its loop structure verbatim (with the response payload changed to "200 OK"). Do not invent a new pattern.

### Acceptance Criteria

1. `test_http_check_success` uses an `accept().await` loop in the spawned task, not a single-shot accept.
2. `test_http_check_success` passes on macOS, Linux, and (verified by CI) Windows.
3. `cargo test -p fdemon-app actions::ready_check` passes on macOS — all HTTP tests still pass.
4. `cargo clippy -p fdemon-app --all-targets -- -D warnings` exits 0.
5. `cargo test -p fdemon-app` passes.
6. `cargo fmt --all -- --check` is clean.
7. No other tests in `ready_check.rs` are modified.
8. Production code in `ready_check.rs` is not modified — this is a test-only fix.

### Testing

```bash
cargo test -p fdemon-app actions::ready_check
```

All `ready_check::tests::*` cases must pass on macOS. The post-merge CI matrix verifies Windows.

If possible, simulate the Windows race locally on macOS by inserting a `tokio::time::sleep(Duration::from_millis(50))` at the start of the spawned task and re-running the test before applying the fix — the test should fail, demonstrating the race. Then apply the fix and re-run.

### Notes

- An alternative fix (`tokio::time::sleep(Duration::from_millis(10))` before creating the check) is more brittle — it papers over the timing rather than removing the dependence. Prefer the loop fix, which is the existing pattern in the file.
- If `test_http_check_non_200_retries` itself uses a slightly different fixture (e.g., a different port, different URL path, different response wording), match it exactly for `test_http_check_success` modulo the response status code. Consistency between the two tests reduces maintenance friction.
- If after this fix the test still flakes on Windows in CI, file a follow-up to investigate further (deeper into `run_http_check`'s retry/timeout logic). The most likely cause would have been resolved by the loop fix; further investigation is only needed if the loop fix is insufficient.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/ready_check.rs` | Converted `test_http_check_success` spawned task from single-shot `if let Ok` accept to `loop { if let Ok ... }` accept-loop, matching the pattern of `test_http_check_non_200_retries` |

### Notable Decisions/Tradeoffs

1. **Pattern match to sibling test**: Used the same `loop { if let Ok((mut sock, _)) = listener.accept().await { ... } }` structure as `test_http_check_non_200_retries`, not the `let Ok(...) else { break }` style shown in the task description. The task explicitly says "copy its loop structure verbatim" referring to the sibling test, which uses `if let Ok`.

### Testing Performed

- `cargo test -p fdemon-app actions::ready_check` — Passed (18 tests)
- `cargo test -p fdemon-app` — Passed (1898 tests + 1 doc-test)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed (no warnings)
- `cargo fmt --all -- --check` — Passed (no formatting changes needed)

### Risks/Limitations

1. **Windows CI verification**: The race condition fix can only be fully verified on Windows via CI. The local macOS run passes, and the loop structure eliminates the root cause (single-shot accept being exhausted before the client connects).
