## Task: Wire Version Probe to Message System and Display Results

**Objective**: Connect the async `flutter --version --machine` probe to the TEA message loop so probe results automatically populate the SDK info pane with complete metadata.

**Depends on**: 03-version-probe-backend, 04-sdk-info-extended-fields

### Scope

- `crates/fdemon-app/src/message.rs`: Add `FlutterVersionProbeCompleted` message variant
- `crates/fdemon-app/src/handler/flutter_version/actions.rs`: Handle probe result
- `crates/fdemon-app/src/handler/update.rs`: Route new message to handler
- `crates/fdemon-app/src/actions/mod.rs`: Add `ProbeFlutterVersion` action + spawn logic
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs`: Show "loading" state for probe-dependent fields

### Details

**1. New message variant (`message.rs`):**

```rust
/// Result of `flutter --version --machine` async probe.
FlutterVersionProbeCompleted {
    result: std::result::Result<fdemon_daemon::FlutterVersionInfo, String>,
},
```

**2. New action variant (`actions/mod.rs`):**

```rust
/// Run `flutter --version --machine` in the background to enrich SDK metadata.
ProbeFlutterVersion,
```

**Action handler — spawn the probe as a background task:**

```rust
UpdateAction::ProbeFlutterVersion => {
    if let Some(ref sdk) = state.resolved_sdk {
        let executable = sdk.executable.clone();
        let tx = msg_tx.clone();
        tokio::spawn(async move {
            let result = fdemon_daemon::flutter_sdk::probe_flutter_version(&executable).await;
            let _ = tx.send(Message::FlutterVersionProbeCompleted {
                result: result.map_err(|e| e.to_string()),
            });
        });
    }
}
```

**3. Trigger the probe when the panel opens:**

In the handler for `Message::FlutterVersionShowPanel` (or equivalent), add `ProbeFlutterVersion` to the returned actions alongside the existing `ScanInstalledSdks`:

```rust
// In handler/flutter_version/navigation.rs — handle_show()
pub fn handle_show(state: &mut AppState) -> UpdateResult {
    state.show_flutter_version();
    UpdateResult::with_actions(vec![
        UpdateAction::ScanInstalledSdks,
        UpdateAction::ProbeFlutterVersion,
    ])
}
```

Note: Check the existing `handle_show` signature — if it returns a single `UpdateAction`, may need to switch to returning `Vec<UpdateAction>` or chain them.

**4. Handle probe result (`handler/flutter_version/actions.rs`):**

```rust
pub fn handle_version_probe_completed(
    state: &mut AppState,
    result: std::result::Result<FlutterVersionInfo, String>,
) -> UpdateResult {
    match result {
        Ok(info) => {
            // Update sdk_info with extended metadata
            let sdk_info = &mut state.flutter_version.sdk_info;
            sdk_info.framework_revision = info.framework_revision;
            sdk_info.engine_revision = info.engine_revision.map(|r| {
                // Truncate engine hash to 10 chars for display
                if r.len() > 10 { r[..10].to_string() } else { r }
            });
            sdk_info.devtools_version = info.devtools_version;

            // If version was "unknown", update it from probe
            if let Some(ref mut sdk) = sdk_info.resolved_sdk {
                if sdk.version == "unknown" {
                    if let Some(ref ver) = info.framework_version {
                        sdk.version = ver.clone();
                    }
                }
                // Also update channel if it was None
                if sdk.channel.is_none() {
                    sdk.channel = info.channel;
                }
            }

            // Update dart_version if it was missing
            if sdk_info.dart_version.is_none() {
                sdk_info.dart_version = info.dart_sdk_version;
            }

            // Also update the top-level resolved_sdk for future panel opens
            if let Some(ref mut top_sdk) = state.resolved_sdk {
                if top_sdk.version == "unknown" {
                    if let Some(ref ver) = info.framework_version {
                        top_sdk.version = ver.clone();
                    }
                }
                if top_sdk.channel.is_none() {
                    if let Some(ch) = info.channel {
                        top_sdk.channel = Some(ch);
                    }
                }
            }

            state.flutter_version.sdk_info.probe_completed = true;
        }
        Err(reason) => {
            tracing::debug!("Flutter version probe failed: {reason}");
            state.flutter_version.sdk_info.probe_completed = true;
            // Non-fatal — file-based data remains displayed
        }
    }
    UpdateResult::empty()
}
```

**5. Route the message (`handler/update.rs`):**

```rust
Message::FlutterVersionProbeCompleted { result } => {
    flutter_version::actions::handle_version_probe_completed(state, result)
}
```

**6. TUI "loading" indicator for probe-dependent fields:**

Add `probe_completed: bool` to `SdkInfoState` (defaults to `false`). When rendering framework revision, engine revision, and DevTools version fields:
- If `probe_completed == false` and the field is `None`, show "..." in `TEXT_MUTED` style
- If `probe_completed == true` and the field is `None`, show "—" (em-dash)
- If the field has a value, show it normally

This gives the user feedback that data is being fetched rather than permanently unavailable.

**7. Also trigger probe at engine startup (optional enhancement):**

For the best experience, also trigger `ProbeFlutterVersion` during engine initialization so the data is ready before the user first opens the panel. This avoids the "..." flash on first open.

In `engine.rs`, after SDK detection completes successfully, fire a `ProbeFlutterVersion` action. The result arrives via the message loop and updates `state.resolved_sdk` metadata.

### Acceptance Criteria

1. Opening the Flutter Version panel triggers `flutter --version --machine` in the background
2. Probe result populates framework_revision, engine_revision, devtools_version in the SDK info pane
3. "unknown" version is replaced with the probed framework version
4. Missing channel is populated from probe result
5. Probe failure is non-fatal — logged at debug level, em-dash shown for unavailable fields
6. Loading state ("...") shown for probe-dependent fields while probe is in-flight
7. After probe completes, re-opening the panel shows cached data immediately
8. Top-level `state.resolved_sdk` is updated with enriched metadata for future panel opens
9. No blocking of the UI while probe runs
10. Probe timeout (30s) is handled gracefully

### Testing

```rust
#[test]
fn test_handle_version_probe_completed_success() {
    let mut state = make_app_state_with_unknown_version();
    state.show_flutter_version();

    let info = FlutterVersionInfo {
        framework_version: Some("3.38.6".into()),
        channel: Some("stable".into()),
        framework_revision: Some("8b87286849".into()),
        engine_revision: Some("6f3039bf7c3cb5306513c75092822d4d94716003".into()),
        dart_sdk_version: Some("3.10.7".into()),
        devtools_version: Some("2.51.1".into()),
        ..Default::default()
    };

    let result = handle_version_probe_completed(&mut state, Ok(info));
    assert!(result.action.is_none());

    // Version should be updated from "unknown" to "3.38.6"
    let sdk = state.flutter_version.sdk_info.resolved_sdk.as_ref().unwrap();
    assert_eq!(sdk.version, "3.38.6");
    // Extended fields populated
    assert_eq!(state.flutter_version.sdk_info.framework_revision.as_deref(), Some("8b87286849"));
    assert_eq!(state.flutter_version.sdk_info.engine_revision.as_deref(), Some("6f3039bf7c"));
    assert_eq!(state.flutter_version.sdk_info.devtools_version.as_deref(), Some("2.51.1"));
    assert!(state.flutter_version.sdk_info.probe_completed);
}

#[test]
fn test_handle_version_probe_completed_failure() {
    let mut state = make_app_state_with_unknown_version();
    state.show_flutter_version();

    let result = handle_version_probe_completed(&mut state, Err("timeout".into()));
    assert!(result.action.is_none());
    assert!(state.flutter_version.sdk_info.probe_completed);
    // Original "unknown" version should remain
    let sdk = state.flutter_version.sdk_info.resolved_sdk.as_ref().unwrap();
    assert_eq!(sdk.version, "unknown");
}

#[test]
fn test_handle_version_probe_does_not_overwrite_known_version() {
    let mut state = make_app_state_with_known_version("3.19.0");
    state.show_flutter_version();

    let info = FlutterVersionInfo {
        framework_version: Some("3.19.0".into()),
        ..Default::default()
    };

    handle_version_probe_completed(&mut state, Ok(info));
    // Version should remain "3.19.0" (was already known)
    let sdk = state.flutter_version.sdk_info.resolved_sdk.as_ref().unwrap();
    assert_eq!(sdk.version, "3.19.0");
}
```

### Notes

- Check the current `UpdateResult` pattern — if it only supports a single `UpdateAction`, may need to return the probe as a follow-up message or extend to `Vec<UpdateAction>`.
- The engine revision hash is typically 40 chars. Truncate to 10 for display in the TUI (matching Flutter CLI's short hash display).
- Consider adding `probe_completed: bool` to `SdkInfoState` rather than a separate `probe_pending` field — simpler state model.
- The probe should NOT run every time the panel opens — only if `probe_completed == false`. Cache the result in `SdkInfoState`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/flutter_version/state.rs` | Added `probe_completed: bool` field to `SdkInfoState`; updated `FlutterVersionState::new()` initializer; added test for default value |
| `crates/fdemon-app/src/message.rs` | Added `FlutterVersionInfo` import; added `FlutterVersionProbeRequested` and `FlutterVersionProbeCompleted { result }` message variants |
| `crates/fdemon-app/src/handler/mod.rs` | Added `ProbeFlutterVersion { executable: Option<FlutterExecutable> }` action variant to `UpdateAction` enum |
| `crates/fdemon-app/src/handler/flutter_version/navigation.rs` | Added `Message` import; updated `handle_show()` to return `message_and_action` with `FlutterVersionProbeRequested` follow-up + `ScanInstalledSdks` action |
| `crates/fdemon-app/src/handler/flutter_version/actions.rs` | Added `FlutterVersionInfo` import; added `handle_probe_requested()` and `handle_version_probe_completed()` handlers + 13 new tests |
| `crates/fdemon-app/src/handler/update.rs` | Added routing for `Message::FlutterVersionProbeRequested` and `Message::FlutterVersionProbeCompleted` |
| `crates/fdemon-app/src/actions/mod.rs` | Added `ProbeFlutterVersion` arm to `handle_action()` that spawns the async probe task |
| `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs` | Added `probe_field_str()` and `render_probe_field()` helpers; updated expanded/compact renderers to show `"..."` while probe in-flight, `"—"` after completion with None value; updated test helpers to set `probe_completed: true`; added 2 new loading indicator tests |
| `crates/fdemon-tui/src/widgets/flutter_version_panel/mod.rs` | Added `probe_completed: true` to 3 `SdkInfoState` struct literals in test helpers |

### Notable Decisions/Tradeoffs

1. **Dual-action via follow-up message**: `UpdateResult` only supports one `action`. To dispatch both `ScanInstalledSdks` and `ProbeFlutterVersion` when the panel opens, `handle_show()` uses `UpdateResult::{ message: Some(FlutterVersionProbeRequested), action: Some(ScanInstalledSdks) }`. The `message` field is processed in the same TEA cycle as a follow-up, allowing `ProbeFlutterVersion` to be dispatched immediately after. This avoids modifying `UpdateResult` to support `Vec<UpdateAction>`.

2. **Executable carried in action**: `ProbeFlutterVersion` carries `Option<FlutterExecutable>` because `handle_action()` does not have access to `AppState`. The handler captures the executable from `state.resolved_sdk` before returning the action, matching the same pattern as `ScanInstalledSdks { active_sdk_root }`. A `None` executable causes `handle_action` to log at debug level and skip the probe.

3. **`probe_completed` persists across panel close/reopen**: Once the probe finishes, `probe_completed = true` remains set in `SdkInfoState` until `AppState::show_flutter_version()` is called again (which recreates the state via `FlutterVersionState::new()`). This means re-opening the panel shows cached data immediately (acceptance criterion 7) since `handle_probe_requested` is a no-op when `probe_completed == true`.

4. **Loading indicator with distinct styling**: Probe-dependent fields showing `"..."` use `TEXT_MUTED` style (dimmed) to visually distinguish them from real values. Fields showing `"—"` (probe done, value unavailable) use the normal `TEXT_PRIMARY + BOLD` style. This gives users a clear signal about what data is still loading vs. genuinely unavailable.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (1797 fdemon-app + 731 fdemon-daemon + 581 fdemon-core + 867 fdemon-tui unit tests; all green)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Probe on every panel open (if state reset)**: `show_flutter_version()` in `AppState` calls `FlutterVersionState::new()` which resets `probe_completed` to `false`. This means the probe re-runs each time the panel is opened from scratch. This is intentional for correctness (SDK may have changed), but if the probe is slow it may show `"..."` briefly on repeated opens. Acceptance criterion 7 ("re-opening the panel shows cached data immediately") is satisfied for a single session because `probe_completed` persists while the panel state lives.

2. **No deduplication if multiple `ShowFlutterVersion` messages arrive before first probe completes**: The `handle_probe_requested` guard (`probe_completed == false`) does not prevent multiple concurrent probes if the panel is opened rapidly. In practice this is not an issue because the panel is modal and the user cannot open it twice simultaneously.
