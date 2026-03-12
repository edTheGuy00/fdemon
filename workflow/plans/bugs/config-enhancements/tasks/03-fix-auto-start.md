## Task: Fix auto_start Launch Flow

**Objective**: Wire the existing `Message::StartAutoLaunch` infrastructure into the TUI startup sequence so that `auto_start = true` in `launch.toml` (or `behavior.auto_start = true` in `config.toml`) causes fdemon to auto-launch instead of showing the NewSessionDialog.

**Depends on**: None

### Scope

- `crates/fdemon-tui/src/startup.rs:22-36`: Add conditional auto-start branch
- `crates/fdemon-tui/src/runner.rs:22-55`: Handle new `StartupAction::AutoStart` variant

### Details

**Change 1: startup.rs — Add auto-start detection**

Modify `startup_flutter()` to check for auto_start configs:

```rust
use fdemon_app::config::{self, load_all_configs, get_first_auto_start, LoadedConfigs};

/// Result of startup initialization
#[derive(Debug)]
pub enum StartupAction {
    /// Enter normal mode, no auto-start — show NewSessionDialog
    Ready,
    /// Auto-start detected — send StartAutoLaunch message
    AutoStart { configs: LoadedConfigs },
}

pub fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    let configs = load_all_configs(project_path);

    // Check if any config has auto_start = true, or behavior.auto_start is enabled
    let has_auto_start_config = get_first_auto_start(&configs).is_some();
    let behavior_auto_start = settings.behavior.auto_start;

    if has_auto_start_config || behavior_auto_start {
        // Return AutoStart — runner will send StartAutoLaunch message
        return StartupAction::AutoStart { configs };
    }

    // Default: show NewSessionDialog
    state.show_new_session_dialog(configs.clone());
    state.ui_mode = UiMode::Startup;
    StartupAction::Ready
}
```

**Change 2: runner.rs — Handle AutoStart action**

In `run_with_project()`, match on the startup result:

```rust
let startup_result =
    startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);

// Render first frame
if let Err(e) = term.draw(|frame| render::view(frame, &mut engine.state)) {
    error!("Failed to render initial frame: {}", e);
}

// Trigger startup discovery (non-blocking)
spawn::spawn_tool_availability_check(engine.msg_sender());

match startup_result {
    startup::StartupAction::AutoStart { configs } => {
        // Send StartAutoLaunch — this triggers device discovery + auto-launch
        let _ = engine.msg_sender().send(Message::StartAutoLaunch { configs }).await;
    }
    startup::StartupAction::Ready => {
        // No auto-start — discover devices for the NewSessionDialog
        spawn::spawn_device_discovery(engine.msg_sender());
    }
}
```

Note: When auto-launching, device discovery is handled internally by `spawn_auto_launch()` (triggered by the `StartAutoLaunch` handler), so we don't need to call `spawn_device_discovery()` separately. The `StartAutoLaunch` handler in `update.rs:853-861` sets a loading overlay and returns `UpdateAction::DiscoverDevicesAndAutoLaunch`.

**Important: Check `run_with_project_and_dap()` too** — This function at `runner.rs:57+` may have its own startup logic that also needs the same auto-start check. Verify and update if needed.

### Acceptance Criteria

1. `auto_start = true` on a launch config in `launch.toml` causes auto-launch on startup
2. `behavior.auto_start = true` in `config.toml` causes auto-launch even without a specific launch config
3. When both are false (or absent), the NewSessionDialog is shown as before
4. Auto-launch failure (no devices) falls back to showing the dialog with an error message (this is already handled by `AutoLaunchResult` handler)
5. The loading overlay is shown during device discovery (already handled by `StartAutoLaunch` handler)
6. `run_with_project_and_dap()` is also updated if it has its own startup path

### Testing

```bash
cargo test -p fdemon-tui -- startup
cargo test -p fdemon-app -- auto_launch
```

### Notes

- The entire `StartAutoLaunch` → `spawn_auto_launch` → `AutoLaunchResult` chain already exists and is tested. This task is purely about sending the initial `StartAutoLaunch` message.
- `_settings` parameter in `startup_flutter` is currently unused (prefixed with `_`); it will be used after this change.
- The `StartAutoLaunch` message requires a `LoadedConfigs` parameter, which is already loaded in `startup_flutter()`.
- `get_first_auto_start()` is in `crates/fdemon-app/src/config/priority.rs:93-95`.

---

## Completion Summary

**Status:** Not Started
