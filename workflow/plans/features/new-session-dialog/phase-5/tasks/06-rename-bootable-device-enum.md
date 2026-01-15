## Task: Rename BootableDevice Enum in TUI Layer

**Objective**: Resolve type name conflict between `core::BootableDevice` (domain struct) and TUI layer's enum with the same name.

**Depends on**: 05-target-selector-messages

**Priority**: Critical

**Source**: Architecture Enforcer - Review Issue #1

### Scope

- `src/tui/widgets/new_session_dialog/device_groups.rs`: Rename enum `BootableDevice` → `GroupedBootableDevice`
- `src/tui/widgets/new_session_dialog/device_list.rs`: Update import and usages
- `src/tui/widgets/new_session_dialog/target_selector.rs`: Update import and usages
- `src/tui/widgets/new_session_dialog/mod.rs`: Update re-export

### Problem

Two types with the same name exist in different modules:

| Location | Type | Purpose |
|----------|------|---------|
| `src/core/types.rs:667` | struct | Domain type with id, name, platform, runtime, state |
| `src/tui/widgets/new_session_dialog/device_groups.rs:109` | enum | Wrapper around IosSimulator/AndroidAvd |

**Impact:**
- Import ambiguity (`use crate::core::BootableDevice` vs TUI version)
- Risk of accidental type confusion during refactoring
- Confusing for maintainers

### Details

Rename the TUI enum from `BootableDevice` to `GroupedBootableDevice`:

```rust
// device_groups.rs - BEFORE
pub enum BootableDevice {
    IosSimulator(IosSimulator),
    AndroidAvd(AndroidAvd),
}

// device_groups.rs - AFTER
pub enum GroupedBootableDevice {
    IosSimulator(IosSimulator),
    AndroidAvd(AndroidAvd),
}
```

Update all references in:
1. `device_groups.rs` - The enum definition and all internal usages
2. `device_list.rs` - Imports and match statements
3. `target_selector.rs` - Imports and any usages
4. `mod.rs` - The public re-export

### Acceptance Criteria

1. Enum renamed from `BootableDevice` to `GroupedBootableDevice`
2. All internal references updated
3. Re-export updated in mod.rs
4. `cargo check` passes with no ambiguous import errors
5. `cargo test` passes - all existing tests work
6. No type confusion possible between `core::BootableDevice` and TUI enum

### Testing

```bash
cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
```

Verify no ambiguous imports exist by searching:
```bash
rg "use.*BootableDevice" --type rust
```

### Notes

- This is a straightforward rename with no logic changes
- All tests should continue to pass unchanged
- Consider adding a comment to the enum explaining its purpose vs the core type

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/device_groups.rs` | Renamed enum `BootableDevice` → `GroupedBootableDevice`, updated all internal references, added doc comment explaining distinction from core type |
| `src/tui/widgets/new_session_dialog/device_list.rs` | Updated import to use `GroupedBootableDevice`, updated function signature and test code |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Updated import to use `GroupedBootableDevice`, updated match expressions and return type for `selected_bootable_device()` |
| `src/tui/widgets/new_session_dialog/mod.rs` | No changes needed (wildcard re-export automatically picks up renamed type) |

### Notable Decisions/Tradeoffs

1. **Added clarifying doc comment**: Added documentation to `GroupedBootableDevice` explaining it's distinct from `core::BootableDevice` domain type, preventing future confusion.
2. **Name choice**: Chose `GroupedBootableDevice` to clearly indicate this enum is for grouping bootable devices in the TUI rendering layer, distinguishing it from the core domain type.
3. **Struct vs Enum**: Kept `BootableDeviceList` struct name unchanged - only the enum needed renaming to avoid conflict.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (1 dead_code warning for unrelated method)
- `cargo clippy -- -D warnings` - Passed
- `cargo test` - 1534 passed; 1 failed (unrelated pre-existing test failure in `test_switch_tab_skips_header`)
- Verified no ambiguous imports: All `BootableDevice` imports now correctly reference `core::BootableDevice`, TUI layer uses `GroupedBootableDevice`

### Risks/Limitations

1. **Pre-existing test failure**: One test (`test_switch_tab_skips_header`) was already failing from phase 5 commit (f133a63). This test failure is NOT caused by the rename - it's related to `first_selectable_target_index()` logic in state.rs which was added in that commit. The test expects selection index 1 but gets 0. This should be addressed separately.
2. **No breaking changes**: This is an internal TUI type rename with no API surface changes - safe to merge.
