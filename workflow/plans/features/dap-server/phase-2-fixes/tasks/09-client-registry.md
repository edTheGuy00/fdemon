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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Changed `DapStatus::Running { client_count: usize }` to `{ clients: HashSet<String> }`; updated `client_count()` method to return `clients.len()`; `HashSet` was already imported |
| `crates/fdemon-app/src/handler/dap.rs` | Added `use std::collections::HashSet`; updated `handle_started` to use `clients: HashSet::new()`; updated `handle_client_connected` to use `clients.insert()`; updated `handle_client_disconnected` to use `clients.remove()`; rewrote all tests referencing `client_count: N` to use `clients: HashSet` constructions; replaced `test_client_connected_multiple_times_increments_correctly` with `test_client_connected_duplicate_is_idempotent` and `test_client_connected_multiple_distinct_clients`; replaced `test_client_disconnected_saturates_at_zero` with `test_client_disconnected_unknown_id_is_noop` |

### Notable Decisions/Tradeoffs

1. **`use std::collections::HashSet` already present in state.rs**: The import was already at the top of `state.rs` for other uses, so no new import was needed there. Added it to `handler/dap.rs` for the `HashSet::new()` calls in handler logic and tests.
2. **Test semantics updated to match new model**: The old `test_client_connected_multiple_times_increments_correctly` test (which pre-seeded `client_count: 1`) was replaced with two clearer tests: one proving duplicate-ID inserts are idempotent and one proving distinct-ID inserts each count. The old `test_client_disconnected_saturates_at_zero` was replaced with `test_client_disconnected_unknown_id_is_noop` since `HashSet::remove` is already a no-op for missing keys.
3. **`header.rs` unchanged**: The widget reads `dap_status.client_count()` whose signature is unchanged (`-> usize`), so no TUI changes were required.

### Testing Performed

- `cargo test -p fdemon-app` — Passed (1265 tests)
- `cargo test --workspace` — Passed (all crates, 0 failures)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)
- `cargo fmt --all -- --check` — Passed (no formatting issues)

### Risks/Limitations

1. **None**: The change is purely mechanical — same external API (`client_count() -> usize`), stronger internal correctness guarantee (set semantics vs. integer arithmetic).
