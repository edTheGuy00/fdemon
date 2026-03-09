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

**Status:** Not Started
