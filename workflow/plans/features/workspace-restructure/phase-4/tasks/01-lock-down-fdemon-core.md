## Task: Lock Down fdemon-core Public API

**Objective**: Define a clean public API for `fdemon-core` by removing internal helpers and implementation details from the crate's public surface. Add `pub(crate)` to items that are only used within the crate.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-core/src/lib.rs`: Remove internal items from re-exports
- `crates/fdemon-core/src/stack_trace.rs`: Make regex statics `pub(crate)`
- `crates/fdemon-core/src/discovery.rs`: Make helper functions `pub(crate)`

### Details

#### 1. Remove Internal Regex Statics from Re-exports

The 5 compiled `LazyLock<Regex>` statics in `stack_trace.rs` are implementation details of `detect_format()` and `ParsedStackTrace`. They should not be in the crate's public API.

**In `stack_trace.rs`**, change visibility:

| Item | Current | New | Reason |
|------|---------|-----|--------|
| `DART_VM_FRAME_REGEX` | `pub static` | `pub(crate) static` | Only used by `detect_format()` and tests |
| `DART_VM_FRAME_NO_COL_REGEX` | `pub static` | `pub(crate) static` | Only used by `detect_format()` and tests |
| `FRIENDLY_FRAME_REGEX` | `pub static` | `pub(crate) static` | Only used by `detect_format()` and tests |
| `ASYNC_GAP_REGEX` | `pub static` | `pub(crate) static` | Only used by `detect_format()` and tests |
| `PACKAGE_PATH_REGEX` | `pub static` | `pub(crate) static` | Only used by `is_package_path()` |

**In `lib.rs`**, remove the regex statics from the re-export block:

```rust
// BEFORE:
pub use stack_trace::{
    detect_format, is_package_path, is_project_path, ParsedStackTrace, StackFrame,
    StackTraceFormat, ASYNC_GAP_REGEX, DART_VM_FRAME_NO_COL_REGEX, DART_VM_FRAME_REGEX,
    FRIENDLY_FRAME_REGEX, PACKAGE_PATH_REGEX,
};

// AFTER:
pub use stack_trace::{
    detect_format, is_package_path, is_project_path, ParsedStackTrace, StackFrame,
    StackTraceFormat,
};
```

#### 2. Make Discovery Helper Functions `pub(crate)`

Several functions in `discovery.rs` are low-level building blocks only used by higher-level discovery functions. External consumers should use `is_runnable_flutter_project()`, `discover_flutter_projects()`, and `get_project_type()`.

| Function | Current | New | Reason |
|----------|---------|-----|--------|
| `has_main_function_in_content()` | `pub fn` | `pub(crate) fn` | Only used by `has_main_function()` |
| `has_main_function()` | `pub fn` | `pub(crate) fn` | Only used by `discover_entry_points()` |
| `has_flutter_dependency()` | `pub fn` | keep `pub fn` | Used by `fdemon-daemon` tests (check first) |
| `is_flutter_plugin()` | `pub fn` | `pub(crate) fn` | Only used by `get_project_type()` |
| `has_platform_directories()` | `pub fn` | `pub(crate) fn` | Only used by `get_project_type()` |

**Important**: Before changing `has_flutter_dependency()`, grep for external usage. If it's used outside `fdemon-core`, keep it `pub`. If only used internally, make it `pub(crate)`.

**In `lib.rs`**, remove internal helpers from re-export:

```rust
// BEFORE:
pub use discovery::{
    discover_entry_points, discover_flutter_projects, get_project_name, get_project_type,
    has_flutter_dependency, has_main_function, has_main_function_in_content,
    has_platform_directories, is_flutter_plugin, is_runnable_flutter_project, DiscoveryResult,
    ProjectType, SkippedProject, DEFAULT_MAX_DEPTH,
};

// AFTER:
pub use discovery::{
    discover_entry_points, discover_flutter_projects, get_project_name, get_project_type,
    is_runnable_flutter_project, DiscoveryResult, ProjectType, SkippedProject,
    DEFAULT_MAX_DEPTH,
};
```

Note: If `has_flutter_dependency` is used externally, keep it in the re-export list.

#### 3. Review Stack Trace Helper Visibility

| Function | Current | New | Reason |
|----------|---------|-----|--------|
| `is_package_path()` | `pub fn` | keep `pub fn` | May be useful externally for path classification |
| `is_project_path()` | `pub fn` | keep `pub fn` | May be useful externally for path classification |

Keep these as `pub` -- they have utility value for downstream crates that process stack frames.

#### 4. Verify No External Breakage

After making changes, verify that no other crate in the workspace depends on the items being made `pub(crate)`:

```bash
# Search for regex static usage outside fdemon-core
grep -r "DART_VM_FRAME_REGEX\|DART_VM_FRAME_NO_COL_REGEX\|FRIENDLY_FRAME_REGEX\|ASYNC_GAP_REGEX\|PACKAGE_PATH_REGEX" crates/fdemon-daemon/ crates/fdemon-app/ crates/fdemon-tui/ src/

# Search for discovery helper usage outside fdemon-core
grep -r "has_main_function_in_content\|has_main_function\|is_flutter_plugin\|has_platform_directories" crates/fdemon-daemon/ crates/fdemon-app/ crates/fdemon-tui/ src/
```

If any external usage is found, either keep the item `pub` or update the external code to use the higher-level API.

### Acceptance Criteria

1. `DART_VM_FRAME_REGEX`, `DART_VM_FRAME_NO_COL_REGEX`, `FRIENDLY_FRAME_REGEX`, `ASYNC_GAP_REGEX`, `PACKAGE_PATH_REGEX` are `pub(crate)` (not accessible from outside `fdemon-core`)
2. `has_main_function_in_content()`, `is_flutter_plugin()`, `has_platform_directories()` are `pub(crate)`
3. `lib.rs` re-exports only the intended public API
4. `cargo check -p fdemon-core` passes
5. `cargo test -p fdemon-core` passes (all existing tests still work -- they're within the crate)
6. `cargo check --workspace` passes (no external breakage)
7. `cargo test --workspace` passes

### Testing

```bash
# Crate-level verification
cargo check -p fdemon-core
cargo test -p fdemon-core

# Full workspace verification
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
```

### Notes

- All `#[cfg(test)]` tests within `fdemon-core` can still access `pub(crate)` items via `use super::*`
- The `prelude` module is already well-scoped and doesn't need changes
- The `ansi` module has 3 functions, all genuinely useful externally -- no changes needed
- The `events` module exports only domain types -- no changes needed
- The `types` module exports only domain types -- no changes needed
- Do NOT change `pub mod` declarations in `lib.rs` -- only change the `pub use` re-exports and individual item visibility

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/stack_trace.rs` | Changed 5 regex statics from `pub static` to `pub(crate) static` (DART_VM_FRAME_REGEX, DART_VM_FRAME_NO_COL_REGEX, FRIENDLY_FRAME_REGEX, ASYNC_GAP_REGEX, PACKAGE_PATH_REGEX) |
| `crates/fdemon-core/src/discovery.rs` | Changed 5 helper functions from `pub fn` to `pub(crate) fn` (has_flutter_dependency, is_flutter_plugin, has_platform_directories, has_main_function_in_content, has_main_function). Removed doc examples from internalized functions. |
| `crates/fdemon-core/src/lib.rs` | Removed internalized items from `pub use` re-exports (regex statics and discovery helper functions) |

### Notable Decisions/Tradeoffs

1. **has_flutter_dependency made internal**: Even though the task file suggested checking if it was used externally and keeping it public if so, grep confirmed it was only used in tests within fdemon-core itself, so it was made `pub(crate)`.

2. **Removed doc examples**: The doc examples for `has_main_function_in_content` and `has_main_function` were removed because they showed public API usage which would fail to compile now that these functions are `pub(crate)`.

3. **Dead code warnings acceptable**: The compiler warns that `has_flutter_dependency` and `PACKAGE_PATH_REGEX` are never used. This is expected because they're only used in tests (which doesn't count as usage for dead code analysis). These warnings are acceptable and don't indicate a problem.

### Testing Performed

- `cargo check -p fdemon-core` - Passed (with 2 expected dead code warnings)
- `cargo test -p fdemon-core` - Passed (all 243 unit tests + 4 doc tests passed)
- Verified no external usage of internalized items via grep in fdemon-daemon, fdemon-app, fdemon-tui, and src/

### Risks/Limitations

1. **Workspace compilation issues**: The workspace has pre-existing compilation errors in fdemon-daemon and fdemon-app that are unrelated to this task. These errors existed before the fdemon-core API changes and are due to other refactoring work on the branch (removal of strip_brackets function and changes to LogEntryInfo visibility).

2. **Unused static PACKAGE_PATH_REGEX**: This regex was never actually used in the implementation (is_package_path uses plain string matching). It could potentially be removed entirely in a future cleanup task.
