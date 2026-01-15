# Task: Implement Auto-Save Action

## Summary

Implement the `AutoSaveConfig` action handler that is currently a placeholder, so that FDemon config changes are actually persisted to `.fdemon/launch.toml`.

## Files

| File | Action |
|------|--------|
| `src/tui/actions.rs` | Modify (implement action) |

## Background

The code review identified that the `AutoSaveConfig` action is a placeholder that silently does nothing. This creates misleading behavior where the UI suggests functionality but nothing happens.

**Current (placeholder):**
```rust
UpdateAction::AutoSaveConfig { config_index: _ } => {
    // TODO: Implement auto-save logic in a future task
    tracing::debug!("Auto-save config triggered (not yet implemented)");
}
```

## Implementation

### 1. Implement AutoSaveConfig action handler

Location: `src/tui/actions.rs`

```rust
UpdateAction::AutoSaveConfig { config_index } => {
    // Get the config data from state
    if let Some(ref dialog) = state.new_session_dialog_state {
        // Only save FDemon configs
        let config = dialog.launch_context_state.configs.get(config_index);
        if let Some(config) = config {
            if config.source != ConfigSource::FDemon {
                tracing::debug!("Skipping auto-save for non-FDemon config");
                return;
            }
        }

        // Clone the data needed for async save
        let configs = dialog.launch_context_state.configs.clone();
        let project_path = state.project_path.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            match crate::config::writer::save_fdemon_configs(&project_path, &configs) {
                Ok(()) => {
                    tracing::debug!("Config auto-saved successfully");
                    let _ = tx.send(Message::NewSessionDialogConfigSaved);
                }
                Err(e) => {
                    tracing::error!("Config auto-save failed: {}", e);
                    let _ = tx.send(Message::NewSessionDialogConfigSaveFailed {
                        error: e.to_string(),
                    });
                }
            }
        });
    }
}
```

### 2. Add ConfigSaved/ConfigSaveFailed handlers if missing

Ensure the message handlers exist in `update.rs`:

```rust
Message::NewSessionDialogConfigSaved => {
    tracing::info!("Configuration saved to .fdemon/launch.toml");
    // Optionally show transient notification to user
    None
}

Message::NewSessionDialogConfigSaveFailed { error } => {
    tracing::error!("Failed to save configuration: {}", error);
    // Show error notification to user
    state.notifications.push(Notification::error(
        format!("Failed to save config: {}", error)
    ));
    None
}
```

### 3. Verify save_fdemon_configs function exists

The `config::writer::save_fdemon_configs` function should already exist from task 03. Verify it:
- Takes project path and configs
- Writes to `.fdemon/launch.toml`
- Returns `Result<(), Error>`

## Acceptance Criteria

1. `AutoSaveConfig` action actually saves to `.fdemon/launch.toml`
2. Only FDemon configs are saved (not VSCode or others)
3. Save errors are logged and reported to user
4. No silent failures - users see feedback on save
5. `cargo test config_auto_save` passes

## Verification

```bash
cargo fmt && cargo check && cargo test config && cargo clippy -- -D warnings
```

## Manual Testing

1. Open NewSessionDialog
2. Select an FDemon config
3. Change the mode
4. Check that `.fdemon/launch.toml` is updated
5. Select a VSCode config
6. Change mode (should be blocked, but even if it wasn't, no save should occur)

## Notes

- This task completes the auto-save feature that was stubbed in task 05
- Consider debouncing rapid saves if not already implemented in ConfigAutoSaver
- The async spawn pattern ensures UI doesn't block on file I/O
