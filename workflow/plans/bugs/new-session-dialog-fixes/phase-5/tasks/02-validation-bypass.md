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

**Status:** Not Started
