//! # Flutter SDK Types
//!
//! Core type definitions for representing a resolved Flutter SDK,
//! how it was discovered, and how to invoke the flutter binary.

use std::path::{Path, PathBuf};

use fdemon_core::prelude::*;

/// How the Flutter SDK was discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkSource {
    /// User-specified path in config.toml `[flutter] sdk_path`
    ExplicitConfig,
    /// `FLUTTER_ROOT` environment variable
    EnvironmentVariable,
    /// FVM version manager (.fvmrc or .fvm/fvm_config.json)
    Fvm { version: String },
    /// Puro version manager (.puro.json)
    Puro { env: String },
    /// asdf version manager (.tool-versions)
    Asdf { version: String },
    /// mise version manager (.mise.toml)
    Mise { version: String },
    /// proto version manager (.prototools)
    Proto { version: String },
    /// flutter_wrapper (flutterw + .flutter/ directory)
    FlutterWrapper,
    /// System PATH lookup (which/where flutter, symlinks resolved)
    SystemPath,
    /// Flutter binary found on system PATH, but SDK could not be fully resolved.
    /// The executable path is usable but version/channel may be unknown.
    PathInferred,
}

impl std::fmt::Display for SdkSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExplicitConfig => write!(f, "config.toml"),
            Self::EnvironmentVariable => write!(f, "FLUTTER_ROOT"),
            Self::Fvm { version } => write!(f, "FVM ({version})"),
            Self::Puro { env } => write!(f, "Puro ({env})"),
            Self::Asdf { version } => write!(f, "asdf ({version})"),
            Self::Mise { version } => write!(f, "mise ({version})"),
            Self::Proto { version } => write!(f, "proto ({version})"),
            Self::FlutterWrapper => write!(f, "flutter_wrapper"),
            Self::SystemPath => write!(f, "system PATH"),
            Self::PathInferred => write!(f, "system PATH (inferred)"),
        }
    }
}

/// How to invoke the Flutter binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlutterExecutable {
    /// Unix shell script or direct executable — invoke directly
    Direct(PathBuf),
    /// Windows .bat file — requires `cmd /c` wrapper
    WindowsBatch(PathBuf),
}

impl FlutterExecutable {
    /// Returns the path to the flutter executable.
    pub fn path(&self) -> &Path {
        match self {
            Self::Direct(p) | Self::WindowsBatch(p) => p.as_path(),
        }
    }

    /// Configures a [`tokio::process::Command`] for this executable type.
    ///
    /// - `Direct`: `Command::new(path)` — invoked directly
    /// - `WindowsBatch`: `Command::new("cmd").args(["/c", path])` — requires cmd wrapper
    pub fn command(&self) -> tokio::process::Command {
        match self {
            Self::Direct(path) => tokio::process::Command::new(path),
            Self::WindowsBatch(path) => {
                let mut cmd = tokio::process::Command::new("cmd");
                cmd.args(["/c", &*path.to_string_lossy()]);
                cmd
            }
        }
    }
}

/// A resolved Flutter SDK with metadata.
#[derive(Debug, Clone)]
pub struct FlutterSdk {
    /// Root directory of the Flutter SDK (e.g., ~/fvm/versions/3.19.0/)
    pub root: PathBuf,
    /// Path to the flutter executable (bin/flutter or bin/flutter.bat)
    pub executable: FlutterExecutable,
    /// How this SDK was discovered
    pub source: SdkSource,
    /// Flutter version string (from VERSION file)
    pub version: String,
    /// Current channel (stable, beta, main) if detectable
    pub channel: Option<String>,
}

/// Validates that a directory contains a complete Flutter SDK.
///
/// Checks:
/// - `<root>/bin/flutter` (or `flutter.bat` on Windows) exists
/// - `<root>/VERSION` file is readable
/// - `<root>/bin/cache/dart-sdk/` directory exists
///
/// Returns the validated [`FlutterExecutable`] on success.
pub fn validate_sdk_path(root: &Path) -> Result<FlutterExecutable> {
    // Check for the flutter executable (platform-specific)
    #[cfg(target_os = "windows")]
    let (flutter_bin, executable_ctor): (PathBuf, fn(PathBuf) -> FlutterExecutable) = (
        root.join("bin").join("flutter.bat"),
        FlutterExecutable::WindowsBatch,
    );

    #[cfg(not(target_os = "windows"))]
    let (flutter_bin, executable_ctor): (PathBuf, fn(PathBuf) -> FlutterExecutable) =
        (root.join("bin").join("flutter"), FlutterExecutable::Direct);

    if !flutter_bin.is_file() {
        return Err(Error::flutter_sdk_invalid(
            root,
            format!("flutter binary not found at {}", flutter_bin.display()),
        ));
    }

    // Check VERSION file is readable
    let version_file = root.join("VERSION");
    if !version_file.is_file() {
        return Err(Error::flutter_sdk_invalid(
            root,
            format!("VERSION file not found at {}", version_file.display()),
        ));
    }

    // Check Dart SDK directory exists (non-fatal — may be absent on fresh installs
    // before first `flutter run` or `flutter doctor` populates the cache)
    let dart_sdk = root.join("bin").join("cache").join("dart-sdk");
    if !dart_sdk.is_dir() {
        tracing::debug!(
            "Dart SDK cache not yet populated at {} (expected on fresh installs)",
            dart_sdk.display()
        );
    }

    Ok(executable_ctor(flutter_bin))
}

/// Lenient variant of [`validate_sdk_path`] that does NOT require a VERSION file.
///
/// Checks only that `<root>/bin/flutter` (or `flutter.bat` on Windows) exists.
/// Used by the lenient PATH fallback (strategy 11) where the VERSION file may be
/// absent (Homebrew shims, snap installs, etc.).
///
/// Returns the validated [`FlutterExecutable`] on success.
pub fn validate_sdk_path_lenient(root: &Path) -> Result<FlutterExecutable> {
    #[cfg(target_os = "windows")]
    let (flutter_bin, executable_ctor): (PathBuf, fn(PathBuf) -> FlutterExecutable) = (
        root.join("bin").join("flutter.bat"),
        FlutterExecutable::WindowsBatch,
    );

    #[cfg(not(target_os = "windows"))]
    let (flutter_bin, executable_ctor): (PathBuf, fn(PathBuf) -> FlutterExecutable) =
        (root.join("bin").join("flutter"), FlutterExecutable::Direct);

    if !flutter_bin.is_file() {
        return Err(Error::flutter_sdk_invalid(
            root,
            format!("flutter binary not found at {}", flutter_bin.display()),
        ));
    }

    Ok(executable_ctor(flutter_bin))
}

/// Reads the Flutter version string from `<root>/VERSION`.
///
/// The VERSION file typically contains a version like `3.19.0\n`.
/// This function reads and trims the content.
pub fn read_version_file(root: &Path) -> Result<String> {
    let version_file = root.join("VERSION");
    let content = std::fs::read_to_string(&version_file).map_err(|e| {
        Error::flutter_sdk_invalid(
            root,
            format!(
                "failed to read VERSION file at {}: {}",
                version_file.display(),
                e
            ),
        )
    })?;
    Ok(content.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_sdk_path_valid() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // Create expected structure
        fs::create_dir_all(root.join("bin/cache/dart-sdk")).unwrap();
        fs::write(root.join("bin/flutter"), "#!/bin/sh").unwrap();
        fs::write(root.join("VERSION"), "3.19.0").unwrap();

        let result = validate_sdk_path(root);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_sdk_path_missing_flutter_binary() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("bin/cache/dart-sdk")).unwrap();
        fs::write(root.join("VERSION"), "3.19.0").unwrap();
        // No bin/flutter

        let result = validate_sdk_path(root);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_sdk_path_missing_version_file() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("bin/cache/dart-sdk")).unwrap();
        fs::write(root.join("bin/flutter"), "#!/bin/sh").unwrap();
        // No VERSION file

        let result = validate_sdk_path(root);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_sdk_path_missing_dart_sdk_still_valid() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("bin/flutter"), "#!/bin/sh").unwrap();
        fs::write(root.join("VERSION"), "3.19.0").unwrap();
        // No bin/cache/dart-sdk/ — should still succeed (fresh install)

        let result = validate_sdk_path(root);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_version_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("VERSION"), "3.19.0\n").unwrap();
        let version = read_version_file(tmp.path()).unwrap();
        assert_eq!(version, "3.19.0");
    }

    #[test]
    fn test_read_version_file_missing() {
        let tmp = TempDir::new().unwrap();
        // No VERSION file
        let result = read_version_file(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_sdk_source_display_explicit_config() {
        assert_eq!(SdkSource::ExplicitConfig.to_string(), "config.toml");
    }

    #[test]
    fn test_sdk_source_display_environment_variable() {
        assert_eq!(SdkSource::EnvironmentVariable.to_string(), "FLUTTER_ROOT");
    }

    #[test]
    fn test_sdk_source_display_fvm() {
        assert_eq!(
            SdkSource::Fvm {
                version: "3.19.0".into()
            }
            .to_string(),
            "FVM (3.19.0)"
        );
    }

    #[test]
    fn test_sdk_source_display_puro() {
        assert_eq!(
            SdkSource::Puro {
                env: "default".into()
            }
            .to_string(),
            "Puro (default)"
        );
    }

    #[test]
    fn test_sdk_source_display_asdf() {
        assert_eq!(
            SdkSource::Asdf {
                version: "3.19.0".into()
            }
            .to_string(),
            "asdf (3.19.0)"
        );
    }

    #[test]
    fn test_sdk_source_display_mise() {
        assert_eq!(
            SdkSource::Mise {
                version: "3.19.0".into()
            }
            .to_string(),
            "mise (3.19.0)"
        );
    }

    #[test]
    fn test_sdk_source_display_proto() {
        assert_eq!(
            SdkSource::Proto {
                version: "3.19.0".into()
            }
            .to_string(),
            "proto (3.19.0)"
        );
    }

    #[test]
    fn test_sdk_source_display_flutter_wrapper() {
        assert_eq!(SdkSource::FlutterWrapper.to_string(), "flutter_wrapper");
    }

    #[test]
    fn test_sdk_source_display_system_path() {
        assert_eq!(SdkSource::SystemPath.to_string(), "system PATH");
    }

    #[test]
    fn test_sdk_source_display_path_inferred() {
        assert_eq!(
            SdkSource::PathInferred.to_string(),
            "system PATH (inferred)"
        );
    }

    #[test]
    fn test_flutter_executable_direct_path() {
        let exe = FlutterExecutable::Direct(PathBuf::from("/usr/local/flutter/bin/flutter"));
        assert_eq!(exe.path(), Path::new("/usr/local/flutter/bin/flutter"));
    }

    #[test]
    fn test_flutter_executable_windows_batch_path() {
        let exe = FlutterExecutable::WindowsBatch(PathBuf::from("C:\\flutter\\bin\\flutter.bat"));
        assert_eq!(exe.path(), Path::new("C:\\flutter\\bin\\flutter.bat"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_flutter_executable_direct_command() {
        let exe = FlutterExecutable::Direct(PathBuf::from("/usr/local/flutter/bin/flutter"));
        // Just ensure it builds a command without panicking
        let _cmd = exe.command();
    }

    #[test]
    fn test_flutter_sdk_invalid_error_is_fatal() {
        let err = Error::flutter_sdk_invalid("/path/to/sdk", "missing binary");
        assert!(err.is_fatal());
    }

    #[test]
    fn test_flutter_sdk_invalid_error_display() {
        let err = Error::flutter_sdk_invalid("/path/to/sdk", "missing binary");
        let msg = err.to_string();
        assert!(msg.contains("/path/to/sdk"));
        assert!(msg.contains("missing binary"));
    }

    #[test]
    fn test_validate_sdk_path_returns_direct_on_unix() {
        #[cfg(not(target_os = "windows"))]
        {
            let tmp = TempDir::new().unwrap();
            let root = tmp.path();
            fs::create_dir_all(root.join("bin/cache/dart-sdk")).unwrap();
            fs::write(root.join("bin/flutter"), "#!/bin/sh").unwrap();
            fs::write(root.join("VERSION"), "3.19.0").unwrap();

            let exe = validate_sdk_path(root).unwrap();
            assert!(matches!(exe, FlutterExecutable::Direct(_)));
            assert_eq!(exe.path(), root.join("bin/flutter"));
        }
    }
}
