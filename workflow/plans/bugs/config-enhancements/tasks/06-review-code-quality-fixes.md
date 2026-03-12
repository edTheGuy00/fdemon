## Task: Review Code Quality Fixes

**Objective**: Fix 3 code quality issues flagged as "Must Fix" in the review — unnecessary clone, stale doc comment, and hardcoded test paths.

**Depends on**: None

**Priority**: Must Fix (blocks merge)

### Scope

- `crates/fdemon-tui/src/startup.rs`: Remove unnecessary clone, replace hardcoded test paths
- `crates/fdemon-app/src/watcher/mod.rs`: Update stale module doc comment

### Details

#### Fix 1: Remove unnecessary `configs.clone()` (startup.rs:44)

The `Ready` branch calls `state.show_new_session_dialog(configs.clone())` but `configs` is never used after this line — it is dropped when `StartupAction::Ready` is returned. The `AutoStart` branch already moves `configs` without cloning. `LoadedConfigs` contains a `Vec<SourcedConfig>` so the clone involves heap allocation.

```rust
// Before:
state.show_new_session_dialog(configs.clone());

// After:
state.show_new_session_dialog(configs);
```

#### Fix 2: Update stale module doc (watcher/mod.rs:1-4)

The module doc comment says "Watches the `lib/` directory" which is no longer accurate after the configurable paths fix. Update to reflect multi-path support.

```rust
// Before:
//! File watcher module for auto-reload functionality
//!
//! Watches the `lib/` directory for Dart file changes and triggers
//! automatic hot reload with debouncing.

// After:
//! File watcher module for auto-reload functionality
//!
//! Watches one or more configured directories for Dart file changes
//! and triggers automatic hot reload with debouncing.
```

#### Fix 3: Replace hardcoded `/tmp/test` in tests (startup.rs:61,75)

Two tests use `Path::new("/tmp/test")` instead of `tempfile::tempdir()`. The other 5 tests in the same file already use `tempdir()`. `/tmp/test` is non-portable and inconsistent with project testing standards (see `docs/CODE_STANDARDS.md`: "Use `tempdir()` for file-based tests").

```rust
// Before:
let project_path = Path::new("/tmp/test");

// After:
let dir = tempfile::tempdir().unwrap();
let project_path = dir.path();
```

### Acceptance Criteria

1. `configs.clone()` replaced with `configs` move on the Ready path
2. Watcher module doc accurately describes configurable multi-path watching
3. Both test functions use `tempfile::tempdir()` instead of `/tmp/test`
4. `cargo test -p fdemon-tui -- startup` passes
5. `cargo clippy --workspace -- -D warnings` clean

### Testing

Run:
```bash
cargo test -p fdemon-tui -- startup
cargo clippy --workspace -- -D warnings
```

### Notes

- All 3 fixes are single-line changes — this task should be very quick
- The clone fix may also be caught by clippy in future versions as a redundant clone warning
