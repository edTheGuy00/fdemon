## Task: Minor Cleanups (validate, magic number, syslog warning)

**Objective**: Address three minor code quality issues: dead `validate()` code, magic number in tag filter, and silent syslog failure on non-macOS.

**Depends on**: None

**Review Issues**: #7 (MINOR), #9 (MINOR), #10 (MINOR)

### Scope

- `crates/fdemon-app/src/actions/native_logs.rs`: Call `validate()` from spawn path (~line 260)
- `crates/fdemon-app/src/config/types.rs`: No changes needed (validate already exists)
- `crates/fdemon-tui/src/widgets/tag_filter.rs`: Promote magic number to named constant (~line 95)
- `crates/fdemon-daemon/src/native_logs/formats.rs`: Add warning for syslog on non-macOS (~line 155)

### Details

**Issue #7 — `CustomSourceConfig::validate()` is dead code:**

`validate()` (config/types.rs:606-626) checks empty name, empty command, and warns about platform tag shadowing. But `spawn_custom_sources` (actions/native_logs.rs:260-266) duplicates the empty-name/empty-command check inline and skips the platform tag warning.

**Fix:** Replace the inline guard with a call to `validate()`:
```rust
// Before:
if source_config.name.trim().is_empty() || source_config.command.trim().is_empty() {
    tracing::warn!("Skipping custom log source with empty name or command...");
    continue;
}

// After:
if let Err(e) = source_config.validate() {
    tracing::warn!("Skipping invalid custom log source for session {}: {}", session_id, e);
    continue;
}
```

This activates the `KNOWN_PLATFORM_TAGS` advisory warning and removes code duplication.

---

**Issue #9 — Magic number `20` for tag column width:**

`tag_filter.rs:95` has `let tag_col_width: usize = 20;` as a local binding. The file already has two proper named constants (`TAG_FILTER_MIN_WIDTH`, `TAG_FILTER_MAX_VISIBLE_TAGS`).

**Fix:** Promote to a module-level constant:
```rust
/// Width of the tag name column in the filter overlay, in characters.
/// Derived from: overlay min-width (42) minus checkbox "[x] " (4),
/// count suffix " (N entries)" (~14), and padding.
const TAG_COLUMN_WIDTH: usize = 20;
```

Replace `let tag_col_width: usize = 20;` with `TAG_COLUMN_WIDTH` at the usage sites.

---

**Issue #10 — Syslog format silently produces no output on non-macOS:**

`parse_syslog` in `formats.rs` has a `#[cfg(not(target_os = "macos"))]` stub that returns `None` unconditionally. A user on Linux configuring `format = "syslog"` gets zero output with no indication of the error.

**Fix (two complementary steps):**

1. Add a `tracing::warn!` when a custom source with `format = "syslog"` is started on non-macOS. Best placed in the custom source runner before the capture loop starts (not per-line):
   ```rust
   #[cfg(not(target_os = "macos"))]
   if format == OutputFormat::Syslog {
       tracing::warn!(
           "Custom source '{}': syslog format is only supported on macOS; output will be treated as raw text",
           source_name
       );
       // Fall back to raw format
   }
   ```

2. Alternatively, add validation in `CustomSourceConfig::validate()`:
   ```rust
   #[cfg(not(target_os = "macos"))]
   if self.format == Some(OutputFormat::Syslog) {
       return Err(format!(
           "custom_source '{}': syslog format is only supported on macOS",
           self.name
       ));
   }
   ```

Option 2 (config-time rejection) is preferred since it catches the error before any process is spawned. Option 1 (runtime warning with fallback to raw) is a softer approach.

### Acceptance Criteria

1. `CustomSourceConfig::validate()` is called from `spawn_custom_sources` — no inline duplication
2. Tag column width `20` is a named `const TAG_COLUMN_WIDTH` with derivation comment
3. Syslog format on non-macOS either rejects at config time or warns at startup
4. No new magic numbers introduced
5. Existing tests pass

### Testing

- Issue #7: Existing `validate()` tests in `config/types.rs` are now exercised through the production path
- Issue #9: No new tests needed (constant rename, same value)
- Issue #10: Add a `#[cfg(not(target_os = "macos"))]` test that verifies the syslog warning/rejection

### Notes

- Issue #7 also connects to issue #11 (duplicate names) — if task 07 adds a `NativeLogsSettings::validate()`, call it from the same validation point
- Issue #10: The "fall back to raw" approach is more user-friendly but less explicit. Config-time rejection is safer.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/native_logs.rs` | Replaced inline empty-name/empty-command guard with `source_config.validate()` call; updated doc comment |
| `crates/fdemon-app/src/config/types.rs` | Added `#[cfg(not(target_os = "macos"))]` syslog rejection to `CustomSourceConfig::validate()`; added two new tests (`test_custom_source_syslog_format_rejected_on_non_macos`, `test_custom_source_syslog_format_allowed_on_macos`) |
| `crates/fdemon-tui/src/widgets/tag_filter.rs` | Promoted `let tag_col_width: usize = 20;` to module-level `const TAG_COLUMN_WIDTH: usize = 20;` with derivation doc comment; replaced all usage sites |

### Notable Decisions/Tradeoffs

1. **Issue #10 — Config-time rejection chosen over runtime warning**: The task preferred Option 2 (config-time rejection via `validate()`) over Option 1 (runtime warning in `formats.rs`). Since `validate()` is now called in `spawn_custom_sources` (fix #7), syslog rejection flows naturally through the same validation gate without any changes to `formats.rs`. The `#[cfg(not(target_os = "macos"))]` block in `validate()` surfaces the error before any process is spawned, which is cleaner than warning per-line in the format parser.

2. **Task 07 connection**: The task notes mention calling `NativeLogsSettings::validate()` once it's added by task 07. Task 07 is not yet implemented, so the connection point in `spawn_custom_sources` is not yet hooked up to the higher-level duplicate-name check. When task 07 is done, a call to `settings.validate()` can be added before the loop.

3. **`formats.rs` left unchanged**: The task listed `formats.rs` as a potential target for a runtime warning (Option 1), but since Option 2 was chosen (and flows through `validate()` which is now live), no changes to `formats.rs` were needed.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (6 crates, no errors)
- `cargo test -p fdemon-app --lib` - Passed (1551 tests, 4 ignored)
- `cargo test -p fdemon-tui --lib tag_filter` - Passed (17 tests)
- `cargo test -p fdemon-daemon --lib` - Passed (574 tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

**Pre-existing failures (unrelated to this task):** 4 snapshot tests in `fdemon-tui::render::tests` fail due to version string drift (`v0.1.0` in snapshots vs `v0.2.1` in binary). Not introduced by this task.

### Risks/Limitations

1. **Non-macOS syslog rejection is only caught if `validate()` is called**: The validation only runs at spawn time (in `spawn_custom_sources`). If `CustomSourceConfig` is used elsewhere without calling `validate()`, the check won't fire. The config-load path does not currently call `validate()`, so a user could still deserialize a syslog config without getting an error — they'd only see the error when `spawn_native_log_capture` is called. This is acceptable for now; a future improvement would be to call `validate()` during config loading.
