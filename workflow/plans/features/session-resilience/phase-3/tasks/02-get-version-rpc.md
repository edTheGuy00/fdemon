## Task: Add `getVersion` RPC Method to VmServiceClient

**Objective**: Add a typed `get_version()` method to `VmServiceClient` that calls the Dart VM Service `getVersion` RPC. This is the lightest possible probe (no isolate ID needed) and will be used as a heartbeat ping in task 03.

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/vm_service/protocol.rs`: Add `VersionInfo` response struct
- `crates/fdemon-daemon/src/vm_service/client.rs`: Add `get_version()` method
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Re-export `VersionInfo`

### Details

#### Response Type

Add to `protocol.rs`, near the existing `VmInfo` struct (line ~136):

```rust
/// Response body from the `getVersion` RPC call.
///
/// Returns the Dart VM Service protocol version. This is the lightest
/// possible RPC — no parameters, no isolate context required.
#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    /// VM Service protocol major version.
    pub major: u32,
    /// VM Service protocol minor version.
    pub minor: u32,
}
```

The Dart VM Service `getVersion` response JSON looks like:
```json
{
  "jsonrpc": "2.0",
  "id": "42",
  "result": {
    "type": "Version",
    "major": 4,
    "minor": 16
  }
}
```

The `"type": "Version"` field is present but not needed — `serde` will ignore unknown fields by default with `Deserialize`.

#### Client Method

Add to `VmServiceClient` in `client.rs`, alongside `get_vm()` and `get_isolate()` (after line ~452):

```rust
/// Call `getVersion` — returns the VM Service protocol version.
///
/// This is the lightest possible RPC probe: no parameters, no isolate
/// context. Useful as a heartbeat/liveness check.
///
/// # Errors
///
/// Returns [`Error::VmService`] if the response cannot be parsed as
/// [`VersionInfo`], or a transport error if the request fails.
pub async fn get_version(&self) -> Result<VersionInfo> {
    let result = self.request("getVersion", None).await?;
    serde_json::from_value(result)
        .map_err(|e| Error::vm_service(format!("parse getVersion response: {e}")))
}
```

This follows the exact same pattern as `get_vm()` (line 448) — delegates to `self.request()`, deserializes with `serde_json::from_value`, wraps parse errors in `Error::vm_service`.

#### Re-export

In `crates/fdemon-daemon/src/vm_service/mod.rs`, add `VersionInfo` to the existing `pub use protocol::` block (line 88):

```rust
pub use protocol::{
    parse_vm_message, IsolateGroupRef, IsolateInfo, IsolateRef, LibraryRef, StreamEvent,
    StreamEventParams, VmClientEvent, VmInfo, VmRequestTracker, VmServiceError, VmServiceEvent,
    VmServiceMessage, VmServiceRequest, VmServiceResponse, VersionInfo,
};
```

### Acceptance Criteria

1. `VersionInfo` struct exists in `protocol.rs` with `major: u32` and `minor: u32` fields
2. `VmServiceClient::get_version()` calls `"getVersion"` RPC with no params
3. Return type is `Result<VersionInfo>`
4. Error handling matches the `get_vm()` pattern (parse error -> `Error::vm_service`)
5. `VersionInfo` is re-exported from `vm_service::mod.rs`
6. `cargo check --workspace` passes
7. `cargo clippy --workspace -- -D warnings` clean

### Testing

Add unit tests in `crates/fdemon-daemon/src/vm_service/protocol.rs` (or adjacent test module):

```rust
#[test]
fn test_version_info_deserialize() {
    let json = serde_json::json!({
        "type": "Version",
        "major": 4,
        "minor": 16
    });
    let info: VersionInfo = serde_json::from_value(json).unwrap();
    assert_eq!(info.major, 4);
    assert_eq!(info.minor, 16);
}

#[test]
fn test_version_info_deserialize_minimal() {
    // Without the "type" field (just major + minor)
    let json = serde_json::json!({ "major": 3, "minor": 0 });
    let info: VersionInfo = serde_json::from_value(json).unwrap();
    assert_eq!(info.major, 3);
    assert_eq!(info.minor, 0);
}
```

### Notes

- `getVersion` is part of the core VM Service protocol (not an extension), so it's always available
- Unlike `getVM`, `getVersion` does NOT require the VM to enumerate isolates, making it even cheaper
- The method is on `VmServiceClient` (not `VmRequestHandle`) because the heartbeat runs inside `forward_vm_events` which owns the client. If needed on `VmRequestHandle` too, the same `self.request("getVersion", None)` pattern works since both share the `request()` method signature.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/protocol.rs` | Added `VersionInfo` struct with `major: u32` and `minor: u32` fields; added 3 unit tests (`test_version_info_deserialize`, `test_version_info_deserialize_minimal`, `test_version_info_deserialize_missing_fields_fails`) |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added `VersionInfo` to the `use super::protocol::` import block; added `get_version()` async method to `VmServiceClient` |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Added `VersionInfo` to the `pub use protocol::` re-export block |

### Notable Decisions/Tradeoffs

1. **Placement of `VersionInfo`**: Placed before `VmInfo` in protocol.rs in the "VM / Isolate information types" section, matching the task's instruction to add it "near the existing `VmInfo` struct". This groups all VM-level response types together.
2. **Import addition in client.rs**: Added `VersionInfo` to the existing `use super::protocol::` block rather than a separate import — consistent with project style.
3. **Missing-fields test**: Added a third test (`test_version_info_deserialize_missing_fields_fails`) beyond the two specified in the task, verifying that deserialization fails when both `major` and `minor` are absent. This improves test coverage of the error path.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)
- `cargo test -p fdemon-daemon` - Passed (378 passed, 3 ignored, 0 failed)
- `cargo test -p fdemon-daemon version_info` - Passed (3/3 new tests)

### Risks/Limitations

1. **No integration test**: `get_version()` is not covered by an integration test that actually connects to a Dart VM — this is consistent with the rest of the codebase where WebSocket-level methods are not integration-tested in the unit test suite.
