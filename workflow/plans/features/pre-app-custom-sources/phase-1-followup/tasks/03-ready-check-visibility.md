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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/mod.rs` | Changed `pub mod ready_check` to `pub(super) mod ready_check` on line 25 |

### Notable Decisions/Tradeoffs

1. **Minimal change**: Only the visibility modifier was changed; no other code was touched. The existing `super::ready_check` access pattern in `native_logs.rs` is unaffected since `pub(super)` makes the module visible to the parent (`actions`) and all its children.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo check --workspace` - Passed (fdemon-app, fdemon-tui, flutter-demon all checked clean)
- `cargo test -p fdemon-app` - Passed (1,644 unit tests passed, 0 failed)

### Risks/Limitations

1. **None**: The change is purely a visibility restriction that brings `ready_check` in line with its sibling modules. No external consumers reference this module directly.
