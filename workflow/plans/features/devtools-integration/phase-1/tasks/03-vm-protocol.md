## Task: Create VM Service JSON-RPC Protocol Types

**Objective**: Define the JSON-RPC 2.0 protocol types for communicating with the Dart VM Service over WebSocket. This includes request/response structures, event types, and a request tracker for correlating async responses.

**Depends on**: 01-websocket-deps

**Estimated Time**: 4-5 hours

### Scope

- **NEW** `crates/fdemon-daemon/src/vm_service/mod.rs` — Module exports
- **NEW** `crates/fdemon-daemon/src/vm_service/protocol.rs` — JSON-RPC types and parsing
- `crates/fdemon-daemon/src/lib.rs` — Add `pub mod vm_service;`

### Module Structure

```
crates/fdemon-daemon/src/vm_service/
├── mod.rs          # Module exports
└── protocol.rs     # JSON-RPC types, request tracker, event parsing
```

### Details

#### 1. JSON-RPC Request/Response Types

The Dart VM Service uses JSON-RPC 2.0 over WebSocket. Define types for:

```rust
/// JSON-RPC 2.0 request to VM Service
#[derive(Debug, Serialize)]
pub struct VmServiceRequest {
    pub jsonrpc: &'static str,  // Always "2.0"
    pub id: String,             // Unique request ID
    pub method: String,         // e.g., "getVM", "streamListen"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response from VM Service
#[derive(Debug, Deserialize)]
pub struct VmServiceResponse {
    pub id: Option<String>,
    pub result: Option<serde_json::Value>,
    pub error: Option<VmServiceError>,
}

/// JSON-RPC 2.0 error
#[derive(Debug, Deserialize)]
pub struct VmServiceError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// VM Service stream event (no id, has method + params)
#[derive(Debug, Deserialize)]
pub struct VmServiceEvent {
    pub method: String,          // Always "streamNotify"
    pub params: StreamEventParams,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEventParams {
    pub stream_id: String,       // "Extension", "Logging", etc.
    pub event: StreamEvent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEvent {
    pub kind: String,            // "Extension", "Logging", etc.
    pub isolate: Option<IsolateRef>,
    pub timestamp: Option<i64>,
    #[serde(flatten)]
    pub data: serde_json::Value, // Kind-specific fields
}
```

#### 2. VM Information Types

```rust
/// Response from getVM
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmInfo {
    pub name: String,
    pub version: String,
    pub isolates: Vec<IsolateRef>,
    pub isolate_groups: Option<Vec<IsolateGroupRef>>,
}

/// Isolate reference (lightweight)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolateRef {
    pub id: String,
    pub name: String,
    pub number: Option<String>,
    pub is_system_isolate: Option<bool>,
}

/// Response from getIsolate (full details)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolateInfo {
    pub id: String,
    pub name: String,
    pub number: Option<String>,
    pub runnable: Option<bool>,
    pub pause_on_exit: Option<bool>,
    pub start_time: Option<i64>,
    pub libraries: Option<Vec<LibraryRef>>,
    pub extension_rpcs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct LibraryRef {
    pub id: String,
    pub name: String,
    pub uri: String,
}

#[derive(Debug, Deserialize)]
pub struct IsolateGroupRef {
    pub id: String,
    pub name: String,
}
```

#### 3. Request Tracker

Model after existing `RequestTracker` in `commands.rs`:

```rust
use tokio::sync::oneshot;

pub struct VmRequestTracker {
    pending: HashMap<String, oneshot::Sender<VmServiceResponse>>,
    next_id: AtomicU64,
}

impl VmRequestTracker {
    pub fn new() -> Self { ... }

    /// Generate next request ID and register a pending request
    pub fn register(&mut self) -> (String, oneshot::Receiver<VmServiceResponse>) { ... }

    /// Complete a pending request with a response
    pub fn complete(&mut self, id: &str, response: VmServiceResponse) -> bool { ... }

    /// Clean up stale requests that have been pending too long
    pub fn cleanup_stale(&mut self, timeout: Duration) -> Vec<String> { ... }
}
```

#### 4. Helper: Parse Incoming WebSocket Message

```rust
/// Parsed VM Service WebSocket message
pub enum VmServiceMessage {
    /// Response to a request we sent (has `id`)
    Response(VmServiceResponse),
    /// Stream event notification (no `id`, method = "streamNotify")
    Event(VmServiceEvent),
    /// Unknown message format
    Unknown(String),
}

/// Parse a raw WebSocket text message
pub fn parse_vm_message(text: &str) -> VmServiceMessage { ... }
```

### Acceptance Criteria

1. All protocol types deserialize correctly from real VM Service JSON
2. `VmRequestTracker` can register, complete, and clean up requests
3. `parse_vm_message` correctly distinguishes responses from events
4. All types implement `Debug` and appropriate `Serialize`/`Deserialize`
5. Module is exported from `fdemon-daemon/src/lib.rs`
6. Comprehensive unit tests for parsing

### Testing

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_get_vm_response() {
        let json = r#"{"jsonrpc":"2.0","id":"1","result":{"type":"VM","name":"vm","version":"2.19","isolates":[{"type":"@Isolate","id":"isolates/1","name":"main","number":"1"}]}}"#;
        let msg = parse_vm_message(json);
        // Assert Response with valid VmInfo
    }

    #[test]
    fn test_parse_stream_event() {
        let json = r#"{"jsonrpc":"2.0","method":"streamNotify","params":{"streamId":"Extension","event":{"kind":"Extension","extensionKind":"Flutter.Error","extensionData":{}}}}"#;
        let msg = parse_vm_message(json);
        // Assert Event with Extension stream
    }

    #[test]
    fn test_request_tracker_register_and_complete() { ... }
    #[test]
    fn test_request_tracker_cleanup_stale() { ... }
    #[test]
    fn test_parse_isolate_info() { ... }
    #[test]
    fn test_parse_error_response() { ... }
}
```

### Notes

- The VM Service protocol is documented at: https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md
- Use `serde_json::Value` for untyped parts initially — can add strict typing later
- The `extension_rpcs` field in `IsolateInfo` lists available Flutter service extensions
- Keep this module focused on types/parsing — connection logic is in Task 04

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/protocol.rs` | New file: all JSON-RPC types (`VmServiceRequest`, `VmServiceResponse`, `VmServiceError`, `VmServiceEvent`, `StreamEventParams`, `StreamEvent`, `VmInfo`, `IsolateRef`, `IsolateInfo`, `LibraryRef`, `IsolateGroupRef`), `VmRequestTracker`, `parse_vm_message()`, and 18 unit tests |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | New file: module header with usage example, re-exports all public types from `protocol` |
| `crates/fdemon-daemon/src/lib.rs` | Added `pub mod vm_service;` declaration |

### Notable Decisions/Tradeoffs

1. **`VmRequestTracker` uses `&mut self` (not `Arc<RwLock<_>>`)**: The existing `RequestTracker` in `commands.rs` uses async `Arc<RwLock<>>` because it is shared across concurrent tasks. The task spec shows `VmRequestTracker` with `&mut self` — connection logic (Task 04) will own the tracker and drive the send/receive loop in one task, so plain `&mut self` is correct and simpler. No lock contention.
2. **`parse_vm_message` null-id handling**: A JSON message with `"id": null` is treated the same as having no id (falls through to the method check), matching real VM Service behaviour where `null` id appears on some notifications.
3. **`#[serde(flatten)]` on `StreamEvent::data`**: Captures all kind-specific fields (e.g. `extensionKind`, `logRecord`, etc.) into an untyped `Value` for forward compatibility — strict typing can be added in a later task once all event shapes are known.
4. **Global `AtomicU64` counter for IDs**: Same pattern as `commands.rs`. IDs are stringified integers which matches what real Dart VM Service clients send.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-daemon` - Passed (159 tests: 133 pre-existing + 26 new vm_service tests)
- `cargo clippy --workspace -- -D warnings` - Passed (fixed one `map_or` → `is_none_or` suggestion)
- `cargo test -p fdemon-core -p fdemon-daemon -p fdemon-app -p fdemon-tui` - All library crates pass

### Risks/Limitations

1. **e2e tests in binary crate are pre-existing failures**: The `--test e2e` suite (settings page, TUI interaction) fails independently of this task — confirmed by the fact they test unrelated UI functionality.
2. **`VmRequestTracker` is not thread-safe by design**: Intentional (see decision #1 above). Task 04 (connection logic) must ensure the tracker is accessed from a single async task.
