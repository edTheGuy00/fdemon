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
