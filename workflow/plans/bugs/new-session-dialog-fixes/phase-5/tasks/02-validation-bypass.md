## Task: Remove Direct Config Mutation (Validation Bypass)

**Objective**: Remove code that directly mutates config fields, bypassing the editability validation in `set_flavor()` and `set_dart_defines()`.

**Priority**: Critical

**Depends on**: None

### Scope

- `src/app/handler/new_session/launch_context.rs`: Remove direct config mutation blocks

### Problem Analysis

After calling validated setter methods, the code directly mutates config structs:

**Flavor Handler (lines 185-199):**
```rust
// Apply the flavor to state (GOOD - goes through validation)
state
    .new_session_dialog_state
    .launch_context
    .set_flavor(flavor.clone());

// ... then later ...

// Update the config with the new flavor (BAD - bypasses validation)
if let Some(config_idx) = state
    .new_session_dialog_state
    .launch_context
    .selected_config_index
{
    if let Some(config) = state
        .new_session_dialog_state
        .launch_context
        .configs
        .configs
        .get_mut(config_idx)
    {
        config.config.flavor = flavor;  // ← BYPASSES is_flavor_editable()!
    }
}
```

**Dart-Defines Handler (lines 285-303):**
```rust
// Apply the dart-defines to state (GOOD - goes through validation)
state
    .new_session_dialog_state
    .launch_context
    .set_dart_defines(defines.clone());

// ... then later ...

// Update the config with the new dart-defines (BAD - bypasses validation)
if let Some(config_idx) = state
    .new_session_dialog_state
    .launch_context
    .selected_config_index
{
    if let Some(config) = state
        .new_session_dialog_state
        .launch_context
        .configs
        .configs
        .get_mut(config_idx)
    {
        config.config.dart_defines = defines  // ← BYPASSES are_dart_defines_editable()!
            .iter()
            .map(|d| (d.key.clone(), d.value.clone()))
            .collect();
    }
}
```

### Why This Is Dangerous

The validation methods check `is_field_editable()` which returns `false` for VSCode configs (they should be read-only). The direct mutation bypasses this check, allowing modification of configs that should be immutable.

### Solution

**Delete the direct config mutation blocks entirely.** The validated setters (`set_flavor()`, `set_dart_defines()`) are sufficient. Config persistence should happen through the auto-save mechanism, not through direct struct mutation.

### Implementation

**In `handle_flavor_selected()` - DELETE lines 185-199:**

```rust
// DELETE THIS ENTIRE BLOCK:
// Update the config with the new flavor
if let Some(config_idx) = state
    .new_session_dialog_state
    .launch_context
    .selected_config_index
{
    if let Some(config) = state
        .new_session_dialog_state
        .launch_context
        .configs
        .configs
        .get_mut(config_idx)
    {
        config.config.flavor = flavor;
    }
}
```

**In `handle_dart_defines_updated()` - DELETE lines 293-309:**

```rust
// DELETE THIS ENTIRE BLOCK:
// Update the config with the new dart-defines
if let Some(config_idx) = state
    .new_session_dialog_state
    .launch_context
    .selected_config_index
{
    if let Some(config) = state
        .new_session_dialog_state
        .launch_context
        .configs
        .configs
        .get_mut(config_idx)
    {
        // Convert Vec<DartDefine> to HashMap<String, String>
        config.config.dart_defines = defines
            .iter()
            .map(|d| (d.key.clone(), d.value.clone()))
            .collect();
    }
}
```

### Acceptance Criteria

1. No direct mutation of `config.config.flavor` in handlers
2. No direct mutation of `config.config.dart_defines` in handlers
3. All field changes go through validated setter methods only
4. VSCode configs remain read-only (cannot be modified)
5. Editable configs still work correctly
6. All existing tests pass

### Testing

```bash
cargo test launch_context
cargo test flavor
cargo test dart_defines
```

Add test to verify validation:
```rust
#[test]
fn test_vscode_config_flavor_not_modifiable() {
    // Create state with VSCode config selected
    // Attempt to set flavor
    // Verify flavor was NOT changed
}
```

### Notes

- Config persistence should be handled by the auto-save feature, not inline mutation
- If configs need to be synced, that should be a separate, validated operation

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/new_session/launch_context.rs` | Removed direct config mutation blocks in `handle_flavor_selected()` (lines 184-199) and `handle_dart_defines_updated()` (lines 292-311) that bypassed validation. Updated tests to verify launch_context state instead of config struct mutation. |

### Notable Decisions/Tradeoffs

1. **Test Assertion Updates**: Tests were updated to check `state.new_session_dialog_state.launch_context.flavor` and `.dart_defines` instead of the underlying config struct fields. This correctly validates that the validated setters (`set_flavor()`, `set_dart_defines()`) are working, not that config structs are being mutated directly.

2. **Validation Enforcement**: By removing the bypass code, VSCode configs are now truly read-only. The validated setters check `is_flavor_editable()` and `are_dart_defines_editable()` which return `false` for VSCode configs, preventing any modification.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test launch_context` - Passed (33 tests)
- `cargo test flavor` - Passed (15 tests)
- `cargo test dart_defines` - Passed (47 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Config Persistence**: Config changes are now only persisted through the auto-save mechanism, not through direct mutation. This is the correct behavior - the auto-save action is triggered for FDemon configs only, ensuring VSCode configs remain read-only.

2. **State Synchronization**: The launch_context state fields (`flavor`, `dart_defines`) are the source of truth, not the config struct fields. When launching, these values are read from launch_context state via `build_launch_params()`.
