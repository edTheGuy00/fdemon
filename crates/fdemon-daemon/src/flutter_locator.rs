//! Flutter SDK locator helper
//!
//! This module provides functionality to locate the Flutter SDK executable
//! across different installation methods (official, puro, fvm, etc.)

use std::env;
use std::path::{Path, PathBuf};

/// Environment variable name for Flutter SDK root
const FLUTTER_ROOT_ENV: &str = "FLUTTER_ROOT";

/// Find the Flutter executable path
///
/// This function tries multiple strategies to find the Flutter SDK:
/// 1. Check FLUTTER_ROOT environment variable
/// 2. Check common installation paths
/// 3. Fall back to system PATH
///
/// On Windows, it checks for .bat files since Rust's Command doesn't
/// automatically resolve batch files like the shell does.
pub fn find_flutter_executable() -> Option<FlutterExecutable> {
    // Strategy 1: Check FLUTTER_ROOT
    if let Some(flutter_root) = env::var(FLUTTER_ROOT_ENV).ok() {
        let flutter_path = PathBuf::from(&flutter_root).join("bin");
        if let Some(exe) = find_flutter_in_dir(&flutter_path) {
            return Some(exe);
        }
    }

    // Strategy 2: Check PATH for flutter executable
    if let Ok(path_var) = env::var("PATH") {
        for path in path_var.split(if cfg!(windows) { ';' } else { ':' }) {
            let dir = PathBuf::from(path);
            if let Some(exe) = find_flutter_in_dir(&dir) {
                return Some(exe);
            }
        }
    }

    None
}

/// Find flutter executable in a specific directory
///
/// On Windows, checks for flutter.bat first, then flutter.exe
/// On Unix, checks for flutter script
fn find_flutter_in_dir(dir: &Path) -> Option<FlutterExecutable> {
    if cfg!(windows) {
        // On Windows, try flutter.bat first (for puro, fvm, etc.)
        let bat_path = dir.join("flutter.bat");
        if bat_path.exists() {
            return Some(FlutterExecutable::WindowsBatch(bat_path));
        }

        // Then try flutter.exe
        let exe_path = dir.join("flutter.exe");
        if exe_path.exists() {
            return Some(FlutterExecutable::Direct(exe_path));
        }
    } else {
        // On Unix, just check for the flutter script
        let script_path = dir.join("flutter");
        if script_path.exists() {
            return Some(FlutterExecutable::Direct(script_path));
        }
    }

    None
}

/// Represents a found Flutter executable
#[derive(Debug, Clone)]
pub enum FlutterExecutable {
    /// Direct executable (flutter on Unix, flutter.exe on Windows)
    Direct(PathBuf),
    /// Windows batch file (flutter.bat)
    WindowsBatch(PathBuf),
}

impl FlutterExecutable {
    /// Get the path to the executable
    pub fn path(&self) -> &Path {
        match self {
            FlutterExecutable::Direct(p) => p,
            FlutterExecutable::WindowsBatch(p) => p,
        }
    }

    /// Check if this is a Windows batch file
    pub fn is_windows_batch(&self) -> bool {
        matches!(self, FlutterExecutable::WindowsBatch(_))
    }

    /// Convert to a command that can be executed
    ///
    /// On Windows batch files, this returns the cmd.exe path with /c argument
    pub fn to_command(&self) -> (String, Vec<String>) {
        match self {
            FlutterExecutable::Direct(path) => (path.to_string_lossy().to_string(), vec![]),
            FlutterExecutable::WindowsBatch(path) => {
                // For batch files on Windows, we need to use cmd /c
                (
                    "cmd".to_string(),
                    vec!["/c".to_string(), path.to_string_lossy().to_string()],
                )
            }
        }
    }
}

/// Get the Flutter SDK root directory from the executable path
pub fn get_flutter_root(exe: &FlutterExecutable) -> Option<PathBuf> {
    exe.path()
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flutter_executable_direct() {
        let exe = FlutterExecutable::Direct(PathBuf::from("/flutter/bin/flutter"));
        assert!(!exe.is_windows_batch());
        let (cmd, args) = exe.to_command();
        assert_eq!(cmd, "/flutter/bin/flutter");
        assert!(args.is_empty());
    }

    #[test]
    fn test_flutter_executable_windows_batch() {
        let exe = FlutterExecutable::WindowsBatch(PathBuf::from("C:\\flutter\\bin\\flutter.bat"));
        assert!(exe.is_windows_batch());
        let (cmd, args) = exe.to_command();
        assert_eq!(cmd, "cmd");
        assert_eq!(args, vec!["/c", "C:\\flutter\\bin\\flutter.bat"]);
    }

    #[test]
    fn test_get_flutter_root() {
        let exe = FlutterExecutable::Direct(PathBuf::from("/home/user/flutter/bin/flutter"));
        let root = get_flutter_root(&exe);
        assert_eq!(root, Some(PathBuf::from("/home/user/flutter")));
    }
}
