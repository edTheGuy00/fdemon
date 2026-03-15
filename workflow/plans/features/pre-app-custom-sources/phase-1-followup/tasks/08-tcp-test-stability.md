## Task: Fix Fragile TCP Timeout Test

**Objective**: Replace the hardcoded port 1 in `test_tcp_check_timeout_on_closed_port` with a dynamically bound-then-dropped port for deterministic behavior across all environments.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/actions/ready_check.rs`: Modify `test_tcp_check_timeout_on_closed_port` (lines 369-379)

### Details

#### Current Code

```rust
#[tokio::test]
async fn test_tcp_check_timeout_on_closed_port() {
    let check = ReadyCheck::Tcp {
        host: "127.0.0.1".to_string(),
        port: 1, // Port 1 is almost certainly not listening
        interval_ms: 100,
        timeout_s: 1,
    };
    let result = run_ready_check(&check, "test", None).await;
    assert!(matches!(result, ReadyCheckResult::TimedOut(_)));
}
```

Port 1 (tcpmux) is typically closed but could be open on some CI environments or machines with non-standard configurations.

#### Fix

Bind a random port, immediately drop the listener, then test against the now-closed port:

```rust
#[tokio::test]
async fn test_tcp_check_timeout_on_closed_port() {
    // Bind to port 0 to get an OS-assigned port, then drop the listener
    // so the port is guaranteed to be closed.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let check = ReadyCheck::Tcp {
        host: "127.0.0.1".to_string(),
        port,
        interval_ms: 100,
        timeout_s: 1,
    };
    let result = run_ready_check(&check, "test", None).await;
    assert!(matches!(result, ReadyCheckResult::TimedOut(_)));
}
```

### Acceptance Criteria

1. Test uses a dynamically allocated port that is guaranteed to be closed
2. No hardcoded port numbers
3. Test passes deterministically across environments

### Notes

- There is a theoretical TOCTOU race (another process could bind the port between `drop(listener)` and the TCP connect attempt), but this is extremely unlikely in practice and far more reliable than hardcoding port 1
- `std::net::TcpListener` (not tokio) is fine here since we immediately drop it — no async needed
