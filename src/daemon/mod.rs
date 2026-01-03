//! Flutter daemon infrastructure layer

pub mod commands;
pub mod devices;
pub mod emulators;
pub mod events;
pub mod process;
pub mod protocol;

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
