//! Core domain types - pure business logic with no external dependencies

pub mod ansi;
pub mod discovery;
pub mod events;
pub mod stack_trace;
pub mod types;

pub use ansi::{contains_ansi_codes, strip_ansi_codes};
pub use discovery::{
    discover_flutter_projects, get_project_name, get_project_type, has_flutter_dependency,
    has_platform_directories, is_flutter_plugin, is_runnable_flutter_project, DiscoveryResult,
    ProjectType, SkippedProject, DEFAULT_MAX_DEPTH,
};
pub use events::*;
pub use stack_trace::{
    detect_format, is_package_path, is_project_path, ParsedStackTrace, StackFrame,
    StackTraceFormat, ASYNC_GAP_REGEX, DART_VM_FRAME_NO_COL_REGEX, DART_VM_FRAME_REGEX,
    FRIENDLY_FRAME_REGEX, PACKAGE_PATH_REGEX,
};
pub use types::*;
