## Task: HeadlessEvent DAP Variant and Remove Unused Dependency

**Objective**: Route DAP server port output through the `HeadlessEvent` pattern instead of a standalone function, and remove the unused `fdemon-daemon` dependency from `fdemon-dap`.

**Depends on**: merge (post-merge improvement)

**Priority**: LOW

**Review Source**: REVIEW.md Issues #10, #11 (Architecture Enforcer)

### Scope

- `src/headless/mod.rs`: Add `DapServerStarted` variant to `HeadlessEvent`
- `src/headless/runner.rs`: Replace `emit_dap_port_json` with `HeadlessEvent` usage
- `crates/fdemon-dap/Cargo.toml`: Remove `fdemon-daemon` dependency

### Background

**HeadlessEvent bypass** (Issue #11): The `emit_dap_port_json` function at `runner.rs:145-159` writes a manually constructed JSON object directly to stdout:

```rust
fn emit_dap_port_json(port: u16) {
    let json = json!({ "event": "dap_server_started", "dapPort": port });
    let mut stdout = std::io::stdout().lock();
    // ...
}
```

All other headless events use the `HeadlessEvent` enum (defined in `headless/mod.rs:28-105`) which serializes via serde with a `#[serde(tag = "event", rename_all = "snake_case")]` tag. The DAP port output should follow this pattern for consistency and to benefit from the enum's type-safe serialization.

**Unused dependency** (Issue #10): `crates/fdemon-dap/Cargo.toml` declares `fdemon-daemon.workspace = true` but no code in `fdemon-dap` imports from `fdemon-daemon` in Phase 2. This adds unnecessary compile time.

### Details

#### 1. Add HeadlessEvent Variant

In `src/headless/mod.rs`, add to the `HeadlessEvent` enum:

```rust
DapServerStarted {
    port: u16,
    timestamp: i64,
},
```

This follows the existing pattern — all variants have a `timestamp: i64` field. The `#[serde(tag = "event", rename_all = "snake_case")]` attribute will serialize it as `{"event": "dap_server_started", "port": 1234, "timestamp": ...}`.

Note: the existing `emit_dap_port_json` uses `"dapPort"` as the field name. The new variant uses `port` which serializes to `"port"` (snake_case). This is a **minor wire format change** — document it. If backward compatibility with `"dapPort"` is needed, add `#[serde(rename = "dapPort")]` on the field.

#### 2. Replace emit_dap_port_json

In `src/headless/runner.rs`, change the `emit_pre_message_events` handler from:

```rust
Message::DapServerStarted { port } => {
    emit_dap_port_json(*port);
}
```

to:

```rust
Message::DapServerStarted { port } => {
    HeadlessEvent::dap_server_started(*port).emit();
}
```

Add a constructor on `HeadlessEvent` (following the pattern of other variants if one exists, or inline the construction).

Remove the `emit_dap_port_json` function entirely.

#### 3. Remove Unused Dependency

In `crates/fdemon-dap/Cargo.toml`, remove:

```toml
fdemon-daemon.workspace = true
```

Run `cargo check -p fdemon-dap` to verify nothing breaks.

### Acceptance Criteria

1. `HeadlessEvent::DapServerStarted { port, timestamp }` variant exists
2. DAP server port is emitted via `HeadlessEvent::emit()` in headless mode
3. `emit_dap_port_json` function is removed
4. `fdemon-daemon` is not in `fdemon-dap/Cargo.toml`
5. `cargo check -p fdemon-dap` passes without `fdemon-daemon`
6. `cargo test --workspace` passes
7. `cargo clippy --workspace -- -D warnings` clean

### Testing

- Verify headless event JSON output format (may need to update integration tests if any assert on `"dapPort"` field name)
- `cargo check -p fdemon-dap` confirms no compile errors after removing dependency

### Notes

- The wire format change (`"dapPort"` -> `"port"`) should be documented if external tools parse headless JSON output. Consider using `#[serde(rename = "dapPort")]` if backward compatibility matters.
- The `fdemon-daemon` dependency will be re-added when Phase 3 implements the VM Service bridge for debug operations.
