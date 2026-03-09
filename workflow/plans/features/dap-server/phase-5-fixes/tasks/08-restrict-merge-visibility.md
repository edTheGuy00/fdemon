## Task: Restrict Internal Merge Utilities to `pub(crate)`

**Objective**: Change the public re-exports of internal merge utilities from `pub` to `pub(crate)`. These are implementation details, not public API.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/ide_config/mod.rs`: Change `pub use merge::{...}` to `pub(crate) use merge::{...}` at lines ~254-257
- `crates/fdemon-app/src/ide_config/merge.rs`: Change `pub fn clean_jsonc` to `pub(crate) fn clean_jsonc` (and any other `pub` items that should be `pub(crate)`)

### Details

**Current re-export block** (`ide_config/mod.rs:254-257`):
```rust
pub use merge::{
    clean_jsonc, find_json_entry_by_field, merge_json_array_entry, to_pretty_json,
    FDEMON_CONFIG_NAME, FDEMON_MARKER_FIELD,
};
```

These utilities are used within `fdemon-app` by:
- `ide_config/vscode.rs` — `clean_jsonc`, `merge_json_array_entry`, etc.
- `config/vscode.rs` — `clean_jsonc` (after Task 02 dedup)

No crate outside `fdemon-app` uses these. They should be `pub(crate)`.

**Fix:**
```rust
pub(crate) use merge::{
    clean_jsonc, find_json_entry_by_field, merge_json_array_entry, to_pretty_json,
    FDEMON_CONFIG_NAME, FDEMON_MARKER_FIELD,
};
```

Also check `merge.rs` itself — if `clean_jsonc` is `pub fn`, change to `pub(crate) fn` since the re-export handles cross-module access within the crate.

### Acceptance Criteria

1. No `pub` re-exports of internal merge utilities
2. `cargo check --workspace` — Pass (no external crate depends on these)
3. `cargo test --workspace` — Pass

### Testing

- Compile check is sufficient. If an external crate used these, `cargo check` would fail.

### Notes

- Coordinate with Task 02 (deduplicate JSONC) — after dedup, `config/vscode.rs` imports `clean_jsonc` via `crate::ide_config::clean_jsonc`. This import path works with `pub(crate)`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/merge.rs` | Changed all `pub const` / `pub fn` items to `pub(crate)`. Added `#[allow(dead_code)]` with explanatory doc comment to `FDEMON_MARKER_FIELD` (protocol constant unused in production code, only referenced in tests). |
| `crates/fdemon-app/src/ide_config/mod.rs` | Changed `pub mod merge` to `pub(crate) mod merge`. Trimmed the re-export from the full 6-item list to only `pub(crate) use merge::{merge_json_array_entry, to_pretty_json}` — the 2 items actually consumed via `super::` by `zed.rs`. Updated the test block to import `clean_jsonc` and `find_json_entry_by_field` directly from `merge` instead of through the now-trimmed re-export. |

### Notable Decisions/Tradeoffs

1. **Re-export trimming**: Changing from `pub` to `pub(crate)` caused `unused_imports` warnings for 4 of the 6 re-exported items because `vscode.rs` imports directly from `super::merge::` (bypassing the `super::` re-export) and the other items had no consumers through `super::`. Rather than add `#[allow(unused_imports)]`, the re-export was trimmed to only the 2 items (`merge_json_array_entry`, `to_pretty_json`) actually used by `zed.rs` via `super::`. This is strictly more correct.

2. **`FDEMON_MARKER_FIELD` dead_code**: Reducing visibility from `pub` to `pub(crate)` caused a `dead_code` lint because the constant is only used in tests. Added `#[allow(dead_code)]` with doc comment explaining it is an intentional protocol constant reserved for future generators.

3. **`pub(crate) mod merge`**: The `merge` submodule declaration was also restricted to `pub(crate)` since no external crate accesses `fdemon_app::ide_config::merge` directly.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (0 warnings)
- `cargo test --workspace` - Passed (3,769 tests: 0 failed, 66 ignored)
- `cargo clippy --workspace -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **Task 02 coordination**: The task notes `config/vscode.rs` may import `clean_jsonc` via `crate::ide_config::clean_jsonc` after Task 02 dedup. That path still works since `clean_jsonc` remains in the `pub(crate)` re-export... but the re-export was trimmed and `clean_jsonc` is no longer in it. If Task 02 adds that import path, the re-export will need to be extended with `clean_jsonc`. Alternatively, Task 02 can import directly via `crate::ide_config::merge::clean_jsonc`.
