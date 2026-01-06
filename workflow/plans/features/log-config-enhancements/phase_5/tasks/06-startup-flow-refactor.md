# Task: Startup Flow Refactor

**Objective**: Update `startup_flutter()` to implement the new priority-based startup logic with settings.local.toml preferences and startup dialog integration.

**Depends on**: Task 01 (Config Priority), Task 02 (State), Task 03 (Widget), Task 05 (Preferences)

## Scope

- `src/tui/startup.rs` — Refactor `startup_flutter()` function
- `src/app/handler/mod.rs` — Handle `StartupDialogConfirm` message

## Details

### New Startup Logic

```
┌─────────────────────────────────────────────────────────────────┐
│                     auto_start = true?                          │
└───────────────────────────┬─────────────────────────────────────┘
                            │
            ┌───────────────┴───────────────┐
            │ Yes                           │ No
            ▼                               ▼
┌───────────────────────┐       ┌───────────────────────────────┐
│ Load settings.local   │       │ Show StartupDialog            │
│ for last_config/device│       │ Load configs, discover devices│
└───────────┬───────────┘       └───────────────────────────────┘
            │
            │ Found & valid?
            │
    ┌───────┴───────┐
    │ Yes           │ No
    ▼               ▼
┌───────────┐   ┌───────────────────────┐
│ Use saved │   │ Find first auto_start │
│ selection │   │ config from launch.toml│
└─────┬─────┘   └───────────┬───────────┘
      │                     │
      │                     │ Found?
      │             ┌───────┴───────┐
      │             │ Yes           │ No
      │             ▼               ▼
      │         ┌───────┐   ┌───────────────────┐
      │         │ Use it│   │ Use first config  │
      │         └───┬───┘   │ from launch.toml  │
      │             │       │ or launch.json    │
      │             │       └─────────┬─────────┘
      │             │                 │
      └─────────────┴─────────────────┤
                                      │
                                      │ Config found?
                              ┌───────┴───────┐
                              │ Yes           │ No
                              ▼               ▼
                    ┌─────────────────┐  ┌──────────────────────┐
                    │ Discover devices│  │ Bare flutter run     │
                    │ Find matching   │  │ with first device    │
                    │ device or first │  └──────────────────────┘
                    └─────────────────┘
```

### Refactored startup_flutter()

```rust
// src/tui/startup.rs

use crate::app::state::UiMode;
use crate::config::{
    load_all_configs, load_last_selection, validate_last_selection,
    get_first_auto_start, get_first_config, LoadedConfigs, SourcedConfig,
};

/// Handle auto-start or show startup dialog
pub async fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
) -> Option<UpdateAction> {
    // Load all configs upfront (needed for both paths)
    let configs = load_all_configs(project_path);

    if settings.behavior.auto_start {
        auto_start_session(state, &configs, project_path, msg_tx).await
    } else {
        show_startup_dialog(state, configs, msg_tx)
    }
}

/// Auto-start mode: try to launch immediately based on preferences
async fn auto_start_session(
    state: &mut AppState,
    configs: &LoadedConfigs,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
) -> Option<UpdateAction> {
    state.ui_mode = UiMode::Loading;

    // Step 1: Check settings.local.toml for saved selection
    if let Some(selection) = load_last_selection(project_path) {
        // Discover devices first to validate selection
        match devices::discover_devices().await {
            Ok(result) => {
                if let Some(validated) = validate_last_selection(&selection, configs, &result.devices) {
                    return launch_with_validated_selection(
                        state,
                        configs,
                        &result.devices,
                        validated,
                    );
                }
                // Selection invalid, fall through to auto_start config
                return try_auto_start_config(state, configs, result.devices, project_path, msg_tx);
            }
            Err(e) => {
                state.ui_mode = UiMode::DeviceSelector;
                state.device_selector.set_error(e.to_string());
                return None;
            }
        }
    }

    // Step 2: No saved selection, discover devices and find config
    match devices::discover_devices().await {
        Ok(result) => {
            try_auto_start_config(state, configs, result.devices, project_path, msg_tx)
        }
        Err(e) => {
            state.ui_mode = UiMode::DeviceSelector;
            state.device_selector.set_error(e.to_string());
            None
        }
    }
}

/// Try to find and use an auto_start config
fn try_auto_start_config(
    state: &mut AppState,
    configs: &LoadedConfigs,
    devices: Vec<Device>,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
) -> Option<UpdateAction> {
    // Find config: auto_start > first config > bare run
    let config = get_first_auto_start(configs)
        .or_else(|| get_first_config(configs));

    if let Some(config) = config {
        // Find matching device
        let device = if config.config.device == "auto" {
            devices.first().cloned()
        } else {
            devices::find_device(&devices, &config.config.device).cloned()
                .or_else(|| devices.first().cloned())
        };

        if let Some(device) = device {
            return launch_session(state, Some(&config.config), &device, project_path);
        }
    }

    // No config with matching device, try bare run with first device
    if let Some(device) = devices.first() {
        return launch_session(state, None, device, project_path);
    }

    // No devices at all, show device selector
    state.ui_mode = UiMode::DeviceSelector;
    state.device_selector.set_devices(devices);
    spawn::spawn_device_discovery(msg_tx);
    None
}

/// Launch with validated selection from settings.local.toml
fn launch_with_validated_selection(
    state: &mut AppState,
    configs: &LoadedConfigs,
    devices: &[Device],
    validated: ValidatedSelection,
) -> Option<UpdateAction> {
    let config = validated.config_idx.and_then(|i| configs.configs.get(i));
    let device = validated.device_idx.and_then(|i| devices.get(i))?;

    launch_session(state, config.map(|c| &c.config), device, &state.project_path.clone())
}

/// Launch a session with optional config
fn launch_session(
    state: &mut AppState,
    config: Option<&LaunchConfig>,
    device: &Device,
    project_path: &Path,
) -> Option<UpdateAction> {
    // Create session via SessionManager
    let result = if let Some(cfg) = config {
        state.session_manager.create_session_with_config(device, cfg.clone())
    } else {
        state.session_manager.create_session(device)
    };

    match result {
        Ok(session_id) => {
            state.ui_mode = UiMode::Normal;

            // Save selection for next time
            let _ = config::save_last_selection(
                project_path,
                config.map(|c| c.name.as_str()),
                Some(&device.id),
            );

            Some(UpdateAction::SpawnSession {
                session_id,
                device: device.clone(),
                config: config.map(|c| Box::new(c.clone())),
            })
        }
        Err(e) => {
            if let Some(session) = state.session_manager.selected_mut() {
                session.session.log_error(
                    LogSource::App,
                    format!("Failed to create session: {}", e),
                );
            }
            None
        }
    }
}

/// Show startup dialog (manual mode)
fn show_startup_dialog(
    state: &mut AppState,
    configs: LoadedConfigs,
    msg_tx: mpsc::Sender<Message>,
) -> Option<UpdateAction> {
    state.show_startup_dialog(configs);
    spawn::spawn_device_discovery(msg_tx);
    None
}
```

### Handle StartupDialogConfirm

Add to `src/app/handler/mod.rs`:

```rust
Message::StartupDialogConfirm => {
    handle_startup_dialog_confirm(state)
}

fn handle_startup_dialog_confirm(state: &mut AppState) -> Option<UpdateAction> {
    let dialog = &state.startup_dialog_state;

    // Get selected device (required)
    let device = dialog.selected_device()?.clone();

    // Build launch config from dialog state
    let config = dialog.selected_config().map(|sourced| {
        let mut cfg = sourced.config.clone();

        // Override with dialog values
        cfg.mode = dialog.mode;

        if !dialog.flavor.is_empty() {
            cfg.flavor = Some(dialog.flavor.clone());
        }

        if !dialog.dart_defines.is_empty() {
            // Parse "KEY=VALUE,KEY2=VALUE2" format
            cfg.dart_defines = parse_dart_defines(&dialog.dart_defines);
        }

        cfg
    });

    // Save selection
    let _ = config::save_last_selection(
        &state.project_path,
        config.as_ref().map(|c| c.name.as_str()),
        Some(&device.id),
    );

    // Create session
    let result = if let Some(ref cfg) = config {
        state.session_manager.create_session_with_config(&device, cfg.clone())
    } else {
        state.session_manager.create_session(&device)
    };

    match result {
        Ok(session_id) => {
            state.ui_mode = UiMode::Normal;
            Some(UpdateAction::SpawnSession {
                session_id,
                device,
                config: config.map(Box::new),
            })
        }
        Err(e) => {
            state.startup_dialog_state.error = Some(format!("Failed to create session: {}", e));
            None
        }
    }
}

fn parse_dart_defines(input: &str) -> HashMap<String, String> {
    input
        .split(',')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?.trim().to_string();
            let value = parts.next()?.trim().to_string();
            if key.is_empty() {
                None
            } else {
                Some((key, value))
            }
        })
        .collect()
}
```

## Acceptance Criteria

1. Auto-start checks settings.local.toml first
2. Auto-start falls back to auto_start config, then first config
3. Auto-start works with bare flutter run if no configs
4. Startup dialog shows when auto_start=false
5. Dialog confirm saves selection to settings.local.toml
6. Dialog confirm launches session with overridden mode/flavor/dart-defines
7. Error states handled gracefully

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dart_defines() {
        let input = "API_URL=https://api.com,DEBUG=true";
        let result = parse_dart_defines(input);

        assert_eq!(result.get("API_URL"), Some(&"https://api.com".to_string()));
        assert_eq!(result.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_dart_defines_empty() {
        let result = parse_dart_defines("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_dart_defines_single() {
        let input = "KEY=VALUE";
        let result = parse_dart_defines(input);

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("KEY"), Some(&"VALUE".to_string()));
    }

    #[test]
    fn test_parse_dart_defines_with_spaces() {
        let input = " KEY = VALUE , KEY2 = VALUE2 ";
        let result = parse_dart_defines(input);

        assert_eq!(result.get("KEY"), Some(&"VALUE".to_string()));
        assert_eq!(result.get("KEY2"), Some(&"VALUE2".to_string()));
    }
}
```

## Notes

- `create_session()` without config already exists for bare flutter run
- Selection auto-saved on successful launch
- If saved device disappears, falls back to first available device
- Error logging uses existing LogSource::App pattern

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
- `cargo test startup` - Pending
