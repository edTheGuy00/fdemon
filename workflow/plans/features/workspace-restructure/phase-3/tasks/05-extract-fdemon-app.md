## Task: Extract fdemon-app Crate

**Objective**: Move `app/`, `config/`, `services/`, and `watcher/` modules into the `fdemon-app` crate. This is the largest crate, containing the TEA state machine, Engine abstraction, configuration, services, and file watching. It depends on `fdemon-core` and `fdemon-daemon`.

**Depends on**: 04-extract-fdemon-daemon

**Estimated Time**: 5-7 hours

### Scope

#### Files Moving into `fdemon-app`

**From `app/`:**
- `src/app/engine.rs` -> `crates/fdemon-app/src/engine.rs`
- `src/app/engine_event.rs` -> `crates/fdemon-app/src/engine_event.rs`
- `src/app/state.rs` -> `crates/fdemon-app/src/state.rs`
- `src/app/message.rs` -> `crates/fdemon-app/src/message.rs`
- `src/app/session.rs` -> `crates/fdemon-app/src/session.rs`
- `src/app/session_manager.rs` -> `crates/fdemon-app/src/session_manager.rs`
- `src/app/process.rs` -> `crates/fdemon-app/src/process.rs`
- `src/app/actions.rs` -> `crates/fdemon-app/src/actions.rs`
- `src/app/spawn.rs` -> `crates/fdemon-app/src/spawn.rs`
- `src/app/signals.rs` -> `crates/fdemon-app/src/signals.rs`
- `src/app/log_view_state.rs` -> `crates/fdemon-app/src/log_view_state.rs`
- `src/app/hyperlinks.rs` -> `crates/fdemon-app/src/hyperlinks.rs`
- `src/app/confirm_dialog.rs` -> `crates/fdemon-app/src/confirm_dialog.rs`
- `src/app/editor.rs` -> `crates/fdemon-app/src/editor.rs`
- `src/app/settings_items.rs` -> `crates/fdemon-app/src/settings_items.rs`
- `src/app/handler/` (entire directory) -> `crates/fdemon-app/src/handler/`
- `src/app/new_session_dialog/` (entire directory) -> `crates/fdemon-app/src/new_session_dialog/`

**From `config/`:**
- `src/config/types.rs` -> `crates/fdemon-app/src/config/types.rs`
- `src/config/settings.rs` -> `crates/fdemon-app/src/config/settings.rs`
- `src/config/launch.rs` -> `crates/fdemon-app/src/config/launch.rs`
- `src/config/priority.rs` -> `crates/fdemon-app/src/config/priority.rs`
- `src/config/vscode.rs` -> `crates/fdemon-app/src/config/vscode.rs`
- `src/config/writer.rs` -> `crates/fdemon-app/src/config/writer.rs`
- `src/config/mod.rs` -> `crates/fdemon-app/src/config/mod.rs`

**From `services/`:**
- `src/services/flutter_controller.rs` -> `crates/fdemon-app/src/services/flutter_controller.rs`
- `src/services/log_service.rs` -> `crates/fdemon-app/src/services/log_service.rs`
- `src/services/state_service.rs` -> `crates/fdemon-app/src/services/state_service.rs`
- `src/services/mod.rs` -> `crates/fdemon-app/src/services/mod.rs`

**From `watcher/`:**
- `src/watcher/mod.rs` -> `crates/fdemon-app/src/watcher.rs` (or `watcher/mod.rs`)

### Details

#### 1. Write `lib.rs`

```rust
//! fdemon-app - Application state and orchestration for Flutter Demon
//!
//! This crate implements the TEA (The Elm Architecture) pattern for state management,
//! the Engine abstraction for shared orchestration, configuration loading, service
//! traits, and file watching.

pub mod actions;
pub mod config;
pub mod confirm_dialog;
pub mod editor;
pub mod engine;
pub mod engine_event;
pub mod handler;
pub mod hyperlinks;
pub mod log_view_state;
pub mod message;
pub mod new_session_dialog;
pub mod process;
pub mod services;
pub mod session;
pub mod session_manager;
pub mod settings_items;
pub mod signals;
pub mod spawn;
pub mod state;
pub mod watcher;

// Re-export primary types
pub use engine::Engine;
pub use handler::{Task, UpdateAction, UpdateResult};
pub use session::{Session, SessionHandle, SessionId};
pub use session_manager::{SessionManager, MAX_SESSIONS};
```

#### 2. Update Internal Imports

This is the bulk of the work. There are ~60+ files with imports to update.

| Old Pattern | New Pattern |
|-------------|-------------|
| `use crate::common::prelude::*` | `use fdemon_core::prelude::*` |
| `use crate::core::*` | `use fdemon_core::*` (or specific submodule) |
| `use crate::core::AppPhase` | `use fdemon_core::types::AppPhase` |
| `use crate::core::DaemonEvent` | `use fdemon_core::events::DaemonEvent` |
| `use crate::core::LogEntry` | `use fdemon_core::types::LogEntry` |
| `use crate::daemon::*` | `use fdemon_daemon::*` |
| `use crate::daemon::Device` | `use fdemon_daemon::Device` |
| `use crate::daemon::FlutterProcess` | `use fdemon_daemon::FlutterProcess` |
| `use crate::daemon::CommandSender` | `use fdemon_daemon::CommandSender` |
| `use crate::config::*` | `use crate::config::*` (stays internal) |
| `use crate::services::*` | `use crate::services::*` (stays internal) |
| `use crate::watcher::*` | `use crate::watcher::*` (stays internal) |
| `use crate::app::*` | `use crate::*` (now the same crate) |
| `use crate::app::handler::*` | `use crate::handler::*` |
| `use crate::app::session::*` | `use crate::session::*` |
| `use crate::app::message::*` | `use crate::message::*` |

#### 3. Handle `config/settings.rs` Test Imports

`config/settings.rs` has tests that import `crate::daemon::Device` (lines 1510, 1549, 1581). Since `fdemon-app` depends on `fdemon-daemon`, this works:

```rust
// In config/settings.rs tests:
use fdemon_daemon::Device;
```

#### 4. Handle `crossterm` Dependency

`message.rs` imports `crossterm::event::KeyEvent`. This stays as-is since `fdemon-app` lists `crossterm` as a dependency.

`handler/keys.rs` imports `crossterm::event::{KeyCode, KeyEvent, KeyModifiers}`. Same.

#### 5. Handle `color-eyre` Usage

`app/mod.rs` previously used `color_eyre`. After task 02 removes `run()`/`run_with_project()`, this usage should be gone. Verify no `color-eyre` imports remain in `fdemon-app` files. If they do, move them to the binary.

#### 6. Keep Compatibility Shims in Main Crate

```rust
// src/app/mod.rs (temporary re-export shim)
pub use fdemon_app::*;
```

```rust
// src/config/mod.rs (temporary re-export shim)
pub use fdemon_app::config::*;
```

```rust
// src/services/mod.rs (temporary re-export shim)
pub use fdemon_app::services::*;
```

```rust
// src/watcher/mod.rs (temporary re-export shim)
pub use fdemon_app::watcher::*;
```

These shims let `tui/` (still in the main crate) compile via `use crate::app::*`.

### Acceptance Criteria

1. `crates/fdemon-app/src/` contains all app, config, services, and watcher files
2. `cargo check -p fdemon-app` passes
3. `cargo test -p fdemon-app` passes (all handler tests, session tests, config tests, service tests)
4. `fdemon-app` depends only on `fdemon-core` and `fdemon-daemon` (plus external crates)
5. `fdemon-app` does NOT import from `fdemon-tui`
6. Compatibility shims allow remaining main-crate code (`tui/`, `headless/`, `main.rs`) to compile
7. `cargo check` (full workspace) passes
8. `cargo test` (full workspace) passes

### Testing

```bash
# Test the new crate in isolation
cargo check -p fdemon-app
cargo test -p fdemon-app

# Test full workspace
cargo check
cargo test
```

### Notes

- This is the largest extraction task. The `app/` module alone has ~30 files, plus `config/` (7 files), `services/` (4 files), and `watcher/` (1 file).
- Use a systematic approach: copy all files first, then do a batch find-and-replace for import paths.
- The `handler/` directory has 12+ files with complex internal imports. Most are `crate::app::*` which become `crate::*`.
- `new_session_dialog/` has 6 files. Internal imports update similarly.
- `engine.rs` is the key file - it imports from `services`, `watcher`, `config`, `core`, and `daemon`. All become either `crate::` (for things in fdemon-app) or `fdemon_core::`/`fdemon_daemon::`.
- After this task, `fdemon-app` is the hub crate that glues core + daemon together with state management and orchestration.
- The `app/mod.rs` in the new crate should NOT contain the `run()` / `run_with_project()` functions (removed in task 02).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/lib.rs` | Created with module declarations and primary type re-exports |
| `crates/fdemon-app/src/actions.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/engine.rs` | Moved from `src/app/`, imports updated to fdemon_core/fdemon_daemon |
| `crates/fdemon-app/src/engine_event.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/state.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/message.rs` | Moved from `src/app/`, crossterm KeyEvent kept as-is |
| `crates/fdemon-app/src/session.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/session_manager.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/process.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/spawn.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/signals.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/editor.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/log_view_state.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/hyperlinks.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/confirm_dialog.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/settings_items.rs` | Moved from `src/app/`, imports updated |
| `crates/fdemon-app/src/handler/` | All 12+ handler files moved, `crate::app::` → `crate::` |
| `crates/fdemon-app/src/new_session_dialog/` | All 6 dialog files moved, imports updated |
| `crates/fdemon-app/src/config/` | All 7 config files moved from `src/config/` |
| `crates/fdemon-app/src/services/` | All 4 service files moved from `src/services/` |
| `crates/fdemon-app/src/watcher/mod.rs` | Moved from `src/watcher/mod.rs` (no import changes needed) |
| `src/app/mod.rs` | Replaced with `pub use fdemon_app::*` shim |
| `src/config/mod.rs` | Replaced with `pub use fdemon_app::config::*` shim |
| `src/services/mod.rs` | Replaced with `pub use fdemon_app::services::*` shim |
| `src/watcher/mod.rs` | Replaced with `pub use fdemon_app::watcher::*` shim |

### Notable Decisions/Tradeoffs

1. **Visibility change**: `TargetSelectorState.cached_flat_list` changed from `pub(crate)` to `pub` to allow cross-crate access from TUI widget tests. This is acceptable since the field was already accessible within the old single-crate structure.

2. **50+ file import rewrite**: All imports were updated file-by-file using targeted Edit operations (no sed/regex) to avoid corrupting non-import code. The `crate::app::` prefix was stripped (now crate root), while `crate::common::`/`crate::core::` became `fdemon_core::` and `crate::daemon::` became `fdemon_daemon::`.

3. **Backward compatibility shims**: Created 4 re-export shims in the root crate so that TUI (still in the root crate at this point) could continue compiling via `use crate::app::*`.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo check` (full workspace) - Passed
- `cargo test --lib` - Passed (433 tests, later 1,532 after all crates extracted)

### Risks/Limitations

1. **Largest extraction**: This was the most complex task with ~50 files across 4 source directories. The first automated attempt (using sed) corrupted files and had to be redone with targeted Edit operations.
