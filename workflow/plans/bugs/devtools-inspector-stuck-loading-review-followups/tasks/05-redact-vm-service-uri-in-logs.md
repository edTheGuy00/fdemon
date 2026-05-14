## Task: Redact VM Service Auth Token in Log Output

**Objective**: Stop logging Dart VM Service WebSocket URIs in plain text. The URI's path component contains an auth token; anyone reading log files can obtain the token and execute arbitrary VM Service RPCs (hot reload, read heap, invoke service extensions).

**Depends on**: 01-cache-fallback-isolate-resolution (both write `vm_service/client.rs` — schedule sequentially)

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/vm_service/mod.rs` (or a new sibling file like `vm_service/redact.rs`) — add `redact_vm_service_token(uri: &str) -> String` helper
- `crates/fdemon-daemon/src/vm_service/client.rs:515` — apply redaction to `info!("Connecting to VM Service at ...")`
- `crates/fdemon-app/src/actions/vm_service.rs:54-57` — apply redaction to the timeout `warn!`

**Files Read (Dependencies):**
- None (no existing redaction helper in the codebase — verified by grep)

### Details

The Dart VM Service WebSocket URI has the form:
```
ws://127.0.0.1:PORT/AUTH_TOKEN/ws
```
Where `AUTH_TOKEN` is a random session token that authorizes RPC calls. The token is the first path segment.

**Helper function (new, in `crates/fdemon-daemon/src/vm_service/`):**
```rust
/// Redact the auth token from a Dart VM Service WebSocket URI.
///
/// The URI's first path segment is the auth token; replace it with
/// `[REDACTED]` for safe logging.
///
/// Returns the URI unchanged if it doesn't match the expected shape
/// (defensive — should not block logging on unexpected input).
pub(crate) fn redact_vm_service_token(uri: &str) -> String {
    // Parse host + port; replace first path segment with [REDACTED].
    // ws://127.0.0.1:PORT/AUTH_TOKEN/ws → ws://127.0.0.1:PORT/[REDACTED]/ws
    // ...
}
```

Implementation suggestion: use `url::Url` (already a workspace dependency via `tungstenite`) to parse, replace the first path segment, and serialize back. Or implement string-only if the URL shape is sufficiently constrained.

**Apply at log sites:**
```rust
// client.rs:515
let safe_uri = redact_vm_service_token(ws_uri);
info!("Connecting to VM Service at {}", safe_uri);

// actions/vm_service.rs:54-57
let safe_uri = redact_vm_service_token(&ws_uri);
warn!("VM Service: connection timed out for session {} ({})", session_id, safe_uri);
```

### Acceptance Criteria

1. A `redact_vm_service_token` helper exists, takes `&str`, returns `String`.
2. Helper correctly handles:
   - Normal Dart VM Service URI: `ws://127.0.0.1:8080/AbC123/ws` → `ws://127.0.0.1:8080/[REDACTED]/ws`
   - URI without auth path: `ws://127.0.0.1:8080/ws` → unchanged or sensibly redacted
   - Malformed input: returns input unchanged (does not panic, does not block logging)
3. Both log sites (`client.rs:515` and `actions/vm_service.rs:54-57`) emit redacted output. Verifiable by `git grep -E 'ws_uri\s*\)' crates/` returning no production `info!`/`warn!`/`error!` matches.
4. Unit tests for the helper cover all three input shapes.
5. No regression in connection behavior — the redaction is logging-only; `ws_uri` itself is still passed unchanged to the WebSocket library.

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::redact_vm_service_token;

    #[test]
    fn test_redact_normal_vm_service_uri() {
        let raw = "ws://127.0.0.1:8080/AbCdEf123XyZ/ws";
        let red = redact_vm_service_token(raw);
        assert!(!red.contains("AbCdEf123XyZ"));
        assert!(red.contains("[REDACTED]"));
        assert!(red.starts_with("ws://127.0.0.1:8080/"));
    }

    #[test]
    fn test_redact_uri_without_path_returns_unchanged() {
        let raw = "ws://127.0.0.1:8080/";
        let red = redact_vm_service_token(raw);
        // No auth token → safe to leave as-is
        assert_eq!(red, raw);
    }

    #[test]
    fn test_redact_malformed_uri_does_not_panic() {
        // Should not panic; output is implementation-defined
        let _ = redact_vm_service_token("not a uri");
        let _ = redact_vm_service_token("");
    }
}
```

### Notes

- Visibility: `pub(crate)` is appropriate — only daemon and app layers need this helper.
- The previous review (`workflow/reviews/bugs/browser-devtools-dds-registration/REVIEW.md:79`) flagged this category; this task closes the gap permanently.
- Do NOT demote the `info!` to `debug!` as a workaround — the connection event is genuinely useful at `info!` level. The redaction is the correct fix.
- If you need to reach into the daemon crate's helper from `fdemon-app/src/actions/vm_service.rs`, ensure the helper is exported as `pub fn` from `fdemon-daemon`'s `vm_service` module (or re-exported from the daemon crate root).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a1af140323ddbf685

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Added `pub fn redact_vm_service_token(uri: &str) -> String` with 6 unit tests and 1 doc-test |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Applied `super::redact_vm_service_token(ws_uri)` at the `info!` log site (line 520) |
| `crates/fdemon-app/src/actions/vm_service.rs` | Imported `redact_vm_service_token` from `fdemon_daemon::vm_service`; applied redaction at the timeout `warn!` site |

### Notable Decisions/Tradeoffs

1. **Placement in `vm_service/mod.rs` not a new file**: The helper is small enough to live directly in `mod.rs` as `pub fn`. A separate `redact.rs` would be premature — `mod.rs` already exports many small helpers.
2. **String parsing over `url` crate**: Consistent with `fdemon-core/src/url.rs` patterns. No need to add the `url` crate to `fdemon-daemon`'s Cargo.toml — the URI structure is sufficiently constrained for string manipulation.
3. **`pub fn` visibility (not `pub(crate)`)**: The task notes suggest `pub(crate)` but `fdemon-app` is a different crate and imports via `fdemon_daemon::vm_service`, so `pub` is required. The function does not leak security-sensitive data itself (it only handles redaction) so `pub` is safe.
4. **`wss://` scheme supported**: Added defensively alongside `ws://` for completeness and forward-compatibility.
5. **URI-without-auth-token returns unchanged**: A URI with only one path segment (e.g. `ws://127.0.0.1:8080/ws`) has no auth token to redact — it is returned as-is, matching acceptance criterion 2.

### Testing Performed

- `cargo test -p fdemon-daemon -- redact` — 6 unit tests + 1 doc-test: PASS
- `cargo test --workspace --lib` — all 1,018+ tests: PASS (0 failures)
- `cargo clippy -p fdemon-daemon -p fdemon-app` — PASS (no warnings)
- `cargo build --workspace` — PASS
- `git grep -En 'ws_uri' crates/` filtered through log macros — no raw `ws_uri` at production log sites

### Risks/Limitations

1. **IPv6 URIs**: Not tested, but the string approach handles them correctly since it finds the first `/` after the scheme prefix, which works regardless of whether the authority is an IP, hostname, or bracketed IPv6 address.
2. **No re-export from daemon crate root**: `redact_vm_service_token` is accessible via `fdemon_daemon::vm_service::redact_vm_service_token`. The task notes mention optionally re-exporting from daemon root — omitted as `fdemon-app` already imports many items from the `vm_service` sub-module path directly.
