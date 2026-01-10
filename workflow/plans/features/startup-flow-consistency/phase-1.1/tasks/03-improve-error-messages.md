## Task: Improve Error Messages and Logging

**Objective**: Replace `unwrap()` with descriptive `expect()` calls and add warning logs for silent fallback behaviors to improve debugging.

**Depends on**: 01-fix-session-error-handler

**Estimated Time**: 0.5-1 hours

**Severity**: Minor (polish)

### Problem

Code review identified three minor issues in `src/tui/spawn.rs`:

1. **Line 205**: `unwrap()` has safety comment but no descriptive panic message
2. **Lines 174-180**: Silent fallthrough when saved selection validation fails
3. **Lines 189-192**: Silent device fallback when configured device not found

These make debugging harder when things go wrong.

### Scope

- `src/tui/spawn.rs`: Improve `find_auto_launch_target()` function

### Changes

#### Change 1: Use `expect()` instead of `unwrap()`

**Location:** Line 205

**Current:**
```rust
device: devices.first().unwrap().clone(), // Safe: we checked devices.is_empty() above
```

**Fix:**
```rust
device: devices.first().expect("devices non-empty; checked at spawn_auto_launch line 137").clone(),
```

#### Change 2: Log warning when saved selection lookup fails

**Location:** Lines 174-180

Add logging when the saved config/device indices don't match current state:

```rust
// Priority 1: Check settings.local.toml for saved selection
if let Some(selection) = load_last_selection(project_path) {
    if let Some(validated) = validate_last_selection(&selection, configs, devices) {
        let config = validated.config_idx.and_then(|i| configs.configs.get(i));
        if let Some(device) = validated.device_idx.and_then(|i| devices.get(i)) {
            return AutoLaunchSuccess {
                device: device.clone(),
                config: config.map(|c| c.config.clone()),
            };
        } else {
            tracing::warn!(
                "Saved device index {} not found in {} available devices, falling back to Priority 2",
                validated.device_idx.unwrap_or(0),
                devices.len()
            );
        }
    } else {
        tracing::debug!("Saved selection validation failed, falling back to Priority 2");
    }
}
```

#### Change 3: Log warning when device fallback occurs

**Location:** Lines 189-192

Add logging when configured device is not found:

```rust
let device = if sourced.config.device == "auto" {
    devices.first()
} else {
    let found = devices::find_device(devices, &sourced.config.device);
    if found.is_none() {
        tracing::warn!(
            "Configured device '{}' not found, falling back to first available device",
            sourced.config.device
        );
    }
    found.or_else(|| devices.first())
};
```

### Implementation Steps

1. Open `src/tui/spawn.rs`
2. Apply Change 1: Replace `unwrap()` with `expect()` at line 205
3. Apply Change 2: Add warning log in Priority 1 section
4. Apply Change 3: Add warning log in Priority 2 device lookup
5. Run `cargo check` to verify changes compile
6. Run `cargo clippy` to ensure no new warnings

### Acceptance Criteria

1. No bare `unwrap()` calls in `find_auto_launch_target()`
2. Warning logged when saved selection device index is out of bounds
3. Warning logged when configured device name not found in available devices
4. All logs use appropriate level (`warn` for unexpected fallbacks, `debug` for normal fallthrough)
5. `cargo clippy -- -D warnings` passes

### Testing

These changes are defensive improvements - no behavioral change to test. Verification:

```bash
# Compile check
cargo check

# Lint check
cargo clippy -- -D warnings

# Manual: run with RUST_LOG=warn to see warning output
RUST_LOG=fdemon=warn cargo run
```

### Notes

- Use `tracing::warn!` for user-actionable warnings (device not found)
- Use `tracing::debug!` for expected fallthrough (validation failed)
- The exact line numbers may shift after Task 01; adjust accordingly
- Keep messages concise but informative for troubleshooting

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/spawn.rs` | Replaced `unwrap()` with descriptive `expect()` at line 220-222, added warning log when saved device index is out of bounds (lines 181-185), added debug log when saved selection validation fails (line 188), added warning log when configured device not found (lines 202-205) |

### Notable Decisions/Tradeoffs

1. **Log Levels**: Used `tracing::warn!` for user-actionable issues (device not found) and `tracing::debug!` for expected fallthrough (validation failed), as specified in the task requirements.
2. **Message Clarity**: Ensured warning messages are concise but informative, including context like device counts and configured device names to aid troubleshooting.

### Testing Performed

- `cargo fmt` - Passed (auto-formatted long `expect()` to multiple lines)
- `cargo check` - Passed
- `cargo test --lib` - Passed (1336 tests)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

None. These are defensive improvements that don't change behavior, only improve debuggability when fallback scenarios occur.
