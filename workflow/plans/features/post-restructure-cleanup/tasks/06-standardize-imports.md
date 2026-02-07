## Task: Standardize DaemonMessage imports and add test-helpers feature flag

**Objective**: Establish a canonical import convention for `DaemonMessage` across the codebase and replace `#[cfg(debug_assertions)]` with a proper `test-helpers` feature flag for cross-crate test utilities.

**Review Issues**: #8 (MINOR) - DaemonMessage dual import paths, #10 (MINOR) - debug_assertions for test utility visibility

**Depends on**: Task 01 (move-parse-to-daemon) -- import paths should be standardized after parse logic moves

### Scope

#### Import Standardization (Issue #8)
- Multiple files across workspace: Standardize import paths
- `crates/fdemon-daemon/src/lib.rs`: Evaluate re-export removal
- `crates/fdemon-daemon/src/protocol.rs`: Evaluate re-export cleanup

#### Test-Helpers Feature Flag (Issue #10)
- `crates/fdemon-daemon/Cargo.toml`: Add `[features] test-helpers = []`
- `crates/fdemon-daemon/src/lib.rs:13`: Change `#[cfg(any(test, debug_assertions))]` to `#[cfg(any(test, feature = "test-helpers"))]`
- `crates/fdemon-daemon/src/commands.rs:278`: Change `#[cfg(any(test, debug_assertions))]` to `#[cfg(any(test, feature = "test-helpers"))]`
- `crates/fdemon-app/Cargo.toml`: Add `fdemon-daemon = { workspace = true, features = ["test-helpers"] }` under `[dev-dependencies]`

### Details

#### Import Convention

After task 01 completes, the ownership model will be:
- **Type definition**: `fdemon_core::DaemonMessage` (and `fdemon_core::events::DaemonMessage`)
- **Parsing functions**: `fdemon_daemon::parse_daemon_message()`, `fdemon_daemon::to_log_entry()`
- **Re-export**: `fdemon_daemon::DaemonMessage` (convenience alias)

**Canonical convention:**

| Context | Import |
|---------|--------|
| Using the type (matching, storing, passing) | `use fdemon_core::DaemonMessage` |
| Parsing JSON-RPC | `use fdemon_daemon::parse_daemon_message` |
| In integration tests (binary crate) | `use fdemon_daemon::parse_daemon_message` (for parsing) + `use fdemon_core::DaemonMessage` (for type matching) |

**Current inconsistent imports to fix:**

| File | Current Import | Should Be |
|------|---------------|-----------|
| `crates/fdemon-app/src/actions.rs:15` | `use fdemon_core::{DaemonEvent, DaemonMessage}` | Keep (correct) |
| `crates/fdemon-app/src/process.rs:17` | `use fdemon_core::{DaemonEvent, DaemonMessage}` | Keep (correct) |
| `crates/fdemon-app/src/handler/session.rs:8` | `use fdemon_core::{AppPhase, DaemonMessage, ...}` | Keep (correct) |
| `tests/fixture_parsing_test.rs:5` | `use fdemon_daemon::DaemonMessage` | Change to `use fdemon_core::DaemonMessage` + `use fdemon_daemon::parse_daemon_message` |
| `tests/e2e/daemon_interaction.rs:7` | `use fdemon_daemon::{DaemonCommand, DaemonMessage}` | Change to `use fdemon_core::DaemonMessage` + `use fdemon_daemon::DaemonCommand` |
| `tests/e2e/hot_reload.rs:10` | `use fdemon_daemon::{DaemonCommand, DaemonMessage}` | Same as above |
| `tests/e2e.rs:95-98` | `fdemon_daemon::DaemonMessage` (inline) | Change to `fdemon_core::DaemonMessage` + `fdemon_daemon::parse_daemon_message` |

**Decision: Keep or remove fdemon-daemon re-export?**

Recommendation: **Keep** `pub use fdemon_core::DaemonMessage` in `fdemon-daemon/src/lib.rs` for external consumer convenience, but add a doc comment indicating the canonical source:

```rust
/// Re-exported from `fdemon_core` for convenience. Canonical import: `fdemon_core::DaemonMessage`.
pub use fdemon_core::DaemonMessage;
```

#### Test-Helpers Feature Flag (Issue #10)

**Current state:** `test_utils` module and `CommandSender::new_for_test()` use `#[cfg(any(test, debug_assertions))]`, making them available in all debug builds.

**Problem:** `new_for_test()` creates a `CommandSender` with a dummy channel that silently drops all commands. If accidentally used in non-test code during development, Flutter commands would silently fail.

**Fix:** Replace with a `test-helpers` Cargo feature flag.

**Step 1: Add feature to fdemon-daemon/Cargo.toml:**
```toml
[features]
test-helpers = []
```

**Step 2: Update cfg attributes in fdemon-daemon:**
```rust
// lib.rs:13
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_utils;

// commands.rs:278
#[cfg(any(test, feature = "test-helpers"))]
pub fn new_for_test() -> Self { ... }
```

**Step 3: Activate feature in fdemon-app's dev-dependencies:**
```toml
# crates/fdemon-app/Cargo.toml
[dev-dependencies]
fdemon-daemon = { workspace = true, features = ["test-helpers"] }
```

**Step 4: Update any other crates that use test_utils across the workspace boundary.** Check `fdemon-tui` as well (it has fdemon-daemon as a dev-dependency).

### Acceptance Criteria

1. All `DaemonMessage` imports in library crates use `fdemon_core::DaemonMessage`
2. All parsing function calls use `fdemon_daemon::parse_daemon_message`
3. The re-export in `fdemon-daemon/lib.rs` has a doc comment indicating canonical source
4. `#[cfg(any(test, debug_assertions))]` replaced with `#[cfg(any(test, feature = "test-helpers"))]` in fdemon-daemon
5. `fdemon-app` and any other cross-crate test consumers activate `test-helpers` feature in `[dev-dependencies]`
6. `cargo test --workspace --lib` passes
7. `cargo clippy --workspace --lib -- -D warnings` passes
8. `cargo build` (release mode) does NOT include test_utils or new_for_test()

### Testing

No new tests needed. Existing tests validate that the imports resolve correctly. The feature flag change requires verifying that:
- `cargo test -p fdemon-app` still compiles (test-helpers feature is activated)
- `cargo build --release` does NOT include test utilities

### Notes

- This task should be done AFTER task 01, since the parse logic move changes the import landscape
- The `protocol.rs` re-export `pub use fdemon_core::{DaemonMessage, LogEntryInfo}` will change after task 01 (LogEntryInfo moves to daemon), so coordinate the cleanup
- A clippy lint (`disallowed-imports` or similar) could enforce the convention going forward, but that is out of scope for this task

---

## Completion Summary

**Status:** Not Started
