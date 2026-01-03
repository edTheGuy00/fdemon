//! JSON-RPC protocol handling for Flutter daemon

use serde::{Deserialize, Serialize};

/// Strip the outer brackets from a daemon message
///
/// The Flutter daemon wraps all messages in `[...]` for resilience.
/// Returns the inner content if brackets are present.
pub fn strip_brackets(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        Some(&trimmed[1..trimmed.len() - 1])
    } else {
        None
    }
}

/// A raw daemon message (before parsing into typed events)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RawMessage {
    /// A response to a request we sent
    Response {
        id: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<serde_json::Value>,
    },
    /// An event from the daemon (unsolicited)
    Event {
        event: String,
        params: serde_json::Value,
    },
}

impl RawMessage {
    /// Parse a JSON string into a RawMessage
    pub fn parse(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    /// Check if this is an event
    pub fn is_event(&self) -> bool {
        matches!(self, RawMessage::Event { .. })
    }

    /// Get the event name if this is an event
    pub fn event_name(&self) -> Option<&str> {
        match self {
            RawMessage::Event { event, .. } => Some(event),
            _ => None,
        }
    }

    /// Get a human-readable summary of this message
    pub fn summary(&self) -> String {
        match self {
            RawMessage::Response { id, error, .. } => {
                if error.is_some() {
                    format!("Response #{}: error", id)
                } else {
                    format!("Response #{}: ok", id)
                }
            }
            RawMessage::Event { event, .. } => {
                format!("Event: {}", event)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_brackets_valid() {
        assert_eq!(
            strip_brackets(r#"[{"event":"test"}]"#),
            Some(r#"{"event":"test"}"#)
        );
    }

    #[test]
    fn test_strip_brackets_whitespace() {
        assert_eq!(strip_brackets("  [content]  "), Some("content"));
    }

    #[test]
    fn test_strip_brackets_invalid() {
        assert_eq!(strip_brackets("no brackets"), None);
        assert_eq!(strip_brackets("[missing end"), None);
        assert_eq!(strip_brackets("missing start]"), None);
    }

    #[test]
    fn test_parse_event() {
        let json = r#"{"event":"app.log","params":{"message":"hello"}}"#;
        let msg = RawMessage::parse(json).unwrap();
        assert!(msg.is_event());
        assert_eq!(msg.event_name(), Some("app.log"));
    }

    #[test]
    fn test_parse_response() {
        let json = r#"{"id":1,"result":"0.1.0"}"#;
        let msg = RawMessage::parse(json).unwrap();
        assert!(!msg.is_event());
    }

    #[test]
    fn test_parse_invalid_json() {
        assert!(RawMessage::parse("not json").is_none());
    }

    #[test]
    fn test_message_summary() {
        let event = RawMessage::parse(r#"{"event":"app.log","params":{}}"#).unwrap();
        assert_eq!(event.summary(), "Event: app.log");

        let response = RawMessage::parse(r#"{"id":1,"result":"ok"}"#).unwrap();
        assert_eq!(response.summary(), "Response #1: ok");

        let error_resp = RawMessage::parse(r#"{"id":2,"error":"failed"}"#).unwrap();
        assert_eq!(error_resp.summary(), "Response #2: error");
    }
}
