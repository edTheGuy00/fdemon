## Task: Watcher Path Resolution Tests

**Objective**: Add unit tests covering all watcher path resolution scenarios, including relative paths with `../`, absolute paths, non-existent paths, and default behavior.

**Depends on**: 01-fix-watcher-paths

### Scope

- `crates/fdemon-app/src/watcher/mod.rs`: Add tests for path resolution in `run_watcher`
- `crates/fdemon-app/src/engine.rs`: Add tests verifying settings are passed to WatcherConfig

### Details

**Test cases for path resolution (watcher/mod.rs):**

1. **Default paths** — When no custom paths configured, `["lib"]` is watched relative to project root
2. **Single relative path** — `"lib"` resolves to `{project_root}/lib`
3. **Parent-relative path** — `"../../shared"` resolves to the canonicalized path two directories up
4. **Multiple relative paths** — `["lib", "../common/lib", "test"]` all resolve correctly
5. **Absolute path** — `/tmp/shared_lib` is used as-is (not joined with project_root)
6. **Mixed absolute and relative** — Both types in the same config work correctly
7. **Non-existent path** — Produces a warning, doesn't crash the watcher
8. **Empty paths list** — No directories watched, watcher still starts without error

**Test cases for settings pass-through (engine.rs):**

1. **Custom paths passed** — `settings.watcher.paths` values appear in the `WatcherConfig` used by `FileWatcher`
2. **Custom extensions passed** — `settings.watcher.extensions` values appear in the `WatcherConfig`
3. **Default settings** — When no custom config, default `["lib"]` and `["dart"]` are used

### Acceptance Criteria

1. All path resolution scenarios have passing tests
2. Tests use `tempdir()` for filesystem-based tests (no real project dependency)
3. Tests verify both the path joining logic and canonicalization
4. Tests cover the `is_absolute()` branching logic
5. No regressions in existing watcher tests

### Testing

```bash
cargo test -p fdemon-app -- watcher
cargo test -p fdemon-app -- engine
```

### Notes

- Use `tempfile::tempdir()` to create realistic directory structures for testing
- For canonicalization tests, create actual directories so `canonicalize()` succeeds
- For non-existent path tests, use a path within the tempdir that hasn't been created

---

## Completion Summary

**Status:** Not Started
