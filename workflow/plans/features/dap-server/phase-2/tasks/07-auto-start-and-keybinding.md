## Task: Smart Auto-Start Logic and D Keybinding Toggle

**Objective**: Implement the smart auto-start logic that detects the parent IDE and auto-starts the DAP server when appropriate, and add the `D` (uppercase) keybinding in Normal mode to toggle the DAP server on/off at runtime.

**Depends on**: 05 (DapService + CLI), 06 (Status bar display)

### Scope

- `crates/fdemon-app/src/handler/keys.rs` — Add `D` keybinding in Normal mode
- `crates/fdemon-app/src/config/settings.rs` — Use existing `detect_parent_ide()` for auto-start decision
- `crates/fdemon-app/src/engine.rs` — Add auto-start evaluation on Engine init
- `src/tui/runner.rs` — Trigger auto-start after Engine creation
- `src/headless/runner.rs` — Trigger auto-start after Engine creation

### Details

#### 1. Keybinding (`handler/keys.rs`)

Add `D` (uppercase) to `handle_key_normal()`. Insert near the existing `d` (lowercase) for DevTools:

```rust
// 'D' for DAP server toggle (requires active session for context,
// but DAP server is a global service)
InputKey::Char('D') => Some(Message::ToggleDap),
```

Design note: Unlike `d` (DevTools), which requires an active session, the DAP toggle does NOT require an active session because the DAP server can be started before any Flutter session is running (the IDE connects first, then the user starts a session). However, the DAP server is more useful with an active session, so consider whether to add a guard or just allow it always.

Recommendation: Allow `D` toggle always in Normal mode (no guard). The server can start and wait for sessions.

#### 2. Auto-Start Logic

The auto-start decision is evaluated at startup, after the Engine is initialized:

```rust
/// Determine whether the DAP server should auto-start.
///
/// Decision tree:
/// 1. CLI `--dap` flag present? → YES (already handled in Task 05)
/// 2. `dap.enabled = true` in config? → YES
/// 3. IDE detected AND `dap.auto_start_in_ide = true`? → YES
/// 4. None of the above? → NO
pub fn should_auto_start_dap(settings: &Settings) -> bool {
    // Check 1: --dap CLI flag is handled separately (overrides settings.dap.enabled)
    if settings.dap.enabled {
        return true;
    }

    // Check 2: auto_start_in_ide + IDE detection
    if settings.dap.auto_start_in_ide {
        if let Some(ide) = detect_parent_ide() {
            tracing::info!(
                "Detected parent IDE: {} — auto-starting DAP server",
                ide.display_name()
            );
            return true;
        }
    }

    false
}
```

The `detect_parent_ide()` function already exists at `crates/fdemon-app/src/config/settings.rs:73-123`. It detects: VS Code, VS Code Insiders, Cursor, Zed, IntelliJ, Android Studio, Neovim. For Phase 2, this is sufficient. Phase 5 will add Emacs and Helix detection.

#### 3. Runner Integration

In both `src/tui/runner.rs` and `src/headless/runner.rs`, after Engine creation and CLI flag processing:

```rust
// Evaluate DAP auto-start (covers both config-enabled and IDE-detected scenarios).
// --dap-port already sets dap.enabled=true in Task 05, so this covers all cases.
if should_auto_start_dap(&engine.settings) {
    engine.process_message(Message::StartDapServer);
}
```

Since `--dap-port` sets `dap.enabled = true` (Task 05), and `should_auto_start_dap()` checks `dap.enabled` first, there's no double-start issue — the single call handles all startup paths.

#### 4. Interaction Matrix

| Scenario | How DAP starts | User effort |
|----------|:--------------|:------------|
| Running inside VS Code/Neovim/Zed/etc. | Auto-start (IDE detected + `auto_start_in_ide = true`) | Zero config |
| Plain terminal, wants debugging | Press `D` at runtime | One keypress |
| Always want DAP regardless of IDE | Set `dap.enabled = true` in `.fdemon/config.toml` | One-time config |
| CI/scripting with fixed port | `fdemon --dap-port 4711` | CLI flag |
| Don't want DAP ever | Set `auto_start_in_ide = false` in config | One-time config |

#### 5. Headless Mode DAP Port Output

In headless mode, when the DAP server starts, output the port as JSON to stdout:

```rust
// In headless runner, when DapServerStarted is processed:
if let Message::DapServerStarted { port } = &message {
    println!("{}", serde_json::json!({ "dapPort": port }));
}
```

This allows external tooling (CI/CD, IDE plugins) to discover the DAP port programmatically.

### Acceptance Criteria

1. Pressing `D` in Normal mode sends `Message::ToggleDap`
2. `D` keybinding works regardless of whether a session is active
3. `should_auto_start_dap()` returns `true` when `dap.enabled = true`
4. `should_auto_start_dap()` returns `true` when `auto_start_in_ide = true` and an IDE is detected
5. `should_auto_start_dap()` returns `false` when `auto_start_in_ide = false` and `enabled = false`
6. `should_auto_start_dap()` returns `false` when `auto_start_in_ide = true` but no IDE is detected
7. Auto-start triggers `Message::StartDapServer` at Engine startup
8. CLI `--dap` flag and auto-start don't double-start the server
9. In headless mode, DAP port is printed as JSON on startup
10. `D` toggle when DAP is running → stops server, status badge disappears
11. `D` toggle when DAP is off → starts server, status badge appears
12. `cargo check --workspace` passes
13. `cargo test --workspace` passes
14. `cargo clippy --workspace -- -D warnings` clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_d_key_sends_toggle_dap() {
        let mut state = test_state_normal_mode();
        let result = handle_key(&mut state, key_event('D'));
        assert_eq!(result, Some(Message::ToggleDap));
    }

    #[test]
    fn test_d_key_works_without_active_session() {
        let mut state = test_state_normal_mode();
        // No active session
        state.session_manager.clear();
        let result = handle_key(&mut state, key_event('D'));
        assert_eq!(result, Some(Message::ToggleDap));
    }

    #[test]
    fn test_should_auto_start_when_enabled() {
        let mut settings = Settings::default();
        settings.dap.enabled = true;
        assert!(should_auto_start_dap(&settings));
    }

    #[test]
    fn test_should_not_auto_start_when_disabled_no_ide() {
        let settings = Settings::default();
        // Default: enabled=false, auto_start_in_ide=true but no IDE detected
        // In test env, no IDE env vars are set
        assert!(!should_auto_start_dap(&settings));
    }

    #[test]
    fn test_should_not_auto_start_when_auto_start_disabled() {
        let mut settings = Settings::default();
        settings.dap.auto_start_in_ide = false;
        assert!(!should_auto_start_dap(&settings));
    }
}
```

### Notes

- The `detect_parent_ide()` function checks environment variables (`$TERM_PROGRAM`, `$VSCODE_*`, `$NVIM`, etc.). In test environments, these are typically not set, so `should_auto_start_dap()` returns `false` by default. Tests that need to simulate IDE detection should temporarily set env vars (with proper cleanup via `std::env::set_var`/`remove_var`).
- Phase 5 will extend `ParentIde` with `Emacs` (via `$INSIDE_EMACS`) and `Helix` (via `$HELIX_RUNTIME`). The auto-start logic doesn't need to change — it uses `detect_parent_ide()` which returns `Option<ParentIde>`.
- The `D` keybinding uses uppercase to avoid conflicting with `d` (DevTools). This is consistent with other uppercase/lowercase pairs in the keybinding scheme: `r`/`R` (reload/restart), `f`/`F` (level filter/source filter), `e`/`E` (next/prev error).
- In headless mode, `println!` is used for the DAP port JSON output because headless mode doesn't own the terminal (no TUI). This follows the existing pattern for headless event output.
- The auto-start check happens once at startup. If the user changes `dap.auto_start_in_ide` in settings at runtime, it doesn't retroactively start/stop the server — the user must press `D` for runtime toggle.
