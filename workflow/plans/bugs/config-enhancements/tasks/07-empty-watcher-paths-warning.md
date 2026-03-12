## Task: Add Warning for Empty Watcher Paths

**Objective**: Emit a `warn!` log when `config.paths` is empty so users know the watcher won't trigger any reloads, instead of silently doing nothing.

**Depends on**: None

**Priority**: Should Fix

### Scope

- `crates/fdemon-app/src/watcher/mod.rs`: Add early warning in `run_watcher` when `config.paths` is empty

### Details

Currently if a user writes `paths = []` in config.toml, the watcher thread starts successfully, the debouncer is created, but zero paths are registered — no reloads will ever fire and no log message is emitted. This is technically correct but confusing for users debugging why their auto-reload isn't working.

Add a warning log before the `resolve_watch_paths` loop (around line 217) when the paths list is empty:

```rust
if config.paths.is_empty() {
    warn!("No watch paths configured — file watcher will not trigger reloads");
}
```

This should be a `warn!` (not `error!`) since it's a valid configuration, just likely unintentional. The watcher should still run (it may be needed for future dynamic path additions).

### Acceptance Criteria

1. `warn!` emitted when `config.paths` is empty
2. Watcher still starts normally (no early return — just the warning)
3. No warning emitted when paths are non-empty
4. Unit test verifying the code path (optional — the log itself is hard to test, but a comment documenting the behavior is sufficient)

### Testing

```bash
cargo test -p fdemon-app -- watcher
cargo clippy -p fdemon-app -- -D warnings
```

### Notes

- Do not add an early return — just the warning log. The watcher thread still needs to run for the stop-signal loop
- Consider whether `resolve_watch_paths` returning an empty vec (after filtering non-existent paths) should also trigger a similar warning — e.g., "All configured watch paths are invalid"

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/watcher/mod.rs` | Added two `warn!` calls in `run_watcher`: one before `resolve_watch_paths` when `config.paths` is empty, and one after when all configured paths resolve to non-existent entries. Added two new unit tests documenting both code paths. |

### Notable Decisions/Tradeoffs

1. **Secondary warning for all-invalid paths**: The task asked to "consider" this case. It was implemented because a user who configures `paths = ["wrong/dir"]` would get per-path warnings in the existing loop but no aggregate summary. The secondary `warn!` fires only when `config.paths` is non-empty AND every resolved path fails `exists()`, giving a clear signal before the per-path warnings.
2. **No early return**: Both warnings are purely advisory — the watcher continues to the stop-signal loop as required by the task spec.
3. **`warn` import was already present**: No import changes needed; `tracing::{debug, error, info, warn}` is on line 12.

### Testing Performed

- `cargo test -p fdemon-app -- watcher` - Passed (31 tests, including 2 new: `test_empty_paths_resolves_to_empty_vec` and `test_all_nonexistent_paths_none_exist`)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (clippy auto-formatted the multi-condition `if` guard; no warnings)

### Risks/Limitations

1. **Log assertion in unit tests**: The `warn!` calls themselves cannot be asserted without a tracing subscriber. The new tests exercise the same data invariants (empty paths → empty resolved vec; all-nonexistent paths → all fail `exists()`) and document the intent via comments. This matches the task's guidance that "a comment documenting the behavior is sufficient".
