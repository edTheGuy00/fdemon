//! fdemon-core - Core domain types for Flutter Demon
//!
//! This crate provides the foundational types shared across all Flutter Demon
//! crates: error handling, domain types, event definitions, and project discovery.

pub mod ansi;
pub mod discovery;
pub mod error;
pub mod events;
pub mod logging;
pub mod stack_trace;
pub mod types;

/// Prelude for common imports used throughout all Flutter Demon crates
pub mod prelude {
    pub use super::error::{Error, Result, ResultExt};
    pub use tracing::{debug, error, info, instrument, trace, warn};
}

// Re-export commonly used types at crate root for convenience
pub use ansi::{contains_ansi_codes, contains_word, strip_ansi_codes};
pub use discovery::{
    discover_entry_points, discover_flutter_projects, get_project_name, get_project_type,
    has_flutter_dependency, has_main_function, has_main_function_in_content,
    has_platform_directories, is_flutter_plugin, is_runnable_flutter_project, DiscoveryResult,
    ProjectType, SkippedProject, DEFAULT_MAX_DEPTH,
};
pub use error::{Error, Result, ResultExt};
pub use events::{
    AppDebugPort, AppLog, AppProgress, AppStart, AppStarted, AppStop, DaemonConnected, DaemonEvent,
    DaemonLogMessage, DaemonMessage, DeviceInfo,
};
pub use stack_trace::{
    detect_format, is_package_path, is_project_path, ParsedStackTrace, StackFrame,
    StackTraceFormat, ASYNC_GAP_REGEX, DART_VM_FRAME_NO_COL_REGEX, DART_VM_FRAME_REGEX,
    FRIENDLY_FRAME_REGEX, PACKAGE_PATH_REGEX,
};
pub use types::{
    AppPhase, BootableDevice, DeviceState, FilterState, LogEntry, LogLevel, LogLevelFilter,
    LogSource, LogSourceFilter, Platform, SearchMatch, SearchState,
};
