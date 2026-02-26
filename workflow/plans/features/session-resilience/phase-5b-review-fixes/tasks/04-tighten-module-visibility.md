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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/mod.rs` | Changed `pub mod session` to `pub(super) mod session`; removed the `pub use session::execute_task` re-export line; updated two call sites from bare `execute_task(...)` to `session::execute_task(...)` |
| `crates/fdemon-app/src/actions/session.rs` | Changed `pub async fn execute_task` to `pub(super) async fn execute_task` |

### Notable Decisions/Tradeoffs

1. **Re-export removed instead of narrowed**: The task plan specified `pub(super) use session::execute_task` as the re-export, but Rust prohibits re-exporting an item beyond its own visibility (`pub(super)` in `session.rs` restricts to the `actions` module; a re-export in `mod.rs` would expose it to `actions`'s parent). The correct fix was to drop the re-export entirely and qualify the two call sites as `session::execute_task(...)`. This achieves the same encapsulation goal — no external callers — with valid Rust. Acceptance criterion 2 ("re-export uses `pub(super) use`") is met in spirit: the re-export is gone, which is strictly narrower than `pub(super) use`.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo check --workspace` - Passed (no broken references across any crate)
- `cargo test -p fdemon-app` - Passed (1161 unit tests + 1 doc test, 0 failures)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Passed (no formatting changes needed)

### Risks/Limitations

1. **None**: This is a pure visibility restriction with no behavioural change. The `cargo check --workspace` pass confirms no external callers depended on the wider visibility.
