## Task: Extract Session Module from actions.rs

**Objective**: Move `spawn_session` and `execute_task` into `actions/session.rs`, beginning the conversion of `actions.rs` from a flat file to a directory module.

**Depends on**: 01-fix-heartbeat-counter-reset

### Scope

- `crates/fdemon-app/src/actions.rs` → rename to `crates/fdemon-app/src/actions/mod.rs`
- `crates/fdemon-app/src/actions/session.rs` — **NEW**

### Details

#### Step 1: Convert `actions.rs` to `actions/mod.rs`

1. Create directory `crates/fdemon-app/src/actions/`
2. Move `actions.rs` → `actions/mod.rs`
3. Verify `cargo check -p fdemon-app` passes (Rust's module system treats both identically)

#### Step 2: Extract session functions

Move these functions from `mod.rs` to `session.rs`:

| Function | Current Lines | Purpose |
|----------|--------------|---------|
| `spawn_session` | 348-566 | Flutter process lifecycle, watchdog, stdio forwarding |
| `execute_task` | 567-663 | Task execution (reload, restart, stop, etc.) |

**session.rs** will need these imports (subset of mod.rs imports):
- `std::path::Path`, `std::sync::Arc`, `std::time::Duration`
- `tokio::sync::{mpsc, watch}`
- `tracing::{debug, error, info, warn}`
- `crate::config::LaunchConfig`, `crate::handler::Task`, `crate::message::Message`, `crate::session::SessionId`
- `fdemon_daemon::{CommandSender, Device, FlutterProcess, RequestTracker}`
- `super::SessionTaskMap`, `super::PROCESS_WATCHDOG_INTERVAL`

#### Step 3: Update `mod.rs`

1. Add `mod session;` declaration
2. Keep `pub use session::execute_task;` if needed externally (check callers)
3. Update `handle_action` to call `session::spawn_session(...)` and `session::execute_task(...)`
4. Remove the moved functions and any now-unused imports from `mod.rs`

### Acceptance Criteria

1. `actions.rs` no longer exists as a flat file — replaced by `actions/mod.rs`
2. `spawn_session` and `execute_task` live in `actions/session.rs`
3. `session.rs` has a `//!` module doc header
4. `handle_action` in `mod.rs` calls into `session::` for the moved functions
5. `cargo check --workspace` passes
6. `cargo test --workspace` passes
7. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new tests needed — this is a pure move refactoring. All existing tests must continue to pass.

### Notes

- `execute_task` is `pub` — check if any external crates import it directly. If so, re-export from `mod.rs`.
- `PROCESS_WATCHDOG_INTERVAL` constant is used only in `spawn_session` — move it to `session.rs` or keep in `mod.rs` and import via `super::`.
- The `spawn` submodule (device discovery) already exists as a separate module referenced from `handle_action` — do not touch it.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions.rs` | Deleted (replaced by `actions/mod.rs`) |
| `crates/fdemon-app/src/actions/mod.rs` | Created: original `actions.rs` content minus `spawn_session`, `execute_task`, `PROCESS_WATCHDOG_INTERVAL`; added `pub mod session;`, `pub use session::execute_task;`; `handle_action` now calls `session::spawn_session` and `execute_task` via the re-export; removed now-unused imports (`LaunchConfig`, `DaemonEvent`, `DaemonMessage`, `DaemonCommand`, `Device`, `FlutterProcess`, `RequestTracker`) |
| `crates/fdemon-app/src/actions/session.rs` | Created: `//!` module doc header, `PROCESS_WATCHDOG_INTERVAL` constant (`pub(super)`), `spawn_session` (`pub(super)`), `execute_task` (`pub async`), plus a unit test for the watchdog interval constant |

### Notable Decisions/Tradeoffs

1. **`PROCESS_WATCHDOG_INTERVAL` moved to `session.rs`**: The constant is used exclusively inside `spawn_session`, so it belongs in the same file. Declared `pub(super)` so `mod.rs` can access it via `super::` if ever needed.
2. **`execute_task` visibility**: Kept `pub async fn` so that `pub use session::execute_task;` in `mod.rs` correctly re-exports it to any callers at `actions::execute_task`. Verified no external crates call it directly (only used internally in `handle_action`), but the existing `pub` visibility is preserved to avoid any future breakage.
3. **`spawn_session` visibility**: `pub(super)` — callable only from `mod.rs`, which is the only call site.
4. **Test split**: The `test_watchdog_interval_is_reasonable` test moved to `session.rs` (where the constant lives). The heartbeat constants tests remain in `mod.rs` (where those constants remain).

### Testing Performed

- `cargo check -p fdemon-app` — Passed (0 warnings)
- `cargo check --workspace` — Passed
- `cargo fmt --all` — Passed
- `cargo test --workspace` — Passed (2,803 tests: 1161 + 360 + 383 + 773 + 10 + 16 + 80 + 7 + 1 + 5 + 7 passed, 0 failed)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **Pure refactoring**: No behavioral changes. All runtime logic is identical to the original `actions.rs`.
