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
