## Task: Fix AVD Running Check Signature

**Objective**: Make the `is_avd_running()` function signature match its actual behavior, or implement proper AVD-specific checking.

**Depends on**: 05-discovery-integration

**Source**: Code Quality Inspector (Review Issue #1)

### Scope

- `src/daemon/avds.rs`: Fix function signature to match behavior

### Details

Currently `is_avd_running(_avd_name: &str)` accepts an AVD name parameter but doesn't use it. The function only checks if *any* emulator is running via `adb devices`, not whether the specific AVD is running.

**Current Code:**
```rust
pub async fn is_avd_running(_avd_name: &str) -> bool {
    // Parameter unused - only checks for any emulator
    Command::new("adb")
        .args(["devices"])
        // ...
}
```

**Option A (Recommended - Simpler):** Remove the unused parameter and rename the function:
```rust
pub async fn is_any_emulator_running() -> bool {
    Command::new("adb")
        .args(["devices"])
        // ...
}
```

**Option B (More Accurate):** Implement AVD-specific checking:
```rust
pub async fn is_avd_running(avd_name: &str) -> bool {
    // Query emulator console to get AVD name
    // This requires connecting to each emulator's console port
    // More complex, consider deferring to future enhancement
}
```

### Acceptance Criteria

1. Function signature accurately reflects its behavior
2. If Option A: Function renamed to `is_any_emulator_running()` and callers updated
3. If Option B: Function correctly checks if specific AVD is running
4. `cargo test avds` passes
5. `cargo clippy -- -D warnings` shows no warnings about unused parameter

### Testing

Update or add tests:
```rust
#[tokio::test]
async fn test_is_any_emulator_running() {
    // Test returns false when no emulators running
    // (actual emulator testing requires Android SDK)
}
```

### Notes

- Check all callers of this function and update their call sites
- Option A is recommended for this task; Option B can be a future enhancement
- The function is used in `boot_avd()` to check if we need to wait for boot

---

## Completion Summary

**Status:** Not started
