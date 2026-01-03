//! Core domain types - pure business logic with no external dependencies

pub mod discovery;
pub mod events;
pub mod types;

pub use discovery::{
    discover_flutter_projects, get_project_type, has_flutter_dependency, has_platform_directories,
    is_flutter_plugin, is_runnable_flutter_project, DiscoveryResult, ProjectType, SkippedProject,
    DEFAULT_MAX_DEPTH,
};
pub use events::*;
pub use types::*;
