## Task: Fix Event Queue Performance

**Objective**: Replace `Vec::remove(0)` with `VecDeque::pop_front()` for O(1) FIFO operations in the mock daemon event queue.

**Depends on**: None (can be done independently)

**Priority**: Critical (performance issue identified in code review)

**Source**: [REVIEW.md](../../../REVIEW.md) - Logic Reasoning Review, Critical Issue #3

### Scope

- `tests/e2e/mock_daemon.rs`: Replace `Vec<DaemonEvent>` with `VecDeque<DaemonEvent>`

### Details

The current implementation uses `Vec::remove(0)` for FIFO event processing:

```rust
// Current (O(n) per removal)
event_queue: Vec<DaemonEvent>,
// ...
let event = self.event_queue.remove(0);  // Line 154
```

This is O(n) for each removal, creating O(n) total complexity for n events. The production code uses `VecDeque` for the same pattern.

**Fix:**

```rust
use std::collections::VecDeque;

// In MockFlutterDaemon struct
event_queue: VecDeque<DaemonEvent>,

// In run() method
let event = self.event_queue.pop_front().unwrap();

// In MockScenarioBuilder, update initial_events type
initial_events: Vec<serde_json::Value>,  // Keep as Vec, convert on build

// In MockFlutterDaemon::new() initialization
event_queue: VecDeque::new(),
```

Also update `MockScenarioBuilder::build()`:
```rust
for event in self.initial_events {
    daemon.event_queue.push_back(DaemonEvent::Stdout(...));
}
```

### Acceptance Criteria

1. `event_queue` field changed from `Vec<DaemonEvent>` to `VecDeque<DaemonEvent>`
2. All `.push()` calls changed to `.push_back()`
3. `remove(0)` changed to `pop_front().unwrap()` (or handle None case)
4. All existing tests pass
5. `cargo clippy --test e2e` passes with no new warnings

### Testing

```bash
cargo test --test e2e
cargo clippy --test e2e
```

All 56 tests should continue to pass.

### Notes

- `VecDeque` is already available in `std::collections`
- This aligns with production code patterns
- No behavioral change, only performance improvement
