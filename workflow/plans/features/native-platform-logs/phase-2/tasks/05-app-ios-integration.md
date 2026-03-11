## Task: App Layer iOS Integration

**Objective**: Wire iOS platform support into the app layer — add `"ios"` to platform guards in session handler, build `IosLogConfig` in actions, and detect simulator vs physical device.

**Depends on**: 01-ios-tool-availability, 02-ios-log-config, 03-ios-simulator-capture, 04-ios-physical-capture

### Scope

- `crates/fdemon-app/src/handler/session.rs`: Add `"ios"` to `needs_capture` check in `maybe_start_native_log_capture()`
- `crates/fdemon-app/src/actions/native_logs.rs`: Add iOS config building branch, `derive_ios_process_name()`, `detect_ios_simulator()`
- `crates/fdemon-app/src/message.rs`: No changes needed — existing `UpdateAction::StartNativeLogCapture` already carries all needed fields

### Details

#### 1. Add `"ios"` to `needs_capture` in session handler

In `crates/fdemon-app/src/handler/session.rs`, the `maybe_start_native_log_capture()` function has a `needs_capture` check. Add `"ios"`:

```rust
// Only Android, macOS, and iOS need a separate capture process.
let needs_capture = platform == "android"
    || (cfg!(target_os = "macos") && platform == "macos")
    || (cfg!(target_os = "macos") && platform == "ios");
```

#### 2. Detect simulator vs physical device

The session's `device_id` and other metadata can be used to determine whether the target is a simulator or physical device. Flutter's device discovery marks simulators distinctly.

**Approach**: Check if the device platform name contains "simulator" or if the device category is "simulator". The `Session` struct has `device_id`, `device_name`, and `platform` fields. Flutter uses `"ios"` for both simulators and physical devices, but the device ID format differs:
- Simulator UDIDs are standard UUIDs (e.g., `AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE`)
- Physical device UDIDs are 40-char hex strings (e.g., `00008030000011ABC000DEF`)

However, the most reliable approach is to check if the `device_name` contains "Simulator" or if the device was discovered via `xcrun simctl`:

```rust
/// Detect whether an iOS device is a simulator based on its metadata.
///
/// Heuristic: Simulator device names from Flutter's device discovery
/// typically contain "Simulator" (e.g., "iPhone 15 Simulator").
/// Physical device names are the user-set device name (e.g., "Ed's iPhone").
///
/// Falls back to `false` (physical) if detection is ambiguous.
fn is_ios_simulator(device_name: &str, device_id: &str) -> bool {
    // Flutter device names for simulators include "Simulator"
    if device_name.to_lowercase().contains("simulator") {
        return true;
    }
    // Simulator UDIDs are standard UUIDs with hyphens (36 chars)
    // Physical UDIDs are 40-char hex without hyphens (or 24-char for newer devices)
    if device_id.len() == 36 && device_id.chars().filter(|c| *c == '-').count() == 4 {
        return true;
    }
    false
}
```

#### 3. Build `IosLogConfig` in actions

In `crates/fdemon-app/src/actions/native_logs.rs`, add the iOS config building branch alongside the existing Android and macOS branches:

```rust
#[cfg(target_os = "macos")]
use fdemon_daemon::native_logs::IosLogConfig;

// Inside the spawned async block:

#[cfg(target_os = "macos")]
let ios_config = if platform == "ios" {
    let process_name = derive_ios_process_name(&app_id);
    let is_simulator = is_ios_simulator(&device_name, &device_id);

    tracing::info!(
        "Starting iOS native log capture for session {} ({}, {})",
        session_id,
        if is_simulator { "simulator" } else { "physical" },
        process_name,
    );

    Some(IosLogConfig {
        device_udid: device_id.clone(),
        is_simulator,
        process_name,
        exclude_tags: exclude_tags.clone(),
        include_tags: include_tags.clone(),
        min_level: min_level.clone(),
    })
} else {
    None
};

// Update the create_native_log_capture call:
let capture = create_native_log_capture(
    &platform,
    android_config,
    #[cfg(target_os = "macos")]
    macos_config,
    #[cfg(target_os = "macos")]
    ios_config,
);
```

#### 4. Add `derive_ios_process_name()` helper

iOS Flutter apps use "Runner" as the default process name (same as macOS):

```rust
/// Derive the iOS process name from the Flutter app ID.
///
/// For iOS Flutter apps the process name is typically "Runner" (the default
/// binary name). When the app has been renamed in Xcode, it uses the product
/// name instead.
///
/// Uses the same logic as `derive_macos_process_name()` since both platforms
/// use the same naming convention.
fn derive_ios_process_name(app_id: &Option<String>) -> String {
    // iOS uses the same bundle ID / process name derivation as macOS.
    derive_macos_process_name(app_id)
}
```

Or simply reuse `derive_macos_process_name()` directly if adding a separate function is unnecessary.

#### 5. Pass `device_name` to the action

The `UpdateAction::StartNativeLogCapture` already has `device_id` but may not have `device_name`. Check if `device_name` is available in the session handle. If not, add it to the action payload:

```rust
// In message.rs, if device_name is not already available:
UpdateAction::StartNativeLogCapture {
    session_id: SessionId,
    platform: String,
    device_id: String,
    device_name: String,  // NEW — needed for simulator detection
    app_id: Option<String>,
    settings: NativeLogsSettings,
}
```

In `maybe_start_native_log_capture()`, populate from the session:

```rust
device_name: handle.session.device_name.clone().unwrap_or_default(),
```

#### 6. Update platform guard in `spawn_native_log_capture()`

The top-level platform guard in `spawn_native_log_capture()` currently only allows `"android"` and `"macos"`. Add `"ios"`:

```rust
if platform != "android" {
    #[cfg(not(target_os = "macos"))]
    {
        tracing::debug!(
            "Native log capture not supported on platform '{}' — skipping",
            platform,
        );
        return;
    }
    #[cfg(target_os = "macos")]
    if platform != "macos" && platform != "ios" {
        tracing::debug!(
            "Native log capture not supported on platform '{}' — skipping",
            platform,
        );
        return;
    }
}
```

### Acceptance Criteria

1. `maybe_start_native_log_capture()` emits `StartNativeLogCapture` for `platform == "ios"` sessions
2. `spawn_native_log_capture()` does not return early for `platform == "ios"` on macOS hosts
3. `IosLogConfig` is built with correct `device_udid`, `is_simulator`, `process_name` from session metadata
4. `is_ios_simulator()` correctly identifies simulators vs physical devices
5. `derive_ios_process_name()` extracts last bundle ID component, falls back to "Runner"
6. `create_native_log_capture()` receives `ios_config` and returns a capture backend
7. Tool availability guard checks `native_logs_available("ios")` before emitting the action
8. Double-start guard prevents spawning a second iOS capture on hot-restart
9. Existing Android and macOS capture paths are unaffected
10. `cargo check --workspace` compiles
11. `cargo test -p fdemon-app` passes

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ios_simulator_by_name() {
        assert!(is_ios_simulator("iPhone 15 Simulator", "some-id"));
        assert!(is_ios_simulator("iPad Air (5th generation) Simulator", "some-id"));
        assert!(!is_ios_simulator("Ed's iPhone", "some-id"));
    }

    #[test]
    fn test_is_ios_simulator_by_udid_format() {
        // Simulator UDID: standard UUID format
        assert!(is_ios_simulator("iPhone 15", "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE"));
        // Physical UDID: 40-char hex
        assert!(!is_ios_simulator("iPhone 15", "00008030000011ABC000DEF1234567890abcdef0"));
    }

    #[test]
    fn test_derive_ios_process_name_from_bundle_id() {
        assert_eq!(
            derive_ios_process_name(&Some("com.example.myApp".to_string())),
            "myApp"
        );
    }

    #[test]
    fn test_derive_ios_process_name_fallback() {
        assert_eq!(derive_ios_process_name(&None), "Runner");
    }

    // Test that maybe_start_native_log_capture returns Some for iOS platform
    #[test]
    fn test_maybe_start_native_log_capture_ios() {
        // Build a minimal AppState with an iOS session
        let mut state = AppState::default();
        state.settings.native_logs.enabled = true;

        // Create a mock session with platform "ios"
        // ... (follow existing test patterns from session.rs tests)

        // Verify the action is emitted for an AppStart message
    }
}
```

### Notes

- **Simulator detection heuristic**: The combination of device name and UDID format gives reliable detection. Flutter's `--machine` device discovery JSON output includes a `"category"` field for some devices, but the `Session` struct may not store it. The UDID format check (UUID with hyphens = simulator, hex string = physical) is a well-known iOS convention.
- **`device_name` availability**: Check if `handle.session.device_name` is an `Option<String>` or `String`. If it's not available in the action payload, it may need to be added to `UpdateAction::StartNativeLogCapture`. The alternative is to pass `is_simulator` as a pre-computed boolean in the action.
- **Process name**: Flutter iOS apps default to "Runner" as the process name. Custom product names in Xcode change this, but `derive_macos_process_name()` already handles this by extracting the last bundle ID component.
- **Tool availability**: The guard in `maybe_start_native_log_capture()` already calls `tool_availability.native_logs_available(platform)` which now returns `true` for `"ios"` when `xcrun_simctl || idevicesyslog` (from task 01).
- **All iOS code is `#[cfg(target_os = "macos")]` gated** since you can only develop for iOS from a macOS host.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Added `device_name: String` field to `UpdateAction::StartNativeLogCapture`; updated doc comment to include `"ios"` |
| `crates/fdemon-app/src/handler/session.rs` | Added `"ios"` to `needs_capture` guard in `maybe_start_native_log_capture()`; populate `device_name` from `handle.session.device_name` in returned action |
| `crates/fdemon-app/src/actions/mod.rs` | Added `device_name` to the `StartNativeLogCapture` destructuring and forwarded it to `spawn_native_log_capture()` |
| `crates/fdemon-app/src/actions/native_logs.rs` | Added `device_name` parameter to `spawn_native_log_capture()`; added `IosLogConfig` import (cfg-gated); added iOS config building branch; updated macOS platform guard to also allow `"ios"`; added `derive_ios_process_name()` and `is_ios_simulator()` helpers; added 8 new unit tests |

### Notable Decisions/Tradeoffs

1. **`derive_ios_process_name` delegates to `derive_macos_process_name`**: Both platforms use the same bundle ID convention, so reusing the existing function avoids duplication. The separate function name makes the call site self-documenting.

2. **`device_name` added to `UpdateAction::StartNativeLogCapture` rather than pre-computing `is_simulator`**: Carrying `device_name` in the action keeps the action data honest — it's plain metadata from the session. Pre-computing a boolean would push policy into the handler layer and make the action harder to inspect/debug.

3. **Double heuristic for simulator detection**: Device name check first (most reliable Flutter-side signal), then UDID format (well-known Apple convention). The combination covers edge cases where a user has renamed their device to include "iPhone 15" without "Simulator" but still has a UUID UDID.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1511 passed; 0 failed; 4 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Applied; no structural changes

### Risks/Limitations

1. **Simulator name heuristic**: If Flutter changes device naming (e.g., drops "Simulator" suffix), the name-based heuristic will fall through to the UDID format check, which is still reliable.
2. **Physical UDID length variation**: Newer Apple Silicon devices use 24-char UDIDs; the UDID-format heuristic will correctly fall through to `false` (physical) for these, which is correct.
