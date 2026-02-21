//! VM Service extension wrappers for `ext.dart.io.*` network profiling APIs.
//!
//! This module provides the data pipeline for the Network Monitor tab, enabling:
//! - Enabling/disabling HTTP timeline logging
//! - Polling for HTTP profile data (request list)
//! - Fetching full request details with headers and bodies
//! - Clearing the HTTP profile
//! - Querying and toggling socket profiling
//!
//! ## Extension Availability
//!
//! The `ext.dart.io.*` extensions are registered by `dart:io` and are available
//! in debug and profile mode. In release mode (or when `dart:io` is not imported),
//! calls will fail with error code -32601. Use
//! [`is_extension_not_available`][super::extensions::is_extension_not_available]
//! to detect this and show an appropriate message in the UI.
//!
//! ## Polling
//!
//! Network profiling uses polling rather than stream subscriptions. Call
//! [`get_http_profile`] with `updated_since: None` to get all recorded requests,
//! then pass the returned [`HttpProfile::timestamp`] as `updated_since` in
//! subsequent calls to receive only new or updated entries.

use std::collections::HashMap;

use fdemon_core::network::{
    ConnectionInfo, HttpProfileEntry, HttpProfileEntryDetail, HttpProfileEvent, SocketEntry,
};
use fdemon_core::prelude::*;

use super::client::{VmRequestHandle, VmServiceClient};
use super::extensions::ext;

// ── HttpProfile ───────────────────────────────────────────────────────────────

/// Parsed response from `ext.dart.io.getHttpProfile`.
pub struct HttpProfile {
    /// Snapshot timestamp (microseconds since Unix epoch).
    ///
    /// Pass this as `updated_since` in the next poll to receive only requests
    /// that started or changed after this point.
    pub timestamp: i64,
    /// List of HTTP request summaries.
    pub requests: Vec<HttpProfileEntry>,
}

// ── enable_http_timeline_logging ──────────────────────────────────────────────

/// Enable or disable HTTP timeline logging in the Dart VM.
///
/// Must be called with `enabled: true` before [`get_http_profile`] returns
/// data. Uses the `ext.dart.io.httpEnableTimelineLogging` extension.
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the response is missing the `enabled` field,
/// or [`Error::ChannelClosed`] if the VM Service client is closed.
pub async fn enable_http_timeline_logging(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: bool,
) -> Result<bool> {
    let mut args = HashMap::new();
    args.insert("enabled".to_string(), enabled.to_string());
    let result = client
        .call_extension(ext::HTTP_ENABLE_TIMELINE_LOGGING, isolate_id, Some(args))
        .await?;
    // Response: { "enabled": true/false } — may be JSON bool or string
    result
        .get("enabled")
        .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
        .ok_or_else(|| Error::protocol("missing 'enabled' in httpEnableTimelineLogging response"))
}

// ── get_http_profile ──────────────────────────────────────────────────────────

/// Fetch the HTTP profile — a list of recorded HTTP requests.
///
/// When `updated_since` is provided (microseconds since epoch), only requests
/// that started or were updated after that timestamp are returned. This enables
/// efficient incremental polling: store [`HttpProfile::timestamp`] from each
/// response and pass it as `updated_since` on the next call.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the VM Service client is closed, or
/// [`Error::Protocol`] if the response cannot be parsed.
pub async fn get_http_profile(
    client: &VmServiceClient,
    isolate_id: &str,
    updated_since: Option<i64>,
) -> Result<HttpProfile> {
    let mut args = HashMap::new();
    if let Some(ts) = updated_since {
        args.insert("updatedSince".to_string(), ts.to_string());
    }
    let args = if args.is_empty() { None } else { Some(args) };
    let result = client
        .call_extension(ext::GET_HTTP_PROFILE, isolate_id, args)
        .await?;
    parse_http_profile(&result)
}

/// Parse a `getHttpProfile` response into an [`HttpProfile`].
///
/// Returns `Ok(HttpProfile { timestamp: 0, requests: [] })` if the response
/// is missing expected fields (graceful degradation for partial responses).
///
/// # Errors
///
/// This function is infallible in the sense that it returns `Ok` even for
/// partially malformed responses — individual entries that cannot be parsed
/// are silently skipped.
pub fn parse_http_profile(result: &serde_json::Value) -> Result<HttpProfile> {
    let timestamp = result
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let requests = result
        .get("requests")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_http_profile_entry).collect())
        .unwrap_or_default();

    Ok(HttpProfile {
        timestamp,
        requests,
    })
}

/// Parse a single entry from a `getHttpProfile` response.
///
/// Returns `None` if required fields (`id`, `method`, `uri`) are missing,
/// allowing callers to skip malformed entries gracefully.
pub fn parse_http_profile_entry(value: &serde_json::Value) -> Option<HttpProfileEntry> {
    let id = value.get("id")?.as_str()?.to_string();
    let method = value.get("method")?.as_str()?.to_string();
    let uri = value.get("uri")?.as_str()?.to_string();

    let start_time_us = value.get("startTime").and_then(|v| v.as_i64()).unwrap_or(0);
    let end_time_us = value.get("endTime").and_then(|v| v.as_i64());

    // Status code comes from the nested response object
    let status_code = value
        .get("response")
        .and_then(|r| r.get("statusCode"))
        .and_then(|v| v.as_u64())
        .and_then(|v| u16::try_from(v).ok());

    // Content-Type from response headers (array of values — take the first)
    let content_type = value
        .get("response")
        .and_then(|r| r.get("headers"))
        .and_then(|h| h.get("content-type"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let request_content_length = value
        .get("request")
        .and_then(|r| r.get("contentLength"))
        .and_then(|v| v.as_i64());

    let response_content_length = value
        .get("response")
        .and_then(|r| r.get("contentLength"))
        .and_then(|v| v.as_i64());

    // Error may appear in either the request or response sub-object
    let error = value
        .get("request")
        .and_then(|r| r.get("error"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            value
                .get("response")
                .and_then(|r| r.get("error"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    Some(HttpProfileEntry {
        id,
        method,
        uri,
        status_code,
        content_type,
        start_time_us,
        end_time_us,
        request_content_length,
        response_content_length,
        error,
    })
}

// ── get_http_profile_request ──────────────────────────────────────────────────

/// Fetch full details for a single HTTP request, including headers and bodies.
///
/// Bodies are returned as `Vec<u8>` — the VM Service transmits them as JSON
/// int arrays (e.g., `[72, 101, 108, 108, 111]` for `"Hello"`).
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the VM Service client is closed, or
/// [`Error::Protocol`] if the response cannot be parsed.
pub async fn get_http_profile_request(
    client: &VmServiceClient,
    isolate_id: &str,
    request_id: &str,
) -> Result<HttpProfileEntryDetail> {
    let mut args = HashMap::new();
    args.insert("id".to_string(), request_id.to_string());
    let result = client
        .call_extension(ext::GET_HTTP_PROFILE_REQUEST, isolate_id, Some(args))
        .await?;
    parse_http_profile_request_detail(&result)
}

/// Parse a `getHttpProfileRequest` response into an [`HttpProfileEntryDetail`].
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the base entry fields (`id`, `method`, `uri`)
/// are missing.
pub fn parse_http_profile_request_detail(
    result: &serde_json::Value,
) -> Result<HttpProfileEntryDetail> {
    let entry = parse_http_profile_entry(result).ok_or_else(|| {
        Error::protocol("failed to parse base HttpProfileEntry from detail response")
    })?;

    let request_headers = parse_headers(result.get("request").and_then(|r| r.get("headers")));
    let response_headers = parse_headers(result.get("response").and_then(|r| r.get("headers")));

    let request_body = parse_body_bytes(result.get("requestBody"));
    let response_body = parse_body_bytes(result.get("responseBody"));

    let events = result
        .get("events")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_http_event).collect())
        .unwrap_or_default();

    let connection_info = result
        .get("request")
        .and_then(|r| r.get("connectionInfo"))
        .and_then(parse_connection_info);

    Ok(HttpProfileEntryDetail {
        entry,
        request_headers,
        response_headers,
        request_body,
        response_body,
        events,
        connection_info,
    })
}

/// Parse a headers object into a list of `(name, values)` pairs.
///
/// The VM Service encodes headers as a JSON object where each value is an
/// array of strings, supporting multi-value headers.
pub fn parse_headers(headers_value: Option<&serde_json::Value>) -> Vec<(String, Vec<String>)> {
    headers_value
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| {
                    let values = v
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    (k.clone(), values)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a body value from a JSON int array into raw bytes.
///
/// The VM Service transmits request/response bodies as JSON arrays of integers
/// (each 0–255). Returns an empty `Vec` if the value is absent or null.
pub fn parse_body_bytes(value: Option<&serde_json::Value>) -> Vec<u8> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a single HTTP timeline event.
fn parse_http_event(value: &serde_json::Value) -> Option<HttpProfileEvent> {
    Some(HttpProfileEvent {
        event: value.get("event")?.as_str()?.to_string(),
        timestamp_us: value.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0),
    })
}

/// Parse connection info from a `connectionInfo` sub-object.
fn parse_connection_info(value: &serde_json::Value) -> Option<ConnectionInfo> {
    Some(ConnectionInfo {
        local_port: value
            .get("localPort")
            .and_then(|v| v.as_u64())
            .and_then(|v| u16::try_from(v).ok()),
        remote_address: value
            .get("remoteAddress")
            .and_then(|v| v.as_str())
            .map(String::from),
        remote_port: value
            .get("remotePort")
            .and_then(|v| v.as_u64())
            .and_then(|v| u16::try_from(v).ok()),
    })
}

// ── clear_http_profile ────────────────────────────────────────────────────────

/// Clear all recorded HTTP profile data.
///
/// After calling this, [`get_http_profile`] will return an empty list until
/// new requests are made.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the VM Service client is closed.
pub async fn clear_http_profile(client: &VmServiceClient, isolate_id: &str) -> Result<()> {
    client
        .call_extension(ext::CLEAR_HTTP_PROFILE, isolate_id, None)
        .await?;
    Ok(())
}

// ── VmRequestHandle variants ──────────────────────────────────────────────────
//
// These mirror the `VmServiceClient`-accepting functions above but accept a
// `VmRequestHandle` instead. Background polling tasks (which hold a handle,
// not the full client) use these variants.

/// Enable or disable HTTP timeline logging via a `VmRequestHandle`.
///
/// Equivalent to [`enable_http_timeline_logging`] but accepts a handle
/// for use in background tasks that do not have access to the full client.
pub async fn enable_http_timeline_logging_handle(
    handle: &VmRequestHandle,
    isolate_id: &str,
    enabled: bool,
) -> Result<bool> {
    let mut args = HashMap::new();
    args.insert("enabled".to_string(), enabled.to_string());
    let result = handle
        .call_extension(ext::HTTP_ENABLE_TIMELINE_LOGGING, isolate_id, Some(args))
        .await?;
    result
        .get("enabled")
        .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
        .ok_or_else(|| Error::protocol("missing 'enabled' in httpEnableTimelineLogging response"))
}

/// Fetch the HTTP profile via a `VmRequestHandle`.
///
/// Equivalent to [`get_http_profile`] but accepts a handle for use in
/// background polling tasks.
pub async fn get_http_profile_handle(
    handle: &VmRequestHandle,
    isolate_id: &str,
    updated_since: Option<i64>,
) -> Result<HttpProfile> {
    let mut args = HashMap::new();
    if let Some(ts) = updated_since {
        args.insert("updatedSince".to_string(), ts.to_string());
    }
    let args = if args.is_empty() { None } else { Some(args) };
    let result = handle
        .call_extension(ext::GET_HTTP_PROFILE, isolate_id, args)
        .await?;
    parse_http_profile(&result)
}

/// Fetch full request detail via a `VmRequestHandle`.
///
/// Equivalent to [`get_http_profile_request`] but accepts a handle for use
/// in background tasks.
pub async fn get_http_profile_request_handle(
    handle: &VmRequestHandle,
    isolate_id: &str,
    request_id: &str,
) -> Result<HttpProfileEntryDetail> {
    let mut args = HashMap::new();
    args.insert("id".to_string(), request_id.to_string());
    let result = handle
        .call_extension(ext::GET_HTTP_PROFILE_REQUEST, isolate_id, Some(args))
        .await?;
    parse_http_profile_request_detail(&result)
}

/// Clear the HTTP profile via a `VmRequestHandle`.
///
/// Equivalent to [`clear_http_profile`] but accepts a handle for use in
/// background tasks.
pub async fn clear_http_profile_handle(handle: &VmRequestHandle, isolate_id: &str) -> Result<()> {
    handle
        .call_extension(ext::CLEAR_HTTP_PROFILE, isolate_id, None)
        .await?;
    Ok(())
}

/// Enable or disable socket profiling via a `VmRequestHandle`.
///
/// Equivalent to [`set_socket_profiling_enabled`] but accepts a handle for
/// use in background tasks.
pub async fn set_socket_profiling_enabled_handle(
    handle: &VmRequestHandle,
    isolate_id: &str,
    enabled: bool,
) -> Result<bool> {
    let mut args = HashMap::new();
    args.insert("enabled".to_string(), enabled.to_string());
    let result = handle
        .call_extension(ext::SOCKET_PROFILING_ENABLED, isolate_id, Some(args))
        .await?;
    result
        .get("enabled")
        .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
        .ok_or_else(|| Error::protocol("missing 'enabled' in socketProfilingEnabled response"))
}

// ── get_socket_profile ────────────────────────────────────────────────────────

/// Fetch socket profiling data.
///
/// Returns all sockets (open and closed) recorded since socket profiling was
/// last enabled. Enable socket profiling first with
/// [`set_socket_profiling_enabled`].
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the VM Service client is closed, or
/// [`Error::Protocol`] if the response cannot be parsed.
pub async fn get_socket_profile(
    client: &VmServiceClient,
    isolate_id: &str,
) -> Result<Vec<SocketEntry>> {
    let result = client
        .call_extension(ext::GET_SOCKET_PROFILE, isolate_id, None)
        .await?;
    parse_socket_profile(&result)
}

/// Parse a `getSocketProfile` response into a list of [`SocketEntry`]s.
///
/// Individual entries that cannot be parsed are silently skipped.
pub fn parse_socket_profile(result: &serde_json::Value) -> Result<Vec<SocketEntry>> {
    let sockets = result
        .get("sockets")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_socket_entry).collect())
        .unwrap_or_default();
    Ok(sockets)
}

/// Parse a single socket entry from a `getSocketProfile` response.
fn parse_socket_entry(value: &serde_json::Value) -> Option<SocketEntry> {
    Some(SocketEntry {
        id: value.get("id")?.as_str()?.to_string(),
        address: value
            .get("address")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        port: value
            .get("port")
            .and_then(|v| v.as_u64())
            .and_then(|v| u16::try_from(v).ok())
            .unwrap_or(0),
        socket_type: value
            .get("socketType")
            .and_then(|v| v.as_str())
            .unwrap_or("tcp")
            .to_string(),
        start_time_us: value.get("startTime").and_then(|v| v.as_i64()).unwrap_or(0),
        end_time_us: value.get("endTime").and_then(|v| v.as_i64()),
        read_bytes: value.get("readBytes").and_then(|v| v.as_u64()).unwrap_or(0),
        write_bytes: value
            .get("writeBytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

// ── set_socket_profiling_enabled ──────────────────────────────────────────────

/// Enable or disable socket profiling.
///
/// Returns the resulting enabled state as confirmed by the VM Service.
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the response is missing the `enabled` field,
/// or [`Error::ChannelClosed`] if the VM Service client is closed.
pub async fn set_socket_profiling_enabled(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: bool,
) -> Result<bool> {
    let mut args = HashMap::new();
    args.insert("enabled".to_string(), enabled.to_string());
    let result = client
        .call_extension(ext::SOCKET_PROFILING_ENABLED, isolate_id, Some(args))
        .await?;
    result
        .get("enabled")
        .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
        .ok_or_else(|| Error::protocol("missing 'enabled' in socketProfilingEnabled response"))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_http_profile_empty() {
        let result = json!({ "type": "HttpProfile", "timestamp": 1000, "requests": [] });
        let profile = parse_http_profile(&result).unwrap();
        assert_eq!(profile.timestamp, 1000);
        assert!(profile.requests.is_empty());
    }

    #[test]
    fn test_parse_http_profile_with_requests() {
        let result = json!({
            "timestamp": 5000,
            "requests": [{
                "id": "req_1",
                "method": "GET",
                "uri": "https://example.com/api",
                "startTime": 1000,
                "endTime": 2000,
                "request": { "contentLength": 0 },
                "response": {
                    "statusCode": 200,
                    "contentLength": 512,
                    "headers": { "content-type": ["application/json"] }
                }
            }]
        });
        let profile = parse_http_profile(&result).unwrap();
        assert_eq!(profile.requests.len(), 1);
        let req = &profile.requests[0];
        assert_eq!(req.id, "req_1");
        assert_eq!(req.method, "GET");
        assert_eq!(req.status_code, Some(200));
        assert_eq!(req.content_type, Some("application/json".to_string()));
        assert_eq!(req.response_content_length, Some(512));
    }

    #[test]
    fn test_parse_http_profile_entry_pending() {
        let value = json!({
            "id": "req_2", "method": "POST", "uri": "https://example.com/submit",
            "startTime": 1000,
            "request": {}, "response": null
        });
        let entry = parse_http_profile_entry(&value).unwrap();
        assert!(entry.is_pending());
        assert!(entry.status_code.is_none());
    }

    #[test]
    fn test_parse_body_bytes() {
        let value = json!([72, 101, 108, 108, 111]);
        let bytes = parse_body_bytes(Some(&value));
        assert_eq!(bytes, b"Hello");
    }

    #[test]
    fn test_parse_body_bytes_empty() {
        let bytes = parse_body_bytes(None);
        assert!(bytes.is_empty());
    }

    #[test]
    fn test_parse_body_bytes_null_value() {
        let bytes = parse_body_bytes(Some(&json!(null)));
        assert!(bytes.is_empty());
    }

    #[test]
    fn test_parse_headers() {
        let value = json!({
            "content-type": ["application/json"],
            "accept": ["text/html", "application/json"]
        });
        let headers = parse_headers(Some(&value));
        assert_eq!(headers.len(), 2);
        let ct = headers.iter().find(|(k, _)| k == "content-type").unwrap();
        assert_eq!(ct.1, vec!["application/json"]);
        let accept = headers.iter().find(|(k, _)| k == "accept").unwrap();
        assert_eq!(accept.1, vec!["text/html", "application/json"]);
    }

    #[test]
    fn test_parse_headers_none() {
        let headers = parse_headers(None);
        assert!(headers.is_empty());
    }

    #[test]
    fn test_parse_http_profile_request_detail() {
        let result = json!({
            "id": "req_1", "method": "POST", "uri": "https://example.com/api",
            "startTime": 1000, "endTime": 2000,
            "request": {
                "headers": { "content-type": ["application/json"] },
                "contentLength": 5,
                "connectionInfo": {
                    "localPort": 54321,
                    "remoteAddress": "93.184.216.34",
                    "remotePort": 443
                }
            },
            "response": {
                "statusCode": 201,
                "headers": { "content-length": ["128"] },
                "contentLength": 128
            },
            "requestBody": [123, 34, 97, 34, 125],
            "responseBody": [123, 34, 111, 107, 34, 125],
            "events": [
                { "event": "connection established", "timestamp": 1010 },
                { "event": "response started", "timestamp": 1500 }
            ]
        });
        let detail = parse_http_profile_request_detail(&result).unwrap();
        assert_eq!(detail.entry.method, "POST");
        assert_eq!(detail.entry.status_code, Some(201));
        assert_eq!(detail.request_body, b"{\"a\"}");
        assert_eq!(detail.response_body, b"{\"ok\"}");
        assert_eq!(detail.events.len(), 2);
        assert_eq!(
            detail.connection_info.as_ref().unwrap().remote_port,
            Some(443)
        );
    }

    #[test]
    fn test_parse_http_profile_request_detail_missing_required_fields_returns_error() {
        let result = json!({ "startTime": 1000 }); // Missing id, method, uri
        assert!(parse_http_profile_request_detail(&result).is_err());
    }

    #[test]
    fn test_parse_socket_profile() {
        let result = json!({
            "sockets": [{
                "id": "sock_1",
                "address": "93.184.216.34",
                "port": 443,
                "socketType": "tcp",
                "startTime": 1000,
                "endTime": 2000,
                "readBytes": 4096,
                "writeBytes": 512
            }]
        });
        let sockets = parse_socket_profile(&result).unwrap();
        assert_eq!(sockets.len(), 1);
        assert_eq!(sockets[0].port, 443);
        assert_eq!(sockets[0].read_bytes, 4096);
    }

    #[test]
    fn test_parse_socket_profile_empty() {
        let result = json!({ "sockets": [] });
        let sockets = parse_socket_profile(&result).unwrap();
        assert!(sockets.is_empty());
    }

    #[test]
    fn test_parse_connection_info() {
        let value = json!({
            "localPort": 54321,
            "remoteAddress": "93.184.216.34",
            "remotePort": 443
        });
        let info = parse_connection_info(&value).unwrap();
        assert_eq!(info.local_port, Some(54321));
        assert_eq!(info.remote_address, Some("93.184.216.34".to_string()));
        assert_eq!(info.remote_port, Some(443));
    }

    #[test]
    fn test_parse_connection_info_partial() {
        // Only remoteAddress present — ports default to None
        let value = json!({ "remoteAddress": "10.0.0.1" });
        let info = parse_connection_info(&value).unwrap();
        assert!(info.local_port.is_none());
        assert_eq!(info.remote_address, Some("10.0.0.1".to_string()));
        assert!(info.remote_port.is_none());
    }

    #[test]
    fn test_parse_http_profile_no_requests_field() {
        // Graceful degradation: missing "requests" → empty vec
        let result = json!({ "timestamp": 100 });
        let profile = parse_http_profile(&result).unwrap();
        assert_eq!(profile.timestamp, 100);
        assert!(profile.requests.is_empty());
    }

    #[test]
    fn test_parse_http_profile_entry_skips_missing_required_fields() {
        // Entry without "uri" should be skipped
        let value = json!({ "id": "req_1", "method": "GET" });
        assert!(parse_http_profile_entry(&value).is_none());
    }

    #[test]
    fn test_parse_http_profile_entry_with_error() {
        let value = json!({
            "id": "req_err", "method": "GET", "uri": "https://example.com",
            "startTime": 0,
            "request": { "error": "Connection refused" }
        });
        let entry = parse_http_profile_entry(&value).unwrap();
        assert_eq!(entry.error, Some("Connection refused".to_string()));
    }

    #[test]
    fn test_parse_socket_entry_skips_missing_id() {
        let value = json!({ "address": "1.2.3.4", "port": 80 });
        assert!(parse_socket_entry(&value).is_none());
    }

    #[test]
    fn test_parse_socket_profile_partial_entries() {
        // One valid entry, one missing "id" — only valid entry is returned
        let result = json!({
            "sockets": [
                { "id": "sock_1", "address": "1.2.3.4", "port": 80, "socketType": "tcp",
                  "startTime": 0, "readBytes": 0, "writeBytes": 0 },
                { "address": "5.6.7.8", "port": 443 }  // Missing id
            ]
        });
        let sockets = parse_socket_profile(&result).unwrap();
        assert_eq!(sockets.len(), 1);
        assert_eq!(sockets[0].id, "sock_1");
    }
}
