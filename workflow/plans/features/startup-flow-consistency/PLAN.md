# Plan: Startup Flow Consistency (Always Enter Normal Mode First)

## TL;DR

Unify startup behavior so the app always enters Normal mode immediately, regardless of `auto_start` setting. When `auto_start=true`, the app enters Normal mode first, then shows the loading dialog and auto-launches. This provides visual consistency and better aligns with the TEA pattern where all state changes flow through messages.

---

## Background

### Current Behavior

The startup flow currently branches early based on `auto_start` setting:

**auto_start=true:**
1. `runner.rs:61` sets loading state BEFORE main loop
2. `startup_flutter()` runs device discovery synchronously with loading animation
3. Session spawns before entering `run_loop()`
4. User sees: Loading screen → Running session

**auto_start=false:**
1. `startup_flutter()` immediately calls `enter_normal_mode_disconnected()`
2. User sees: Normal mode ("Not Connected") → Press '+' → Startup dialog

### Problems

1. **Inconsistent Visual Flow:** Auto-start users never see the Normal mode UI before their session starts
2. **Pre-loop Blocking:** `startup_flutter()` runs synchronously before the event loop, which:
   - Couples startup logic to the runner
   - Makes the loading animation depend on special `animate_during_async()` handling
3. **Harder to Test:** Auto-start path is harder to unit test since it bypasses the message loop

### Desired Behavior

**Both paths:**
1. App enters Normal mode immediately (user sees "Not Connected" briefly)
2. First frame renders
3. If `auto_start=true`: Send `Message::StartAutoLaunch` to trigger auto-start flow
4. Message handler shows loading screen, runs device discovery, spawns session
5. User sees: Normal mode (brief) → Loading → Running session

This provides:
- **Consistency:** Same visual entry point for all users
- **TEA Alignment:** All state changes flow through messages
- **Testability:** Auto-start can be tested via message dispatch
- **Separation:** Runner only handles loop; startup logic is in handlers

---

## Affected Modules

- `src/tui/startup.rs` - Simplify to always return `None`, trigger auto-start via message
- `src/tui/runner.rs` - Remove pre-loop loading state, send auto-start message after first render
- `src/app/message.rs` - Add `Message::StartAutoLaunch` variant
- `src/app/handler/update.rs` - Add handler for `StartAutoLaunch` (reuse existing auto-start logic)
- `src/app/state.rs` - No changes needed (loading state methods already exist)

---

## Development Phases

### Phase 1: Add StartAutoLaunch Message and Handler

**Goal**: Create the message infrastructure for triggering auto-start from the event loop.

#### Steps

1. **Add `Message::StartAutoLaunch` variant** (`src/app/message.rs`)
   - New variant: `StartAutoLaunch { configs: LoadedConfigs }`
   - Carries pre-loaded configs to avoid re-loading in handler

2. **Move auto-start logic to handler** (`src/app/handler/update.rs`)
   - Create `handle_start_auto_launch()` function
   - Set `UiMode::Loading` and loading message
   - Return `UpdateAction::DiscoverDevicesAndAutoLaunch { configs }` (new action)

3. **Add new UpdateAction** (`src/app/handler/mod.rs`)
   - Add `UpdateAction::DiscoverDevicesAndAutoLaunch { configs: LoadedConfigs }`
   - This action will be handled by a new async task

4. **Create async auto-launch task** (`src/tui/spawn.rs`)
   - Add `spawn_auto_launch()` function
   - Discovers devices, validates selection, spawns session
   - Sends progress messages: `DeviceDiscoveryProgress`, `AutoLaunchResult`

**Milestone**: Message and action infrastructure is in place.

---

### Phase 2: Update Runner to Use Message-Based Auto-Start

**Goal**: Remove synchronous startup logic from runner; use messages instead.

#### Steps

1. **Simplify `startup_flutter()`** (`src/tui/startup.rs`)
   - Remove all async device discovery logic
   - Always return `None` (no immediate action)
   - Just set `UiMode::Normal` for both paths
   - If `auto_start=true`, return a message to send (or set a flag)

2. **Update runner to send auto-start message** (`src/tui/runner.rs`)
   - Remove lines 60-65 (pre-loop loading state setup)
   - After first render, check if `auto_start=true`
   - If yes, send `Message::StartAutoLaunch { configs }` to channel
   - Configs are pre-loaded before the message is sent

3. **Handle loading animation in event loop** (`src/tui/runner.rs`)
   - Loading animation is already handled by `Message::Tick` (lines 145-147 in update.rs)
   - No special `animate_during_async()` needed once we're in the loop

**Milestone**: Auto-start flow runs through the message loop.

---

### Phase 3: Implement Auto-Launch Task and Result Handling

**Goal**: Complete the async auto-launch flow with proper state updates.

#### Steps

1. **Add progress messages** (`src/app/message.rs`)
   - `AutoLaunchProgress { message: String }` - Update loading message
   - `AutoLaunchResult { result: Result<(Device, Option<LaunchConfig>), String> }` - Final result

2. **Handle `DiscoverDevicesAndAutoLaunch`** (`src/tui/actions.rs`)
   - Spawn background task
   - Discover devices (async)
   - Send `AutoLaunchProgress` messages during discovery
   - Validate last selection or find auto-start config
   - Send `AutoLaunchResult` with device and config

3. **Handle `AutoLaunchProgress`** (`src/app/handler/update.rs`)
   - Update `state.update_loading_message(message)`

4. **Handle `AutoLaunchResult`** (`src/app/handler/update.rs`)
   - On success: Create session, clear loading, return `SpawnSession`
   - On failure: Clear loading, show `StartupDialog` with error

**Milestone**: Auto-start works end-to-end through message loop.

---

### Phase 4: Cleanup and Testing

**Goal**: Remove dead code, update tests, verify behavior.

#### Steps

1. **Remove dead code from `startup.rs`**
   - Remove `auto_start_session()` (logic moved to handler/task)
   - Remove `try_auto_start_config()` (logic moved to task)
   - Remove `launch_with_validated_selection()` (logic moved to task)
   - Remove `animate_during_async()` (no longer needed)
   - Keep `enter_normal_mode_disconnected()` and `cleanup_sessions()`

2. **Update handler tests** (`src/app/handler/tests.rs`)
   - Add tests for `Message::StartAutoLaunch`
   - Add tests for `Message::AutoLaunchProgress`
   - Add tests for `Message::AutoLaunchResult`

3. **Update snapshot tests** (`src/tui/render/tests.rs`)
   - Verify loading screen still renders correctly
   - Verify transition from Normal → Loading → Running

4. **Manual E2E verification**
   - Test with `auto_start=true`: Normal (brief) → Loading → Running
   - Test with `auto_start=false`: Normal → Press '+' → Dialog
   - Test with no devices: Normal → Loading → Dialog with error

**Milestone**: All tests pass, clean codebase.

---

## Alternative Approaches Considered

### A: Keep Synchronous Startup, Just Render First Frame Before

**Approach:** Render Normal mode frame, then run existing `startup_flutter()` synchronously.

**Pros:**
- Minimal code change
- Reuses existing `animate_during_async()`

**Cons:**
- Still blocks before entering event loop
- Loading animation still needs special handling
- Doesn't improve testability

**Decision:** Rejected. Moving to message-based is cleaner long-term.

### B: Use Message Channel from spawn_startup()

**Approach:** Spawn startup as background task immediately, communicate via messages.

**Pros:**
- Non-blocking from the start
- Fully async

**Cons:**
- More complex coordination
- Need to handle race conditions if user interacts before auto-start completes

**Decision:** This is essentially what we're proposing, but we wait for first render before sending the message.

---

## Edge Cases & Risks

### Race Condition: User Presses '+' During Auto-Launch

- **Risk:** User might press '+' while auto-launch is loading
- **Mitigation:** Handler should ignore `ShowStartupDialog` when `UiMode::Loading` is active. The loading screen doesn't process most key events anyway.

### Error During Device Discovery

- **Risk:** Device discovery fails, user stuck on loading
- **Mitigation:** `AutoLaunchResult` handles errors by clearing loading and showing StartupDialog with error message (same as current behavior).

### First Frame "Flash"

- **Risk:** Normal mode shows briefly before loading, might look like a flash
- **Mitigation:**
  - Keep first frame render time short
  - Consider: Don't render status bar during first frame, or show "Starting..." status
  - Acceptable trade-off for consistency

### Regression in Auto-Start Speed

- **Risk:** Message loop adds overhead
- **Mitigation:** Overhead is negligible (one message send). If profiling shows issues, optimize later.

---

## Message Summary

### New Messages

| Message | Purpose |
|---------|---------|
| `StartAutoLaunch { configs }` | Trigger auto-launch flow from Normal mode |
| `AutoLaunchProgress { message }` | Update loading screen message during discovery |
| `AutoLaunchResult { result }` | Report auto-launch success/failure |

### New Actions

| Action | Purpose |
|--------|---------|
| `DiscoverDevicesAndAutoLaunch { configs }` | Spawn async task for device discovery + auto-launch |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `Message::StartAutoLaunch` exists and compiles
- [ ] `UpdateAction::DiscoverDevicesAndAutoLaunch` exists
- [ ] Handler scaffolding in place (can be no-op initially)
- [ ] `cargo check` passes

### Phase 2 Complete When:
- [ ] `runner.rs` no longer sets loading state before loop
- [ ] `startup_flutter()` always enters Normal mode
- [ ] Auto-start message is sent after first render
- [ ] `cargo check` passes

### Phase 3 Complete When:
- [ ] Auto-launch task discovers devices and spawns session
- [ ] Loading screen animates correctly during auto-launch
- [ ] Errors show StartupDialog with error message
- [ ] `cargo test` passes

### Phase 4 Complete When:
- [ ] Dead code removed from `startup.rs`
- [ ] Handler tests cover new messages
- [ ] Manual E2E verification passes
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

---

## Future Enhancements

1. **Splash Screen:** Could show a branded splash instead of "Not Connected" during the brief Normal mode
2. **Startup Animation:** Fade transition from Normal → Loading for smoother UX
3. **Parallel Operations:** Start file watcher setup during device discovery

---

## References

- Previous startup rework plan: `workflow/plans/features/startup-flow-rework/PLAN.md`
- TEA architecture: `docs/ARCHITECTURE.md`
- Current startup logic: `src/tui/startup.rs`
- Current runner logic: `src/tui/runner.rs`
- Message handlers: `src/app/handler/update.rs`
