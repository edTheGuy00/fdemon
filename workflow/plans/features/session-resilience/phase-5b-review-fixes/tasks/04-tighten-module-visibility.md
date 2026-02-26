## Task: Tighten Module Visibility for `session` Submodule

**Objective**: Change the `session` submodule from `pub` to `pub(super)` for consistency with all other submodules, and narrow the `execute_task` re-export and function visibility accordingly.

**Depends on**: None

**Review Issue**: #4 (Minor — 3 review agents flagged this)

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Lines 18-19 — change `pub mod session` and `pub use` to `pub(super)`
- `crates/fdemon-app/src/actions/session.rs`: Line 252 — change `pub async fn execute_task` to `pub(super)`

### Details

**Current state at `mod.rs:18-24`:**
```rust
pub mod session;                   // ← pub (inconsistent with siblings)
pub use session::execute_task;     // ← pub re-export

pub(super) mod inspector;
pub(super) mod network;
pub(super) mod performance;
pub(super) mod vm_service;
```

The `session` module is the only submodule with `pub` visibility. Research confirms:
- `execute_task` is only called within `mod.rs` itself (lines 45, 55)
- No external callers exist anywhere in the workspace
- The parent `actions` module is already `pub(crate)` in `lib.rs:60`, so `pub` is already crate-bounded
- The `pub use session::execute_task` re-export is never consumed from outside the module

**After fix:**
```rust
pub(super) mod session;
pub(super) use session::execute_task;

pub(super) mod inspector;
pub(super) mod network;
pub(super) mod performance;
pub(super) mod vm_service;
```

**And in `session.rs:252`:**
```rust
// Before:
pub async fn execute_task(
// After:
pub(super) async fn execute_task(
```

### Acceptance Criteria

1. All 5 submodule declarations in `mod.rs` use `pub(super)` visibility
2. The `execute_task` re-export uses `pub(super) use`
3. `execute_task` function in `session.rs` uses `pub(super)` visibility
4. `cargo check --workspace` passes (no broken references)
5. `cargo test -p fdemon-app` passes

### Testing

No new tests needed — this is a visibility restriction. If any external code depended on the `pub` visibility, `cargo check` would fail with a compilation error.

### Notes

- The `SessionTaskMap` type alias and `handle_action` function are defined directly in `mod.rs` (not re-exported from a submodule), so their visibility is unaffected.
- `lib.rs:94` re-exports `actions::SessionTaskMap` — this is defined in `mod.rs` and is unaffected by submodule visibility changes.
