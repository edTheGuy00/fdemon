## Task: Update Tests for IconSet and IconMode

**Objective**: Update all existing tests that reference icon constants, add comprehensive tests for `IconSet` and `IconMode`, and ensure full test coverage for both icon modes.

**Depends on**: 03-wire-icon-set-to-tui, 04-settings-panel

### Scope

- `crates/fdemon-tui/src/theme/icons.rs`: New `IconSet` tests (replaces old constant tests)
- `crates/fdemon-tui/src/theme/styles.rs`: Update `phase_indicator` tests to pass `IconSet`
- `crates/fdemon-tui/src/widgets/header.rs`: Update `device_icon_for_platform` tests
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: Update any tests referencing icon constants
- `crates/fdemon-app/src/config/types.rs`: Add `IconMode` tests
- `crates/fdemon-app/src/config/settings.rs`: Add env var override tests

### Details

**1. `icons.rs` — New IconSet tests**

Replace the existing tests (which test static constants) with `IconSet`-based tests:

- `test_unicode_icons_are_non_empty` — All `IconSet::new(Unicode)` methods return non-empty strings
- `test_nerd_font_icons_are_non_empty` — All `IconSet::new(NerdFonts)` methods return non-empty strings
- `test_unicode_and_nerd_font_differ` — Key icons return different values for each mode
- `test_terminal_and_command_are_distinct` — `terminal()` != `command()` in both modes
- `test_phase_indicator_icons_match` — `dot()`, `circle()`, `refresh()`, `close()` return expected Unicode values
- `test_icon_set_is_copy` — Verify `Copy` semantics

**2. `styles.rs` — Update phase_indicator tests**

Tests currently assert hardcoded strings like `assert_eq!(icon, "●")`. Update to:

```rust
#[test]
fn test_phase_indicator_running() {
    let icons = IconSet::new(IconMode::Unicode);
    let (icon, label, style) = phase_indicator(&AppPhase::Running, &icons);
    assert_eq!(icon, icons.dot());
    assert_eq!(label, "Running");
    // ...style assertions unchanged
}
```

Also add tests for `NerdFonts` mode:

```rust
#[test]
fn test_phase_indicator_running_nerd_fonts() {
    let icons = IconSet::new(IconMode::NerdFonts);
    let (icon, label, _) = phase_indicator(&AppPhase::Running, &icons);
    assert_eq!(icon, icons.dot());
    assert_eq!(label, "Running");
}
```

**3. `header.rs` — Update device_icon_for_platform tests**

Tests currently check against `icons::ICON_SMARTPHONE` etc. Update to use `IconSet`:

```rust
#[test]
fn test_device_icon_ios() {
    let icons = IconSet::new(IconMode::Unicode);
    assert_eq!(device_icon_for_platform(Some("ios"), &icons), icons.smartphone());
}

#[test]
fn test_device_icon_nerd_fonts() {
    let icons = IconSet::new(IconMode::NerdFonts);
    assert_eq!(device_icon_for_platform(Some("ios"), &icons), icons.smartphone());
}
```

**4. `config/types.rs` — IconMode tests**

```rust
#[test]
fn test_icon_mode_default_is_unicode() {
    assert_eq!(IconMode::default(), IconMode::Unicode);
}

#[test]
fn test_icon_mode_display() {
    assert_eq!(IconMode::Unicode.to_string(), "unicode");
    assert_eq!(IconMode::NerdFonts.to_string(), "nerd_fonts");
}

#[test]
fn test_icon_mode_serde_roundtrip() {
    let toml = r#"icons = "nerd_fonts""#;
    #[derive(Debug, Deserialize, Serialize)]
    struct W { icons: IconMode }
    let w: W = toml::from_str(toml).unwrap();
    assert_eq!(w.icons, IconMode::NerdFonts);
    let serialized = toml::to_string(&w).unwrap();
    assert!(serialized.contains("nerd_fonts"));
}

#[test]
fn test_settings_icons_default() {
    let settings = Settings::default();
    assert_eq!(settings.ui.icons, IconMode::Unicode);
}
```

**5. `config/settings.rs` — Env var override tests**

```rust
#[test]
fn test_load_settings_with_icons() {
    let temp = tempdir().unwrap();
    let fdemon_dir = temp.path().join(".fdemon");
    std::fs::create_dir_all(&fdemon_dir).unwrap();
    std::fs::write(
        fdemon_dir.join("config.toml"),
        "[ui]\nicons = \"nerd_fonts\"\n",
    ).unwrap();

    let settings = load_settings(temp.path());
    assert_eq!(settings.ui.icons, IconMode::NerdFonts);
}

#[test]
fn test_save_settings_roundtrip_with_icons() {
    let temp = tempdir().unwrap();
    let mut settings = Settings::default();
    settings.ui.icons = IconMode::NerdFonts;

    save_settings(temp.path(), &settings).unwrap();
    let loaded = load_settings(temp.path());
    assert_eq!(loaded.ui.icons, IconMode::NerdFonts);
}

#[test]
fn test_default_config_includes_icons_field() {
    let content = generate_default_config();
    assert!(content.contains("icons"));
}
```

**6. Verify all tests pass**

Run the full test suite:
```bash
cargo test --workspace --lib
```

Ensure no test regressions. The total test count should remain close to 1,532 (plus new tests).

### Acceptance Criteria

1. All existing tests pass (no regressions)
2. New `IconSet` tests cover all icon methods for both modes
3. Phase indicator tests verify both `Unicode` and `NerdFonts` modes
4. Device icon tests verify both modes
5. `IconMode` serde roundtrip is tested
6. Config default includes `icons` field
7. `cargo test --workspace --lib` passes with 0 failures
8. `cargo clippy --workspace -- -D warnings` passes

### Testing

```bash
cargo test -p fdemon-tui --lib     # IconSet + widget tests
cargo test -p fdemon-app --lib     # IconMode + config tests
cargo test --workspace --lib       # Full suite
cargo clippy --workspace -- -D warnings
```

### Notes

- Some existing tests may hardcode icon values like `"●"` or `"⚠"`. These should be updated to use `IconSet::new(IconMode::Unicode).method()` to stay in sync if values ever change.
- The render snapshot tests in `crates/fdemon-tui/src/render/tests.rs` may need updating if they capture icon characters in their expected output.
- Don't add tests for the `FDEMON_ICONS` env var override in unit tests — env var tests are flaky in parallel test runners. Document manual verification instead.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/theme/styles.rs` | Added 3 NerdFonts mode tests for phase indicators |
| `crates/fdemon-tui/src/widgets/header.rs` | Added 6 tests for device icon platform mapping (Unicode + NerdFonts) |
| `crates/fdemon-app/src/config/settings.rs` | Replaced env var tests with 3 stable tests for icon config loading/saving |

### Notable Decisions/Tradeoffs

1. **Existing Tests Already Updated**: Tasks 01-04 already updated most tests. Found comprehensive tests already in place:
   - `icons.rs` (lines 168-228): All IconSet tests for both modes
   - `styles.rs` (lines 246-327): Phase indicator tests with IconSet
   - `header.rs` (lines 239-401): Widget rendering tests with IconSet
   - `log_view/tests.rs` (lines 4-5, 24-27): Helper using IconSet
   - `config/types.rs` (lines 918-958): IconMode serde/display/default tests

2. **Removed Env Var Tests**: Replaced 6 environment variable tests in `config/settings.rs` (lines 1453-1540) with 3 stable tests that don't manipulate `std::env`, as env var tests are flaky in parallel test runners per task requirements.

3. **Added NerdFonts Coverage**: Added tests specifically for NerdFonts mode in:
   - `styles.rs`: 3 tests verifying phase indicators work with NerdFonts and differ from Unicode
   - `header.rs`: Tests for device icon mapping with both icon modes

### Testing Performed

- `cargo test --workspace --lib` - **PASSED** (1,550 tests total)
  - fdemon-app: 744 passed (5 ignored)
  - fdemon-core: 243 passed
  - fdemon-daemon: 136 passed (3 ignored)
  - fdemon-tui: 427 passed
- `cargo clippy --workspace -- -D warnings` - **PASSED** (no warnings)

### Risks/Limitations

1. **Environment Variable Testing**: Manual testing required for `FDEMON_ICONS` env var override feature. Document verification:
   ```bash
   FDEMON_ICONS=nerd_fonts cargo run
   FDEMON_ICONS=unicode cargo run
   ```
