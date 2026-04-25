# Task 03 — Wire `launch.toml` into headless auto-start

**Agent:** implementor
**Worktree:** isolated (no write-file overlap with sibling tasks)
**Depends on:** none
**Plan:** [../BUG.md](../BUG.md)

## Problem

`headless_auto_start` (`src/headless/runner.rs:237-296`) discovers devices and then unconditionally takes `result.devices.first()`:

```rust
// Pick first device for auto-start
if let Some(device) = result.devices.first() {
    info!("Auto-starting with device: {} ({})", device.name, device.id);
    match engine.state.session_manager.create_session(device) {
        Ok(session_id) => {
            ...
            engine.dispatch_spawn_session(session_id, device.clone(), None);
```

`launch.toml` is never loaded in the headless path, and the `LaunchConfig` argument to `dispatch_spawn_session` is always `None`. This makes `device = "ios"` / `device = "android"` / `device = "macos"` ineffective in headless mode — independently of Task 01's matcher fix.

The TUI path solves this with `find_auto_launch_target` (`crates/fdemon-app/src/spawn.rs:215-275`), which encapsulates the full priority chain: saved last selection → first `auto_start` config → first config → bare run with first device. We should reuse it.

## Goal

Headless auto-start honours `launch.toml`:
- If a `launch.toml` config matches a discovered device, the headless session spawns on that device with that config.
- If `launch.toml` is missing entirely, headless falls back to the existing "first device, no config" behaviour (per user instruction).
- If `launch.toml` exists but no device matches, headless falls back to `devices.first()` and the warning surfaced by Task 02 (when both land) appears in logs as it does in TUI mode.

## Implementation Notes

1. In `src/headless/runner.rs`, after `discover_devices` returns successfully and devices are cached:
   - Load `LoadedConfigs` for the current project. Inspect how the TUI runner does this — search for `launch_configs`, `LoadedConfigs`, or `config::load_configs` in `crates/fdemon-app/src/` and `crates/fdemon-tui/src/runner.rs`. Reuse the same loader.
   - If config loading fails (file IO error, parse error), log a warning via `tracing::warn!` and continue with the existing first-device fallback. Do not abort headless startup just because `launch.toml` is missing or malformed.
2. Call `fdemon_app::spawn::find_auto_launch_target(&configs, &result.devices, project_path)` (or the appropriate re-export — extract a public re-export if the function is currently private to the `spawn` module's parent). The function returns `AutoLaunchSuccess { device, config: Option<LaunchConfig> }`.
3. Replace the existing `result.devices.first()` block with the resolved `(device, config)`. Pass `config.map(Box::new)` to `engine.dispatch_spawn_session`.
4. Preserve the current emission order: `device_detected` events → cache devices → `session_created` → `dispatch_spawn_session`.
5. Keep the existing "no devices found" error path unchanged (`HeadlessEvent::error("No devices found", true)`).

## Public API Considerations

- `find_auto_launch_target` and `AutoLaunchSuccess` are currently module-private. **Task 02 owns the visibility change** (it is the sole writer of `spawn.rs` in this wave) — by the time this task lands, both will be `pub`. **Do not write to `crates/fdemon-app/src/spawn.rs` from this task.** If you need to verify the visibility is in place before merging, sync against Task 02's branch.
- If the project path required by `find_auto_launch_target` is not directly available in `headless_auto_start`, source it from `engine.state.project_path` (see `AppState` definition in `crates/fdemon-app/src/state.rs` — the field already exists per the architecture doc).

## Acceptance Criteria

- [ ] `headless_auto_start` loads `launch.toml` (via the same loader the TUI runner uses) before deciding which device to spawn.
- [ ] `headless_auto_start` calls `find_auto_launch_target` and uses its returned `device` and `config`.
- [ ] When `launch.toml` is missing or fails to load, headless still auto-starts on `devices.first()` with `config = None` (existing behavior preserved).
- [ ] `dispatch_spawn_session` receives `Some(Box::new(config))` when a launch config was selected.
- [ ] No public API changes to `find_auto_launch_target`'s signature.
- [ ] **No writes to `crates/fdemon-app/src/spawn.rs`** — Task 02 owns it (including the `pub` visibility change).
- [ ] **No writes to `crates/fdemon-app/src/lib.rs`** — `pub mod spawn;` already exists at line 81.

## Tests to Add

Add a unit/integration test for headless auto-start. Search `src/headless/` and `tests/` for the existing test layout — there is precedent for a `tests` module inside `runner.rs` (see lines 298+).

| Test name | Scenario |
|-----------|----------|
| `test_headless_auto_start_picks_configured_device_over_first` | Three discovered devices `[android-1, ios-1, macos-1]`; `launch.toml` declares `device = "ios"`, `auto_start = true` → headless dispatches `SpawnSession` for `ios-1` (not `android-1`) |
| `test_headless_auto_start_falls_back_to_first_when_no_launch_toml` | Devices `[android-1, ios-1]`; no `launch.toml` → dispatches `SpawnSession` for `android-1` with `config = None` |
| `test_headless_auto_start_falls_back_to_first_when_configured_device_not_found` | Devices `[android-1]`; `launch.toml` declares `device = "ios"` → dispatches `SpawnSession` for `android-1` with the configured `Some(launch_config)` (matching TUI behaviour) |

If the `Engine` is not directly testable from this layer (mocking constraints), assert at the level of `find_auto_launch_target`'s integration with the loader instead, and cover the `dispatch_spawn_session` call via a small extracted helper that takes `(devices, configs, project_path)` and returns `(Device, Option<LaunchConfig>)`.

## Verification

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Manual smoke test:
- With `device = "ios"` (or `"macos"` once Task 01 lands) in `launch.toml` and multiple device types connected, run `fdemon --headless` — confirm the configured device is the one a session is created for (the headless event stream emits `session_created` with that device's name).

## Files

| File | Change |
|------|--------|
| `src/headless/runner.rs` | Load `LoadedConfigs`; call `find_auto_launch_target`; pass resolved `device` + `Some(Box::new(config))` to `dispatch_spawn_session`; preserve fallbacks; add tests |
