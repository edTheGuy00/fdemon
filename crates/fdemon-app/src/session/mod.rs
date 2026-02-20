//! Per-instance session state for a running Flutter app

mod block_state;
mod collapse;
mod handle;
pub(crate) mod log_batcher;
pub(crate) mod performance;
#[allow(clippy::module_inception)]
mod session;

#[cfg(test)]
mod tests;

// Re-export all public types at the session:: level
pub use block_state::LogBlockState;
pub use collapse::CollapseState;
pub use handle::SessionHandle;
pub use log_batcher::LogBatcher;
pub(crate) use performance::STATS_RECOMPUTE_INTERVAL;
pub use performance::{AllocationSortColumn, PerformanceState, DEFAULT_MEMORY_SAMPLE_SIZE};
pub use session::Session;

// SessionId and next_session_id live here in mod.rs
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for a session
pub type SessionId = u64;

static SESSION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a new unique session ID
pub fn next_session_id() -> SessionId {
    SESSION_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}
