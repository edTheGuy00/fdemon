## Task: Remove Blanket `#[allow(dead_code)]` and Clean Handler Dead Code

**Objective**: Remove all blanket `#[allow(dead_code)]` attributes from handler submodule declarations in `handler/mod.rs`, then delete or fix the genuinely dead code they were masking.

**Depends on**: None

**Severity**: MAJOR (masks 17+ dead functions)

**Source**: Architecture Enforcer, Code Quality Inspector, Logic & Reasoning Checker (ACTION_ITEMS.md Major #1)

### Scope

- `crates/fdemon-app/src/handler/mod.rs:18-37`: Remove blanket `#[allow(dead_code)]`
- `crates/fdemon-app/src/handler/log_view.rs`: Delete 16 dead functions
- `crates/fdemon-app/src/handler/helpers.rs:25`: Handle `is_logger_block_line`

### Details

**The problem:** Every handler submodule except `update` has `#[allow(dead_code)]`. The comment at line 16-17 claims "the compiler cannot trace `pub(crate)` cross-module usage" -- this is incorrect. Rust's `dead_code` lint handles `pub(crate)` accurately. The blanket allows mask genuinely dead code.

**Research findings -- submodule status:**

| Submodule | All Used? | Dead Items | Action |
|-----------|-----------|------------|--------|
| `daemon` | Yes | 0 | Remove `#[allow(dead_code)]` only |
| `helpers` | Mostly | 1 (`is_logger_block_line`) | Move to `#[cfg(test)]` |
| `keys` | Yes | 0 | Remove `#[allow(dead_code)]` only |
| `session` | Yes | 0 | Remove `#[allow(dead_code)]` only |
| `session_lifecycle` | Yes | 0 | Remove `#[allow(dead_code)]` only |
| `scroll` | Yes | 0 | Remove `#[allow(dead_code)]` only |
| **`log_view`** | **No** | **16 functions + 1 private helper** | Delete dead functions |
| `new_session` | Yes | 0 | Remove `#[allow(dead_code)]` only |
| `settings` | Yes | 0 | Remove `#[allow(dead_code)]` only |
| `settings_handlers` | Yes | 0 | Remove `#[allow(dead_code)]` only |

**`log_view.rs` dead functions (16 of 17 are dead):**

These are duplicates of logic already inlined in `update.rs`. The dispatch in `update.rs` was never updated to call the extracted functions:

- `handle_clear_logs` (duplicated at update.rs:445)
- `handle_cycle_level_filter` (duplicated at update.rs:456)
- `handle_cycle_source_filter` (duplicated at update.rs:463)
- `handle_reset_filters` (duplicated at update.rs:470)
- `handle_start_search` (duplicated at update.rs:480)
- `handle_cancel_search` (duplicated at update.rs:488)
- `handle_clear_search` (duplicated at update.rs:496)
- `handle_search_input` (duplicated at update.rs:504)
- `handle_next_search_match` (duplicated at update.rs:522)
- `handle_prev_search_match` (duplicated at update.rs:534)
- `handle_search_completed` (duplicated at update.rs:546)
- `handle_next_error` (duplicated at update.rs:556)
- `handle_prev_error` (duplicated at update.rs:565)
- `handle_toggle_stack_trace` (duplicated at update.rs:577)
- `handle_enter_link_mode` (duplicated at update.rs:592)
- `handle_exit_link_mode` (duplicated at update.rs:623)
- `scroll_to_log_entry` (private, dead -- only called by dead functions above)

Only `handle_select_link` is actually called (from update.rs:632). Keep it.

**Also fix:**
- Remove the incorrect comment at mod.rs lines 16-17
- Remove unused re-exports at mod.rs lines 52-55 (`detect_raw_line_level`, `handle_key`) which have their own `#[allow(unused_imports)]`

### Implementation Steps

1. Remove all `#[allow(dead_code)]` from submodule declarations in `mod.rs`
2. Remove the incorrect comment about `pub(crate)` tracing
3. Delete the 16 dead functions + 1 private helper from `log_view.rs` (keep only `handle_select_link`)
4. Move `is_logger_block_line` in `helpers.rs` to `#[cfg(test)]` block
5. Remove unused re-exports at mod.rs lines 52-55
6. Run `cargo check -p fdemon-app` -- fix any new warnings
7. Run `cargo test -p fdemon-app --lib` -- verify tests pass

### Acceptance Criteria

1. No blanket `#[allow(dead_code)]` on any submodule declaration
2. `log_view.rs` contains only `handle_select_link` (and any imports it needs)
3. `is_logger_block_line` only exists inside `#[cfg(test)]`
4. No `#[allow(unused_imports)]` on mod.rs re-exports (because they're removed)
5. `cargo check -p fdemon-app` produces zero `dead_code` warnings
6. `cargo test -p fdemon-app --lib` passes (726+ handler tests)

### Testing

```bash
# Check for dead_code warnings
cargo check -p fdemon-app 2>&1 | grep 'dead_code'

# Run handler tests
cargo test -p fdemon-app --lib

# Verify no blanket allows remain
rg '#\[allow\(dead_code\)\]' crates/fdemon-app/src/handler/mod.rs
```

### Notes

- The 16 dead `log_view` functions appear to have been an incomplete extraction -- `update.rs` still handles these messages inline. Deleting the dead copies is correct; the inline handling in `update.rs` is the live code path.
- After this cleanup, if future refactoring extracts log_view handlers from update.rs, it should update the dispatch to call them (not just create dead copies).
- The `helpers.rs` function `is_logger_block_line` has tests -- those tests should remain, they just need to be in a `#[cfg(test)]` block alongside the function.

---

## Completion Summary

**Status:** Not started
