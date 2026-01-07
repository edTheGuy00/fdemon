## Task: Add expect() Context for Better Debugging

**Objective**: Replace `.unwrap()` calls with `.expect()` to provide better error context when tests fail.

**Depends on**: None (can be done independently)

**Priority**: Major (code quality improvement identified in review)

**Source**: [REVIEW.md](../../../REVIEW.md) - Code Quality Review, Major Issue #2

### Scope

- `tests/e2e/mock_daemon.rs`: Multiple `.unwrap()` calls
- `tests/e2e.rs`: Fixture loading functions

### Details

The current implementation uses bare `.unwrap()` without context:

```rust
// Current - no context on failure
serde_json::from_str(json).unwrap();
serde_json::to_string(&event).unwrap();
```

**Problem:** When tests fail, the panic message doesn't indicate what operation failed or why.

**Fix: Add descriptive expect() messages**

In `mock_daemon.rs`:
```rust
// In handle_command()
let parsed: serde_json::Value = match serde_json::from_str(json) {
    Ok(v) => v,
    Err(e) => {
        // Log or handle parse error more gracefully
        eprintln!("[mock_daemon] Failed to parse command JSON: {}", e);
        return true;
    }
};

// In send_event()
let json = serde_json::to_string(event)
    .expect("Failed to serialize daemon event to JSON");

// In send_response()
let json = serde_json::to_string(&response)
    .expect("Failed to serialize daemon response to JSON");

// In MockScenarioBuilder::build()
daemon.event_queue.push(DaemonEvent::Stdout(format!(
    "[{}]",
    serde_json::to_string(&event)
        .expect("Failed to serialize initial event")
)));
```

In `tests/e2e.rs` (already has good context, but verify):
```rust
// Already good:
std::fs::read_to_string(&path)
    .unwrap_or_else(|e| panic!("Failed to load fixture {}: {}", name, e))
```

### Locations to Update

1. `mock_daemon.rs:279` - `serde_json::to_string(event).unwrap()`
2. `mock_daemon.rs:292` - `serde_json::to_string(&response).unwrap()`
3. `mock_daemon.rs:357` - `serde_json::to_string(&event).unwrap()`

### Acceptance Criteria

1. All `.unwrap()` calls in mock daemon replaced with `.expect()` or error handling
2. Error messages are descriptive and include relevant context
3. All existing tests pass
4. `cargo clippy --test e2e` passes

### Testing

```bash
cargo test --test e2e
cargo clippy --test e2e
```

### Notes

- This is a test-only change
- Better error messages help debug CI failures
- Consider whether some errors should be handled gracefully vs panicking
