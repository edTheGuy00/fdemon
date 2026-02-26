## Task: Add Test Modules and Fix Empty Test

**Objective**: Add `#[cfg(test)]` modules with constant verification tests to the 3 untested files, and replace the empty test body in `vm_service.rs` with meaningful assertions.

**Depends on**: Task 03 (constants must be at module scope before they can be tested)

**Review Issues**: #7 (Minor), #8 (Minor)

### Scope

- `crates/fdemon-app/src/actions/vm_service.rs`: Fix empty test at lines 317-326
- `crates/fdemon-app/src/actions/performance.rs`: Add `#[cfg(test)] mod tests`
- `crates/fdemon-app/src/actions/network.rs`: Add `#[cfg(test)] mod tests`
- `crates/fdemon-app/src/actions/inspector/mod.rs`: Add `#[cfg(test)] mod tests`

### Details

#### Issue #7: Empty test in `vm_service.rs`

**Current code at lines 317-326:**
```rust
#[test]
fn test_heartbeat_counter_reset_on_reconnection() {
    // The consecutive_failures counter in forward_vm_events is reset to 0 in:
    // 1. The Reconnecting arm (prevents accumulation during backoff)
    // 2. The Reconnected arm (clean slate after successful reconnect)
    // 3. The Ok(Ok(_)) heartbeat success arm (normal operation)
    //
    // This is an async integration concern that cannot be unit tested here.
    // Verified by code review.
}
```

**Replace with a meaningful invariant test:**
```rust
#[test]
fn test_heartbeat_counter_reset_on_reconnection() {
    // The counter reset to 0 on Reconnecting/Reconnected events is only
    // observable if MAX_HEARTBEAT_FAILURES > 1. If it were 1, a single
    // failure would immediately disconnect before any reset could occur.
    assert!(
        MAX_HEARTBEAT_FAILURES > 1,
        "MAX_HEARTBEAT_FAILURES must be > 1 for counter reset to have effect"
    );
}
```

#### Issue #8: Missing test modules

**`performance.rs`** — has 2 constants at module scope:
```rust
pub(super) const PERF_POLL_MIN_MS: u64 = 500;
pub(super) const ALLOC_PROFILE_POLL_MIN_MS: u64 = 1000;
```

Add:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_poll_constants_are_reasonable() {
        assert_eq!(PERF_POLL_MIN_MS, 500, "perf poll minimum should be 500ms");
        assert_eq!(ALLOC_PROFILE_POLL_MIN_MS, 1000, "alloc profile poll minimum should be 1000ms");
        assert!(
            ALLOC_PROFILE_POLL_MIN_MS >= PERF_POLL_MIN_MS,
            "allocation profiling is more expensive and should never poll faster than memory polling"
        );
    }
}
```

**`network.rs`** — has 1 constant at module scope:
```rust
pub(super) const NETWORK_POLL_MIN_MS: u64 = 500;
```

Add:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_poll_min_ms_is_reasonable() {
        assert_eq!(NETWORK_POLL_MIN_MS, 500, "network poll minimum should be 500ms");
    }
}
```

**`inspector/mod.rs`** — after Task 03 promotes `LAYOUT_FETCH_TIMEOUT` to module scope:

Add:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_layout_fetch_timeout_is_reasonable() {
        assert_eq!(
            LAYOUT_FETCH_TIMEOUT,
            Duration::from_secs(10),
            "layout fetch timeout should be 10 seconds"
        );
        assert!(
            LAYOUT_FETCH_TIMEOUT >= Duration::from_secs(5),
            "layout fetch timeout must be at least 5 seconds to avoid false timeouts"
        );
    }
}
```

### Acceptance Criteria

1. `vm_service.rs` test `test_heartbeat_counter_reset_on_reconnection` has at least one `assert!`
2. `performance.rs` has a `#[cfg(test)] mod tests` with constant verification
3. `network.rs` has a `#[cfg(test)] mod tests` with constant verification
4. `inspector/mod.rs` has a `#[cfg(test)] mod tests` with constant verification
5. All 7 files in `actions/` have at least one test with an assertion
6. `cargo test -p fdemon-app` passes with new tests
7. `cargo clippy --workspace -- -D warnings` clean

### Testing

Run the new tests individually to verify:
```bash
cargo test -p fdemon-app test_performance_poll_constants_are_reasonable
cargo test -p fdemon-app test_network_poll_min_ms_is_reasonable
cargo test -p fdemon-app test_layout_fetch_timeout_is_reasonable
cargo test -p fdemon-app test_heartbeat_counter_reset_on_reconnection
```

### Notes

- Follow the existing test pattern from `session.rs:348-360` and `vm_service.rs:294-315` — these test named constants with `assert_eq!` and derived invariants with `assert!`.
- The `inspector/widget_tree.rs` helper file also lacks a test module. It contains two pure functions (`is_transient_error`, `is_method_not_found`) that could be unit tested, but this is out of scope for this task — note it as a future improvement.
- The `Duration` import in the `inspector/mod.rs` test module may need to be added explicitly if `Duration` is not already in scope via `use super::*`.
