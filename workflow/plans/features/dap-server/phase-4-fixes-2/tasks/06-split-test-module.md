## Task: Split Test Module into Themed Submodule Files

**Objective**: Break the ~4,500-line `mod tests` block in `mod.rs` into themed test submodule files under a `tests/` directory within the adapter module.

**Depends on**: 05-move-mocks-to-helpers

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: Replace inline `mod tests` with submodule declarations
- `crates/fdemon-dap/src/adapter/tests/`: **CREATE** directory with themed test files

### Details

**Directory structure:**

```
crates/fdemon-dap/src/adapter/
├── mod.rs              (≤300 lines — struct + constructors + mod declarations)
├── tests/
│   ├── mod.rs          (submodule declarations + shared helpers)
│   ├── adapter_core.rs (construction, dispatch, error handling)
│   ├── attach_threads.rs
│   ├── breakpoints.rs
│   ├── execution.rs
│   ├── stack_scopes_variables.rs
│   ├── events_logging.rs
│   ├── hot_operations.rs
│   ├── conditional_breakpoints.rs
│   ├── logpoints.rs
│   ├── custom_events.rs
│   ├── breakpoint_persistence.rs
│   └── production_hardening.rs
```

**Test file breakdown:**

| File | Test Groups | Approx. Lines |
|------|-------------|---------------|
| `adapter_core.rs` | `test_adapter_new_*`, `handle_request` dispatch, `on_resume`, `pause_reason_to_dap_str`, `path_to_dart_uri`, `exception_filter_to_mode` | ~310 |
| `attach_threads.rs` | `handle_attach` tests, `handle_threads` tests, thread name lifecycle | ~300 |
| `breakpoints.rs` | `handle_set_breakpoints`, `handle_set_exception_breakpoints`, `BreakpointResolved` events | ~450 |
| `execution.rs` | continue/next/stepIn/stepOut/pause tests | ~340 |
| `stack_scopes_variables.rs` | `handle_stack_trace`, `handle_scopes`, `instance_ref_to_variable`, `handle_variables` | ~510 |
| `events_logging.rs` | `handle_debug_event`, `log_level_to_category`, `LogOutput` events, `DapEvent::output`, `emit_output`, `BackendError` type safety | ~370 |
| `hot_operations.rs` | hotReload/hotRestart custom requests | ~250 |
| `conditional_breakpoints.rs` | Conditional breakpoint integration tests | ~455 |
| `logpoints.rs` | Logpoint tests | ~490 |
| `custom_events.rs` | Custom DAP events (debuggerUris, appStart, etc.) | ~220 |
| `breakpoint_persistence.rs` | Breakpoint persistence across hot restart | ~420 |
| `production_hardening.rs` | Error codes, VM disconnect, rate limiting, constants | ~480 |

**`tests/mod.rs` structure:**

```rust
//! Integration tests for the DAP adapter.

mod adapter_core;
mod attach_threads;
mod breakpoints;
mod execution;
mod stack_scopes_variables;
mod events_logging;
mod hot_operations;
mod conditional_breakpoints;
mod logpoints;
mod custom_events;
mod breakpoint_persistence;
mod production_hardening;
```

**Each test file should:**

```rust
use crate::adapter::test_helpers::*;  // MockBackend, MockBackendWithUri, etc.
use crate::adapter::*;                // DapAdapter, DebugEvent, etc.
use crate::{DapMessage, DapRequest, DapResponse};
// ... other imports as needed
```

**Shared test helpers** (like `make_request`, `make_set_breakpoints_request`, `drain_events`) should go in `tests/mod.rs` as `pub(super)` functions, or in a dedicated `tests/helpers.rs` submodule.

**Update `adapter/mod.rs`:**

Replace:
```rust
#[cfg(test)]
mod tests {
    // ... 4,500+ lines
}
```

With:
```rust
#[cfg(test)]
mod tests;
```

### Acceptance Criteria

1. `adapter/mod.rs` total is ≤ 300 lines (no inline test block)
2. `tests/` directory created with 12 themed test files
3. All ~163 test functions preserved — no tests deleted
4. Shared helper functions accessible to all test submodules
5. `make_request`, `make_set_breakpoints_request`, etc. available to all test files
6. All existing tests pass with identical behavior
7. `cargo check --workspace` — Pass
8. `cargo test --workspace` — Pass
9. `cargo test -p fdemon-dap` — all 581+ tests pass
10. `cargo clippy --workspace -- -D warnings` — Pass

### Notes

- In Rust, `#[cfg(test)] mod tests;` works — the compiler looks for `tests.rs` or `tests/mod.rs` in the same directory. The `#[cfg(test)]` attribute applies to the entire module tree.
- Each test submodule automatically inherits the `#[cfg(test)]` from the parent `mod tests`
- The inline mock structs (`ErrorEvalBackend`, `TrackingBackend`, `StopTrackingBackend`, `StopTrackingBackend2`) stay in their respective test files — they're small and coupled to specific tests
- Helper functions like `make_conditional_adapter` and `make_logpoint_adapter` go in the test file that uses them (`conditional_breakpoints.rs` and `logpoints.rs` respectively)
- This is the largest task — ~4,500 lines being reorganized into 12 files. Test behavior must be preserved exactly.
