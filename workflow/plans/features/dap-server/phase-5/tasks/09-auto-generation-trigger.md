## Task: Auto-Generation Trigger and Lifecycle Integration

**Objective**: Wire IDE config auto-generation into the DAP server lifecycle so config files are automatically created/updated when the DAP server starts, triggered by `Message::DapServerStarted`, and coordinated through the TEA action system.

**Depends on**: 03-dap-settings-and-messages, 04-vscode-generator, 05-neovim-generator, 06-helix-generator, 07-zed-generator, 08-emacs-generator

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-app/src/handler/dap.rs`: Extend `DapServerStarted` handler to return `UpdateAction::GenerateIdeConfig` when `auto_configure_ide` is enabled
- `crates/fdemon-app/src/actions/mod.rs`: Handle `UpdateAction::GenerateIdeConfig` — call `generate_ide_config()` and send `Message::DapConfigGenerated`
- `crates/fdemon-app/src/engine.rs`: No changes needed (action system already handles dispatching)

### Details

#### 1. Trigger on `DapServerStarted` (`handler/dap.rs`)

Currently, the `DapServerStarted` handler (lines 56–67) sets `dap_status` and returns `UpdateResult::none()`. Extend it to optionally return a `GenerateIdeConfig` action:

```rust
Message::DapServerStarted { port } => {
    state.dap_status = DapStatus::Running {
        port,
        clients: HashSet::new(),
    };
    tracing::info!(
        "DAP server listening on {}:{}",
        state.settings.dap.bind_address,
        port
    );

    // Trigger IDE config generation if enabled
    if state.settings.dap.auto_configure_ide {
        UpdateResult::action(UpdateAction::GenerateIdeConfig { port })
    } else {
        UpdateResult::none()
    }
}
```

#### 2. Handle `GenerateIdeConfig` action (`actions/mod.rs`)

Add a new match arm in the action handler:

```rust
UpdateAction::GenerateIdeConfig { port } => {
    let project_root = engine.project_path.clone();
    let msg_tx = engine.msg_sender();

    tokio::spawn(async move {
        let ide = crate::config::detect_parent_ide();

        match crate::ide_config::generate_ide_config(ide, port, &project_root) {
            Ok(Some(result)) => {
                let action_str = match &result.action {
                    crate::ide_config::ConfigAction::Created => "Created".to_string(),
                    crate::ide_config::ConfigAction::Updated => "Updated".to_string(),
                    crate::ide_config::ConfigAction::Skipped(reason) => {
                        format!("Skipped: {}", reason)
                    }
                };
                let ide_name = ide
                    .map(|i| i.display_name().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                let _ = msg_tx
                    .send(Message::DapConfigGenerated {
                        ide_name,
                        path: result.path,
                        action: action_str,
                    })
                    .await;
            }
            Ok(None) => {
                // No IDE detected or IDE doesn't support DAP config
                tracing::debug!(
                    "No IDE config generated (no IDE detected or IDE unsupported)"
                );
            }
            Err(e) => {
                tracing::warn!("Failed to generate IDE DAP config: {}", e);
            }
        }
    });
}
```

#### 3. Lifecycle considerations

**Port change (toggle off/on):** When the DAP server is toggled off and back on, `DapServerStarted` fires again with potentially a new port. The handler triggers `GenerateIdeConfig` again, which updates the port in the existing config file. This is the correct behavior.

**fdemon exit:** Generated config files are **not cleaned up** on exit. The next fdemon run will update the port if needed. This is documented in the plan.

**No IDE detected:** When `detect_parent_ide()` returns `None`, `generate_ide_config()` returns `Ok(None)`, and no message is sent. A debug-level log is emitted.

**Race condition:** The config generation runs on a spawned task (async) to avoid blocking the TEA cycle. The `DapConfigGenerated` message arrives asynchronously. This is fine — the config file is written immediately, and the message is just for TUI display.

#### 4. Neovim special handling

The Neovim generator writes a secondary `.nvim-dap.lua` file in addition to `.vscode/launch.json`. The `generate_ide_config()` dispatch function handles this — it's encapsulated within the `NeovimGenerator` implementation (Task 05). No special handling is needed in the action handler.

However, when merging (file already exists), the Neovim generator's `merge_config()` only updates `.vscode/launch.json`. The `.nvim-dap.lua` file also needs updating. The dispatch function should call a secondary method on the Neovim generator after merge. Options:

1. Add a `post_write(&self, port, project_root)` method to the `IdeConfigGenerator` trait (default no-op, Neovim overrides)
2. Handle Neovim specifically in the dispatch function

Option 1 is cleaner. Add to the trait:

```rust
/// Optional post-generation hook for secondary file writes.
/// Default implementation is a no-op.
fn post_write(&self, _port: u16, _project_root: &Path) -> crate::Result<()> {
    Ok(())
}
```

The dispatch function calls `generator.post_write(port, project_root)?` after both `generate()` and `merge_config()`.

### Acceptance Criteria

1. `DapServerStarted` handler returns `GenerateIdeConfig` action when `auto_configure_ide` is `true`
2. `DapServerStarted` handler returns no action when `auto_configure_ide` is `false`
3. `GenerateIdeConfig` action spawns async task that calls `generate_ide_config()`
4. Success sends `DapConfigGenerated` message with IDE name, path, and action
5. No IDE detected → no message sent, debug log emitted
6. Error → warning logged, no crash, no message sent
7. Config is regenerated on DAP server restart (port update)
8. `post_write()` hook is called for Neovim's `.nvim-dap.lua` updates
9. `cargo check --workspace` — Pass
10. `cargo test --workspace` — Pass
11. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_dap_server_started_triggers_ide_config_when_enabled() {
    let mut state = AppState::default();
    state.settings.dap.auto_configure_ide = true;
    let result = handle_dap_message(&mut state, &Message::DapServerStarted { port: 4711 });
    assert!(matches!(
        result.action,
        Some(UpdateAction::GenerateIdeConfig { port: 4711 })
    ));
}

#[test]
fn test_dap_server_started_skips_ide_config_when_disabled() {
    let mut state = AppState::default();
    state.settings.dap.auto_configure_ide = false;
    let result = handle_dap_message(&mut state, &Message::DapServerStarted { port: 4711 });
    assert!(result.action.is_none());
}

#[test]
fn test_dap_server_started_still_sets_status() {
    let mut state = AppState::default();
    state.settings.dap.auto_configure_ide = true;
    handle_dap_message(&mut state, &Message::DapServerStarted { port: 4711 });
    assert!(matches!(
        state.dap_status,
        DapStatus::Running { port: 4711, .. }
    ));
}

// Integration test using tempdir
#[test]
fn test_generate_ide_config_in_vscode_terminal() {
    let dir = tempdir().unwrap();
    // Simulate VS Code detection
    let result = generate_ide_config(Some(ParentIde::VSCode), 4711, dir.path()).unwrap();
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.action, ConfigAction::Created);
    assert!(dir.path().join(".vscode/launch.json").exists());
}

#[test]
fn test_generate_ide_config_no_ide() {
    let dir = tempdir().unwrap();
    let result = generate_ide_config(None, 4711, dir.path()).unwrap();
    assert!(result.is_none());
}
```

### Notes

- The action is handled asynchronously (`tokio::spawn`) to avoid blocking the TEA cycle with file I/O. This is consistent with how other actions (e.g., `SpawnDapServer`) are handled.
- The `detect_parent_ide()` call happens inside the spawned task, not in the handler. This is because env var detection is cheap but logically belongs with the config generation flow.
- If the user toggles `auto_configure_ide` off in settings while the DAP server is running, existing config files are left in place. They are only generated/updated when the server starts.
- The `post_write()` trait hook is a minor trait extension. If it feels over-engineered, the alternative is to handle Neovim specifically in the dispatch function with `if matches!(ide, ParentIde::Neovim) { ... }`.
