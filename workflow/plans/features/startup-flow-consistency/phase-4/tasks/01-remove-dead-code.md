## Task: Remove Dead Code from startup.rs

**Objective**: Remove functions that are no longer used after the startup flow refactor. Clean up the module to contain only the new implementation.

**Depends on**: Phase 3 complete

**Estimated Time**: 1 hour

### Scope

- `src/tui/startup.rs`: Remove dead functions, clean up module

### Details

#### Functions to Remove

After the refactor, these functions in `startup.rs` are no longer called:

1. **`auto_start_session()`** (lines ~82-165)
   - Was called by old `startup_flutter()` when `auto_start=true`
   - Logic moved to `spawn_auto_launch()` in Phase 1

2. **`try_auto_start_config()`** (lines ~167-202)
   - Was called by `auto_start_session()`
   - Logic moved to `find_auto_launch_target()` in Phase 1

3. **`launch_with_validated_selection()`** (lines ~204-216)
   - Was called by `auto_start_session()`
   - Logic moved to `find_auto_launch_target()` in Phase 1

4. **`launch_session()`** (lines ~218-262)
   - Was called by auto-start functions
   - Logic moved to `AutoLaunchResult` handler in Phase 1

5. **`animate_during_async()`** (lines ~27-57)
   - Was used to animate loading during sync device discovery
   - No longer needed - animation handled by event loop

#### Functions to Keep

1. **`startup_flutter()`** - new simplified version
2. **`StartupAction` enum** - new return type
3. **`enter_normal_mode_disconnected()`** - still used (or inline it)
4. **`cleanup_sessions()`** - still used for shutdown

#### Cleanup Steps

1. **Remove dead functions** listed above
2. **Remove unused imports** that were only used by dead functions
3. **Update module doc comment** to reflect new behavior
4. **Consider inlining** `enter_normal_mode_disconnected()` if trivial

#### Updated startup.rs Structure

```rust
//! Startup and cleanup functions for the TUI runner
//!
//! Contains initialization logic and graceful shutdown handling:
//! - `startup_flutter`: Initialize startup state (always enters Normal mode)
//! - `cleanup_sessions`: Session shutdown and process cleanup

use std::path::Path;

use tokio::sync::{mpsc, watch};
use tracing::{info, warn};

use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::state::{AppState, UiMode};
use crate::config::{self, load_all_configs, LoadedConfigs};
use crate::core::LogSource;

use super::actions::SessionTaskMap;
use super::render;

/// Result of startup initialization
#[derive(Debug)]
pub enum StartupAction {
    /// Enter normal mode, no auto-start
    Ready,
    /// Enter normal mode, then trigger auto-start
    AutoStart {
        configs: LoadedConfigs,
    },
}

/// Initialize startup state
///
/// Always enters Normal mode first. Returns whether auto-start
/// should be triggered after the first render.
pub fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    let configs = load_all_configs(project_path);
    state.ui_mode = UiMode::Normal;

    if settings.behavior.auto_start {
        StartupAction::AutoStart { configs }
    } else {
        StartupAction::Ready
    }
}

/// Cleanup sessions on shutdown
///
/// All sessions are managed through the session task system.
/// This function signals all background tasks to shut down and waits for them.
pub async fn cleanup_sessions(
    state: &mut AppState,
    term: &mut ratatui::DefaultTerminal,
    session_tasks: SessionTaskMap,
    shutdown_tx: watch::Sender<bool>,
) {
    // ... existing implementation unchanged ...
}
```

### Verification

After removing dead code:

```bash
# Check for unused code warnings
cargo clippy -- -D warnings

# Ensure no compilation errors
cargo check

# Run tests
cargo test
```

### Acceptance Criteria

1. `auto_start_session()` removed
2. `try_auto_start_config()` removed
3. `launch_with_validated_selection()` removed
4. `launch_session()` removed
5. `animate_during_async()` removed
6. Unused imports removed
7. Module doc comment updated
8. No dead code warnings
9. `cargo check` passes
10. `cargo clippy -- -D warnings` passes

### Notes

- Be careful not to remove `cleanup_sessions()` - it's still used
- The `StartupAction` enum should already exist from Phase 2
- If any function is still referenced, trace the call and fix the caller
- Git diff should show significant line reduction in startup.rs

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Lines Removed:**
- (pending - expect ~150-200 lines)

**Implementation Details:**

(pending)

**Testing Performed:**
- (pending)

**Notable Decisions:**
- (pending)

**Risks/Limitations:**
- (pending)
