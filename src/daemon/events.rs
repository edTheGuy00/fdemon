//! Typed event structs for Flutter daemon JSON-RPC messages

use serde::{Deserialize, Serialize};

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
