## Task: Capture device supportability from discovery

**Objective**: Parse and store the `isSupported` flag (and optionally the `capabilities` object) that `flutter devices --machine` already emits per device, so downstream code can filter unrunnable targets. Backward-compatible: missing flag means "assume runnable."

**Depends on**: None

**Estimated Time**: 1–2h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/devices.rs`: Add `is_supported: bool` field (serde `default = true`) and an optional `capabilities: Option<DeviceCapabilities>` field to the `Device` struct; add a `DeviceCapabilities` struct (`hot_reload`, `hot_restart`); update the test fixture builder `sample_device` and add parsing tests.

**Files Read (Dependencies):**
- None (self-contained struct + parser change).

### Details

The `Device` struct (`devices.rs:19-54`) is `#[derive(Debug, Clone, Deserialize, Serialize)]` with `#[serde(rename_all = "camelCase")]`. `flutter devices --machine` emits `isSupported` and a `capabilities: { "hotReload": bool, "hotRestart": bool }` object (confirmed in the existing `test_parse_devices_with_target_platform` fixture at `devices.rs:417-424`, where these keys are currently silently discarded).

Add to `Device`:

```rust
/// Whether the Flutter toolchain can run this device for the current project.
/// `flutter devices --machine` emits `isSupported`; older/abbreviated payloads
/// and daemon `device.added` events omit it — default true keeps them visible.
#[serde(default = "default_is_supported")]
pub is_supported: bool,

/// Per-device capability flags (hot reload / hot restart). Optional; absent on
/// abbreviated payloads. Parse-and-store only for now (future UI use).
#[serde(default)]
pub capabilities: Option<DeviceCapabilities>,
```

Helper + struct (module level):

```rust
fn default_is_supported() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCapabilities {
    #[serde(default)]
    pub hot_reload: bool,
    #[serde(default)]
    pub hot_restart: bool,
}
```

**Update the test fixture builder** `sample_device` (`devices.rs:315-326`) to set the two new fields (`is_supported: true`, `capabilities: None`) so every existing test that uses it keeps compiling. There is no `#[serde(deny_unknown_fields)]`, so this change does not break any other deserialization.

**Re-export note:** `Device` (and therefore the new `DeviceCapabilities`) is re-exported by `fdemon-app/src/lib.rs:118-120` and consumed by `fdemon-tui`. If `DeviceCapabilities` needs to be public to those crates, ensure it is `pub` and exported alongside `Device` — but the downstream tasks (02, 03) only need `is_supported`, so exporting `DeviceCapabilities` is optional this phase.

### Acceptance Criteria

1. `Device` has `is_supported: bool` defaulting to `true` when the JSON key is absent.
2. `Device` has `capabilities: Option<DeviceCapabilities>` populated from the `capabilities` object when present, `None` when absent.
3. `isSupported: false` in JSON deserializes to `is_supported == false`.
4. All existing `devices.rs` tests still pass (fixture builder updated).
5. `cargo test -p fdemon-daemon`, `cargo fmt`, `cargo clippy -p fdemon-daemon` pass.

### Testing

Extend the existing test module (`devices.rs:310+`). Add focused parsing tests:

```rust
#[test]
fn test_is_supported_defaults_true_when_absent() {
    let output = r#"[{"id":"x","name":"X","platform":"ios","emulator":false}]"#;
    let devices = parse_devices_output(output).unwrap();
    assert!(devices[0].is_supported, "absent isSupported must default to true");
    assert!(devices[0].capabilities.is_none());
}

#[test]
fn test_is_supported_false_is_parsed() {
    let output = r#"[{"id":"x","name":"X","targetPlatform":"web-javascript",
        "emulator":false,"isSupported":false}]"#;
    let devices = parse_devices_output(output).unwrap();
    assert!(!devices[0].is_supported);
}

#[test]
fn test_capabilities_parsed_when_present() {
    let output = r#"[{"id":"x","name":"X","targetPlatform":"ios","emulator":false,
        "isSupported":true,"capabilities":{"hotReload":true,"hotRestart":true}}]"#;
    let devices = parse_devices_output(output).unwrap();
    let caps = devices[0].capabilities.as_ref().unwrap();
    assert!(caps.hot_reload && caps.hot_restart);
}
```

Also strengthen `test_parse_devices_with_target_platform` (`devices.rs:410`) to assert `devices[0].is_supported` and that `devices[0].capabilities` carries the hot-reload/restart flags now that they are no longer discarded.

### Notes

- **Why default true:** the daemon `device.added` live-event path and any abbreviated payload omit `isSupported`; defaulting true guarantees filtering only ever *removes* explicitly-unsupported devices, never accidentally hides one. See plan "Runnable-device filtering" risks.
- The live `device.added` path uses a separate `DeviceInfo` struct (`fdemon-core/src/events.rs:93`) that never reaches the dialog, so no change is needed there.
- Keep `capabilities` parse-and-store only; do not wire it into any UI this phase (Future Enhancement).
