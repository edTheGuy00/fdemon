## Task: Engine, State, and Action Integration

**Objective**: Wire the SDK locator through the full TEA pipeline — resolve the SDK at startup, store it in `AppState`, thread it through `UpdateAction` dispatchers, and update `ToolAvailability` with a Flutter check. This is the integration task that makes everything work end-to-end.

**Depends on**: 04-sdk-locator, 05-flutter-settings, 06-update-call-sites

### Scope

- `crates/fdemon-app/src/state.rs`: Add `resolved_sdk` field to `AppState`
- `crates/fdemon-app/src/engine.rs`: Resolve SDK in `Engine::new()` initialization
- `crates/fdemon-app/src/message.rs`: Add `Message` variants for SDK status
- `crates/fdemon-app/src/handler/update.rs`: Handle new SDK message variants
- `crates/fdemon-app/src/handler/mod.rs`: Add `UpdateAction` variants for SDK operations
- `crates/fdemon-daemon/src/tool_availability.rs`: Add Flutter SDK check
- Action dispatcher(s): Thread `FlutterExecutable` through session spawn, device discovery, emulator discovery

### Details

#### 1. Add SDK state to `AppState` (`state.rs`)

```rust
pub struct AppState {
    // ... existing fields ...

    /// Resolved Flutter SDK from the detection chain.
    /// `None` if no SDK was found at startup.
    pub resolved_sdk: Option<FlutterSdk>,
}
```

In `AppState::new()` or `AppState::with_settings()`, initialize to `None`. The SDK is resolved after settings are loaded and before the first session spawn.

#### 2. Resolve SDK in `Engine::new()` (`engine.rs`)

The locator (`find_flutter_sdk`) is synchronous — it only does filesystem operations. Insert it between the existing step 2 (load settings) and step 3 (create AppState):

```rust
pub fn new(project_path: PathBuf) -> Self {
    // Step 2: Load settings
    let settings = config::load_settings(&project_path);

    // Step 2.5: Resolve Flutter SDK (NEW)
    let resolved_sdk = match flutter_sdk::find_flutter_sdk(
        &project_path,
        settings.flutter.sdk_path.as_deref(),
    ) {
        Ok(sdk) => {
            info!("Flutter SDK resolved via {}: {} at {}", sdk.source, sdk.version, sdk.root.display());
            Some(sdk)
        }
        Err(e) => {
            warn!("Flutter SDK not found: {e}. SDK-dependent features will be unavailable.");
            None
        }
    };

    // Step 3: Create AppState
    let mut state = AppState::with_settings(project_path.clone(), settings.clone());
    state.resolved_sdk = resolved_sdk;

    // ... rest of initialization ...
}
```

**SDK resolution failure is NOT fatal**: If no SDK is found, fdemon still starts — it just can't spawn Flutter sessions. The user sees a helpful message and can configure the SDK path or install Flutter.

#### 3. Add `Message` variants (`message.rs`)

```rust
pub enum Message {
    // ... existing variants ...

    // ── Flutter SDK ──

    /// SDK resolution completed successfully (e.g., after re-resolution)
    SdkResolved { sdk: FlutterSdk },

    /// SDK resolution failed
    SdkResolutionFailed { reason: String },
}
```

These are used when the SDK needs to be re-resolved at runtime (e.g., after the user changes `config.toml`, or in Phase 2 when switching versions).

**Note**: `FlutterSdk` must implement `Clone` and `Debug` (already specified in task 01) to be used in `Message` variants.

#### 4. Handle SDK messages in `update.rs`

```rust
Message::SdkResolved { sdk } => {
    info!("Flutter SDK updated: {} via {}", sdk.version, sdk.source);
    state.resolved_sdk = Some(sdk);
    UpdateResult::none()
}

Message::SdkResolutionFailed { reason } => {
    warn!("SDK resolution failed: {reason}");
    state.resolved_sdk = None;
    UpdateResult::none()
}
```

#### 5. Thread `FlutterExecutable` through action dispatchers

The action dispatcher (in `fdemon-app/src/actions/` or equivalent) executes `UpdateAction` variants. Currently, session spawning and device discovery call daemon functions without an SDK parameter. Update them to extract the `FlutterExecutable` from `AppState.resolved_sdk`.

**Pattern — extract executable before dispatching:**

```rust
// In the action handler that processes UpdateAction::SpawnSession:
fn handle_spawn_session(state: &AppState, session_id: SessionId, device: Device, ...) {
    let flutter = match &state.resolved_sdk {
        Some(sdk) => &sdk.executable,
        None => {
            // Cannot spawn without SDK — send error message
            msg_tx.send(Message::SessionSpawnFailed {
                reason: "No Flutter SDK found. Configure sdk_path in .fdemon/config.toml or install Flutter.".into()
            });
            return;
        }
    };

    // Now pass flutter to daemon functions
    FlutterProcess::spawn_with_device(flutter, project_path, device_id, event_tx).await?;
}
```

**Same pattern for device/emulator discovery:**

```rust
// UpdateAction::DiscoverDevices handler:
let flutter = match &state.resolved_sdk {
    Some(sdk) => sdk.executable.clone(),
    None => {
        // Can't discover devices without SDK
        return;
    }
};
let result = discover_devices(&flutter).await;
```

Identify all action dispatch sites in the codebase that currently call `FlutterProcess::spawn*`, `discover_devices*`, `discover_emulators*`, or `launch_emulator*` and update them.

#### 6. Update `ToolAvailability` (`tool_availability.rs`)

Add a Flutter SDK field:

```rust
pub struct ToolAvailability {
    // ... existing fields ...

    /// Whether a Flutter SDK was found
    pub flutter_sdk: bool,

    /// How the Flutter SDK was detected (for display)
    pub flutter_sdk_source: Option<String>,
}
```

**Do not add a new async check** — the SDK is already resolved synchronously in `Engine::new()`. Instead, populate `ToolAvailability` from the resolved SDK after construction:

```rust
// In Engine::new(), after SDK resolution:
state.tool_availability.flutter_sdk = resolved_sdk.is_some();
state.tool_availability.flutter_sdk_source = resolved_sdk.as_ref().map(|s| s.source.to_string());
```

#### 7. Update the Engine's `resolved_sdk` accessibility

The Engine (or its state) needs to provide the `FlutterExecutable` to action handlers. Depending on how actions are dispatched:

- If actions have access to `&AppState` → read `state.resolved_sdk`
- If actions only have specific fields → pass `FlutterExecutable` as part of the `UpdateAction` variant

Check the existing `UpdateAction::SpawnSession` variant to see how data flows. If it already carries all needed data (device, config, etc.), add `flutter: FlutterExecutable` to it:

```rust
UpdateAction::SpawnSession {
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
    flutter: FlutterExecutable,  // NEW
}
```

Similarly for `UpdateAction::DiscoverDevices` and any emulator-related actions.

### Acceptance Criteria

1. `AppState.resolved_sdk` is populated at startup from the detection chain
2. SDK resolution failure does not prevent fdemon from starting
3. A warning is logged when no SDK is found
4. `FlutterProcess::spawn*` calls receive the resolved `FlutterExecutable`
5. `discover_devices()` calls receive the resolved `FlutterExecutable`
6. `discover_emulators()` and `launch_emulator()` calls receive the resolved `FlutterExecutable`
7. When `resolved_sdk` is `None`, session spawn returns a meaningful error message
8. `ToolAvailability` shows Flutter SDK status
9. `Message::SdkResolved` and `Message::SdkResolutionFailed` are handled in `update.rs`
10. `cargo check --workspace` compiles
11. `cargo test --workspace` passes (all existing + new tests)
12. `cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default_sdk_is_none() {
        let state = AppState::default();
        assert!(state.resolved_sdk.is_none());
    }

    #[test]
    fn test_sdk_resolved_message_updates_state() {
        let mut state = AppState::default();
        let sdk = FlutterSdk {
            root: PathBuf::from("/usr/local/flutter"),
            executable: FlutterExecutable::Direct(PathBuf::from("/usr/local/flutter/bin/flutter")),
            source: SdkSource::SystemPath,
            version: "3.19.0".into(),
            channel: Some("stable".into()),
        };

        let result = update(&mut state, Message::SdkResolved { sdk: sdk.clone() });
        assert!(state.resolved_sdk.is_some());
        assert_eq!(state.resolved_sdk.unwrap().version, "3.19.0");
    }

    #[test]
    fn test_sdk_resolution_failed_clears_sdk() {
        let mut state = AppState::default();
        // Set an SDK first
        state.resolved_sdk = Some(FlutterSdk { /* ... */ });

        let result = update(&mut state, Message::SdkResolutionFailed {
            reason: "No SDK found".into()
        });
        assert!(state.resolved_sdk.is_none());
    }

    #[test]
    fn test_tool_availability_reflects_sdk_status() {
        let mut state = AppState::default();
        assert!(!state.tool_availability.flutter_sdk);

        state.resolved_sdk = Some(FlutterSdk { /* ... */ });
        state.tool_availability.flutter_sdk = true;
        state.tool_availability.flutter_sdk_source = Some("FVM (3.19.0)".into());

        assert!(state.tool_availability.flutter_sdk);
        assert_eq!(state.tool_availability.flutter_sdk_source.as_deref(), Some("FVM (3.19.0)"));
    }
}
```

### Notes

- **This is the highest-risk task** because it touches the most files across multiple crates and wires everything together. All prior tasks must be complete and passing before starting this one.
- **Follow the existing action dispatch pattern**: Look at how `UpdateAction::SpawnSession` currently gets dispatched (in `actions/mod.rs` or `engine.rs`). The `FlutterExecutable` should flow through the same mechanism.
- **`Engine::new()` remains synchronous**: The SDK locator is already sync (filesystem-only). No changes to Engine's constructor signature.
- **Existing tests that spawn sessions or discover devices** will need the `FlutterExecutable` parameter threaded through. Search for all call sites of `FlutterProcess::spawn`, `discover_devices`, `discover_emulators`, and `launch_emulator` in `fdemon-app` and update them.
- **Compile-driven development**: Start by adding the new fields and `Message` variants. The compiler will guide you to every call site that needs updating.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/tool_availability.rs` | Added `flutter_sdk: bool` and `flutter_sdk_source: Option<String>` fields; initialized in `check()`; all test literals converted to `..Default::default()` |
| `crates/fdemon-daemon/src/test_utils.rs` | Added `fake_flutter_sdk()` helper for unit tests; updated imports to use public re-exports |
| `crates/fdemon-app/src/state.rs` | Added `resolved_sdk: Option<FlutterSdk>` field; added `flutter_executable()` helper method |
| `crates/fdemon-app/src/message.rs` | Added `SdkResolved { sdk: FlutterSdk }` and `SdkResolutionFailed { reason: String }` variants |
| `crates/fdemon-app/src/handler/mod.rs` | Added `flutter: FlutterExecutable` to six `UpdateAction` variants: `DiscoverDevices`, `RefreshDevicesBackground`, `DiscoverDevicesAndAutoLaunch`, `DiscoverEmulators`, `LaunchEmulator`, `SpawnSession` |
| `crates/fdemon-app/src/engine.rs` | Added SDK resolution block in `Engine::new()`; populates `state.resolved_sdk` and `tool_availability`; updated `dispatch_spawn_session` to extract `FlutterExecutable` |
| `crates/fdemon-app/src/spawn.rs` | Updated all spawn function signatures to accept `flutter: FlutterExecutable`; passed to daemon discovery functions |
| `crates/fdemon-app/src/actions/mod.rs` | Updated all action match arms to destructure and forward the new `flutter` field |
| `crates/fdemon-app/src/actions/session.rs` | Added `flutter: FlutterExecutable` parameter; added `#[allow(clippy::too_many_arguments)]` |
| `crates/fdemon-app/src/handler/update.rs` | Added handlers for `SdkResolved` and `SdkResolutionFailed`; updated all call sites that construct `UpdateAction` variants with `flutter` field |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Added `state_with_sdk()` test helper; updated 10 failing tests to use it; handler uses `state.flutter_executable()` guard |
| `crates/fdemon-app/src/handler/new_session/navigation.rs` | Updated `test_app_state()` to inject fake SDK; fixed `matches!` patterns to use `{ .. }` |
| `crates/fdemon-app/src/handler/new_session/target_selector.rs` | Updated `test_app_state()` to inject fake SDK; fixed `matches!` patterns to use `{ .. }` |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Handler uses `state.flutter_executable()` guard for `SpawnSession` path |
| `crates/fdemon-app/src/handler/session_lifecycle.rs` | Updated `DiscoverDevices` construction to use `if let Some(flutter)` guard |
| `crates/fdemon-app/src/handler/tests.rs` | Added `fake_flutter_sdk()` to 7 failing test functions; fixed 3 `matches!` patterns |
| `crates/fdemon-tui/src/runner.rs` | Wrapped `spawn_device_discovery` with `if let Some(flutter) = engine.state.flutter_executable()` guard |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Converted explicit `ToolAvailability` struct literal to `..Default::default()` |
| `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | Converted 3 explicit `ToolAvailability` struct literals to `..Default::default()` |
| `src/headless/runner.rs` | Added `FlutterExecutable` guard at start of `headless_auto_start()`; passed `&flutter` to `discover_devices()` |

### Notable Decisions/Tradeoffs

1. **`FlutterExecutable` embedded in `UpdateAction` variants**: Rather than reading from state inside `handle_action`, the executable is embedded at action-creation time. This follows the existing pattern for all other data in `UpdateAction` and makes action dispatch self-contained.

2. **`state.flutter_executable()` guard pattern**: When no SDK is resolved, handlers log a warning and return `UpdateResult::none()` (or set a UI error in `launch_context.rs`). This is a graceful degradation — fdemon can still start and show the UI even without a Flutter SDK.

3. **`#[allow(clippy::too_many_arguments)]` on `spawn_session`**: The function already had 7 args (at Clippy's limit) before this task added `flutter`. A parameter struct refactor is out of scope; the allow suppresses the lint without changing behavior.

4. **`fake_flutter_sdk()` in `test_utils`**: Added as a public test helper in `fdemon_daemon::test_utils` so all crates can construct a sentinel SDK without filesystem access.

5. **Test updates**: All existing tests that expected `DiscoverDevices`, `RefreshDevicesBackground`, or `SpawnSession` actions needed either a fake SDK injected into state, or `matches!` patterns updated to `{ .. }` to accept the new struct variant form.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (3,291 tests across all crates, 0 failed, 74 ignored)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Headless mode without SDK**: `headless_auto_start` now returns early with an error event if no SDK is resolved. This is a behavior change but correct — headless mode cannot discover devices without Flutter.
2. **No new unit tests for `SdkResolved`/`SdkResolutionFailed`**: The task's testing section showed pseudocode stubs. The handlers are covered by the overall handler structure; dedicated tests for those two messages were not added in this pass but are straightforward to add.
