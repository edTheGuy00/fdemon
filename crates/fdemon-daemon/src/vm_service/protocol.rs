//! JSON-RPC 2.0 protocol types for the Dart VM Service WebSocket interface.
//!
//! The Dart VM Service communicates over WebSocket using JSON-RPC 2.0. This module
//! defines the types for requests, responses, and stream events, plus a request
//! tracker for correlating async responses with their originating requests.
//!
//! Protocol reference:
//! <https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md>

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::oneshot;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// JSON-RPC 2.0 request to the Dart VM Service.
#[derive(Debug, Serialize)]
pub struct VmServiceRequest {
    /// Always `"2.0"`.
    pub jsonrpc: &'static str,
    /// Unique request ID used to correlate the response.
    pub id: String,
    /// Method name, e.g. `"getVM"` or `"streamListen"`.
    pub method: String,
    /// Optional method parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl VmServiceRequest {
    /// Create a new request with auto-assigned ID.
    pub fn new(id: String, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC 2.0 response from the Dart VM Service.
#[derive(Debug, Deserialize)]
pub struct VmServiceResponse {
    /// The ID matching the original request. `None` for notifications.
    pub id: Option<String>,
    /// Successful result payload.
    pub result: Option<Value>,
    /// Error payload, present when the call failed.
    pub error: Option<VmServiceError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Deserialize)]
pub struct VmServiceError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional error data.
    pub data: Option<Value>,
}

// ---------------------------------------------------------------------------
// Stream event types
// ---------------------------------------------------------------------------

/// VM Service stream event notification (no `id` field, `method = "streamNotify"`).
#[derive(Debug, Deserialize)]
pub struct VmServiceEvent {
    /// Always `"streamNotify"` for event notifications.
    pub method: String,
    /// Stream-specific event payload.
    pub params: StreamEventParams,
}

/// Parameters of a `"streamNotify"` notification.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEventParams {
    /// The stream identifier, e.g. `"Extension"`, `"Logging"`, `"GC"`.
    pub stream_id: String,
    /// The event itself.
    pub event: StreamEvent,
}

/// A single VM Service stream event.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEvent {
    /// Event kind, e.g. `"Extension"`, `"Logging"`, `"GC"`.
    pub kind: String,
    /// Isolate that generated the event, if applicable.
    pub isolate: Option<IsolateRef>,
    /// Milliseconds since epoch when the event was generated.
    pub timestamp: Option<i64>,
    /// Kind-specific fields, captured untyped for forward compatibility.
    #[serde(flatten)]
    pub data: Value,
}

/// Events emitted by the VM Service client through the event channel.
///
/// Wraps raw stream notifications (`VmServiceEvent`) with connection lifecycle
/// events so consumers can react to reconnection status changes.
#[derive(Debug)]
pub enum VmClientEvent {
    /// A stream notification from the VM Service (e.g., Extension, Logging, GC).
    StreamEvent(VmServiceEvent),
    /// The client is attempting to reconnect after a connection loss.
    Reconnecting {
        /// Current attempt number (1-based).
        attempt: u32,
        /// Maximum attempts before giving up.
        max_attempts: u32,
    },
    /// The client successfully reconnected after a connection loss.
    Reconnected,
    /// All reconnection attempts exhausted; the client has given up.
    PermanentlyDisconnected,
}

// ---------------------------------------------------------------------------
// VM / Isolate information types
// ---------------------------------------------------------------------------

/// Response body from the `getVM` RPC call.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmInfo {
    /// VM name (usually `"vm"`).
    pub name: String,
    /// Dart VM version string.
    pub version: String,
    /// Running isolates.
    pub isolates: Vec<IsolateRef>,
    /// Running isolate groups (Dart 2.15+).
    pub isolate_groups: Option<Vec<IsolateGroupRef>>,
}

/// Lightweight reference to a Dart isolate.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolateRef {
    /// Unique isolate ID (e.g. `"isolates/1234"`).
    pub id: String,
    /// Human-readable isolate name.
    pub name: String,
    /// Isolate number as a string, if provided.
    pub number: Option<String>,
    /// Whether this is an internal VM system isolate.
    pub is_system_isolate: Option<bool>,
}

/// Full isolate details from the `getIsolate` RPC call.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolateInfo {
    /// Unique isolate ID.
    pub id: String,
    /// Human-readable isolate name.
    pub name: String,
    /// Isolate number as a string.
    pub number: Option<String>,
    /// Whether the isolate is currently runnable.
    pub runnable: Option<bool>,
    /// Whether the isolate pauses on exit.
    pub pause_on_exit: Option<bool>,
    /// Epoch milliseconds when the isolate was started.
    pub start_time: Option<i64>,
    /// Loaded library references.
    pub libraries: Option<Vec<LibraryRef>>,
    /// Flutter service extension RPCs registered by this isolate.
    pub extension_rpcs: Option<Vec<String>>,
}

/// Lightweight reference to a Dart library.
#[derive(Debug, Deserialize)]
pub struct LibraryRef {
    /// Library object ID.
    pub id: String,
    /// Library name.
    pub name: String,
    /// Library URI (e.g. `"package:my_app/main.dart"`).
    pub uri: String,
}

/// Lightweight reference to a Dart isolate group.
#[derive(Debug, Deserialize)]
pub struct IsolateGroupRef {
    /// Isolate group object ID.
    pub id: String,
    /// Isolate group name.
    pub name: String,
}

// ---------------------------------------------------------------------------
// Parsed message discriminant
// ---------------------------------------------------------------------------

/// The result of parsing a raw VM Service WebSocket text frame.
#[derive(Debug)]
pub enum VmServiceMessage {
    /// A response to a request we sent (has an `id` field).
    Response(VmServiceResponse),
    /// A stream event notification (no `id`, `method = "streamNotify"`).
    Event(VmServiceEvent),
    /// A message we received but could not fully interpret.
    Unknown(String),
}

/// Parse a raw WebSocket text message into a typed [`VmServiceMessage`].
///
/// Dispatch logic:
/// - If the JSON has a top-level `"id"` field → treat as [`VmServiceResponse`].
/// - If the JSON has a top-level `"method"` field (no `"id"`) → treat as
///   [`VmServiceEvent`].
/// - Anything else → [`VmServiceMessage::Unknown`].
pub fn parse_vm_message(text: &str) -> VmServiceMessage {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return VmServiceMessage::Unknown(text.to_string()),
    };

    let has_id = value.get("id").is_some_and(|v| !v.is_null());
    let has_method = value.get("method").is_some();

    if has_id {
        // Response to one of our requests
        match serde_json::from_value::<VmServiceResponse>(value) {
            Ok(response) => VmServiceMessage::Response(response),
            Err(_) => VmServiceMessage::Unknown(text.to_string()),
        }
    } else if has_method {
        // Unsolicited stream notification
        match serde_json::from_value::<VmServiceEvent>(value) {
            Ok(event) => VmServiceMessage::Event(event),
            Err(_) => VmServiceMessage::Unknown(text.to_string()),
        }
    } else {
        VmServiceMessage::Unknown(text.to_string())
    }
}

// ---------------------------------------------------------------------------
// Request tracker
// ---------------------------------------------------------------------------

/// Global monotonically-increasing counter for VM Service request IDs.
static VM_REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique VM Service request ID string.
fn next_vm_request_id() -> String {
    VM_REQUEST_ID_COUNTER
        .fetch_add(1, Ordering::SeqCst)
        .to_string()
}

/// A registered pending request waiting for a VM Service response.
struct PendingVmRequest {
    /// Channel half used to deliver the response to the caller.
    response_tx: oneshot::Sender<VmServiceResponse>,
    /// Timestamp of registration, used for stale-request cleanup.
    created_at: Instant,
}

/// Tracks in-flight VM Service requests and matches them to responses.
///
/// This is analogous to [`crate::commands::RequestTracker`] but operates on
/// string IDs and [`VmServiceResponse`] values rather than integer IDs and
/// daemon-specific responses.
pub struct VmRequestTracker {
    pending: HashMap<String, PendingVmRequest>,
}

impl VmRequestTracker {
    /// Create a new, empty tracker.
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Generate a fresh request ID and register a pending slot for it.
    ///
    /// Returns `(id, receiver)` where `id` must be sent in the JSON-RPC
    /// request and `receiver` will yield the response when it arrives.
    pub fn register(&mut self) -> (String, oneshot::Receiver<VmServiceResponse>) {
        let id = next_vm_request_id();
        let (tx, rx) = oneshot::channel();

        self.pending.insert(
            id.clone(),
            PendingVmRequest {
                response_tx: tx,
                created_at: Instant::now(),
            },
        );

        (id, rx)
    }

    /// Deliver a response to its waiting caller.
    ///
    /// Returns `true` if `id` was found in the pending map (response routed),
    /// or `false` if no matching pending request exists.
    pub fn complete(&mut self, id: &str, response: VmServiceResponse) -> bool {
        if let Some(pending) = self.pending.remove(id) {
            // The receiver may have been dropped; ignore the error.
            let _ = pending.response_tx.send(response);
            true
        } else {
            false
        }
    }

    /// Remove and cancel all requests that have been pending longer than
    /// `timeout`.
    ///
    /// Returns the IDs of the requests that were removed.
    pub fn cleanup_stale(&mut self, timeout: Duration) -> Vec<String> {
        let now = Instant::now();

        let stale: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, req)| now.duration_since(req.created_at) > timeout)
            .map(|(id, _)| id.clone())
            .collect();

        for id in &stale {
            self.pending.remove(id);
        }

        stale
    }

    /// Return the number of currently pending requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for VmRequestTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_vm_message ----------------------------------------------------

    #[test]
    fn test_parse_get_vm_response() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": "1",
            "result": {
                "type": "VM",
                "name": "vm",
                "version": "2.19.0",
                "isolates": [
                    {
                        "type": "@Isolate",
                        "id": "isolates/1",
                        "name": "main",
                        "number": "1"
                    }
                ]
            }
        }"#;

        let msg = parse_vm_message(json);

        match msg {
            VmServiceMessage::Response(resp) => {
                assert_eq!(resp.id.as_deref(), Some("1"));
                assert!(resp.result.is_some());
                assert!(resp.error.is_none());
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_stream_event() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "streamNotify",
            "params": {
                "streamId": "Extension",
                "event": {
                    "kind": "Extension",
                    "extensionKind": "Flutter.Error",
                    "extensionData": {}
                }
            }
        }"#;

        let msg = parse_vm_message(json);

        match msg {
            VmServiceMessage::Event(event) => {
                assert_eq!(event.method, "streamNotify");
                assert_eq!(event.params.stream_id, "Extension");
                assert_eq!(event.params.event.kind, "Extension");
            }
            other => panic!("Expected Event, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_error_response() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": "42",
            "error": {
                "code": -32601,
                "message": "Method not found",
                "data": null
            }
        }"#;

        let msg = parse_vm_message(json);

        match msg {
            VmServiceMessage::Response(resp) => {
                assert_eq!(resp.id.as_deref(), Some("42"));
                assert!(resp.result.is_none());
                let err = resp.error.expect("error should be present");
                assert_eq!(err.code, -32601);
                assert_eq!(err.message, "Method not found");
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_invalid_json_returns_unknown() {
        let msg = parse_vm_message("not json at all {{{");
        assert!(matches!(msg, VmServiceMessage::Unknown(_)));
    }

    #[test]
    fn test_parse_empty_object_returns_unknown() {
        // No id and no method → Unknown
        let msg = parse_vm_message(r#"{"jsonrpc": "2.0"}"#);
        assert!(matches!(msg, VmServiceMessage::Unknown(_)));
    }

    #[test]
    fn test_parse_null_id_treated_as_event_path() {
        // id present but null → treated as "no id", falls through to method check
        let json = r#"{
            "jsonrpc": "2.0",
            "id": null,
            "method": "streamNotify",
            "params": {
                "streamId": "GC",
                "event": { "kind": "GC" }
            }
        }"#;
        let msg = parse_vm_message(json);
        // null id → has_id is false → falls to method check → Event
        assert!(matches!(msg, VmServiceMessage::Event(_)));
    }

    // -- VmServiceRequest ----------------------------------------------------

    #[test]
    fn test_vm_service_request_serializes_correctly() {
        let req = VmServiceRequest::new("7".to_string(), "getVM", None);
        let json = serde_json::to_string(&req).unwrap();
        let val: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(val["jsonrpc"], "2.0");
        assert_eq!(val["id"], "7");
        assert_eq!(val["method"], "getVM");
        assert!(!val.as_object().unwrap().contains_key("params"));
    }

    #[test]
    fn test_vm_service_request_with_params() {
        let params = serde_json::json!({ "streamId": "Extension" });
        let req = VmServiceRequest::new("3".to_string(), "streamListen", Some(params));
        let json = serde_json::to_string(&req).unwrap();
        let val: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(val["params"]["streamId"], "Extension");
    }

    // -- VmInfo deserialization ----------------------------------------------

    #[test]
    fn test_parse_isolate_info() {
        let json = r#"{
            "type": "Isolate",
            "id": "isolates/9",
            "name": "root",
            "number": "9",
            "runnable": true,
            "pauseOnExit": false,
            "startTime": 1700000000000,
            "libraries": [
                { "id": "libraries/1", "name": "dart:core", "uri": "dart:core" }
            ],
            "extensionRpcs": ["ext.flutter.reassemble"]
        }"#;

        let info: IsolateInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "isolates/9");
        assert_eq!(info.name, "root");
        assert_eq!(info.runnable, Some(true));
        let exts = info.extension_rpcs.unwrap();
        assert!(exts.contains(&"ext.flutter.reassemble".to_string()));
        let libs = info.libraries.unwrap();
        assert_eq!(libs[0].uri, "dart:core");
    }

    #[test]
    fn test_parse_vm_info() {
        let json = r#"{
            "type": "VM",
            "name": "vm",
            "version": "3.0.0",
            "isolates": [
                { "id": "isolates/1", "name": "main", "number": "1" }
            ],
            "isolateGroups": [
                { "id": "isolateGroups/1", "name": "main" }
            ]
        }"#;

        let vm: VmInfo = serde_json::from_str(json).unwrap();
        assert_eq!(vm.name, "vm");
        assert_eq!(vm.version, "3.0.0");
        assert_eq!(vm.isolates.len(), 1);
        assert_eq!(vm.isolates[0].id, "isolates/1");
        let groups = vm.isolate_groups.unwrap();
        assert_eq!(groups[0].name, "main");
    }

    // -- VmRequestTracker ----------------------------------------------------

    #[test]
    fn test_request_tracker_register_and_complete() {
        let mut tracker = VmRequestTracker::new();

        let (id, mut rx) = tracker.register();
        assert!(!id.is_empty());
        assert_eq!(tracker.pending_count(), 1);

        let response = VmServiceResponse {
            id: Some(id.clone()),
            result: Some(serde_json::json!({ "ok": true })),
            error: None,
        };

        let matched = tracker.complete(&id, response);
        assert!(matched, "complete() should return true for a known id");
        assert_eq!(tracker.pending_count(), 0);

        // Verify the response was delivered through the channel.
        let received = rx.try_recv().expect("response should be available");
        assert!(received.result.is_some());
    }

    #[test]
    fn test_request_tracker_complete_unknown_id_returns_false() {
        let mut tracker = VmRequestTracker::new();

        let response = VmServiceResponse {
            id: Some("999".to_string()),
            result: None,
            error: None,
        };

        let matched = tracker.complete("999", response);
        assert!(!matched, "complete() should return false for unknown id");
    }

    #[test]
    fn test_request_tracker_multiple_requests() {
        let mut tracker = VmRequestTracker::new();

        let (id1, _rx1) = tracker.register();
        let (id2, _rx2) = tracker.register();
        let (id3, _rx3) = tracker.register();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_eq!(tracker.pending_count(), 3);
    }

    #[test]
    fn test_request_tracker_cleanup_stale() {
        let mut tracker = VmRequestTracker::new();

        let (_id, _rx) = tracker.register();
        assert_eq!(tracker.pending_count(), 1);

        // With a zero timeout every request is immediately stale.
        let removed = tracker.cleanup_stale(Duration::ZERO);
        assert_eq!(removed.len(), 1);
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn test_request_tracker_cleanup_stale_keeps_fresh_requests() {
        let mut tracker = VmRequestTracker::new();

        let (_id, _rx) = tracker.register();
        assert_eq!(tracker.pending_count(), 1);

        // With a very long timeout nothing should be cleaned up.
        let removed = tracker.cleanup_stale(Duration::from_secs(3600));
        assert!(removed.is_empty());
        assert_eq!(tracker.pending_count(), 1);
    }

    #[test]
    fn test_request_tracker_default() {
        let tracker = VmRequestTracker::default();
        assert_eq!(tracker.pending_count(), 0);
    }

    // -- Logging stream event ------------------------------------------------

    #[test]
    fn test_parse_logging_stream_event() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "streamNotify",
            "params": {
                "streamId": "Logging",
                "event": {
                    "kind": "Logging",
                    "timestamp": 1700000001234,
                    "logRecord": {
                        "message": { "valueAsString": "Hello world" },
                        "level": { "valueAsString": "INFO" }
                    }
                }
            }
        }"#;

        let msg = parse_vm_message(json);

        match msg {
            VmServiceMessage::Event(event) => {
                assert_eq!(event.params.stream_id, "Logging");
                assert_eq!(event.params.event.kind, "Logging");
                assert_eq!(event.params.event.timestamp, Some(1_700_000_001_234));
            }
            other => panic!("Expected Event, got {:?}", other),
        }
    }

    // -- IsolateRef deserialization ------------------------------------------

    #[test]
    fn test_isolate_ref_is_system_isolate() {
        let json = r#"{
            "id": "isolates/vm-service",
            "name": "vm-service",
            "number": "42",
            "isSystemIsolate": true
        }"#;

        let isolate: IsolateRef = serde_json::from_str(json).unwrap();
        assert_eq!(isolate.is_system_isolate, Some(true));
    }
}
