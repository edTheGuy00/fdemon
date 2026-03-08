## Task: Delete Stale Files + Extract types.rs and backend.rs

**Objective**: Remove the 5 stale dead files created by task-02 (never wired up), then extract types/enums/constants and the DebugBackend trait from `mod.rs` into fresh `types.rs` and `backend.rs` modules.

**Depends on**: None

### Background

Task 02 from phase-4-fixes created `handlers.rs`, `events.rs`, `variables.rs`, `types.rs`, `backend.rs` but never added `mod` declarations or removed the original code from `mod.rs`. Subsequent tasks (03–15) modified `mod.rs` directly, making the dead files stale. They must be deleted and recreated fresh from the live code.

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: Remove types/backend code, add module declarations
- `crates/fdemon-dap/src/adapter/types.rs`: **DELETE then RECREATE** — fresh extraction from mod.rs
- `crates/fdemon-dap/src/adapter/backend.rs`: **DELETE then RECREATE** — fresh extraction from mod.rs
- `crates/fdemon-dap/src/adapter/handlers.rs`: **DELETE** (stale, will be recreated in task 03)
- `crates/fdemon-dap/src/adapter/events.rs`: **DELETE** (stale, will be recreated in task 02)
- `crates/fdemon-dap/src/adapter/variables.rs`: **DELETE** (stale, will be recreated in task 04)

### Details

**Step 1: Delete all 5 stale files**

```bash
rm crates/fdemon-dap/src/adapter/handlers.rs
rm crates/fdemon-dap/src/adapter/events.rs
rm crates/fdemon-dap/src/adapter/variables.rs
rm crates/fdemon-dap/src/adapter/types.rs
rm crates/fdemon-dap/src/adapter/backend.rs
```

**Step 2: Create fresh `types.rs`**

Extract from `mod.rs` (approximate line ranges — verify by reading the file):

- `enum StepMode` (~lines 485-496)
- `struct BreakpointResult` (~lines 496-509)
- `enum BackendError` (~lines 509-529)
- `enum DapExceptionPauseMode` (~lines 529-549)
- `enum DebugEvent` (~lines 549-634)
- `enum PauseReason` (~lines 634-665)
- `pub fn log_level_to_category` (~lines 665-672)
- All constants: `EVENT_CHANNEL_CAPACITY`, `MAX_VARIABLES_PER_REQUEST`, `REQUEST_TIMEOUT`, `ERR_NOT_CONNECTED`, `ERR_NO_DEBUG_SESSION`, `ERR_THREAD_NOT_FOUND`, `ERR_EVAL_FAILED`, `ERR_TIMEOUT`, `ERR_VM_DISCONNECTED` (~lines 673-727)

Add appropriate `pub` or `pub(crate)` visibility. The file should include its own imports (e.g., `use std::time::Duration` if `REQUEST_TIMEOUT` needs it).

**Step 3: Create fresh `backend.rs`**

Extract from `mod.rs`:

- `LocalDebugBackend` / `DebugBackend` trait (~lines 57-215)
- `DynDebugBackendInner` trait + `DynDebugBackend` struct + `impl DebugBackend for DynDebugBackend` (~lines 217-477)

Imports should reference `crate::adapter::types::{StepMode, BreakpointResult, BackendError, DapExceptionPauseMode}`.

**Step 4: Update `mod.rs`**

- Add module declarations: `pub mod types;` and `pub mod backend;` (after the existing `pub mod` declarations)
- Add `pub use` re-exports for all public items from `types` and `backend` that were previously directly accessible
- Remove the extracted code sections from `mod.rs`
- Update any remaining `mod.rs` code that referenced the moved items to use the new module paths (or rely on `use` imports from the new modules)

### Acceptance Criteria

1. The 5 stale files (`handlers.rs`, `events.rs`, `variables.rs`, `types.rs`, `backend.rs`) are deleted
2. Fresh `types.rs` created with all types, enums, constants, and `log_level_to_category` from `mod.rs`
3. Fresh `backend.rs` created with `DebugBackend` trait and `DynDebugBackend` wrapper from `mod.rs`
4. `mod types;` and `mod backend;` declarations added to `mod.rs`
5. All public items from extracted code are re-exported via `pub use` in `mod.rs`
6. The extracted code sections are removed from `mod.rs`
7. `mod.rs` production code reduced by ~670 lines (420 backend + 250 types)
8. `cargo check --workspace` — Pass
9. `cargo test --workspace` — Pass (all existing tests green)
10. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

No new tests needed — this is a pure extraction refactor. All existing tests must continue to pass.

### Notes

- The `DebugBackend` trait uses `#[trait_variant::make(DebugBackend: Send)]` — ensure the `trait_variant` attribute is preserved
- Some types like `BackendError` have `Display` and `Error` impls — ensure those move with the type
- Constants may need `pub(crate)` visibility since they're used by sibling modules within `adapter/`
- Don't forget to preserve doc comments on all extracted items

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/mod.rs` | Removed backend trait (~420 lines) and types/constants (~250 lines); added `pub mod backend;`, `pub mod types;` declarations and re-exports; moved test-only constants to test module imports |
| `crates/fdemon-dap/src/adapter/types.rs` | Deleted stale version, recreated fresh from mod.rs with all types, enums, constants, and `log_level_to_category` |
| `crates/fdemon-dap/src/adapter/backend.rs` | Deleted stale version, recreated fresh from mod.rs with `LocalDebugBackend`/`DebugBackend` trait and `DynDebugBackend` wrapper |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Deleted (stale file) |
| `crates/fdemon-dap/src/adapter/events.rs` | Deleted (stale file) |
| `crates/fdemon-dap/src/adapter/variables.rs` | Deleted (stale file) |

### Notable Decisions/Tradeoffs

1. **Test-only constant imports**: Constants like `REQUEST_TIMEOUT`, `ERR_NOT_CONNECTED`, `ERR_NO_DEBUG_SESSION`, `ERR_THREAD_NOT_FOUND`, `ERR_EVAL_FAILED`, `ERR_TIMEOUT` are only used in tests. Rather than importing them at the production level (causing clippy's `-D warnings` to fail with "unused import"), they are imported only inside the `#[cfg(test)] mod tests` block via `use super::types::{...};`. Production-level imports only include constants used in production code: `ERR_VM_DISCONNECTED`, `EVENT_CHANNEL_CAPACITY`, `MAX_VARIABLES_PER_REQUEST`.

2. **`BackendError` re-export from `backend.rs`**: The `backend.rs` module does `pub use crate::adapter::types::BackendError;` to make `BackendError` accessible at the `backend` module level, preserving the API surface used by external consumers.

3. **Line reduction**: `mod.rs` reduced from 8,025 to 7,368 lines (657 line reduction), matching the ~670 target. `types.rs` has 255 lines and `backend.rs` has 433 lines.

### Testing Performed

- `cargo check -p fdemon-dap` - Passed (no warnings)
- `cargo test -p fdemon-dap` - Passed (581 tests)
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all tests across all crates)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Tasks 02-04 pending**: The stale `handlers.rs`, `events.rs`, and `variables.rs` files have been deleted as required. They will be recreated in their respective tasks (02, 03, 04) of this phase.
