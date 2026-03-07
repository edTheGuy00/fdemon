## Task: Consolidate duplicate mock backends in tests

**Objective**: Reduce test boilerplate by consolidating 6 full-boilerplate mock backend structs that each repeat 14-17 identical `DebugBackend` trait method implementations.

**Depends on**: 02-split-adapter-mod

**Severity**: Minor

### Scope

- `crates/fdemon-dap/src/adapter/` test modules (post-split)

### Details

**Current state — 6 top-level mock backends with massive duplication:**

| Mock | Purpose | Unique Methods |
|------|---------|----------------|
| `MockBackend` | General no-op | 0 (all default) |
| `MockBackendWithUri` | Returns known ws_uri/device_id/mode | 3 |
| `AttachMockBackend` | `get_vm()` returns 2 named isolates | 1 |
| `FailingVmBackend` | All calls return `Err` | ~17 (all override) |
| `StackMockBackend` | `get_stack()` returns 3-frame stack | 1 |
| `VarMockBackend` | `get_stack()`/`get_object()` return test data | 2 |

Plus ~8 inline test-function mocks: `ErrorEvalBackend`, `TrackingBackend`, `CondMockBackend`, `LogpointMockBackend`, `HotOpMockBackend`, etc.

**Proposed approach — `DefaultMockBackend` + selective override:**

Option A: **Macro-based approach**
```rust
macro_rules! mock_backend {
    ($name:ident { $($method:item)* }) => {
        struct $name;
        #[async_trait]
        impl DebugBackend for $name {
            // Default implementations for all 17 methods
            // ...
            $($method)*  // Override specific methods
        }
    };
}
```

Option B: **Delegation via inner struct**
```rust
struct DefaultMockBackend;
// Full default implementation

struct StackMockBackend {
    inner: DefaultMockBackend,
}
// Only override get_stack(); delegate rest to inner
```

Option C: **Provide default method implementations on a test trait** (if Rust's async_trait allows)

**Recommendation:** Option A (macro) is most idiomatic for Rust test code and eliminates the most boilerplate.

### Acceptance Criteria

1. No two mock backends share >3 identical method implementations
2. Adding a new `DebugBackend` method requires updating only one place (the macro/base), not 6+ structs
3. All existing tests pass unchanged
4. `cargo test -p fdemon-dap` — Pass

### Notes

- This is the largest cleanup task by line count (~2,000+ lines of duplicated boilerplate)
- Do NOT change test behavior — only reduce boilerplate
- The macro should be defined in a `#[cfg(test)]` module so it doesn't appear in production builds
- Consider placing shared test infrastructure in `adapter/test_helpers.rs` or similar

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/test_helpers.rs` | Created: `MockTestBackend` trait with `fn ... -> impl Future + Send` default implementations for all 18 `DebugBackend` methods, plus blanket `impl<T: MockTestBackend> DebugBackend for T` |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `#[cfg(test)] pub(crate) mod test_helpers;` declaration; converted all 14 test mock backends from full 18-method `impl DebugBackend` blocks to minimal `impl MockTestBackend` overrides; 9121 lines → 8025 lines (1096 lines removed) |

### Backends Converted

| Mock | Before (methods) | After (unique overrides) |
|------|-----------------|--------------------------|
| `MockBackend` | 18 | 1 (`get_source`) |
| `MockBackendWithUri` | 18 | 3 (`ws_uri`, `device_id`, `build_mode`) |
| `AttachMockBackend` | 18 | 1 (`get_vm`) |
| `FailingVmBackend` | 18 | 10 (all failure methods) |
| `StackMockBackend` | 18 | 1 (`get_stack`) |
| `VarMockBackend` | 18 | 2 (`get_stack`, `get_object`) |
| `NotConnectedBackend` | 18 | 14 (all `NotConnected` error methods) |
| `HotOpMockBackend` | 18 | 2 (`hot_reload`, `hot_restart`) |
| `CondMockBackend` | 18 | 3 (`resume`, `evaluate_in_frame`, `get_vm`) |
| `LogpointMockBackend` | 18 | 3 (`resume`, `evaluate_in_frame`, `get_vm`) |
| `ErrorEvalBackend` | 18 | 3 (`add_breakpoint`, `evaluate_in_frame`, `get_vm`) |
| `TrackingBackend` | 18 | 2 (`resume`, `add_breakpoint`) |
| `StopTrackingBackend` | 18 | 2 (`add_breakpoint`, `stop_app`) |
| `StopTrackingBackend2` | 18 | 2 (`add_breakpoint`, `stop_app`) |

### Notable Decisions/Tradeoffs

1. **`fn ... -> impl Future + Send` defaults instead of `async fn`**: The `MockTestBackend` trait defaults use `std::future::ready(...)` returning `impl Future + Send` rather than `async fn`. This is required because the blanket `DebugBackend` impl awaits these futures and `DebugBackend` (from `trait_variant::make`) requires `Send` futures. Plain `async fn` in trait defaults does not automatically produce `Send` futures.

2. **`async fn` for overrides still works**: Override impls in tests use `async fn` (ergonomic), which produces `Send` futures because all test struct fields are `Arc<Mutex<...>>` / `serde_json::Value` (all `Send + Sync`). Rust accepts this since the concrete future type does implement `Send`.

3. **Blanket impl approach over macro**: Chosen over the task's recommended macro approach because `macro_rules!` cannot conditionally override trait methods — duplicate method names in an `impl` block cause a compile error. The blanket trait impl achieves the same "update one place" goal without macro complexity.

4. **`FailingVmBackend` and `NotConnectedBackend` still have many overrides**: These backends are inherently non-default (they return errors for most methods), so they still need ~10-14 overrides. This is semantically correct — they are not "mostly default" mocks.

### Testing Performed

- `cargo check -p fdemon-dap` — Passed
- `cargo test -p fdemon-dap` — Passed (581 tests, 0 failed)
- `cargo clippy -p fdemon-dap -- -D warnings` — Passed (production code clean)
- `cargo fmt --all` — Passed

### Risks/Limitations

1. **Pre-existing clippy failures in `--tests` mode**: `cargo clippy -p fdemon-dap --tests -- -D warnings` fails on 5 `manual_range_contains` warnings in `threads.rs`. These pre-exist this task (threads.rs was not modified) and are out of scope.

2. **Pre-existing `fdemon-app` compile errors**: `cargo check --workspace` fails on `fdemon-app/src/handler/devtools/debug.rs` with `UpdateAction` import errors. These are pre-existing from other tasks and unrelated to mock backend consolidation.
