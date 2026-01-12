## Task: Use Tool Availability Cache

**Objective**: Modify spawn functions to use the cached `ToolAvailability` from `AppState` instead of re-checking tool availability on every call.

**Depends on**: 05-discovery-integration

**Source**: Code Quality Inspector, Risks & Tradeoffs Analyzer (Review Issue #3)

### Scope

- `src/tui/spawn.rs`: Accept `ToolAvailability` parameter instead of calling check
- `src/app/message.rs`: Add tool availability to relevant messages if needed
- `src/app/handler/update.rs`: Pass cached tool availability to spawn functions
- `src/tui/actions.rs`: Update action creation if needed

### Details

Currently `spawn_bootable_device_discovery()` and `spawn_device_boot()` call `ToolAvailability::check().await` on each invocation (lines 283, 309). This duplicates work since `ToolAvailability` is already cached in `AppState.tool_availability`.

**Current Pattern:**
```rust
pub async fn spawn_bootable_device_discovery(...) -> ... {
    let tools = ToolAvailability::check().await;  // Re-checks every time!
    // ...
}
```

**Required Pattern:**
```rust
pub async fn spawn_bootable_device_discovery(
    tools: ToolAvailability,
    // ... other params
) -> ... {
    // Use passed-in tools directly
}
```

**Handler Change:**
```rust
// In update.rs
UpdateAction::DiscoverBootableDevices => {
    let tools = state.tool_availability.clone();
    spawn_bootable_device_discovery(tools, /* ... */);
}
```

### Acceptance Criteria

1. `spawn_bootable_device_discovery()` accepts `ToolAvailability` as parameter
2. `spawn_device_boot()` accepts `ToolAvailability` as parameter (if it uses tools)
3. No `ToolAvailability::check()` calls in `spawn.rs` (except at app startup)
4. Handlers pass cached value from `state.tool_availability`
5. `cargo test` passes
6. `cargo clippy -- -D warnings` passes

### Testing

Verify spawn functions work correctly with passed-in tool availability:
```rust
#[tokio::test]
async fn test_spawn_bootable_device_discovery_uses_passed_tools() {
    let tools = ToolAvailability::default();  // or mock
    // Test that spawn function respects tool availability flags
}
```

### Notes

- Follow TEA pattern: state flows through actions, not re-computed in spawns
- Check if `spawn_device_boot()` actually needs tool availability or just device info
- Consider if `ToolAvailability` should be `Clone` or passed by reference
- Update all call sites in handlers

---

## Completion Summary

**Status:** Not started
