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
//! - [`extensions`] — Flutter service extension call infrastructure and constants.
//! - [`timeline`] — Flutter.Frame Extension event parsing for frame timing data.
//! - [`performance`] — Memory/GC RPC wrappers (`getMemoryUsage`, `getAllocationProfile`) and GC event parsing.
//!
//! ## Quick start
//!
//! ```ignore
//! use fdemon_daemon::vm_service::{VmServiceClient, VmClientEvent, VmRequestTracker, parse_vm_message, VmServiceMessage};
//!
//! // Connect to the VM Service
//! let mut client = VmServiceClient::connect("ws://127.0.0.1:8181/ws").await?;
//!
//! // Send a JSON-RPC request
//! let result = client.request("getVM", None).await?;
//!
//! // Call a Flutter service extension
//! let isolate_id = client.main_isolate_id().await?;
//! let result = client.call_extension(
//!     vm_service::extensions::ext::REPAINT_RAINBOW,
//!     &isolate_id,
//!     Some([("enabled".to_string(), "true".to_string())].into()),
//! ).await?;
//! let enabled = vm_service::extensions::parse_bool_extension_response(&result)?;
//!
//! // Receive stream events (yields VmClientEvent)
//! while let Some(event) = client.event_receiver().recv().await {
//!     match event {
//!         VmClientEvent::StreamEvent(e) => {
//!             tracing::debug!("Stream event: {:?}", e.params.stream_id);
//!         }
//!         VmClientEvent::Reconnecting { attempt, max_attempts } => {
//!             tracing::warn!("Reconnecting {}/{}", attempt, max_attempts);
//!         }
//!         VmClientEvent::Reconnected => tracing::info!("Reconnected"),
//!         VmClientEvent::PermanentlyDisconnected => break,
//!     }
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
pub mod extensions;
pub mod logging;
pub mod network;
pub mod performance;
pub mod protocol;
pub mod timeline;

pub use client::{ConnectionState, VmRequestHandle, VmServiceClient, MAX_RECONNECT_ATTEMPTS};
pub use errors::{flutter_error_to_log_entry, parse_flutter_error, FlutterErrorEvent};
pub use extensions::{
    debug_dump, debug_dump_app, debug_dump_layer_tree, debug_dump_render_tree, debug_paint, ext,
    extract_layout_info, extract_layout_tree, fetch_layout_data, flip_overlay, get_details_subtree,
    get_layout_node, get_root_widget_tree, get_selected_widget, is_extension_not_available,
    parse_bool_extension_response, parse_data_extension_response, parse_diagnostics_node_response,
    parse_optional_diagnostics_node_response, performance_overlay, query_all_overlays,
    repaint_rainbow, toggle_bool_extension, widget_inspector, DebugDumpKind, DebugOverlayState,
    ObjectGroupManager, WidgetInspector,
};
pub use logging::{parse_log_record, vm_level_to_log_level, vm_log_to_log_entry, VmLogRecord};
pub use network::{
    clear_http_profile, clear_http_profile_handle, enable_http_timeline_logging,
    enable_http_timeline_logging_handle, get_http_profile, get_http_profile_handle,
    get_http_profile_request, get_http_profile_request_handle, get_socket_profile,
    set_socket_profiling_enabled, set_socket_profiling_enabled_handle, HttpProfile,
};
pub use performance::{
    get_allocation_profile, get_memory_sample, get_memory_usage, parse_allocation_profile,
    parse_gc_event, parse_memory_usage,
};
pub use protocol::{
    parse_vm_message, IsolateGroupRef, IsolateInfo, IsolateRef, LibraryRef, StreamEvent,
    StreamEventParams, VersionInfo, VmClientEvent, VmInfo, VmRequestTracker, VmServiceError,
    VmServiceEvent, VmServiceMessage, VmServiceRequest, VmServiceResponse,
};
pub use timeline::{
    enable_frame_tracking, flutter_extension_kind, is_frame_event, parse_frame_timing,
    parse_str_u64,
};
