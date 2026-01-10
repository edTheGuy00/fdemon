## Task: Simplify startup_flutter() to Always Enter Normal Mode

**Objective**: Modify `startup_flutter()` to always enter Normal mode and return a signal indicating whether auto-start should be triggered, instead of running async device discovery synchronously.

**Depends on**: Phase 1 complete

**Estimated Time**: 1 hour

### Scope

- `src/tui/startup.rs`: Simplify `startup_flutter()` function

### Details

#### Current Behavior

```rust
pub async fn startup_flutter(...) -> Option<UpdateAction> {
    let configs = load_all_configs(project_path);

    if settings.behavior.auto_start {
        auto_start_session(state, &configs, ...).await  // Blocks with device discovery
    } else {
        enter_normal_mode_disconnected(state)  // Returns None
    }
}
```

#### New Behavior

Replace the async function with a sync function that returns a `StartupAction` enum:

```rust
/// Result of startup initialization
pub enum StartupAction {
    /// Enter normal mode, no auto-start
    Ready,
    /// Enter normal mode, then trigger auto-start
    AutoStart {
        /// Pre-loaded configs for auto-start flow
        configs: LoadedConfigs,
    },
}

/// Initialize startup state
///
/// Always enters Normal mode. Returns whether auto-start should be triggered.
/// The caller is responsible for sending the auto-start message if needed.
pub fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    // Load configs upfront (needed for auto-start path)
    let configs = load_all_configs(project_path);

    // Always enter Normal mode first
    state.ui_mode = UiMode::Normal;

    if settings.behavior.auto_start {
        StartupAction::AutoStart { configs }
    } else {
        StartupAction::Ready
    }
}
```

### Changes Required

1. **Remove `async`** from function signature
2. **Remove unused parameters**: `msg_tx`, `term` (no longer needed)
3. **Change return type** from `Option<UpdateAction>` to `StartupAction`
4. **Remove the call to `auto_start_session()`** - this logic moves to Phase 1's spawn function
5. **Always set `UiMode::Normal`** regardless of auto_start setting
6. **Return enum** indicating whether auto-start should happen

### Keep These Functions (for now)

Keep the following functions in `startup.rs` - they will be removed in Phase 4:
- `auto_start_session()` - dead code after this change
- `try_auto_start_config()` - dead code
- `launch_with_validated_selection()` - dead code
- `launch_session()` - dead code
- `animate_during_async()` - dead code

Keep these functions (still used):
- `enter_normal_mode_disconnected()` - can be inlined or kept
- `cleanup_sessions()` - still needed for shutdown

### Acceptance Criteria

1. `startup_flutter()` is no longer async
2. `startup_flutter()` always sets `UiMode::Normal`
3. `startup_flutter()` returns `StartupAction` enum
4. Function signature removes `msg_tx` and `term` parameters
5. No compilation errors (runner will be updated in next task)
6. `cargo check` passes (may need to temporarily comment out runner call)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_flutter_auto_start_returns_configs() {
        let mut state = AppState::new();
        let mut settings = Settings::default();
        settings.behavior.auto_start = true;
        let project_path = Path::new("/tmp/test");

        let result = startup_flutter(&mut state, &settings, project_path);

        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(matches!(result, StartupAction::AutoStart { .. }));
    }

    #[test]
    fn test_startup_flutter_no_auto_start_returns_ready() {
        let mut state = AppState::new();
        let mut settings = Settings::default();
        settings.behavior.auto_start = false;
        let project_path = Path::new("/tmp/test");

        let result = startup_flutter(&mut state, &settings, project_path);

        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(matches!(result, StartupAction::Ready));
    }
}
```

### Notes

- This task intentionally breaks the runner temporarily - Task 02 fixes it
- The dead code will produce warnings; that's expected until Phase 4
- Consider adding `#[allow(dead_code)]` temporarily to suppress warnings
- The `StartupAction` enum is simple enough to define in `startup.rs`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/startup.rs` | - Added `StartupAction` enum with `Ready` and `AutoStart` variants<br>- Changed `startup_flutter()` from async to sync function<br>- Removed `msg_tx` and `term` parameters<br>- Changed return type from `Option<UpdateAction>` to `StartupAction`<br>- Always sets `UiMode::Normal` immediately<br>- Added `#[allow(dead_code)]` to 5 functions: `animate_during_async`, `auto_start_session`, `try_auto_start_config`, `launch_with_validated_selection`, `launch_session`, `enter_normal_mode_disconnected`<br>- Added unit tests for both auto-start and manual modes |
| `src/tui/runner.rs` | - Temporarily commented out the call to `startup_flutter()` with TODO comment<br>- Removed unused import `handle_action` (will be re-enabled in Task 02) |

### Notable Decisions/Tradeoffs

1. **Sync over Async**: Changed `startup_flutter()` from async to sync, removing the blocking device discovery from the startup path. This aligns with Phase 1's deferred device discovery approach.

2. **Dead Code Attributes**: Added `#[allow(dead_code)]` to 6 functions that are no longer called but will be removed in Phase 4. This keeps the codebase clean while preserving code for reference during the refactor.

3. **Temporary Comment in Runner**: Commented out the call to `startup_flutter()` in `runner.rs` to allow `cargo check` to pass. Task 02 will update the runner to use the new signature.

4. **Always Normal Mode**: The function now always enters Normal mode immediately, regardless of auto_start setting. This provides a consistent initial state for the TUI.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib startup::tests` - Passed (2 new tests)
- `cargo test --lib` - Passed (1337 tests total)
- `cargo clippy --lib` - Passed (no warnings)

### Risks/Limitations

1. **Temporary Breakage**: The runner is temporarily broken and commented out. The application cannot be run until Task 02 is complete. This is expected and documented in the task plan.

2. **Dead Code**: Six functions are marked with `#[allow(dead_code)]` and will remain unused until Phase 4 cleanup. This is intentional to preserve code for reference.
