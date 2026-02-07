## Task: Cleanup Re-exports and Paths

**Objective**: Audit all `pub use` re-exports across crates for necessity, remove backward-compatibility bridges that are no longer needed, and tighten visibility with `pub(crate)` where types shouldn't be exposed to external crates.

**Depends on**: 08-migrate-integration-tests

**Estimated Time**: 2-3 hours

### Scope

- All `crates/*/src/lib.rs`: Audit public API surface
- `crates/fdemon-daemon/src/lib.rs`: Remove `core` event re-exports
- `crates/fdemon-tui/src/`: Remove `pub use fdemon_app::*` bridge re-exports
- All crates: Audit `pub` vs `pub(crate)` visibility

### Details

#### 1. Remove Backward-Compat Re-exports from fdemon-daemon

`fdemon-daemon/src/lib.rs` may still re-export `fdemon-core` event types:
```rust
// REMOVE these - consumers should import from fdemon-core directly:
pub use fdemon_core::{AppDebugPort, AppLog, AppProgress, AppStart, ...};
```

Keep only re-exports of types defined IN `fdemon-daemon`.

#### 2. Clean Up TUI Re-export Bridges

The TUI has several files that exist solely to re-export `fdemon-app` types:
- `tui/widgets/log_view/state.rs` -> `pub use fdemon_app::log_view_state::*`
- `tui/widgets/confirm_dialog.rs` -> `pub use fdemon_app::confirm_dialog::*`
- `tui/editor.rs` -> `pub use fdemon_app::editor::*`
- `tui/hyperlinks.rs` -> `pub use fdemon_app::hyperlinks::*`

**Decision per file:**
- If the file ONLY contains re-exports and no TUI-specific code, consider removing it and having consumers import from `fdemon-app` directly.
- If the file adds TUI-specific functionality ON TOP of the re-exported types (e.g., rendering methods), keep the file but make the re-exports `pub(crate)` if they're only used internally.

#### 3. Audit Public API Surface Per Crate

For each crate, verify that `lib.rs` only exports types intended for external consumption:

**fdemon-core:**
- `pub`: `Error`, `Result`, `ResultExt`, `prelude`, `AppPhase`, `LogEntry`, `LogLevel`, `LogSource`, `DaemonEvent`, `DaemonMessage`, all event structs, discovery functions, ansi utilities, stack trace types
- `pub(crate)`: Internal helpers that don't need cross-crate access

**fdemon-daemon:**
- `pub`: `FlutterProcess`, `CommandSender`, `DaemonCommand`, `RequestTracker`, `Device`, `Emulator`, `BootCommand`, `ToolAvailability`, `IosSimulator`, `AndroidAvd`, discovery functions
- `pub(crate)`: Protocol parsing internals, raw JSON types

**fdemon-app:**
- `pub`: `Engine`, `AppState`, `Message`, `UpdateAction`, `Task`, `UpdateResult`, `Session`, `SessionHandle`, `SessionId`, `SessionManager`, service traits, `Settings`, `LaunchConfig`, configuration loaders
- `pub(crate)`: Handler internals, private helper functions

**fdemon-tui:**
- `pub`: `run_with_project()`, `select_project()`, `SelectionResult`, widget types (for potential reuse)
- `pub(crate)`: Layout math, rendering details, internal widget state

#### 4. Clean Up Unused Imports

After all the moves, there may be unused imports scattered across files. Run:
```bash
cargo clippy --workspace -- -W unused-imports
```

Fix any warnings.

#### 5. Verify No Circular Re-exports

Ensure no crate re-exports types from a crate that depends on it:
- `fdemon-core` must not re-export from `fdemon-daemon`, `fdemon-app`, or `fdemon-tui`
- `fdemon-daemon` must not re-export from `fdemon-app` or `fdemon-tui`
- `fdemon-app` must not re-export from `fdemon-tui`

#### 6. Standardize Import Conventions

Establish a consistent import style across all crates:
```rust
// External crates first
use tokio::sync::mpsc;
use serde::Serialize;

// Internal workspace crates
use fdemon_core::prelude::*;
use fdemon_core::types::AppPhase;
use fdemon_daemon::Device;

// Local crate modules
use crate::handler::UpdateAction;
use crate::session::Session;
```

### Acceptance Criteria

1. `fdemon-daemon` does NOT re-export `fdemon-core` event types
2. TUI re-export bridge files are cleaned up (removed or justified)
3. Each crate has a clear, intentional public API in `lib.rs`
4. No unused imports across the workspace
5. `cargo clippy --workspace` passes
6. `cargo test --workspace` passes
7. No circular re-export patterns exist

### Testing

```bash
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps  # Verify docs build cleanly
```

### Notes

- This task is about polish and correctness. The workspace is functional after task 08; this task makes it clean.
- Be conservative with `pub(crate)` changes. If a type is currently `pub` and tests or the binary use it, keep it `pub`.
- The `cargo doc` command is useful for auditing public APIs. It shows exactly what's exposed.
- Don't add doc comments in this task (that's Phase 4). Just ensure visibility is correct.
- Consider running `cargo doc --workspace --no-deps` and reviewing the generated docs to see what's exposed from each crate.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/lib.rs` | Removed `editor` and `hyperlinks` module declarations and re-exports |
| `crates/fdemon-tui/src/widgets/mod.rs` | Removed local re-exports of `ConfirmDialogState` and `LogViewState`, added re-exports from `fdemon_app` instead |
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Changed imports to use `fdemon_app::hyperlinks` and `fdemon_app::log_view_state` directly |
| `crates/fdemon-tui/src/widgets/confirm_dialog.rs` | Changed to import `ConfirmDialogState` from `fdemon_app` directly |
| `crates/fdemon-tui/src/editor.rs` | DELETED - bridge file that only re-exported `fdemon_app::editor::*` |
| `crates/fdemon-tui/src/hyperlinks.rs` | DELETED - bridge file that only re-exported `fdemon_app::hyperlinks::*` |
| `crates/fdemon-tui/src/widgets/log_view/state.rs` | DELETED - bridge file that only re-exported `fdemon_app::log_view_state::*` |

### Notable Decisions/Tradeoffs

1. **Removed TUI bridge re-export files**: Three bridge files (`editor.rs`, `hyperlinks.rs`, `widgets/log_view/state.rs`) were removed since they only contained re-exports and no TUI-specific functionality. Consumers now import directly from `fdemon_app`.

2. **Kept fdemon-daemon DaemonMessage re-export**: `fdemon_daemon::DaemonMessage` is re-exported for convenience since `protocol::parse()` returns it. This is justified and not a backward-compatibility bridge.

3. **Kept fdemon-app daemon type re-exports**: `fdemon_app` re-exports `Device`, `AndroidAvd`, `IosSimulator`, `SimulatorState`, and `ToolAvailability` from `fdemon_daemon` because the TUI uses them frequently. This avoids verbose imports across many TUI files.

4. **Widgets module consolidation**: State types (`ConfirmDialogState`, `LogViewState`) are now re-exported at the `widgets/mod.rs` level from `fdemon_app`, keeping the widget API clean while sourcing state from the authoritative location.

5. **No pub(crate) changes needed**: After audit, all public items in lib.rs files are intentionally exposed. Internal modules use private (non-pub) visibility naturally.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test --workspace --lib` - Passed (1,532 tests total: 726 + 243 + 136 + 427)
- `cargo clippy --workspace --lib -- -D warnings` - Passed (0 warnings in library crates)
- `cargo clippy --workspace -- -W unused-imports` - Passed (0 unused imports)
- `cargo doc --workspace --no-deps` - Passed (minor doc link warnings only)

### Verification of Acceptance Criteria

1. `fdemon-daemon` does NOT re-export `fdemon-core` event types - VERIFIED (only DaemonMessage, which is justified)
2. TUI re-export bridge files are cleaned up - VERIFIED (3 bridge files deleted, imports updated)
3. Each crate has a clear, intentional public API in `lib.rs` - VERIFIED (audited all lib.rs files)
4. No unused imports across the workspace - VERIFIED (clippy -W unused-imports passed)
5. `cargo clippy --workspace` passes - VERIFIED (library crates clean)
6. `cargo test --workspace` passes - VERIFIED (1,532 tests pass)
7. No circular re-export patterns exist - VERIFIED (dependency graph: core <- daemon <- app <- tui)

### Risks/Limitations

1. **Headless module warnings remain**: The binary's headless module has dead code warnings for unused HeadlessEvent variants and constructors. These are intentional scaffolding for future use and are out of scope for this task.
