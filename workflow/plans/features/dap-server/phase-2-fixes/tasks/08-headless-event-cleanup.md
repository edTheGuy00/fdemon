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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/headless/mod.rs` | Added `DapServerStarted { port: u16, timestamp: i64 }` variant to `HeadlessEvent` enum; added `dap_server_started(port: u16) -> Self` constructor; added `test_dap_server_started_serialization` unit test |
| `src/headless/runner.rs` | Replaced `emit_dap_port_json(*port)` call with `HeadlessEvent::dap_server_started(*port).emit()`; removed `emit_dap_port_json` function entirely; removed unused `use serde_json::json` import |
| `crates/fdemon-dap/Cargo.toml` | Removed `fdemon-daemon.workspace = true` from `[dependencies]` |
| `crates/fdemon-dap/src/lib.rs` | Updated module doc comment to reflect that `fdemon-daemon` is no longer a dependency |
| `src/main.rs` | Updated CLI arg doc comment to reflect new wire format (`"port"` instead of `"dapPort"`) |

### Notable Decisions/Tradeoffs

1. **Wire format change**: The field name changed from `"dapPort"` to `"port"` (snake_case, per `HeadlessEvent` serde convention). No integration tests asserted on `"dapPort"`, so no backward compatibility annotation (`#[serde(rename = "dapPort")]`) was added. The doc comment in `src/main.rs` was updated to reflect the new format.
2. **Doc comment in `runner.rs`**: The `run_headless` function's doc comment already used the new format (`"port"` not `"dapPort"`), so no change was needed there.

### Testing Performed

- `cargo check -p fdemon-dap` - Passed (fdemon-dap compiles without fdemon-daemon)
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all tests pass; new `test_dap_server_started_serialization` test runs clean)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Wire format change**: External tools that parse headless JSON output and check for `"dapPort"` will need to be updated to use `"port"`. This is documented in the task and noted in the `src/main.rs` CLI arg doc comment.
