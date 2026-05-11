## Task: Store DevTools Endpoint on `Session`

**Objective**: Add a `devtools_endpoint: Option<DevToolsEndpoint>` field to `Session` and an internal `Message::DevToolsServed { session_id, host, port }` (and `DevToolsServeFailed`) so the handler layer can populate it.

**Depends on**: 03-protocol-parse-daemon-devtools-event

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/session.rs`: Add a small struct + field on `Session`:
  ```rust
  pub struct DevToolsEndpoint {
      pub host: String,
      pub port: u16,
      /// When this endpoint was last verified (for staleness checks if needed).
      pub served_at: Instant,
  }

  pub struct Session {
      // ... existing fields ...
      pub devtools_endpoint: Option<DevToolsEndpoint>,
      /// True between sending `ServeDevTools` and receiving a response. Used to
      /// avoid duplicate serve requests.
      pub devtools_serve_pending: bool,
  }
  ```
- `crates/fdemon-app/src/message.rs`: Add two `Message` variants:
  - `DevToolsServed { session_id: SessionId, host: String, port: u16 }`
  - `DevToolsServeFailed { session_id: SessionId, reason: String }`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/session.rs`: Existing fields for placement.

### Details

`DevToolsEndpoint` is a small DTO; keep it `Clone + Debug` consistent with neighbors. Put it adjacent to the existing session fields.

```rust
// session.rs
#[derive(Debug, Clone)]
pub struct DevToolsEndpoint {
    pub host: String,
    pub port: u16,
    pub served_at: Instant,
}

impl DevToolsEndpoint {
    pub fn url(&self, ws_uri: &str) -> String {
        let encoded = percent_encode_uri(ws_uri);
        format!("http://{}:{}/?uri={}", self.host, self.port, encoded)
    }
}
```

The `url()` helper centralizes URL construction so tasks 06 and 07 don't duplicate it.

### Acceptance Criteria

1. `Session` has `devtools_endpoint: Option<DevToolsEndpoint>` and `devtools_serve_pending: bool`.
2. `DevToolsEndpoint::url(ws_uri)` produces `http://<host>:<port>/?uri=<encoded>`.
3. `Message::DevToolsServed` and `Message::DevToolsServeFailed` exist with the session-id-bearing shape.
4. New unit tests cover `DevToolsEndpoint::url()` with various inputs (ws://, wss://, ws_uri containing `/ws` suffix).
5. `cargo check --workspace --all-targets` passes.

### Testing

```rust
#[test]
fn devtools_endpoint_url_encodes_ws_uri() {
    let ep = DevToolsEndpoint { host: "127.0.0.1".into(), port: 9100, served_at: Instant::now() };
    let url = ep.url("ws://127.0.0.1:1234/abc=/ws");
    assert_eq!(url, "http://127.0.0.1:9100/?uri=ws%3A%2F%2F127.0.0.1%3A1234%2Fabc%3D%2Fws");
}
```

### Notes

- The `served_at` timestamp lets us add staleness detection later; not used in this fix.
- Re-use the existing `percent_encode_uri` helper from `handler/devtools/mod.rs` (move it to a shared location if needed).
- Keep this task surgical — no behavior change yet, just state shape.
