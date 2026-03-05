//! Flutter Demon - A high-performance TUI for Flutter development
//!
//! This is the binary entry point.

mod dap_stdio;
mod headless;
mod tui;

use std::path::PathBuf;

use clap::Parser;
use fdemon_core::prelude::*;
use fdemon_core::{
    discover_flutter_projects, get_project_type, is_runnable_flutter_project, ProjectType,
    DEFAULT_MAX_DEPTH,
};
use fdemon_tui::{select_project, SelectionResult};

/// Flutter Demon - A high-performance TUI for Flutter development
#[derive(Parser, Debug)]
#[command(name = "fdemon", version)]
#[command(about = "A high-performance TUI for Flutter development", long_about = None)]
struct Args {
    /// Path to Flutter project
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Run in headless mode (JSON output, no TUI)
    #[arg(long)]
    headless: bool,

    /// Start the DAP server on a specific port (implies DAP enabled).
    ///
    /// Use 0 to let the OS assign an ephemeral port.
    /// In headless mode the assigned port is printed as JSON: {"event":"dap_server_started","port":54321,"timestamp":...}
    #[arg(long, value_name = "PORT")]
    dap_port: Option<u16>,

    /// Run as a DAP adapter over stdin/stdout (for IDE integration).
    ///
    /// When this flag is set, fdemon acts as a DAP adapter subprocess:
    /// - The TUI is not started (stdin/stdout are used for the DAP wire protocol).
    /// - All tracing/logging output is written to stderr.
    /// - The process exits when the DAP client disconnects.
    ///
    /// This is the preferred transport for Zed, Helix, and nvim-dap. Example
    /// Zed configuration:
    ///   { "adapter": "fdemon", "command": "fdemon", "args": ["--dap-stdio"] }
    ///
    /// Cannot be combined with --dap-port (mutually exclusive transports).
    #[arg(long, conflicts_with = "dap_port")]
    dap_stdio: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize error handling (must happen once at binary startup)
    color_eyre::install().map_err(|e| Error::terminal(e.to_string()))?;

    // Initialize logging (to file, since TUI owns stdout)
    fdemon_core::logging::init()?;

    info!("═══════════════════════════════════════════════════════");
    info!("Flutter Demon starting");
    info!("═══════════════════════════════════════════════════════");

    let args = Args::parse();

    // --dap-stdio: run as a DAP adapter subprocess over stdin/stdout.
    // This mode does not require a Flutter project path and must not start the TUI.
    // All tracing output is already going to a file (fdemon_core::logging::init above),
    // so stdout is clean for the DAP wire protocol.
    if args.dap_stdio {
        return dap_stdio::runner::run_dap_stdio().await;
    }

    // Get base path from args or use current directory
    let base_path = args
        .path
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Step 1: Check if base_path is directly a runnable Flutter project
    if is_runnable_flutter_project(&base_path) {
        info!("Project path: {}", base_path.display());
        return if args.headless {
            headless::runner::run_headless(&base_path, args.dap_port).await
        } else {
            tui::runner::run_with_project_and_dap(&base_path, args.dap_port).await
        };
    }

    // Step 2: If base_path has pubspec but isn't runnable, explain why
    if base_path.join("pubspec.yaml").exists() {
        match get_project_type(&base_path) {
            Some(ProjectType::Plugin) => {
                eprintln!("📦 Detected Flutter plugin at: {}", base_path.display());
                eprintln!("   Plugins cannot be run directly. Searching for runnable examples...");
                eprintln!();
            }
            Some(ProjectType::FlutterPackage) => {
                eprintln!("📦 Detected Flutter package at: {}", base_path.display());
                eprintln!("   Package has no platform directories (android/, ios/, etc.).");
                eprintln!("   Searching for runnable projects...");
                eprintln!();
            }
            Some(ProjectType::DartPackage) => {
                eprintln!("📦 Detected Dart package at: {}", base_path.display());
                eprintln!("   Dart-only packages cannot be run with flutter run.");
                eprintln!("   Searching for Flutter projects...");
                eprintln!();
            }
            _ => {}
        }
    }

    // Step 3: Discover runnable Flutter projects in subdirectories
    let discovery = discover_flutter_projects(&base_path, DEFAULT_MAX_DEPTH);

    // Log skipped projects for debugging (only if there are some and we found nothing)
    if !discovery.skipped.is_empty() && discovery.projects.is_empty() {
        for skipped in &discovery.skipped {
            eprintln!(
                "   Skipped {:?}: {} ({})",
                skipped.project_type,
                skipped.path.display(),
                skipped.reason
            );
        }
        eprintln!();
    }

    match discovery.projects.len() {
        0 => {
            // No runnable projects found - show helpful error
            eprintln!(
                "❌ No runnable Flutter projects found in: {}",
                base_path.display()
            );
            eprintln!("   Searched {} levels deep.", discovery.max_depth);
            eprintln!();
            eprintln!("A runnable Flutter project must have:");
            eprintln!("  • pubspec.yaml with 'sdk: flutter' dependency");
            eprintln!("  • At least one platform directory (android/, ios/, macos/, web/, linux/, windows/)");
            eprintln!("  • NOT be a plugin (no 'flutter: plugin:' section)");
            eprintln!();
            eprintln!("Hint: Run flutter-demon from a Flutter app directory,");
            eprintln!("      or pass the project path as an argument:");
            eprintln!("      fdemon /path/to/flutter/app");
            std::process::exit(1);
        }
        1 => {
            // Exactly one runnable project found - auto-select
            let project = &discovery.projects[0];
            eprintln!("✅ Found Flutter project: {}", project.display());
            info!("Project path: {}", project.display());
            if args.headless {
                headless::runner::run_headless(project, args.dap_port).await
            } else {
                tui::runner::run_with_project_and_dap(project, args.dap_port).await
            }
        }
        _ => {
            // Multiple runnable projects found - show selector
            if args.headless {
                // In headless mode, we can't show a selector, so just use the first project
                let project = &discovery.projects[0];
                eprintln!(
                    "Multiple projects found, using first: {}",
                    project.display()
                );
                info!("Project path: {}", project.display());
                headless::runner::run_headless(project, args.dap_port).await
            } else {
                match select_project(&discovery.projects, &discovery.searched_from)? {
                    SelectionResult::Selected(project) => {
                        info!("Project path: {}", project.display());
                        tui::runner::run_with_project_and_dap(&project, args.dap_port).await
                    }
                    SelectionResult::Cancelled => {
                        eprintln!("Selection cancelled.");
                        Ok(())
                    }
                }
            }
        }
    }
}
