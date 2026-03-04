## Task: Create fdemon-dap Crate with DAP Protocol Types and Codec

**Objective**: Create the `fdemon-dap` workspace crate with hand-rolled DAP protocol types (Request, Response, Event, Capabilities) and a Content-Length framed async codec for DAP wire communication.

**Depends on**: None

### Scope

- `crates/fdemon-dap/` — **NEW CRATE**: entire directory structure
- `crates/fdemon-dap/Cargo.toml` — crate manifest with workspace dependencies
- `crates/fdemon-dap/src/lib.rs` — public API, module declarations
- `crates/fdemon-dap/src/protocol/mod.rs` — module root, re-exports
- `crates/fdemon-dap/src/protocol/types.rs` — **NEW**: DAP message types (Request, Response, Event, Capabilities)
- `crates/fdemon-dap/src/protocol/codec.rs` — **NEW**: Content-Length framed async reader/writer
- `Cargo.toml` (root) — add `fdemon-dap` to `[workspace.dependencies]`

### Details

#### 1. Crate Scaffold

Create `crates/fdemon-dap/Cargo.toml`:

```toml
[package]
name = "fdemon-dap"
version = "0.1.0"
edition = "2021"

[dependencies]
fdemon-core.workspace = true
fdemon-daemon.workspace = true
tokio = { workspace = true, features = ["net", "io-util", "sync", "macros"] }
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "rt-multi-thread"] }
```

Note: `fdemon-app` is intentionally NOT a dependency of `fdemon-dap`. The DAP crate depends only on `core` (domain types) and `daemon` (VmRequestHandle, debug types). The `app` crate will depend on `fdemon-dap` (not the reverse). This avoids circular dependencies and keeps the DAP crate focused on protocol + transport.

Add to root `Cargo.toml` `[workspace.dependencies]`:
```toml
fdemon-dap = { path = "crates/fdemon-dap" }
```

The `crates/*` glob automatically includes `fdemon-dap` as a workspace member.

#### 2. DAP Protocol Types (`protocol/types.rs`)

Hand-roll the subset of DAP types needed for Phase 2 (initialization handshake) and Phase 3 (debugging). Reference the [DAP specification](https://microsoft.github.io/debug-adapter-protocol/specification) and `dapts` crate for field names and structure.

**Wire-level message envelope:**

```rust
use serde::{Deserialize, Serialize};

/// Top-level DAP protocol message — discriminated by "type" field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DapMessage {
    #[serde(rename = "request")]
    Request(DapRequest),
    #[serde(rename = "response")]
    Response(DapResponse),
    #[serde(rename = "event")]
    Event(DapEvent),
}

/// A DAP request from the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapRequest {
    pub seq: i64,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// A DAP response sent to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapResponse {
    pub seq: i64,
    pub request_seq: i64,
    pub success: bool,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// A DAP event sent to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapEvent {
    pub seq: i64,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}
```

**Capabilities (for InitializeResponse):**

```rust
/// Server capabilities advertised during initialization.
/// Only fields relevant to Flutter debugging are included.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_configuration_done_request: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_terminate_debuggee: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_evaluate_for_hovers: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_exception_info_request: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_set_variable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_value_formatting_options: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_loaded_sources_request: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_log_points: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_breakpoint_locations_request: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_delayed_stack_trace_loading: Option<bool>,
    // Phase 3+ additions go here (new fields are backward-compatible)
}
```

**InitializeRequestArguments (from client):**

```rust
/// Arguments sent by the client in the "initialize" request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequestArguments {
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub adapter_id: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub lines_start_at1: Option<bool>,
    #[serde(default)]
    pub columns_start_at1: Option<bool>,
    #[serde(default)]
    pub path_format: Option<String>,
    #[serde(default)]
    pub supports_variable_type: Option<bool>,
    #[serde(default)]
    pub supports_variable_paging: Option<bool>,
    #[serde(default)]
    pub supports_run_in_terminal_request: Option<bool>,
    #[serde(default)]
    pub supports_memory_references: Option<bool>,
    #[serde(default)]
    pub supports_progress_reporting: Option<bool>,
    #[serde(default)]
    pub supports_invalidated_event: Option<bool>,
    #[serde(default)]
    pub supports_memory_event: Option<bool>,
}
```

**Helper constructors:**

```rust
impl DapResponse {
    /// Create a success response for a given request.
    pub fn success(request: &DapRequest, body: Option<serde_json::Value>) -> Self { ... }

    /// Create an error response for a given request.
    pub fn error(request: &DapRequest, message: impl Into<String>) -> Self { ... }
}

impl DapEvent {
    /// Create a new event with the given name and optional body.
    pub fn new(event: impl Into<String>, body: Option<serde_json::Value>) -> Self { ... }

    /// Create the "initialized" event (sent after initialize response).
    pub fn initialized() -> Self { ... }

    /// Create a "terminated" event.
    pub fn terminated() -> Self { ... }

    /// Create an "output" event for debug console output.
    pub fn output(category: &str, output: &str) -> Self { ... }
}

impl Capabilities {
    /// Default capabilities for fdemon's Flutter DAP adapter.
    pub fn fdemon_defaults() -> Self {
        Self {
            supports_configuration_done_request: Some(true),
            support_terminate_debuggee: Some(true),
            supports_evaluate_for_hovers: Some(true),
            supports_exception_info_request: Some(true),
            supports_loaded_sources_request: Some(true),
            supports_log_points: Some(true),
            supports_delayed_stack_trace_loading: Some(true),
            ..Default::default()
        }
    }
}
```

#### 3. Content-Length Codec (`protocol/codec.rs`)

Implement async reading and writing of DAP's Content-Length framed JSON messages.

**Wire format (from DAP spec):**
```
Content-Length: <byte-count>\r\n
\r\n
<utf-8-encoded-json-body>
```

Rules:
- Header section is ASCII
- `Content-Length` is the **byte count** of the JSON body (use `.as_bytes().len()`)
- Header terminated by blank line (`\r\n\r\n` separates headers from body)
- Body is UTF-8 encoded JSON

```rust
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// Maximum allowed message body size (10 MB — prevents OOM from malformed headers).
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Read a single DAP message from the stream.
/// Returns `None` on clean EOF (stream closed).
pub async fn read_message<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
) -> Result<Option<DapMessage>> {
    // 1. Read headers line by line until blank line
    // 2. Extract Content-Length value
    // 3. Validate: reject if missing Content-Length or exceeds MAX_MESSAGE_SIZE
    // 4. Read exactly content_length bytes
    // 5. Deserialize JSON into DapMessage
    ...
}

/// Write a single DAP message to the stream.
pub async fn write_message<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    message: &DapMessage,
) -> Result<()> {
    // 1. Serialize message to JSON bytes
    // 2. Write "Content-Length: {len}\r\n\r\n"
    // 3. Write JSON bytes
    // 4. Flush
    ...
}
```

Edge cases to handle:
- **Partial reads**: `read_exact` handles this automatically (blocks until all bytes received)
- **Missing Content-Length**: return error with descriptive message
- **Oversized messages**: reject with error before allocating buffer
- **Clean EOF**: return `Ok(None)` when the stream is closed (first `read_line` returns 0 bytes)
- **Extra headers**: ignore unknown headers (DAP spec says only `Content-Length` is defined, but parsers should be lenient)
- **Malformed JSON**: return error with the raw body for diagnostics

#### 4. Module Structure

```
crates/fdemon-dap/
├── Cargo.toml
└── src/
    ├── lib.rs              # pub mod protocol; re-exports
    └── protocol/
        ├── mod.rs           # pub mod types; pub mod codec; re-exports
        ├── types.rs         # DapMessage, DapRequest, DapResponse, DapEvent, Capabilities
        └── codec.rs         # read_message(), write_message()
```

`lib.rs` should re-export:
- `protocol::types::*` (all DAP types)
- `protocol::codec::{read_message, write_message}`

### Acceptance Criteria

1. `cargo check -p fdemon-dap` compiles with no errors
2. `DapMessage` roundtrips through serde (serialize → deserialize produces identical structure)
3. `DapRequest`, `DapResponse`, `DapEvent` serialize to DAP-spec-compliant JSON (correct field names, `"type"` tag)
4. `Capabilities::fdemon_defaults()` includes all essential fields for Flutter debugging
5. `InitializeRequestArguments` deserializes from real VS Code initialize request JSON
6. `DapResponse::success()` and `DapResponse::error()` set correct `request_seq` and `success` fields
7. `DapEvent::initialized()` produces `{"seq":0,"type":"event","event":"initialized"}`
8. `read_message()` correctly parses Content-Length framed messages
9. `read_message()` returns `Ok(None)` on clean EOF
10. `read_message()` returns error on missing Content-Length
11. `read_message()` returns error on oversized messages (> MAX_MESSAGE_SIZE)
12. `write_message()` produces correctly framed output with accurate Content-Length
13. Roundtrip test: `write_message` → `read_message` produces identical `DapMessage`
14. `cargo clippy -p fdemon-dap -- -D warnings` clean

### Testing

Write comprehensive tests for both types and codec:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // === Type tests ===

    #[test]
    fn test_dap_request_serialization() {
        let req = DapRequest { seq: 1, command: "initialize".into(), arguments: None };
        let json = serde_json::to_value(&DapMessage::Request(req)).unwrap();
        assert_eq!(json["type"], "request");
        assert_eq!(json["command"], "initialize");
    }

    #[test]
    fn test_dap_response_success_helper() {
        let req = DapRequest { seq: 5, command: "initialize".into(), arguments: None };
        let resp = DapResponse::success(&req, None);
        assert_eq!(resp.request_seq, 5);
        assert!(resp.success);
        assert_eq!(resp.command, "initialize");
    }

    #[test]
    fn test_capabilities_fdemon_defaults() {
        let caps = Capabilities::fdemon_defaults();
        assert_eq!(caps.supports_configuration_done_request, Some(true));
        assert_eq!(caps.support_terminate_debuggee, Some(true));
    }

    #[test]
    fn test_initialize_request_from_vscode_json() {
        let json = serde_json::json!({
            "clientID": "vscode",
            "clientName": "Visual Studio Code",
            "adapterID": "dart",
            "pathFormat": "path",
            "linesStartAt1": true,
            "columnsStartAt1": true,
            "supportsVariableType": true,
            "supportsVariablePaging": true,
            "supportsRunInTerminalRequest": true,
            "supportsMemoryReferences": true,
            "supportsProgressReporting": true,
            "supportsInvalidatedEvent": true,
            "supportsMemoryEvent": true
        });
        let args: InitializeRequestArguments = serde_json::from_value(json).unwrap();
        assert_eq!(args.client_id.as_deref(), Some("vscode"));
        assert_eq!(args.lines_start_at1, Some(true));
    }

    // === Codec tests ===

    #[tokio::test]
    async fn test_write_then_read_roundtrip() {
        let msg = DapMessage::Event(DapEvent::initialized());
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();
        let mut reader = BufReader::new(buf.as_slice());
        let read_msg = read_message(&mut reader).await.unwrap().unwrap();
        // Verify structure matches
    }

    #[tokio::test]
    async fn test_read_message_eof_returns_none() {
        let mut reader = BufReader::new(&b""[..]);
        let result = read_message(&mut reader).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_read_message_missing_content_length() {
        let data = b"Invalid-Header: 42\r\n\r\n{}";
        let mut reader = BufReader::new(&data[..]);
        let result = read_message(&mut reader).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_message_oversized_rejected() {
        let header = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_SIZE + 1);
        let mut reader = BufReader::new(header.as_bytes());
        let result = read_message(&mut reader).await;
        assert!(result.is_err());
    }
}
```

### Notes

- DAP types are hand-rolled rather than imported from `dapts` (v0.0.6) — the crate is minimally maintained (0 GitHub stars, single author). We reference `dapts` and the [DAP spec](https://microsoft.github.io/debug-adapter-protocol/specification) for field name correctness but own our type definitions.
- The `DapMessage` tagged enum uses `#[serde(tag = "type")]` to match the DAP wire format where each message has a `"type": "request"|"response"|"event"` field.
- `DapEvent.seq` is set to 0 for simplicity in Phase 2 — the server session (Task 04) will assign monotonic seq numbers when sending events.
- The codec uses fdemon-core's `Result<T>` type alias for error handling, with `Error::io()` or `Error::protocol()` for specific failure modes.
- Additional DAP types for Phase 3 (SetBreakpointsArguments, StackTraceResponse, etc.) will be added to `types.rs` as needed — the module is designed to grow.
