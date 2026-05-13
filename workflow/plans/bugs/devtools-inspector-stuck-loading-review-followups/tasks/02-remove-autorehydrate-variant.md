## Task: Remove `FetchTrigger::AutoRehydrate` Variant (YAGNI)

**Objective**: Eliminate the unused `FetchTrigger::AutoRehydrate` variant. It has no construction sites, its doc-comment contradicts ARCHITECTURE.md, and removing it now prevents future contributors from being misled by the inconsistency. Reintroduce in the same PR that adds its first caller.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mod.rs` — remove `AutoRehydrate` variant from `FetchTrigger`
- `crates/fdemon-app/src/actions/inspector/mod.rs` — confirm the poll-skip guard still reads correctly (`if trigger != FetchTrigger::Refresh` remains valid with two variants)
- `crates/fdemon-app/src/lib.rs` — re-export still references `FetchTrigger` (no change needed unless task 10 has already landed)

**Files Read (Dependencies):**
- None

### Details

**Current state (`handler/mod.rs:76-95`):**
```rust
pub enum FetchTrigger {
    /// First fetch ... full poll budget applies.
    Initial,
    /// User pressed `r` ... poll is **skipped**.
    Refresh,
    /// Programmatic re-fetch (e.g., after a focused-panel change).
    /// Uses the full poll budget for safety, same as `Initial`.
    AutoRehydrate,
}
```

**Target state:**
```rust
pub enum FetchTrigger {
    /// First fetch ... full poll budget applies.
    Initial,
    /// User pressed `r` ... poll is **skipped**.
    Refresh,
}
```

Confirm no other call site references `AutoRehydrate`:
```bash
git grep "AutoRehydrate" crates/
```

The poll-skip guard at `actions/inspector/mod.rs:93` is `if trigger != FetchTrigger::Refresh`, which remains semantically correct with two variants (Initial → poll; Refresh → skip).

### Acceptance Criteria

1. `FetchTrigger::AutoRehydrate` does not exist anywhere in the codebase (verified by `git grep AutoRehydrate` returning no source matches).
2. The poll-skip guard in `actions/inspector/mod.rs` still reads correctly.
3. No test in the workspace references `AutoRehydrate`.
4. `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` all pass.
5. Existing tests that reference `FetchTrigger::Initial` or `FetchTrigger::Refresh` continue to pass without change.

### Testing

No new tests needed. Verify existing tests still cover both surviving variants:
- `refresh_after_render_uses_refresh_trigger`
- `refresh_before_first_render_uses_initial_trigger`
- `switch_panel_inspector_uses_initial_trigger`

### Notes

- The ARCHITECTURE.md mention of `AutoRehydrate` is cleaned up in task 03 (doc_maintainer). Code changes are independent.
- If future work needs a third trigger variant, the type system will guide the reintroduction. `cargo check` will flag every site that needs an updated match arm.
- Per user direction: YAGNI removal is preferred over "make AutoRehydrate skip the poll".
