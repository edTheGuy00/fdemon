## Task: Comprehensive Custom Source Tests

**Objective**: Add thorough unit tests covering custom source config parsing, format parsers, the custom runner, and app-layer integration to ensure robust behavior.

**Depends on**: 03-custom-source-runner (implementation must be complete to write meaningful tests)

### Scope

- `crates/fdemon-app/src/config/types.rs` — Config parsing tests (may already be partially covered in task 01)
- `crates/fdemon-daemon/src/native_logs/formats.rs` — Format parser tests (may already be partially covered in task 02)
- `crates/fdemon-daemon/src/native_logs/custom.rs` — Runner integration tests
- `crates/fdemon-app/src/handler/update.rs` — Handler tests for new message variants

### Details

This task focuses on testing edge cases, error paths, and integration scenarios that may not be covered by the unit tests in individual tasks.

#### Config Parsing Edge Cases

```rust
// Round-trip tests
#[test]
fn test_custom_sources_round_trip_serde() {
    let settings = NativeLogsSettings {
        custom_sources: vec![
            CustomSourceConfig {
                name: "test".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                format: OutputFormat::Raw,
                working_dir: None,
                env: HashMap::new(),
            },
        ],
        ..Default::default()
    };
    let toml = toml::to_string(&settings).unwrap();
    let parsed: NativeLogsSettings = toml::from_str(&toml).unwrap();
    assert_eq!(parsed.custom_sources.len(), 1);
    assert_eq!(parsed.custom_sources[0].name, "test");
}

// Multiple custom sources
#[test]
fn test_multiple_custom_sources_parse() {
    let toml = r#"
    [[native_logs.custom_sources]]
    name = "source-a"
    command = "cmd-a"

    [[native_logs.custom_sources]]
    name = "source-b"
    command = "cmd-b"
    format = "json"
    "#;
    // Verify both sources parse with correct defaults
}

// All format variants
#[test]
fn test_all_output_format_variants_deserialize() {
    for (input, expected) in [
        ("raw", OutputFormat::Raw),
        ("json", OutputFormat::Json),
        ("logcat-threadtime", OutputFormat::LogcatThreadtime),
        ("syslog", OutputFormat::Syslog),
    ] {
        // Verify deserialization
    }
}

// Missing optional fields default correctly
#[test]
fn test_custom_source_optional_fields_default() {
    let toml = r#"
    [[native_logs.custom_sources]]
    name = "minimal"
    command = "echo"
    "#;
    // Verify: args=[], format=Raw, working_dir=None, env={}
}

// Env as inline table
#[test]
fn test_custom_source_env_inline_table() {
    let toml = r#"
    [[native_logs.custom_sources]]
    name = "with-env"
    command = "my-tool"
    env = { VERBOSE = "1", PATH_PREFIX = "/opt" }
    "#;
    // Verify both env vars parsed
}
```

#### Format Parser Edge Cases

```rust
// JSON with extra fields (should be ignored)
#[test]
fn test_json_format_ignores_unknown_fields() {
    let line = r#"{"message": "hello", "extra": "ignored", "nested": {"deep": true}}"#;
    let event = parse_line(&OutputFormat::Json, line, "test").unwrap();
    assert_eq!(event.message, "hello");
}

// JSON with numeric level
#[test]
fn test_json_format_string_level_only() {
    let line = r#"{"message": "hello", "level": 3}"#;
    // Numeric level should be ignored (not a string) — default to Info
}

// JSON with empty message
#[test]
fn test_json_format_empty_message_returns_none() {
    let line = r#"{"message": "", "level": "info"}"#;
    assert!(parse_line(&OutputFormat::Json, line, "test").is_none());
}

// Raw with various whitespace
#[test]
fn test_raw_format_whitespace_handling() {
    assert!(parse_line(&OutputFormat::Raw, "", "test").is_none());
    assert!(parse_line(&OutputFormat::Raw, "   ", "test").is_none());
    assert!(parse_line(&OutputFormat::Raw, "\t", "test").is_none());
    let event = parse_line(&OutputFormat::Raw, "  hello  ", "test").unwrap();
    assert_eq!(event.message, "hello");
}

// Logcat threadtime with malformed lines
#[test]
fn test_logcat_threadtime_malformed_returns_none() {
    assert!(parse_line(&OutputFormat::LogcatThreadtime, "not a logcat line", "test").is_none());
    assert!(parse_line(&OutputFormat::LogcatThreadtime, "--------- beginning of main", "test").is_none());
}
```

#### Runner Edge Cases

```rust
#[tokio::test]
async fn test_custom_capture_stderr_does_not_produce_events() {
    // Spawn a command that writes to stderr only
    // Verify: no events produced (stderr is captured but not parsed)
}

#[tokio::test]
async fn test_custom_capture_with_working_dir() {
    // Spawn: pwd
    // with working_dir = "/tmp"
    // Verify: output contains "/tmp"
}

#[tokio::test]
async fn test_custom_capture_respects_exclude_tags() {
    // Configure with json format and exclude_tags = ["noisy"]
    // Send JSON with tag="noisy" and tag="useful"
    // Verify: only "useful" events received
}

#[tokio::test]
async fn test_custom_capture_respects_include_tags() {
    // Configure with json format and include_tags = ["wanted"]
    // Send JSON with tag="wanted" and tag="other"
    // Verify: only "wanted" events received
}

#[tokio::test]
async fn test_custom_capture_concurrent_shutdown() {
    // Start capture, immediately send shutdown
    // Verify: no panic, clean exit
}
```

#### Handler Integration Tests

```rust
#[test]
fn test_custom_source_tags_appear_in_native_tag_state() {
    // Process NativeLog events with custom source tags
    // Verify: tags tracked in NativeTagState via observe_tag()
}

#[test]
fn test_custom_source_respects_enabled_toggle() {
    // Set native_logs.enabled = false
    // Verify: custom sources not spawned
}

#[test]
fn test_custom_source_min_level_filtering() {
    // Set min_level = "warning"
    // Send custom source events at Info and Warning levels
    // Verify: only Warning event passes through
}
```

### Acceptance Criteria

1. Config parsing tests cover all field combinations, defaults, and edge cases
2. Format parser tests cover all 4 formats including malformed input
3. Runner tests cover spawn, shutdown, process exit, env, working_dir, and tag filtering
4. Handler tests verify custom source events integrate with existing NativeLog pipeline
5. All tests pass with `cargo test --workspace`
6. No test flakiness from async timing

### Notes

- For runner tests that spawn real processes, use simple commands: `echo`, `printf`, `cat`, `yes` (with immediate kill for infinite commands)
- Platform-specific tests should be gated with `#[cfg(unix)]` since they use Unix commands
- Use `tokio::time::timeout` in async tests to prevent hangs from broken shutdown paths
- Follow the test naming convention from `docs/CODE_STANDARDS.md`: `test_<scenario>_<expected_outcome>`
