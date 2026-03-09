## Task: Add auto_configure_ide Setting and DapConfigGenerated Message

**Objective**: Add the `auto_configure_ide` boolean field to `DapSettings`, add a `DapConfigGenerated` message variant for reporting config generation results, and add a `GenerateIdeConfig` update action for triggering generation.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `auto_configure_ide: bool` field to `DapSettings` with `#[serde(default = "default_auto_configure_ide")]` defaulting to `true`
- `crates/fdemon-app/src/message.rs`: Add `DapConfigGenerated { path: PathBuf, action: String }` variant to `Message` enum
- `crates/fdemon-app/src/handler/dap.rs`: Add handler for `DapConfigGenerated` message (log result, update state)
- `crates/fdemon-app/src/handler/mod.rs`: Add `GenerateIdeConfig { port: u16 }` variant to `UpdateAction` enum
- `crates/fdemon-app/src/state.rs`: Add `dap_config_status: Option<DapConfigStatus>` field to `AppState` for TUI display

### Details

#### 1. Extend `DapSettings` (`config/types.rs:477-500`)

Add the new field after `suppress_reload_on_pause`:

```rust
pub struct DapSettings {
    pub enabled: bool,
    pub auto_start_in_ide: bool,
    pub port: u16,
    pub bind_address: String,
    pub suppress_reload_on_pause: bool,
    /// Automatically generate IDE DAP config when server starts.
    /// Default: true — generates launch.json/languages.toml/etc. on server bind.
    #[serde(default = "default_auto_configure_ide")]
    pub auto_configure_ide: bool,
}

fn default_auto_configure_ide() -> bool {
    true
}
```

Update the `Default` impl and any existing `DapSettings::new()` or builder patterns to include this field.

#### 2. Add `DapConfigStatus` to state (`state.rs`)

```rust
/// Status of IDE DAP config generation, shown in TUI status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DapConfigStatus {
    /// The IDE config was generated/updated for.
    pub ide_name: String,
    /// The config file path.
    pub path: PathBuf,
    /// What happened.
    pub action: String,
}
```

Add to `AppState`:
```rust
pub dap_config_status: Option<DapConfigStatus>,
```

#### 3. Add `DapConfigGenerated` message (`message.rs`)

Add to the DAP section of the `Message` enum (after the existing `DapClientDisconnected`):

```rust
/// IDE DAP config was generated/updated/skipped.
DapConfigGenerated {
    ide_name: String,
    path: PathBuf,
    action: String, // "Created", "Updated", "Skipped: <reason>"
},
```

#### 4. Add `GenerateIdeConfig` update action (`handler/mod.rs`)

Add to the `UpdateAction` enum:

```rust
/// Generate IDE-specific DAP config file (Phase 5).
GenerateIdeConfig { port: u16 },
```

#### 5. Handle `DapConfigGenerated` in handler (`handler/dap.rs`)

```rust
Message::DapConfigGenerated { ide_name, path, action } => {
    state.dap_config_status = Some(DapConfigStatus {
        ide_name: ide_name.clone(),
        path: path.clone(),
        action: action.clone(),
    });
    tracing::info!("DAP config for {}: {} at {}", ide_name, action, path.display());
    UpdateResult::none()
}
```

#### 6. Wire `GenerateIdeConfig` in update.rs routing

Ensure the existing DAP message routing in `handler/update.rs` includes the new `DapConfigGenerated` variant in the match arm that delegates to `dap::handle_dap_message()`.

### Acceptance Criteria

1. `DapSettings` has `auto_configure_ide: bool` field defaulting to `true`
2. Existing configs without `auto_configure_ide` deserialize correctly (serde default)
3. `Message::DapConfigGenerated` variant exists and is routed to DAP handler
4. `UpdateAction::GenerateIdeConfig` variant exists
5. `DapConfigStatus` struct is available on `AppState`
6. Handler for `DapConfigGenerated` stores status in `AppState`
7. `cargo check --workspace` — Pass
8. `cargo test --workspace` — Pass
9. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_dap_settings_default_auto_configure_ide() {
    let settings = DapSettings::default();
    assert!(settings.auto_configure_ide);
}

#[test]
fn test_dap_settings_deserialize_without_auto_configure_ide() {
    let toml = r#"
    enabled = false
    port = 0
    bind_address = "127.0.0.1"
    "#;
    let settings: DapSettings = toml::from_str(toml).unwrap();
    assert!(settings.auto_configure_ide); // default true
}

#[test]
fn test_dap_settings_deserialize_with_auto_configure_ide_false() {
    let toml = r#"
    enabled = false
    auto_configure_ide = false
    "#;
    let settings: DapSettings = toml::from_str(toml).unwrap();
    assert!(!settings.auto_configure_ide);
}

#[test]
fn test_handle_dap_config_generated_stores_status() {
    let mut state = AppState::default();
    let msg = Message::DapConfigGenerated {
        ide_name: "VS Code".into(),
        path: PathBuf::from(".vscode/launch.json"),
        action: "Created".into(),
    };
    let result = handle_dap_message(&mut state, &msg);
    assert!(result.action.is_none());
    assert!(state.dap_config_status.is_some());
    let status = state.dap_config_status.unwrap();
    assert_eq!(status.ide_name, "VS Code");
    assert_eq!(status.action, "Created");
}
```

### Notes

- `auto_configure_ide` uses the same `#[serde(default)]` pattern as `auto_start_in_ide` to ensure backward compatibility with existing config files.
- The `action` field in `DapConfigGenerated` is a `String` rather than the `ConfigAction` enum to keep the message type simple and avoid coupling `message.rs` to `ide_config/` types. The handler can parse it if needed.
- The settings panel (`,` keybinding) already renders `DapSettings` fields. Adding `auto_configure_ide` to the settings panel rendering is deferred to Task 11 (TUI integration).
- `DapConfigStatus` is stored on `AppState` for the TUI to read during rendering. It persists until the next DAP server restart.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Added `auto_configure_ide: bool` field to `DapSettings` with `#[serde(default = "default_auto_configure_ide")]` defaulting to `true`; added `default_auto_configure_ide()` fn; updated `Default` impl; added 3 new tests |
| `crates/fdemon-app/src/state.rs` | Added `DapConfigStatus` struct with `ide_name`, `path`, `action` fields; added `dap_config_status: Option<DapConfigStatus>` field to `AppState`; initialized to `None` in `with_settings()` |
| `crates/fdemon-app/src/message.rs` | Added `DapConfigGenerated { ide_name, path, action }` variant to `Message` enum in the DAP section |
| `crates/fdemon-app/src/handler/mod.rs` | Added `GenerateIdeConfig { port: u16 }` variant to `UpdateAction` enum |
| `crates/fdemon-app/src/handler/dap.rs` | Added `DapConfigStatus` to import; added `DapConfigGenerated` match arm in `handle_dap_message`; added `handle_config_generated()` function; added 3 new tests |
| `crates/fdemon-app/src/handler/update.rs` | Added `Message::DapConfigGenerated { .. }` to the DAP message routing match arm |

### Notable Decisions/Tradeoffs

1. **`GenerateIdeConfig` was already in `actions/mod.rs`**: Task 02 (already done) had added the action handler in `actions/mod.rs`, so no changes were needed there. The new variant in `handler/mod.rs` was the only addition needed for the enum definition.
2. **Handler function signature uses `&str` / `&Path`**: The `handle_config_generated` function takes string slices rather than owned strings to match the pattern used by other handlers in `dap.rs`, avoiding unnecessary clones at the call site.
3. **`DapConfigStatus` placement**: Added in the DAP section of `state.rs` just before `DapStatus` definitions for logical grouping.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all tests pass: 1367 fdemon-app, 360 fdemon-core, 460 fdemon-daemon, 581 fdemon-dap, 796 fdemon-tui, plus integration tests)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **`dap_config_status` persists across restarts**: As noted in the task, the status field persists until the next DAP server restart. This is by design — TUI rendering of this field is deferred to Task 11.
