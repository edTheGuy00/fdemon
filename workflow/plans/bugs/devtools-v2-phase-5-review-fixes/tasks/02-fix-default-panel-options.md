## Task: Fix Stale `default_panel` Enum Options

**Objective**: Update the `default_panel` setting's options list to reflect the actual `DevToolsPanel` enum variants: inspector, performance, network. Remove the stale "layout" option.

**Depends on**: None

**Severity**: HIGH — Settings UI shows invalid option, hides valid one

### Scope

- `crates/fdemon-app/src/settings_items.rs:174-186`: Fix options in both `.value()` and `.default()` calls
- `crates/fdemon-app/src/config/types.rs:288`: Fix doc comment

### Details

**Current options (stale):**
```rust
options: vec!["inspector".to_string(), "layout".to_string(), "performance".to_string()]
```

**Correct options:**
```rust
options: vec!["inspector".to_string(), "performance".to_string(), "network".to_string()]
```

The options list appears twice in `settings_items.rs` — once in the `.value()` builder (line ~174) and once in the `.default()` builder (line ~183). Both must be updated.

Additionally, the doc comment in `config/types.rs:288` says `("inspector", "layout", "performance")` — update to `("inspector", "performance", "network")`.

**No runtime fix needed:** `parse_default_panel()` in `handler/devtools/mod.rs:88-98` already handles `"network"` correctly and maps `"layout"` to `Inspector` as a backward-compat fallback. The parser is fine; only the UI metadata is stale.

### Acceptance Criteria

1. Settings panel shows "inspector", "performance", "network" as `default_panel` options
2. No "layout" visible in settings UI
3. Config doc comment matches actual options
4. Existing tests for `parse_default_panel` still pass (backward compat for "layout" in config files)
5. `cargo test -p fdemon-app` passes

### Testing

Verify existing tests in `handler/devtools/mod.rs` cover the parser — no new tests needed for the UI options fix. Optionally add a test:

```rust
#[test]
fn test_default_panel_options_match_enum_variants() {
    let settings = Settings::default();
    let items = project_settings_items(&settings);
    let panel_item = items.iter().find(|i| i.id == "devtools.default_panel").unwrap();
    if let SettingValue::Enum { options, .. } = &panel_item.value {
        assert!(options.contains(&"inspector".to_string()));
        assert!(options.contains(&"performance".to_string()));
        assert!(options.contains(&"network".to_string()));
        assert!(!options.contains(&"layout".to_string()));
    }
}
```

### Notes

- The `parse_default_panel` backward-compat fallback for `"layout"` in `handler/devtools/mod.rs` should be kept — users may have old config files with `default_panel = "layout"`
- The website docs at `website/src/pages/docs/devtools.rs` also reference stale panel names — that is out of scope for this task but should be tracked separately

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/settings_items.rs` | Replaced "layout" with "network" in both `.value()` and `.default()` options for `devtools.default_panel`; added `#[cfg(test)]` module with two tests verifying the correct options in both value and default builders |
| `crates/fdemon-app/src/config/types.rs` | Updated doc comment on `default_panel` field from `("inspector", "layout", "performance")` to `("inspector", "performance", "network")` |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Added "network" and "net" assertions to `test_default_panel_maps_to_devtools_panel_enum` test to cover those parser branches; backward-compat "layout" fallback left intact |

### Notable Decisions/Tradeoffs

1. **Test module placement**: Added the test module at the end of `settings_items.rs` (after `vscode_config_items`) rather than before the `vscode_config_items` doc comment, consistent with Rust convention of `#[cfg(test)]` at end of file.
2. **Two tests instead of one**: Split the task-suggested test into two — one for `.value` options and one for `.default` options — to give more precise failure messages if either list drifts.
3. **Parser unchanged**: `parse_default_panel()` in `handler/devtools/mod.rs` was not modified; the backward-compat "layout" → Inspector fallback is preserved exactly as required.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1046 unit tests + 1 doc test; up from 1039 with 7 new tests)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **Stale options in other locations**: The task notes that `website/src/pages/docs/devtools.rs` also references stale panel names — that is explicitly out of scope and should be tracked separately.
