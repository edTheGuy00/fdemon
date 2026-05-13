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
