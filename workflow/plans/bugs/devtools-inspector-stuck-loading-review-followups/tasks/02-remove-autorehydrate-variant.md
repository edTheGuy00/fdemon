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

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Removed `AutoRehydrate` variant and its doc-comment from `FetchTrigger` enum (lines 91-94). Enum now has only `Initial` and `Refresh`. |

### Notable Decisions/Tradeoffs

1. **Only one file required changes**: `git grep "AutoRehydrate" crates/` confirmed the variant appeared only in `handler/mod.rs`. `actions/inspector/mod.rs` and `lib.rs` required no changes — the poll-skip guard `if trigger != FetchTrigger::Refresh` remains semantically correct, and the re-export of `FetchTrigger` is unaffected.
2. **Remaining `AutoRehydrate` references are in docs/workflow only**: The `docs/ARCHITECTURE.md` mention is handled by task 03 (doc_maintainer agent). Workflow and review files reference it historically and do not need cleanup.

### Testing Performed

- `git grep "AutoRehydrate" crates/` — no matches (variant fully eliminated from source)
- `cargo check --workspace --all-targets` — Passed (clean build, 0 errors/warnings)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)
- `cargo test --workspace` — Passed (2190+ tests, 0 failures)
- Three named tests confirmed passing: `refresh_after_render_uses_refresh_trigger`, `refresh_before_first_render_uses_initial_trigger`, `switch_panel_inspector_uses_initial_trigger`

### Risks/Limitations

1. **ARCHITECTURE.md still mentions AutoRehydrate**: This is intentionally deferred to task 03 (doc_maintainer). The code is clean; the doc lag is tracked.
