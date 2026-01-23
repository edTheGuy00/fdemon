## Task: Replace Unwrap Calls with Safe Error Handling

**Objective**: Replace `.unwrap()` calls in handler logging with safe pattern matching to prevent potential panics in production.

**Priority**: Major

**Depends on**: None

### Scope

- `src/app/handler/new_session/launch_context.rs`: Lines 170 and 276

### Problem Analysis

The code calls `.unwrap()` on `selected_config()` immediately after `create_and_select_default_config()`:

**Flavor Handler (lines 164-173):**
```rust
if state
    .new_session_dialog_state
    .launch_context
    .selected_config_index
    .is_none()
{
    state
        .new_session_dialog_state
        .launch_context
        .create_and_select_default_config();
    tracing::info!(
        "Auto-created config '{}' for flavor selection",
        state
            .new_session_dialog_state
            .launch_context
            .selected_config()
            .unwrap()  // ← POTENTIAL PANIC
            .config
            .name
    );
}
```

**Dart-Defines Handler (lines 270-279):**
```rust
tracing::info!(
    "Auto-created config '{}' for dart-defines",
    state
        .new_session_dialog_state
        .launch_context
        .selected_config()
        .unwrap()  // ← POTENTIAL PANIC
        .config
        .name
);
```

### Why This Violates Standards

From `CODE_STANDARDS.md`:
> **❌ Panicking in Library Code**
> ```rust
> // ❌ BAD: Panicking in library code
> let value = some_option.unwrap();
> ```

Even though the config was just created, if there's ever a bug in `create_and_select_default_config()` that causes it to fail silently, this would cause a panic in production.

### Solution

Replace unwrap with safe pattern matching using `if let`:

### Implementation

**Replace in `handle_flavor_selected()` (around line 170):**

```rust
// BEFORE:
tracing::info!(
    "Auto-created config '{}' for flavor selection",
    state
        .new_session_dialog_state
        .launch_context
        .selected_config()
        .unwrap()
        .config
        .name
);

// AFTER:
if let Some(config) = state.new_session_dialog_state.launch_context.selected_config() {
    tracing::info!("Auto-created config '{}' for flavor selection", config.config.name);
}
```

**Replace in `handle_dart_defines_updated()` (around line 276):**

```rust
// BEFORE:
tracing::info!(
    "Auto-created config '{}' for dart-defines",
    state
        .new_session_dialog_state
        .launch_context
        .selected_config()
        .unwrap()
        .config
        .name
);

// AFTER:
if let Some(config) = state.new_session_dialog_state.launch_context.selected_config() {
    tracing::info!("Auto-created config '{}' for dart-defines", config.config.name);
}
```

### Acceptance Criteria

1. No `.unwrap()` calls on `selected_config()` in the launch_context handler
2. Logging still works when config exists
3. No panic if `selected_config()` returns `None` (graceful no-op)
4. `cargo clippy -- -D warnings` passes with no unwrap warnings
5. All existing tests pass

### Testing

```bash
cargo clippy -- -D warnings
cargo test launch_context
```

### Notes

- This is a defensive coding practice - the unwrap "should" be safe, but safe patterns prevent future bugs
- Consider using `tracing::debug!` instead of `info!` for auto-create messages (less noisy)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/new_session/launch_context.rs` | Replaced `.unwrap()` calls on lines 170 and 276 with safe `if let Some(config)` pattern matching for logging |

### Notable Decisions/Tradeoffs

1. **Safe Pattern Matching**: Replaced both `.unwrap()` calls with `if let Some(config) = state.new_session_dialog_state.launch_context.selected_config()` pattern, which gracefully handles the case where `selected_config()` returns `None` by simply not logging (no-op behavior).
2. **Logging Preserved**: The logging still occurs when a config exists, maintaining the same informational output while preventing potential panics.
3. **Pre-existing Test Failures**: Tests `test_flavor_selected_no_config_creates_default`, `test_flavor_selected_existing_config_no_create`, `test_dart_defines_updated_no_config_creates_default`, and `test_dart_defines_updated_existing_config_no_create` are failing due to incomplete feature implementation (missing logic to update config struct with flavor/dart-defines), NOT due to the unwrap replacements. This appears to be from an incomplete refactoring visible in git history.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed (no unwrap warnings)
- `cargo test launch_context` - 29 passed, 4 failed (pre-existing failures unrelated to unwrap changes)

### Risks/Limitations

1. **Test Failures**: The 4 failing tests indicate incomplete functionality in the handlers (missing config struct updates), but this is outside the scope of this unwrap safety task. The unwrap replacements themselves are correct and prevent potential panics.
2. **No Behavior Change**: The if-let pattern maintains existing behavior - logging occurs when config exists, silently skips when None (which should never happen after `create_and_select_default_config()`, but defensive coding prevents panics).
