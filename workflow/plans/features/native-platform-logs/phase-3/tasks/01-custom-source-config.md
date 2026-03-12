## Task: Custom Source Configuration Types

**Objective**: Add `CustomSourceConfig` struct and TOML parsing for `[[native_logs.custom_sources]]` array, allowing users to define arbitrary log source processes in their `.fdemon/config.toml`.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `CustomSourceConfig` struct to `NativeLogsSettings`, add `OutputFormat` enum for format selection
- `crates/fdemon-app/src/config/tests.rs` (or inline tests): Parsing and validation tests

### Details

Add to the existing `NativeLogsSettings` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeLogsSettings {
    // ... existing fields ...
    pub enabled: bool,
    pub exclude_tags: Vec<String>,
    pub include_tags: Vec<String>,
    pub min_level: String,
    pub tags: HashMap<String, TagConfig>,

    // NEW: custom log source processes
    #[serde(default)]
    pub custom_sources: Vec<CustomSourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSourceConfig {
    /// Display name — becomes the tag in the log view and tag filter
    pub name: String,

    /// Path to the command to execute
    pub command: String,

    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// Output format parser to use
    #[serde(default = "OutputFormat::default")]
    pub format: OutputFormat,

    /// Working directory for the command (optional)
    pub working_dir: Option<String>,

    /// Environment variables to set (optional)
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// Each line becomes a message with level=Info, tag=source name
    #[default]
    Raw,
    /// JSON objects with level/tag/message fields
    Json,
    /// Android logcat threadtime format: `MM-DD HH:MM:SS.mmm PID TID PRIO TAG : message`
    LogcatThreadtime,
    /// macOS/iOS syslog/compact format
    Syslog,
}
```

TOML example that must parse correctly:

```toml
[[native_logs.custom_sources]]
name = "go-backend"
command = "adb"
args = ["logcat", "GoLog:D", "*:S", "-v", "threadtime"]
format = "logcat-threadtime"

[[native_logs.custom_sources]]
name = "my-server"
command = "/usr/local/bin/my-log-tool"
args = ["--follow", "--json"]
format = "json"
env = { LOG_LEVEL = "debug" }

[[native_logs.custom_sources]]
name = "sidecar"
command = "tail"
args = ["-f", "/tmp/sidecar.log"]
format = "raw"
working_dir = "/tmp"
```

### Validation

Add a `validate()` method to `CustomSourceConfig`:
- `name` must be non-empty and non-whitespace
- `command` must be non-empty
- Log a warning (don't error) if `name` matches a known platform tag like `"flutter"` — it will work but may confuse users

### Acceptance Criteria

1. `NativeLogsSettings` deserializes `custom_sources` from TOML (empty by default when not specified)
2. `OutputFormat` deserializes all 4 variants from kebab-case strings
3. `CustomSourceConfig` round-trips through serialize/deserialize
4. Validation rejects empty `name` or `command`
5. `Default` for `NativeLogsSettings` includes `custom_sources: vec![]`
6. Existing config parsing is unaffected (backwards compatible — empty `custom_sources` when field absent)

### Testing

```rust
#[test]
fn test_custom_source_config_deserialize() {
    let toml = r#"
    [[native_logs.custom_sources]]
    name = "go-backend"
    command = "adb"
    args = ["logcat", "GoLog:D", "*:S", "-v", "threadtime"]
    format = "logcat-threadtime"
    "#;
    // Parse and verify all fields
}

#[test]
fn test_custom_source_default_format_is_raw() { ... }

#[test]
fn test_custom_source_empty_name_fails_validation() { ... }

#[test]
fn test_output_format_kebab_case_serde() {
    // "logcat-threadtime" → LogcatThreadtime
    // "raw" → Raw
    // "json" → Json
    // "syslog" → Syslog
}

#[test]
fn test_existing_config_without_custom_sources_still_works() { ... }
```

### Notes

- `OutputFormat` will also be used by the format parsers in task 02 — keep it in `fdemon-app/src/config/types.rs` for now since it's part of the config struct, but consider whether `fdemon-daemon` should own the enum. If `fdemon-daemon` needs it, it may need to be in `fdemon-core` instead for proper layer boundaries. Check layer dependencies.
- The `env` field uses `HashMap<String, String>` for simplicity. TOML inline tables work: `env = { KEY = "value" }`.
- `working_dir` is optional — defaults to the Flutter project directory if not specified.
