//! `fdemon doctor` subcommand — read-only toolchain diagnostics.
//!
//! Runs [`fdemon_daemon::toolchain::run_preflight`] against the specified
//! project directory (or the current working directory), prints a structured
//! component report followed by the captured `flutter doctor -v` lines, and
//! exits with code 0 when all components are [`ComponentStatus::Ok`], or 1
//! otherwise.
//!
//! This subcommand never starts the TUI or the Engine — it is a pure
//! diagnostic tool intended for CI pipelines and manual toolchain debugging.

use std::path::PathBuf;
use std::process::ExitCode;

use fdemon_daemon::toolchain::{run_preflight, ComponentStatus};

/// Run the `fdemon doctor` diagnostics subcommand.
///
/// # Arguments
///
/// * `cwd` — The project directory to pass to the SDK locator. Typically the
///   current working directory or an explicitly-provided path.
/// * `explicit_sdk` — An optional Flutter SDK path taken from
///   `.fdemon/config.toml` `[flutter] sdk_path`, if any was configured for
///   the project.
///
/// # Returns
///
/// [`ExitCode::SUCCESS`] (0) when every component is `Ok`; [`ExitCode`] 1
/// otherwise.
pub async fn run_doctor(cwd: PathBuf, explicit_sdk: Option<PathBuf>) -> ExitCode {
    // Warn the user that preflight can take a while before blocking.
    eprintln!("Running toolchain checks…");

    let report = run_preflight(&cwd, explicit_sdk.as_deref()).await;

    let mut all_ok = true;
    for c in &report.components {
        if c.status != ComponentStatus::Ok {
            all_ok = false;
        }
        println!("[{:>4}] {} — {}", c.status, c.kind, c.detail);
    }

    if let Some(lines) = &report.doctor {
        println!("\nflutter doctor:");
        for l in lines {
            println!("  {}", l.text);
        }
    }

    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
