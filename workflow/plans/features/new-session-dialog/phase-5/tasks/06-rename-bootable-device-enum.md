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
