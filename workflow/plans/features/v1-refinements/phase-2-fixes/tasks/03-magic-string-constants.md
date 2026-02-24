## Task: Replace magic string literals with named constants

**Objective**: Define named constants for field routing identifiers (`"dart_defines"`, `"extra_args"`, `"launch.__add_new__"`) and the add-new button count (`+ 1`), then replace all scattered usages across the codebase. Add a doc comment to `PRESET_EXTRA_ARGS`.

**Depends on**: None

**Estimated Time**: 1-2 hours

**Review Issues**: Major #3, Minor #10, Minor #11

### Scope

- `crates/fdemon-app/src/settings_items.rs`: Define constants, use in `launch_config_items()` and `get_selected_item()`
- `crates/fdemon-app/src/handler/settings_handlers.rs`: Use constants in `handle_settings_toggle_edit()` and `get_item_count_for_tab()`
- `crates/fdemon-app/src/handler/settings.rs`: Use constants in `apply_launch_config_change()`
- `crates/fdemon-app/src/handler/settings_extra_args.rs`: Add doc comment to `PRESET_EXTRA_ARGS`

### Details

#### 1. Define constants in `settings_items.rs`

Add at the top of the file (before functions):

```rust
/// Field suffix for dart defines items in launch config settings.
/// Used in item IDs with format: `launch.{idx}.dart_defines`
pub const FIELD_DART_DEFINES: &str = "dart_defines";

/// Field suffix for extra args items in launch config settings.
/// Used in item IDs with format: `launch.{idx}.extra_args`
pub const FIELD_EXTRA_ARGS: &str = "extra_args";

/// Sentinel item ID for the "Add New Configuration" button in launch config settings.
pub const SENTINEL_ADD_NEW: &str = "launch.__add_new__";

/// Number of virtual items appended after real launch config items (the "Add New" button).
pub const ADD_NEW_BUTTON_COUNT: usize = 1;
```

`settings_items.rs` is the natural home for these constants since it generates the items that use these IDs.

#### 2. Replace usages in `settings_items.rs`

In `get_selected_item()` (around line 44-49):
```rust
// Before:
SettingItem::new("launch.__add_new__", "Add New Configuration")
// After:
SettingItem::new(SENTINEL_ADD_NEW, "Add New Configuration")
```

In `launch_config_items()` (around lines 397, 408):
```rust
// Before:
SettingItem::new(format!("{}.dart_defines", prefix), "Dart Defines")
SettingItem::new(format!("{}.extra_args", prefix), "Extra Args")
// After:
SettingItem::new(format!("{}.{}", prefix, FIELD_DART_DEFINES), "Dart Defines")
SettingItem::new(format!("{}.{}", prefix, FIELD_EXTRA_ARGS), "Extra Args")
```

#### 3. Replace usages in `settings_handlers.rs`

In `handle_settings_toggle_edit()` (around lines 86, 93, 106):
```rust
// Before:
if item.id == "launch.__add_new__" {
if item.id.ends_with(".dart_defines") {
if item.id.ends_with(".extra_args") {
// After:
use crate::settings_items::{SENTINEL_ADD_NEW, FIELD_DART_DEFINES, FIELD_EXTRA_ARGS};
if item.id == SENTINEL_ADD_NEW {
if item.id.ends_with(&format!(".{}", FIELD_DART_DEFINES)) {
if item.id.ends_with(&format!(".{}", FIELD_EXTRA_ARGS)) {
```

In `get_item_count_for_tab()` (around line 401):
```rust
// Before:
item_count + 1
// After:
use crate::settings_items::ADD_NEW_BUTTON_COUNT;
item_count + ADD_NEW_BUTTON_COUNT
```

#### 4. Replace usages in `settings.rs`

In `apply_launch_config_change()` (around lines 200, 215):
```rust
// Before:
"dart_defines" => { ... }
"extra_args" => { ... }
// After:
use crate::settings_items::{FIELD_DART_DEFINES, FIELD_EXTRA_ARGS};
field if field == FIELD_DART_DEFINES => { ... }
field if field == FIELD_EXTRA_ARGS => { ... }
```

Note: Match arms with string constants require `field if field == CONST` pattern since `const` strings cannot be used directly as match patterns in Rust. Alternatively, use an `if`/`else if` chain instead of `match` if the conversion is cleaner.

#### 5. Add doc comment to `PRESET_EXTRA_ARGS`

In `settings_extra_args.rs` (around line 17):
```rust
/// Preset Flutter CLI flags shown in the extra args fuzzy picker when
/// the launch config has no existing extra args. Users can always type
/// custom flags via the modal's custom input support.
const PRESET_EXTRA_ARGS: &[&str] = &[
    "--verbose",
    ...
];
```

### Acceptance Criteria

1. No bare `"dart_defines"`, `"extra_args"`, or `"launch.__add_new__"` string literals remain in handler/settings code (only in constant definitions and test assertions)
2. No bare `+ 1` for add-new button count
3. All constants are defined in `settings_items.rs` with doc comments
4. All existing tests pass without modification
5. `PRESET_EXTRA_ARGS` has a doc comment
6. `cargo clippy -- -D warnings` passes

### Testing

Existing tests should continue to pass unchanged — this is a pure refactoring. String literals in test assertions are acceptable and need not use constants (tests serve as documentation of expected values).

### Notes

- The `match field { "dart_defines" => ... }` pattern in `apply_launch_config_change()` cannot use constants directly in Rust match arms. Use `field if field == FIELD_DART_DEFINES =>` guard pattern, or convert to `if`/`else if` chain.
- Test assertion strings (e.g., `assert_eq!(item.id, "launch.__add_new__")`) may optionally be updated to use the constants, but this is not required since tests serve as regression anchors for the actual string values.
- The `ends_with` pattern in `settings_handlers.rs` could alternatively use `ends_with(&format!(".{}", CONST))` or a helper function — prefer whichever is more readable.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/settings_items.rs` | Added 4 pub constants with doc comments (`FIELD_DART_DEFINES`, `FIELD_EXTRA_ARGS`, `SENTINEL_ADD_NEW`, `ADD_NEW_BUTTON_COUNT`); replaced bare `"dart_defines"` and `"extra_args"` format string literals in `launch_config_items()` with constant references |

### Notable Decisions/Tradeoffs

1. **Scope limited to what exists in the worktree**: The worktree branch (`agent-af630cb4`) is on a different branch from the main project (`develop`). Files `settings_handlers.rs` and `settings.rs` in the worktree do NOT yet contain the `"launch.__add_new__"`, `".dart_defines"`, `".extra_args"` string patterns described in the task (those exist only in the main branch's newer commits). Similarly, `settings_extra_args.rs` does not exist in the worktree. Therefore, only the magic strings actually present in the worktree were replaced.

2. **Constants still defined for future use**: All 4 constants (`FIELD_DART_DEFINES`, `FIELD_EXTRA_ARGS`, `SENTINEL_ADD_NEW`, `ADD_NEW_BUTTON_COUNT`) were added to `settings_items.rs` as `pub const` with doc comments, making them available for when the other handler code is merged/added. This satisfies acceptance criterion #3 completely.

3. **Acceptance criteria #5 (PRESET_EXTRA_ARGS doc comment) not applicable**: The file `settings_extra_args.rs` does not exist in the worktree branch. This criterion cannot be addressed without inventing code that doesn't exist yet.

4. **Acceptance criteria #1 and #2 are satisfied**: No bare magic strings exist in handler/settings code in the worktree (they weren't there to begin with, and the ones that were in `settings_items.rs` have been replaced).

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (906 tests passed, 0 failed, 5 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **Partial implementation due to branch divergence**: The worktree branch is behind `develop` and doesn't contain the settings launch tab modals feature (commit `854a05a`). When that feature is merged, the handler code containing the magic strings should use the constants defined here.
