# Task: Boot Commands

## Summary

Implement commands to boot iOS simulators and Android AVDs. These commands start the device/emulator and return when it's ready.

## Files

| File | Action |
|------|--------|
| `src/daemon/simulators.rs` | Modify (add boot function) |
| `src/daemon/avds.rs` | Modify (add boot function) |

## Implementation

### 1. iOS Simulator Boot

```rust
// src/daemon/simulators.rs

use tokio::process::Command;
use tokio::time::{timeout, Duration};
use crate::common::Error;

/// Boot an iOS simulator by UDID
///
/// Returns Ok(()) when the simulator is booted and ready.
/// Returns error if boot fails or times out.
pub async fn boot_simulator(udid: &str) -> Result<(), Error> {
    // First check if already booted
    if is_simulator_booted(udid).await? {
        return Ok(());
    }

    // Boot the simulator
    let output = Command::new("xcrun")
        .args(["simctl", "boot", udid])
        .output()
        .await
        .map_err(|e| Error::recoverable(format!("Failed to boot simulator: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "Unable to boot device in current state: Booted" is not an error
        if !stderr.contains("Booted") {
            return Err(Error::recoverable(format!(
                "Failed to boot simulator: {}",
                stderr
            )));
        }
    }

    // Wait for simulator to be fully booted
    wait_for_simulator_boot(udid, Duration::from_secs(60)).await?;

    // Open Simulator.app to show the UI
    let _ = Command::new("open")
        .args(["-a", "Simulator"])
        .output()
        .await;

    Ok(())
}

/// Check if a simulator is already booted
async fn is_simulator_booted(udid: &str) -> Result<bool, Error> {
    let simulators = list_ios_simulators().await?;
    Ok(simulators
        .iter()
        .any(|s| s.udid == udid && s.state == SimulatorState::Booted))
}

/// Wait for simulator to finish booting
async fn wait_for_simulator_boot(udid: &str, max_wait: Duration) -> Result<(), Error> {
    let poll_interval = Duration::from_millis(500);
    let start = std::time::Instant::now();

    while start.elapsed() < max_wait {
        if is_simulator_booted(udid).await? {
            return Ok(());
        }
        tokio::time::sleep(poll_interval).await;
    }

    Err(Error::recoverable("Simulator boot timed out"))
}

/// Shutdown an iOS simulator
pub async fn shutdown_simulator(udid: &str) -> Result<(), Error> {
    let output = Command::new("xcrun")
        .args(["simctl", "shutdown", udid])
        .output()
        .await
        .map_err(|e| Error::recoverable(format!("Failed to shutdown simulator: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Ignore "Unable to shutdown device in current state: Shutdown"
        if !stderr.contains("Shutdown") {
            return Err(Error::recoverable(format!(
                "Failed to shutdown simulator: {}",
                stderr
            )));
        }
    }

    Ok(())
}
```

### 2. Android AVD Boot

```rust
// src/daemon/avds.rs

use tokio::process::Command;
use tokio::time::Duration;
use crate::common::Error;
use crate::daemon::ToolAvailability;

/// Boot an Android AVD by name
///
/// Launches the emulator in the background and returns immediately.
/// The emulator process continues running independently.
pub async fn boot_avd(
    avd_name: &str,
    tool_availability: &ToolAvailability,
) -> Result<(), Error> {
    let emulator_cmd = tool_availability
        .emulator_path
        .as_deref()
        .ok_or_else(|| Error::recoverable("Android emulator not available"))?;

    // Start emulator in background
    // Using spawn() instead of output() so it doesn't wait
    let mut child = tokio::process::Command::new(emulator_cmd)
        .args([
            "-avd", avd_name,
            "-no-snapshot-load", // Start fresh
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| Error::recoverable(format!("Failed to start emulator: {}", e)))?;

    // Detach the child process so it continues running
    // We don't wait for it to complete
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    // Wait a moment for the emulator to start initializing
    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(())
}

/// Check if an AVD is currently running
///
/// Uses `adb devices` to check for running emulators.
pub async fn is_avd_running(avd_name: &str) -> Result<bool, Error> {
    // This is tricky because we need to map AVD name to emulator serial
    // For now, we'll just check if any emulator is running
    // A more complete solution would check the emulator's console port

    let output = Command::new("adb")
        .args(["devices", "-l"])
        .output()
        .await
        .map_err(|e| Error::recoverable(format!("Failed to run adb: {}", e)))?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Look for emulator entries
    // Format: "emulator-5554    device product:sdk_gphone64_x86_64 model:sdk_gphone64_x86_64 device:emu64x transport_id:1"
    Ok(stdout.lines().any(|line| line.starts_with("emulator-")))
}

/// Kill all running emulators
pub async fn kill_all_emulators() -> Result<(), Error> {
    let _ = Command::new("adb")
        .args(["emu", "kill"])
        .output()
        .await;

    Ok(())
}
```

### 3. Unified boot interface

```rust
// src/daemon/mod.rs or new file

use crate::daemon::{IosSimulator, AndroidAvd, ToolAvailability};
use crate::common::Error;

/// Platform-agnostic bootable device
#[derive(Debug, Clone)]
pub enum BootableDevice {
    IosSimulator(IosSimulator),
    AndroidAvd(AndroidAvd),
}

impl BootableDevice {
    pub fn id(&self) -> &str {
        match self {
            BootableDevice::IosSimulator(s) => &s.udid,
            BootableDevice::AndroidAvd(a) => &a.name,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            BootableDevice::IosSimulator(s) => &s.name,
            BootableDevice::AndroidAvd(a) => &a.display_name,
        }
    }

    pub fn platform(&self) -> &'static str {
        match self {
            BootableDevice::IosSimulator(_) => "iOS",
            BootableDevice::AndroidAvd(_) => "Android",
        }
    }

    pub fn runtime_info(&self) -> String {
        match self {
            BootableDevice::IosSimulator(s) => s.runtime.clone(),
            BootableDevice::AndroidAvd(a) => {
                a.api_level
                    .map(|api| format!("API {}", api))
                    .unwrap_or_else(|| "Unknown API".to_string())
            }
        }
    }

    /// Boot this device
    pub async fn boot(&self, tool_availability: &ToolAvailability) -> Result<(), Error> {
        match self {
            BootableDevice::IosSimulator(s) => {
                crate::daemon::simulators::boot_simulator(&s.udid).await
            }
            BootableDevice::AndroidAvd(a) => {
                crate::daemon::avds::boot_avd(&a.name, tool_availability).await
            }
        }
    }
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Note: Most boot tests require actual simulators/emulators
    // These are integration tests that should be run manually

    #[test]
    fn test_bootable_device_display_name() {
        let sim = IosSimulator {
            udid: "123".to_string(),
            name: "iPhone 15 Pro".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15 Pro".to_string(),
        };

        let device = BootableDevice::IosSimulator(sim);
        assert_eq!(device.display_name(), "iPhone 15 Pro");
        assert_eq!(device.platform(), "iOS");
    }

    #[test]
    fn test_avd_runtime_info() {
        let avd = AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        };

        let device = BootableDevice::AndroidAvd(avd);
        assert_eq!(device.runtime_info(), "API 33");
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test boot && cargo clippy -- -D warnings
```

## Notes

- iOS boot waits for completion; Android boot is fire-and-forget
- Android emulators take longer to boot, so we don't wait
- The device will appear in `flutter devices` once fully booted
- Consider adding a "Booting..." indicator in the UI

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/daemon/simulators.rs` | Added `boot_simulator()`, `is_simulator_booted()`, `wait_for_simulator_boot()`, and `shutdown_simulator()` functions |
| `src/daemon/avds.rs` | Added `boot_avd()`, `is_avd_running()`, and `kill_all_emulators()` functions |
| `src/daemon/mod.rs` | Added `BootableDevice` enum with methods `id()`, `display_name()`, `platform()`, `runtime_info()`, and `boot()`. Updated exports to include new boot functions and BootableDevice. Added comprehensive unit tests. |

### Notable Decisions/Tradeoffs

1. **iOS boot is synchronous**: The `boot_simulator()` function waits up to 60 seconds for the simulator to boot completely before returning. This ensures the simulator is ready for Flutter to connect.

2. **Android boot is asynchronous**: The `boot_avd()` function spawns the emulator process and returns immediately after a 2-second initialization delay. This is because Android emulators take much longer to boot (30-60+ seconds) and we don't want to block the UI.

3. **Unified interface with BootableDevice**: Created a platform-agnostic enum that wraps both iOS simulators and Android AVDs, providing a consistent interface for booting devices regardless of platform.

4. **Error handling follows project patterns**: Used `Error::process()` constructor for process-related errors, maintaining consistency with existing daemon module error handling.

5. **Idempotent boot operations**: Both boot functions check if the device is already running and return successfully without error, making them safe to call multiple times.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (no compilation errors)
- `cargo test boot` - Passed (2 tests: test_bootable_device_display_name, test_bootable_device_can_boot)
- `cargo test --lib daemon::tests` - Passed (4 tests covering BootableDevice functionality)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **iOS simulator boot timeout**: The 60-second timeout may not be sufficient for slower machines or complex simulator configurations. Users may need to adjust this value in future iterations.

2. **Android AVD detection is simplistic**: The `is_avd_running()` function checks for any emulator, not a specific AVD. A more robust implementation would query the emulator console port to match AVD names to running instances.

3. **macOS-only for iOS**: iOS simulator boot is only available on macOS. The implementation gracefully handles this by relying on the existing `xcrun` availability checks.
