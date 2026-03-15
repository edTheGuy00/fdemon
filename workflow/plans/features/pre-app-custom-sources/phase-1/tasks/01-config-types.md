## Task: Config Types — ReadyCheck Enum + CustomSourceConfig Extension

**Objective**: Add the `ReadyCheck` serde enum and extend the app-layer `CustomSourceConfig` with `start_before_app` and `ready_check` fields, plus validation logic and a helper on `NativeLogsSettings`.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `ReadyCheck` enum, extend `CustomSourceConfig`, extend `NativeLogsSettings`, add validation

### Details

#### 1. Add `ReadyCheck` Enum

Add a new tagged enum **in the same file** as `CustomSourceConfig` (`config/types.rs`), near line 563 (before `CustomSourceConfig`):

```rust
/// Readiness check configuration for pre-app custom sources.
///
/// Determines how fdemon verifies that a custom source process is ready
/// before launching the Flutter app. Only valid when `start_before_app = true`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReadyCheck {
    /// Poll an HTTP endpoint until it returns a 2xx status.
    Http {
        /// Full URL to GET (e.g., `http://localhost:8080/health`).
        url: String,
        /// Milliseconds between poll attempts.
        #[serde(default = "default_ready_check_interval_ms")]
        interval_ms: u64,
        /// Seconds before giving up and proceeding with Flutter launch.
        #[serde(default = "default_ready_check_timeout_s")]
        timeout_s: u64,
    },
    /// Poll a TCP host:port until a connection succeeds.
    Tcp {
        /// Hostname to connect to (e.g., `localhost`).
        host: String,
        /// Port number to connect to.
        port: u16,
        /// Milliseconds between poll attempts.
        #[serde(default = "default_ready_check_interval_ms")]
        interval_ms: u64,
        /// Seconds before giving up and proceeding with Flutter launch.
        #[serde(default = "default_ready_check_timeout_s")]
        timeout_s: u64,
    },
    /// Run an external command in a loop until it exits with code 0.
    Command {
        /// Executable to run (e.g., `grpcurl`, `pg_isready`).
        command: String,
        /// Arguments to pass to the command.
        #[serde(default)]
        args: Vec<String>,
        /// Milliseconds between poll attempts.
        #[serde(default = "default_ready_check_interval_ms")]
        interval_ms: u64,
        /// Seconds before giving up and proceeding with Flutter launch.
        #[serde(default = "default_ready_check_timeout_s")]
        timeout_s: u64,
    },
    /// Watch stdout for a regex pattern match.
    Stdout {
        /// Regex pattern to match against stdout lines.
        pattern: String,
        /// Seconds before giving up and proceeding with Flutter launch.
        #[serde(default = "default_ready_check_timeout_s")]
        timeout_s: u64,
    },
    /// Wait a fixed duration before proceeding.
    Delay {
        /// Seconds to wait.
        #[serde(default = "default_ready_check_delay_s")]
        seconds: u64,
    },
}

fn default_ready_check_interval_ms() -> u64 { 500 }
fn default_ready_check_timeout_s() -> u64 { 30 }
fn default_ready_check_delay_s() -> u64 { 5 }
```

The `#[serde(tag = "type", rename_all = "snake_case")]` enables the TOML inline table syntax from the plan:
```toml
ready_check = { type = "http", url = "http://localhost:8080/health" }
```

#### 2. Extend `CustomSourceConfig`

Add two fields to the existing struct at `config/types.rs:563`:

```rust
pub struct CustomSourceConfig {
    // ... existing fields ...

    /// Start this source before the Flutter app launches.
    /// When true, the source is spawned during the pre-app phase and its
    /// readiness check (if any) must pass before Flutter launches.
    #[serde(default)]
    pub start_before_app: bool,

    /// Optional readiness check. Only valid when `start_before_app = true`.
    /// If set, Flutter launch is gated until the check passes or times out.
    #[serde(default)]
    pub ready_check: Option<ReadyCheck>,
}
```

#### 3. Add Validation to `ReadyCheck`

Add a `validate()` method on `ReadyCheck`:

```rust
impl ReadyCheck {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            ReadyCheck::Http { url, .. } => {
                // Must parse as a URL with scheme + host
                let parsed = url::Url::parse(url)
                    .map_err(|e| format!("invalid ready_check url '{}': {}", url, e))?;
                if parsed.host().is_none() {
                    return Err(format!("ready_check url '{}' has no host", url));
                }
                Ok(())
            }
            ReadyCheck::Tcp { port, .. } => {
                if *port == 0 {
                    return Err("ready_check tcp port must not be 0".to_string());
                }
                Ok(())
            }
            ReadyCheck::Command { command, .. } => {
                if command.trim().is_empty() {
                    return Err("ready_check command must not be empty".to_string());
                }
                Ok(())
            }
            ReadyCheck::Stdout { pattern, .. } => {
                regex::Regex::new(pattern)
                    .map_err(|e| format!("ready_check stdout pattern '{}' is invalid regex: {}", pattern, e))?;
                Ok(())
            }
            ReadyCheck::Delay { seconds } => {
                if *seconds == 0 {
                    return Err("ready_check delay seconds must be > 0".to_string());
                }
                Ok(())
            }
        }
    }
}
```

**Note on dependencies**: `url` crate may need to be added to `fdemon-app/Cargo.toml`. Check if it's already a transitive dependency. If adding it is undesirable, parse manually with string ops (check for `://` + extract host:port). The `regex` crate is already used transitively; confirm it's in `fdemon-app` deps — if not, add it.

#### 4. Extend `CustomSourceConfig::validate()`

Add to the existing `validate()` method (currently at lines 607-637):

```rust
// Existing checks...

// ready_check requires start_before_app
if self.ready_check.is_some() && !self.start_before_app {
    return Err(format!(
        "custom_source '{}': ready_check requires start_before_app = true",
        self.name
    ));
}

// Validate ready_check if present
if let Some(ref check) = self.ready_check {
    check.validate().map_err(|e| format!(
        "custom_source '{}': {}", self.name, e
    ))?;
}

Ok(())
```

#### 5. Add `has_pre_app_sources()` Helper

Add to `NativeLogsSettings` impl block:

```rust
/// Returns `true` if any custom source has `start_before_app = true`.
pub fn has_pre_app_sources(&self) -> bool {
    self.custom_sources.iter().any(|s| s.start_before_app)
}

/// Returns an iterator over custom sources with `start_before_app = true`.
pub fn pre_app_sources(&self) -> impl Iterator<Item = &CustomSourceConfig> {
    self.custom_sources.iter().filter(|s| s.start_before_app)
}

/// Returns an iterator over custom sources with `start_before_app = false` (post-app).
pub fn post_app_sources(&self) -> impl Iterator<Item = &CustomSourceConfig> {
    self.custom_sources.iter().filter(|s| !s.start_before_app)
}
```

### Acceptance Criteria

1. `ReadyCheck` enum deserializes correctly from TOML inline table syntax with `type` tag
2. All five variants parse with correct defaults (`interval_ms=500`, `timeout_s=30`, `seconds=5`)
3. `start_before_app` defaults to `false` (backward compatible — existing configs unaffected)
4. `ready_check` defaults to `None` (backward compatible)
5. Validation rejects `ready_check` when `start_before_app = false`
6. Validation passes for `start_before_app = true` without `ready_check` (fire-and-forget)
7. HTTP url validation rejects malformed URLs
8. Stdout pattern validation rejects invalid regex
9. TCP port validation rejects port 0
10. Command validation rejects empty command
11. Delay validation rejects 0 seconds
12. `has_pre_app_sources()` returns false when no sources have `start_before_app = true`
13. `has_pre_app_sources()` returns true when at least one source has `start_before_app = true`
14. All existing tests in `config/` pass without modification

### Testing

Test deserialization of each `ReadyCheck` variant from TOML:

```rust
#[test]
fn test_ready_check_http_deserialize() {
    let toml = r#"
        name = "server"
        command = "cargo"
        args = ["run"]
        start_before_app = true
        ready_check = { type = "http", url = "http://localhost:8080/health" }
    "#;
    let config: CustomSourceConfig = toml::from_str(toml).unwrap();
    assert!(config.start_before_app);
    assert!(matches!(config.ready_check, Some(ReadyCheck::Http { .. })));
}

#[test]
fn test_ready_check_defaults() {
    let toml = r#"
        name = "server"
        command = "cargo"
        args = ["run"]
        start_before_app = true
        ready_check = { type = "http", url = "http://localhost:8080/health" }
    "#;
    let config: CustomSourceConfig = toml::from_str(toml).unwrap();
    if let Some(ReadyCheck::Http { interval_ms, timeout_s, .. }) = config.ready_check {
        assert_eq!(interval_ms, 500);
        assert_eq!(timeout_s, 30);
    }
}

#[test]
fn test_backward_compat_no_new_fields() {
    let toml = r#"
        name = "watcher"
        command = "tail"
        args = ["-f", "/tmp/app.log"]
    "#;
    let config: CustomSourceConfig = toml::from_str(toml).unwrap();
    assert!(!config.start_before_app);
    assert!(config.ready_check.is_none());
}

#[test]
fn test_validate_ready_check_requires_start_before_app() {
    let toml = r#"
        name = "server"
        command = "cargo"
        ready_check = { type = "http", url = "http://localhost:8080/health" }
    "#;
    let config: CustomSourceConfig = toml::from_str(toml).unwrap();
    assert!(config.validate().is_err());
}

#[test]
fn test_validate_start_before_app_without_ready_check_ok() {
    let toml = r#"
        name = "worker"
        command = "python"
        args = ["worker.py"]
        start_before_app = true
    "#;
    let config: CustomSourceConfig = toml::from_str(toml).unwrap();
    assert!(config.validate().is_ok());
}

#[test]
fn test_has_pre_app_sources_false_when_none() {
    let settings = NativeLogsSettings {
        custom_sources: vec![/* sources with start_before_app = false */],
        ..Default::default()
    };
    assert!(!settings.has_pre_app_sources());
}

#[test]
fn test_has_pre_app_sources_true_when_present() {
    // Build a settings with one pre-app source
    // ...
    assert!(settings.has_pre_app_sources());
}
```

Also test each `ReadyCheck::validate()` error case (invalid URL, invalid regex, empty command, port 0, delay 0).

### Notes

- Check whether `url` and `regex` crates are already in `fdemon-app/Cargo.toml`. If `url` is not present, consider parsing manually to avoid adding a dependency — the plan's Decision 6 uses raw TCP for HTTP checks, so we only need basic URL decomposition (scheme, host, port, path). A simple `url` parse at validation time is fine since it's a dev-time check, not hot path.
- The `ReadyCheck` type lives in `fdemon-app` (not `fdemon-core`) because it's a config/application concern, not a domain type. The PLAN.md originally suggested `fdemon-core/types.rs` but the config parsing and validation tightly couples it to the app layer.
- `#[serde(tag = "type")]` produces `{ "type": "http", ... }` in JSON and `type = "http"` in TOML inline tables — this matches the PLAN.md config examples exactly.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/Cargo.toml` | Added `url.workspace = true` dependency |
| `crates/fdemon-app/src/config/types.rs` | Added `ReadyCheck` enum with 5 variants, `ReadyCheck::validate()`, extended `CustomSourceConfig` with `start_before_app` and `ready_check` fields, added validation to `CustomSourceConfig::validate()`, added `has_pre_app_sources()` / `pre_app_sources()` / `post_app_sources()` helpers to `NativeLogsSettings`, updated all existing struct literal constructions to include new fields, added ~100 new tests |
| `crates/fdemon-app/src/handler/tests.rs` | Updated one `CustomSourceConfig` struct literal to include new fields |

### Notable Decisions/Tradeoffs

1. **`url` crate added to `fdemon-app`**: The `url` crate was already in the workspace (`Cargo.toml` at root specifies `url = "2"`), so it was straightforward to add as a workspace dependency. This avoids manual URL parsing and gives correct host extraction.
2. **Existing tests updated in-place**: All pre-existing struct literal constructions of `CustomSourceConfig` required adding `start_before_app: false, ready_check: None`. There are no default implementations to allow partial construction, matching the explicit design intent.
3. **`#[serde(tag = "type")]` placement**: The `ReadyCheck` enum uses `tag = "type"` with `rename_all = "snake_case"`, matching the TOML inline table syntax `{ type = "http", ... }` exactly as specified in the plan.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1611 tests, 0 failed, 4 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **`file:///` host check**: The `url` crate correctly returns `None` for `host()` on `file:///path/to/file` URLs (no host component), so the HTTP validation correctly rejects it.
2. **Daemon `CustomSourceConfig` is separate**: `fdemon-daemon` has its own `CustomSourceConfig` type in `native_logs/custom.rs` with different fields (`exclude_tags`, `include_tags`, `ready_pattern`). The new fields live only in the app-layer config type and conversion between the two types (done in `fdemon-app/src/actions/native_logs.rs`) will need updating in a later task when the pre-app launch logic is implemented.
