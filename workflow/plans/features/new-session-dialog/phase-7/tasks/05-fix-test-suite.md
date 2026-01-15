# Task: Fix Test Suite Compilation

## Summary

Fix 168 test compilation errors caused by API changes in Phase 7. Tests reference old methods, fields, and constructors that were refactored.

**Priority:** CRITICAL (Blocking merge)

## Files

| File | Action |
|------|--------|
| `src/app/handler/tests.rs` | Modify (update API references) |
| `src/tui/widgets/new_session_dialog/state/tests/dialog_tests.rs` | Modify (update API references) |

## Problem

The Phase 7 refactoring changed the NewSessionDialogState API but tests weren't updated:

### Field Access Patterns

```rust
// OLD → NEW
state.flavor                → state.launch_context.flavor
state.loading_bootable      → state.target_selector.bootable_loading
state.active_pane           → state.focused_pane
state.target_tab            → state.target_selector.active_tab
```

### Method Calls

```rust
// OLD → NEW
state.switch_tab(tab)               → state.target_selector.set_tab(tab)
state.open_fuzzy_modal(type, items) → state.open_config_modal() / state.open_flavor_modal(items)
state.target_up()                   → state.target_selector.select_previous()
state.context_down()                → state.launch_context.focused_field = field.next()
```

### Constructor Calls

```rust
// OLD → NEW
NewSessionDialogState::new()              → NewSessionDialogState::new(LoadedConfigs::default())
NewSessionDialogState::with_configs(cfg)  → NewSessionDialogState::new(configs)
```

## Implementation

### Step 1: Audit test failures

Run `cargo test --lib 2>&1 | head -200` to get the full list of failing tests and error messages.

### Step 2: Update handler tests

In `src/app/handler/tests.rs`:
1. Find all `NewSessionDialogState::new()` calls and add `LoadedConfigs::default()` parameter
2. Update field access patterns to use nested state
3. Update method calls to use new API

### Step 3: Update dialog state tests

In `src/tui/widgets/new_session_dialog/state/tests/dialog_tests.rs`:
1. Update constructor calls
2. Update field access patterns
3. Update method calls

### Step 4: Verify compilation

```bash
cargo test --lib --no-run
```

## Acceptance Criteria

1. `cargo test --lib` compiles without errors
2. All existing tests pass (no regressions)
3. No new `#[ignore]` attributes added

## Testing

```bash
cargo fmt && cargo check && cargo test --lib && cargo clippy -- -D warnings
```

## Notes

- Do NOT add new tests in this task - focus only on fixing existing tests
- If a test is testing obsolete behavior, update it to test the new equivalent behavior
- Keep test names unchanged where possible for git history
