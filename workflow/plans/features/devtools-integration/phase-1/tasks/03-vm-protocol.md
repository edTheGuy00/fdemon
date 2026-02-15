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

**Status:** Not Started
