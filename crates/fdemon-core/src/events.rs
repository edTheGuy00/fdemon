//! Domain event definitions

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────
// Event Structs (moved from daemon/events.rs)
// ─────────────────────────────────────────────────────────

/// Connected event - sent when daemon is ready
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonConnected {
    pub version: String,
    pub pid: u32,
}

/// Log message from the daemon itself (not the app)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonLogMessage {
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub stack_trace: Option<String>,
}

/// App start event - when app begins launching
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStart {
    pub app_id: String,
    pub device_id: String,
    pub directory: String,
    #[serde(default)]
    pub launch_mode: Option<String>,
    pub supports_restart: bool,
}

/// App started event - when app is fully running
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStarted {
    pub app_id: String,
}

/// App log event - Flutter print() and debug output
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLog {
    pub app_id: String,
    pub log: String,
    #[serde(default)]
    pub error: bool,
    #[serde(default)]
    pub stack_trace: Option<String>,
}

/// Progress notification during build/reload
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProgress {
    pub app_id: String,
    pub id: String,
    #[serde(default)]
    pub progress_id: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub finished: bool,
}

/// App stop event
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStop {
    pub app_id: String,
    #[serde(default)]
    pub error: Option<String>,
}

/// Debug port information for DevTools
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDebugPort {
    pub app_id: String,
    pub port: u16,
    pub ws_uri: String,
}

/// Device added/removed event
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    #[serde(default)]
    pub emulator: bool,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub platform_type: Option<String>,
    #[serde(default)]
    pub ephemeral: bool,
}

// ─────────────────────────────────────────────────────────
// DaemonMessage Enum (moved from daemon/protocol.rs)
// ─────────────────────────────────────────────────────────

/// Fully typed daemon message
#[derive(Debug, Clone)]
pub enum DaemonMessage {
    // Connection
    DaemonConnected(DaemonConnected),
    DaemonLogMessage(DaemonLogMessage),

    // App lifecycle
    AppStart(AppStart),
    AppStarted(AppStarted),
    AppStop(AppStop),
    AppLog(AppLog),
    AppProgress(AppProgress),
    AppDebugPort(AppDebugPort),

    // Devices
    DeviceAdded(DeviceInfo),
    DeviceRemoved(DeviceInfo),

    // Responses
    Response {
        id: serde_json::Value,
        result: Option<serde_json::Value>,
        error: Option<serde_json::Value>,
    },

    /// DevTools server is reachable at the given base URL.
    ///
    /// Emitted when the Flutter daemon fires an `app.devTools` event (primary path,
    /// Flutter ≥ 1.22.0). The `base_url` field holds the raw base URL exactly as
    /// provided by DDS — e.g. `http://127.0.0.1:9100` or
    /// `http://127.0.0.1:59123/tbrR0DzW2j8=/devtools` for DDS-integrated DevTools.
    ///
    /// Callers construct the final browser URL by appending `?uri=<encoded_ws_uri>`:
    /// ```text
    /// format!("{}?uri={}", base_url, percent_encode(ws_uri))
    /// ```
    DevToolsServed {
        /// The app ID that triggered this DevTools availability notification.
        app_id: String,
        /// Base DevTools server URL, ready for use as the browser target (without
        /// the `?uri=` query parameter).
        base_url: String,
    },

    /// The Flutter daemon could not serve DevTools.
    ///
    /// Possible causes:
    /// - `devtools.serve` RPC returned a JSON-RPC error (e.g., `-32601 Method not found`
    ///   on Flutter < 1.22.0).
    /// - `devtools.serve` returned a success response but with `null` host/port fields,
    ///   indicating the DevTools bundle is unavailable.
    DevToolsServeFailed {
        /// Human-readable reason for the failure, suitable for display in a toast/log.
        reason: String,
    },

    // Fallback for unknown events
    UnknownEvent {
        event: String,
        params: serde_json::Value,
    },
}

// ─────────────────────────────────────────────────────────
// Pure Methods (moved from daemon/protocol.rs)
// ─────────────────────────────────────────────────────────

impl DaemonMessage {
    /// Get the app ID if this message relates to an app
    pub fn app_id(&self) -> Option<&str> {
        match self {
            DaemonMessage::AppStart(e) => Some(&e.app_id),
            DaemonMessage::AppStarted(e) => Some(&e.app_id),
            DaemonMessage::AppStop(e) => Some(&e.app_id),
            DaemonMessage::AppLog(e) => Some(&e.app_id),
            DaemonMessage::AppProgress(e) => Some(&e.app_id),
            DaemonMessage::AppDebugPort(e) => Some(&e.app_id),
            DaemonMessage::DevToolsServed { app_id, .. } => Some(app_id),
            _ => None,
        }
    }

    /// Check if this is an error message
    pub fn is_error(&self) -> bool {
        match self {
            DaemonMessage::AppLog(log) => log.error,
            DaemonMessage::AppStop(stop) => stop.error.is_some(),
            DaemonMessage::Response { error, .. } => error.is_some(),
            DaemonMessage::DevToolsServeFailed { .. } => true,
            _ => false,
        }
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        match self {
            DaemonMessage::DaemonConnected(c) => {
                format!("Daemon connected (v{})", c.version)
            }
            DaemonMessage::DaemonLogMessage(m) => {
                format!("[{}] {}", m.level, m.message)
            }
            DaemonMessage::AppStart(s) => {
                format!("App starting on {}", s.device_id)
            }
            DaemonMessage::AppStarted(_) => "App started".to_string(),
            DaemonMessage::AppStop(s) => {
                if let Some(err) = &s.error {
                    format!("App stopped: {}", err)
                } else {
                    "App stopped".to_string()
                }
            }
            DaemonMessage::AppLog(log) => log.log.clone(),
            DaemonMessage::AppProgress(p) => p
                .message
                .clone()
                .unwrap_or_else(|| "Progress...".to_string()),
            DaemonMessage::AppDebugPort(d) => {
                format!("DevTools at port {}", d.port)
            }
            DaemonMessage::DeviceAdded(d) => {
                format!("Device added: {} ({})", d.name, d.platform)
            }
            DaemonMessage::DeviceRemoved(d) => {
                format!("Device removed: {}", d.name)
            }
            DaemonMessage::Response { id, error, .. } => {
                if error.is_some() {
                    format!("Response #{}: error", id)
                } else {
                    format!("Response #{}: ok", id)
                }
            }
            DaemonMessage::DevToolsServed { base_url, .. } => {
                format!("DevTools available at {}", base_url)
            }
            DaemonMessage::DevToolsServeFailed { reason } => {
                format!("DevTools unavailable: {}", reason)
            }
            DaemonMessage::UnknownEvent { event, .. } => {
                format!("Event: {}", event)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────
// DaemonEvent
// ─────────────────────────────────────────────────────────

/// Events from the Flutter daemon process
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    /// Raw stdout line from daemon (JSON-RPC wrapped)
    Stdout(String),

    /// Parsed daemon message
    Message(DaemonMessage),

    /// Stderr output (usually errors/warnings)
    Stderr(String),

    /// Daemon process has exited
    Exited { code: Option<i32> },

    /// Process spawn failed
    SpawnFailed { reason: String },
}
