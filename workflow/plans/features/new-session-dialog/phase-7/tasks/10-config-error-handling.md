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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/new_session/navigation.rs` | Added informational logging when no launch configurations are found (lines 159-165) |

### Notable Decisions/Tradeoffs

1. **Used `tracing::info!` instead of `tracing::warn!`**: Since `load_all_configs` returns `LoadedConfigs` directly (not a `Result`), there are no loading failures to handle. An empty config list is an expected case (projects without custom launch configs), so `info!` level is appropriate rather than `warn!`.

2. **No error handling for loading failures**: The function signature of `load_all_configs(&Path) -> LoadedConfigs` doesn't return a `Result`, so there are no errors to catch. The function internally handles any file reading issues and returns an empty `LoadedConfigs` on failure. This aligns with the "graceful degradation" approach - the dialog still opens with defaults.

3. **Checked `configs.configs.is_empty()` instead of `configs.is_empty`**: While `LoadedConfigs` has an `is_empty` field, checking `configs.configs.is_empty()` is more explicit and doesn't rely on the field being properly updated.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (3.17s)
- `cargo clippy -- -D warnings` - Passed (3.34s)

### Risks/Limitations

1. **No visibility of parsing errors**: Since `load_all_configs` doesn't return errors, if there's a malformed TOML or JSON file, the user won't see any feedback beyond the info log. This is acceptable per the task requirements ("don't show error dialogs to user for config loading - just use defaults"), but may make debugging config issues harder.

2. **Log visibility**: The `tracing::info!` log is written to the log file (not visible in the TUI). Users won't see this message unless they check the log file. This is intentional per the task requirements.
