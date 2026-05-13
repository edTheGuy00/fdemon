## Task: Rename `readiness_poll_*` Config Keys to `inspector_readiness_poll_*`

**Objective**: Match the existing `inspector_*` prefix convention used by sibling keys (e.g., `inspector_fetch_timeout_secs`). The current flat keys would collide with future `network_readiness_poll_*` or `performance_readiness_poll_*` if added later. Pre-release rename has no migration cost; post-release would require a shim.

**Depends on**: 08-clamp-readiness-poll-config (both touch the dispatch sites in `handler/devtools/mod.rs`)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/types.rs` — rename `readiness_poll_attempts`, `readiness_poll_interval_ms`, `readiness_poll_call_timeout_ms` fields and their `default_*` functions
- `crates/fdemon-app/src/config/settings.rs` — update the sample config template
- `crates/fdemon-app/src/handler/devtools/mod.rs` — update the three dispatch sites and the `clamped_readiness_poll_config` helper from task 08
- `crates/fdemon-app/src/handler/mod.rs` — update field names on `UpdateAction::FetchWidgetTree` (if they leak through; otherwise the action field names can stay as-is)
- `crates/fdemon-app/src/actions/inspector/mod.rs` — update if function parameter names mirror the config keys
- `crates/fdemon-app/src/process.rs` — update if hydration code references the field names

**Files Read (Dependencies):**
- All call sites that mention `readiness_poll_attempts`, `readiness_poll_interval_ms`, or `readiness_poll_call_timeout_ms`

### Details

Find and rename:

```
readiness_poll_attempts          → inspector_readiness_poll_attempts
readiness_poll_interval_ms       → inspector_readiness_poll_interval_ms
readiness_poll_call_timeout_ms   → inspector_readiness_poll_call_timeout_ms
```

For:
- Field names in `DevToolsSettings` (`config/types.rs`)
- Default function names (`default_readiness_poll_attempts` → `default_inspector_readiness_poll_attempts`, etc.)
- TOML key strings in `config/settings.rs` sample
- Test names and assertions that reference the keys

**Verification command:**
```bash
git grep -nE "readiness_poll_(attempts|interval_ms|call_timeout_ms)" crates/
```
After the rename, all matches should be prefixed with `inspector_`.

### Acceptance Criteria

1. No occurrence of unqualified `readiness_poll_attempts` / `readiness_poll_interval_ms` / `readiness_poll_call_timeout_ms` remains in `crates/`. All are renamed to `inspector_readiness_poll_*`.
2. The sample config template in `config/settings.rs` uses the new key names under `[devtools]`.
3. TOML deserialization tests use the new key names.
4. The `ReadinessPollConfig` struct fields (in `widget_tree.rs`) keep their generic names (`attempts`, `interval_ms`, `call_timeout_ms`) — only the *config* layer names change, not the internal type.
5. All CI quality gates pass.

### Testing

Update existing tests:
- `settings_readiness_poll_defaults_to_2_attempts` → `settings_inspector_readiness_poll_defaults_to_2_attempts`
- `settings_readiness_poll_custom_values_deserialize` → `settings_inspector_readiness_poll_custom_values_deserialize`

Add a regression test confirming the old key name does *not* parse (or is silently ignored — Serde behavior depends on `#[serde(deny_unknown_fields)]`):

```rust
#[test]
fn test_old_readiness_poll_key_does_not_silently_override_default() {
    // If the config has the old key name `readiness_poll_attempts`, it should
    // either error (if deny_unknown_fields) or be ignored (taking the default).
    // The new key `inspector_readiness_poll_attempts` is what takes effect.
    let toml = r#"
        [devtools]
        readiness_poll_attempts = 5
    "#;
    let parsed: DevToolsSettings = toml::from_str(toml).expect("parses");
    // Old key was ignored; default applies
    assert_eq!(parsed.inspector_readiness_poll_attempts, 2);
}
```

### Notes

- This is the last opportunity for a free rename. After release, renaming requires a migration shim that reads both names for at least one release cycle.
- The `ReadinessPollConfig` struct in `widget_tree.rs` is an internal type, not a config key — its field names (`attempts`, `interval_ms`, `call_timeout_ms`) are fine and don't need the prefix.
- If you opt to keep the rename minimal (only the TOML-facing serde rename, not the Rust field), use `#[serde(rename = "inspector_readiness_poll_attempts")]` on the existing field. This is cleaner but harder to discover — explicit field rename is preferred.
