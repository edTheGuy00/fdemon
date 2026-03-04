## Task: Consolidate CLI DAP Settings Override

**Objective**: Replace the duplicated dual-mutation pattern in both runners with a single `Engine::apply_cli_dap_override(port)` method that atomically updates both `engine.settings` and `engine.state.settings`.

**Depends on**: merge (post-merge improvement)

**Priority**: MEDIUM

**Review Source**: REVIEW.md Issue #4 (Architecture Enforcer, Code Quality Inspector)

### Scope

- `crates/fdemon-app/src/engine.rs`: Add `apply_cli_dap_override` method
- `crates/fdemon-tui/src/runner.rs`: Replace inline mutation (lines 79-88)
- `src/headless/runner.rs`: Replace inline mutation (lines 37-48)

### Background

Both TUI and headless runners apply `--dap-port` by directly writing to two independent copies of settings:

```rust
// TUI runner (runner.rs:81-86) and headless runner (runner.rs:39-44) — identical pattern:
if let Some(port) = dap_port {
    engine.settings.dap.port = port;
    engine.settings.dap.enabled = true;
    engine.state.settings.dap.port = port;
    engine.state.settings.dap.enabled = true;
}
```

`engine.settings` (the cached `Settings` in Engine) and `engine.state.settings` (the clone embedded in `AppState`) are independent copies created during `Engine::new()`. Writing to only one would cause the DAP handler (reads `state.settings`) and `should_auto_start_dap` (reads `engine.settings`) to disagree. The current dual-write works but is fragile and violates DRY.

### Details

#### 1. Add Engine Method

In `crates/fdemon-app/src/engine.rs`, add:

```rust
/// Apply a CLI `--dap-port` override.
///
/// Sets the DAP port and forces `enabled = true` in both the cached
/// settings and the embedded AppState settings, keeping them in sync.
pub fn apply_cli_dap_override(&mut self, port: u16) {
    self.settings.dap.port = port;
    self.settings.dap.enabled = true;
    self.state.settings.dap.port = port;
    self.state.settings.dap.enabled = true;
    tracing::info!("DAP server port overridden by --dap-port: {}", port);
}
```

#### 2. Update TUI Runner

Replace the inline block in `crates/fdemon-tui/src/runner.rs` (lines 79-88) with:

```rust
if let Some(port) = dap_port {
    engine.apply_cli_dap_override(port);
}
```

#### 3. Update Headless Runner

Replace the inline block in `src/headless/runner.rs` (lines 37-48) with:

```rust
if let Some(port) = dap_port {
    engine.apply_cli_dap_override(port);
}
```

### Acceptance Criteria

1. `Engine::apply_cli_dap_override` exists and updates both settings copies
2. Both runners call the new method instead of inline mutation
3. Existing behavior is identical — `--dap-port` still works in both TUI and headless modes
4. `cargo test --workspace` passes
5. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new unit test needed — this is a pure refactor with no behavior change. Existing tests for DAP auto-start and settings cover the correctness. If desired, add a unit test on `Engine` that calls `apply_cli_dap_override` and verifies both `engine.settings.dap` and `engine.state.settings.dap` are updated.

### Notes

- If future CLI flags need similar dual-write behavior (e.g., a `--bind-address` override), this pattern can be extended to a more general `apply_cli_overrides` method.
- The `tracing::info!` log is moved into the Engine method to keep it centralized.
