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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/watcher/mod.rs` | Extracted path resolution into `pub(crate) fn resolve_watch_paths()`, updated `run_watcher` to use it, added 8 new unit tests covering all path resolution scenarios |
| `crates/fdemon-app/src/engine.rs` | Added 3 new tests verifying `WatcherConfig` construction from `Settings` defaults and custom paths/extensions |

### Notable Decisions/Tradeoffs

1. **Extracted `resolve_watch_paths` helper**: The path resolution logic was embedded in `run_watcher` (a blocking fn tied to the notify debouncer). Extracting it as a `pub(crate)` pure function makes it directly testable without starting an actual file system watcher. This also makes the logic more readable in `run_watcher` itself.

2. **`test_resolve_parent_relative_path` uses `../shared` not `../../shared`**: The original task description mentioned `../../shared` as an example, but the correct relative path depends on the directory structure. Using `root/project` + `../shared` is cleaner and unambiguous. Also, on macOS, `tempfile::tempdir()` returns a `/var/...` path (non-canonical); the test canonicalizes `project_root` before calling `resolve_watch_paths` to ensure the `..` traversal resolves correctly via `realpath(3)`.

3. **Engine tests mirror `start_file_watcher` logic**: Since `file_watcher` is a private field, engine tests verify the settings-to-WatcherConfig mapping by reproducing the same builder chain used in `start_file_watcher`. This is a white-box test of the mapping logic, not an integration test of the watcher itself.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app -- watcher` - Passed (29 tests, 8 new)
- `cargo test -p fdemon-app -- engine` - Passed (29 tests, 3 new + 1 existing engine default settings test)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Pre-existing snapshot failures**: 4 snapshot tests in `fdemon-tui` fail due to version string mismatch (`v0.1.0` vs `v0.2.1`) — these failures pre-date this task and are unrelated to watcher changes.
2. **macOS-specific `..` traversal**: `canonicalize()` on macOS resolves `..` via the kernel (not string manipulation), which requires the parent directories to exist at call time. The test accounts for this by canonicalizing `project_root` first.
