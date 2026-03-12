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
