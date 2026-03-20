## Task: Replace Bare `unwrap()` on `selected_id()` with `let-else`

**Objective**: Replace two bare `.unwrap()` calls on `selected_id()` in production handler code with `let-else` early returns, eliminating potential panic paths and aligning with codebase standards.

**Depends on**: None

**Estimated Time**: 0.5 hours

**PR Review Comments**: #3 (session_lifecycle.rs:167), #6 (devtools/mod.rs:135)

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/session_lifecycle.rs`: Replace `selected_id().unwrap()` at ~line 167
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Replace `selected_id().unwrap()` at ~line 135

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/devtools/network.rs`: Reference pattern at line 290 — uses `let Some(id) = ... else { return }`
- `crates/fdemon-app/src/handler/update.rs`: Reference pattern at lines 2330, 2345, 2355, 2397 — uses `if let Some(id) = ...`

### Details

#### Current State

Both sites follow the same implicit-guard pattern:

```rust
// 1. Check if selected() is Some via a conditional
let needs_start = if let Some(handle) = state.session_manager.selected() {
    handle.perf_shutdown_tx.is_none() && handle.session.vm_connected
} else {
    false
};

// 2. Only reach this line when needs_start == true (i.e., selected() was Some)
if needs_start {
    let session_id = state.session_manager.selected_id().unwrap(); // safe but implicit
```

The unwraps are logically safe in the current code because `selected()` and `selected_id()` index the same `session_order[selected_index]`. But:

1. **CODE_STANDARDS.md** explicitly flags `unwrap()` without justification as a red flag (line 99) and anti-pattern (lines 44-52)
2. **Every other production caller** of `selected_id()` uses `if let Some` or `let-else`
3. The safety proof requires reading the surrounding conditional — fragile under refactoring

#### Fix

Replace both sites with `let-else`, matching the pattern at `network.rs:290`:

**Site A — `session_lifecycle.rs:167`:**
```rust
// Before:
let session_id = state.session_manager.selected_id().unwrap();

// After:
let Some(session_id) = state.session_manager.selected_id() else {
    return UpdateResult::none();
};
```

**Site B — `devtools/mod.rs:135`:**
```rust
// Before:
let session_id = state.session_manager.selected_id().unwrap();

// After:
let Some(session_id) = state.session_manager.selected_id() else {
    return UpdateResult::none();
};
```

### Acceptance Criteria

1. Zero bare `unwrap()` calls on `selected_id()` in non-test handler code
2. Both sites use `let-else` with `return UpdateResult::none()` fallback
3. Behavior is unchanged — the early return handles the `None` case that was previously unreachable
4. `cargo test --workspace` passes
5. `cargo clippy --workspace -- -D warnings` passes

### Testing

No new tests needed — this is a defensive refactor. The `None` branch returns `UpdateResult::none()`, which is a no-op. Existing tests cover the `Some` path. The `None` path is a safety net for edge cases (e.g., last session removed while entering DevTools).

### Notes

- The `let-else` pattern is idiomatic Rust (stabilized in 1.65) and already used throughout the handler codebase.
- This does NOT change any behavior for the normal case — just makes the implicit guard explicit and eliminates the panic path.
