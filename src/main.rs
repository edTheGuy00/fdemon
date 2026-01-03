//! Flutter Demon - A high-performance TUI for Flutter development
//!
//! This is the binary entry point. All logic lives in the library.

use std::path::PathBuf;

use flutter_demon::common::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Get project path from args or use current directory
    let project_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Check if this looks like a Flutter project
    if project_path.join("pubspec.yaml").exists() {
        flutter_demon::run_with_project(&project_path).await
    } else {
        // Run in demo mode if no Flutter project found
        eprintln!("No pubspec.yaml found in {}", project_path.display());
        eprintln!("Running in demo mode...");
        flutter_demon::run().await
    }
}
