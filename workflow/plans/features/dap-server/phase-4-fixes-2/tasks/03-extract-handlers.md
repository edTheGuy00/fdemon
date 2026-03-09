## Task: Extract Request Handlers into handlers.rs

**Objective**: Move all request-handling methods from the `DapAdapter` impl block in `mod.rs` into a fresh `handlers.rs` module.

**Depends on**: 02-extract-events

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: Remove handler methods from impl block, add `mod handlers;`
- `crates/fdemon-dap/src/adapter/handlers.rs`: **CREATE** with extracted handler methods

### Details

Extract these methods from the `DapAdapter<B>` impl block in `mod.rs`:

| Method | Approx. Lines | Visibility |
|--------|---------------|------------|
| `handle_request` | ~871–909 | `pub async` |
| `handle_attach` | ~1451–1537 | `async` (private) |
| `handle_threads` | ~1539–1562 | `async` (private) |
| `handle_set_breakpoints` | ~1564–1782 | `async` (private) |
| `handle_set_exception_breakpoints` | ~1784–1847 | `async` (private) |
| `primary_isolate_id` | ~1849–1858 | `fn` (private) |
| `most_recent_paused_isolate` | ~1860–1866 | `fn` (private) |
| `handle_continue` | ~1868–1904 | `async` (private) |
| `handle_next` | ~1906–1912 | `async` (private) |
| `handle_step_in` | ~1914–1920 | `async` (private) |
| `handle_step_out` | ~1922–1928 | `async` (private) |
| `step` | ~1930–1960 | `async` (private) |
| `handle_pause` | ~1962–1989 | `async` (private) |
| `handle_disconnect` | ~2256–2305 | `async` (private) |
| `handle_evaluate` | ~2614–2637 | `async` (private) |
| `handle_source` | ~2639–2709 | `async` (private) |
| `handle_hot_reload` | ~2711–2730 | `async` (private) |
| `handle_hot_restart` | ~2732–2755 | `async` (private) |

Also extract these free functions:
| Function | Approx. Lines | Visibility |
|----------|---------------|------------|
| `parse_args` | ~2757–2772 | `pub(crate)` |
| `path_to_dart_uri` | ~2774–2792 | `pub(crate)` |
| `entry_to_dap_breakpoint` | ~2794–2815 | `pub(crate)` |
| `exception_filter_to_mode` | ~2817–2829 | `pub(crate)` |

**File structure for `handlers.rs`:**

```rust
//! # Request Handlers
//!
//! DapAdapter methods for dispatching and handling DAP protocol requests.

use crate::adapter::backend::DebugBackend;
use crate::adapter::breakpoints::{...};
use crate::adapter::types::{...};
use crate::adapter::DapAdapter;
// ... other necessary imports

impl<B: DebugBackend> DapAdapter<B> {
    pub async fn handle_request(&mut self, request: &DapRequest) -> DapResponse { ... }
    // ... all handle_* methods
}

pub(crate) fn parse_args<T: serde::de::DeserializeOwned>(request: &DapRequest) -> Result<T, String> { ... }
pub(crate) fn path_to_dart_uri(path: &str) -> String { ... }
// ... other free functions
```

**Update `mod.rs`:**
- Add `mod handlers;` declaration
- Remove all extracted methods from the impl block
- Remove all extracted free functions
- If any of the free functions are used in `mod.rs` tests, add appropriate `use` imports in the test module

### Acceptance Criteria

1. `handlers.rs` contains all 18 methods + 4 free functions listed above
2. Methods and functions removed from `mod.rs`
3. `mod handlers;` declaration added to `mod.rs`
4. `handle_request` remains the public entry point — no API changes
5. All existing tests pass
6. `cargo check --workspace` — Pass
7. `cargo test --workspace` — Pass
8. `cargo clippy --workspace -- -D warnings` — Pass

### Notes

- `handle_request` dispatches to other `handle_*` methods. Since they're all in the same impl block in `handlers.rs`, this works naturally.
- `handle_request` also dispatches to `handle_stack_trace`, `handle_scopes`, `handle_variables` which will be in `variables.rs` (task 04). For now, those methods are still in `mod.rs`, so the dispatch works. Task 04 will move them.
- `handle_evaluate` delegates to `crate::adapter::evaluate::handle_evaluate` — this import stays the same.
- Free functions like `parse_args` need `pub(crate)` visibility since `variables.rs` (task 04) will need to call them.
- `send_event` (now in `events.rs`) is called by handler methods. It must be accessible — verify its visibility after task 02.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/handlers.rs` | Created new file with all 18 extracted handler methods and 4 free functions |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `mod handlers;`, removed 18 handler methods + 4 free functions, updated imports |

### Notable Decisions/Tradeoffs

1. **`parse_args` accessibility**: `parse_args` is `pub(crate)` in `handlers.rs`, used by the remaining `handle_stack_trace`, `handle_scopes`, `handle_variables` methods still in `mod.rs`. Added `use handlers::parse_args;` to `mod.rs` module level to make this work cleanly.

2. **`ERR_VM_DISCONNECTED` in tests**: The constant was previously imported into `mod.rs` scope via `use types::{}` and then re-exported via `use super::*` to tests. After removing it from `mod.rs`'s module-level import, added it explicitly to the test module's `use super::types::{}` import list.

3. **`path_to_dart_uri` and `exception_filter_to_mode` in tests**: These moved to `handlers.rs` as `pub(crate)`. Since `mod handlers;` is a private declaration, `use super::*` does not pull in items from submodules. Added explicit `use super::handlers::{exception_filter_to_mode, path_to_dart_uri};` to the test module.

4. **Method visibility**: Private handler methods in `handlers.rs` use `pub(super)` so they remain accessible from within the `adapter` module (including tests in `mod.rs`). `handle_request` remains `pub`.

### Testing Performed

- `cargo check -p fdemon-dap` — Passed
- `cargo test -p fdemon-dap` — Passed (581 tests)
- `cargo clippy -p fdemon-dap -- -D warnings` — Passed (no warnings)
- `cargo fmt --all` — Passed

### Risks/Limitations

1. **Remaining methods in mod.rs**: `handle_stack_trace`, `handle_scopes`, `handle_variables`, `get_scope_variables`, `instance_ref_to_variable`, `expand_object` remain in `mod.rs` and will be moved in task 04 (variables.rs extraction).
