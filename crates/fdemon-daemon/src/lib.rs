//! # fdemon-daemon - Flutter Process Management
//!
//! Manages Flutter child processes, JSON-RPC communication (`--machine` mode),
//! device discovery, and emulator/simulator lifecycle.
//!
//! Depends on [`fdemon_core`] for domain types and error handling.
//!
//! ## Public API
//!
//! ### Process Management
//! - [`FlutterProcess`] - Spawn and manage `flutter run --machine` child processes
//! - [`CommandSender`] - Send JSON-RPC commands to a running Flutter process
//! - [`RequestTracker`] - Track pending request/response pairs
//!
//! ### Protocol Parsing
//! - [`parse_daemon_message()`] - Parse a line of Flutter `--machine` output
//! - [`to_log_entry()`] - Convert a parsed message to a log entry
//! - [`detect_log_level()`] - Determine log level from message content
//!
//! ### Device Discovery
//! - [`Device`] - Connected Flutter device (physical or emulator)
//! - [`discover_devices()`] - List connected devices via `flutter devices`
//!
//! ### Emulator Management
//! - [`Emulator`] - Available emulator/simulator
//! - [`discover_emulators()`] - List available emulators
//! - [`launch_emulator()`] - Start an emulator
//! - [`BootCommand`] - Platform-specific boot command (iOS Simulator / Android AVD)
//!
//! ### Platform Utilities
//! - [`IosSimulator`], [`AndroidAvd`] - Platform-specific device types
//! - [`ToolAvailability`] - Check for Android SDK, iOS tools

pub mod avds;
pub mod commands;
pub mod devices;
pub mod emulators;
pub mod process;
pub mod protocol;
pub mod simulators;
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_utils;
pub mod tool_availability;

// Public API re-exports
pub use avds::{
    boot_avd, is_any_emulator_running, kill_all_emulators, list_android_avds, AndroidAvd,
};
pub use commands::{CommandResponse, CommandSender, DaemonCommand, RequestTracker};
pub use devices::{
    discover_devices, discover_devices_with_timeout, filter_by_platform, find_device,
    group_by_platform, has_devices, Device, DeviceDiscoveryResult,
};
pub use emulators::{
    android_emulators, discover_emulators, discover_emulators_with_timeout, has_emulators,
    ios_simulators, launch_emulator, launch_emulator_cold, launch_emulator_with_options,
    launch_ios_simulator, Emulator, EmulatorDiscoveryResult, EmulatorLaunchOptions,
    EmulatorLaunchResult,
};
/// Re-exported from `fdemon_core` for convenience. Canonical import: `fdemon_core::DaemonMessage`.
pub use fdemon_core::DaemonMessage;
pub use process::FlutterProcess;
pub use protocol::{
    detect_log_level, parse_daemon_message, parse_flutter_log, to_log_entry, LogEntryInfo,
};
pub use simulators::{
    boot_simulator, group_simulators_by_runtime, list_ios_simulators, shutdown_simulator,
    IosSimulator, SimulatorState,
};
pub use tool_availability::ToolAvailability;

use fdemon_core::prelude::*;
use fdemon_core::types::{DeviceState, Platform};

/// Platform-specific boot command for offline devices
///
/// Represents the capability to boot a device (simulator/AVD) that is not currently running.
/// This is distinct from `core::BootableDevice` which is the UI/state representation.
#[derive(Debug, Clone)]
pub enum BootCommand {
    IosSimulator(IosSimulator),
    AndroidAvd(AndroidAvd),
}

impl BootCommand {
    pub fn id(&self) -> &str {
        match self {
            BootCommand::IosSimulator(s) => &s.udid,
            BootCommand::AndroidAvd(a) => &a.name,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            BootCommand::IosSimulator(s) => &s.name,
            BootCommand::AndroidAvd(a) => &a.display_name,
        }
    }

    pub fn platform(&self) -> &'static str {
        match self {
            BootCommand::IosSimulator(_) => "iOS",
            BootCommand::AndroidAvd(_) => "Android",
        }
    }

    pub fn runtime_info(&self) -> String {
        match self {
            BootCommand::IosSimulator(s) => s.runtime.clone(),
            BootCommand::AndroidAvd(a) => a
                .api_level
                .map(|api| format!("API {}", api))
                .unwrap_or_else(|| "Unknown API".to_string()),
        }
    }

    /// Boot this device
    pub async fn boot(&self, tool_availability: &ToolAvailability) -> Result<()> {
        match self {
            BootCommand::IosSimulator(s) => boot_simulator(&s.udid).await,
            BootCommand::AndroidAvd(a) => boot_avd(&a.name, tool_availability).await,
        }
    }
}

/// Convert BootCommand to core::BootableDevice for UI/state representation
impl From<BootCommand> for fdemon_core::types::BootableDevice {
    fn from(cmd: BootCommand) -> Self {
        match cmd {
            BootCommand::IosSimulator(sim) => {
                let state = match sim.state {
                    SimulatorState::Shutdown => DeviceState::Shutdown,
                    SimulatorState::Booted => DeviceState::Booted,
                    SimulatorState::Booting => DeviceState::Booting,
                    SimulatorState::Unknown => DeviceState::Unknown,
                };

                fdemon_core::types::BootableDevice::new(
                    sim.udid,
                    sim.name,
                    Platform::IOS,
                    sim.runtime,
                )
                .with_state(state)
            }
            BootCommand::AndroidAvd(avd) => {
                let runtime = avd
                    .api_level
                    .map(|api| format!("API {}", api))
                    .unwrap_or_else(|| "Unknown API".to_string());

                fdemon_core::types::BootableDevice::new(
                    avd.name.clone(),
                    avd.display_name,
                    Platform::Android,
                    runtime,
                )
                // AVDs discovered via list_android_avds are offline by definition
                .with_state(DeviceState::Shutdown)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_command_display_name() {
        let sim = IosSimulator {
            udid: "123".to_string(),
            name: "iPhone 15 Pro".to_string(),
            runtime: "iOS 17.2".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15 Pro".to_string(),
        };

        let cmd = BootCommand::IosSimulator(sim);
        assert_eq!(cmd.display_name(), "iPhone 15 Pro");
        assert_eq!(cmd.platform(), "iOS");
        assert_eq!(cmd.id(), "123");
    }

    #[test]
    fn test_avd_runtime_info() {
        let avd = AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6".to_string(),
            api_level: Some(33),
            target: None,
        };

        let cmd = BootCommand::AndroidAvd(avd);
        assert_eq!(cmd.runtime_info(), "API 33");
        assert_eq!(cmd.platform(), "Android");
        assert_eq!(cmd.id(), "Pixel_6_API_33");
    }

    #[test]
    fn test_avd_without_api_level() {
        let avd = AndroidAvd {
            name: "Custom_AVD".to_string(),
            display_name: "Custom AVD".to_string(),
            api_level: None,
            target: None,
        };

        let cmd = BootCommand::AndroidAvd(avd);
        assert_eq!(cmd.runtime_info(), "Unknown API");
    }

    #[test]
    fn test_ios_simulator_runtime_info() {
        let sim = IosSimulator {
            udid: "456".to_string(),
            name: "iPad Pro".to_string(),
            runtime: "iOS 16.4".to_string(),
            state: SimulatorState::Booted,
            device_type: "iPad Pro".to_string(),
        };

        let cmd = BootCommand::IosSimulator(sim);
        assert_eq!(cmd.runtime_info(), "iOS 16.4");
    }

    #[test]
    fn test_boot_command_to_bootable_device_ios() {
        let cmd = BootCommand::IosSimulator(IosSimulator {
            udid: "ABC-123".to_string(),
            name: "iPhone 15".to_string(),
            runtime: "iOS 17.0".to_string(),
            state: SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        });

        let device: fdemon_core::types::BootableDevice = cmd.into();
        assert_eq!(device.platform, Platform::IOS);
        assert_eq!(device.id, "ABC-123");
        assert_eq!(device.name, "iPhone 15");
        assert_eq!(device.runtime, "iOS 17.0");
        assert_eq!(device.state, DeviceState::Shutdown);
    }

    #[test]
    fn test_boot_command_to_bootable_device_android() {
        let cmd = BootCommand::AndroidAvd(AndroidAvd {
            name: "Pixel_6_API_33".to_string(),
            display_name: "Pixel 6 Pro".to_string(),
            api_level: Some(33),
            target: None,
        });

        let device: fdemon_core::types::BootableDevice = cmd.into();
        assert_eq!(device.platform, Platform::Android);
        assert_eq!(device.id, "Pixel_6_API_33");
        assert_eq!(device.name, "Pixel 6 Pro");
        assert_eq!(device.runtime, "API 33");
        assert_eq!(device.state, DeviceState::Shutdown);
    }

    #[test]
    fn test_boot_command_ios_state_mapping() {
        let states = vec![
            (SimulatorState::Shutdown, DeviceState::Shutdown),
            (SimulatorState::Booted, DeviceState::Booted),
            (SimulatorState::Booting, DeviceState::Booting),
            (SimulatorState::Unknown, DeviceState::Unknown),
        ];

        for (sim_state, expected_state) in states {
            let cmd = BootCommand::IosSimulator(IosSimulator {
                udid: "test".to_string(),
                name: "Test".to_string(),
                runtime: "iOS 17".to_string(),
                state: sim_state,
                device_type: "iPhone".to_string(),
            });

            let device: fdemon_core::types::BootableDevice = cmd.into();
            assert_eq!(device.state, expected_state);
        }
    }
}
