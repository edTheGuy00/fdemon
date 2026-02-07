//! Logging configuration using tracing

use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::error::Result;

/// Initialize the logging subsystem
///
/// Logs are written to `~/.local/share/flutter-demon/logs/`
/// Log level is controlled by `FDEMON_LOG` environment variable.
///
/// # Examples
/// ```bash
/// FDEMON_LOG=debug cargo run
/// FDEMON_LOG=trace cargo run
/// ```
pub fn init() -> Result<()> {
    let log_dir = get_log_directory()?;
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "fdemon.log");

    // Default to info, allow override via FDEMON_LOG
    let env_filter = EnvFilter::try_from_env("FDEMON_LOG")
        .unwrap_or_else(|_| EnvFilter::new("flutter_demon=info,warn"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true)
                .with_timer(fmt::time::ChronoLocal::new(
                    "%Y-%m-%d %H:%M:%S%.3f".to_string(),
                )),
        )
        .init();

    tracing::info!("═══════════════════════════════════════════════════════");
    tracing::info!("Flutter Demon starting");
    tracing::info!("Log directory: {}", log_dir.display());
    tracing::info!("═══════════════════════════════════════════════════════");

    Ok(())
}

/// Get the log directory path
fn get_log_directory() -> Result<PathBuf> {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    Ok(base.join("flutter-demon").join("logs"))
}

/// Get the log file path for the current day
pub fn get_current_log_file() -> Result<PathBuf> {
    let dir = get_log_directory()?;
    Ok(dir.join("fdemon.log"))
}
