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

**Status:** Not started
