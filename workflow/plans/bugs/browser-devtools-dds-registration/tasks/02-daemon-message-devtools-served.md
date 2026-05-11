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
