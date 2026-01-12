## Task: Code Quality Improvements

**Objective**: Address minor code quality issues identified in the review: magic numbers, swallowed errors, and platform string matching.

**Depends on**: 06-fix-regex-compilation, 07-fix-avd-running-check

**Source**: Code Quality Inspector (Review Minor Issues #1-5)

### Scope

- `src/daemon/simulators.rs`: Extract iOS boot timeout constant
- `src/daemon/avds.rs`: Extract Android init delay constant
- `src/daemon/tool_availability.rs`: Add debug logging for swallowed errors
- `src/tui/spawn.rs`: Use Platform enum instead of string matching (optional)

### Details

#### 1. Magic Number: iOS Boot Timeout

**File:** `src/daemon/simulators.rs:165`

**Current:**
```rust
tokio::time::timeout(Duration::from_secs(60), async { ... })
```

**Required:**
```rust
const SIMULATOR_BOOT_TIMEOUT: Duration = Duration::from_secs(60);

tokio::time::timeout(SIMULATOR_BOOT_TIMEOUT, async { ... })
```

#### 2. Magic Number: Android Init Delay

**File:** `src/daemon/avds.rs:125`

**Current:**
```rust
tokio::time::sleep(Duration::from_secs(2)).await;
```

**Required:**
```rust
const AVD_INIT_DELAY: Duration = Duration::from_secs(2);

tokio::time::sleep(AVD_INIT_DELAY).await;
```

#### 3. Swallowed Errors in Tool Checks

**File:** `src/daemon/tool_availability.rs:51`

**Current:**
```rust
.unwrap_or(false)
```

**Required:**
```rust
.inspect_err(|e| tracing::debug!("Tool check failed: {}", e))
.unwrap_or(false)
```

Apply similar pattern to all `.unwrap_or(false)` calls in tool availability checks.

#### 4. Platform String Matching (Optional)

**File:** `src/tui/spawn.rs:306`

**Current:**
```rust
match platform.as_str() {
    "iOS" => { ... }
    "Android" => { ... }
}
```

**Suggested:**
```rust
match platform {
    Platform::Ios => { ... }
    Platform::Android => { ... }
    _ => { ... }
}
```

Note: This requires changing the function signature to accept `Platform` enum.

#### 5. Large Enum in mod.rs (Deferred)

Moving `BootableDevice` to its own module is deferred to Task 09, which handles the type unification/renaming.

### Acceptance Criteria

1. iOS boot timeout uses named constant `SIMULATOR_BOOT_TIMEOUT`
2. Android init delay uses named constant `AVD_INIT_DELAY`
3. Tool availability errors are logged at debug level before fallback
4. (Optional) Platform matching uses enum instead of strings
5. `cargo test` passes
6. `cargo clippy -- -D warnings` passes

### Testing

Existing tests should pass. No new tests required for these refactoring changes.

### Notes

- Constants should be defined near their usage or at module top
- Use `tracing::debug!` for error logging (not `log::debug!`)
- Platform enum change in spawn.rs may require updating callers
- These are non-breaking refactoring changes

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/daemon/simulators.rs` | Added `SIMULATOR_BOOT_TIMEOUT` constant (60 seconds) at module top, updated `boot_simulator()` to use constant instead of inline Duration |
| `src/daemon/avds.rs` | Added `AVD_INIT_DELAY` constant (2 seconds) at module top, updated `boot_avd()` to use constant instead of inline Duration |
| `src/daemon/tool_availability.rs` | Added `.inspect_err()` calls with `tracing::debug!` logging to both `check_xcrun_simctl()` and `check_android_emulator()` for better debugging of swallowed errors |

### Notable Decisions/Tradeoffs

1. **Skipped Platform enum refactoring (Optional Task 4)**: The `spawn_device_boot()` function uses `platform: String` parameter which is passed through Message enum and UpdateAction enum. Converting to Platform enum would require changing multiple files (message.rs, handler/mod.rs, handler/update.rs, actions.rs, spawn.rs) and all call sites. Since this was marked optional and would be a larger refactoring affecting multiple modules, it was deferred. The string matching pattern is functional and the scope of changes outweighs the benefit for this task.

2. **Constant placement**: Constants were placed at module top after imports, following Rust conventions and making them easily discoverable.

3. **Debug logging pattern**: Used `.inspect_err()` pattern as recommended in task specification. This allows errors to be logged for debugging while maintaining the fallback behavior (`unwrap_or(false)`).

### Testing Performed

- `cargo fmt` - Passed (auto-formatted)
- `cargo test --lib` - Passed (1455 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Debug logging only visible with debug level**: The added error logging uses `tracing::debug!` which means errors are only visible when debug logging is enabled. This is intentional as these are fallback cases where the tools are simply not available, not critical errors. Users running with info-level logging won't see noise from unavailable tools.
