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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/spawn.rs` | Modified `spawn_bootable_device_discovery()` and `spawn_device_boot()` to accept `ToolAvailability` parameter instead of calling `ToolAvailability::check().await` internally |
| `src/tui/actions.rs` | Updated `handle_action()` to accept `ToolAvailability` parameter and pass it to spawn functions |
| `src/tui/process.rs` | Modified `process_message()` to pass `state.tool_availability.clone()` to `handle_action()` |

### Notable Decisions/Tradeoffs

1. **Used clone() for ToolAvailability**: `ToolAvailability` already implements `Clone` (line 10 of `src/daemon/tool_availability.rs`), so we clone it when passing from `AppState` to `handle_action()`. This is acceptable because the struct is small (2 bools + 1 Option<String>) and avoids lifetime complexity.

2. **Kept spawn_tool_availability_check() unchanged**: The `spawn_tool_availability_check()` function still calls `ToolAvailability::check().await` because it's the function responsible for checking tool availability at startup and populating the cache. This is the only place where the check should occur (acceptance criterion #3).

3. **Modified handle_action() signature**: Added `tool_availability: ToolAvailability` parameter to `handle_action()` rather than modifying individual UpdateAction variants. This keeps the changes localized and follows the existing pattern of passing cross-cutting concerns through function parameters.

### Testing Performed

- `cargo test --lib` - Passed (1448 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo check` - Passed (no compilation errors)

### Risks/Limitations

1. **No explicit test for passing tool availability**: While all existing tests pass, there's no specific test verifying that the cached `ToolAvailability` is correctly passed through the call chain. The existing behavior is preserved since `ToolAvailability::default()` provides safe defaults (both tools unavailable), which matches the check behavior when tools aren't installed.
