//! Configuration types for Flutter Demon
//!
//! Defines:
//! - `LaunchConfig` - A single launch configuration
//! - `Settings` - Global application settings
//! - Related sub-types and enums

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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

    /// Default panel when entering DevTools mode ("inspector", "layout", "performance")
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

    /// Auto-enable repaint rainbow on VM connect
    #[serde(default)]
    pub auto_repaint_rainbow: bool,

    /// Auto-enable performance overlay on VM connect
    #[serde(default)]
    pub auto_performance_overlay: bool,

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
            auto_repaint_rainbow: false,
            auto_performance_overlay: false,
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
            ParentIde::Neovim => "file", // Neovim doesn't have URL scheme
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
        assert!(!settings.auto_repaint_rainbow);
        assert!(!settings.auto_performance_overlay);
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
            auto_repaint_rainbow = true
            auto_performance_overlay = false

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
        assert!(settings.auto_repaint_rainbow);
        assert!(settings.logging.show_source_indicator);
        assert_eq!(settings.logging.dedupe_threshold_ms, 200);
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
}
