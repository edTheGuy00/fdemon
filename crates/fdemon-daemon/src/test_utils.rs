//! Test utilities for daemon types
//!
//! Provides helper functions for creating test Device objects.

use super::Device;

/// Creates a test device with basic defaults.
///
/// # Arguments
/// * `id` - Device identifier
/// * `name` - Human-readable device name
///
/// # Returns
/// A Device with iOS platform, non-emulator defaults.
pub fn test_device(id: &str, name: &str) -> Device {
    test_device_full(id, name, "ios", false)
}

/// Creates a test device with platform specification.
///
/// # Arguments
/// * `id` - Device identifier
/// * `name` - Human-readable device name
/// * `platform` - Platform string (e.g., "ios", "android", "macos")
pub fn test_device_with_platform(id: &str, name: &str, platform: &str) -> Device {
    test_device_full(id, name, platform, false)
}

/// Creates a test device with full control over all fields.
///
/// # Arguments
/// * `id` - Device identifier
/// * `name` - Human-readable device name
/// * `platform` - Platform string
/// * `emulator` - Whether this is an emulator/simulator
pub fn test_device_full(id: &str, name: &str, platform: &str, emulator: bool) -> Device {
    Device {
        id: id.to_string(),
        name: name.to_string(),
        platform: platform.to_string(),
        emulator,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_basic() {
        let device = test_device("test-id", "Test Device");
        assert_eq!(device.id, "test-id");
        assert_eq!(device.name, "Test Device");
        assert_eq!(device.platform, "ios");
        assert!(!device.emulator);
    }

    #[test]
    fn test_device_with_platform_android() {
        let device = test_device_with_platform("android-id", "Android Device", "android");
        assert_eq!(device.platform, "android");
    }

    #[test]
    fn test_device_full_emulator() {
        let device = test_device_full("sim-id", "Simulator", "ios", true);
        assert!(device.emulator);
    }
}
