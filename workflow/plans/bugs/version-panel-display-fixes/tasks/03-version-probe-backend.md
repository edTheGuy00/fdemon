## Task: Add Async `flutter --version --machine` Probe Backend

**Objective**: Create an async function that runs `flutter --version --machine` and parses the JSON output into a structured result, providing the complete version metadata that file-based detection cannot.

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/version_probe.rs`: **NEW** — async probe runner + JSON parser
- `crates/fdemon-daemon/src/flutter_sdk/types.rs`: Add `FlutterVersionInfo` struct for extended metadata
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: Re-export new types and module
- `crates/fdemon-daemon/src/lib.rs`: Re-export `FlutterVersionInfo` if needed

### Details

**`flutter --version --machine` output format (JSON):**

```json
{
  "frameworkVersion": "3.38.6",
  "channel": "stable",
  "repositoryUrl": "https://github.com/flutter/flutter.git",
  "frameworkRevision": "8b87286849",
  "frameworkCommitDate": "2026-01-08 10:49:17 -0800",
  "engineRevision": "6f3039bf7c3cb5306513c75092822d4d94716003",
  "dartSdkVersion": "3.10.7",
  "devToolsVersion": "2.51.1",
  "flutterRoot": "/path/to/flutter"
}
```

**New types:**

```rust
// types.rs — new struct for extended metadata from `flutter --version --machine`

/// Extended Flutter SDK metadata obtained from `flutter --version --machine`.
///
/// All fields are optional because the probe is async and may fail.
/// This complements the file-based `FlutterSdk` detection with richer metadata
/// that can only be obtained by running the Flutter CLI.
#[derive(Debug, Clone, Default)]
pub struct FlutterVersionInfo {
    /// Full Flutter framework version (e.g., "3.38.6")
    pub framework_version: Option<String>,
    /// Release channel (e.g., "stable", "beta", "main")
    pub channel: Option<String>,
    /// Git repository URL
    pub repository_url: Option<String>,
    /// Framework commit hash (e.g., "8b87286849")
    pub framework_revision: Option<String>,
    /// Framework commit date (e.g., "2026-01-08 10:49:17 -0800")
    pub framework_commit_date: Option<String>,
    /// Engine revision hash
    pub engine_revision: Option<String>,
    /// Bundled Dart SDK version (e.g., "3.10.7")
    pub dart_sdk_version: Option<String>,
    /// Bundled DevTools version (e.g., "2.51.1")
    pub devtools_version: Option<String>,
}
```

**New module — `version_probe.rs`:**

```rust
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
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        cmd.output()
    ).await
    .map_err(|_| Error::config("flutter --version --machine timed out after 30s"))?
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
fn parse_version_json(json_str: &str) -> Result<FlutterVersionInfo> {
    let value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| Error::config(format!("invalid JSON from flutter --version --machine: {e}")))?;

    Ok(FlutterVersionInfo {
        framework_version: value.get("frameworkVersion").and_then(|v| v.as_str()).map(String::from),
        channel: value.get("channel").and_then(|v| v.as_str()).map(String::from),
        repository_url: value.get("repositoryUrl").and_then(|v| v.as_str()).map(String::from),
        framework_revision: value.get("frameworkRevision").and_then(|v| v.as_str()).map(String::from),
        framework_commit_date: value.get("frameworkCommitDate").and_then(|v| v.as_str()).map(String::from),
        engine_revision: value.get("engineRevision").and_then(|v| v.as_str()).map(String::from),
        dart_sdk_version: value.get("dartSdkVersion").and_then(|v| v.as_str()).map(String::from),
        devtools_version: value.get("devToolsVersion").and_then(|v| v.as_str()).map(String::from),
    })
}
```

**Key design decisions:**
- 30-second timeout: `flutter --version` can be slow on first run (downloads Dart SDK)
- `serde_json::Value` parsing instead of a derived `Deserialize` struct: avoids tight coupling to the exact JSON schema, handles missing fields gracefully
- All fields `Option<String>`: probe failure leaves a usable default
- Stderr suppressed: Flutter CLI may print progress messages to stderr
- `FlutterVersionInfo` is a separate struct from `FlutterSdk` — it represents probe results, not the base detection

### Acceptance Criteria

1. `probe_flutter_version()` runs `flutter --version --machine` with the correct executable
2. JSON output is parsed into `FlutterVersionInfo` with all 8 fields
3. Missing JSON fields result in `None`, not errors
4. Subprocess timeout (30s) returns a proper error
5. Non-zero exit status returns a proper error
6. Invalid JSON returns a proper error
7. `FlutterVersionInfo` is re-exported from `fdemon_daemon::flutter_sdk`
8. Unit tests cover: valid JSON parsing, partial JSON, empty JSON, invalid JSON

### Testing

```rust
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
        assert_eq!(info.framework_revision.as_deref(), Some("8b87286849"));
        assert_eq!(info.engine_revision.as_deref(), Some("6f3039bf7c"));
        assert_eq!(info.dart_sdk_version.as_deref(), Some("3.10.7"));
        assert_eq!(info.devtools_version.as_deref(), Some("2.51.1"));
    }

    #[test]
    fn test_parse_version_json_partial() {
        let json = r#"{"frameworkVersion": "3.38.6"}"#;
        let info = parse_version_json(json).unwrap();
        assert_eq!(info.framework_version.as_deref(), Some("3.38.6"));
        assert!(info.engine_revision.is_none());
        assert!(info.devtools_version.is_none());
    }

    #[test]
    fn test_parse_version_json_empty_object() {
        let json = "{}";
        let info = parse_version_json(json).unwrap();
        assert!(info.framework_version.is_none());
    }

    #[test]
    fn test_parse_version_json_invalid() {
        let result = parse_version_json("not json");
        assert!(result.is_err());
    }
}
```

### Notes

- Check if `serde_json` is already in `fdemon-daemon`'s `Cargo.toml` dependencies (it likely is for JSON-RPC parsing).
- The probe function is deliberately not called during `find_flutter_sdk()` — it's an async enrichment step triggered separately.
- Consider whether the 30s timeout is reasonable. On CI or cold starts, Flutter may download the Dart SDK on first version check.
- The probe should use the same `FlutterExecutable` from the resolved SDK, not a hardcoded `flutter` path.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/flutter_sdk/types.rs` | Added `FlutterVersionInfo` struct with 8 `Option<String>` fields and `#[derive(Debug, Clone, Default)]` |
| `crates/fdemon-daemon/src/flutter_sdk/version_probe.rs` | NEW — async `probe_flutter_version()` + private `parse_version_json()` with 7 unit tests |
| `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | Declared `version_probe` module, re-exported `FlutterVersionInfo`, `probe_flutter_version`, added doc section |
| `crates/fdemon-daemon/src/lib.rs` | Added `FlutterVersionInfo` and `probe_flutter_version` to public re-exports and crate doc comments |

### Notable Decisions/Tradeoffs

1. **Named timeout constant**: Used `const VERSION_PROBE_TIMEOUT_SECS: u64 = 30` following CODE_STANDARDS no-magic-numbers rule, rather than inlining `30` in the timeout call.

2. **serde_json::Value instead of Deserialize derive**: Matches the task spec's design decision — avoids tight coupling to the JSON schema and handles future field additions or type mismatches without breakage. A numeric `frameworkVersion` field gracefully produces `None` rather than an error.

3. **Extra tests beyond spec**: Added `test_parse_version_json_non_object_json` (array input), `test_parse_version_json_extra_fields_are_ignored`, and `test_parse_version_json_numeric_field_produces_none` to improve edge-case coverage per CODE_STANDARDS testing requirements.

4. **`Error::config` for all probe errors**: Probe failures (timeout, non-zero exit, invalid JSON) use `Error::config` which is recoverable (`is_fatal()` returns false), consistent with how the probe is meant to be an optional enrichment step.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (734 tests total; all 7 new version_probe tests pass)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (zero warnings)

### Risks/Limitations

1. **No integration test for subprocess execution**: `probe_flutter_version()` is not integration-tested because it requires a real Flutter executable on PATH. The JSON parsing logic (which contains all the non-trivial logic) is fully unit tested via `parse_version_json()`.

2. **Pre-existing flaky test**: `flutter_sdk::locator::tests::test_flutter_wrapper_detection` occasionally fails when run alongside other tests (environment state issue). This is unrelated to this task and passes in isolation.
