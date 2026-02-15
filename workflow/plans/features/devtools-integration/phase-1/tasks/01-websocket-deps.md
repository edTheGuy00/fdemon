## Task: Add WebSocket Dependencies

**Objective**: Add `tokio-tungstenite` and `futures-util` to the Cargo workspace and `fdemon-daemon` crate to enable async WebSocket communication with the Dart VM Service.

**Depends on**: None

**Estimated Time**: 1 hour

### Scope

- `Cargo.toml` (workspace root): Add `tokio-tungstenite` and `futures-util` to `[workspace.dependencies]`
- `crates/fdemon-daemon/Cargo.toml`: Add `tokio-tungstenite.workspace = true` and `futures-util.workspace = true`

### Details

Add the following workspace dependencies:

```toml
# In [workspace.dependencies]
tokio-tungstenite = { version = "0.26", features = ["native-tls"] }
futures-util = "0.3"
```

Then reference them from `fdemon-daemon`:

```toml
# In crates/fdemon-daemon/Cargo.toml [dependencies]
tokio-tungstenite.workspace = true
futures-util.workspace = true
```

**Why `fdemon-daemon`?** The VM Service client is I/O-level infrastructure (WebSocket connection to the Dart VM), same layer as `FlutterProcess` which manages the Flutter daemon's stdin/stdout. This keeps `fdemon-app` free of I/O concerns.

**Why `native-tls`?** Some Flutter DevTools setups may use HTTPS/WSS URIs. The `native-tls` feature provides TLS support using the OS-native TLS library.

### Acceptance Criteria

1. `cargo check --workspace` passes with no errors
2. `tokio-tungstenite` is available in `fdemon-daemon` (can be imported)
3. `futures-util` is available in `fdemon-daemon` (can be imported)
4. No other crates pull in the new dependencies unnecessarily
5. `cargo clippy --workspace -- -D warnings` passes

### Testing

No new tests needed — this is a dependency addition. Verified by successful compilation.

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

### Notes

- `serde_json` is already a workspace dependency — no need to add it
- `tokio` is already a workspace dependency — no need to add it
- The `url` crate may be useful for URI parsing but is not strictly needed in Phase 1 (ws_uri is a plain string)

---

## Completion Summary

**Status:** Not Started
