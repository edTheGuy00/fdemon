## Task: Thread `--dap-config` CLI Override Through Combined Mode

**Objective**: Wire the `--dap-config <IDE>` CLI argument through the Engine/AppState so that `handle_started()` can pass it as `ide_override: Some(ide)` to `GenerateIdeConfig`. Currently the parsed value is validated then discarded, making the feature silently non-functional in combined mode.

**Depends on**: None

**Severity**: Critical

### Scope

- `crates/fdemon-app/src/state.rs`: Add `cli_dap_config_override: Option<ParentIde>` field to `AppState`
- `crates/fdemon-app/src/engine.rs`: Add `apply_cli_dap_config_override(ide: ParentIde)` method (mirrors existing `apply_cli_dap_override(port)`)
- `crates/fdemon-app/src/handler/dap.rs`: Read `state.cli_dap_config_override` in `handle_started()` and pass to `GenerateIdeConfig`
- `src/main.rs`: Store parsed `ParentIde` and pass to runner functions
- `src/tui/runner.rs`: Accept `dap_config: Option<ParentIde>` parameter and call `engine.apply_cli_dap_config_override()`
- `src/headless/runner.rs`: Same as tui runner

### Details

**Current flow (broken):**

```
main.rs:134  parse_ide_name("neovim") → Ok(ParentIde::Neovim)  [DROPPED]
main.rs:152  run_with_project_and_dap(&path, args.dap_port)     [dap_config absent]
runner.rs    Engine::new(path)                                   [no override field]
dap.rs:76    GenerateIdeConfig { ide_override: None }            [always None]
actions.rs   detect_parent_ide()                                 [env fallback → may be None]
```

**Target flow (fixed):**

```
main.rs      parse_ide_name("neovim") → Ok(ParentIde::Neovim)   [STORED]
main.rs      run_with_project_and_dap(&path, dap_port, Some(Neovim))
runner.rs    engine.apply_cli_dap_config_override(Neovim)
             → state.cli_dap_config_override = Some(Neovim)
dap.rs       GenerateIdeConfig { ide_override: Some(Neovim) }   [from state]
actions.rs   ide_override.or_else(|| detect_parent_ide())        [uses Neovim]
```

**Step-by-step:**

1. Add field to `AppState`:
   ```rust
   /// CLI-provided IDE override for DAP config generation (`--dap-config <ide>`).
   /// When set, bypasses environment-based IDE detection.
   pub cli_dap_config_override: Option<crate::config::ParentIde>,
   ```

2. Add method to `Engine` (follow the pattern of `apply_cli_dap_override`):
   ```rust
   /// Apply a CLI-provided IDE config override.
   pub fn apply_cli_dap_config_override(&mut self, ide: crate::config::ParentIde) {
       self.state.cli_dap_config_override = Some(ide);
   }
   ```

3. In `handle_started()` (`handler/dap.rs:74-76`), read the override:
   ```rust
   UpdateResult::action(UpdateAction::GenerateIdeConfig {
       port,
       ide_override: state.cli_dap_config_override,
   })
   ```

4. In `main.rs`, store the parsed value and pass to runners:
   ```rust
   // Combined mode: store the override for later use
   let dap_config_override = if let Some(ref ide_str) = args.dap_config {
       if args.dap_port.is_some() { /* standalone path unchanged */ }
       Some(fdemon_app::ide_config::parse_ide_name(ide_str)?)
   } else {
       None
   };
   ```

5. Update all runner call sites (3 in main.rs) to pass `dap_config_override`.

6. Fix misleading comment at `main.rs:132-133`.

### Acceptance Criteria

1. `fdemon --dap-config neovim` (no `--dap-port`, no `$NVIM` env) generates Neovim config when DAP starts
2. `fdemon --dap-config vscode --dap-port 4711` standalone mode still works (no regression)
3. `fdemon` with no `--dap-config` still falls back to env detection (no regression)
4. The misleading comment at `main.rs:132-133` is corrected or removed
5. Unit test: set the override on `AppState`, call `handle_started()`, verify emitted action has `ide_override: Some(expected_ide)`

### Testing

```rust
#[test]
fn test_handle_started_uses_cli_override() {
    let mut state = test_state();
    state.cli_dap_config_override = Some(ParentIde::Neovim);
    let result = handle_started(&mut state, 12345);
    match result.action {
        Some(UpdateAction::GenerateIdeConfig { ide_override, .. }) => {
            assert_eq!(ide_override, Some(ParentIde::Neovim));
        }
        _ => panic!("expected GenerateIdeConfig action"),
    }
}

#[test]
fn test_handle_started_without_override_emits_none() {
    let mut state = test_state();
    // cli_dap_config_override defaults to None
    let result = handle_started(&mut state, 12345);
    match result.action {
        Some(UpdateAction::GenerateIdeConfig { ide_override, .. }) => {
            assert_eq!(ide_override, None);
        }
        _ => panic!("expected GenerateIdeConfig action"),
    }
}
```

### Notes

- The `Engine::apply_cli_dap_config_override()` method follows the established pattern from `Engine::apply_cli_dap_override()` (port override).
- Storing on `AppState` is the TEA-idiomatic approach — `handle_started()` already has `&mut AppState`.
- Runner function signatures change from `(path, dap_port)` to `(path, dap_port, dap_config)` — check for all call sites in main.rs.

---

## Completion Summary

**Status:** Not Started
