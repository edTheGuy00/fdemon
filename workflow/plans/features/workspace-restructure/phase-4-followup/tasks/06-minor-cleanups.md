## Task: Minor Cleanups (4 Items)

**Objective**: Address the four minor issues from the phase-4 review: debug logging, plugin doc ordering, test-only function, and handler::update re-export.

**Depends on**: None

**Severity**: MINOR

**Source**: ACTION_ITEMS.md Minor #1-4

### Scope

- `crates/fdemon-tui/src/event.rs:42-62`: Downgrade/remove debug logging
- `crates/fdemon-app/src/plugin.rs:22-29`: Fix callback ordering documentation
- `crates/fdemon-app/src/lib.rs:86`: Add `update` to re-exports
- `tests/e2e/hot_reload.rs:7`, `tests/e2e/session_management.rs:7`: Update imports

### Details

#### 1. Downgrade Debug Logging in event.rs

**File:** `crates/fdemon-tui/src/event.rs:54-62`

```rust
if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
    tracing::warn!(
        "ENTER/SPACE KEY DETECTED: code={:?}, kind={:?}, modifiers={:?}",
        key.code, key.kind, key.modifiers
    );
}
```

This is leftover debug instrumentation (comment on line 42 says "Temporary debug logging to investigate PTY key event handling"). It fires on every Enter/Space press at `warn!` level, which is visible in production logs.

**Action:** Remove the entire block (lines 42-62, including the comment "Temporary debug logging" and the "Special logging for Enter and Space keys" block). Also remove the `debug!` logging at lines 43-52 if similarly marked as temporary.

#### 2. Fix Plugin Callback Ordering Documentation

**File:** `crates/fdemon-app/src/plugin.rs:22-29`

The trait documentation says:
```
1. on_start()     -- Engine begins event loop
2. on_message()   -- After each message is processed
3. on_event()     -- For each emitted EngineEvent
4. on_shutdown()  -- Engine shuts down
```

But the actual execution order in `engine.rs:231-255` is:
1. `emit_events()` calls `plugin.on_event()` (line 251)
2. `notify_plugins_message()` calls `plugin.on_message()` (line 254)

So `on_event` fires **before** `on_message`, not after.

**Action:** Fix the documentation to reflect the actual order:
```
1. on_start()     -- Engine begins event loop
2. on_event()     -- For each emitted EngineEvent (after state change)
3. on_message()   -- After each message is processed (with full post-state)
4. on_shutdown()  -- Engine shuts down
```

#### 3. Re-export `handler::update` from lib.rs

**File:** `crates/fdemon-app/src/lib.rs:86`

Currently:
```rust
pub use handler::{Task, UpdateAction, UpdateResult};
```

The `update` function is not re-exported, but E2E tests use `fdemon_app::handler::update`. Per ARCHITECTURE.md line 764: "External consumers should only use items exported from `lib.rs`."

**Action:** Add `update` to the re-export:
```rust
pub use handler::{update, Task, UpdateAction, UpdateResult};
```

Then update E2E test imports:
- `tests/e2e/hot_reload.rs:7`: `use fdemon_app::update;`
- `tests/e2e/session_management.rs:7`: `use fdemon_app::update;`

#### 4. Verify E2E Test Imports

After adding the re-export, update any tests that import `fdemon_app::handler::update` to use `fdemon_app::update` instead.

### Acceptance Criteria

1. No `warn!("ENTER/SPACE...")` in event.rs
2. Plugin trait docs show correct callback order (`on_event` before `on_message`)
3. `fdemon_app::update` is accessible as a crate-root import
4. E2E tests use `fdemon_app::update` (not `fdemon_app::handler::update`)
5. `cargo check --workspace` passes
6. `cargo test --workspace --lib` passes

### Testing

```bash
# Verify warn! removed
rg 'ENTER/SPACE' crates/

# Verify re-export exists
rg 'pub use handler.*update' crates/fdemon-app/src/lib.rs

# Build and test
cargo check --workspace
cargo test --workspace --lib
```

### Notes

- The debug logging removal is the simplest change -- just delete the block
- The plugin doc fix should match the actual code execution order, not hypothetical ordering
- The `handler::update` path will continue to work (it's `pub mod handler` with `pub use update::update` inside) -- we're just adding a shorter canonical path

---

## Completion Summary

**Status:** Not started
