# Task: Add Error Handling to Config Loading

## Summary

Add proper error handling and logging when loading launch configurations for the new session dialog.

**Priority:** Major

## Files

| File | Action |
|------|--------|
| `src/app/handler/new_session/navigation.rs` | Modify (lines 160-166) |

## Problem

Current code at `src/app/handler/new_session/navigation.rs:160-166`:

```rust
let configs = crate::config::load_all_configs(&state.project_path);
```

No error handling or user feedback if config loading fails.

## Implementation

Add error handling and informational logging:

```rust
pub fn handle_open_new_session_dialog(state: &mut AppState) -> UpdateResult {
    // Load configs with error handling
    let configs = crate::config::load_all_configs(&state.project_path);

    // Log warning if no configs found (not an error, just informational)
    if configs.configs.is_empty() {
        tracing::info!("No launch configurations found, using defaults");
    }

    // Show the dialog
    state.show_new_session_dialog(configs);

    // Trigger device discovery
    UpdateResult::action(UpdateAction::DiscoverDevices)
}
```

### If `load_all_configs` returns Result

If the function returns a `Result`, handle errors gracefully:

```rust
pub fn handle_open_new_session_dialog(state: &mut AppState) -> UpdateResult {
    // Load configs with error handling
    let configs = match crate::config::load_all_configs(&state.project_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to load launch configurations: {}", e);
            LoadedConfigs::default()
        }
    };

    if configs.configs.is_empty() {
        tracing::info!("No launch configurations found, using defaults");
    }

    state.show_new_session_dialog(configs);
    UpdateResult::action(UpdateAction::DiscoverDevices)
}
```

## Acceptance Criteria

1. Config loading failures are handled gracefully (no panics)
2. Empty configs results in informational log message
3. Dialog still opens even if configs fail to load
4. User gets reasonable defaults when configs unavailable

## Testing

```bash
cargo fmt && cargo check && cargo clippy -- -D warnings
```

## Notes

- Check actual return type of `load_all_configs()` before implementing
- Use `tracing::info!` for expected cases (no configs)
- Use `tracing::warn!` for unexpected failures
- Don't show error dialogs to user for config loading - just use defaults
