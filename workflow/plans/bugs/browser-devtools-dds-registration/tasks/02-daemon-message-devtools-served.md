## Task: Add `DaemonMessage::DevToolsServed` Variant

**Objective**: Extend `DaemonMessage` in `fdemon-core` with a new variant carrying the DevTools server's host + port (and optionally pid). Add a complementary `DevToolsServeFailed { reason }` for the `-32601 Method not found` and other error responses.

**Depends on**: 01-daemon-command-serve-devtools

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/events.rs`: Add two variants to `DaemonMessage`:
  - `DevToolsServed { host: String, port: u16, pid: Option<u32> }`
  - `DevToolsServeFailed { reason: String }`
- Update `Debug`, `Clone`, `PartialEq` derives as needed (match existing variants).

**Files Read (Dependencies):**
- `workflow/plans/bugs/browser-devtools-dds-registration/RESEARCH.md`: For response/event shape.
- `crates/fdemon-core/src/events.rs`: Existing variants for reference.

### Details

```rust
pub enum DaemonMessage {
    // ... existing ...

    /// DevTools server is registered with DDS and reachable at the given host:port.
    /// Emitted by the Flutter daemon either as a response to `daemon.devtools.serve`
    /// or as a `daemon.devtools` event after registration completes.
    DevToolsServed {
        host: String,
        port: u16,
        pid: Option<u32>,
    },

    /// The Flutter daemon could not serve DevTools (e.g., method not supported on this SDK,
    /// or DevTools bundle missing).
    DevToolsServeFailed {
        reason: String,
    },
}
```

If RESEARCH.md indicates a different shape (e.g., `address: String` instead of separate `host`/`port`), align with the verified shape.

### Acceptance Criteria

1. Both new variants exist on `DaemonMessage` with the verified field set.
2. Existing variants are not altered.
3. `cargo check --workspace --all-targets` passes.
4. `cargo clippy --workspace -- -D warnings` passes.

### Testing

No new tests required at this layer — these are pure data types. Integration tests follow in tasks 03 and onward.

### Notes

- Keep field types simple — `String` for host, `u16` for port. No `Url` types.
- If `DaemonMessage` is `Serialize + Deserialize`, ensure the new variants honor that.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/events.rs` | Added `DevToolsServed { app_id, base_url }` and `DevToolsServeFailed { reason }` variants to `DaemonMessage`; updated `app_id()`, `is_error()`, and `summary()` methods to handle the new variants |

### Notable Decisions/Tradeoffs

1. **`base_url: String` instead of `host: String, port: u16`**: RESEARCH.md confirms the primary async channel is the `app.devTools` event which carries a raw base URL string (e.g., `http://127.0.0.1:9100` or `http://127.0.0.1:59123/tbrR0DzW2j8=/devtools`). Storing a single `base_url` avoids reconstruction from host+port and naturally handles DDS-integrated DevTools URLs that include auth token path segments. Task 03 (protocol parser) should extract `params.uri` from `app.devTools` events and store it directly in this field. When handling the `devtools.serve` fallback RPC response (which does carry `host`+`port`), Task 03 should construct the `base_url` via `format!("http://{}:{}", host, port)`.

2. **`app_id: String` field on `DevToolsServed`**: The `app.devTools` event carries both `appId` and `uri` fields. Retaining `app_id` allows callers in `fdemon-app` to correlate the DevTools URL with a specific Flutter session (by matching against `session.app_id`). This is necessary for the multi-session architecture where multiple sessions may be running simultaneously.

3. **`DaemonMessage` is not `Serialize + Deserialize`**: The enum derives `Debug, Clone` only (confirmed by reading the file). The new variants match this pattern — no serde derives needed.

4. **`is_error()` returns `true` for `DevToolsServeFailed`**: This follows the existing convention where error conditions return `true`, making the failed variant detectable via the existing method.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-core` - Passed (372 unit tests + 5 doc-tests; no regressions)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **`devtools.serve` fallback URL construction**: When Task 03 handles the `devtools.serve` RPC response, it must construct `base_url` as `format!("http://{}:{}", host, port)` — the `DevToolsServed` shape does not carry separate host/port fields. This is documented here for Task 03's implementor. The trade-off is a slightly different code path for the fallback but a consistent, simpler shape for all consumers downstream.
