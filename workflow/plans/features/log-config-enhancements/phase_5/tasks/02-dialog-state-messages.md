# Task: Startup Dialog State & Messages

**Objective**: Add `StartupDialogState` struct and related messages to support the new startup dialog UI for session launching.

**Depends on**: None

## Scope

- `src/app/state.rs` — Add `StartupDialogState`, `UiMode::StartupDialog`
- `src/app/message.rs` — Add startup dialog messages

## Details

### New UiMode Variant

Update `UiMode` enum in `src/app/state.rs`:

```rust
pub enum UiMode {
    // ... existing variants ...

    /// Startup dialog - comprehensive session launch UI
    /// Shows config selection, mode, flavor, dart-defines, and device list
    StartupDialog,
}
```

### StartupDialogState Struct

Add to `src/app/state.rs`:

```rust
use crate::config::{FlutterMode, LoadedConfigs, SourcedConfig};
use crate::daemon::Device;

/// Which section of the startup dialog is focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogSection {
    #[default]
    Configs,      // Launch config selection
    Mode,         // Debug/Profile/Release
    Flavor,       // Flavor text input
    DartDefines,  // Dart-define text input
    Devices,      // Device selection
}

impl DialogSection {
    pub fn next(&self) -> Self {
        match self {
            Self::Configs => Self::Mode,
            Self::Mode => Self::Flavor,
            Self::Flavor => Self::DartDefines,
            Self::DartDefines => Self::Devices,
            Self::Devices => Self::Configs,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Configs => Self::Devices,
            Self::Mode => Self::Configs,
            Self::Flavor => Self::Mode,
            Self::DartDefines => Self::Flavor,
            Self::Devices => Self::DartDefines,
        }
    }
}

/// State for the startup dialog
#[derive(Debug, Clone)]
pub struct StartupDialogState {
    /// Loaded configurations (launch.toml + launch.json)
    pub configs: LoadedConfigs,

    /// Available devices
    pub devices: Vec<Device>,

    /// Currently selected config index (None = no config, bare flutter run)
    pub selected_config: Option<usize>,

    /// Currently selected device index
    pub selected_device: Option<usize>,

    /// Selected build mode
    pub mode: FlutterMode,

    /// Flavor input (optional)
    pub flavor: String,

    /// Dart-define input (optional, format: KEY=VALUE,KEY2=VALUE2)
    pub dart_defines: String,

    /// Currently focused section
    pub active_section: DialogSection,

    /// Whether currently editing flavor/dart-defines
    pub editing: bool,

    /// Loading state (discovering devices)
    pub loading: bool,

    /// Refreshing devices in background
    pub refreshing: bool,

    /// Error message (if any)
    pub error: Option<String>,

    /// Animation frame for loading indicator
    pub animation_frame: u64,
}

impl Default for StartupDialogState {
    fn default() -> Self {
        Self {
            configs: LoadedConfigs::default(),
            devices: Vec::new(),
            selected_config: None,
            selected_device: None,
            mode: FlutterMode::Debug,
            flavor: String::new(),
            dart_defines: String::new(),
            active_section: DialogSection::Configs,
            editing: false,
            loading: true,
            refreshing: false,
            error: None,
            animation_frame: 0,
        }
    }
}

impl StartupDialogState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize with configs
    pub fn with_configs(configs: LoadedConfigs) -> Self {
        let selected_config = if configs.configs.is_empty() {
            None
        } else {
            Some(0) // Select first config by default
        };

        Self {
            configs,
            selected_config,
            ..Self::default()
        }
    }

    /// Set devices after discovery
    pub fn set_devices(&mut self, devices: Vec<Device>) {
        self.devices = devices;
        self.loading = false;
        self.refreshing = false;
        self.error = None;

        // Auto-select first device if none selected
        if self.selected_device.is_none() && !self.devices.is_empty() {
            self.selected_device = Some(0);
        }
    }

    /// Set error state
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
        self.refreshing = false;
    }

    /// Get selected config
    pub fn selected_config(&self) -> Option<&SourcedConfig> {
        self.selected_config
            .and_then(|idx| self.configs.configs.get(idx))
    }

    /// Get selected device
    pub fn selected_device(&self) -> Option<&Device> {
        self.selected_device
            .and_then(|idx| self.devices.get(idx))
    }

    /// Can launch? (need device, config optional)
    pub fn can_launch(&self) -> bool {
        self.selected_device.is_some()
    }

    /// Navigate up in current section
    pub fn navigate_up(&mut self) {
        match self.active_section {
            DialogSection::Configs => {
                if let Some(idx) = self.selected_config {
                    if idx > 0 {
                        self.selected_config = Some(idx - 1);
                    } else {
                        // Wrap to end or set to None (no config)
                        self.selected_config = Some(self.configs.configs.len().saturating_sub(1));
                    }
                }
            }
            DialogSection::Mode => {
                self.mode = match self.mode {
                    FlutterMode::Debug => FlutterMode::Release,
                    FlutterMode::Profile => FlutterMode::Debug,
                    FlutterMode::Release => FlutterMode::Profile,
                };
            }
            DialogSection::Devices => {
                if let Some(idx) = self.selected_device {
                    if idx > 0 {
                        self.selected_device = Some(idx - 1);
                    } else if !self.devices.is_empty() {
                        self.selected_device = Some(self.devices.len() - 1);
                    }
                }
            }
            _ => {} // Flavor/DartDefines are text inputs
        }
    }

    /// Navigate down in current section
    pub fn navigate_down(&mut self) {
        match self.active_section {
            DialogSection::Configs => {
                if !self.configs.configs.is_empty() {
                    let max = self.configs.configs.len() - 1;
                    let current = self.selected_config.unwrap_or(0);
                    self.selected_config = Some(if current >= max { 0 } else { current + 1 });
                }
            }
            DialogSection::Mode => {
                self.mode = match self.mode {
                    FlutterMode::Debug => FlutterMode::Profile,
                    FlutterMode::Profile => FlutterMode::Release,
                    FlutterMode::Release => FlutterMode::Debug,
                };
            }
            DialogSection::Devices => {
                if !self.devices.is_empty() {
                    let max = self.devices.len() - 1;
                    let current = self.selected_device.unwrap_or(0);
                    self.selected_device = Some(if current >= max { 0 } else { current + 1 });
                }
            }
            _ => {} // Flavor/DartDefines are text inputs
        }
    }

    /// Move to next section
    pub fn next_section(&mut self) {
        self.editing = false;
        self.active_section = self.active_section.next();
    }

    /// Move to previous section
    pub fn prev_section(&mut self) {
        self.editing = false;
        self.active_section = self.active_section.prev();
    }

    /// Tick animation frame
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    /// Apply config defaults when config is selected
    pub fn apply_config_defaults(&mut self) {
        if let Some(config) = self.selected_config() {
            self.mode = config.config.mode;
            if let Some(ref flavor) = config.config.flavor {
                self.flavor = flavor.clone();
            }
            // Convert dart_defines HashMap to string format
            if !config.config.dart_defines.is_empty() {
                self.dart_defines = config
                    .config
                    .dart_defines
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",");
            }
        }
    }
}
```

### New Messages

Add to `src/app/message.rs`:

```rust
// ─────────────────────────────────────────────────────────────
// Startup Dialog Messages (Phase 5)
// ─────────────────────────────────────────────────────────────
/// Show startup dialog
ShowStartupDialog,

/// Hide startup dialog (cancel)
HideStartupDialog,

/// Navigate up in current section
StartupDialogUp,

/// Navigate down in current section
StartupDialogDown,

/// Move to next section (Tab)
StartupDialogNextSection,

/// Move to previous section (Shift+Tab)
StartupDialogPrevSection,

/// Select specific config by index
StartupDialogSelectConfig(usize),

/// Select specific device by index
StartupDialogSelectDevice(usize),

/// Set build mode
StartupDialogSetMode(FlutterMode),

/// Character input for flavor/dart-defines
StartupDialogCharInput(char),

/// Backspace in input field
StartupDialogBackspace,

/// Clear input field
StartupDialogClearInput,

/// Confirm and launch session
StartupDialogConfirm,

/// Refresh device list
StartupDialogRefreshDevices,
```

### Update AppState

Add field to `AppState` in `src/app/state.rs`:

```rust
pub struct AppState {
    // ... existing fields ...

    /// Startup dialog state
    pub startup_dialog_state: StartupDialogState,
}

impl AppState {
    pub fn with_settings(project_path: PathBuf, settings: Settings) -> Self {
        // ... existing code ...
        Self {
            // ... existing fields ...
            startup_dialog_state: StartupDialogState::new(),
        }
    }

    /// Show startup dialog
    pub fn show_startup_dialog(&mut self, configs: LoadedConfigs) {
        self.startup_dialog_state = StartupDialogState::with_configs(configs);
        self.ui_mode = UiMode::StartupDialog;
    }

    /// Hide startup dialog
    pub fn hide_startup_dialog(&mut self) {
        self.ui_mode = UiMode::Normal;
    }
}
```

## Acceptance Criteria

1. `UiMode::StartupDialog` variant exists
2. `DialogSection` enum with all 5 sections
3. `StartupDialogState` has all required fields
4. Navigation methods work correctly (up/down/next/prev section)
5. `can_launch()` returns true only when device selected
6. Config defaults apply when config selected
7. All startup dialog messages defined

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_section_navigation() {
        assert_eq!(DialogSection::Configs.next(), DialogSection::Mode);
        assert_eq!(DialogSection::Devices.next(), DialogSection::Configs);
        assert_eq!(DialogSection::Configs.prev(), DialogSection::Devices);
        assert_eq!(DialogSection::Mode.prev(), DialogSection::Configs);
    }

    #[test]
    fn test_startup_dialog_state_defaults() {
        let state = StartupDialogState::new();

        assert!(state.loading);
        assert!(state.devices.is_empty());
        assert!(state.selected_config.is_none());
        assert_eq!(state.mode, FlutterMode::Debug);
        assert!(state.flavor.is_empty());
        assert_eq!(state.active_section, DialogSection::Configs);
    }

    #[test]
    fn test_can_launch_requires_device() {
        let mut state = StartupDialogState::new();
        assert!(!state.can_launch());

        state.set_devices(vec![Device {
            id: "test".to_string(),
            name: "Test".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            ..Default::default()
        }]);

        assert!(state.can_launch());
    }

    #[test]
    fn test_mode_cycling() {
        let mut state = StartupDialogState::new();
        state.active_section = DialogSection::Mode;

        state.navigate_down();
        assert_eq!(state.mode, FlutterMode::Profile);

        state.navigate_down();
        assert_eq!(state.mode, FlutterMode::Release);

        state.navigate_down();
        assert_eq!(state.mode, FlutterMode::Debug);
    }
}
```

## Notes

- `FlutterMode` already exists in `config/types.rs`
- `Device` already exists in `daemon/devices.rs`
- State is separate from widget - widget receives state as parameter
- Animation frame reuses pattern from `DeviceSelectorState`

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**
(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy -- -D warnings` - Pending
- `cargo test dialog` - Pending
