//! Dart VM Service WebSocket protocol types and utilities.
//!
//! This module contains types and helpers for communicating with the Dart VM
//! Service over WebSocket using the JSON-RPC 2.0 protocol.
//!
//! ## Modules
//!
//! - [`protocol`] — All JSON-RPC types, the request tracker, and the message
//!   parser.
//! - [`client`] — Async WebSocket client with reconnection and channel-based API.
//! - [`logging`] — VM Service Logging stream event parsing (`dart:developer log()`).
//! - [`errors`] — VM Service Flutter error event parsing.
//!
//! ## Quick start
//!
//! ```ignore
//! use fdemon_daemon::vm_service::{VmServiceClient, VmRequestTracker, parse_vm_message, VmServiceMessage};
//!
//! // Connect to the VM Service
//! let mut client = VmServiceClient::connect("ws://127.0.0.1:8181/ws").await?;
//!
//! // Send a JSON-RPC request
//! let result = client.request("getVM", None).await?;
//!
//! // Receive stream events
//! while let Some(event) = client.event_receiver().recv().await {
//!     tracing::debug!("Event: {:?}", event.params.stream_id);
//! }
//!
//! // Or use the tracker directly:
//! let mut tracker = VmRequestTracker::new();
//! let (id, rx) = tracker.register();
//!
//! // ... send VmServiceRequest with `id` over WebSocket ...
//!
//! // When the response frame arrives:
//! let text = r#"{"id":"1","result":{"type":"VM","name":"vm","version":"3.0","isolates":[]}}"#;
//! if let VmServiceMessage::Response(resp) = parse_vm_message(text) {
//!     if let Some(ref response_id) = resp.id {
//!         tracker.complete(response_id, resp);
//!     }
//! }
//! ```

pub mod client;
pub mod errors;
pub mod logging;
pub mod protocol;

pub use client::{ConnectionState, VmServiceClient};
pub use errors::{flutter_error_to_log_entry, parse_flutter_error, FlutterErrorEvent};
pub use logging::{parse_log_record, vm_level_to_log_level, vm_log_to_log_entry, VmLogRecord};
pub use protocol::{
    parse_vm_message, IsolateGroupRef, IsolateInfo, IsolateRef, LibraryRef, StreamEvent,
    StreamEventParams, VmInfo, VmRequestTracker, VmServiceError, VmServiceEvent, VmServiceMessage,
    VmServiceRequest, VmServiceResponse,
};
