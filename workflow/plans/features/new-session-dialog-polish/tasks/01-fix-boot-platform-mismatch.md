# Task 01: Fix Boot Platform Mismatch

## Objective

Fix the case mismatch bug that causes "Unknown platform: ios/android" errors when booting emulators/simulators.

## Priority

**Critical** - Completely blocks emulator/simulator boot functionality

## Problem

When user presses `b` to boot a device from the Bootable tab:

```
Failed to boot 6488AF1E-BC33-445B-90BB-564A3AB30F89: Unknown platform: ios
Failed to boot Pixel_9_Pro_Fold: Unknown platform: android
```

**Root Cause:** Case mismatch between platform strings:
- Handler sends: `"ios"`, `"android"` (lowercase)
- Spawner expects: `"iOS"`, `"Android"` (capitalized)

## Solution

Use the existing `Platform` enum instead of raw strings for type-safe platform handling.

### Step 1: Update Message Type

**File:** `src/app/message.rs`

Change `BootDevice` message to use `Platform` enum:

```rust
// Before
BootDevice { device_id: String, platform: String },

// After
BootDevice { device_id: String, platform: crate::core::Platform },
```

### Step 2: Update UpdateAction Type

**File:** `src/app/handler/mod.rs`

Change `UpdateAction::BootDevice` to use `Platform` enum:

```rust
// Before
BootDevice { device_id: String, platform: String },

// After
BootDevice { device_id: String, platform: crate::core::Platform },
```

### Step 3: Fix Handler Platform Extraction

**File:** `src/app/handler/new_session/target_selector.rs` (lines 51-62)

Use `Platform` enum instead of string literals:

```rust
// Before (lines 51-58)
let (device_id, platform) = match device {
    GroupedBootableDevice::IosSimulator(sim) => {
        (sim.udid.clone(), "ios".to_string())  // BUG
    }
    GroupedBootableDevice::AndroidAvd(avd) => {
        (avd.name.clone(), "android".to_string())  // BUG
    }
};

// After
use crate::core::Platform;
let (device_id, platform) = match device {
    GroupedBootableDevice::IosSimulator(sim) => {
        (sim.udid.clone(), Platform::IOS)
    }
    GroupedBootableDevice::AndroidAvd(avd) => {
        (avd.name.clone(), Platform::Android)
    }
};
```

### Step 4: Update Spawn Function

**File:** `src/tui/spawn.rs` (lines 295-330)

Change signature and match on enum:

```rust
// Before
pub async fn spawn_device_boot(
    device_id: String,
    platform: String,  // String type
    ...
) {
    let result = match platform.as_str() {
        "iOS" => ...,
        "Android" => ...,
        _ => { /* Unknown platform error */ }
    };
}

// After
pub async fn spawn_device_boot(
    device_id: String,
    platform: crate::core::Platform,  // Enum type
    ...
) {
    let result = match platform {
        Platform::IOS => crate::daemon::boot_simulator(&device_id).await,
        Platform::Android => crate::daemon::boot_avd(&device_id, &tool_availability).await,
    };
    // No default case needed - enum is exhaustive!
}
```

### Step 5: Update Action Handler

**File:** `src/tui/actions.rs` (around line 101)

Update the action dispatch to pass `Platform` enum:

```rust
UpdateAction::BootDevice { device_id, platform } => {
    spawn_device_boot(device_id, platform, tool_availability.clone(), msg_tx.clone());
}
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/app/message.rs` | Change `BootDevice.platform` to `Platform` enum |
| `src/app/handler/mod.rs` | Change `UpdateAction::BootDevice.platform` to `Platform` enum |
| `src/app/handler/new_session/target_selector.rs` | Use `Platform::IOS` and `Platform::Android` |
| `src/tui/spawn.rs` | Update function signature and match on enum |
| `src/tui/actions.rs` | Update action dispatch |

## Acceptance Criteria

1. iOS simulator boots when pressing `b` on Bootable tab
2. Android AVD boots when pressing `b` on Bootable tab
3. Booted device appears in Connected tab after boot completes
4. No "Unknown platform" errors in logs
5. `cargo check` passes (no type errors)
6. `cargo clippy` passes

## Testing

```bash
cargo check
cargo test boot
cargo clippy -- -D warnings
```

**Manual Testing:**
1. Open NewSessionDialog, switch to Bootable tab
2. Select an iOS simulator, press `b` → Should boot
3. Select an Android AVD, press `b` → Should boot
4. After boot, switch to Connected tab → Should see booted device

## Notes

- The `Platform` enum already exists at `src/core/types.rs:625-637`
- This change makes the code type-safe - the compiler will catch any future mismatches
- No new code paths - just replacing string matching with enum matching

---

## Completion Summary

**Status:** Not Started
