//! Logging configuration using tracing

use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::error::Result;

/// Initialize the logging subsystem.
///
/// When `log_dir_override` is `Some`, logs are written to a per-session file in
/// that directory (`fdemon-<timestamp>.log`). When `None`, logs are written to
/// the default rolling daily file in `~/.local/share/flutter-demon/logs/`.
///
/// Returns `Some(log_path)` when a custom log dir is used (so the caller can
/// print it), or `None` for the default rolling log.
///
/// Log level is controlled by `FDEMON_LOG` environment variable.
///
/// # Examples
/// ```bash
/// FDEMON_LOG=debug cargo run
/// fdemon --log-dir ./tmp example/app3
/// FDEMON_LOG=fdemon_dap=debug fdemon --log-dir ./tmp example/app3
/// ```
pub fn init(log_dir_override: Option<PathBuf>) -> Result<Option<PathBuf>> {
    match log_dir_override {
        Some(dir) => init_session_log(dir),
        None => {
            init_rolling_log()?;
            Ok(None)
        }
    }
}

/// Default rolling daily log to `~/.local/share/flutter-demon/logs/`.
fn init_rolling_log() -> Result<()> {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};

    let log_dir = get_log_directory()?;
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "fdemon.log");

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

    tracing::info!("Log directory: {}", log_dir.display());
    Ok(())
}

/// Per-session log file in a custom directory.
///
/// Uses a more permissive default filter (`info` for all crates) so that
/// DAP adapter logs are visible without needing `FDEMON_LOG`.
fn init_session_log(log_dir: PathBuf) -> Result<Option<PathBuf>> {
    std::fs::create_dir_all(&log_dir)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let log_path = log_dir.join(format!("fdemon-{now}.log"));
    let log_file = std::fs::File::create(&log_path)?;

    // Default to info for all crates when using --log-dir (includes fdemon_dap).
    let env_filter = EnvFilter::try_from_env("FDEMON_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_writer(log_file)
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

    tracing::info!("Session log: {}", log_path.display());
    Ok(Some(log_path))
}

/// Get the default log directory path.
fn get_log_directory() -> Result<PathBuf> {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    Ok(base.join("flutter-demon").join("logs"))
}

/// Get the log file path for the current day.
pub fn get_current_log_file() -> Result<PathBuf> {
    let dir = get_log_directory()?;
    Ok(dir.join("fdemon.log"))
}
