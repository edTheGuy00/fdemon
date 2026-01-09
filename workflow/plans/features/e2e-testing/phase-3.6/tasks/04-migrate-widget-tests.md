## Task: Migrate Widget Tests to Shared test_device() Helper

**Objective**: Replace all duplicated `test_device()` implementations in widget files with imports from `test_utils.rs`.

**Depends on**: 03-extract-test-device

### Scope

- `src/tui/widgets/device_selector.rs`
- `src/tui/widgets/status_bar.rs`
- `src/tui/widgets/header.rs`
- `src/tui/widgets/tabs.rs`
- `src/tui/widgets/startup_dialog/mod.rs`

### Details

For each file:

1. **Remove** the local `test_device()` function definition
2. **Add** import from test_utils
3. **Update** calls to match new API (if signature differs)

**Example migration:**

**Before (device_selector.rs):**
```rust
#[cfg(test)]
mod tests {
    // ... other imports ...

    fn test_device(id: &str, name: &str, emulator: bool) -> Device {
        Device {
            id: id.to_string(),
            // ... fields ...
        }
    }

    #[test]
    fn test_something() {
        let device = test_device("id1", "iPhone", true);
    }
}
```

**After:**
```rust
#[cfg(test)]
mod tests {
    use crate::tui::test_utils::{test_device, test_device_full};
    // ... other imports ...

    #[test]
    fn test_something() {
        // Use test_device_full when emulator flag needed
        let device = test_device_full("id1", "iPhone", "ios", true);
    }
}
```

**Migration map:**

| File | Old Signature | New Call |
|------|---------------|----------|
| `device_selector.rs` | `test_device(id, name, emulator)` | `test_device_full(id, name, "ios", emulator)` |
| `status_bar.rs` | `test_device(id, name)` | `test_device(id, name)` |
| `header.rs` | `test_device(id, name, platform)` | `test_device_with_platform(id, name, platform)` |
| `tabs.rs` | `test_device(id, name)` | `test_device(id, name)` |
| `startup_dialog` | `test_device(id, name)` | `test_device(id, name)` |

### Acceptance Criteria

1. No `test_device()` function defined in any widget file
2. All widget tests import from `crate::tui::test_utils`
3. All widget tests pass
4. No duplicate code for Device creation

### Testing

```bash
# Run all widget tests
cargo test --lib widgets

# Run specific widget tests to verify each migration
cargo test --lib device_selector
cargo test --lib status_bar
cargo test --lib header
cargo test --lib tabs
cargo test --lib startup_dialog
```

### Notes

- Do NOT combine this with the status_bar refactor (task 05)
- Keep test logic unchanged - only update helper usage
- Some tests may need `test_device_with_platform` for platform-specific behavior

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/device_selector.rs` | Removed local `test_device(id, name, emulator)` function; added import of `test_device_full`; updated all calls to use `test_device_full(id, name, "ios", emulator)` |
| `src/tui/widgets/status_bar.rs` | Removed local `test_device(id, name)` function; added import of `test_device`; no call changes needed (signature matches) |
| `src/tui/widgets/header.rs` | Removed local `test_device(id, name, platform)` function; added import of `test_device_with_platform`; updated all calls to use `test_device_with_platform(id, name, platform)` |
| `src/tui/widgets/tabs.rs` | Removed local `test_device(id, name)` function; added import of `test_device`; no call changes needed (signature matches) |
| `src/tui/widgets/startup_dialog/mod.rs` | Removed local `test_device(id, name)` function; added import of `test_device`; no call changes needed (signature matches) |

### Notable Decisions/Tradeoffs

1. **Import Cleanup**: Removed unused `Device` imports from test modules that were only needed for the local `test_device()` function. This reduces clutter and improves code clarity.
2. **Platform Parameter**: For `device_selector.rs`, consistently used `"ios"` as the platform parameter for `test_device_full()` to match the original implementation, ensuring no behavioral changes to tests.
3. **Minimal Migration Scope**: Only migrated the test helper functions as specified, without touching any test logic or assertions. This keeps the change focused and reduces risk.

### Testing Performed

- `cargo test --lib device_selector` - Passed (57 tests)
- `cargo test --lib status_bar` - Passed (34 tests)
- `cargo test --lib header` - Passed (18 tests)
- `cargo test --lib tabs` - Passed (17 tests)
- `cargo test --lib startup_dialog` - Passed (38 tests)
- `cargo test --lib widgets` - Passed (253 tests total)
- `cargo clippy --lib -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None**: All tests pass with no changes to test behavior. The migration is purely a code organization improvement with no functional impact.
