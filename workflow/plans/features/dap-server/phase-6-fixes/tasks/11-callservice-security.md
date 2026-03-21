## Task: callService Security Documentation and Audit Logging

**Objective**: Document the intentional full-passthrough design of `callService` and add structured audit logging for all forwarded RPCs, addressing finding M3.

**Depends on**: 03-hot-operation-refactor (shared file: handlers.rs)

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Add structured audit logging to `handle_call_service`

**Files Read (Dependencies):**
- None

### Details

#### Design Decision: No Allowlist

After research, an allowlist is not the right approach for `callService`:

1. The Dart VM Service API surface is large and version-dependent — maintaining an allowlist would be a constant maintenance burden
2. The VS Code Dart extension uses `callService` to forward `ext.flutter.*` extensions, `getSourceReport`, and other methods that change across Flutter/Dart versions
3. The VM Service itself validates method names and returns errors for unknown methods
4. The DAP server is localhost-only by default, limiting the attack surface

The correct approach is: **document the design decision and add audit logging**.

#### Fix: Add Structured Audit Logging

Currently `handle_call_service` has a `debug!` log at line 1163. Upgrade it to structured `info!`-level logging that captures the method name, whether params were provided, and the result:

```rust
pub(super) async fn handle_call_service(&mut self, request: &DapRequest) -> DapResponse {
    // ... parse method and params ...

    tracing::info!(
        method = method,
        has_params = params.is_some(),
        "callService: forwarding VM Service RPC"
    );

    match with_timeout(self.backend.call_service(method, params)).await {
        Ok(result) => {
            tracing::debug!(method = method, "callService: success");
            DapResponse::success(request, Some(result))
        }
        Err(e) => {
            tracing::warn!(method = method, error = %e, "callService: failed");
            DapResponse::error(request, format!("callService '{}' failed: {}", method, e))
        }
    }
}
```

Also add a doc comment on `handle_call_service` explaining the security model:

```rust
/// Handles the `callService` custom DAP request by forwarding an arbitrary
/// VM Service RPC call to the connected Dart VM.
///
/// # Security Model
///
/// This handler intentionally does NOT filter or restrict the `method` parameter.
/// The VM Service itself validates method names and handles authorization.
/// The DAP server is bound to localhost by default (`127.0.0.1`), limiting access
/// to local processes. When the server is bound to a non-loopback address, a warning
/// is emitted at startup. All forwarded RPCs are logged at `info` level for audit.
///
/// If stronger isolation is needed, enable `require_auth` in the DAP server
/// configuration to require an auth token in the `initialize` handshake.
```

### Acceptance Criteria

1. `callService` has a doc comment explaining the security model and design rationale
2. Forwarded RPCs are logged at `info` level with the method name
3. Failed RPCs are logged at `warn` level with the error
4. No behavioral change — `callService` continues to forward all methods
5. Existing tests pass: `cargo test -p fdemon-dap`
6. `cargo clippy -p fdemon-dap` clean

### Testing

No new tests needed — this is documentation and logging only. Existing `callService` tests should pass unchanged.

### Notes

- The `info!` level is intentional: `callService` is a privileged operation that should appear in default logs, not just debug-level traces. This provides audit trail for security-conscious environments.
- If task 10 (DAP server auth) is implemented, the doc comment should reference it as the recommended mitigation for untrusted environments.
