## Task: Thread `FlutterMode` Through the Monitoring Chain

**Objective**: Pass `FlutterMode` from the session's `LaunchConfig` through `UpdateAction`, hydration, and action dispatch to both `spawn_performance_polling` and `spawn_network_monitoring`, enabling mode-aware behavior in subsequent tasks.

**Depends on**: 01-dedup-memory-rpc, 02-missed-tick-skip

**Estimated Time**: 1.5-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mod.rs`: Add `mode: FlutterMode` field to `UpdateAction::StartPerformanceMonitoring` and `StartNetworkMonitoring`
- `crates/fdemon-app/src/handler/update.rs`: Read mode from `session.launch_config` at `VmServiceConnected` (~line 1398) and `VmServiceReconnected` (~line 1458), pass it into the action
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Read mode from session in `handle_switch_panel` Network branch (~line 190), pass it into `StartNetworkMonitoring`
- `crates/fdemon-app/src/process.rs`: Thread `mode` through `hydrate_start_performance_monitoring` (~line 115) and `hydrate_start_network_monitoring` (~line 288) — pass it through unchanged
- `crates/fdemon-app/src/actions/mod.rs`: Pass `mode` from the action payload to `spawn_performance_polling` (~line 203) and `spawn_network_monitoring` (~line 317)
- `crates/fdemon-app/src/actions/performance.rs`: Add `mode: FlutterMode` parameter to `spawn_performance_polling` signature (~line 69) — store it but don't use it yet (next task)
- `crates/fdemon-app/src/actions/network.rs`: Add `mode: FlutterMode` parameter to `spawn_network_monitoring` signature (~line 52) — store it but don't use it yet

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs`: `FlutterMode` enum definition (~line 75)
- `crates/fdemon-app/src/session/session.rs`: `Session.launch_config: Option<LaunchConfig>` (~line 93)

### Details

#### Current State

- `FlutterMode` enum (`Debug`, `Profile`, `Release`) is defined in `config/types.rs:75-100`
- It lives on `LaunchConfig.mode` (`config/types.rs:26`)
- `Session` stores `launch_config: Option<LaunchConfig>` (`session/session.rs:93`)
- Neither `UpdateAction::StartPerformanceMonitoring` nor `StartNetworkMonitoring` carries a mode field
- Neither `spawn_performance_polling` nor `spawn_network_monitoring` receives a mode parameter

#### Threading Path

```
Session.launch_config.as_ref().map(|c| c.mode).unwrap_or(FlutterMode::Debug)
  │
  ▼
handler/update.rs: VmServiceConnected → UpdateAction::StartPerformanceMonitoring { mode, ... }
handler/devtools/mod.rs: SwitchDevToolsPanel(Network) → UpdateAction::StartNetworkMonitoring { mode, ... }
  │
  ▼
process.rs: hydrate_start_*_monitoring() — passes mode through unchanged
  │
  ▼
actions/mod.rs: dispatches to spawn_*_monitoring(mode, ...)
  │
  ▼
actions/performance.rs: spawn_performance_polling(..., mode) — stores for use
actions/network.rs: spawn_network_monitoring(..., mode) — stores for use
```

#### Step-by-step Changes

**1. `handler/mod.rs` — Add `mode` field to both actions (~lines 161-175 and 232-239)**

```rust
UpdateAction::StartPerformanceMonitoring {
    session_id: SessionId,
    handle: Option<VmRequestHandle>,
    performance_refresh_ms: u64,
    allocation_profile_interval_ms: u64,
    mode: FlutterMode,  // NEW
}

UpdateAction::StartNetworkMonitoring {
    session_id: SessionId,
    handle: Option<VmRequestHandle>,
    poll_interval_ms: u64,
    mode: FlutterMode,  // NEW
}
```

**2. `handler/update.rs` — Read mode at both VmServiceConnected call sites**

At `VmServiceConnected` (~line 1398) and `VmServiceReconnected` (~line 1458):

```rust
let mode = state.session_manager
    .get(&session_id)
    .and_then(|h| h.session.launch_config.as_ref())
    .map(|c| c.mode)
    .unwrap_or(FlutterMode::Debug);
```

Pass `mode` into the returned `UpdateAction::StartPerformanceMonitoring { ..., mode }`.

**3. `handler/devtools/mod.rs` — Read mode in Network panel switch (~line 190)**

In `handle_switch_panel`, the Network branch (~lines 176-197):

```rust
let mode = state.session_manager
    .current()
    .and_then(|h| h.session.launch_config.as_ref())
    .map(|c| c.mode)
    .unwrap_or(FlutterMode::Debug);
```

Pass `mode` into `UpdateAction::StartNetworkMonitoring { ..., mode }`.

**4. `process.rs` — Pass `mode` through hydration unchanged**

In `hydrate_start_performance_monitoring` (~line 115-155): the function already destructures the action fields and reconstructs them. Add `mode` to both the destructure and the reconstruct. Same for `hydrate_start_network_monitoring` (~line 288-322).

**5. `actions/mod.rs` — Pass `mode` to spawn functions (~lines 194-217 and 309-325)**

Extract `mode` from the destructured action and pass it to the spawn call.

**6. `actions/performance.rs` and `actions/network.rs` — Accept and store `mode`**

Add `mode: FlutterMode` to the function signatures. In this task, the mode is accepted but not used — subsequent task 04 will apply interval scaling based on it. The parameter should appear in the signature and be visible in the spawned task's closure for use in task 04.

#### Default Mode When `launch_config` is `None`

When a session is launched without a `LaunchConfig` (bare `flutter run` without `.fdemon/launch.toml`), `session.launch_config` is `None`. Default to `FlutterMode::Debug` — this preserves current behavior (all polling at configured intervals, no throttling).

### Acceptance Criteria

1. `UpdateAction::StartPerformanceMonitoring` has a `mode: FlutterMode` field
2. `UpdateAction::StartNetworkMonitoring` has a `mode: FlutterMode` field
3. `VmServiceConnected` handler reads mode from `session.launch_config` and passes it through
4. `VmServiceReconnected` handler does the same
5. `handle_switch_panel` (Network branch) reads mode and passes it through
6. `spawn_performance_polling` accepts `mode: FlutterMode` parameter
7. `spawn_network_monitoring` accepts `mode: FlutterMode` parameter
8. When `launch_config` is `None`, mode defaults to `FlutterMode::Debug`
9. **No behavioral change yet** — mode is threaded but not acted upon (that's task 04)
10. All existing tests pass: `cargo test --workspace`
11. `cargo clippy --workspace -- -D warnings` — no new warnings (no unused variable warnings — use `_mode` or `let _ = mode;` if needed temporarily)

### Testing

**Test for mode extraction:**

```rust
#[test]
fn test_vm_service_connected_passes_debug_mode_when_no_launch_config() {
    // Session with launch_config = None
    // VmServiceConnected should produce StartPerformanceMonitoring with mode = Debug
}

#[test]
fn test_vm_service_connected_passes_profile_mode_from_launch_config() {
    // Session with launch_config.mode = Profile
    // VmServiceConnected should produce StartPerformanceMonitoring with mode = Profile
}

#[test]
fn test_switch_to_network_panel_passes_mode() {
    // Session with launch_config.mode = Profile
    // SwitchDevToolsPanel(Network) should produce StartNetworkMonitoring with mode = Profile
}
```

**Existing tests:** Many existing tests construct `UpdateAction::StartPerformanceMonitoring` and `StartNetworkMonitoring`. These will fail to compile until the `mode` field is added. Fix by adding `mode: FlutterMode::Debug` to all existing test constructions.

### Notes

- This is a **plumbing-only** task. No behavior changes. The mode value flows through the system but is not acted upon until task 04.
- The `FlutterMode` enum already derives `Clone, Copy, Debug, PartialEq` (`config/types.rs:75`), so it can be cheaply passed by value through all layers.
- The `VmServiceReconnected` handler at `update.rs:1407-1464` follows the same pattern as `VmServiceConnected` — apply the same mode-reading logic there.
- Expect several compiler errors when adding the field to `UpdateAction` — all existing match arms and construction sites must be updated. Search for `StartPerformanceMonitoring` and `StartNetworkMonitoring` across the codebase to find all sites.
- `process.rs` hydration functions are passthrough — they don't read or interpret the mode, just carry it from the action to the spawn call.

---

## Completion Summary

**Status:** Not Started
