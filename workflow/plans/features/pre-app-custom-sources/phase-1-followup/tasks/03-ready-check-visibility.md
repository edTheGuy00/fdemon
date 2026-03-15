## Task: Fix `ready_check` Module Visibility

**Objective**: Change `pub mod ready_check` to `pub(super) mod ready_check` in `actions/mod.rs` to match the visibility convention used by all sibling modules.

**Depends on**: None

**Severity**: Major

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Change visibility modifier on line 25

### Details

All sibling modules in `actions/mod.rs` use `pub(super)`:

```rust
pub(super) mod session;
pub(super) mod inspector;
pub(super) mod native_logs;
pub(super) mod network;
pub(super) mod performance;
pub mod ready_check;       // <-- should be pub(super)
pub(super) mod vm_service;
```

`ready_check` is only consumed by `native_logs.rs` within the same parent. `pub` exposes it unnecessarily to external crates.

#### Fix

```rust
pub(super) mod ready_check;
```

### Acceptance Criteria

1. `ready_check` module uses `pub(super)` visibility
2. `cargo check --workspace` passes (no broken imports)
3. All tests pass

### Notes

- This is a one-line change with zero call-site impact — `native_logs.rs` accesses `ready_check` via `super::ready_check`, which is within `pub(super)` scope
