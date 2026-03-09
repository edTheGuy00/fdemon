# Task 01: Add `extra_args` to `LaunchContextState` and copy in `select_config()`

**File:** `crates/fdemon-app/src/new_session_dialog/state.rs`
**Depends on:** None
**Wave:** 1 (sequential)

## What to do

1. Add `pub extra_args: Vec<String>` field to `LaunchContextState` (around line 431, before `focused_field`)

2. Initialize it in `LaunchContextState::new()` as `extra_args: Vec::new()`

3. In `select_config()` (around line 506-526), after the `dart_defines` copy block, add:
   ```rust
   if !config.config.extra_args.is_empty() {
       self.extra_args = config.config.extra_args.clone();
   }
   ```

## Verification

- `cargo check -p fdemon-app` compiles
- Existing tests in `state.rs` still pass: `cargo test -p fdemon-app -- new_session_dialog::state`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/state.rs` | Added `pub extra_args: Vec<String>` field to `LaunchContextState`; initialized as `Vec::new()` in `new()`; added copy block in `select_config()` after `dart_defines` block |

### Notable Decisions/Tradeoffs

1. **Conditional copy (only when non-empty)**: Mirrors the existing `dart_defines` pattern — extra_args are only copied from config if the config provides at least one value, preserving any user-set state when a config without extra_args is selected.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app -- new_session_dialog::state` - Passed (25 tests)

### Risks/Limitations

1. **No new tests added**: The task specified only adding the field and copy logic; no acceptance criteria required new tests. The existing 25 tests exercise `select_config()` and continue to pass.
