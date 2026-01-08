## Task: Extract test_device() Helper to test_utils.rs

**Objective**: Create shared `test_device()` helper functions in `test_utils.rs` to replace duplicated implementations across widget test files.

**Depends on**: Wave 1 complete

### Scope

- `src/tui/test_utils.rs`: Add new helper functions

### Details

The `test_device()` function is duplicated across 5+ files with varying signatures:

| File | Current Signature |
|------|------------------|
| `device_selector.rs` | `fn test_device(id, name, emulator)` |
| `status_bar.rs` | `fn test_device(id, name)` |
| `header.rs` | `fn test_device(id, name, platform)` |
| `tabs.rs` | `fn test_device(id, name)` |
| `startup_dialog/mod.rs` | `fn test_device(id, name)` |

**Add to test_utils.rs:**

```rust
use crate::daemon::Device;

/// Creates a test device with basic defaults.
///
/// # Arguments
/// * `id` - Device identifier
/// * `name` - Human-readable device name
///
/// # Returns
/// A Device with iOS platform, non-emulator defaults.
pub fn test_device(id: &str, name: &str) -> Device {
    test_device_full(id, name, "ios", false)
}

/// Creates a test device with platform specification.
///
/// # Arguments
/// * `id` - Device identifier
/// * `name` - Human-readable device name
/// * `platform` - Platform string (e.g., "ios", "android", "macos")
pub fn test_device_with_platform(id: &str, name: &str, platform: &str) -> Device {
    test_device_full(id, name, platform, false)
}

/// Creates a test device with full control over all fields.
///
/// # Arguments
/// * `id` - Device identifier
/// * `name` - Human-readable device name
/// * `platform` - Platform string
/// * `emulator` - Whether this is an emulator/simulator
pub fn test_device_full(id: &str, name: &str, platform: &str, emulator: bool) -> Device {
    Device {
        id: id.to_string(),
        name: name.to_string(),
        platform: platform.to_string(),
        emulator,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    }
}
```

### Acceptance Criteria

1. Three helper functions added: `test_device`, `test_device_with_platform`, `test_device_full`
2. All functions have doc comments
3. Functions are public and `#[cfg(test)]` gated
4. Existing tests still compile (helpers not yet migrated)

### Testing

```bash
# Verify test_utils compiles with new helpers
cargo test --lib test_utils

# Verify full test suite still passes
cargo test --lib
```

### Notes

- Keep as `#[cfg(test)]` only - these are test utilities
- The `Device` import may need adjustment based on module structure
- Task 04 will migrate existing usages

---

## Completion Summary

**Status:** ❌ Not done

**Files Modified:**
- (pending)

**Testing Performed:**
- (pending)
