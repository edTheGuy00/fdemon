//! Stateful block tracking for Logger package blocks.

use fdemon_core::LogLevel;

/// Tracks state for Logger package block detection
///
/// Instead of backward-scanning on every block end (O(N*M)), this struct
/// tracks block state incrementally as lines arrive (O(1) per line).
#[derive(Debug, Clone)]
pub struct LogBlockState {
    /// Index where current block started (if any)
    pub(super) block_start: Option<usize>,
    /// Highest severity seen in current block
    pub(super) block_max_level: LogLevel,
}

impl Default for LogBlockState {
    fn default() -> Self {
        Self {
            block_start: None,
            block_max_level: LogLevel::Info,
        }
    }
}
