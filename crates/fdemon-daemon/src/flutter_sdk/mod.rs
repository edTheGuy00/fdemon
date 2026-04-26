//! # Flutter SDK Discovery
//!
//! Multi-strategy SDK detection supporting FVM, Puro, asdf, mise,
//! proto, flutter_wrapper, and system PATH installations.
//!
//! ## Public API
//!
//! ### Core Types
//! - [`FlutterSdk`] - A resolved Flutter SDK with metadata
//! - [`SdkSource`] - How the SDK was discovered
//! - [`FlutterExecutable`] - How to invoke the flutter binary
//!
//! ### Validation
//! - [`validate_sdk_path()`] - Validate a directory contains a complete SDK
//! - [`read_version_file()`] - Read the Flutter version from a VERSION file
//!
//! ### Channel & Version Detection
//! - [`FlutterChannel`] - Known Flutter release channels
//! - [`FlutterVersion`] - Parsed Flutter version components
//! - [`detect_channel()`] - Detect channel from SDK git state
//! - [`read_dart_version()`] - Read bundled Dart SDK version
//!
//! ### Top-Level Locator
//! - [`find_flutter_sdk()`] - Walk the 11-strategy detection chain and return the first valid SDK
//!
//! ### Version Probe
//! - [`FlutterVersionInfo`] - Extended SDK metadata from `flutter --version --machine`
//! - [`probe_flutter_version()`] - Async probe that runs `flutter --version --machine` with 30s timeout
//!
//! ### Cache Scanner
//! - [`InstalledSdk`] - A Flutter SDK version installed in the FVM cache
//! - [`scan_installed_versions()`] - Scan the FVM cache for installed SDK versions
//! - [`scan_installed_versions_from_path()`] - Testable variant with explicit cache path
//!
//! ### Version Manager Detection
//! - [`detect_fvm_modern()`] - FVM `.fvmrc` config
//! - [`detect_fvm_legacy()`] - FVM `.fvm/fvm_config.json` + symlink
//! - [`detect_puro()`] - Puro `.puro.json` config
//! - [`detect_asdf()`] - asdf `.tool-versions`
//! - [`detect_mise()`] - mise `.mise.toml`
//! - [`detect_proto()`] - proto `.prototools`
//! - [`detect_flutter_wrapper()`] - flutter_wrapper `flutterw` + `.flutter/`

pub mod cache_scanner;
mod channel;
mod locator;
mod types;
pub mod version_managers;
pub mod version_probe;

#[cfg(all(test, target_os = "windows"))]
mod windows_tests;

pub use cache_scanner::{
    resolve_fvm_cache_path, scan_installed_versions, scan_installed_versions_from_path,
    InstalledSdk,
};
pub use channel::{detect_channel, read_dart_version, FlutterChannel, FlutterVersion};
pub use locator::find_flutter_sdk;
pub use types::{
    read_version_file, validate_sdk_path, FlutterExecutable, FlutterSdk, FlutterVersionInfo,
    SdkSource,
};
pub use version_managers::{
    detect_asdf, detect_flutter_wrapper, detect_fvm_legacy, detect_fvm_modern, detect_mise,
    detect_proto, detect_puro,
};
pub use version_probe::probe_flutter_version;
