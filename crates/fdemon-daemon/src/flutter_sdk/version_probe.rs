//! # Flutter Version Probe
//!
//! Async probe that runs `flutter --version --machine` and parses the JSON
//! output into a [`FlutterVersionInfo`] struct with extended SDK metadata.
//!
//! This is an async enrichment step and should be called from a background
//! task, not from the main render loop.

use fdemon_core::prelude::*;

use super::types::{FlutterExecutable, FlutterVersionInfo};

/// Timeout for the `flutter --version --machine` subprocess.
const VERSION_PROBE_TIMEOUT_SECS: u64 = 30;

/// Probes the Flutter SDK for extended version metadata by running
/// `flutter --version --machine` and parsing the JSON output.
///
/// This is an async operation that spawns a subprocess. It should be called
/// from a background task, not from the main render loop.
///
/// # Arguments
/// * `executable` — The Flutter executable to invoke
///
/// # Returns
/// * `Ok(FlutterVersionInfo)` with parsed metadata
/// * `Err(...)` if the subprocess fails or output is not valid JSON
pub async fn probe_flutter_version(executable: &FlutterExecutable) -> Result<FlutterVersionInfo> {
    let mut cmd = executable.command();
    cmd.args(["--version", "--machine"]);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(VERSION_PROBE_TIMEOUT_SECS),
        cmd.output(),
    )
    .await
    .map_err(|_| {
        Error::config(format!(
            "flutter --version --machine timed out after {VERSION_PROBE_TIMEOUT_SECS}s"
        ))
    })?
    .map_err(|e| Error::config(format!("failed to run flutter --version --machine: {e}")))?;

    if !output.status.success() {
        return Err(Error::config(format!(
            "flutter --version --machine exited with status {}",
            output.status
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_version_json(&stdout)
}

/// Parse the JSON output from `flutter --version --machine`.
///
/// Uses `serde_json::Value` parsing instead of a derived `Deserialize` struct
/// to avoid tight coupling to the exact JSON schema and to handle missing
/// fields gracefully as `None`.
fn parse_version_json(json_str: &str) -> Result<FlutterVersionInfo> {
    let value: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
        Error::config(format!(
            "invalid JSON from flutter --version --machine: {e}"
        ))
    })?;

    Ok(FlutterVersionInfo {
        framework_version: value
            .get("frameworkVersion")
            .and_then(|v| v.as_str())
            .map(String::from),
        channel: value
            .get("channel")
            .and_then(|v| v.as_str())
            .map(String::from),
        repository_url: value
            .get("repositoryUrl")
            .and_then(|v| v.as_str())
            .map(String::from),
        framework_revision: value
            .get("frameworkRevision")
            .and_then(|v| v.as_str())
            .map(String::from),
        framework_commit_date: value
            .get("frameworkCommitDate")
            .and_then(|v| v.as_str())
            .map(String::from),
        engine_revision: value
            .get("engineRevision")
            .and_then(|v| v.as_str())
            .map(String::from),
        dart_sdk_version: value
            .get("dartSdkVersion")
            .and_then(|v| v.as_str())
            .map(String::from),
        devtools_version: value
            .get("devToolsVersion")
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_json_full() {
        let json = r#"{
            "frameworkVersion": "3.38.6",
            "channel": "stable",
            "repositoryUrl": "https://github.com/flutter/flutter.git",
            "frameworkRevision": "8b87286849",
            "frameworkCommitDate": "2026-01-08 10:49:17 -0800",
            "engineRevision": "6f3039bf7c",
            "dartSdkVersion": "3.10.7",
            "devToolsVersion": "2.51.1"
        }"#;
        let info = parse_version_json(json).unwrap();
        assert_eq!(info.framework_version.as_deref(), Some("3.38.6"));
        assert_eq!(info.channel.as_deref(), Some("stable"));
        assert_eq!(
            info.repository_url.as_deref(),
            Some("https://github.com/flutter/flutter.git")
        );
        assert_eq!(info.framework_revision.as_deref(), Some("8b87286849"));
        assert_eq!(
            info.framework_commit_date.as_deref(),
            Some("2026-01-08 10:49:17 -0800")
        );
        assert_eq!(info.engine_revision.as_deref(), Some("6f3039bf7c"));
        assert_eq!(info.dart_sdk_version.as_deref(), Some("3.10.7"));
        assert_eq!(info.devtools_version.as_deref(), Some("2.51.1"));
    }

    #[test]
    fn test_parse_version_json_partial() {
        let json = r#"{"frameworkVersion": "3.38.6"}"#;
        let info = parse_version_json(json).unwrap();
        assert_eq!(info.framework_version.as_deref(), Some("3.38.6"));
        assert!(info.channel.is_none());
        assert!(info.repository_url.is_none());
        assert!(info.framework_revision.is_none());
        assert!(info.framework_commit_date.is_none());
        assert!(info.engine_revision.is_none());
        assert!(info.dart_sdk_version.is_none());
        assert!(info.devtools_version.is_none());
    }

    #[test]
    fn test_parse_version_json_empty_object() {
        let json = "{}";
        let info = parse_version_json(json).unwrap();
        assert!(info.framework_version.is_none());
        assert!(info.channel.is_none());
        assert!(info.engine_revision.is_none());
        assert!(info.devtools_version.is_none());
    }

    #[test]
    fn test_parse_version_json_invalid() {
        let result = parse_version_json("not json");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid JSON from flutter --version --machine"));
    }

    #[test]
    fn test_parse_version_json_non_object_json() {
        // Valid JSON but not an object — should produce all-None fields
        // serde_json::Value::get() on a non-object returns None
        let result = parse_version_json("[]");
        assert!(result.is_ok());
        let info = result.unwrap();
        assert!(info.framework_version.is_none());
    }

    #[test]
    fn test_parse_version_json_extra_fields_are_ignored() {
        let json = r#"{
            "frameworkVersion": "3.38.6",
            "flutterRoot": "/home/user/flutter",
            "unknownFutureField": "some value"
        }"#;
        let info = parse_version_json(json).unwrap();
        assert_eq!(info.framework_version.as_deref(), Some("3.38.6"));
        // Extra fields are silently ignored
    }

    #[test]
    fn test_parse_version_json_numeric_field_produces_none() {
        // If a field is numeric instead of a string, it should produce None gracefully
        let json = r#"{"frameworkVersion": 3}"#;
        let info = parse_version_json(json).unwrap();
        assert!(info.framework_version.is_none());
    }
}
