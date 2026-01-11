## Task: Define BootableDevice Type

**Objective**: Create the `BootableDevice` type for representing offline simulators and AVDs.

**Depends on**: None

**Estimated Time**: 15 minutes

### Background

The new dialog needs to display offline (bootable) devices separately from connected devices. These devices come from native commands (`xcrun simctl list`, `emulator -list-avds`) and have different properties than Flutter's `Device` type.

### Scope

- `src/core/types.rs`: Add `BootableDevice`, `Platform`, `DeviceState` types

### Changes Required

**Add to `src/core/types.rs`:**

```rust
/// Platform type for bootable devices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    IOS,
    Android,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::IOS => write!(f, "iOS"),
            Platform::Android => write!(f, "Android"),
        }
    }
}

/// State of a bootable device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceState {
    #[default]
    Shutdown,
    Booted,
    Booting,
    ShuttingDown,
    Unknown,
}

impl std::fmt::Display for DeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceState::Shutdown => write!(f, "Shutdown"),
            DeviceState::Booted => write!(f, "Booted"),
            DeviceState::Booting => write!(f, "Booting"),
            DeviceState::ShuttingDown => write!(f, "Shutting Down"),
            DeviceState::Unknown => write!(f, "Unknown"),
        }
    }
}

/// A bootable device (offline simulator or AVD)
///
/// Unlike `Device` which represents connected/running devices from Flutter,
/// this represents devices that can be booted but aren't currently running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootableDevice {
    /// Unique identifier (UDID for iOS, AVD name for Android)
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Platform (iOS or Android)
    pub platform: Platform,

    /// Runtime version (e.g., "iOS 17.2", "API 33")
    pub runtime: String,

    /// Current state (Shutdown, Booted, etc.)
    pub state: DeviceState,
}

impl BootableDevice {
    /// Create a new bootable device
    pub fn new(id: impl Into<String>, name: impl Into<String>, platform: Platform, runtime: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            platform,
            runtime: runtime.into(),
            state: DeviceState::Shutdown,
        }
    }

    /// Check if this device can be booted
    pub fn can_boot(&self) -> bool {
        matches!(self.state, DeviceState::Shutdown | DeviceState::Unknown)
    }

    /// Get display string for the device
    pub fn display_string(&self) -> String {
        format!("{} ({})", self.name, self.runtime)
    }
}
```

**Update `src/core/mod.rs` exports:**

```rust
pub use types::{BootableDevice, DeviceState, Platform};
```

### Acceptance Criteria

1. `BootableDevice` struct defined with all fields
2. `Platform` enum with iOS and Android variants
3. `DeviceState` enum with Shutdown, Booted, Booting, ShuttingDown, Unknown variants
4. Display implementations for all enums
5. `can_boot()` helper method
6. Types exported from `core` module
7. `cargo check` passes
8. `cargo clippy -- -D warnings` passes

### Testing

Add inline tests:

```rust
#[cfg(test)]
mod bootable_device_tests {
    use super::*;

    #[test]
    fn test_bootable_device_can_boot() {
        let device = BootableDevice::new("id", "iPhone 15", Platform::IOS, "iOS 17.2");
        assert!(device.can_boot());

        let mut booted = device.clone();
        booted.state = DeviceState::Booted;
        assert!(!booted.can_boot());
    }

    #[test]
    fn test_display_string() {
        let device = BootableDevice::new("id", "Pixel 8", Platform::Android, "API 34");
        assert_eq!(device.display_string(), "Pixel 8 (API 34)");
    }
}
```

### Notes

- This type is separate from `daemon::Device` which comes from Flutter
- Will be used by the native discovery functions in Phase 2
- State tracking allows showing boot progress in the UI
