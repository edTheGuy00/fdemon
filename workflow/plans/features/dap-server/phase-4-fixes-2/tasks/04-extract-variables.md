## Task: Extract Variable/Scope Handling into variables.rs

**Objective**: Move stack trace, scopes, and variable handling methods from the `DapAdapter` impl block in `mod.rs` into a fresh `variables.rs` module.

**Depends on**: 03-extract-handlers

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: Remove variable/scope methods from impl block, add `mod variables;`
- `crates/fdemon-dap/src/adapter/variables.rs`: **CREATE** with extracted methods

### Details

Extract these methods from the `DapAdapter<B>` impl block in `mod.rs`:

| Method | Approx. Lines | Visibility |
|--------|---------------|------------|
| `handle_stack_trace` | ~1991–2090 | `async` (private) |
| `handle_scopes` | ~2092–2165 | `async` (private) |
| `handle_variables` | ~2167–2254 | `async` (private) |
| `get_scope_variables` | ~2307–2372 | `async` (private) |
| `instance_ref_to_variable` | ~2374–2499 | `fn` (private) |
| `expand_object` | ~2501–2612 | `async` (private) |

**File structure for `variables.rs`:**

```rust
//! # Variable & Scope Handling
//!
//! DapAdapter methods for stack traces, scopes, and variable inspection.

use crate::adapter::backend::DebugBackend;
use crate::adapter::handlers::parse_args;
use crate::adapter::types::{...};
use crate::adapter::DapAdapter;
// ... other necessary imports

impl<B: DebugBackend> DapAdapter<B> {
    pub(super) async fn handle_stack_trace(&mut self, request: &DapRequest) -> DapResponse { ... }
    pub(super) async fn handle_scopes(&mut self, request: &DapRequest) -> DapResponse { ... }
    pub(super) async fn handle_variables(&mut self, request: &DapRequest) -> DapResponse { ... }
    async fn get_scope_variables(&self, isolate_id: &str, frame_index: i64) -> ... { ... }
    fn instance_ref_to_variable(&self, ...) -> DapVariable { ... }
    async fn expand_object(&self, ...) -> Vec<DapVariable> { ... }
}
```

**Update `mod.rs`:**
- Add `mod variables;` declaration (private module)
- Remove all extracted methods from the impl block
- After this task, the `mod.rs` impl block should only contain `new`, `new_with_tx`, and re-exports

**Visibility note:** `handle_stack_trace`, `handle_scopes`, `handle_variables` are called by `handle_request` in `handlers.rs`. Since handlers.rs dispatches via `self.handle_stack_trace(request)`, and `variables.rs` is a sibling module providing an `impl<B> DapAdapter<B>` block, the methods need to be at least `pub(super)` or `pub(crate)` so the compiler finds them as part of the DapAdapter impl.

Actually, in Rust, when you have `impl<B> DapAdapter<B>` in multiple files (mod.rs, events.rs, handlers.rs, variables.rs), **all methods are visible to each other regardless of visibility** because they're all methods on the same type. The `pub`/`pub(crate)` only controls external access. So private methods in `variables.rs` can be called from `handlers.rs` as `self.handle_stack_trace()` because they're inherent methods on the same type.

### Acceptance Criteria

1. `variables.rs` contains all 6 methods listed above
2. Methods removed from `mod.rs` impl block
3. `mod variables;` declaration added to `mod.rs`
4. The `mod.rs` impl block now only contains `new` and `new_with_tx` (~50 lines)
5. `mod.rs` production code is ≤ 300 lines total (struct definition + constructors + module declarations + re-exports)
6. All existing tests pass
7. `cargo check --workspace` — Pass
8. `cargo test --workspace` — Pass
9. `cargo clippy --workspace -- -D warnings` — Pass

### Notes

- `handle_stack_trace` handles `AsyncSuspensionMarker` frames with `presentation_hint: "label"` — ensure this logic is preserved exactly (the stale `variables.rs` was missing this)
- `handle_scopes` returns Locals scope only (Globals was removed by task 13) — verify current behavior matches
- `instance_ref_to_variable` is a complex method (~125 lines) that maps Dart instance types to DAP variables — test carefully
- `parse_args` (from `handlers.rs`) is used by `handle_stack_trace`, `handle_scopes`, `handle_variables` — import via `crate::adapter::handlers::parse_args`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `mod variables;` declaration; removed 6 extracted methods from impl block; cleaned up unused imports (`DapScope`, `DapStackFrame`, `DapVariable`, `ScopesArguments`, `StackTraceArguments`, `VariablesArguments`, `parse_args`, `MAX_VARIABLES_PER_REQUEST`); moved test-only imports to `#[cfg(test)]` blocks; moved `MAX_VARIABLES_PER_REQUEST` to test module's explicit import |
| `crates/fdemon-dap/src/adapter/variables.rs` | **CREATED** — new file containing all 6 extracted methods: `handle_stack_trace`, `handle_scopes`, `handle_variables`, `get_scope_variables`, `instance_ref_to_variable`, `expand_object` |

### Notable Decisions/Tradeoffs

1. **`instance_ref_to_variable` visibility**: The task spec said `fn` (private), but existing tests in `mod.rs` call `adapter.instance_ref_to_variable()` directly. Since `mod tests` is a child of `adapter::mod`, not of `adapter::variables`, the method needs `pub(super)` visibility to be accessible from those tests. Made it `pub(super)` to preserve test coverage.

2. **Test-only imports**: `DapRequest` and `DapResponse` are used only in the test module of `mod.rs`. These were moved to a `#[cfg(test)] use crate::{DapRequest, DapResponse};` block to eliminate clippy's "unused imports" warning while keeping tests working.

3. **`MAX_VARIABLES_PER_REQUEST` in tests**: Moved from a module-level private import in `mod.rs` to an explicit `super::types::MAX_VARIABLES_PER_REQUEST` in the test module's import block — eliminates the "unused import" warning.

4. **Production code line count**: `mod.rs` production code is now 215 lines (struct definition + constructors + module declarations + re-exports), well within the ≤ 300 line limit.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-dap` - Passed (0 warnings)
- `cargo test -p fdemon-dap` - Passed (581 tests, 0 failed)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **`handle_scopes` still returns both Locals and Globals**: The note says "Globals was removed by task 13" but the current code in both the original and extracted version returns both scopes. The test `test_scopes_returns_locals_and_globals` asserts `len() == 2`. This matches the existing behavior exactly — the extraction is faithful to the source.
