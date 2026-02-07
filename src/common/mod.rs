//! Common utilities shared across all modules

pub mod error;
pub mod logging;

/// Prelude for common imports used throughout the application
pub mod prelude {
    pub use super::error::{Error, Result, ResultExt};
    pub use tracing::{debug, error, info, instrument, trace, warn};
}
