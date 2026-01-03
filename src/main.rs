//! Flutter Demon - A high-performance TUI for Flutter development
//!
//! This is the binary entry point. All logic lives in the library.

use std::path::PathBuf;

use flutter_demon::common::prelude::*;
use flutter_demon::core::{
    discover_flutter_projects, get_project_type, is_runnable_flutter_project, ProjectType,
    DEFAULT_MAX_DEPTH,
};
use flutter_demon::tui::{select_project, SelectionResult};

#[tokio::main]
async fn main() -> Result<()> {
    // Get base path from args or use current directory
    let base_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Step 1: Check if base_path is directly a runnable Flutter project
    if is_runnable_flutter_project(&base_path) {
        return flutter_demon::run_with_project(&base_path).await;
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
            flutter_demon::run_with_project(project).await
        }
        _ => {
            // Multiple runnable projects found - show selector
            match select_project(&discovery.projects, &discovery.searched_from)? {
                SelectionResult::Selected(project) => {
                    flutter_demon::run_with_project(&project).await
                }
                SelectionResult::Cancelled => {
                    eprintln!("Selection cancelled.");
                    Ok(())
                }
            }
        }
    }
}
