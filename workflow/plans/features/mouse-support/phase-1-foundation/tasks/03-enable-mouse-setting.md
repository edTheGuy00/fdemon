## Task: Add `[ui] enable_mouse` setting

**Objective**: Introduce a single new `bool` configuration field, `settings.ui.enable_mouse` (default `true`), surface it in the settings panel UI Section, and wire its mutation through the existing `apply_project_setting` switch. This is the user-facing opt-out for the entire mouse feature; runtime use of the flag happens in Tasks 06 (runner enable/disable) and Phase 2+ (any per-mode gates).

**Depends on**: None

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/types.rs` — add `enable_mouse: bool` to `UiSettings`, add `default_true` reuse, update `Default` impl
- `crates/fdemon-app/src/settings_items.rs` — add a `SettingItem` for `"ui.enable_mouse"` in the UI Section
- `crates/fdemon-app/src/handler/settings.rs` — add `"ui.enable_mouse"` arm in `apply_project_setting`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs` — existing `UiSettings` struct (lines 250–292) for placement
- `crates/fdemon-app/src/settings_items.rs` — existing UI Section (lines 120–180) for placement
- `crates/fdemon-app/src/handler/settings.rs` — existing UI arms (lines 44–84) for placement

### Details

#### Step 1: Add field to `UiSettings`

In `crates/fdemon-app/src/config/types.rs` around line 277 (after `pub icons: IconMode,`), add:

```rust
/// Enable mouse interactions in the TUI: clickable header shortcuts,
/// session tabs, log view, DevTools panels, and dialogs. Scroll wheel
/// always works when enabled. Defaults to `true`.
///
/// Set to `false` if your terminal handles mouse reporting poorly or you
/// prefer Shift-free native text selection. Changes take effect on the
/// next fdemon launch.
#[serde(default = "default_true")]
pub enable_mouse: bool,
```

Inside the same file, locate the existing `default_true` helper used by `show_timestamps`. If `default_true` is reusable for `bool` defaults, point `enable_mouse` at it. Otherwise add a `default_true` helper at module scope:

```rust
// Place near the other `default_*` helpers (around line 294):
fn default_true() -> bool {
    true
}
```

(Search the file for `default_true` first — it likely already exists, since `show_timestamps` and `stack_trace_collapsed` both use `#[serde(default = "default_true")]`. Reuse it; do not duplicate.)

Update the `impl Default for UiSettings` block (around line 280) to include the new field:

```rust
impl Default for UiSettings {
    fn default() -> Self {
        Self {
            log_buffer_size: default_log_buffer_size(),
            show_timestamps: true,
            compact_logs: false,
            theme: default_theme(),
            stack_trace_collapsed: true,
            stack_trace_max_frames: default_stack_trace_max_frames(),
            icons: IconMode::default(),
            enable_mouse: true, // ← new
        }
    }
}
```

#### Step 2: Surface in the settings panel

In `crates/fdemon-app/src/settings_items.rs`, in the UI Section of `project_settings_items` (around line 168, after the `ui.icons` item), insert:

```rust
SettingItem::new("ui.enable_mouse", "Mouse Support")
    .description("Enable mouse interactions (click, scroll). Restart required.")
    .value(SettingValue::Bool(settings.ui.enable_mouse))
    .default(SettingValue::Bool(true))
    .section("UI"),
```

Keep the rest of the UI section ordering intact — insert after `ui.icons`, before `ui.stack_trace_collapsed`.

#### Step 3: Wire the apply switch

In `crates/fdemon-app/src/handler/settings.rs`, in `apply_project_setting`, in the UI section (after the `"ui.stack_trace_max_frames"` arm around line 84), insert:

```rust
"ui.enable_mouse" => {
    if let SettingValue::Bool(v) = &item.value {
        settings.ui.enable_mouse = *v;
    }
}
```

### Acceptance Criteria

1. `settings.ui.enable_mouse: bool` exists; reading a `.fdemon/config.toml` *without* `[ui] enable_mouse = ...` yields `true`.
2. Reading a config file with `[ui] enable_mouse = false` correctly produces `false`.
3. Reading a config file with `[ui] enable_mouse = true` correctly produces `true`.
4. `UiSettings::default().enable_mouse == true`.
5. The settings panel shows a "Mouse Support" toggle in the UI section, sourced from `settings.ui.enable_mouse`.
6. Toggling the setting via the panel and committing causes `apply_project_setting` to update `settings.ui.enable_mouse`.
7. `cargo check -p fdemon-app --all-targets` passes.
8. `cargo test -p fdemon-app` passes — including any existing `UiSettings` deserialization tests, which must still match after the new field is added (they use `#[serde(default = ...)]` so they should continue to pass without edits).

### Testing

Add unit tests covering serde defaults and apply behavior. Place these alongside any existing `UiSettings` tests (search for `mod tests` in `config/types.rs` or `config/settings.rs`).

```rust
#[test]
fn test_ui_settings_enable_mouse_defaults_to_true() {
    let s = UiSettings::default();
    assert!(s.enable_mouse);
}

#[test]
fn test_ui_settings_deserializes_without_enable_mouse_field() {
    // Existing config files predating this feature must still load cleanly
    // and inherit the default.
    let toml = r#"
        log_buffer_size = 5000
        show_timestamps = true
    "#;
    let s: UiSettings = toml::from_str(toml).expect("must deserialize");
    assert!(s.enable_mouse, "missing field should default to true");
}

#[test]
fn test_ui_settings_deserializes_explicit_enable_mouse_false() {
    let toml = r#"enable_mouse = false"#;
    let s: UiSettings = toml::from_str(toml).expect("must deserialize");
    assert!(!s.enable_mouse);
}
```

In `crates/fdemon-app/src/handler/settings.rs` tests (or wherever `apply_project_setting` is tested today — search for existing `apply_project_setting` tests):

```rust
#[test]
fn test_apply_setting_toggles_enable_mouse() {
    let mut settings = Settings::default();
    assert!(settings.ui.enable_mouse, "default should be true");

    let item = SettingItem::new("ui.enable_mouse", "Mouse Support")
        .value(SettingValue::Bool(false));

    apply_project_setting(&mut settings, &item);
    assert!(!settings.ui.enable_mouse);

    let item = SettingItem::new("ui.enable_mouse", "Mouse Support")
        .value(SettingValue::Bool(true));
    apply_project_setting(&mut settings, &item);
    assert!(settings.ui.enable_mouse);
}
```

### Notes

- **Default = `true`.** The feature is off by default in many other terminal apps, but fdemon is a daily-driver dev tool where the ergonomic gain is large. Power users who want native shell-style text selection without `Shift+drag` can set this to `false`. We document the trade-off in `docs/MOUSE.md` (Phase 6).
- **No runtime toggle.** Mouse capture lifecycle runs at process start (Task 06). Toggling the setting at runtime via the settings panel does not re-run capture; the description string says "Restart required" so users are not surprised.
- **Reuse `default_true`.** Do not introduce a new `default_true_for_mouse` helper. The existing helper is already used by `show_timestamps` and `stack_trace_collapsed`.
- **No CHANGELOG entry yet.** Phase 6 adds the user-facing changelog entry; Phase 1 is a partial-feature merge.
