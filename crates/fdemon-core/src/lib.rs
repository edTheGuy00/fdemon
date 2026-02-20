//! # fdemon-core - Core Domain Types
//!
//! Foundation crate for Flutter Demon. Provides domain types, error handling,
//! event definitions, project discovery, and stack trace parsing.
//!
//! This crate has **zero internal dependencies** -- it only depends on external
//! crates (serde, chrono, thiserror, regex, tracing).
//!
//! ## Public API
//!
//! ### Domain Types (`types`)
//! - [`AppPhase`] - Application lifecycle phase (Initializing, Running, Reloading, etc.)
//! - [`LogEntry`] - A single log line with level, source, and timestamp
//! - [`LogLevel`] - Log severity (Debug, Info, Warning, Error)
//! - [`LogSource`] - Origin of a log entry (App, Flutter, Daemon)
//! - [`FilterState`], [`SearchState`] - Log filtering and search state
//!
//! ### Events (`events`)
//! - [`DaemonMessage`] - Parsed messages from Flutter's `--machine` JSON-RPC output
//! - [`DaemonEvent`] - Wrapper enum for daemon stdout/stderr/exit events
//!
//! ### Error Handling (`error`)
//! - [`Error`] - Custom error enum with `fatal` vs `recoverable` classification
//! - [`Result`] - Type alias for `std::result::Result<T, Error>`
//! - [`ResultExt`] - Extension trait for adding error context
//!
//! ### Project Discovery (`discovery`)
//! - [`is_runnable_flutter_project()`] - Check if a directory is a runnable Flutter project
//! - [`discover_flutter_projects()`] - Find Flutter projects in subdirectories
//! - [`get_project_type()`] - Determine project type (app, plugin, package)
//!
//! ### Stack Traces (`stack_trace`)
//! - [`ParsedStackTrace`] - Parsed and formatted stack trace
//! - [`StackFrame`] - Individual stack frame with file, line, column
//!
//! ### Exception Blocks (`exception_block`)
//! - [`ExceptionBlock`] - Parsed Flutter framework exception block
//! - [`ExceptionBlockParser`] - Line-by-line state machine parser for exception blocks
//! - [`FeedResult`] - Result of feeding a line to the parser
//!
//! ## Prelude
//!
//! Import commonly used types with:
//! ```rust
//! use fdemon_core::prelude::*;
//! ```

pub mod ansi;
pub mod discovery;
pub mod error;
pub mod events;
pub mod exception_block;
pub mod logging;
pub mod performance;
pub mod stack_trace;
pub mod types;
pub mod widget_tree;

/// Prelude for common imports used throughout all Flutter Demon crates
pub mod prelude {
    pub use super::error::{Error, Result, ResultExt};
    pub use tracing::{debug, error, info, instrument, trace, warn};
}

// Re-export commonly used types at crate root for convenience
pub use ansi::{contains_ansi_codes, contains_word, strip_ansi_codes};
pub use discovery::{
    discover_entry_points, discover_flutter_projects, get_project_name, get_project_type,
    is_runnable_flutter_project, DiscoveryResult, ProjectType, SkippedProject, DEFAULT_MAX_DEPTH,
};
pub use error::{Error, Result, ResultExt};
pub use events::{
    AppDebugPort, AppLog, AppProgress, AppStart, AppStarted, AppStop, DaemonConnected, DaemonEvent,
    DaemonLogMessage, DaemonMessage, DeviceInfo,
};
pub use exception_block::{ExceptionBlock, ExceptionBlockParser, FeedResult};
pub use performance::{
    AllocationProfile, ClassHeapStats, FrameTiming, GcEvent, MemoryUsage, PerformanceStats,
    RingBuffer, FRAME_BUDGET_120FPS_MICROS, FRAME_BUDGET_60FPS_MICROS,
};
pub use stack_trace::{
    detect_format, is_package_path, is_project_path, ParsedStackTrace, StackFrame, StackTraceFormat,
};
pub use types::{
    AppPhase, BootableDevice, DeviceState, FilterState, LogEntry, LogLevel, LogLevelFilter,
    LogSource, LogSourceFilter, Platform, SearchMatch, SearchState,
};
pub use widget_tree::{
    BoxConstraints, CreationLocation, DiagnosticLevel, DiagnosticsNode, EdgeInsets, LayoutInfo,
    WidgetSize,
};
