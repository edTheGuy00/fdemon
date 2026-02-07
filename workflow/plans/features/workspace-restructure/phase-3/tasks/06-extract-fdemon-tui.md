## Task: Extract fdemon-tui Crate

**Objective**: Move the `tui/` module into the `fdemon-tui` crate. This crate provides the terminal user interface using ratatui. It depends on `fdemon-core` and `fdemon-app` (NOT on `fdemon-daemon` directly, except as a dev-dependency for tests).

**Depends on**: 05-extract-fdemon-app

**Estimated Time**: 3-5 hours

### Scope

#### Files Moving into `fdemon-tui`

- `src/tui/runner.rs` -> `crates/fdemon-tui/src/runner.rs`
- `src/tui/event.rs` -> `crates/fdemon-tui/src/event.rs`
- `src/tui/layout.rs` -> `crates/fdemon-tui/src/layout.rs`
- `src/tui/startup.rs` -> `crates/fdemon-tui/src/startup.rs`
- `src/tui/terminal.rs` -> `crates/fdemon-tui/src/terminal.rs`
- `src/tui/selector.rs` -> `crates/fdemon-tui/src/selector.rs`
- `src/tui/editor.rs` -> `crates/fdemon-tui/src/editor.rs`
- `src/tui/hyperlinks.rs` -> `crates/fdemon-tui/src/hyperlinks.rs`
- `src/tui/test_utils.rs` -> `crates/fdemon-tui/src/test_utils.rs`
- `src/tui/render/` (directory) -> `crates/fdemon-tui/src/render/`
- `src/tui/widgets/` (directory) -> `crates/fdemon-tui/src/widgets/`

### Details

#### 1. Write `lib.rs`

```rust
//! fdemon-tui - Terminal UI for Flutter Demon
//!
//! This crate provides the ratatui-based terminal interface. It creates an Engine
//! from fdemon-app and adds terminal rendering, event polling, and widget display.

pub mod editor;
pub mod event;
pub mod hyperlinks;
pub mod layout;
pub mod render;
pub mod runner;
pub mod selector;
pub mod startup;
pub mod terminal;
pub mod widgets;

#[cfg(test)]
pub mod test_utils;

// Re-export main entry points
pub use runner::{run, run_with_project};
pub use selector::{select_project, SelectionResult};

// Re-export types used by binary
pub use editor::{open_in_editor, EditorError, OpenResult};
pub use hyperlinks::FileReference;
```

#### 2. Update Internal Imports

| Old Pattern | New Pattern |
|-------------|-------------|
| `use crate::common::prelude::*` | `use fdemon_core::prelude::*` |
| `use crate::core::*` | `use fdemon_core::*` (or specific submodule) |
| `use crate::app::state::AppState` | `use fdemon_app::state::AppState` |
| `use crate::app::message::Message` | `use fdemon_app::message::Message` |
| `use crate::app::Engine` | `use fdemon_app::Engine` |
| `use crate::app::session::*` | `use fdemon_app::session::*` |
| `use crate::app::session_manager::*` | `use fdemon_app::session_manager::*` |
| `use crate::app::handler::*` | `use fdemon_app::handler::*` |
| `use crate::app::UpdateAction` | `use fdemon_app::UpdateAction` |
| `use crate::app::spawn::*` | `use fdemon_app::spawn::*` |
| `use crate::app::log_view_state::*` | `use fdemon_app::log_view_state::*` |
| `use crate::app::confirm_dialog::*` | `use fdemon_app::confirm_dialog::*` |
| `use crate::app::hyperlinks::*` | `use fdemon_app::hyperlinks::*` |
| `use crate::app::new_session_dialog::*` | `use fdemon_app::new_session_dialog::*` |
| `use crate::app::settings_items::*` | `use fdemon_app::settings_items::*` |
| `use crate::config::*` | `use fdemon_app::config::*` |
| `use crate::daemon::Device` | `use fdemon_app` re-export or `fdemon_daemon::Device` |
| `use crate::tui::*` | `use crate::*` (now same crate) |

#### 3. Handle `daemon` Type References in TUI

Several TUI files import daemon types directly:
- `widgets/status_bar/tests.rs` - `use crate::daemon::Device` (test only)
- `widgets/new_session_dialog/target_selector.rs` - daemon types
- `widgets/new_session_dialog/device_list.rs` - `Device`, `AndroidAvd`, etc.
- `widgets/new_session_dialog/device_groups.rs` - device types
- `startup.rs` - `Device`, `ToolAvailability`
- `test_utils.rs` - `Device`

**Approach**: These daemon types should be accessed through `fdemon-app` re-exports where possible. `fdemon-app` already depends on `fdemon-daemon` and can re-export types that TUI needs:

```rust
// In fdemon-app/src/lib.rs, add:
pub use fdemon_daemon::{Device, AndroidAvd, IosSimulator, SimulatorState, ToolAvailability};
```

For test-only imports, `fdemon-daemon` is listed as a dev-dependency of `fdemon-tui`.

#### 4. Handle `pub use crate::app::*` Re-export Bridges

The TUI currently has several `pub use` re-export bridges from `app`:
- `tui/widgets/log_view/state.rs` -> `pub use crate::app::log_view_state::*`
- `tui/widgets/confirm_dialog.rs` -> `pub use crate::app::confirm_dialog::*`
- `tui/editor.rs` -> `pub use crate::app::editor::*`
- `tui/hyperlinks.rs` -> `pub use crate::app::hyperlinks::*`
- `tui/widgets/new_session_dialog/state/mod.rs` -> `pub use crate::app::new_session_dialog::*`
- `tui/widgets/new_session_dialog/fuzzy_modal.rs` -> `pub use crate::app::new_session_dialog::fuzzy::*`
- `tui/widgets/new_session_dialog/target_selector.rs` -> `pub use crate::app::new_session_dialog::TargetSelectorState`
- `tui/widgets/new_session_dialog/device_groups.rs` -> `pub use crate::app::new_session_dialog::device_groups::*`

These become:
```rust
pub use fdemon_app::log_view_state::*;
pub use fdemon_app::confirm_dialog::*;
// etc.
```

#### 5. Handle `insta` Snapshot Tests

`render/tests.rs` uses `insta` for snapshot testing. Ensure snapshots are stored relative to the new crate location. `insta` respects `CARGO_MANIFEST_DIR`, so snapshots will naturally move to `crates/fdemon-tui/src/render/snapshots/`.

Copy any existing snapshot files from `src/tui/render/snapshots/` to `crates/fdemon-tui/src/render/snapshots/`.

### Acceptance Criteria

1. `crates/fdemon-tui/src/` contains all TUI module files
2. `cargo check -p fdemon-tui` passes
3. `cargo test -p fdemon-tui` passes (render tests, widget tests, snapshot tests)
4. `fdemon-tui` depends on `fdemon-core` + `fdemon-app` (not `fdemon-daemon` in regular deps)
5. `fdemon-tui` has `fdemon-daemon` only in `[dev-dependencies]` (for test utilities)
6. `cargo check` (full workspace) passes
7. `cargo test` (full workspace) passes

### Testing

```bash
# Test the new crate in isolation
cargo check -p fdemon-tui
cargo test -p fdemon-tui

# Verify snapshot tests
cargo test -p fdemon-tui -- render

# Test full workspace
cargo check
cargo test
```

### Notes

- TUI has ~25 files including widgets. The widgets subdirectory is the largest with several multi-file modules (`log_view/`, `status_bar/`, `settings_panel/`, `new_session_dialog/`).
- The `test_utils.rs` module provides `TestTerminal` which wraps ratatui test helpers. It's `#[cfg(test)]` but also `pub` for use by other test modules. In the workspace, it might need to be behind a `test-utils` feature flag if other crates need it.
- Snapshot files (`.snap`) from `insta` must be copied to the new location. If they don't exist yet, they'll be auto-created on first test run.
- The `selector.rs` provides `select_project()` which is used by `main.rs`. This becomes `fdemon_tui::select_project()`.
- After this task, the original `src/tui/` directory can be reduced to a shim, but since the binary is being updated in task 07, the shim is temporary.
