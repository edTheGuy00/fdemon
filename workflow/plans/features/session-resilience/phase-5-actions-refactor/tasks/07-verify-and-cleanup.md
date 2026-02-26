## Task: Verify Module Structure and Final Cleanup

**Objective**: Verify all modules are within the 500-line limit, ensure clean re-exports, add module doc headers, and run the full quality gate.

**Depends on**: 06-extract-network-module

### Scope

- All files in `crates/fdemon-app/src/actions/`

### Details

#### Verification Checklist

1. **Line counts** — verify each file is ≤500 lines:
   - `actions/mod.rs` — target ~350 lines
   - `actions/session.rs` — target ~320 lines
   - `actions/performance.rs` — target ~220 lines
   - `actions/vm_service.rs` — target ~250 lines
   - `actions/inspector.rs` — target ~440 lines
   - `actions/network.rs` — target ~340 lines

2. **Module doc headers** — each file should have a `//!` header:
   - `mod.rs`: `//! Action handlers: UpdateAction dispatch and background task spawning`
   - `session.rs`: `//! Session lifecycle: Flutter process spawning, task execution, and process watchdog`
   - `vm_service.rs`: `//! VM Service connection: WebSocket client lifecycle, event forwarding, and heartbeat monitoring`
   - `performance.rs`: `//! Performance monitoring: periodic memory usage and allocation profile polling`
   - `inspector.rs`: `//! DevTools inspector actions: widget tree, overlay toggle, layout explorer, and group disposal`
   - `network.rs`: `//! Network monitoring: HTTP profile polling, request detail fetching, and browser launch`

3. **Re-exports** — ensure `mod.rs` re-exports everything needed by external callers:
   - `pub use session::execute_task;` (if used outside the module)
   - `pub fn handle_action(...)` stays in `mod.rs`
   - `pub type SessionTaskMap` stays in `mod.rs`

4. **Dead imports** — remove any imports in `mod.rs` that are no longer used after extraction

5. **Test placement** — verify tests moved to their correct submodules:
   - `test_heartbeat_constants_are_reasonable` → `vm_service.rs`
   - `test_heartbeat_counter_reset_on_reconnection` → `vm_service.rs`
   - `test_watchdog_interval_is_reasonable` → `session.rs`

#### Quality Gate

Run the full verification:

```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

### Acceptance Criteria

1. No file in `actions/` exceeds 500 lines
2. All 6 files have `//!` module doc headers
3. No unused imports (clippy would catch this)
4. All existing public API accessible from the same paths as before
5. Full quality gate passes
6. `cargo test --workspace` — all tests pass, same count as before refactoring

### Testing

Run the full test suite and compare test counts before/after. No tests should be lost or broken.

### Notes

- This is the final cleanup task — it catches anything missed during the individual extraction tasks
- If any file exceeds 500 lines, identify further split opportunities and note them (but do not split further in this phase unless the overage is significant)
- Update `docs/ARCHITECTURE.md` if it references `actions.rs` as a flat file — it should now reference the `actions/` directory module
