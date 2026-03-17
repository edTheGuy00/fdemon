//! Configuration types for Flutter Demon
//!
//! Defines:
//! - `LaunchConfig` - A single launch configuration
//! - `Settings` - Global application settings
//! - Related sub-types and enums

use fdemon_core::types::OutputFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single launch configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LaunchConfig {
    /// Display name for this configuration
    pub name: String,

    /// Target device ID, platform prefix, or "auto" for first available
    #[serde(default = "default_device")]
    pub device: String,

    /// Flutter mode: debug, profile, or release
    #[serde(default = "default_mode")]
    pub mode: FlutterMode,

    /// Build flavor (e.g., "development", "production")
    #[serde(default)]
    pub flavor: Option<String>,

    /// Entry point (defaults to lib/main.dart)
    #[serde(default)]
    pub entry_point: Option<PathBuf>,

    /// Dart defines (--dart-define)
    #[serde(default)]
    pub dart_defines: HashMap<String, String>,

    /// Additional arguments to pass to flutter run
    #[serde(default)]
    pub extra_args: Vec<String>,

    /// Whether to start this config automatically
    #[serde(default)]
    pub auto_start: bool,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            device: default_device(),
            mode: default_mode(),
            flavor: None,
            entry_point: None,
            dart_defines: HashMap::new(),
            extra_args: Vec::new(),
            auto_start: false,
        }
    }
}

fn default_device() -> String {
    "auto".to_string()
}

fn default_mode() -> FlutterMode {
    FlutterMode::Debug
}

/// Flutter build mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FlutterMode {
    #[default]
    Debug,
    Profile,
    Release,
}

impl FlutterMode {
    pub fn as_arg(&self) -> &'static str {
        match self {
            FlutterMode::Debug => "--debug",
            FlutterMode::Profile => "--profile",
            FlutterMode::Release => "--release",
        }
    }
}

impl std::fmt::Display for FlutterMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlutterMode::Debug => write!(f, "debug"),
            FlutterMode::Profile => write!(f, "profile"),
            FlutterMode::Release => write!(f, "release"),
        }
    }
}

/// Launch configurations file (.fdemon/launch.toml)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LaunchFile {
    /// List of launch configurations
    #[serde(default)]
    pub configurations: Vec<LaunchConfig>,
}

/// Application settings (.fdemon/config.toml)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub behavior: BehaviorSettings,

    #[serde(default)]
    pub watcher: WatcherSettings,

    #[serde(default)]
    pub ui: UiSettings,

    #[serde(default)]
    pub devtools: DevToolsSettings,

    #[serde(default)]
    pub editor: EditorSettings,

    #[serde(default)]
    pub dap: DapSettings,

    #[serde(default)]
    pub native_logs: NativeLogsSettings,

    #[serde(default)]
    pub flutter: FlutterSettings,
}

// ─────────────────────────────────────────────────────────────────────────────
// Flutter SDK Settings
// ─────────────────────────────────────────────────────────────────────────────

/// Settings for Flutter SDK configuration.
///
/// Corresponds to the `[flutter]` section in config.toml.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FlutterSettings {
    /// Explicit SDK path override. When set, this takes highest priority
    /// in the detection chain, bypassing all version manager detection.
    ///
    /// Example: `/Users/me/flutter` or `C:\flutter`
    #[serde(default)]
    pub sdk_path: Option<PathBuf>,
}

/// Behavior settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BehaviorSettings {
    /// If false, show device selector on startup
    #[serde(default)]
    pub auto_start: bool,

    /// Ask before quitting with running apps
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
}

impl Default for BehaviorSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            confirm_quit: true,
        }
    }
}

/// File watcher settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WatcherSettings {
    /// Paths to watch (relative to project root)
    #[serde(default = "default_watch_paths")]
    pub paths: Vec<String>,

    /// Debounce duration in milliseconds
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,

    /// Auto-reload on file change
    #[serde(default = "default_true")]
    pub auto_reload: bool,

    /// File extensions to watch
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,
}

impl Default for WatcherSettings {
    fn default() -> Self {
        Self {
            paths: default_watch_paths(),
            debounce_ms: default_debounce_ms(),
            auto_reload: true,
            extensions: default_extensions(),
        }
    }
}

fn default_watch_paths() -> Vec<String> {
    vec!["lib".to_string()]
}

fn default_debounce_ms() -> u64 {
    500
}

fn default_extensions() -> Vec<String> {
    vec!["dart".to_string()]
}

fn default_true() -> bool {
    true
}

/// Icon rendering mode for the TUI.
///
/// Controls whether icons use Nerd Font glyphs (default, requires a Nerd Font)
/// or safe Unicode characters (works in all terminals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IconMode {
    /// Safe Unicode characters that work in all terminals
    Unicode,
    /// Nerd Font glyphs — requires a Nerd Font installed in the terminal (default)
    #[default]
    NerdFonts,
}

impl std::fmt::Display for IconMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IconMode::Unicode => write!(f, "unicode"),
            IconMode::NerdFonts => write!(f, "nerd_fonts"),
        }
    }
}

/// UI settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiSettings {
    /// Maximum number of log entries to keep
    #[serde(default = "default_log_buffer_size")]
    pub log_buffer_size: usize,

    /// Show timestamps in logs
    #[serde(default = "default_true")]
    pub show_timestamps: bool,

    /// Collapse similar consecutive logs
    #[serde(default)]
    pub compact_logs: bool,

    /// Theme name
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Whether stack traces start collapsed (default: true)
    #[serde(default = "default_true")]
    pub stack_trace_collapsed: bool,

    /// Maximum frames to show when collapsed (default: 3)
    #[serde(default = "default_stack_trace_max_frames")]
    pub stack_trace_max_frames: usize,

    /// Icon mode: "unicode" (default) or "nerd_fonts"
    #[serde(default)]
    pub icons: IconMode,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            log_buffer_size: default_log_buffer_size(),
            show_timestamps: true,
            compact_logs: false,
            theme: default_theme(),
            stack_trace_collapsed: true,
            stack_trace_max_frames: default_stack_trace_max_frames(),
            icons: IconMode::default(),
        }
    }
}

fn default_stack_trace_max_frames() -> usize {
    3
}

fn default_log_buffer_size() -> usize {
    10_000
}

fn default_theme() -> String {
    "default".to_string()
}

/// DevTools settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevToolsSettings {
    /// Auto-open DevTools when app starts
    #[serde(default)]
    pub auto_open: bool,

    /// Browser to use (empty = system default)
    #[serde(default)]
    pub browser: String,

    /// Default panel when entering DevTools mode ("inspector", "performance", "network")
    #[serde(default = "default_devtools_panel")]
    pub default_panel: String,

    /// Performance data refresh interval in milliseconds (minimum 500ms)
    #[serde(default = "default_performance_refresh_ms")]
    pub performance_refresh_ms: u64,

    /// Memory history size (number of snapshots to retain)
    #[serde(default = "default_memory_history_size")]
    pub memory_history_size: usize,

    /// Widget tree max fetch depth (0 = unlimited)
    #[serde(default)]
    pub tree_max_depth: u32,

    /// Widget tree fetch timeout in seconds (with readiness polling + retries).
    /// Minimum effective value is 5 seconds.
    #[serde(default = "default_inspector_fetch_timeout_secs")]
    pub inspector_fetch_timeout_secs: u64,

    /// Auto-enable repaint rainbow on VM connect
    #[serde(default)]
    pub auto_repaint_rainbow: bool,

    /// Auto-enable performance overlay on VM connect
    #[serde(default)]
    pub auto_performance_overlay: bool,

    /// Allocation profile polling interval in milliseconds.
    ///
    /// Controls how often `getAllocationProfile` is called to capture per-class
    /// heap statistics. This RPC is expensive (walks the entire Dart heap), so
    /// a higher default (5000ms) is used compared to the memory polling interval.
    /// Clamped to a minimum of 1000ms at the polling task level.
    #[serde(default = "default_allocation_profile_interval_ms")]
    pub allocation_profile_interval_ms: u64,

    /// Maximum number of network entries to keep per session (FIFO eviction).
    /// Default: 500.
    #[serde(default = "default_max_network_entries")]
    pub max_network_entries: usize,

    /// Whether to auto-start network recording when entering the Network tab.
    /// Default: true.
    #[serde(default = "default_network_auto_record")]
    pub network_auto_record: bool,

    /// Network profile polling interval in milliseconds.
    /// Controls how often `getHttpProfile` is called when recording.
    /// Clamped to minimum 500ms. Default: 1000.
    #[serde(default = "default_network_poll_interval_ms")]
    pub network_poll_interval_ms: u64,

    /// Logging sub-settings
    #[serde(default)]
    pub logging: DevToolsLoggingSettings,
}

impl Default for DevToolsSettings {
    fn default() -> Self {
        Self {
            auto_open: false,
            browser: String::new(),
            default_panel: default_devtools_panel(),
            performance_refresh_ms: default_performance_refresh_ms(),
            memory_history_size: default_memory_history_size(),
            tree_max_depth: 0,
            inspector_fetch_timeout_secs: default_inspector_fetch_timeout_secs(),
            auto_repaint_rainbow: false,
            auto_performance_overlay: false,
            allocation_profile_interval_ms: default_allocation_profile_interval_ms(),
            max_network_entries: default_max_network_entries(),
            network_auto_record: default_network_auto_record(),
            network_poll_interval_ms: default_network_poll_interval_ms(),
            logging: DevToolsLoggingSettings::default(),
        }
    }
}

fn default_devtools_panel() -> String {
    "inspector".to_string()
}

fn default_performance_refresh_ms() -> u64 {
    2000
}

fn default_memory_history_size() -> usize {
    60
}

fn default_allocation_profile_interval_ms() -> u64 {
    5000
}

fn default_max_network_entries() -> usize {
    500
}

fn default_network_auto_record() -> bool {
    true
}

fn default_network_poll_interval_ms() -> u64 {
    1000
}

fn default_inspector_fetch_timeout_secs() -> u64 {
    60
}

/// Logging sub-settings for the hybrid VM Service + daemon log pipeline.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevToolsLoggingSettings {
    /// Enable hybrid logging (VM Service + daemon)
    #[serde(default = "default_true")]
    pub hybrid_enabled: bool,

    /// Prefer VM Service log level when available
    #[serde(default = "default_true")]
    pub prefer_vm_level: bool,

    /// Show log source indicator ([VM] vs [daemon])
    #[serde(default)]
    pub show_source_indicator: bool,

    /// Dedupe threshold: logs within N ms with same message are duplicates
    #[serde(default = "default_dedupe_threshold_ms")]
    pub dedupe_threshold_ms: u64,
}

impl Default for DevToolsLoggingSettings {
    fn default() -> Self {
        Self {
            hybrid_enabled: true,
            prefer_vm_level: true,
            show_source_indicator: false,
            dedupe_threshold_ms: default_dedupe_threshold_ms(),
        }
    }
}

fn default_dedupe_threshold_ms() -> u64 {
    100
}

// ─────────────────────────────────────────────────────────────────────────────
// Editor Settings
// ─────────────────────────────────────────────────────────────────────────────

/// Editor integration settings for opening files from stack traces.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EditorSettings {
    /// Editor command or name (e.g., "code", "zed", "nvim").
    /// If empty, attempts auto-detection.
    #[serde(default)]
    pub command: String,

    /// Pattern for opening file at line/column.
    /// Variables: $EDITOR, $FILE, $LINE, $COLUMN
    /// Example: "$EDITOR --goto $FILE:$LINE:$COLUMN"
    #[serde(default = "default_open_pattern")]
    pub open_pattern: String,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            command: String::new(), // Auto-detect
            open_pattern: default_open_pattern(),
        }
    }
}

fn default_open_pattern() -> String {
    "$EDITOR $FILE:$LINE".to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// DAP Server Settings
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the embedded DAP (Debug Adapter Protocol) server.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DapSettings {
    /// Always enable DAP server on startup (overrides auto-detection).
    /// Can also use --dap CLI flag.
    #[serde(default)]
    pub enabled: bool,

    /// Auto-start DAP server when running inside a detected IDE terminal
    /// (VS Code, Neovim, Helix, Zed, Emacs). No effect if enabled = true.
    #[serde(default = "default_auto_start_in_ide")]
    pub auto_start_in_ide: bool,

    /// TCP port for DAP connections. 0 = auto-assign an available port.
    /// Use a fixed port for stable IDE configs across restarts.
    #[serde(default)]
    pub port: u16,

    /// Bind address for the DAP server.
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// Suppress auto-reload while debugger is paused at a breakpoint.
    #[serde(default = "default_suppress_reload")]
    pub suppress_reload_on_pause: bool,

    /// Automatically generate IDE DAP config when server starts.
    /// Default: true — generates launch.json/languages.toml/etc. on server bind.
    #[serde(default = "default_auto_configure_ide")]
    pub auto_configure_ide: bool,
}

fn default_auto_start_in_ide() -> bool {
    true
}

fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}

fn default_suppress_reload() -> bool {
    true
}

fn default_auto_configure_ide() -> bool {
    true
}

impl Default for DapSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_start_in_ide: default_auto_start_in_ide(),
            port: 0,
            bind_address: default_bind_address(),
            suppress_reload_on_pause: default_suppress_reload(),
            auto_configure_ide: default_auto_configure_ide(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Native Log Settings
// ─────────────────────────────────────────────────────────────────────────────

/// Per-tag configuration override for native log capture.
///
/// Allows individual tags to have their own minimum log level, overriding the
/// global `min_level` in `NativeLogsSettings`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TagConfig {
    /// Minimum log level for this tag (overrides the global `min_level`).
    /// Options: "verbose", "debug", "info", "warning", "error"
    pub min_level: Option<String>,
}

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

fn default_ready_check_interval_ms() -> u64 {
    500
}
fn default_ready_check_timeout_s() -> u64 {
    30
}
fn default_ready_check_delay_s() -> u64 {
    5
}

impl ReadyCheck {
    /// Validate this readiness check configuration.
    ///
    /// # Errors
    ///
    /// - `Http`: URL is malformed or has no host
    /// - `Tcp`: port is 0
    /// - `Command`: command string is empty or whitespace
    /// - `Stdout`: pattern is not valid regex
    /// - `Delay`: seconds is 0
    pub fn validate(&self) -> Result<(), String> {
        match self {
            ReadyCheck::Http { url, .. } => {
                crate::actions::ready_check::parse_http_url(url)
                    .map_err(|e| format!("invalid ready_check url '{}': {}", url, e))?;
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
                regex::Regex::new(pattern).map_err(|e| {
                    format!(
                        "ready_check stdout pattern '{}' is invalid regex: {}",
                        pattern, e
                    )
                })?;
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

impl std::fmt::Display for ReadyCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadyCheck::Http { url, .. } => write!(f, "http: {}", url),
            ReadyCheck::Tcp { host, port, .. } => write!(f, "tcp: {}:{}", host, port),
            ReadyCheck::Command { command, .. } => write!(f, "command: {}", command),
            ReadyCheck::Stdout { pattern, .. } => write!(f, "stdout: /{}/", pattern),
            ReadyCheck::Delay { seconds } => write!(f, "delay: {}s", seconds),
        }
    }
}

/// Configuration for a custom log source process.
///
/// Defines an external command whose output is captured and parsed as native
/// log entries. The `format` field selects the parser used to convert raw
/// output lines into structured log events.
///
/// In `.fdemon/config.toml`, specify multiple sources using the TOML array-of-tables
/// syntax: `[[native_logs.custom_sources]]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomSourceConfig {
    /// Display name — becomes the tag in the log view and tag filter overlay.
    pub name: String,

    /// Path to the command to execute (e.g., `"adb"`, `"/usr/local/bin/my-tool"`).
    pub command: String,

    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Output format parser to use when interpreting stdout/stderr lines.
    #[serde(default)]
    pub format: OutputFormat,

    /// Working directory for the command.
    ///
    /// If `None`, defaults to the Flutter project root directory.
    pub working_dir: Option<String>,

    /// Environment variables to set for the spawned process.
    ///
    /// In TOML: `env = { LOG_LEVEL = "debug" }`.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Start this source before the Flutter app launches.
    ///
    /// When `true`, the source is spawned during the pre-app phase and its
    /// readiness check (if any) must pass before Flutter launches.
    #[serde(default)]
    pub start_before_app: bool,

    /// Whether this source is shared across all sessions (spawned once).
    ///
    /// When `true`, the source is spawned on first session launch and persists
    /// until fdemon quits. Logs are broadcast to all active sessions.
    /// When `false` (default), the source is per-session.
    #[serde(default)]
    pub shared: bool,

    /// Optional readiness check. Only valid when `start_before_app = true`.
    ///
    /// If set, Flutter launch is gated until the check passes or times out.
    #[serde(default)]
    pub ready_check: Option<ReadyCheck>,
}

/// Well-known platform tag names that would be confusing to reuse as custom source names.
///
/// Used by [`CustomSourceConfig::validate`] to emit advisory warnings.
const KNOWN_PLATFORM_TAGS: &[&str] = &["flutter", "dart", "flutterengine", "flutter engine"];

impl CustomSourceConfig {
    /// Validate this configuration, returning an error string if invalid.
    ///
    /// Returns `Ok(())` when the config is valid. Logs a warning (does not fail)
    /// when `name` shadows a known platform tag.
    ///
    /// # Errors
    ///
    /// - `name` is empty or contains only whitespace
    /// - `command` is empty
    /// - `format = "syslog"` on a non-macOS host (syslog is macOS-only)
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("custom_source name must not be empty".to_string());
        }
        if self.command.is_empty() {
            return Err(format!(
                "custom_source '{}': command must not be empty",
                self.name
            ));
        }
        // Syslog format is only available on macOS; reject it at config-parse
        // time on other platforms so the user gets a clear error instead of
        // silent empty output.
        #[cfg(not(target_os = "macos"))]
        if self.format == OutputFormat::Syslog {
            return Err(format!(
                "custom_source '{}': syslog format is only supported on macOS",
                self.name
            ));
        }
        // Advisory warning — not an error
        let lower = self.name.to_lowercase();
        if KNOWN_PLATFORM_TAGS.contains(&lower.as_str()) {
            tracing::warn!(
                name = %self.name,
                "custom_source name matches a known platform tag; \
                 this will work but may cause confusion in the tag filter"
            );
        }
        // ready_check requires start_before_app = true
        if self.ready_check.is_some() && !self.start_before_app {
            return Err(format!(
                "custom_source '{}': ready_check requires start_before_app = true",
                self.name
            ));
        }
        // Validate ready_check if present
        if let Some(ref check) = self.ready_check {
            check
                .validate()
                .map_err(|e| format!("custom_source '{}': {}", self.name, e))?;
        }
        Ok(())
    }
}

/// Configuration for native platform log capture.
///
/// Controls whether fdemon runs parallel log capture processes (e.g., `adb logcat`,
/// `log stream`) to surface native plugin logs alongside Flutter logs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NativeLogsSettings {
    /// Master toggle for native log capture. When false, no native log processes are spawned.
    #[serde(default = "default_native_logs_enabled")]
    pub enabled: bool,

    /// Tags to exclude from native log output. Default: `["flutter"]` to avoid
    /// duplicating Flutter's own log output which is already captured via `--machine`.
    #[serde(default = "default_native_logs_exclude_tags")]
    pub exclude_tags: Vec<String>,

    /// If set, ONLY show logs from these tags (overrides `exclude_tags`).
    /// Empty means "show all tags (minus exclude_tags)".
    #[serde(default)]
    pub include_tags: Vec<String>,

    /// Minimum native log priority level. Logs below this level are discarded.
    /// Options: "verbose", "debug", "info", "warning", "error"
    #[serde(default = "default_native_logs_min_level")]
    pub min_level: String,

    /// Per-tag configuration overrides.
    ///
    /// Key: tag name (e.g., "GoLog", "OkHttp", "com.example.myplugin").
    /// In `.fdemon/config.toml`, use `[native_logs.tags.GoLog]` or
    /// `[native_logs.tags."com.example.myplugin"]` for dotted names.
    #[serde(default)]
    pub tags: HashMap<String, TagConfig>,

    /// Custom log source processes to capture alongside native platform logs.
    ///
    /// Each entry defines an external command whose stdout/stderr output is
    /// captured and parsed according to the specified `format`.
    ///
    /// In `.fdemon/config.toml`, use `[[native_logs.custom_sources]]` array syntax.
    #[serde(default)]
    pub custom_sources: Vec<CustomSourceConfig>,
}

fn default_native_logs_enabled() -> bool {
    true
}

fn default_native_logs_exclude_tags() -> Vec<String> {
    vec!["flutter".to_string()]
}

fn default_native_logs_min_level() -> String {
    "info".to_string()
}

impl Default for NativeLogsSettings {
    fn default() -> Self {
        Self {
            enabled: default_native_logs_enabled(),
            exclude_tags: default_native_logs_exclude_tags(),
            include_tags: Vec::new(),
            min_level: default_native_logs_min_level(),
            tags: HashMap::new(),
            custom_sources: Vec::new(),
        }
    }
}

impl NativeLogsSettings {
    /// Check if a given tag should be included in native log output.
    ///
    /// When `include_tags` is non-empty, operates in whitelist mode: only tags
    /// present in `include_tags` are included, and `exclude_tags` is ignored.
    ///
    /// When `include_tags` is empty, operates in blacklist mode: all tags are
    /// included except those present in `exclude_tags`.
    ///
    /// Tag matching is case-insensitive.
    pub fn should_include_tag(&self, tag: &str) -> bool {
        fdemon_daemon::native_logs::should_include_tag(&self.include_tags, &self.exclude_tags, tag)
    }

    /// Get the effective minimum log level for a specific tag.
    ///
    /// Returns the per-tag override from `tags` if configured and has a `min_level` set,
    /// otherwise falls back to the global `min_level`.
    ///
    /// Lookup is case-insensitive: `"GoLog"` and `"golog"` resolve to the same
    /// per-tag config entry regardless of how the key was written in the TOML.
    /// This matches the behaviour of the daemon-layer `should_include_tag` and
    /// the session-layer `NativeTagState`.
    pub fn effective_min_level(&self, tag: &str) -> &str {
        self.tags
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(tag))
            .and_then(|(_, tc)| tc.min_level.as_deref())
            .unwrap_or(&self.min_level)
    }

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

    /// Returns `true` if any custom source has `shared = true`.
    #[cfg(test)]
    pub(crate) fn has_shared_sources(&self) -> bool {
        self.custom_sources.iter().any(|s| s.shared)
    }

    /// Returns an iterator over shared custom sources.
    #[cfg(test)]
    pub(crate) fn shared_sources(&self) -> impl Iterator<Item = &CustomSourceConfig> {
        self.custom_sources.iter().filter(|s| s.shared)
    }

    /// Returns `true` if any custom source has `start_before_app = true` AND `shared = true`.
    #[cfg(test)]
    pub(crate) fn has_shared_pre_app_sources(&self) -> bool {
        self.custom_sources
            .iter()
            .any(|s| s.start_before_app && s.shared)
    }

    /// Validate `NativeLogsSettings`, returning an error string if invalid.
    ///
    /// Currently checks that no two `custom_sources` share the same name
    /// (case-insensitive), because `CustomSourceStopped` removes handles by
    /// name and would orphan a process if duplicates exist.
    ///
    /// Also delegates to [`CustomSourceConfig::validate`] for each source.
    ///
    /// # Errors
    ///
    /// - Any `CustomSourceConfig` fails its own validation
    /// - Two custom sources share the same name (case-insensitive)
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for source in &self.custom_sources {
            source.validate()?;
            if !seen.insert(source.name.to_ascii_lowercase()) {
                return Err(format!("Duplicate custom source name: '{}'", source.name));
            }
        }
        Ok(())
    }
}

/// Detected parent IDE when running in an integrated terminal.
///
/// This is used to open files in the *current* IDE instance rather than
/// spawning a new window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentIde {
    VSCode,
    VSCodeInsiders,
    Cursor,
    Zed,
    IntelliJ,
    AndroidStudio,
    Neovim,
    Emacs,
    Helix,
}

impl ParentIde {
    /// URL scheme for OSC 8 hyperlinks (Ctrl+click support).
    pub fn url_scheme(&self) -> &'static str {
        match self {
            ParentIde::VSCode => "vscode",
            ParentIde::VSCodeInsiders => "vscode-insiders",
            ParentIde::Cursor => "cursor",
            ParentIde::Zed => "zed",
            ParentIde::IntelliJ | ParentIde::AndroidStudio => "idea",
            ParentIde::Neovim | ParentIde::Emacs | ParentIde::Helix => "file",
        }
    }

    /// Command-line flag to reuse existing window.
    pub fn reuse_flag(&self) -> Option<&'static str> {
        match self {
            ParentIde::VSCode | ParentIde::VSCodeInsiders | ParentIde::Cursor => {
                Some("--reuse-window")
            }
            _ => None,
        }
    }

    /// Display name for the IDE.
    pub fn display_name(&self) -> &'static str {
        match self {
            ParentIde::VSCode => "VS Code",
            ParentIde::VSCodeInsiders => "VS Code Insiders",
            ParentIde::Cursor => "Cursor",
            ParentIde::Zed => "Zed",
            ParentIde::IntelliJ => "IntelliJ IDEA",
            ParentIde::AndroidStudio => "Android Studio",
            ParentIde::Neovim => "Neovim",
            ParentIde::Emacs => "Emacs",
            ParentIde::Helix => "Helix",
        }
    }

    /// Returns true if this IDE supports auto-generated DAP configuration.
    ///
    /// IntelliJ and Android Studio use proprietary debugging protocols — no standard DAP path.
    pub fn supports_dap_config(&self) -> bool {
        !matches!(self, Self::IntelliJ | Self::AndroidStudio)
    }

    /// Returns the target config file path for DAP auto-configuration.
    ///
    /// Returns `None` for IDEs that don't support DAP config (IntelliJ, Android Studio).
    pub fn dap_config_path(&self, project_root: &Path) -> Option<PathBuf> {
        match self {
            Self::VSCode | Self::VSCodeInsiders | Self::Cursor | Self::Neovim => {
                Some(project_root.join(".vscode/launch.json"))
            }
            Self::Helix => Some(project_root.join(".helix/languages.toml")),
            Self::Zed => Some(project_root.join(".zed/debug.json")),
            Self::Emacs => Some(project_root.join(".fdemon/dap-emacs.el")),
            Self::IntelliJ | Self::AndroidStudio => None,
        }
    }
}

/// Source of a launch configuration (for tracking origin)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    /// From .fdemon/launch.toml
    FDemon,
    /// From .vscode/launch.json
    VSCode,
    /// From command-line arguments
    CommandLine,
    /// Default/fallback
    Default,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::FDemon => write!(f, ".fdemon"),
            ConfigSource::VSCode => write!(f, ".vscode"),
            ConfigSource::CommandLine => write!(f, "CLI"),
            ConfigSource::Default => write!(f, "default"),
        }
    }
}

/// A launch configuration with metadata about its source
#[derive(Debug, Clone)]
pub struct ResolvedLaunchConfig {
    pub config: LaunchConfig,
    pub source: ConfigSource,
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings UI Types
// ─────────────────────────────────────────────────────────────────────────────

/// Tab in the settings panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Project, // config.toml - shared settings
    UserPrefs,    // settings.local.toml - user-specific
    LaunchConfig, // launch.toml - shared launch configs
    VSCodeConfig, // launch.json - read-only display
}

impl SettingsTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::UserPrefs => "User",
            Self::LaunchConfig => "Launch",
            Self::VSCodeConfig => "VSCode",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Self::Project => 0,
            Self::UserPrefs => 1,
            Self::LaunchConfig => 2,
            Self::VSCodeConfig => 3,
        }
    }

    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Project),
            1 => Some(Self::UserPrefs),
            2 => Some(Self::LaunchConfig),
            3 => Some(Self::VSCodeConfig),
            _ => None,
        }
    }

    pub fn next(&self) -> Self {
        Self::from_index((self.index() + 1) % 4).unwrap()
    }

    pub fn prev(&self) -> Self {
        Self::from_index((self.index() + 3) % 4).unwrap()
    }

    /// Icon for tab (optional visual enhancement)
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Project => "⚙",       // Gear for project settings
            Self::UserPrefs => "👤",    // Person for user prefs
            Self::LaunchConfig => "▶",  // Play for launch
            Self::VSCodeConfig => "📁", // Folder for VSCode
        }
    }

    /// Whether this tab is read-only
    pub fn is_readonly(&self) -> bool {
        matches!(self, Self::VSCodeConfig)
    }
}

/// A setting value that can be edited
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    Bool(bool),
    Number(i64),
    Float(f64),
    String(String),
    Enum { value: String, options: Vec<String> },
    List(Vec<String>),
}

impl SettingValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Enum { .. } => "enum",
            Self::List(_) => "list",
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Self::Number(n) => n.to_string(),
            Self::Float(f) => format!("{:.2}", f),
            Self::String(s) => s.clone(),
            Self::Enum { value, .. } => value.clone(),
            Self::List(items) => items.join(", "),
        }
    }
}

/// A single setting item for display/editing
#[derive(Debug, Clone)]
pub struct SettingItem {
    /// Unique identifier (e.g., "behavior.auto_start")
    pub id: String,
    /// Display label (e.g., "Auto Start")
    pub label: String,
    /// Help text / description
    pub description: String,
    /// Current value
    pub value: SettingValue,
    /// Default value (for reset functionality)
    pub default: SettingValue,
    /// Whether this setting is read-only
    pub readonly: bool,
    /// Category/section for grouping
    pub section: String,
}

impl SettingItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            value: SettingValue::Bool(false),
            default: SettingValue::Bool(false),
            readonly: false,
            section: String::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn value(mut self, val: SettingValue) -> Self {
        self.value = val.clone();
        if matches!(self.default, SettingValue::Bool(false)) {
            self.default = val;
        }
        self
    }

    pub fn default(mut self, val: SettingValue) -> Self {
        self.default = val;
        self
    }

    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    pub fn section(mut self, sec: impl Into<String>) -> Self {
        self.section = sec.into();
        self
    }

    pub fn is_modified(&self) -> bool {
        self.value != self.default
    }
}

/// User-specific preferences (stored in settings.local.toml)
/// These override corresponding values in config.toml
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UserPreferences {
    /// Override editor settings
    #[serde(default)]
    pub editor: Option<EditorSettings>,

    /// Override UI theme
    #[serde(default)]
    pub theme: Option<String>,

    /// Last selected device (for quick re-launch)
    #[serde(default)]
    pub last_device: Option<String>,

    /// Last selected launch config name
    #[serde(default)]
    pub last_config: Option<String>,

    /// Window size preference (if supported)
    #[serde(default)]
    pub window: Option<WindowPrefs>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct WindowPrefs {
    pub width: Option<u16>,
    pub height: Option<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flutter_mode_as_arg() {
        assert_eq!(FlutterMode::Debug.as_arg(), "--debug");
        assert_eq!(FlutterMode::Profile.as_arg(), "--profile");
        assert_eq!(FlutterMode::Release.as_arg(), "--release");
    }

    #[test]
    fn test_flutter_mode_display() {
        assert_eq!(FlutterMode::Debug.to_string(), "debug");
        assert_eq!(FlutterMode::Profile.to_string(), "profile");
        assert_eq!(FlutterMode::Release.to_string(), "release");
    }

    #[test]
    fn test_flutter_mode_deserialize() {
        // TOML requires key = value, so we test via a wrapper struct
        #[derive(Debug, Deserialize)]
        struct ModeWrapper {
            mode: FlutterMode,
        }

        let wrapper: ModeWrapper = toml::from_str(r#"mode = "debug""#).unwrap();
        assert_eq!(wrapper.mode, FlutterMode::Debug);

        let wrapper: ModeWrapper = toml::from_str(r#"mode = "release""#).unwrap();
        assert_eq!(wrapper.mode, FlutterMode::Release);

        let wrapper: ModeWrapper = toml::from_str(r#"mode = "profile""#).unwrap();
        assert_eq!(wrapper.mode, FlutterMode::Profile);
    }

    #[test]
    fn test_launch_config_defaults() {
        let config = LaunchConfig::default();
        assert_eq!(config.device, "auto");
        assert_eq!(config.mode, FlutterMode::Debug);
        assert!(!config.auto_start);
    }

    #[test]
    fn test_settings_defaults() {
        let settings = Settings::default();
        assert!(!settings.behavior.auto_start);
        assert!(settings.behavior.confirm_quit);
        assert_eq!(settings.watcher.debounce_ms, 500);
        assert!(settings.watcher.auto_reload);
        assert_eq!(settings.ui.log_buffer_size, 10_000);
    }

    #[test]
    fn test_config_source_display() {
        assert_eq!(ConfigSource::FDemon.to_string(), ".fdemon");
        assert_eq!(ConfigSource::VSCode.to_string(), ".vscode");
        assert_eq!(ConfigSource::CommandLine.to_string(), "CLI");
        assert_eq!(ConfigSource::Default.to_string(), "default");
    }

    #[test]
    fn test_launch_file_deserialize() {
        let toml_content = r#"
[[configurations]]
name = "Development"
device = "iphone"
mode = "debug"
auto_start = true

[configurations.dart_defines]
API_URL = "https://dev.api.com"

[[configurations]]
name = "Production"
device = "ios"
mode = "release"
flavor = "production"
"#;

        let launch_file: LaunchFile = toml::from_str(toml_content).unwrap();
        assert_eq!(launch_file.configurations.len(), 2);

        let dev = &launch_file.configurations[0];
        assert_eq!(dev.name, "Development");
        assert_eq!(dev.device, "iphone");
        assert_eq!(dev.mode, FlutterMode::Debug);
        assert!(dev.auto_start);
        assert_eq!(
            dev.dart_defines.get("API_URL"),
            Some(&"https://dev.api.com".to_string())
        );

        let prod = &launch_file.configurations[1];
        assert_eq!(prod.name, "Production");
        assert_eq!(prod.mode, FlutterMode::Release);
        assert_eq!(prod.flavor, Some("production".to_string()));
    }

    #[test]
    fn test_settings_deserialize_partial() {
        let toml_content = r#"
[behavior]
auto_start = true

[watcher]
debounce_ms = 1000
"#;

        let settings: Settings = toml::from_str(toml_content).unwrap();
        assert!(settings.behavior.auto_start);
        assert!(settings.behavior.confirm_quit); // default
        assert_eq!(settings.watcher.debounce_ms, 1000);
        assert!(settings.watcher.auto_reload); // default
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Editor Settings Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_editor_settings_default() {
        let settings = EditorSettings::default();
        assert!(settings.command.is_empty());
        assert_eq!(settings.open_pattern, "$EDITOR $FILE:$LINE");
    }

    #[test]
    fn test_settings_includes_editor() {
        let settings = Settings::default();
        // Editor settings should be present with defaults
        assert!(settings.editor.command.is_empty());
        assert_eq!(settings.editor.open_pattern, "$EDITOR $FILE:$LINE");
    }

    #[test]
    fn test_parent_ide_equality() {
        assert_eq!(ParentIde::VSCode, ParentIde::VSCode);
        assert_ne!(ParentIde::VSCode, ParentIde::Cursor);
    }

    #[test]
    fn test_parent_ide_clone() {
        let ide = ParentIde::Zed;
        let cloned = ide;
        assert_eq!(ide, cloned);
    }

    #[test]
    fn test_editor_settings_clone() {
        let settings = EditorSettings {
            command: "code".to_string(),
            open_pattern: "code --goto $FILE:$LINE".to_string(),
        };
        let cloned = settings.clone();
        assert_eq!(settings.command, cloned.command);
        assert_eq!(settings.open_pattern, cloned.open_pattern);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Settings UI Types Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_settings_tab_navigation() {
        assert_eq!(SettingsTab::Project.next(), SettingsTab::UserPrefs);
        assert_eq!(SettingsTab::UserPrefs.next(), SettingsTab::LaunchConfig);
        assert_eq!(SettingsTab::LaunchConfig.next(), SettingsTab::VSCodeConfig);
        assert_eq!(SettingsTab::VSCodeConfig.next(), SettingsTab::Project);
        assert_eq!(SettingsTab::Project.prev(), SettingsTab::VSCodeConfig);
        assert_eq!(SettingsTab::VSCodeConfig.prev(), SettingsTab::LaunchConfig);
    }

    #[test]
    fn test_settings_tab_from_index() {
        assert_eq!(SettingsTab::from_index(0), Some(SettingsTab::Project));
        assert_eq!(SettingsTab::from_index(1), Some(SettingsTab::UserPrefs));
        assert_eq!(SettingsTab::from_index(2), Some(SettingsTab::LaunchConfig));
        assert_eq!(SettingsTab::from_index(3), Some(SettingsTab::VSCodeConfig));
        assert_eq!(SettingsTab::from_index(4), None);
    }

    #[test]
    fn test_settings_tab_label() {
        assert_eq!(SettingsTab::Project.label(), "Project");
        assert_eq!(SettingsTab::UserPrefs.label(), "User");
        assert_eq!(SettingsTab::LaunchConfig.label(), "Launch");
        assert_eq!(SettingsTab::VSCodeConfig.label(), "VSCode");
    }

    #[test]
    fn test_settings_tab_index() {
        assert_eq!(SettingsTab::Project.index(), 0);
        assert_eq!(SettingsTab::UserPrefs.index(), 1);
        assert_eq!(SettingsTab::LaunchConfig.index(), 2);
        assert_eq!(SettingsTab::VSCodeConfig.index(), 3);
    }

    #[test]
    fn test_setting_value_display() {
        assert_eq!(SettingValue::Bool(true).display(), "true");
        assert_eq!(SettingValue::Bool(false).display(), "false");
        assert_eq!(SettingValue::Number(42).display(), "42");
        assert_eq!(SettingValue::Float(2.5).display(), "2.50");
        assert_eq!(SettingValue::String("hello".into()).display(), "hello");
        assert_eq!(
            SettingValue::Enum {
                value: "option1".into(),
                options: vec!["option1".into(), "option2".into()]
            }
            .display(),
            "option1"
        );
        assert_eq!(
            SettingValue::List(vec!["a".into(), "b".into(), "c".into()]).display(),
            "a, b, c"
        );
    }

    #[test]
    fn test_setting_value_type_name() {
        assert_eq!(SettingValue::Bool(true).type_name(), "boolean");
        assert_eq!(SettingValue::Number(42).type_name(), "number");
        assert_eq!(SettingValue::Float(2.5).type_name(), "float");
        assert_eq!(SettingValue::String("test".into()).type_name(), "string");
        assert_eq!(
            SettingValue::Enum {
                value: "opt".into(),
                options: vec![]
            }
            .type_name(),
            "enum"
        );
        assert_eq!(SettingValue::List(vec![]).type_name(), "list");
    }

    #[test]
    fn test_setting_item_builder() {
        let item = SettingItem::new("test.id", "Test Label")
            .description("A test setting")
            .value(SettingValue::Bool(true))
            .section("Test");

        assert_eq!(item.id, "test.id");
        assert_eq!(item.label, "Test Label");
        assert_eq!(item.description, "A test setting");
        assert_eq!(item.value, SettingValue::Bool(true));
        assert_eq!(item.section, "Test");
        assert!(!item.readonly);
        assert!(!item.is_modified()); // value == default
    }

    #[test]
    fn test_setting_item_is_modified() {
        let item = SettingItem::new("test", "Test")
            .value(SettingValue::Bool(true))
            .default(SettingValue::Bool(false));

        assert!(item.is_modified());

        let item2 = SettingItem::new("test2", "Test2")
            .value(SettingValue::Number(42))
            .default(SettingValue::Number(42));

        assert!(!item2.is_modified());
    }

    #[test]
    fn test_setting_item_readonly() {
        let item = SettingItem::new("test", "Test").readonly();
        assert!(item.readonly);
    }

    #[test]
    fn test_user_preferences_default() {
        let prefs = UserPreferences::default();
        assert!(prefs.editor.is_none());
        assert!(prefs.theme.is_none());
        assert!(prefs.last_device.is_none());
        assert!(prefs.last_config.is_none());
        assert!(prefs.window.is_none());
    }

    #[test]
    fn test_window_prefs_default() {
        let window = WindowPrefs::default();
        assert!(window.width.is_none());
        assert!(window.height.is_none());
    }

    #[test]
    fn test_user_preferences_deserialize() {
        let toml_content = r#"
theme = "dark"
last_device = "iphone-15"
last_config = "development"

[window]
width = 1920
height = 1080
"#;

        let prefs: UserPreferences = toml::from_str(toml_content).unwrap();
        assert_eq!(prefs.theme, Some("dark".to_string()));
        assert_eq!(prefs.last_device, Some("iphone-15".to_string()));
        assert_eq!(prefs.last_config, Some("development".to_string()));
        assert!(prefs.window.is_some());
        assert_eq!(prefs.window.as_ref().unwrap().width, Some(1920));
        assert_eq!(prefs.window.as_ref().unwrap().height, Some(1080));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // IconMode Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_icon_mode_default() {
        assert_eq!(IconMode::default(), IconMode::NerdFonts);
    }

    #[test]
    fn test_icon_mode_display() {
        assert_eq!(IconMode::Unicode.to_string(), "unicode");
        assert_eq!(IconMode::NerdFonts.to_string(), "nerd_fonts");
    }

    #[test]
    fn test_icon_mode_deserialize() {
        let toml = r#"icons = "nerd_fonts""#;
        #[derive(Deserialize)]
        struct W {
            icons: IconMode,
        }
        let w: W = toml::from_str(toml).unwrap();
        assert_eq!(w.icons, IconMode::NerdFonts);
    }

    #[test]
    fn test_settings_with_icons_field() {
        let toml = r#"
[ui]
icons = "nerd_fonts"
"#;
        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.ui.icons, IconMode::NerdFonts);
    }

    #[test]
    fn test_settings_without_icons_field_defaults() {
        let toml = r#"
[ui]
theme = "default"
"#;
        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.ui.icons, IconMode::NerdFonts);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DevToolsSettings Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_devtools_settings_default_values() {
        let settings = DevToolsSettings::default();
        assert!(!settings.auto_open);
        assert!(settings.browser.is_empty());
        assert_eq!(settings.default_panel, "inspector");
        assert_eq!(settings.performance_refresh_ms, 2000);
        assert_eq!(settings.memory_history_size, 60);
        assert_eq!(settings.tree_max_depth, 0);
        assert_eq!(settings.inspector_fetch_timeout_secs, 60);
        assert!(!settings.auto_repaint_rainbow);
        assert!(!settings.auto_performance_overlay);
        // Network settings defaults
        assert_eq!(settings.max_network_entries, 500);
        assert!(settings.network_auto_record);
        assert_eq!(settings.network_poll_interval_ms, 1000);
    }

    #[test]
    fn test_devtools_settings_backwards_compatible_deserialization() {
        // Old config with only auto_open and browser should still work
        let toml = r#"
            auto_open = true
            browser = "firefox"
        "#;
        let settings: DevToolsSettings = toml::from_str(toml).unwrap();
        assert!(settings.auto_open);
        assert_eq!(settings.browser, "firefox");
        // New fields should have defaults
        assert_eq!(settings.default_panel, "inspector");
        assert_eq!(settings.performance_refresh_ms, 2000);
        assert_eq!(settings.inspector_fetch_timeout_secs, 60);
        // Network fields should have defaults too
        assert_eq!(settings.max_network_entries, 500);
        assert!(settings.network_auto_record);
        assert_eq!(settings.network_poll_interval_ms, 1000);
    }

    #[test]
    fn test_devtools_settings_full_deserialization() {
        let toml = r#"
            auto_open = false
            browser = ""
            default_panel = "performance"
            performance_refresh_ms = 5000
            memory_history_size = 120
            tree_max_depth = 10
            inspector_fetch_timeout_secs = 15
            auto_repaint_rainbow = true
            auto_performance_overlay = false
            max_network_entries = 200
            network_auto_record = false
            network_poll_interval_ms = 2000

            [logging]
            hybrid_enabled = true
            prefer_vm_level = false
            show_source_indicator = true
            dedupe_threshold_ms = 200
        "#;
        let settings: DevToolsSettings = toml::from_str(toml).unwrap();
        assert_eq!(settings.default_panel, "performance");
        assert_eq!(settings.performance_refresh_ms, 5000);
        assert_eq!(settings.memory_history_size, 120);
        assert_eq!(settings.tree_max_depth, 10);
        assert_eq!(settings.inspector_fetch_timeout_secs, 15);
        assert!(settings.auto_repaint_rainbow);
        assert!(settings.logging.show_source_indicator);
        assert_eq!(settings.logging.dedupe_threshold_ms, 200);
        // Network fields
        assert_eq!(settings.max_network_entries, 200);
        assert!(!settings.network_auto_record);
        assert_eq!(settings.network_poll_interval_ms, 2000);
    }

    #[test]
    fn test_devtools_logging_settings_defaults() {
        let logging = DevToolsLoggingSettings::default();
        assert!(logging.hybrid_enabled);
        assert!(logging.prefer_vm_level);
        assert!(!logging.show_source_indicator);
        assert_eq!(logging.dedupe_threshold_ms, 100);
    }

    #[test]
    fn test_settings_devtools_section_defaults() {
        let settings = Settings::default();
        assert_eq!(settings.devtools.default_panel, "inspector");
        assert_eq!(settings.devtools.performance_refresh_ms, 2000);
        assert_eq!(settings.devtools.memory_history_size, 60);
        assert_eq!(settings.devtools.tree_max_depth, 0);
        assert!(!settings.devtools.auto_repaint_rainbow);
        assert!(!settings.devtools.auto_performance_overlay);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DapSettings Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_dap_settings_defaults() {
        let settings = DapSettings::default();
        assert!(!settings.enabled);
        assert!(settings.auto_start_in_ide);
        assert_eq!(settings.port, 0);
        assert_eq!(settings.bind_address, "127.0.0.1");
        assert!(settings.suppress_reload_on_pause);
    }

    #[test]
    fn test_dap_settings_deserialize_from_toml() {
        let toml = r#"
            [dap]
            enabled = true
            port = 4711
            bind_address = "0.0.0.0"
        "#;
        let settings: Settings = toml::from_str(toml).unwrap();
        assert!(settings.dap.enabled);
        assert_eq!(settings.dap.port, 4711);
        assert_eq!(settings.dap.bind_address, "0.0.0.0");
        // Unspecified fields use defaults
        assert!(settings.dap.auto_start_in_ide);
        assert!(settings.dap.suppress_reload_on_pause);
    }

    #[test]
    fn test_settings_without_dap_section_uses_defaults() {
        let toml = r#"
            [behavior]
            auto_start = true
        "#;
        let settings: Settings = toml::from_str(toml).unwrap();
        assert!(!settings.dap.enabled);
        assert!(settings.dap.auto_start_in_ide);
        assert_eq!(settings.dap.port, 0);
        assert_eq!(settings.dap.bind_address, "127.0.0.1");
        assert!(settings.dap.suppress_reload_on_pause);
    }

    #[test]
    fn test_dap_settings_full_deserialization() {
        let toml = r#"
            enabled = true
            auto_start_in_ide = false
            port = 8080
            bind_address = "0.0.0.0"
            suppress_reload_on_pause = false
        "#;
        let dap: DapSettings = toml::from_str(toml).unwrap();
        assert!(dap.enabled);
        assert!(!dap.auto_start_in_ide);
        assert_eq!(dap.port, 8080);
        assert_eq!(dap.bind_address, "0.0.0.0");
        assert!(!dap.suppress_reload_on_pause);
    }

    #[test]
    fn test_settings_includes_dap_defaults() {
        let settings = Settings::default();
        assert!(!settings.dap.enabled);
        assert!(settings.dap.auto_start_in_ide);
        assert_eq!(settings.dap.port, 0);
        assert_eq!(settings.dap.bind_address, "127.0.0.1");
        assert!(settings.dap.suppress_reload_on_pause);
    }

    #[test]
    fn test_dap_settings_default_auto_configure_ide() {
        let settings = DapSettings::default();
        assert!(settings.auto_configure_ide);
    }

    #[test]
    fn test_dap_settings_deserialize_without_auto_configure_ide() {
        let toml = r#"
        enabled = false
        port = 0
        bind_address = "127.0.0.1"
        "#;
        let settings: DapSettings = toml::from_str(toml).unwrap();
        assert!(settings.auto_configure_ide); // default true
    }

    #[test]
    fn test_dap_settings_deserialize_with_auto_configure_ide_false() {
        let toml = r#"
        enabled = false
        auto_configure_ide = false
        "#;
        let settings: DapSettings = toml::from_str(toml).unwrap();
        assert!(!settings.auto_configure_ide);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // ParentIde::Emacs / Helix Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_emacs_display_name() {
        assert_eq!(ParentIde::Emacs.display_name(), "Emacs");
    }

    #[test]
    fn test_helix_display_name() {
        assert_eq!(ParentIde::Helix.display_name(), "Helix");
    }

    #[test]
    fn test_emacs_url_scheme_is_file() {
        assert_eq!(ParentIde::Emacs.url_scheme(), "file");
    }

    #[test]
    fn test_helix_url_scheme_is_file() {
        assert_eq!(ParentIde::Helix.url_scheme(), "file");
    }

    #[test]
    fn test_emacs_reuse_flag_is_none() {
        assert_eq!(ParentIde::Emacs.reuse_flag(), None);
    }

    #[test]
    fn test_helix_reuse_flag_is_none() {
        assert_eq!(ParentIde::Helix.reuse_flag(), None);
    }

    #[test]
    fn test_supports_dap_config_true_for_all_except_intellij() {
        assert!(ParentIde::VSCode.supports_dap_config());
        assert!(ParentIde::VSCodeInsiders.supports_dap_config());
        assert!(ParentIde::Cursor.supports_dap_config());
        assert!(ParentIde::Zed.supports_dap_config());
        assert!(ParentIde::Neovim.supports_dap_config());
        assert!(ParentIde::Emacs.supports_dap_config());
        assert!(ParentIde::Helix.supports_dap_config());
        assert!(!ParentIde::IntelliJ.supports_dap_config());
        assert!(!ParentIde::AndroidStudio.supports_dap_config());
    }

    #[test]
    fn test_dap_config_path_vscode_family() {
        let root = std::path::Path::new("/project");
        assert_eq!(
            ParentIde::VSCode.dap_config_path(root),
            Some(root.join(".vscode/launch.json"))
        );
        assert_eq!(
            ParentIde::VSCodeInsiders.dap_config_path(root),
            Some(root.join(".vscode/launch.json"))
        );
        assert_eq!(
            ParentIde::Cursor.dap_config_path(root),
            Some(root.join(".vscode/launch.json"))
        );
        assert_eq!(
            ParentIde::Neovim.dap_config_path(root),
            Some(root.join(".vscode/launch.json"))
        );
    }

    #[test]
    fn test_dap_config_path_helix() {
        let root = std::path::Path::new("/project");
        assert_eq!(
            ParentIde::Helix.dap_config_path(root),
            Some(root.join(".helix/languages.toml"))
        );
    }

    #[test]
    fn test_dap_config_path_zed() {
        let root = std::path::Path::new("/project");
        assert_eq!(
            ParentIde::Zed.dap_config_path(root),
            Some(root.join(".zed/debug.json"))
        );
    }

    #[test]
    fn test_dap_config_path_emacs() {
        let root = std::path::Path::new("/project");
        assert_eq!(
            ParentIde::Emacs.dap_config_path(root),
            Some(root.join(".fdemon/dap-emacs.el"))
        );
    }

    #[test]
    fn test_dap_config_path_none_for_intellij() {
        assert_eq!(
            ParentIde::IntelliJ.dap_config_path(std::path::Path::new("/p")),
            None
        );
    }

    #[test]
    fn test_dap_config_path_none_for_android_studio() {
        assert_eq!(
            ParentIde::AndroidStudio.dap_config_path(std::path::Path::new("/p")),
            None
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // NativeLogsSettings Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_native_logs_settings_default() {
        let settings = NativeLogsSettings::default();
        assert!(settings.enabled);
        assert_eq!(settings.exclude_tags, vec!["flutter".to_string()]);
        assert!(settings.include_tags.is_empty());
        assert_eq!(settings.min_level, "info");
        assert!(settings.tags.is_empty());
    }

    #[test]
    fn test_should_include_tag_default_excludes_flutter() {
        let settings = NativeLogsSettings::default();
        assert!(!settings.should_include_tag("flutter"));
        assert!(!settings.should_include_tag("Flutter")); // case-insensitive
        assert!(settings.should_include_tag("GoLog"));
        assert!(settings.should_include_tag("OkHttp"));
    }

    #[test]
    fn test_should_include_tag_whitelist_mode() {
        let settings = NativeLogsSettings {
            include_tags: vec!["GoLog".to_string(), "MyPlugin".to_string()],
            ..Default::default()
        };
        assert!(settings.should_include_tag("GoLog"));
        assert!(settings.should_include_tag("golog")); // case-insensitive
        assert!(settings.should_include_tag("MyPlugin"));
        assert!(!settings.should_include_tag("OkHttp"));
        // include_tags overrides exclude_tags
        assert!(!settings.should_include_tag("flutter"));
    }

    #[test]
    fn test_native_logs_settings_toml_deserialization() {
        let toml_str = r#"
            [native_logs]
            enabled = false
            exclude_tags = ["flutter", "art"]
            min_level = "debug"
        "#;
        // Wrap in a Settings-like struct for testing
        #[derive(Deserialize)]
        struct TestConfig {
            native_logs: NativeLogsSettings,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.native_logs.enabled);
        assert_eq!(config.native_logs.exclude_tags, vec!["flutter", "art"]);
        assert_eq!(config.native_logs.min_level, "debug");
    }

    #[test]
    fn test_native_logs_settings_missing_section_uses_defaults() {
        let toml_str = "";
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(settings.native_logs.enabled);
        assert_eq!(
            settings.native_logs.exclude_tags,
            vec!["flutter".to_string()]
        );
    }

    #[test]
    fn test_native_logs_settings_include_tags_deserialization() {
        let toml_str = r#"
            [native_logs]
            include_tags = ["GoLog", "MyPlugin"]
        "#;
        #[derive(Deserialize)]
        struct TestConfig {
            native_logs: NativeLogsSettings,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.native_logs.include_tags,
            vec!["GoLog".to_string(), "MyPlugin".to_string()]
        );
        // include_tags overrides exclude_tags in whitelist mode
        assert!(!config.native_logs.should_include_tag("OkHttp"));
        assert!(config.native_logs.should_include_tag("GoLog"));
    }

    #[test]
    fn test_settings_default_includes_native_logs() {
        let settings = Settings::default();
        assert!(settings.native_logs.enabled);
        assert_eq!(
            settings.native_logs.exclude_tags,
            vec!["flutter".to_string()]
        );
        assert!(settings.native_logs.include_tags.is_empty());
        assert_eq!(settings.native_logs.min_level, "info");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Per-tag configuration tests (Task 08)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_default_settings_empty_tags() {
        let settings = NativeLogsSettings::default();
        assert!(settings.tags.is_empty());
    }

    #[test]
    fn test_effective_min_level_global_fallback() {
        let settings = NativeLogsSettings {
            min_level: "info".to_string(),
            tags: HashMap::new(),
            ..Default::default()
        };
        assert_eq!(settings.effective_min_level("GoLog"), "info");
    }

    #[test]
    fn test_effective_min_level_per_tag_override() {
        let mut settings = NativeLogsSettings::default();
        settings.tags.insert(
            "GoLog".to_string(),
            TagConfig {
                min_level: Some("debug".to_string()),
            },
        );
        assert_eq!(settings.effective_min_level("GoLog"), "debug");
        assert_eq!(settings.effective_min_level("OkHttp"), "info"); // fallback to global
    }

    #[test]
    fn test_effective_min_level_per_tag_none_uses_global() {
        let mut settings = NativeLogsSettings::default();
        settings
            .tags
            .insert("GoLog".to_string(), TagConfig { min_level: None });
        assert_eq!(settings.effective_min_level("GoLog"), "info"); // global default
    }

    #[test]
    fn test_toml_deserialization_with_tags() {
        let toml_str = r#"
enabled = true
exclude_tags = ["flutter"]
min_level = "info"

[tags.GoLog]
min_level = "debug"

[tags.OkHttp]
min_level = "warning"
"#;
        let settings: NativeLogsSettings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.effective_min_level("GoLog"), "debug");
        assert_eq!(settings.effective_min_level("OkHttp"), "warning");
        assert_eq!(settings.effective_min_level("Unknown"), "info");
    }

    #[test]
    fn test_toml_deserialization_no_tags_section() {
        let toml_str = r#"
enabled = true
exclude_tags = ["flutter"]
min_level = "info"
"#;
        let settings: NativeLogsSettings = toml::from_str(toml_str).unwrap();
        assert!(settings.tags.is_empty());
    }

    #[test]
    fn test_toml_deserialization_dotted_tag_name() {
        let toml_str = r#"
enabled = true
min_level = "info"

[tags."com.example.myplugin"]
min_level = "debug"
"#;
        let settings: NativeLogsSettings = toml::from_str(toml_str).unwrap();
        assert_eq!(
            settings.effective_min_level("com.example.myplugin"),
            "debug"
        );
    }

    #[test]
    fn test_tag_config_default_has_no_min_level() {
        let tc = TagConfig::default();
        assert!(tc.min_level.is_none());
    }

    // ── Case-insensitivity tests for effective_min_level (Issue #8) ──────────

    #[test]
    fn test_effective_min_level_case_insensitive() {
        // Config has tags.GoLog.min_level = "error"; lookup must work regardless
        // of the case used at call time.
        let mut settings = NativeLogsSettings::default();
        settings.tags.insert(
            "GoLog".to_string(),
            TagConfig {
                min_level: Some("error".to_string()),
            },
        );
        assert_eq!(settings.effective_min_level("GoLog"), "error");
        assert_eq!(settings.effective_min_level("goLog"), "error");
        assert_eq!(settings.effective_min_level("GOLOG"), "error");
        assert_eq!(settings.effective_min_level("golog"), "error");
    }

    #[test]
    fn test_effective_min_level_case_insensitive_toml() {
        // Same as above but via TOML deserialization to match real usage.
        let toml_str = r#"
min_level = "info"

[tags.GoLog]
min_level = "error"
"#;
        let settings: NativeLogsSettings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.effective_min_level("GoLog"), "error");
        assert_eq!(settings.effective_min_level("golog"), "error");
        assert_eq!(settings.effective_min_level("GOLOG"), "error");
    }

    // ── Duplicate custom source name validation tests (Issue #11) ─────────────

    #[test]
    fn test_native_logs_settings_validate_no_custom_sources() {
        let settings = NativeLogsSettings::default();
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_native_logs_settings_validate_unique_names_passes() {
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "go-backend".to_string(),
                    command: "adb".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "my-server".to_string(),
                    command: "my-tool".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_duplicate_custom_source_name_rejected() {
        // Two sources with the exact same name must fail validation.
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "mylog".to_string(),
                    command: "adb".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "mylog".to_string(),
                    command: "my-tool".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        let result = settings.validate();
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("Duplicate custom source name"),
            "error should mention 'Duplicate custom source name'"
        );
    }

    #[test]
    fn test_duplicate_custom_source_name_case_insensitive_rejected() {
        // "mylog" and "MyLog" are the same name — must be rejected.
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "mylog".to_string(),
                    command: "adb".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "MyLog".to_string(),
                    command: "my-tool".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        let result = settings.validate();
        assert!(
            result.is_err(),
            "case-insensitive duplicate should fail validation"
        );
    }

    #[test]
    fn test_invalid_custom_source_propagates_error() {
        // validate() must propagate errors from CustomSourceConfig::validate().
        let settings = NativeLogsSettings {
            custom_sources: vec![CustomSourceConfig {
                name: String::new(), // invalid: empty name
                command: "adb".to_string(),
                args: vec![],
                format: OutputFormat::Raw,
                working_dir: None,
                env: HashMap::new(),
                start_before_app: false,
                shared: false,
                ready_check: None,
            }],
            ..NativeLogsSettings::default()
        };
        assert!(settings.validate().is_err());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // CustomSourceConfig & OutputFormat Tests (Phase 3 Task 01)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_output_format_kebab_case_serde() {
        #[derive(Debug, Deserialize, Serialize, PartialEq)]
        struct W {
            format: OutputFormat,
        }

        let cases = [
            ("raw", OutputFormat::Raw),
            ("json", OutputFormat::Json),
            ("logcat-threadtime", OutputFormat::LogcatThreadtime),
            ("syslog", OutputFormat::Syslog),
        ];

        for (s, expected) in cases {
            let toml = format!(r#"format = "{}""#, s);
            let w: W = toml::from_str(&toml).unwrap();
            assert_eq!(w.format, expected, "format = {s:?}");
        }
    }

    #[test]
    fn test_output_format_serialize_kebab_case() {
        #[derive(Debug, Deserialize, Serialize)]
        struct W {
            format: OutputFormat,
        }
        let w = W {
            format: OutputFormat::LogcatThreadtime,
        };
        let s = toml::to_string(&w).unwrap();
        assert!(s.contains("logcat-threadtime"), "got: {s}");
    }

    #[test]
    fn test_output_format_default_is_raw() {
        assert_eq!(OutputFormat::default(), OutputFormat::Raw);
    }

    #[test]
    fn test_custom_source_config_deserialize() {
        let toml = r#"
name = "go-backend"
command = "adb"
args = ["logcat", "GoLog:D", "*:S", "-v", "threadtime"]
format = "logcat-threadtime"
"#;
        let cfg: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.name, "go-backend");
        assert_eq!(cfg.command, "adb");
        assert_eq!(
            cfg.args,
            vec!["logcat", "GoLog:D", "*:S", "-v", "threadtime"]
        );
        assert_eq!(cfg.format, OutputFormat::LogcatThreadtime);
        assert!(cfg.working_dir.is_none());
        assert!(cfg.env.is_empty());
    }

    #[test]
    fn test_custom_source_config_full_fields() {
        let toml = r#"
name = "my-server"
command = "/usr/local/bin/my-log-tool"
args = ["--follow", "--json"]
format = "json"
working_dir = "/tmp"

[env]
LOG_LEVEL = "debug"
APP_ENV = "test"
"#;
        let cfg: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.name, "my-server");
        assert_eq!(cfg.command, "/usr/local/bin/my-log-tool");
        assert_eq!(cfg.format, OutputFormat::Json);
        assert_eq!(cfg.working_dir, Some("/tmp".to_string()));
        assert_eq!(cfg.env.get("LOG_LEVEL"), Some(&"debug".to_string()));
        assert_eq!(cfg.env.get("APP_ENV"), Some(&"test".to_string()));
    }

    #[test]
    fn test_custom_source_default_format_is_raw() {
        let toml = r#"
name = "sidecar"
command = "tail"
args = ["-f", "/tmp/sidecar.log"]
"#;
        let cfg: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.format, OutputFormat::Raw);
    }

    #[test]
    fn test_custom_source_config_round_trip() {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "val".to_string());
        let original = CustomSourceConfig {
            name: "test-source".to_string(),
            command: "my-cmd".to_string(),
            args: vec!["--arg".to_string(), "value".to_string()],
            format: OutputFormat::Json,
            working_dir: Some("/tmp".to_string()),
            env,
            start_before_app: false,
            shared: false,
            ready_check: None,
        };
        let serialized = toml::to_string(&original).unwrap();
        let deserialized: CustomSourceConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, original.name);
        assert_eq!(deserialized.command, original.command);
        assert_eq!(deserialized.args, original.args);
        assert_eq!(deserialized.format, original.format);
        assert_eq!(deserialized.working_dir, original.working_dir);
        assert_eq!(deserialized.env, original.env);
    }

    #[test]
    fn test_custom_source_empty_name_fails_validation() {
        let cfg = CustomSourceConfig {
            name: String::new(),
            command: "adb".to_string(),
            args: vec![],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            start_before_app: false,
            shared: false,
            ready_check: None,
        };
        assert!(cfg.validate().is_err());
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("name must not be empty"), "got: {err}");
    }

    #[test]
    fn test_custom_source_whitespace_only_name_fails_validation() {
        let cfg = CustomSourceConfig {
            name: "   ".to_string(),
            command: "adb".to_string(),
            args: vec![],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            start_before_app: false,
            shared: false,
            ready_check: None,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_custom_source_empty_command_fails_validation() {
        let cfg = CustomSourceConfig {
            name: "my-source".to_string(),
            command: String::new(),
            args: vec![],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            start_before_app: false,
            shared: false,
            ready_check: None,
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("command must not be empty"), "got: {err}");
        assert!(
            err.contains("my-source"),
            "error should mention name; got: {err}"
        );
    }

    #[test]
    fn test_custom_source_valid_config_passes_validation() {
        let cfg = CustomSourceConfig {
            name: "logcat-watcher".to_string(),
            command: "adb".to_string(),
            args: vec!["logcat".to_string()],
            format: OutputFormat::LogcatThreadtime,
            working_dir: None,
            env: HashMap::new(),
            start_before_app: false,
            shared: false,
            ready_check: None,
        };
        assert!(cfg.validate().is_ok());
    }

    /// On non-macOS platforms, `format = "syslog"` must be rejected at config
    /// validation time so users get an actionable error instead of silent empty output.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_custom_source_syslog_format_rejected_on_non_macos() {
        let cfg = CustomSourceConfig {
            name: "my-source".to_string(),
            command: "my-tool".to_string(),
            args: vec![],
            format: OutputFormat::Syslog,
            working_dir: None,
            env: HashMap::new(),
            start_before_app: false,
            shared: false,
            ready_check: None,
        };
        let err = cfg.validate().unwrap_err();
        assert!(
            err.contains("syslog format is only supported on macOS"),
            "expected syslog rejection message, got: {:?}",
            err
        );
    }

    /// On macOS, `format = "syslog"` is valid and must pass validation.
    #[cfg(target_os = "macos")]
    #[test]
    fn test_custom_source_syslog_format_allowed_on_macos() {
        let cfg = CustomSourceConfig {
            name: "my-source".to_string(),
            command: "log".to_string(),
            args: vec!["stream".to_string()],
            format: OutputFormat::Syslog,
            working_dir: None,
            env: HashMap::new(),
            start_before_app: false,
            shared: false,
            ready_check: None,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_native_logs_settings_default_has_empty_custom_sources() {
        let settings = NativeLogsSettings::default();
        assert!(settings.custom_sources.is_empty());
    }

    #[test]
    fn test_native_logs_settings_custom_sources_deserialize() {
        let toml_str = r#"
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

[[native_logs.custom_sources]]
name = "sidecar"
command = "tail"
args = ["-f", "/tmp/sidecar.log"]
format = "raw"
working_dir = "/tmp"
"#;
        #[derive(Deserialize)]
        struct TestConfig {
            native_logs: NativeLogsSettings,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.native_logs.custom_sources.len(), 3);

        let go = &config.native_logs.custom_sources[0];
        assert_eq!(go.name, "go-backend");
        assert_eq!(go.command, "adb");
        assert_eq!(go.format, OutputFormat::LogcatThreadtime);

        let server = &config.native_logs.custom_sources[1];
        assert_eq!(server.name, "my-server");
        assert_eq!(server.format, OutputFormat::Json);

        let sidecar = &config.native_logs.custom_sources[2];
        assert_eq!(sidecar.name, "sidecar");
        assert_eq!(sidecar.format, OutputFormat::Raw);
        assert_eq!(sidecar.working_dir, Some("/tmp".to_string()));
    }

    #[test]
    fn test_existing_config_without_custom_sources_still_works() {
        // Existing config without custom_sources should deserialize fine (backward compat)
        let toml_str = r#"
enabled = true
exclude_tags = ["flutter"]
min_level = "info"

[tags.GoLog]
min_level = "debug"
"#;
        let settings: NativeLogsSettings = toml::from_str(toml_str).unwrap();
        assert!(settings.custom_sources.is_empty());
        assert!(settings.enabled);
        assert_eq!(settings.effective_min_level("GoLog"), "debug");
    }

    #[test]
    fn test_settings_without_native_logs_custom_sources_is_backward_compatible() {
        let toml_str = "";
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(settings.native_logs.custom_sources.is_empty());
    }

    #[test]
    fn test_custom_source_env_inline_table_deserialize() {
        // Test that TOML inline table env syntax works as documented
        let toml = r#"
name = "server"
command = "my-tool"
env = { LOG_LEVEL = "debug", TRACE = "1" }
"#;
        let cfg: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.env.get("LOG_LEVEL"), Some(&"debug".to_string()));
        assert_eq!(cfg.env.get("TRACE"), Some(&"1".to_string()));
    }

    // ── Phase 3 Task 05 edge-case tests ──────────────────────────────────────

    /// Round-trip serde at the `NativeLogsSettings` level, not just
    /// `CustomSourceConfig`, to verify the full nesting serializes correctly.
    #[test]
    fn test_custom_sources_round_trip_serde_via_native_logs_settings() {
        let settings = NativeLogsSettings {
            custom_sources: vec![CustomSourceConfig {
                name: "test".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                format: OutputFormat::Raw,
                working_dir: None,
                env: HashMap::new(),
                start_before_app: false,
                shared: false,
                ready_check: None,
            }],
            ..NativeLogsSettings::default()
        };

        let serialized = toml::to_string(&settings).unwrap();
        let parsed: NativeLogsSettings = toml::from_str(&serialized).unwrap();

        assert_eq!(parsed.custom_sources.len(), 1);
        assert_eq!(parsed.custom_sources[0].name, "test");
        assert_eq!(parsed.custom_sources[0].command, "echo");
        assert_eq!(parsed.custom_sources[0].args, vec!["hello"]);
        assert_eq!(parsed.custom_sources[0].format, OutputFormat::Raw);
        assert!(parsed.custom_sources[0].working_dir.is_none());
        assert!(parsed.custom_sources[0].env.is_empty());
    }

    /// All optional fields of `CustomSourceConfig` must have sensible defaults
    /// when omitted from the TOML: args=[], format=Raw, working_dir=None, env={}.
    #[test]
    fn test_custom_source_optional_fields_default_when_omitted() {
        let toml = r#"
name = "minimal"
command = "echo"
"#;
        let cfg: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.name, "minimal");
        assert_eq!(cfg.command, "echo");
        assert!(cfg.args.is_empty(), "args should default to empty vec");
        assert_eq!(
            cfg.format,
            OutputFormat::Raw,
            "format should default to Raw"
        );
        assert!(
            cfg.working_dir.is_none(),
            "working_dir should default to None"
        );
        assert!(cfg.env.is_empty(), "env should default to empty map");
    }

    /// Deserialize all four `OutputFormat` variants inside a `CustomSourceConfig`.
    #[test]
    fn test_all_output_format_variants_deserialize_in_custom_source() {
        let cases = [
            ("raw", OutputFormat::Raw),
            ("json", OutputFormat::Json),
            ("logcat-threadtime", OutputFormat::LogcatThreadtime),
            ("syslog", OutputFormat::Syslog),
        ];

        for (format_str, expected) in cases {
            let toml = format!(
                "name = \"src\"\ncommand = \"cmd\"\nformat = \"{}\"\n",
                format_str
            );
            let cfg: CustomSourceConfig = toml::from_str(&toml)
                .unwrap_or_else(|e| panic!("failed to parse format={format_str}: {e}"));
            assert_eq!(
                cfg.format, expected,
                "format={format_str} should deserialize to {expected:?}"
            );
        }
    }

    /// Env inline-table with the exact example from the task description.
    #[test]
    fn test_custom_source_env_inline_table_with_path_prefix() {
        let toml = r#"
name = "with-env"
command = "my-tool"
env = { VERBOSE = "1", PATH_PREFIX = "/opt" }
"#;
        let cfg: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.env.get("VERBOSE"), Some(&"1".to_string()));
        assert_eq!(cfg.env.get("PATH_PREFIX"), Some(&"/opt".to_string()));
        assert_eq!(cfg.env.len(), 2, "env should have exactly 2 entries");
    }

    // ─── ReadyCheck deserialization ───────────────────────────────────────────

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
    fn test_ready_check_http_defaults() {
        let toml = r#"
name = "server"
command = "cargo"
args = ["run"]
start_before_app = true
ready_check = { type = "http", url = "http://localhost:8080/health" }
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        if let Some(ReadyCheck::Http {
            interval_ms,
            timeout_s,
            ..
        }) = config.ready_check
        {
            assert_eq!(interval_ms, 500);
            assert_eq!(timeout_s, 30);
        } else {
            panic!("expected ReadyCheck::Http");
        }
    }

    #[test]
    fn test_ready_check_tcp_deserialize() {
        let toml = r#"
name = "db"
command = "pg_isready"
start_before_app = true
ready_check = { type = "tcp", host = "localhost", port = 5432 }
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        if let Some(ReadyCheck::Tcp {
            host,
            port,
            interval_ms,
            timeout_s,
        }) = config.ready_check
        {
            assert_eq!(host, "localhost");
            assert_eq!(port, 5432);
            assert_eq!(interval_ms, 500);
            assert_eq!(timeout_s, 30);
        } else {
            panic!("expected ReadyCheck::Tcp");
        }
    }

    #[test]
    fn test_ready_check_command_deserialize() {
        let toml = r#"
name = "grpc"
command = "grpc-server"
start_before_app = true
ready_check = { type = "command", command = "grpcurl", args = ["-plaintext", "localhost:50051", "list"] }
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        if let Some(ReadyCheck::Command {
            command,
            args,
            interval_ms,
            timeout_s,
        }) = config.ready_check
        {
            assert_eq!(command, "grpcurl");
            assert_eq!(args, vec!["-plaintext", "localhost:50051", "list"]);
            assert_eq!(interval_ms, 500);
            assert_eq!(timeout_s, 30);
        } else {
            panic!("expected ReadyCheck::Command");
        }
    }

    #[test]
    fn test_ready_check_stdout_deserialize() {
        let toml = r#"
name = "worker"
command = "python"
start_before_app = true
ready_check = { type = "stdout", pattern = "Server started on port \\d+" }
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        if let Some(ReadyCheck::Stdout { pattern, timeout_s }) = config.ready_check {
            assert_eq!(pattern, r"Server started on port \d+");
            assert_eq!(timeout_s, 30);
        } else {
            panic!("expected ReadyCheck::Stdout");
        }
    }

    #[test]
    fn test_ready_check_delay_deserialize() {
        let toml = r#"
name = "slow-service"
command = "start-service"
start_before_app = true
ready_check = { type = "delay", seconds = 10 }
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        if let Some(ReadyCheck::Delay { seconds }) = config.ready_check {
            assert_eq!(seconds, 10);
        } else {
            panic!("expected ReadyCheck::Delay");
        }
    }

    #[test]
    fn test_ready_check_delay_default_seconds() {
        let toml = r#"
name = "slow-service"
command = "start-service"
start_before_app = true
ready_check = { type = "delay" }
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        if let Some(ReadyCheck::Delay { seconds }) = config.ready_check {
            assert_eq!(seconds, 5);
        } else {
            panic!("expected ReadyCheck::Delay");
        }
    }

    // ─── Backward compatibility ───────────────────────────────────────────────

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

    // ─── CustomSourceConfig::validate new checks ──────────────────────────────

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
    fn test_validate_start_before_app_with_http_ready_check_ok() {
        let toml = r#"
name = "server"
command = "cargo"
start_before_app = true
ready_check = { type = "http", url = "http://localhost:8080/health" }
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert!(config.validate().is_ok());
    }

    // ─── ReadyCheck::validate error cases ────────────────────────────────────

    #[test]
    fn test_ready_check_http_validate_invalid_url() {
        let check = ReadyCheck::Http {
            url: "not-a-url".to_string(),
            interval_ms: 500,
            timeout_s: 30,
        };
        assert!(check.validate().is_err());
    }

    #[test]
    fn test_ready_check_http_validate_url_no_host() {
        let check = ReadyCheck::Http {
            url: "file:///path/to/file".to_string(),
            interval_ms: 500,
            timeout_s: 30,
        };
        assert!(check.validate().is_err());
    }

    #[test]
    fn test_ready_check_http_validate_valid_url() {
        let check = ReadyCheck::Http {
            url: "http://localhost:8080/health".to_string(),
            interval_ms: 500,
            timeout_s: 30,
        };
        assert!(check.validate().is_ok());
    }

    #[test]
    fn test_ready_check_tcp_validate_port_zero() {
        let check = ReadyCheck::Tcp {
            host: "localhost".to_string(),
            port: 0,
            interval_ms: 500,
            timeout_s: 30,
        };
        assert!(check.validate().is_err());
    }

    #[test]
    fn test_ready_check_tcp_validate_valid_port() {
        let check = ReadyCheck::Tcp {
            host: "localhost".to_string(),
            port: 5432,
            interval_ms: 500,
            timeout_s: 30,
        };
        assert!(check.validate().is_ok());
    }

    #[test]
    fn test_ready_check_command_validate_empty_command() {
        let check = ReadyCheck::Command {
            command: "  ".to_string(),
            args: vec![],
            interval_ms: 500,
            timeout_s: 30,
        };
        assert!(check.validate().is_err());
    }

    #[test]
    fn test_ready_check_command_validate_valid() {
        let check = ReadyCheck::Command {
            command: "pg_isready".to_string(),
            args: vec![],
            interval_ms: 500,
            timeout_s: 30,
        };
        assert!(check.validate().is_ok());
    }

    #[test]
    fn test_ready_check_stdout_validate_invalid_regex() {
        let check = ReadyCheck::Stdout {
            pattern: "[invalid regex".to_string(),
            timeout_s: 30,
        };
        assert!(check.validate().is_err());
    }

    #[test]
    fn test_ready_check_stdout_validate_valid_regex() {
        let check = ReadyCheck::Stdout {
            pattern: r"Server started on port \d+".to_string(),
            timeout_s: 30,
        };
        assert!(check.validate().is_ok());
    }

    #[test]
    fn test_ready_check_delay_validate_zero_seconds() {
        let check = ReadyCheck::Delay { seconds: 0 };
        assert!(check.validate().is_err());
    }

    #[test]
    fn test_ready_check_delay_validate_valid() {
        let check = ReadyCheck::Delay { seconds: 5 };
        assert!(check.validate().is_ok());
    }

    // ─── NativeLogsSettings helper methods ───────────────────────────────────

    #[test]
    fn test_has_pre_app_sources_false_when_none() {
        let settings = NativeLogsSettings {
            custom_sources: vec![CustomSourceConfig {
                name: "watcher".to_string(),
                command: "tail".to_string(),
                args: vec![],
                format: OutputFormat::Raw,
                working_dir: None,
                env: HashMap::new(),
                start_before_app: false,
                shared: false,
                ready_check: None,
            }],
            ..NativeLogsSettings::default()
        };
        assert!(!settings.has_pre_app_sources());
    }

    #[test]
    fn test_has_pre_app_sources_false_when_empty() {
        let settings = NativeLogsSettings::default();
        assert!(!settings.has_pre_app_sources());
    }

    #[test]
    fn test_has_pre_app_sources_true_when_present() {
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "post-app".to_string(),
                    command: "tail".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "pre-app".to_string(),
                    command: "server".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: true,
                    shared: false,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        assert!(settings.has_pre_app_sources());
    }

    #[test]
    fn test_pre_app_sources_iterator() {
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "pre".to_string(),
                    command: "server".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: true,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "post".to_string(),
                    command: "tail".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        let pre: Vec<_> = settings.pre_app_sources().collect();
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0].name, "pre");

        let post: Vec<_> = settings.post_app_sources().collect();
        assert_eq!(post.len(), 1);
        assert_eq!(post[0].name, "post");
    }

    // ─── shared field tests ───────────────────────────────────────────────────

    #[test]
    fn test_shared_field_defaults_to_false() {
        let toml = r#"
name = "my-source"
command = "adb"
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert!(
            !config.shared,
            "shared should default to false when omitted"
        );
    }

    #[test]
    fn test_shared_field_parses_true() {
        let toml = r#"
name = "my-source"
command = "adb"
shared = true
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert!(config.shared, "shared should be true when set to true");
    }

    #[test]
    fn test_shared_field_parses_false_explicit() {
        let toml = r#"
name = "my-source"
command = "adb"
shared = false
"#;
        let config: CustomSourceConfig = toml::from_str(toml).unwrap();
        assert!(
            !config.shared,
            "shared should be false when set explicitly to false"
        );
    }

    #[test]
    fn test_has_shared_sources_false_when_none_shared() {
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "source-a".to_string(),
                    command: "cmd".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "source-b".to_string(),
                    command: "cmd2".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: true,
                    shared: false,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        assert!(!settings.has_shared_sources());
    }

    #[test]
    fn test_has_shared_sources_false_when_empty() {
        let settings = NativeLogsSettings::default();
        assert!(!settings.has_shared_sources());
    }

    #[test]
    fn test_has_shared_sources_true_when_one_shared() {
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "per-session".to_string(),
                    command: "cmd".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "shared-source".to_string(),
                    command: "cmd2".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: true,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        assert!(settings.has_shared_sources());
    }

    #[test]
    fn test_shared_sources_iterator() {
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "per-session".to_string(),
                    command: "cmd".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "shared-one".to_string(),
                    command: "cmd2".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: true,
                    shared: true,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "shared-two".to_string(),
                    command: "cmd3".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: true,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        let shared: Vec<_> = settings.shared_sources().collect();
        assert_eq!(shared.len(), 2);
        assert_eq!(shared[0].name, "shared-one");
        assert_eq!(shared[1].name, "shared-two");
    }

    #[test]
    fn test_has_shared_pre_app_sources_false_when_empty() {
        let settings = NativeLogsSettings::default();
        assert!(!settings.has_shared_pre_app_sources());
    }

    #[test]
    fn test_has_shared_pre_app_sources_false_when_shared_but_not_pre_app() {
        let settings = NativeLogsSettings {
            custom_sources: vec![CustomSourceConfig {
                name: "shared-post-app".to_string(),
                command: "cmd".to_string(),
                args: vec![],
                format: OutputFormat::Raw,
                working_dir: None,
                env: HashMap::new(),
                start_before_app: false,
                shared: true,
                ready_check: None,
            }],
            ..NativeLogsSettings::default()
        };
        assert!(!settings.has_shared_pre_app_sources());
    }

    #[test]
    fn test_has_shared_pre_app_sources_false_when_pre_app_but_not_shared() {
        let settings = NativeLogsSettings {
            custom_sources: vec![CustomSourceConfig {
                name: "per-session-pre-app".to_string(),
                command: "cmd".to_string(),
                args: vec![],
                format: OutputFormat::Raw,
                working_dir: None,
                env: HashMap::new(),
                start_before_app: true,
                shared: false,
                ready_check: None,
            }],
            ..NativeLogsSettings::default()
        };
        assert!(!settings.has_shared_pre_app_sources());
    }

    #[test]
    fn test_has_shared_pre_app_sources_true_when_shared_and_pre_app() {
        let settings = NativeLogsSettings {
            custom_sources: vec![
                CustomSourceConfig {
                    name: "per-session-post-app".to_string(),
                    command: "cmd".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: false,
                    shared: false,
                    ready_check: None,
                },
                CustomSourceConfig {
                    name: "shared-pre-app".to_string(),
                    command: "server".to_string(),
                    args: vec![],
                    format: OutputFormat::Raw,
                    working_dir: None,
                    env: HashMap::new(),
                    start_before_app: true,
                    shared: true,
                    ready_check: None,
                },
            ],
            ..NativeLogsSettings::default()
        };
        assert!(settings.has_shared_pre_app_sources());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // FlutterSettings Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_flutter_settings_default() {
        let settings = FlutterSettings::default();
        assert!(settings.sdk_path.is_none());
    }

    #[test]
    fn test_settings_without_flutter_section() {
        let toml_str = r#"
[behavior]
auto_start = true

[ui]
show_timestamps = false
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(settings.flutter.sdk_path.is_none());
    }

    #[test]
    fn test_settings_with_flutter_sdk_path() {
        let toml_str = r#"
[flutter]
sdk_path = "/Users/me/flutter"
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(
            settings.flutter.sdk_path,
            Some(PathBuf::from("/Users/me/flutter"))
        );
    }

    #[test]
    fn test_settings_with_empty_flutter_section() {
        let toml_str = r#"
[flutter]
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(settings.flutter.sdk_path.is_none());
    }

    #[test]
    fn test_settings_roundtrip_serialization() {
        let mut settings = Settings::default();
        settings.flutter.sdk_path = Some(PathBuf::from("/opt/flutter"));

        let serialized = toml::to_string(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&serialized).unwrap();

        assert_eq!(
            deserialized.flutter.sdk_path,
            Some(PathBuf::from("/opt/flutter"))
        );
    }
}
