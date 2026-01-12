//! Flutter daemon infrastructure layer

pub mod avds;
pub mod commands;
pub mod devices;
pub mod emulators;
pub mod events;
pub mod process;
pub mod protocol;
pub mod simulators;
pub mod tool_availability;

pub use avds::{boot_avd, is_avd_running, kill_all_emulators, list_android_avds, AndroidAvd};
pub use commands::{
    next_request_id, CommandResponse, CommandSender, DaemonCommand, RequestTracker,
};
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
pub use events::{
    AppDebugPort, AppLog, AppProgress, AppStart, AppStarted, AppStop, DaemonConnected,
    DaemonLogMessage, DeviceInfo,
};
pub use process::FlutterProcess;
pub use protocol::{strip_brackets, DaemonMessage, LogEntryInfo, RawMessage};
pub use simulators::{
    boot_simulator, group_simulators_by_runtime, list_ios_simulators, shutdown_simulator,
    IosSimulator, SimulatorState,
};
pub use tool_availability::ToolAvailability;

use crate::common::prelude::*;

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
            BootableDevice::AndroidAvd(a) => a
                .api_level
                .map(|api| format!("API {}", api))
                .unwrap_or_else(|| "Unknown API".to_string()),
        }
    }

    /// Boot this device
    pub async fn boot(&self, tool_availability: &ToolAvailability) -> Result<()> {
        match self {
            BootableDevice::IosSimulator(s) => boot_simulator(&s.udid).await,
            BootableDevice::AndroidAvd(a) => boot_avd(&a.name, tool_availability).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(device.id(), "123");
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
        assert_eq!(device.platform(), "Android");
        assert_eq!(device.id(), "Pixel_6_API_33");
    }

    #[test]
    fn test_avd_without_api_level() {
        let avd = AndroidAvd {
            name: "Custom_AVD".to_string(),
            display_name: "Custom AVD".to_string(),
            api_level: None,
            target: None,
        };

        let device = BootableDevice::AndroidAvd(avd);
        assert_eq!(device.runtime_info(), "Unknown API");
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

        let device = BootableDevice::IosSimulator(sim);
        assert_eq!(device.runtime_info(), "iOS 16.4");
    }
}
