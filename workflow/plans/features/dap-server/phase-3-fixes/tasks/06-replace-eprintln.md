## Task: Replace `eprintln!` with `tracing::info!`

**Objective**: Replace the 5 `eprintln!` calls in the `SpawnDapServer` action handler with `tracing::info!` to comply with CODE_STANDARDS.md ("NEVER use `println!` or `eprintln!`"). Also fix the hard-coded `127.0.0.1` in the output to use the actual bind address.

**Depends on**: None

**Estimated Time**: 1–2 hours

**Severity**: MAJOR — violates project coding standards; sets bad precedent.

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Lines 444–458

### Details

#### Current Code

```rust
// actions/mod.rs:444-458
// Print DAP connection info to stderr so IDE users can
// find the port without navigating the TUI status bar.
eprintln!("DAP server listening on 127.0.0.1:{}", actual_port);
eprintln!("Connect with:");
eprintln!(
    "  Zed:   set port {} in .zed/debug.json tcp_connection",
    actual_port
);
eprintln!("  Helix: :debug-remote 127.0.0.1:{}", actual_port);
eprintln!("  nvim:  set port {} in dap.adapters config", actual_port);
```

#### Issues

1. **`eprintln!`**: CODE_STANDARDS.md forbids it. TUI owns stdout; `tracing` owns structured logging.
2. **Hard-coded `127.0.0.1`**: The actual bind address comes from `DapServerConfig::bind_addr`, but the output ignores it. If configured for `0.0.0.0`, the printed address is misleading.

#### Fix

Replace with `tracing::info!`:

```rust
tracing::info!(
    port = actual_port,
    bind_addr = %bind_addr,
    "DAP server listening on {}:{}",
    bind_addr, actual_port
);
tracing::info!(
    "Connect with: Zed (port {} in .zed/debug.json), Helix (:debug-remote {}:{}), nvim (port {} in dap.adapters)",
    actual_port, bind_addr, actual_port, actual_port
);
```

The `bind_addr` variable should already be available in scope from the `SpawnDapServer` action arguments. Verify and use the actual configured address.

#### Stderr Visibility Concern

The original comment argues `eprintln!` is needed because `tracing` goes to a log file. However:
- In TUI mode, the DAP server port is shown in the status bar — users don't need stderr.
- In headless mode, `tracing` output goes to stderr via the subscriber. `tracing::info!` with a stderr subscriber achieves the same visibility.
- If a dedicated stderr subscriber isn't configured for headless mode, that's a separate issue to address in the logging infrastructure, not by bypassing `tracing`.

### Acceptance Criteria

1. No `eprintln!` calls in `crates/fdemon-app/src/actions/mod.rs`
2. DAP server connection info is logged via `tracing::info!`
3. The logged address uses the actual `bind_addr`, not hard-coded `127.0.0.1`
4. `cargo clippy --workspace` passes
5. Grep for `eprintln!` across all library crates returns no matches

### Testing

- Existing tests pass
- Verify with `RUST_LOG=info cargo run -- --headless` that DAP server info appears in tracing output

### Notes

- After this fix, grep the entire workspace for any remaining `eprintln!` or `println!` in library crates and fix if found.
- The 5 `eprintln!` calls can be consolidated into 1–2 `tracing::info!` calls for atomicity and cleaner output.
