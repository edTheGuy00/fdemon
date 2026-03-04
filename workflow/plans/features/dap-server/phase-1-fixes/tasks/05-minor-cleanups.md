## Task: Minor cleanups — doc comments, test notes, import paths, log levels

**Objective**: Address remaining minor review findings: clarify `PauseReason::Step` doc comment, fix direct submodule import, add test module note, and upgrade debug action stubs to `warn!`.

**Depends on**: None

**Review Issues**: #9 (PauseReason::Step doc), #11 (stub log level), #12 (direct submodule import)

### Scope

- `crates/fdemon-app/src/session/debug_state.rs`:
  - Update `PauseReason::Step` doc comment to clarify it maps to the VM's `PauseStep` event and is a forward-looking placeholder (currently unused)
  - Fix import path: change `use fdemon_daemon::vm_service::debugger_types::{ExceptionPauseMode, IsolateRef};` to use the public re-export path from `fdemon_daemon::vm_service` if one exists, or add a comment explaining why the direct path is used

- `crates/fdemon-daemon/src/vm_service/debugger.rs`:
  - Add a `// NOTE:` comment at the top of the `#[cfg(test)] mod tests` block explaining why tests are synchronous parameter-construction tests (no async RPC coverage due to mock transport limitations)

- `crates/fdemon-app/src/actions/mod.rs`:
  - Change the five debug `UpdateAction` stubs from `tracing::debug!` to `tracing::warn!` since reaching them is described as unexpected and `debug!` is off by default in release builds

### Details

**Issue #9 — PauseReason::Step doc comment:**

Current:
```rust
/// Completed a single-step operation.
Step,
```

Fixed:
```rust
/// Paused after a single-step operation (maps to VM `PauseStep` event).
/// Currently unused — the VM sends `PauseStep` but it is not yet mapped
/// in the event pipeline. Reserved for Phase 2 stepping support.
Step,
```

**Issue #12 — Direct submodule import:**

Current:
```rust
use fdemon_daemon::vm_service::debugger_types::{ExceptionPauseMode, IsolateRef};
```

Check whether `fdemon_daemon::vm_service` re-exports these types. If it does, use the re-export. If not, either add the re-export in `vm_service/mod.rs` or add a comment:
```rust
// Imported from internal submodule — no public re-export exists yet.
use fdemon_daemon::vm_service::debugger_types::{ExceptionPauseMode, IsolateRef};
```

**Issue #11 — Debug action stub log level:**

Current (all 5 stubs):
```rust
tracing::debug!(
    "PauseIsolate action for session {} — DAP executor not yet wired (Phase 2)",
    session_id
);
```

Fixed:
```rust
tracing::warn!(
    "PauseIsolate action for session {} — DAP executor not yet wired (Phase 2)",
    session_id
);
```

Also update the comment block (around line 341) from "log at debug" to "log at warn":
```rust
// Reaching these arms in the current build is unexpected; log at warn.
```

### Acceptance Criteria

1. `PauseReason::Step` has a doc comment explaining it is a forward-looking placeholder
2. `debug_state.rs` import path uses re-export or has explanatory comment
3. `debugger.rs` test module has a `// NOTE:` explaining the sync-only test approach
4. All 5 debug `UpdateAction` stubs log at `warn!` level
5. `cargo check --workspace` passes
6. `cargo test --workspace` passes
7. `cargo clippy --workspace -- -D warnings` passes

### Testing

- No new tests required — these are documentation and log-level changes
- Verify `cargo test --workspace` has no regressions

### Notes

- These are all low-risk, low-effort changes that can be done independently
- The `PauseReason::Step` variant should NOT be removed — it will be used in Phase 2 when stepping support is added

---

## Completion Summary

**Status:** Not Started
