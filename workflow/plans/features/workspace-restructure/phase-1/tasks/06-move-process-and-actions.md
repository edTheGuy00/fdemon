## Task: Move process_message, handle_action, and SessionTaskMap from tui/ to app/

**Objective**: Eliminate the `headless/ -> tui/` dependency by moving the TEA event processing and action dispatch logic from `tui/` to `app/`, where it logically belongs. Both TUI and headless runners will then import from `app/`.

**Depends on**: Task 05 (handler dependencies must be cleaned up first, since `handle_action` calls handler code)

**Estimated Time**: 3-4 hours

### Scope

- `src/tui/actions.rs` -> move `SessionTaskMap`, `handle_action()`, and supporting code to `src/app/actions.rs`
- `src/tui/process.rs` -> move `process_message()` to `src/app/process.rs`
- `src/tui/spawn.rs` -> move session spawning logic to `src/app/spawn.rs`
- `src/tui/startup.rs` -> assess and potentially move startup logic
- `src/tui/mod.rs` -> remove/update re-exports
- `src/headless/runner.rs` -> update imports from `tui/` to `app/`
- `src/tui/runner.rs` -> update imports from `tui::actions`/`tui::process` to `app/`

### Details

#### What Moves and Why

The headless runner imports two things from tui/:

1. **`SessionTaskMap`** (`tui/actions.rs:24`):
   ```rust
   pub type SessionTaskMap = Arc<Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>;
   ```
   A pure type alias with no ratatui dependency.

2. **`process::process_message()`** (`tui/process.rs:23`):
   ```rust
   pub fn process_message(
       state: &mut AppState,
       message: Message,
       msg_tx: &mpsc::Sender<Message>,
       session_tasks: &SessionTaskMap,
       shutdown_rx: &watch::Receiver<bool>,
       project_path: &Path,
   )
   ```
   This is the TEA update cycle: calls `handler::update()`, processes follow-up messages, then dispatches `UpdateAction` via `handle_action()`. **No ratatui dependency.**

`process_message()` calls `handle_action()` from `tui/actions.rs`, which in turn calls functions from `tui/spawn.rs`. This entire chain has **no ratatui dependencies** and is pure orchestration logic.

#### Step 1: Move `SessionTaskMap` and `handle_action()` to `app/actions.rs`

Create `src/app/actions.rs`:

```rust
//! Action dispatch for the TEA update loop.
//!
//! When handler::update() returns an UpdateAction, this module
//! executes the corresponding async task (spawn session, discover
//! devices, etc.). Used by both TUI and headless runners.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex};

use crate::app::handler::{Task, UpdateAction};
use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::state::AppState;
// ... other imports from daemon, config, etc.

/// Map of session IDs to their spawned task handles.
pub type SessionTaskMap = Arc<Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>;

/// Dispatch an UpdateAction by spawning the appropriate async task.
pub fn handle_action(
    action: UpdateAction,
    state: &mut AppState,
    msg_tx: &mpsc::Sender<Message>,
    session_tasks: &SessionTaskMap,
    shutdown_rx: &watch::Receiver<bool>,
    project_path: &Path,
) {
    // ... (moved from tui/actions.rs)
}
```

Move ALL content from `src/tui/actions.rs` into `src/app/actions.rs`.

Add to `src/app/mod.rs`:
```rust
pub mod actions;
```

#### Step 2: Move `process_message()` to `app/process.rs`

Create `src/app/process.rs`:

```rust
//! TEA message processing loop.
//!
//! Receives a Message, runs the handler::update() cycle (including
//! follow-up messages), then dispatches any resulting UpdateActions.
//! This is the core of the TEA event loop, shared by TUI and headless.

use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex};
use std::collections::HashMap;

use crate::app::actions::{handle_action, SessionTaskMap};
use crate::app::handler;
use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::state::AppState;
// ... other imports

/// Process a single message through the TEA update cycle.
pub fn process_message(
    state: &mut AppState,
    message: Message,
    msg_tx: &mpsc::Sender<Message>,
    session_tasks: &SessionTaskMap,
    shutdown_rx: &watch::Receiver<bool>,
    project_path: &Path,
) {
    // ... (moved from tui/process.rs)
}
```

Add to `src/app/mod.rs`:
```rust
pub mod process;
```

#### Step 3: Move spawn logic to `app/spawn.rs`

`handle_action()` calls session spawning functions from `tui/spawn.rs`. These also have no ratatui dependency.

Move `src/tui/spawn.rs` to `src/app/spawn.rs`. Update `app/mod.rs`:
```rust
pub mod spawn;
```

Review `tui/startup.rs` -- if it also contains pure logic (device discovery, auto-start) that headless needs, move the relevant functions to `app/`. Startup functions that deal with the TUI terminal or dialogs stay in `tui/`.

#### Step 4: Update headless runner imports

`src/headless/runner.rs` currently imports:

```rust
// Line 20:
use crate::tui::SessionTaskMap;

// Line 183 (scoped):
use crate::tui::process;
```

Change to:
```rust
use crate::app::actions::SessionTaskMap;
use crate::app::process;
```

The headless runner should now have **zero imports from `tui/`**.

#### Step 5: Update TUI runner imports

`src/tui/runner.rs` currently imports from `tui/actions` and `tui/process`. Change to:

```rust
use crate::app::actions::{handle_action, SessionTaskMap};
use crate::app::process::process_message;
```

#### Step 6: Update `tui/mod.rs` re-exports

`src/tui/mod.rs` currently re-exports:
```rust
pub use actions::SessionTaskMap;
```

Either remove this re-export (if all consumers are updated) or leave as a thin re-export:
```rust
pub use crate::app::actions::SessionTaskMap;
```

#### Step 7: Leave thin stubs or delete old files

- `src/tui/actions.rs`: Delete or convert to `pub use crate::app::actions::*;`
- `src/tui/process.rs`: Delete or convert to `pub use crate::app::process::*;`
- `src/tui/spawn.rs`: Delete or convert to `pub use crate::app::spawn::*;`

Prefer deletion if no other TUI code imports from these paths.

### Consumer File Inventory

Files that import from `tui/actions.rs`, `tui/process.rs`, or `tui/spawn.rs`:

| File | Current Import | New Import |
|------|---------------|------------|
| `src/headless/runner.rs:20` | `crate::tui::SessionTaskMap` | `crate::app::actions::SessionTaskMap` |
| `src/headless/runner.rs:183` | `crate::tui::process` | `crate::app::process` |
| `src/tui/runner.rs` | `crate::tui::actions::*` / `crate::tui::process::*` | `crate::app::actions::*` / `crate::app::process::*` |
| `src/tui/mod.rs` | `pub use actions::SessionTaskMap` | `pub use crate::app::actions::SessionTaskMap` or remove |
| `src/tui/startup.rs` | May import from `tui/spawn.rs` | Update to `crate::app::spawn` |

### Acceptance Criteria

1. `src/headless/runner.rs` has zero `use crate::tui::*` imports
2. `SessionTaskMap` is defined in `src/app/actions.rs`
3. `process_message()` is defined in `src/app/process.rs`
4. `handle_action()` is defined in `src/app/actions.rs`
5. TUI runner imports from `app/actions` and `app/process`
6. Headless runner imports from `app/actions` and `app/process`
7. `src/tui/actions.rs` is either deleted or a thin re-export
8. `src/tui/process.rs` is either deleted or a thin re-export
9. `cargo build` succeeds
10. `cargo test` passes
11. `cargo clippy` is clean

### Testing

```bash
cargo test                    # Full suite
cargo test headless           # Headless-specific tests
cargo test handler            # Handler tests (use process_message indirectly)
cargo clippy                  # Lint check
```

Manual verification:
1. Run `fdemon` in TUI mode -- verify normal operation
2. Run `fdemon --headless` -- verify NDJSON output works

### Notes

- **`handle_action` is the largest function** -- it matches on `UpdateAction` variants and spawns tokio tasks for each. It calls into `spawn.rs` for session creation, `daemon::devices` for discovery, etc. All of this is pure orchestration with no rendering.
- **`tui/startup.rs`** contains functions like `startup_flutter()`, `show_device_selector()`, `cleanup_sessions()`. Some of these interact with the terminal (showing dialogs) and must stay in `tui/`. Others (cleanup, auto-start logic) could move. Only move what's needed to break the headless->tui dependency. If headless doesn't use startup functions directly, they can stay.
- **This task is the biggest single win**: It eliminates the dependency that forces headless to import from tui, and establishes the `app/` module as the single source of truth for all non-rendering logic.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/actions.rs` | Created - moved from tui/actions.rs with SessionTaskMap and handle_action() |
| `src/app/process.rs` | Created - moved from tui/process.rs with process_message() |
| `src/app/spawn.rs` | Created - moved from tui/spawn.rs with all background task spawning functions |
| `src/app/mod.rs` | Added actions, process, and spawn module declarations |
| `src/headless/runner.rs` | Updated imports from `crate::tui::*` to `crate::app::*` - **zero tui imports** |
| `src/tui/runner.rs` | Updated imports to use app::actions, app::process, app::spawn |
| `src/tui/startup.rs` | Updated imports to use app::actions and app::spawn |
| `src/tui/mod.rs` | Removed actions, process, spawn modules and re-exports |
| `src/tui/actions.rs` | Deleted (moved to app/) |
| `src/tui/process.rs` | Deleted (moved to app/) |
| `src/tui/spawn.rs` | Deleted (moved to app/) |

### Notable Decisions/Tradeoffs

1. **Keep startup.rs in tui/**: The startup module contains terminal-specific operations (animate_during_async, cleanup_sessions with term.draw()) that require ratatui types. Only moved the spawn helpers it uses to app/, while the startup logic itself stays in tui/ as it's presentation-layer code.

2. **Complete move instead of thin re-exports**: All three files (actions.rs, process.rs, spawn.rs) were fully moved from tui/ to app/ and the old files were deleted. This is cleaner than leaving stub re-exports since there are no internal tui/ dependencies on these modules.

3. **SessionTaskMap now lives in app/**: This is the canonical location for the type alias, and both TUI and headless runners import from here.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test` - Passed (1510 unit tests passed)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

None. This is a pure refactoring with no behavioral changes. The headless runner now has zero tui/ imports as verified by grep.
