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
    /// Windows `.bat` file — metadata marker only.
    ///
    /// The runtime invocation is identical to [`Direct`](Self::Direct): Rust's
    /// stdlib (≥ 1.77.2) handles `.bat` / `.cmd` invocation safely when the
    /// program path has an explicit extension, including the `cmd.exe`
    /// argument-escape rules covered by CVE-2024-24576. The old `cmd /c`
    /// wrapper has been removed because it caused quote-stripping failures on
    /// paths containing whitespace (see issues #32, #34).
    WindowsBatch(PathBuf),
}

impl FlutterExecutable {
    /// Returns the path to the flutter executable.
    pub fn path(&self) -> &Path {
        match self {
            Self::Direct(p) | Self::WindowsBatch(p) => p.as_path(),
        }
    }

    /// Configures a [`tokio::process::Command`] for this executable.
    ///
    /// Both variants now invoke the resolved absolute path directly. Rust's
    /// stdlib (≥ 1.77.2 — our MSRV) handles `.bat` / `.cmd` invocation
    /// safely when the program path has an explicit extension, including
    /// the `cmd.exe` argument-escape rules covered by CVE-2024-24576.
    ///
    /// The `WindowsBatch` variant is retained as a *metadata* marker so callers
    /// and logs can tell that the underlying executable is a batch file. The
    /// previous `cmd /c <path>` wrapper has been removed because it caused
    /// quote-stripping failures on paths containing whitespace
    /// (see issues #32, #34).
    pub fn command(&self) -> tokio::process::Command {
        match self {
            Self::Direct(path) | Self::WindowsBatch(path) => tokio::process::Command::new(path),
        }
    }
}

/// Extended Flutter SDK metadata obtained from `flutter --version --machine`.
///
/// All fields are optional because the probe is async and may fail.
/// This complements the file-based [`FlutterSdk`] detection with richer metadata
/// that can only be obtained by running the Flutter CLI.
#[derive(Debug, Clone, Default)]
pub struct FlutterVersionInfo {
    /// Full Flutter framework version (e.g., "3.38.6")
    pub framework_version: Option<String>,
    /// Release channel (e.g., "stable", "beta", "main")
    pub channel: Option<String>,
    /// Git repository URL
    pub repository_url: Option<String>,
    /// Framework commit hash (e.g., "8b87286849")
    pub framework_revision: Option<String>,
    /// Framework commit date (e.g., "2026-01-08 10:49:17 -0800")
    pub framework_commit_date: Option<String>,
    /// Engine revision hash
    pub engine_revision: Option<String>,
    /// Bundled Dart SDK version (e.g., "3.10.7")
    pub dart_sdk_version: Option<String>,
    /// Bundled DevTools version (e.g., "2.51.1")
    pub devtools_version: Option<String>,
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

    // Check VERSION file is readable (try both `version` and `VERSION` — older
    // Flutter SDKs used uppercase, newer ones use lowercase).
    let version_file = root.join("version");
    let version_file_alt = root.join("VERSION");
    if !version_file.is_file() && !version_file_alt.is_file() {
        return Err(Error::flutter_sdk_invalid(
            root,
            format!(
                "version file not found at {} or {}",
                version_file.display(),
                version_file_alt.display()
            ),
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

/// Reads the Flutter version string from `<root>/version` (or `<root>/VERSION`).
///
/// The version file typically contains a version like `3.19.0\n`.
/// Newer Flutter SDKs use lowercase `version`; older ones use uppercase `VERSION`.
/// This function tries lowercase first, then falls back to uppercase.
pub fn read_version_file(root: &Path) -> Result<String> {
    let lowercase = root.join("version");
    let uppercase = root.join("VERSION");
    let version_file = if lowercase.is_file() {
        lowercase
    } else {
        uppercase
    };
    let content = std::fs::read_to_string(&version_file).map_err(|e| {
        Error::flutter_sdk_invalid(
            root,
            format!(
                "failed to read version file at {}: {}",
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

    #[test]
    fn test_flutter_executable_direct_command_invokes_path() {
        let path = PathBuf::from("/usr/local/flutter/bin/flutter");
        let exe = FlutterExecutable::Direct(path.clone());
        let cmd = exe.command();
        assert_eq!(cmd.as_std().get_program(), path.as_os_str());
    }

    #[test]
    fn test_flutter_executable_windows_batch_command_invokes_path() {
        let path = PathBuf::from("C:\\flutter\\bin\\flutter.bat");
        let exe = FlutterExecutable::WindowsBatch(path.clone());
        let cmd = exe.command();
        // After the fix, WindowsBatch invokes the .bat directly (not cmd.exe)
        assert_eq!(cmd.as_std().get_program(), path.as_os_str());
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
