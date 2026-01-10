## Task: Update Device Cache During Auto-Launch

**Objective**: Ensure the global device cache is updated when auto-launch discovers devices, so subsequent dialogs show fresh data.

**Depends on**: Phase 2 complete

**Estimated Time**: 0.5 hours

### Scope

- `src/tui/spawn.rs`: Send cache update message after discovery
- `src/app/handler/update.rs`: Ensure cache is updated (may already work)

### Details

#### Current Device Cache Flow

When `ShowStartupDialog` or `ShowDeviceSelector` is triggered:
1. Handler returns `UpdateAction::DiscoverDevices`
2. Spawn function discovers devices
3. Sends `Message::DevicesDiscovered { devices }`
4. Handler updates `state.set_device_cache(devices)`

#### Required for Auto-Launch

The `spawn_auto_launch()` function discovers devices but doesn't send `DevicesDiscovered`. We need to either:

**Option A**: Send `DevicesDiscovered` from auto-launch spawn
```rust
// In spawn_auto_launch(), after successful discovery:
let _ = msg_tx.send(Message::DevicesDiscovered {
    devices: devices.clone(),
}).await;
```

**Option B**: Update cache in `AutoLaunchResult` handler
```rust
// In AutoLaunchResult handler, update cache from success:
Message::AutoLaunchResult { result } => {
    match result {
        Ok(success) => {
            // Update cache with the device we found
            // But we don't have full device list here...
        }
        // ...
    }
}
```

**Recommended: Option A** - Send `DevicesDiscovered` from spawn function.

#### Implementation

Update `spawn_auto_launch()` in `src/tui/spawn.rs`:

```rust
pub fn spawn_auto_launch(
    msg_tx: mpsc::Sender<Message>,
    configs: LoadedConfigs,
    project_path: PathBuf,
) {
    tokio::spawn(async move {
        // ... progress message ...

        let discovery_result = devices::discover_devices().await;

        let devices = match discovery_result {
            Ok(result) => {
                // NEW: Update device cache for future dialogs
                let _ = msg_tx.send(Message::DevicesDiscovered {
                    devices: result.devices.clone(),
                }).await;

                result.devices
            }
            Err(e) => {
                let _ = msg_tx.send(Message::AutoLaunchResult {
                    result: Err(e.to_string()),
                }).await;
                return;
            }
        };

        // ... rest of function ...
    });
}
```

### Acceptance Criteria

1. `DevicesDiscovered` message is sent after successful auto-launch discovery
2. Device cache is updated (verified by opening StartupDialog after auto-start)
3. Cache TTL behavior is preserved (30 second expiry)
4. `cargo check` passes
5. `cargo clippy -- -D warnings` passes

### Testing

Manual verification:
1. Start app with `auto_start=true`
2. Wait for session to start
3. Press '+' to open StartupDialog
4. Devices should appear immediately (from cache)
5. No "loading devices..." delay

Unit test (optional):
```rust
#[tokio::test]
async fn test_auto_launch_updates_device_cache() {
    // Would require mocking discover_devices()
    // May be better as integration test
}
```

### Notes

- The `DevicesDiscovered` handler already exists and updates the cache
- Sending this message is lightweight (just updates in-memory cache)
- This ensures consistency: any device discovery updates the cache
- The cache TTL (30s) means the cached data will be fresh

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Implementation Details:**

(pending)

**Testing Performed:**
- (pending)

**Notable Decisions:**
- (pending)

**Risks/Limitations:**
- (pending)
