## Task: Fix macOS `log stream --level` and Deduplicate Tag Filtering

**Objective**: Fix the invalid `--level error` argument passed to macOS `log stream`, and consolidate the triplicated `should_include_tag` logic into a shared function.

**Depends on**: None

**Review Issues:** #3 (Major), #7 (Minor)

### Scope

- `crates/fdemon-daemon/src/native_logs/macos.rs`: Fix `build_log_stream_command` level mapping; replace local `should_include_tag` with shared function call
- `crates/fdemon-daemon/src/native_logs/android.rs`: Replace local `should_include_tag` with shared function call
- `crates/fdemon-daemon/src/native_logs/mod.rs`: Add shared `should_include_tag` function

### Details

#### Fix 1: Invalid `--level error` argument (Issue #3 — Major)

In `build_log_stream_command` (macos.rs:133-141), the current level mapping passes `"error"` to `log stream --level`:

```rust
let level = match config.min_level.to_lowercase().as_str() {
    "verbose" | "debug" => "debug",
    "info" => "info",
    "warning" | "error" => "error",   // BUG: "error" is not valid
    _ => "info",
};
```

macOS `log stream --level` only accepts three values: `"default"`, `"info"`, `"debug"`. There is no `"error"` or `"warning"` level. Passing `--level error` causes `log stream` to reject the argument and exit immediately, silently breaking native log capture for any user with `min_level = "warning"` or `"error"`.

**Fix:** Map `"warning"` and `"error"` to `"default"` — the least-verbose valid level. The severity filtering in the parse loop (`run_log_stream` → `to_native_log_event` → level check) already discards messages below the configured `min_level`, so the command just needs to produce enough output for the filter to work.

```rust
let level = match config.min_level.to_lowercase().as_str() {
    "verbose" | "debug" => "debug",
    "info" => "info",
    // "default" is the least-verbose valid value for `log stream --level`.
    // The parse loop applies min_level filtering, so we just need enough output.
    _ => "default",
};
```

This simplification also handles the `_ => "info"` fallback more conservatively — unrecognized levels now use `"default"` (less verbose) instead of `"info"` (more verbose).

**Add a test** for `build_log_stream_command` to verify the level mapping produces only valid values:

```rust
#[test]
fn test_build_log_stream_command_uses_valid_levels() {
    for min_level in ["verbose", "debug", "info", "warning", "error", "unknown"] {
        let config = MacOsLogConfig {
            process_name: "Runner".to_string(),
            exclude_tags: vec![],
            include_tags: vec![],
            min_level: min_level.to_string(),
        };
        let cmd = build_log_stream_command(&config);
        let args: Vec<_> = cmd.as_std().get_args().collect();
        // Find the --level argument value
        let level_idx = args.iter().position(|a| a == "--level").unwrap();
        let level = args[level_idx + 1].to_str().unwrap();
        assert!(
            ["default", "info", "debug"].contains(&level),
            "min_level={} produced invalid log stream level: {}",
            min_level,
            level
        );
    }
}
```

#### Fix 2: Consolidate `should_include_tag` (Issue #7 — Minor)

Three identical implementations exist:
1. `android.rs:92-103` — `fn should_include_tag(config: &AndroidLogConfig, tag: &str) -> bool`
2. `macos.rs:111-122` — `fn should_include_tag(config: &MacOsLogConfig, tag: &str) -> bool`
3. `config/types.rs:602-615` — `fn should_include_tag(&self, tag: &str) -> bool` (on `NativeLogsSettings`)

All three have identical logic: if `include_tags` is non-empty, only those tags pass; otherwise, tags in `exclude_tags` are dropped. Comparison is case-insensitive.

**Fix:** Add a shared free function in `native_logs/mod.rs`:

```rust
/// Determine whether a tag should be included based on include/exclude lists.
///
/// - If `include_tags` is non-empty, only those tags pass (overrides exclude).
/// - Otherwise, tags in `exclude_tags` are dropped; all others pass.
/// - Comparison is case-insensitive.
pub fn should_include_tag(include_tags: &[String], exclude_tags: &[String], tag: &str) -> bool {
    if !include_tags.is_empty() {
        return include_tags.iter().any(|t| t.eq_ignore_ascii_case(tag));
    }
    !exclude_tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
}
```

Then update `android.rs` and `macos.rs` to call it:

```rust
// android.rs — replace local should_include_tag call
if !super::should_include_tag(&config.include_tags, &config.exclude_tags, &tag) {
    continue;
}

// macos.rs — same pattern
if !super::should_include_tag(&config.include_tags, &config.exclude_tags, &tag) {
    continue;
}
```

Delete the local `should_include_tag` functions from both files.

**The third copy** in `config/types.rs` (`NativeLogsSettings::should_include_tag`) lives in `fdemon-app`, which depends on `fdemon-daemon`. It can delegate to the shared function:

```rust
// config/types.rs
pub fn should_include_tag(&self, tag: &str) -> bool {
    fdemon_daemon::native_logs::should_include_tag(&self.include_tags, &self.exclude_tags, tag)
}
```

**Move existing tests** from `android.rs` and `macos.rs` to `mod.rs` alongside the shared function. The 3+3 platform-specific tests collapse into 3 shared tests (same scenarios: no filter, exclude list, include list).

### Acceptance Criteria

1. `build_log_stream_command` never produces `--level error` or `--level warning` — only `"default"`, `"info"`, or `"debug"`
2. A test verifies all `min_level` values map to valid `log stream --level` arguments
3. `should_include_tag` is defined once in `native_logs/mod.rs` with `pub` visibility
4. `android.rs` and `macos.rs` call `super::should_include_tag(...)` instead of local copies
5. `config/types.rs` `NativeLogsSettings::should_include_tag` delegates to the shared function
6. No duplicated tag-filtering logic remains
7. `cargo test -p fdemon-daemon --lib` passes
8. `cargo test -p fdemon-app --lib` passes
9. `cargo clippy --workspace -- -D warnings` passes

### Testing

- Move the 3 shared test cases (no filter, exclude, include) to `mod.rs`
- Add `test_build_log_stream_command_uses_valid_levels` to `macos.rs`
- Verify `config/types.rs` tests still pass (they should, since the method now delegates)

### Notes

- The `build_log_stream_command` function is currently private and untested. Making it `pub(crate)` or `pub(super)` for testability would be acceptable, or test via a wrapper.
- The `config/types.rs` delegation adds a cross-crate call (`fdemon-app` → `fdemon-daemon`). This is fine because `fdemon-app` already depends on `fdemon-daemon`. If preferred, the `config/types.rs` copy can remain as-is (it's a single function, and keeping it avoids a cross-crate coupling for a 5-line function). The key deduplication target is the two daemon-side copies.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/native_logs/macos.rs` | Fixed `build_log_stream_command` level mapping (`"warning"\|"error"` → `"default"` instead of invalid `"error"`); removed local `should_include_tag` function; updated call site to `super::should_include_tag(...)`; removed 3 `should_include_tag` tests; added `test_build_log_stream_command_uses_valid_levels` test |
| `crates/fdemon-daemon/src/native_logs/android.rs` | Removed local `should_include_tag` function; updated call site to `super::should_include_tag(...)`; removed 3 `should_include_tag` tests (moved to mod.rs) |
| `crates/fdemon-daemon/src/native_logs/mod.rs` | Added `pub fn should_include_tag(include_tags, exclude_tags, tag)` free function; added 4 tests covering no-filter, exclude-list, include-list-overrides-exclude, and case-insensitive include scenarios |
| `crates/fdemon-app/src/config/types.rs` | `NativeLogsSettings::should_include_tag` now delegates to `fdemon_daemon::native_logs::should_include_tag(...)` |

### Notable Decisions/Tradeoffs

1. **`"warning"/"error"` map to `"default"` not `"info"`**: The task spec says map to `"default"` (least verbose valid level). This is intentional — higher-severity filtering still happens downstream in the parse loop. Using `"default"` (which includes more messages) ensures we don't accidentally suppress logs we want to see.

2. **Level test uses `cmd.as_std().get_args()`**: `build_log_stream_command` is private. Rather than changing its visibility, the test calls it within the same module via `#[cfg(test)]` access. The `Command::as_std()` method exposes the underlying `std::process::Command` for inspection.

3. **4 shared tests in mod.rs instead of 3**: Added a 4th test for case-insensitive include matching, which wasn't covered in the original platform-specific tests.

4. **Pre-existing test failure**: `tool_availability::tests::test_check_macos_log_returns_true_on_macos` fails in the test environment because `log` command isn't available. This is unrelated to our changes and was failing before this task.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-daemon --lib native_logs` - Passed (38 tests)
- `cargo test -p fdemon-app --lib` - Passed (1464 tests)
- `cargo clippy --workspace -- -D warnings` - Passed
- `cargo fmt --all` - Applied (single-line reformatting of delegation in types.rs)

### Risks/Limitations

1. **Pre-existing failing test**: `test_check_macos_log_returns_true_on_macos` fails in the sandbox environment because `log` is not available. This is unrelated to this task's changes.
