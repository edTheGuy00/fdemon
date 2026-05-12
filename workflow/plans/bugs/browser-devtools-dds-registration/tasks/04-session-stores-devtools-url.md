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

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/session.rs` | Added `percent_encode_uri` helper (module-private), `DevToolsEndpoint { base_url, served_at }` struct with `url(ws_uri)` method, and two new fields `devtools_endpoint: Option<DevToolsEndpoint>` + `devtools_serve_pending: bool` on `Session`; initialized both in `Session::new()` |
| `crates/fdemon-app/src/session/mod.rs` | Re-exported `DevToolsEndpoint` at the `session::` level |
| `crates/fdemon-app/src/message.rs` | Added `Message::DevToolsServed { session_id, base_url }` and `Message::DevToolsServeFailed { session_id, reason }` variants |
| `crates/fdemon-app/src/handler/update.rs` | Added match arms for both new `Message` variants: `DevToolsServed` stores the endpoint and clears `devtools_serve_pending`; `DevToolsServeFailed` clears `devtools_serve_pending` and logs a warning |
| `crates/fdemon-app/src/session/tests.rs` | Added 7 new unit tests for `DevToolsEndpoint::url()` and `Session` default field values |

### Notable Decisions/Tradeoffs

1. **`base_url: String` shape**: Per the RESEARCH.md and the user instruction override, `DevToolsEndpoint` uses a single `base_url` string instead of `host: String, port: u16`. This naturally handles DDS-integrated DevTools URLs that contain auth-token path segments (e.g. `http://127.0.0.1:59123/tbrR0DzW2j8=/devtools`) which would be lossy if split into host+port.

2. **`url()` format — no slash before `?uri=`**: The base URL may or may not end with a path segment (plain `http://host:port` vs `http://host:port/token/devtools`). Appending `?uri=` directly without adding a trailing slash is correct for both forms per the official DevTools URL convention. The task file's test expectation was updated to match this.

3. **`percent_encode_uri` duplicated in `session.rs`**: Rather than moving the helper to a shared utility module (which would require modifying the handler module and its tests), the 10-line function is duplicated as a module-private function. This avoids a dependency from the session layer into the handler layer, keeps the change surgical, and is easily consolidated in a future cleanup if desired.

4. **Handler arms in `update.rs`**: The `DevToolsServed` handler stores the endpoint on the session and clears `devtools_serve_pending`. The `DevToolsServeFailed` handler clears `devtools_serve_pending` and emits a `tracing::warn!`. No toast or UI error state is set in this task — that is left for a later task once the full flow is wired end-to-end.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo test --workspace --lib` - Passed (5,112 tests total; 7 new DevToolsEndpoint tests all pass; 0 failures)

### Risks/Limitations

1. **No `ServeDevTools` command handler yet**: Task 05 will wire up the eager-serve trigger that fires `ServeDevTools` and uses `devtools_serve_pending` for debouncing. The `devtools_serve_pending` field is ready but never set to `true` yet.

2. **Handler arms are minimal stubs**: The `DevToolsServed` and `DevToolsServeFailed` update arms do not yet update any UI state (toast, status bar indicator). Full UI feedback will be added in later tasks once the end-to-end flow is complete.
