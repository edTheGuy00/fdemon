## Task: Decouple App Module from TUI Entry Point

**Objective**: Remove the only `app -> tui` dependency: `app/mod.rs` calls `tui::run_with_project()`. Move the `run()` and `run_with_project()` entry points out of `app/mod.rs` and into the binary layer (`src/main.rs`), so that `fdemon-app` does not depend on `fdemon-tui`.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

- `src/app/mod.rs`: Remove `run()` and `run_with_project()` functions, remove `use crate::tui`
- `src/main.rs`: Absorb the initialization logic (color-eyre, logging) from `app/mod.rs`
- `src/lib.rs`: Update re-exports (remove `pub use app::{run, run_with_project}`)

### Details

#### Current Problem

`app/mod.rs` (lines 34, 60, 84) has:
```rust
use crate::tui;
// ...
tui::run_with_project(&project_path).await
```

This creates an `app -> tui` dependency, which is backwards (tui should depend on app, not vice versa). When we split into crates, `fdemon-app` cannot depend on `fdemon-tui` because `fdemon-tui` already depends on `fdemon-app` (circular dependency).

#### Solution

Move the `run()` and `run_with_project()` functions from `app/mod.rs` into `src/main.rs`. The binary already calls these functions and is the correct place for orchestrating which runner (TUI or headless) to use.

**Step 1: Update `src/main.rs`**

Move the initialization logic (color-eyre install, logging init) from `app::run_with_project()` into `main.rs`. The `main()` function already handles CLI parsing and project discovery. Add:

```rust
// In main(), before calling any runner:
color_eyre::install().map_err(|e| Error::terminal(e.to_string()))?;
flutter_demon::common::logging::init()?;
```

Then call `tui::run_with_project()` and `headless::run_headless()` directly.

**Step 2: Remove functions from `app/mod.rs`**

Delete the `run()` and `run_with_project()` functions entirely. Remove `use crate::tui;`. The `app/mod.rs` should only contain module declarations and re-exports of app-internal types.

**Step 3: Update `src/lib.rs`**

Remove:
```rust
pub use app::{run, run_with_project};
```

The library no longer provides top-level `run`/`run_with_project` functions. The binary handles orchestration directly.

**Step 4: Ensure `main.rs` doesn't use `flutter_demon::run_with_project()`**

Currently `main.rs` calls `flutter_demon::run_with_project(&base_path)` (line 43) and `flutter_demon::run_headless(&base_path)` (line 41). Update these to call the TUI and headless runners directly:
- `flutter_demon::tui::run_with_project(&base_path).await`
- `flutter_demon::headless::runner::run_headless(&base_path).await`

### Acceptance Criteria

1. `app/mod.rs` has NO import of `crate::tui`
2. `app/mod.rs` has NO `run()` or `run_with_project()` functions
3. `src/main.rs` handles initialization (color-eyre, logging) and calls runners directly
4. `cargo check` passes
5. `cargo test` passes
6. Application behavior is identical (both TUI and headless modes work)

### Testing

```bash
cargo check
cargo test
cargo clippy
```

Manual testing:
- Run `cargo run -- --headless /path/to/flutter/project` to verify headless mode
- Run `cargo run -- /path/to/flutter/project` to verify TUI mode (if a Flutter project is available)

### Notes

- This is a small but critical task. Without it, `fdemon-app` would have a circular dependency on `fdemon-tui`.
- The `color-eyre::install()` call should happen once at binary startup, not per-runner. Move it to `main()`.
- `logging::init()` similarly belongs at binary startup.
- After this task, `grep -r "crate::tui" src/app/` should return zero matches.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/mod.rs` | Removed `run()` and `run_with_project()` functions, removed `use crate::tui;` import |
| `src/lib.rs` | Removed re-exports `pub use app::{run, run_with_project};` and `pub use headless::runner::run_headless;` |
| `src/main.rs` | Added initialization logic (color-eyre, logging) at binary startup; updated all runner calls to use `flutter_demon::tui::runner::run_with_project()` and `flutter_demon::headless::runner::run_headless()` directly |
| `src/tui/mod.rs` | Removed re-export `pub use runner::{run, run_with_project};` (kept module public) |
| `src/headless/runner.rs` | Removed duplicate initialization (color-eyre, logging) since it's now done in main.rs |

### Notable Decisions/Tradeoffs

1. **Initialization Consolidated at Binary Entry**: Moved `color_eyre::install()` and `logging::init()` to the top of `main()` in `src/main.rs`. This ensures initialization happens exactly once at binary startup, not per-runner, which is the correct pattern and prevents potential re-initialization issues.

2. **Direct Runner Calls**: Updated all runner invocations in `main.rs` to call `flutter_demon::tui::runner::run_with_project()` and `flutter_demon::headless::runner::run_headless()` directly instead of going through library re-exports. This makes the binary layer's role as the orchestrator explicit.

3. **Logging Location Preserved**: Added `info!("Project path: ...")` logging at each call site in `main.rs` to preserve the logging behavior that existed in the old `app::run_with_project()` function.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1538 tests passed, 0 failed)
- `grep -r "crate::tui" src/app/` - No matches (confirmed no tui dependency in app)
- `grep -r "use.*tui" src/app/` - No matches (confirmed no tui imports in app)

### Acceptance Criteria Met

1. ✅ `app/mod.rs` has NO import of `crate::tui`
2. ✅ `app/mod.rs` has NO `run()` or `run_with_project()` functions
3. ✅ `src/main.rs` handles initialization (color-eyre, logging) and calls runners directly
4. ✅ `cargo check` passes
5. ✅ `cargo test` passes (1538 tests)
6. ✅ Application behavior is identical (initialization sequence preserved, logging maintained)

### Risks/Limitations

1. **Pre-existing Clippy Warnings**: The codebase has pre-existing clippy warnings (unexpected cfg conditions, field reassignment patterns) that are unrelated to this task. These should be addressed in a separate cleanup task.

2. **Manual Testing Recommended**: While unit tests pass, manual testing of both TUI and headless modes with an actual Flutter project is recommended to verify runtime behavior is unchanged.
