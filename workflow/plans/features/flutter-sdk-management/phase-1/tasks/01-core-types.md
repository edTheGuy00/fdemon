## Task: Core Types, Module Scaffold, and Dependencies

**Objective**: Create the `flutter_sdk/` directory module in `fdemon-daemon` with core type definitions, SDK validation helpers, error variants, and the necessary Cargo dependency additions. This is the foundation that all other Phase 1 tasks build on.

**Depends on**: None

### Scope

- `crates/fdemon-daemon/Cargo.toml`: Add `toml` and `dirs` workspace dependencies
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: **NEW** — Module root with public re-exports
- `crates/fdemon-daemon/src/flutter_sdk/types.rs`: **NEW** — `FlutterSdk`, `SdkSource`, `FlutterExecutable`, validation
- `crates/fdemon-daemon/src/lib.rs`: Add `pub mod flutter_sdk` and re-exports
- `crates/fdemon-core/src/error.rs`: Add SDK-specific error variants

### Details

#### 1. Add workspace dependencies to `crates/fdemon-daemon/Cargo.toml`

```toml
[dependencies]
# ... existing deps ...
toml.workspace = true
dirs.workspace = true
```

Both `toml` and `dirs` are already workspace-level dependencies declared in the root `Cargo.toml`. They just need to be added to fdemon-daemon's own `[dependencies]`.

#### 2. Create `flutter_sdk/types.rs`

Define the three core types from the PLAN:

```rust
//! # Flutter SDK Types
//!
//! Core type definitions for representing a resolved Flutter SDK,
//! how it was discovered, and how to invoke the flutter binary.

use std::path::{Path, PathBuf};

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
}

/// How to invoke the Flutter binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlutterExecutable {
    /// Unix shell script or direct executable — invoke directly
    Direct(PathBuf),
    /// Windows .bat file — requires `cmd /c` wrapper
    WindowsBatch(PathBuf),
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
```

#### 3. Implement `FlutterExecutable` methods

```rust
impl FlutterExecutable {
    /// Returns the path to the flutter executable.
    pub fn path(&self) -> &Path { ... }

    /// Configures a `tokio::process::Command` for this executable type.
    /// For `Direct`: `Command::new(path)`
    /// For `WindowsBatch`: `Command::new("cmd").args(["/c", path.to_str()])`
    pub fn command(&self) -> tokio::process::Command { ... }
}
```

#### 4. Implement SDK validation

```rust
/// Validates that a directory contains a complete Flutter SDK.
///
/// Checks:
/// - `<root>/bin/flutter` (or `flutter.bat` on Windows) exists
/// - `<root>/VERSION` file is readable
/// - `<root>/bin/cache/dart-sdk/` directory exists
///
/// Returns the validated `FlutterExecutable` on success.
pub fn validate_sdk_path(root: &Path) -> Result<FlutterExecutable> { ... }

/// Reads the Flutter version string from `<root>/VERSION`.
pub fn read_version_file(root: &Path) -> Result<String> { ... }
```

Validation must:
- On Unix: check `root/bin/flutter` exists and is a file
- On Windows: check `root/bin/flutter.bat` exists, return `WindowsBatch` variant
- Cross-platform: `root/VERSION` readable, `root/bin/cache/dart-sdk/` is a directory
- Use `cfg(target_os = "windows")` for platform-specific logic

#### 5. Create `flutter_sdk/mod.rs`

```rust
//! # Flutter SDK Discovery
//!
//! Multi-strategy SDK detection supporting FVM, Puro, asdf, mise,
//! proto, flutter_wrapper, and system PATH installations.

mod types;

pub use types::{FlutterExecutable, FlutterSdk, SdkSource};
pub use types::{validate_sdk_path, read_version_file};
```

Initially only re-exports types. As other tasks add `locator.rs`, `version_managers.rs`, `channel.rs`, their public items will be added here.

#### 6. Wire into `fdemon-daemon/src/lib.rs`

Add `pub mod flutter_sdk;` to the module declarations and add relevant re-exports:

```rust
pub mod flutter_sdk;

// Re-exports
pub use flutter_sdk::{FlutterSdk, FlutterExecutable, SdkSource};
```

#### 7. Add error variants to `fdemon-core/src/error.rs`

The existing `FlutterNotFound` is a unit variant with no context. Add a richer variant for SDK-specific failures while keeping the existing one for backward compatibility:

```rust
// In the Error enum, under the Flutter/Daemon section:

/// Flutter SDK path was found but validation failed (incomplete/corrupt SDK)
FlutterSdkInvalid { path: PathBuf, reason: String },
```

Add convenience constructor:
```rust
pub fn flutter_sdk_invalid(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
    Self::FlutterSdkInvalid { path: path.into(), reason: reason.into() }
}
```

Classify as **fatal** in `is_fatal()` — an invalid SDK cannot be recovered from at runtime.

#### 8. Implement `Display` for `SdkSource`

The `SdkSource` needs a human-readable display for logging and UI:

```rust
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
        }
    }
}
```

### Acceptance Criteria

1. `cargo check -p fdemon-daemon` compiles with no errors
2. `cargo check -p fdemon-core` compiles with no errors
3. `FlutterSdk`, `SdkSource`, `FlutterExecutable` are publicly accessible from `fdemon_daemon::flutter_sdk::*`
4. `validate_sdk_path()` returns `Ok(FlutterExecutable::Direct(...))` for a valid SDK directory
5. `validate_sdk_path()` returns `Err` for missing `bin/flutter`, missing `VERSION`, or missing `bin/cache/dart-sdk/`
6. `FlutterExecutable::command()` returns `Command::new(path)` for `Direct` and `Command::new("cmd").args(["/c", path])` for `WindowsBatch`
7. `SdkSource` displays human-readable strings
8. `read_version_file()` reads and trims the VERSION file content
9. `Error::FlutterSdkInvalid` is classified as fatal

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

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
    fn test_validate_sdk_path_missing_dart_sdk() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("bin/flutter"), "#!/bin/sh").unwrap();
        fs::write(root.join("VERSION"), "3.19.0").unwrap();
        // No bin/cache/dart-sdk/

        let result = validate_sdk_path(root);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_version_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("VERSION"), "3.19.0\n").unwrap();
        let version = read_version_file(tmp.path()).unwrap();
        assert_eq!(version, "3.19.0");
    }

    #[test]
    fn test_sdk_source_display() {
        assert_eq!(SdkSource::ExplicitConfig.to_string(), "config.toml");
        assert_eq!(SdkSource::Fvm { version: "3.19.0".into() }.to_string(), "FVM (3.19.0)");
        assert_eq!(SdkSource::SystemPath.to_string(), "system PATH");
    }

    #[test]
    fn test_flutter_executable_direct_path() {
        let exe = FlutterExecutable::Direct(PathBuf::from("/usr/local/flutter/bin/flutter"));
        assert_eq!(exe.path(), Path::new("/usr/local/flutter/bin/flutter"));
    }
}
```

### Notes

- `tempfile` is already a dev-dependency of `fdemon-daemon` — use it for all filesystem-based tests
- Use `#[cfg(target_os = "windows")]` / `#[cfg(not(target_os = "windows"))]` for platform-specific validation paths
- Keep `validate_sdk_path` synchronous — it only does filesystem checks, no process spawning
- The `FlutterExecutable::command()` method returns a `tokio::process::Command` since that's what all call sites use

---

## Completion Summary

**Status:** Not Started
