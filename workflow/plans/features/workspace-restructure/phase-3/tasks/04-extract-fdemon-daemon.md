## Task: Extract fdemon-daemon Crate

**Objective**: Move the `daemon/` module into the `fdemon-daemon` crate. This crate provides Flutter process management, device/emulator discovery, and JSON-RPC protocol handling. It depends only on `fdemon-core`.

**Depends on**: 03-extract-fdemon-core

**Estimated Time**: 3-4 hours

### Scope

- `src/daemon/process.rs` -> `crates/fdemon-daemon/src/process.rs`
- `src/daemon/protocol.rs` -> `crates/fdemon-daemon/src/protocol.rs`
- `src/daemon/commands.rs` -> `crates/fdemon-daemon/src/commands.rs`
- `src/daemon/devices.rs` -> `crates/fdemon-daemon/src/devices.rs`
- `src/daemon/emulators.rs` -> `crates/fdemon-daemon/src/emulators.rs`
- `src/daemon/simulators.rs` -> `crates/fdemon-daemon/src/simulators.rs`
- `src/daemon/avds.rs` -> `crates/fdemon-daemon/src/avds.rs`
- `src/daemon/tool_availability.rs` -> `crates/fdemon-daemon/src/tool_availability.rs`
- `src/daemon/test_utils.rs` -> `crates/fdemon-daemon/src/test_utils.rs`
- `src/daemon/mod.rs` -> `crates/fdemon-daemon/src/lib.rs` (with modifications)
- `crates/fdemon-daemon/src/lib.rs`: Module declarations + public API

### Details

#### 1. File Moves

Copy all files from `src/daemon/` into `crates/fdemon-daemon/src/`.

#### 2. Write `lib.rs`

Transform `daemon/mod.rs` into `lib.rs`. Key changes:

```rust
//! fdemon-daemon - Flutter process management for Flutter Demon
//!
//! Manages Flutter child processes, JSON-RPC communication, device discovery,
//! and emulator/simulator lifecycle.

pub mod avds;
pub mod commands;
pub mod devices;
pub mod emulators;
pub mod process;
pub mod protocol;
pub mod simulators;
#[cfg(test)]
pub mod test_utils;
pub mod tool_availability;

// Public API re-exports
pub use avds::{boot_avd, is_any_emulator_running, kill_all_emulators, list_android_avds, AndroidAvd};
pub use commands::{next_request_id, CommandResponse, CommandSender, DaemonCommand, RequestTracker};
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
pub use process::FlutterProcess;
pub use protocol::{strip_brackets, LogEntryInfo, RawMessage};
pub use simulators::{
    boot_simulator, group_simulators_by_runtime, list_ios_simulators, shutdown_simulator,
    IosSimulator, SimulatorState,
};
pub use tool_availability::ToolAvailability;

// BootCommand and conversion
// ... (keep BootCommand and From<BootCommand> impl)
```

#### 3. Update Internal Imports

Replace import paths within `fdemon-daemon` files:

| Old Pattern | New Pattern |
|-------------|-------------|
| `use crate::common::prelude::*` | `use fdemon_core::prelude::*` |
| `use crate::core::DaemonEvent` | `use fdemon_core::events::DaemonEvent` |
| `use crate::core::DaemonMessage` | `use fdemon_core::events::DaemonMessage` |
| `use crate::core::{DeviceState, Platform}` | `use fdemon_core::types::{DeviceState, Platform}` |
| `use crate::core::{contains_word, strip_ansi_codes}` | `use fdemon_core::ansi::{contains_word, strip_ansi_codes}` |
| `use crate::core::{LogLevel, LogSource, ...}` | `use fdemon_core::types::{LogLevel, LogSource, ...}` |
| `use crate::config::LaunchConfig` | See note below |

#### 4. Handle `daemon/process.rs` -> `config::LaunchConfig` Dependency

`daemon/process.rs` imports `crate::config::LaunchConfig` (used in `FlutterProcess::spawn()`). Since `config/` will live in `fdemon-app`, this would create a circular dependency (`fdemon-daemon` -> `fdemon-app` -> `fdemon-daemon`).

**Solution**: `FlutterProcess::spawn()` should accept primitive arguments instead of `LaunchConfig`. Look at what fields it actually uses from `LaunchConfig` and pass them directly:

```rust
// Instead of: pub async fn spawn(config: &LaunchConfig, project_path: &Path) -> Result<Self>
// Use: pub async fn spawn(args: FlutterRunArgs, project_path: &Path) -> Result<Self>

/// Arguments for spawning a Flutter process
pub struct FlutterRunArgs {
    pub device_id: Option<String>,
    pub mode: Option<String>,       // "debug", "profile", "release"
    pub flavor: Option<String>,
    pub target: Option<String>,
    pub dart_defines: Vec<String>,
    pub additional_args: Vec<String>,
}
```

The conversion from `LaunchConfig` -> `FlutterRunArgs` happens in `fdemon-app` where both types are available.

Alternatively, if `FlutterProcess::spawn()` already accepts raw arguments (check the actual implementation), this may not be needed.

#### 5. Remove Core Re-exports

The current `daemon/mod.rs` re-exports event types from `core` for backward compatibility:
```rust
pub use crate::core::{AppDebugPort, AppLog, AppProgress, AppStart, ...};
```

In the workspace, these re-exports are removed. Consumers import from `fdemon-core` directly. Keep them only if `fdemon-daemon` itself uses them internally (check actual usage).

#### 6. Keep Compatibility Shim in Main Crate

```rust
// src/daemon/mod.rs (temporary re-export shim)
pub use fdemon_daemon::*;
```

This allows `app/`, `tui/`, etc. (still in the main crate) to continue using `use crate::daemon::*`.

### Acceptance Criteria

1. `crates/fdemon-daemon/src/` contains all daemon module files
2. `cargo check -p fdemon-daemon` passes
3. `cargo test -p fdemon-daemon` passes
4. `fdemon-daemon` depends only on `fdemon-core` (and external crates)
5. `fdemon-daemon` does NOT depend on `config/` or `app/` types
6. Compatibility shim in `src/daemon/mod.rs` re-exports from `fdemon-daemon`
7. `cargo check` (full workspace) passes
8. `cargo test` (full workspace) passes

### Testing

```bash
# Test the new crate in isolation
cargo check -p fdemon-daemon
cargo test -p fdemon-daemon

# Test full workspace still works
cargo check
cargo test
```

### Notes

- The `daemon/test_utils.rs` module is `#[cfg(test)]` only. It provides helpers for constructing test `Device` instances. Keep it in the crate as `pub mod test_utils` behind `#[cfg(test)]`.
- The `BootCommand` enum and `From<BootCommand> for BootableDevice` impl stay in `fdemon-daemon` since `BootCommand` wraps daemon-specific types (`IosSimulator`, `AndroidAvd`) and converts to core types.
- `protocol.rs` has inline tests that use `DaemonMessage::parse()` - these should work after updating imports to `fdemon_core`.
- The `LaunchConfig` decoupling is the trickiest part. Investigate `FlutterProcess::spawn()` signature carefully before deciding the approach.
