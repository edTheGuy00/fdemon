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
//! - [`debugger_types`] — VM Service debug type definitions for debugging RPCs and Debug/Isolate stream events.
//! - [`debugger`] — Debug RPC wrappers (`pause`, `resume`, `addBreakpointWithScriptUri`, `getStack`, `evaluate`, etc.).
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
pub mod debugger;
pub mod debugger_types;
pub mod errors;
pub mod extensions;
pub mod logging;
pub mod network;
pub mod performance;
pub mod protocol;
pub mod request_api;
pub mod timeline;

pub use client::{ConnectionState, VmRequestHandle, VmServiceClient, MAX_RECONNECT_ATTEMPTS};
pub use debugger::{
    add_breakpoint_with_script_uri, evaluate, evaluate_in_frame, get_object, get_scripts,
    get_source_report, get_stack, pause, remove_breakpoint, resume, set_isolate_pause_mode,
};
pub use debugger_types::{
    parse_debug_event, parse_isolate_event, BoundVariable, Breakpoint, ClassRef, DebugEvent,
    ExceptionPauseMode, Frame, FrameKind, FunctionRef, InstanceRef, IsolateEvent,
    IsolateRef as DebugIsolateRef, ScriptList, ScriptRef, SourceLocation, Stack, StepOption,
};
pub use errors::{flutter_error_to_log_entry, parse_flutter_error, FlutterErrorEvent};
pub use extensions::{
    debug_dump, debug_dump_app, debug_dump_layer_tree, debug_dump_render_tree, debug_paint, ext,
    extract_layout_info, extract_layout_tree, fetch_layout_data, flip_overlay, get_details_subtree,
    get_layout_node, get_root_widget_tree, get_selected_widget, is_extension_not_available,
    parse_bool_extension_response, parse_data_extension_response, parse_diagnostics_node_response,
    parse_optional_diagnostics_node_response, performance_overlay, query_all_overlays,
    repaint_rainbow, toggle_bool_extension, widget_inspector, widget_location_id_map_handle,
    DebugDumpKind, DebugOverlayState, ObjectGroupManager, WidgetInspector,
};
pub use logging::{parse_log_record, vm_level_to_log_level, vm_log_to_log_entry, VmLogRecord};
pub use network::{
    clear_http_profile, clear_http_profile_handle, enable_http_timeline_logging,
    enable_http_timeline_logging_handle, get_http_profile, get_http_profile_handle,
    get_http_profile_request, get_http_profile_request_handle, get_socket_profile,
    set_socket_profiling_enabled, set_socket_profiling_enabled_handle, HttpProfile,
};
pub use performance::{
    get_allocation_profile, get_memory_sample, get_memory_sample_from_usage, get_memory_usage,
    parse_allocation_profile, parse_gc_event, parse_memory_usage,
};
pub use protocol::{
    parse_vm_message, IsolateGroupRef, IsolateInfo, IsolateRef, LibraryRef, StreamEvent,
    StreamEventParams, VersionInfo, VmClientEvent, VmInfo, VmRequestTracker, VmServiceError,
    VmServiceEvent, VmServiceMessage, VmServiceRequest, VmServiceResponse,
};
pub use request_api::VmRequestApi;
pub use timeline::{
    enable_frame_tracking, fetch_timeline_chunk, fetch_timeline_chunk_with_metadata,
    flutter_extension_kind, get_vm_timeline_micros, is_frame_event, parse_frame_timing,
    parse_str_u64,
};

/// Redact the auth token from a Dart VM Service WebSocket URI.
///
/// The Dart VM Service URI has the form:
/// ```text
/// ws://127.0.0.1:PORT/AUTH_TOKEN/ws
/// ```
/// Where `AUTH_TOKEN` is a random session token that authorises RPC calls.
/// Logging this URI in plain text exposes the token to anyone who can read
/// log files, allowing them to execute arbitrary VM Service RPCs (hot reload,
/// heap reads, service extension invocations).
///
/// This function replaces the first path segment (the auth token) with
/// `[REDACTED]` so the connection event can still be logged at `info!` level
/// without leaking credentials.
///
/// Returns the input unchanged if it does not match the expected shape —
/// defensive behaviour that prevents the redaction step from blocking logging
/// on unexpected input.
///
/// # Examples
///
/// ```
/// use fdemon_daemon::vm_service::redact_vm_service_token;
///
/// // Normal VM Service URI — token is hidden.
/// let safe = redact_vm_service_token("ws://127.0.0.1:8080/AbC123/ws");
/// assert!(safe.contains("[REDACTED]"));
/// assert!(!safe.contains("AbC123"));
/// assert!(safe.starts_with("ws://127.0.0.1:8080/"));
///
/// // URI without an auth token (local dev with no auth) — returned unchanged.
/// assert_eq!(
///     redact_vm_service_token("ws://127.0.0.1:8080/ws"),
///     "ws://127.0.0.1:8080/ws",
/// );
/// ```
pub fn redact_vm_service_token(uri: &str) -> String {
    // Identify the scheme+authority prefix (everything up to the first path '/').
    let scheme_end = if let Some(rest) = uri.strip_prefix("ws://") {
        uri.len() - rest.len()
    } else if let Some(rest) = uri.strip_prefix("wss://") {
        uri.len() - rest.len()
    } else {
        // Unknown scheme — return unchanged rather than panic or corrupt.
        return uri.to_string();
    };

    // Find the first '/' that starts the path component (after the authority).
    let path_start = match uri[scheme_end..].find('/') {
        Some(rel) => scheme_end + rel,
        // No path at all — nothing to redact.
        None => return uri.to_string(),
    };

    let (authority, path) = uri.split_at(path_start);
    // `path` starts with '/'. Strip the leading slash to inspect segments.
    let path_body = &path[1..];

    // Split path into segments.  The expected form is `AUTH_TOKEN/ws` (2+ segments).
    // If there is only one segment (e.g. just `/ws`) there is no auth token —
    // return unchanged.
    let slash_pos = match path_body.find('/') {
        Some(p) => p,
        None => return uri.to_string(),
    };

    // Everything after the first path segment is the remainder (e.g. `/ws`).
    let remainder = &path_body[slash_pos..];

    let mut out = String::with_capacity(authority.len() + "[REDACTED]".len() + remainder.len() + 2);
    out.push_str(authority);
    out.push('/');
    out.push_str("[REDACTED]");
    out.push_str(remainder);
    out
}

#[cfg(test)]
mod redact_tests {
    use super::redact_vm_service_token;

    #[test]
    fn test_redact_normal_vm_service_uri() {
        let raw = "ws://127.0.0.1:8080/AbCdEf123XyZ/ws";
        let red = redact_vm_service_token(raw);
        assert!(!red.contains("AbCdEf123XyZ"));
        assert!(red.contains("[REDACTED]"));
        assert!(red.starts_with("ws://127.0.0.1:8080/"));
        assert_eq!(red, "ws://127.0.0.1:8080/[REDACTED]/ws");
    }

    #[test]
    fn test_redact_uri_without_path_returns_unchanged() {
        // Only one path segment — no auth token, leave as-is.
        let raw = "ws://127.0.0.1:8080/ws";
        let red = redact_vm_service_token(raw);
        assert_eq!(red, raw);
    }

    #[test]
    fn test_redact_uri_trailing_slash_only_returns_unchanged() {
        // Trailing slash only → single empty segment.
        let raw = "ws://127.0.0.1:8080/";
        let red = redact_vm_service_token(raw);
        assert_eq!(red, raw);
    }

    #[test]
    fn test_redact_malformed_uri_does_not_panic() {
        // Must not panic; output is implementation-defined.
        let _ = redact_vm_service_token("not a uri");
        let _ = redact_vm_service_token("");
    }

    #[test]
    fn test_redact_wss_scheme() {
        let raw = "wss://example.com:443/SECRET_TOKEN/ws";
        let red = redact_vm_service_token(raw);
        assert!(!red.contains("SECRET_TOKEN"));
        assert!(red.contains("[REDACTED]"));
        assert_eq!(red, "wss://example.com:443/[REDACTED]/ws");
    }

    #[test]
    fn test_redact_longer_path() {
        // Token followed by more than one trailing segment.
        let raw = "ws://127.0.0.1:9999/TOKEN123=/ws/extra";
        let red = redact_vm_service_token(raw);
        assert!(!red.contains("TOKEN123="));
        assert!(red.contains("[REDACTED]"));
        assert_eq!(red, "ws://127.0.0.1:9999/[REDACTED]/ws/extra");
    }
}
