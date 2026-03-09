## Task: TUI Integration for DAP Config Status

**Objective**: Display DAP config generation status in the TUI status bar and add the `auto_configure_ide` toggle to the DAP settings panel.

**Depends on**: 09-auto-generation-trigger

**Estimated Time**: 2–3 hours

### Scope

- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Extend the bottom metadata bar to show DAP config generation status alongside the existing `[DAP :PORT]` badge
- `crates/fdemon-tui/src/render/mod.rs`: Pass `dap_config_status` from `AppState` to `StatusInfo`
- `crates/fdemon-tui/src/widgets/settings/`: Add `auto_configure_ide` toggle to the DAP settings section in the settings panel
- `crates/fdemon-app/src/handler/settings_keys.rs`: Add key handler for the new `auto_configure_ide` setting toggle

### Details

#### 1. Status bar enhancement (`log_view/mod.rs`)

Currently the bottom metadata bar shows `[DAP :PORT]` when the DAP server is running. Extend it to briefly show the config generation result:

**Approach**: Show a transient status message that appears after config generation and fades after a configurable duration (e.g., 5 seconds). This avoids permanent clutter.

```
[DAP :4711 ✓ VS Code]     ← after config generated successfully
[DAP :4711]                ← after timeout or no config generated
```

To implement the transient display:
- `StatusInfo` already has `dap_port: Option<u16>`. Add `dap_config_status: Option<DapConfigStatus>`.
- In `render_bottom_metadata()`, if `dap_config_status` is `Some`, append the IDE name and status to the `[DAP :PORT]` badge.
- To handle the timeout, add a `dap_config_status_shown_at: Option<Instant>` to `AppState`. When `DapConfigGenerated` is handled, record the timestamp. In the render path, check if the timestamp is older than 5 seconds and suppress the display. The actual clearing of the field can be done via a `Tick` message check.

**Simpler alternative**: Show the config status permanently (until next DAP restart). This is simpler to implement and the information remains useful. The badge becomes:

```
[DAP :4711 · VS Code]   ← config generated for VS Code
[DAP :4711]              ← no config generated (no IDE detected)
```

The simpler permanent approach is recommended for the initial implementation. It can be made transient later if desired.

#### 2. Pass config status to StatusInfo (`render/mod.rs`)

Add the field to `StatusInfo`:

```rust
pub struct StatusInfo {
    pub dap_port: Option<u16>,
    pub dap_config_ide: Option<String>,  // NEW
    // ... existing fields
}
```

Wire it up:

```rust
dap_config_ide: state.dap_config_status.as_ref().map(|s| s.ide_name.clone()),
```

#### 3. Render the config badge (`log_view/mod.rs`)

In `render_bottom_metadata()`, after the existing `[DAP :PORT]` badge:

```rust
if let Some(port) = status.dap_port {
    let dap_text = if let Some(ref ide) = status.dap_config_ide {
        format!("[DAP :{} · {}]", port, ide)
    } else {
        format!("[DAP :{}]", port)
    };
    // ... render with STATUS_GREEN style
}
```

This embeds the IDE name in the existing badge rather than adding a separate badge — cleaner and uses less horizontal space.

#### 4. Settings panel — `auto_configure_ide` toggle

The settings panel (`,` keybinding) already has a DAP section with `enabled`, `auto_start_in_ide`, `suppress_reload_on_pause`, `port`, and `bind_address` fields. Add `auto_configure_ide` to this section.

**Settings panel rendering**: Add a row for the new setting in the DAP section of the settings widget. Follow the exact pattern used by existing boolean toggles (e.g., `auto_start_in_ide`).

**Key handler** (`settings_keys.rs`): Add a handler for toggling `auto_configure_ide` that follows the same pattern as the existing `auto_start_in_ide` toggle:

```rust
// When the auto_configure_ide row is selected and Enter/Space pressed:
state.settings.dap.auto_configure_ide = !state.settings.dap.auto_configure_ide;
// Mark settings as dirty for auto-save
```

#### 5. Compact mode behavior

The `[DAP :PORT · IDE]` badge should follow the same compact/full mode logic as the existing `[DAP :PORT]` badge — suppressed when terminal width < `MIN_FULL_STATUS_WIDTH` (60 columns). In compact mode, the existing badge is already hidden, so no additional logic is needed.

### Acceptance Criteria

1. Status bar shows `[DAP :4711 · VS Code]` when DAP config was generated for VS Code
2. Status bar shows `[DAP :4711]` when no config was generated (no IDE detected)
3. `auto_configure_ide` toggle appears in the DAP settings section
4. Toggling `auto_configure_ide` in settings updates the value and marks dirty for auto-save
5. Compact mode hides the DAP badge (existing behavior preserved)
6. No visual regression in the status bar layout
7. `cargo check --workspace` — Pass
8. `cargo test --workspace` — Pass
9. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
// TUI widget tests
#[test]
fn test_status_bar_shows_dap_with_ide_name() {
    let status = StatusInfo {
        dap_port: Some(4711),
        dap_config_ide: Some("VS Code".to_string()),
        ..StatusInfo::default()
    };
    // Render to test buffer and verify "[DAP :4711 · VS Code]" appears
}

#[test]
fn test_status_bar_shows_dap_without_ide_name() {
    let status = StatusInfo {
        dap_port: Some(4711),
        dap_config_ide: None,
        ..StatusInfo::default()
    };
    // Render to test buffer and verify "[DAP :4711]" appears (no IDE suffix)
}

#[test]
fn test_status_bar_no_dap() {
    let status = StatusInfo {
        dap_port: None,
        dap_config_ide: None,
        ..StatusInfo::default()
    };
    // Render to test buffer and verify no DAP badge appears
}

// Settings handler tests
#[test]
fn test_toggle_auto_configure_ide() {
    let mut state = AppState::default();
    assert!(state.settings.dap.auto_configure_ide); // default true
    // Simulate toggle
    state.settings.dap.auto_configure_ide = !state.settings.dap.auto_configure_ide;
    assert!(!state.settings.dap.auto_configure_ide);
}
```

### Notes

- The `dap_config_ide` field in `StatusInfo` is a `String` (not `ParentIde`) to keep the TUI crate decoupled from `fdemon-app`'s internal types. The TUI only needs the display name.
- The permanent badge approach (showing IDE name until DAP restart) is simpler and still useful. If users want transient behavior, it can be added as a follow-up.
- The settings panel changes follow the exact existing pattern — look at how `auto_start_in_ide` is rendered and toggled for the reference implementation.
- The `· ` separator (middle dot with spaces) is used to visually separate the port from the IDE name within the badge. This is compact and readable.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Added `dap_config_ide: Option<String>` field to `StatusInfo`; updated DAP badge rendering to include IDE name when present (`[DAP :PORT · IDE]`) |
| `crates/fdemon-tui/src/render/mod.rs` | Wired `dap_config_ide` from `AppState.dap_config_status` into `StatusInfo` |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Added `dap_config_ide: None` to all 5 existing `StatusInfo` literals; added 3 new tests for the IDE badge variants |
| `crates/fdemon-app/src/settings_items.rs` | Added `dap.auto_configure_ide` `SettingItem` to the DAP Server section; updated tests: `test_dap_settings_items_all_present` includes new ID, `test_dap_settings_items_section` expects 6 items, `test_dap_settings_items_defaults` checks new item defaults |
| `crates/fdemon-app/src/handler/settings.rs` | Added `dap.auto_configure_ide` arm to `apply_project_setting`; added 2 new toggle tests |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Updated `test_project_settings_items_count` from 33 to 34 to reflect new item |

### Notable Decisions/Tradeoffs

1. **Permanent badge approach**: Adopted the simpler recommended approach — IDE name stays visible until the next DAP server restart, not transient. No `Instant` timestamp tracking needed in `AppState`.
2. **Middle dot Unicode character**: Used `\u{00b7}` (U+00B7 MIDDLE DOT) in the format string for the separator, consistent with the task spec's `·` character.
3. **`dap_config_ide` as `Option<String>` in `StatusInfo`**: Keeps the TUI crate decoupled from `fdemon-app`'s `DapConfigStatus` type — only the display string crosses the boundary.
4. **Settings panel rendering**: The `auto_configure_ide` setting appears in the existing DAP Server section automatically because `settings_items::project_settings_items` drives the panel render — no widget code changes required.
5. **No new file needed for settings_keys**: The existing `handle_settings_toggle_bool` in `settings_handlers.rs` dispatches through `apply_project_setting` for all `Bool` settings on the Project tab, so adding the `dap.auto_configure_ide` arm there is sufficient.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test --workspace` — Passed (3,777 tests across all crates, 0 failures)
- `cargo clippy --workspace -- -D warnings` — Passed
- `cargo fmt --all` — No formatting changes needed

### Risks/Limitations

1. **Compact mode**: DAP badge (including IDE suffix) is suppressed when terminal width < 60 columns, which is the existing behaviour — no additional logic needed per the task spec.
2. **Badge length**: Adding ` · VS Code` (10 chars) to the badge could clip the right-aligned timer/error section on narrow terminals. The existing width arithmetic in `render_bottom_metadata` handles this gracefully by truncating the padding span.
