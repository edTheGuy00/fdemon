## Task: Split adapter/mod.rs into submodules

**Objective**: Decompose the 5,000+ line (9,122 with tests) `adapter/mod.rs` into focused submodules, each under the 800-line limit per `CODE_STANDARDS.md`.

**Depends on**: None

**Severity**: Critical (blocking merge)

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs` → split into:
  - `adapter/backend.rs` — **NEW**
  - `adapter/types.rs` — **NEW**
  - `adapter/handlers.rs` — **NEW**
  - `adapter/events.rs` — **NEW**
  - `adapter/variables.rs` — **NEW**
  - `adapter/mod.rs` — Slim facade with re-exports

### Details

Current section boundaries (from research):

| Section | Current Lines | Target File |
|---------|---------------|-------------|
| `LocalDebugBackend` trait | 55–209 | `backend.rs` |
| `DynDebugBackendInner` trait | 211–318 | `backend.rs` |
| `DynDebugBackend` struct + impl | 320–471 | `backend.rs` |
| `BackendError` | ~500 | `backend.rs` |
| `StepMode`, `BreakpointResult`, `DapExceptionPauseMode` | 473–531 | `types.rs` |
| `DebugEvent`, `PauseReason` enums | 533–641 | `types.rs` |
| `log_level_to_category` helper | 643–665 | `types.rs` |
| Constants | 667–722 | `types.rs` |
| `DapAdapter` struct definition | 724–810 | `mod.rs` |
| Constructor methods (`new`, `new_with_tx`) | 812–863 | `mod.rs` |
| `handle_request` dispatch | 865–903 | `handlers.rs` |
| `handle_debug_event` | 905–1317 | `events.rs` |
| `emit_output`, `interpolate_log_message`, `on_resume`, `on_hot_restart`, `send_event` | 1319–1443 | `events.rs` |
| Request handler methods | 1445–2748 | `handlers.rs` |
| Internal helpers | 2751–2835 | `handlers.rs` (or `mod.rs`) |
| Tests | 2837–9121 | Move to per-submodule `#[cfg(test)]` blocks or a `tests/` subdir |

**Estimated file sizes after split:**

| File | Estimated Lines | Content |
|------|----------------|---------|
| `mod.rs` | ~150 | `DapAdapter` struct, constructors, `pub mod` + re-exports |
| `backend.rs` | ~450 | `LocalDebugBackend`, `DynDebugBackendInner`, `DynDebugBackend`, `BackendError` |
| `types.rs` | ~250 | Enums, constants, helper functions |
| `events.rs` | ~550 | `handle_debug_event`, `emit_output`, `interpolate_log_message`, `on_resume`, `on_hot_restart`, `send_event` |
| `handlers.rs` | ~800 | `handle_request` dispatch + all `handle_*` methods + internal helpers |
| `variables.rs` | ~300 | `get_scope_variables`, `expand_object`, `instance_ref_to_variable` |

**Test strategy:** Tests (~6,280 lines) should be split to accompany their respective modules. Each submodule gets its own `#[cfg(test)] mod tests` block with relevant tests. Mock backends can remain in a shared `tests` submodule or in `mod.rs`'s test section.

### Implementation Steps

1. Create `backend.rs` — move `LocalDebugBackend`, `DynDebugBackendInner`, `DynDebugBackend`, `BackendError`
2. Create `types.rs` — move `StepMode`, `BreakpointResult`, `DapExceptionPauseMode`, `DebugEvent`, `PauseReason`, constants, helpers
3. Create `events.rs` — move `handle_debug_event` and associated methods as `impl<B: DebugBackend> DapAdapter<B>` methods
4. Create `variables.rs` — move scope/variable methods
5. Create `handlers.rs` — move request dispatch and handler methods
6. Slim `mod.rs` — struct definition, constructors, `pub mod` declarations, re-exports
7. Distribute tests to their respective submodule files
8. Verify all public API re-exports remain accessible from `fdemon_dap::adapter::*`

### Acceptance Criteria

1. No file in `crates/fdemon-dap/src/adapter/` exceeds 800 lines (excluding inline test blocks)
2. All 561+ existing adapter tests pass: `cargo test -p fdemon-dap`
3. Public API unchanged — all types re-exported from `adapter::*`
4. `cargo check --workspace` — Pass
5. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```bash
cargo test -p fdemon-dap              # All adapter tests pass
cargo check --workspace               # No broken imports anywhere
cargo clippy --workspace -- -D warnings  # No new warnings
```

### Notes

- This is a pure refactor — no behavioral changes
- The test section alone is 6,280 lines; splitting tests is the bulk of the work
- Use `pub(crate)` for internal methods that other adapter submodules need but external consumers don't
- `DapAdapter` struct fields may need `pub(super)` visibility for submodule methods to access them
- Consider whether some `impl DapAdapter` blocks need to import types from sibling submodules
