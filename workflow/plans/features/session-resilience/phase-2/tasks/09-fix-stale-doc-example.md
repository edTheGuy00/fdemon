## Task: Fix Stale Doc Example in vm_service/mod.rs

**Objective**: Update the quick-start doc example in `mod.rs` to reflect the new `VmClientEvent` enum type returned by `event_receiver()`.

**Depends on**: None

**Review Reference**: Phase-2 Review Issue #4

### Scope

- `crates/fdemon-daemon/src/vm_service/mod.rs`: Lines 38-39

### Details

The doc example shows:
```rust
// while let Some(event) = client.event_receiver().recv().await {
//     tracing::debug!("Event: {:?}", event.params.stream_id);
// }
```

After the phase-2 changes, `event_receiver()` yields `VmClientEvent` (an enum with `StreamEvent`, `Reconnecting`, `Reconnected`, `PermanentlyDisconnected` variants), not a bare `VmServiceEvent`. The field `event.params.stream_id` no longer exists on the enum type.

**Fix**: Update to destructure `VmClientEvent` variants:
```rust
// while let Some(event) = client.event_receiver().recv().await {
//     match event {
//         VmClientEvent::StreamEvent(e) => {
//             tracing::debug!("Stream event: {:?}", e.params.stream_id);
//         }
//         other => tracing::debug!("Lifecycle event: {:?}", other),
//     }
// }
```

### Acceptance Criteria

1. Doc example compiles conceptually against the current `VmClientEvent` type
2. Example demonstrates handling both stream events and lifecycle events
3. `cargo clippy --workspace -- -D warnings` clean

### Notes

- The example is inside a comment block so it won't cause compilation errors, but it misleads anyone reading the docs
