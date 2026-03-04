//! # DAP Content-Length Codec
//!
//! Implements the framing protocol used by the Debug Adapter Protocol (DAP).
//! Each message is preceded by an HTTP-like header section containing a
//! `Content-Length` field that specifies the byte length of the JSON body.
//!
//! ## Wire Format
//!
//! ```text
//! Content-Length: <byte-count>\r\n
//! \r\n
//! <utf-8-encoded-json-body>
//! ```
//!
//! Rules:
//! - The header section is ASCII text.
//! - `Content-Length` is the **byte count** (not character count) of the JSON body.
//! - Headers are terminated by a blank line (`\r\n\r\n`).
//! - The body is UTF-8 encoded JSON.
//! - Additional headers are legal per the spec but currently only `Content-Length`
//!   is defined; unknown headers are silently ignored.

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use fdemon_core::error::{Error, Result};

use super::types::DapMessage;

/// Maximum allowed message body size (10 MB).
///
/// Prevents allocation of arbitrarily large buffers from a malformed or
/// malicious `Content-Length` header before any bytes are read.
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Read a single DAP message from the given async reader.
///
/// Reads the header section line-by-line, extracts the `Content-Length` value,
/// then reads exactly that many bytes and deserializes the JSON body.
///
/// # Returns
/// - `Ok(Some(msg))` when a message was successfully read.
/// - `Ok(None)` on a clean EOF (the stream was closed before any header bytes
///   were received — i.e., the very first `read_line` returns 0 bytes).
/// - `Err(_)` for malformed headers, oversized messages, I/O errors, or JSON
///   deserialization failures.
///
/// # Errors
/// - [`Error::Protocol`] — Missing `Content-Length`, oversized message, or
///   invalid JSON body.
/// - [`Error::Io`] — Any underlying I/O error from the stream.
pub async fn read_message<R>(reader: &mut BufReader<R>) -> Result<Option<DapMessage>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut content_length: Option<usize> = None;

    // ── 1. Read headers line-by-line until blank line ─────────────────────
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;

        // A zero-byte read on the very first line means clean EOF.
        if bytes_read == 0 {
            if content_length.is_none() {
                // No headers seen at all — this is a clean stream close.
                return Ok(None);
            }
            // EOF mid-header-block is an unexpected termination.
            return Err(Error::protocol(
                "DAP stream closed unexpectedly while reading headers",
            ));
        }

        // Normalize line endings: strip trailing \r\n or \n.
        let trimmed = line.trim_end_matches(['\r', '\n']);

        // A blank line signals the end of the header block.
        if trimmed.is_empty() {
            break;
        }

        // Parse known headers; ignore unknown ones (lenient per spec).
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            let value = value.trim();
            let length: usize = value.parse().map_err(|_| {
                Error::protocol(format!("DAP: invalid Content-Length value: {:?}", value))
            })?;
            content_length = Some(length);
        }
        // Unknown headers (e.g., Content-Type) are silently ignored.
    }

    // ── 2. Validate Content-Length was present ────────────────────────────
    let content_length = content_length.ok_or_else(|| {
        Error::protocol("DAP: message header block missing required Content-Length field")
    })?;

    // ── 3. Guard against oversized allocations ────────────────────────────
    if content_length > MAX_MESSAGE_SIZE {
        return Err(Error::protocol(format!(
            "DAP: message body size {} exceeds maximum allowed size of {} bytes",
            content_length, MAX_MESSAGE_SIZE
        )));
    }

    // ── 4. Read exactly content_length bytes ──────────────────────────────
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).await?;

    // ── 5. Deserialize JSON body ──────────────────────────────────────────
    let message = serde_json::from_slice::<DapMessage>(&body).map_err(|e| {
        // Include a truncated snippet of the raw body for diagnostics.
        let snippet = std::str::from_utf8(&body)
            .unwrap_or("<invalid UTF-8>")
            .chars()
            .take(200)
            .collect::<String>();
        Error::protocol(format!(
            "DAP: failed to deserialize message body: {} — body: {:?}",
            e, snippet
        ))
    })?;

    Ok(Some(message))
}

/// Write a single DAP message to the given async writer.
///
/// Serializes `message` to JSON, then writes:
/// 1. `Content-Length: {byte_length}\r\n\r\n`
/// 2. The JSON bytes.
/// 3. Flushes the underlying writer.
///
/// # Errors
/// - [`Error::Json`] — If JSON serialization fails (highly unlikely for
///   well-formed `DapMessage` values).
/// - [`Error::Io`] — Any underlying I/O error on the writer.
pub async fn write_message<W>(writer: &mut W, message: &DapMessage) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    // ── 1. Serialize to JSON bytes ─────────────────────────────────────────
    let body = serde_json::to_vec(message)?;

    // ── 2. Write Content-Length header + blank line ───────────────────────
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes()).await?;

    // ── 3. Write JSON body ────────────────────────────────────────────────
    writer.write_all(&body).await?;

    // ── 4. Flush ──────────────────────────────────────────────────────────
    writer.flush().await?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::types::{Capabilities, DapEvent, DapRequest, DapResponse};

    // ── write_message helpers ─────────────────────────────────────────────────

    /// Serialize a `DapMessage` to a Vec<u8> using `write_message`.
    async fn encode(msg: &DapMessage) -> Vec<u8> {
        let mut buf = Vec::new();
        write_message(&mut buf, msg).await.unwrap();
        buf
    }

    /// Parse a `DapMessage` from raw bytes using `read_message`.
    async fn decode(bytes: &[u8]) -> Option<DapMessage> {
        let mut reader = BufReader::new(bytes);
        read_message(&mut reader).await.unwrap()
    }

    // ── Roundtrip tests ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_write_then_read_roundtrip_event() {
        let msg = DapMessage::Event(DapEvent::initialized());
        let bytes = encode(&msg).await;
        let result = decode(&bytes).await.unwrap();
        match result {
            DapMessage::Event(e) => {
                assert_eq!(e.event, "initialized");
                assert!(e.body.is_none());
            }
            other => panic!("Expected Event, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_write_then_read_roundtrip_request() {
        let msg = DapMessage::Request(DapRequest {
            seq: 99,
            command: "setBreakpoints".into(),
            arguments: Some(serde_json::json!({"source": {"path": "/app/main.dart"}})),
        });
        let bytes = encode(&msg).await;
        let result = decode(&bytes).await.unwrap();
        match result {
            DapMessage::Request(r) => {
                assert_eq!(r.seq, 99);
                assert_eq!(r.command, "setBreakpoints");
                let args = r.arguments.as_ref().unwrap();
                assert_eq!(args["source"]["path"], "/app/main.dart");
            }
            other => panic!("Expected Request, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_write_then_read_roundtrip_response() {
        let req = DapRequest {
            seq: 1,
            command: "initialize".into(),
            arguments: None,
        };
        let body = serde_json::to_value(Capabilities::fdemon_defaults()).unwrap();
        let msg = DapMessage::Response(DapResponse::success(&req, Some(body)));
        let bytes = encode(&msg).await;
        let result = decode(&bytes).await.unwrap();
        match result {
            DapMessage::Response(r) => {
                assert_eq!(r.request_seq, 1);
                assert!(r.success);
                assert_eq!(r.command, "initialize");
                assert!(r.body.is_some());
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_write_then_read_multiple_messages_sequentially() {
        // Write two messages back-to-back and read them sequentially.
        let msg1 = DapMessage::Event(DapEvent::initialized());
        let msg2 = DapMessage::Event(DapEvent::terminated());

        let mut buf = Vec::new();
        write_message(&mut buf, &msg1).await.unwrap();
        write_message(&mut buf, &msg2).await.unwrap();

        let mut reader = BufReader::new(buf.as_slice());
        let r1 = read_message(&mut reader).await.unwrap().unwrap();
        let r2 = read_message(&mut reader).await.unwrap().unwrap();
        // Third read should see EOF.
        let r3 = read_message(&mut reader).await.unwrap();

        match r1 {
            DapMessage::Event(e) => assert_eq!(e.event, "initialized"),
            other => panic!("Expected Event(initialized), got {:?}", other),
        }
        match r2 {
            DapMessage::Event(e) => assert_eq!(e.event, "terminated"),
            other => panic!("Expected Event(terminated), got {:?}", other),
        }
        assert!(r3.is_none(), "Expected None on EOF after two messages");
    }

    // ── Content-Length header tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_write_message_content_length_is_accurate() {
        let msg = DapMessage::Event(DapEvent::output("stdout", "hello world\n"));
        let bytes = encode(&msg).await;

        // Parse the header manually to verify Content-Length accuracy.
        let header_end = bytes
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("header separator not found");
        let header_str = std::str::from_utf8(&bytes[..header_end]).unwrap();
        let cl_line = header_str
            .lines()
            .find(|l| l.starts_with("Content-Length:"))
            .expect("Content-Length header missing");
        let declared_len: usize = cl_line
            .strip_prefix("Content-Length:")
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        let body_start = header_end + 4; // skip \r\n\r\n
        let actual_body_len = bytes.len() - body_start;
        assert_eq!(
            declared_len, actual_body_len,
            "Content-Length must equal actual body byte length"
        );
    }

    // ── EOF handling ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_read_message_eof_returns_none() {
        let mut reader = BufReader::new(&b""[..]);
        let result = read_message(&mut reader).await.unwrap();
        assert!(result.is_none(), "Empty stream should return Ok(None)");
    }

    // ── Error handling ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_read_message_missing_content_length() {
        // A header block with no Content-Length field must be rejected.
        let data = b"Content-Type: application/vscode-jsonrpc\r\n\r\n{}";
        let mut reader = BufReader::new(&data[..]);
        let result = read_message(&mut reader).await;
        assert!(
            result.is_err(),
            "Missing Content-Length should return an error"
        );
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Content-Length"),
            "Error should mention Content-Length, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_read_message_invalid_header_no_content_length() {
        let data = b"Invalid-Header: 42\r\n\r\n{}";
        let mut reader = BufReader::new(&data[..]);
        let result = read_message(&mut reader).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_message_oversized_rejected() {
        // Content-Length exceeds MAX_MESSAGE_SIZE — must be rejected before
        // any buffer is allocated.
        let header = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_SIZE + 1);
        let mut reader = BufReader::new(header.as_bytes());
        let result = read_message(&mut reader).await;
        assert!(
            result.is_err(),
            "Oversized Content-Length should be rejected"
        );
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("exceeds maximum"),
            "Error should mention size limit, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_read_message_invalid_json_body() {
        let body = b"not valid json!!";
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut data = header.into_bytes();
        data.extend_from_slice(body);
        let mut reader = BufReader::new(data.as_slice());
        let result = read_message(&mut reader).await;
        assert!(result.is_err(), "Invalid JSON body should return an error");
    }

    #[tokio::test]
    async fn test_read_message_invalid_content_length_value() {
        let data = b"Content-Length: not_a_number\r\n\r\n{}";
        let mut reader = BufReader::new(&data[..]);
        let result = read_message(&mut reader).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_message_extra_unknown_headers_ignored() {
        // DAP parsers should be lenient about extra headers.
        let body_json = serde_json::json!({
            "type": "event",
            "seq": 0,
            "event": "initialized"
        });
        let body_bytes = serde_json::to_vec(&body_json).unwrap();
        let header = format!(
            "Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n",
            body_bytes.len()
        );
        let mut data = header.into_bytes();
        data.extend_from_slice(&body_bytes);

        let mut reader = BufReader::new(data.as_slice());
        let result = read_message(&mut reader).await.unwrap().unwrap();
        match result {
            DapMessage::Event(e) => assert_eq!(e.event, "initialized"),
            other => panic!("Expected Event, got {:?}", other),
        }
    }

    // ── write_message format verification ────────────────────────────────────

    #[tokio::test]
    async fn test_write_message_header_format() {
        let msg = DapMessage::Event(DapEvent::initialized());
        let bytes = encode(&msg).await;
        let as_str = String::from_utf8(bytes.clone()).unwrap();

        // Header must start with "Content-Length: "
        assert!(
            as_str.starts_with("Content-Length: "),
            "Output must start with Content-Length header"
        );
        // Header must contain \r\n\r\n separator
        assert!(
            as_str.contains("\r\n\r\n"),
            "Output must contain \\r\\n\\r\\n header separator"
        );
    }

    #[tokio::test]
    async fn test_write_message_body_is_valid_json() {
        let msg = DapMessage::Event(DapEvent::initialized());
        let bytes = encode(&msg).await;

        // Find header/body separator
        let sep = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let body = &bytes[sep + 4..];

        // Body must be valid JSON
        let parsed: serde_json::Value = serde_json::from_slice(body).unwrap();
        assert_eq!(parsed["type"], "event");
        assert_eq!(parsed["event"], "initialized");
    }
}
