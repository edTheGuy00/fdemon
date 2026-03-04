## Task: Replace Client Counter with Client Registry

**Objective**: Replace the `client_count: usize` integer in `DapStatus::Running` with a `HashSet<String>` of client IDs, making the count self-correcting against lost connect/disconnect events.

**Depends on**: merge (post-merge improvement)

**Priority**: LOW

**Review Source**: REVIEW.md Issue #12 (Risks & Tradeoffs Analyzer)

### Scope

- `crates/fdemon-app/src/state.rs`: Change `DapStatus::Running` variant
- `crates/fdemon-app/src/handler/dap.rs`: Update connect/disconnect handlers
- `crates/fdemon-tui/src/widgets/header.rs`: Update any display that reads `client_count()`

### Background

`DapStatus::Running` currently holds `client_count: usize` (state.rs:771-774). The handler increments on `ClientConnected` and decrements (with `saturating_sub`) on `ClientDisconnected`. This is fragile: if a `ClientDisconnected` event is lost (e.g., channel drop), the count drifts permanently high. Conversely, duplicate `ClientConnected` events double-count.

A `HashSet<String>` keyed by client ID is self-correcting:
- Duplicate connects are idempotent (inserting an existing key is a no-op)
- The count is always `.len()`, derived from actual tracked clients

### Details

#### 1. Change DapStatus Variant

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DapStatus {
    #[default]
    Off,
    Starting,
    Running {
        port: u16,
        clients: HashSet<String>,
    },
    Stopping,
}
```

Update `client_count()` to return `clients.len()`:

```rust
pub fn client_count(&self) -> usize {
    match self {
        DapStatus::Running { clients, .. } => clients.len(),
        _ => 0,
    }
}
```

#### 2. Update Handler

In `handle_started`:

```rust
state.dap_status = DapStatus::Running {
    port,
    clients: HashSet::new(),
};
```

In `handle_client_connected`:

```rust
if let DapStatus::Running { clients, .. } = &mut state.dap_status {
    clients.insert(id.to_string());
}
```

In `handle_client_disconnected`:

```rust
if let DapStatus::Running { clients, .. } = &mut state.dap_status {
    clients.remove(&id.to_string());
}
```

Note: The `id` parameter comes from `DapServerEvent::ClientConnected { client_id }` which is already a `String`.

#### 3. Update Display

Check `widgets/header.rs` and any TUI code that reads `dap_status.client_count()`. The `client_count()` method signature is unchanged (`-> usize`), so display code should work without changes.

### Acceptance Criteria

1. `DapStatus::Running` holds `clients: HashSet<String>` instead of `client_count: usize`
2. `client_count()` returns `clients.len()`
3. Duplicate `ClientConnected` events with the same ID are idempotent
4. `ClientDisconnected` for an unknown ID is a silent no-op (not an error)
5. All existing handler tests pass (update assertions from `client_count: N` to `clients` set comparisons)
6. `cargo test -p fdemon-app` passes
7. `cargo test --workspace` passes

### Testing

Update existing tests in `dap.rs`:

```rust
#[test]
fn test_client_connected_increments_count() {
    // ... setup Running with empty clients ...
    handle_dap_message(&mut state, &Message::DapClientConnected { client_id: "c1".into() });
    assert_eq!(state.dap_status.client_count(), 1);
}

#[test]
fn test_client_connected_duplicate_is_idempotent() {
    // ... setup Running with clients = {"c1"} ...
    handle_dap_message(&mut state, &Message::DapClientConnected { client_id: "c1".into() });
    assert_eq!(state.dap_status.client_count(), 1); // still 1, not 2
}
```

### Notes

- `HashSet<String>` requires `use std::collections::HashSet` in `state.rs`.
- The `PartialEq` and `Eq` derives on `DapStatus` work with `HashSet<String>` since `String: Eq + Hash`.
- For Phase 3, the client registry could be extended to store metadata (e.g., connected timestamp, client name from initialize request).
