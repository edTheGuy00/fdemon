## Task: Add Full-Screen Snapshot Tests

**Objective**: Create insta snapshot tests for complete screen renders in each UI mode using TestBackend.

**Depends on**: 07-header-widget-tests, 08-statusbar-widget-tests, 09-device-selector-tests, 10-confirm-dialog-tests

### Scope

- `src/tui/render/tests.rs`: **NEW** - Full-screen snapshot tests
- `src/tui/render.rs`: Convert to directory module if needed
- `src/tui/snapshots/`: Snapshot storage

### Details

#### 1. Create Test File

Create `src/tui/render/tests.rs`:

```rust
//! Full-screen snapshot tests for TUI rendering
//!
//! These tests capture the entire screen render for each UI mode
//! and compare against golden snapshots using insta.

use super::view;
use crate::app::state::{AppState, UiMode};
use crate::core::AppPhase;
use crate::tui::test_utils::TestTerminal;
use insta::assert_snapshot;
use std::path::PathBuf;

fn create_base_state() -> AppState {
    let mut state = AppState::new(PathBuf::from("/test/flutter_app"));
    state.project_name = Some("flutter_app".to_string());
    state
}

// Helper to render full screen and return content
fn render_screen(state: &mut AppState) -> String {
    let mut term = TestTerminal::new();
    term.terminal.draw(|frame| view(frame, state)).unwrap();
    term.content()
}

// ===========================================================================
// Normal Mode Snapshots
// ===========================================================================

#[test]
fn snapshot_normal_mode_initializing() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Initializing;

    let content = render_screen(&mut state);
    assert_snapshot!("normal_initializing", content);
}

#[test]
fn snapshot_normal_mode_running() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;
    state.device_name = Some("Linux Desktop".to_string());
    state.reload_count = 3;

    let content = render_screen(&mut state);
    assert_snapshot!("normal_running", content);
}

#[test]
fn snapshot_normal_mode_reloading() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Reloading;
    state.device_name = Some("Linux Desktop".to_string());

    let content = render_screen(&mut state);
    assert_snapshot!("normal_reloading", content);
}

#[test]
fn snapshot_normal_mode_error() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Error;
    // Add some error logs
    state.add_log_entry("Error: Compilation failed".to_string(), crate::core::LogLevel::Error);

    let content = render_screen(&mut state);
    assert_snapshot!("normal_error", content);
}

// ===========================================================================
// Device Selector Mode Snapshots
// ===========================================================================

#[test]
fn snapshot_device_selector_with_devices() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::DeviceSelector;

    // Add mock devices
    // state.device_selector.add_device(...);

    let content = render_screen(&mut state);
    assert_snapshot!("device_selector_with_devices", content);
}

#[test]
fn snapshot_device_selector_empty() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::DeviceSelector;
    // No devices added

    let content = render_screen(&mut state);
    assert_snapshot!("device_selector_empty", content);
}

// ===========================================================================
// Confirm Dialog Mode Snapshots
// ===========================================================================

#[test]
fn snapshot_confirm_dialog_quit() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit());

    let content = render_screen(&mut state);
    assert_snapshot!("confirm_dialog_quit", content);
}

// ===========================================================================
// Loading Mode Snapshots
// ===========================================================================

#[test]
fn snapshot_loading_mode() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Loading;
    state.loading_state = Some(LoadingState::new("Starting Flutter..."));

    let content = render_screen(&mut state);
    assert_snapshot!("loading", content);
}

// ===========================================================================
// Compact Terminal Snapshots
// ===========================================================================

#[test]
fn snapshot_compact_normal() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;

    let mut term = TestTerminal::compact();
    term.terminal.draw(|frame| view(frame, &mut state)).unwrap();

    assert_snapshot!("compact_normal", term.content());
}

#[test]
fn snapshot_compact_device_selector() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::DeviceSelector;

    let mut term = TestTerminal::compact();
    term.terminal.draw(|frame| view(frame, &mut state)).unwrap();

    assert_snapshot!("compact_device_selector", term.content());
}
```

#### 2. Update render.rs Module Structure

If `render.rs` is a file, convert to directory module:

```
src/tui/render.rs  →  src/tui/render/mod.rs
                      src/tui/render/tests.rs
```

Add to `src/tui/render/mod.rs`:
```rust
#[cfg(test)]
mod tests;
```

#### 3. Create Snapshots Directory

```bash
mkdir -p src/tui/snapshots
```

Snapshots will be stored as:
```
src/tui/snapshots/
├── render__tests__snapshot_normal_initializing.snap
├── render__tests__snapshot_normal_running.snap
├── render__tests__snapshot_device_selector.snap
└── ...
```

### Test Coverage

| Snapshot | UI Mode | Phase/State |
|----------|---------|-------------|
| `normal_initializing` | Normal | Initializing |
| `normal_running` | Normal | Running |
| `normal_reloading` | Normal | Reloading |
| `normal_error` | Normal | Error |
| `device_selector_with_devices` | DeviceSelector | Populated |
| `device_selector_empty` | DeviceSelector | Empty |
| `confirm_dialog_quit` | ConfirmDialog | Quit |
| `loading` | Loading | Starting |
| `compact_normal` | Normal (40x12) | Running |
| `compact_device_selector` | DeviceSelector (40x12) | - |

### Acceptance Criteria

1. Snapshots capture full screen renders
2. All UI modes have snapshots
3. Compact terminal variants tested
4. `cargo insta test` workflow works
5. Snapshots committed to version control

### Testing

```bash
# Generate/update snapshots
cargo test tui::render::tests --lib

# Review pending snapshots
cargo insta review

# Accept snapshots
cargo insta accept

# Verify no changes
cargo insta test --check
```

### Notes

- Run `cargo insta review` after first run to accept snapshots
- Snapshot content depends on terminal dimensions
- Dynamic content (time, paths) should be filtered if present
- Keep snapshots in version control for regression detection

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/test_utils.rs` | Made `terminal` field public for direct test access |
| `src/tui/render.rs` → `src/tui/render/mod.rs` | Converted to directory module |
| `src/tui/render/tests.rs` | **NEW** - Full-screen snapshot tests for all UI modes |
| `src/tui/render/snapshots/` | **NEW** - 14 snapshot files for regression testing |

### Notable Decisions/Tradeoffs

1. **Loading Message Filter**: Used insta's filter feature to normalize randomized loading messages with regex `r"\s*⠋[^│\n]+"` to handle variable whitespace padding and message truncation at box borders.

2. **Test Coverage**: Created 15 tests covering all major UI modes:
   - Normal mode (4 states: Initializing, Running, Reloading, Stopped)
   - Device Selector (empty and populated)
   - Confirm Dialog (single and multiple sessions)
   - Loading screen (with filtered random message)
   - Settings mode
   - Compact terminal variants (2 tests)
   - Edge cases (no project name, very long project name)
   - SearchInput mode (basic render verification)

3. **Module Structure**: Converted `render.rs` to a directory module to accommodate tests file, following the pattern used by other widgets like `log_view`.

4. **TestTerminal Enhancement**: Made the `terminal` field public to allow tests direct access to `draw()` method for full-screen rendering.

### Testing Performed

- `cargo test tui::render::tests --lib -- --nocapture` - **Passed** (15 tests, 100% success rate across 3 consecutive runs)
- `cargo test --lib` - **Passed** (1312 tests)
- `cargo clippy --lib` - **Passed** (no warnings)
- `cargo fmt --check` - **Passed** (formatted)

### Risks/Limitations

1. **Snapshot Stability**: Loading screen snapshot requires regex filtering due to randomized messages. Filter is robust but may need adjustment if loading message format changes.

2. **SearchInput Mode**: Test verifies basic rendering without a session. Future enhancement could add session-based search input testing once session mocking utilities are available.

3. **Snapshot Maintenance**: 14 snapshot files will need review whenever UI layout changes. This is by design for regression detection but requires developer attention during reviews.
