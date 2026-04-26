//! Shared diagnostic helpers for Flutter spawn-path errors.
//!
//! The `windows_hint()` helper is appended to user-facing error strings
//! produced by `devices.rs` and `emulators.rs` when a spawn or non-zero
//! exit is observed on Windows. It points users at the explicit
//! `[flutter] sdk_path` config option for shim-installer environments.

/// Returns a user-facing hint string suitable for appending to an error
/// message. On Windows, returns advice about setting `[flutter] sdk_path`.
/// On other platforms, returns an empty string.
///
/// On Windows, package-manager shims (Chocolatey, scoop, winget) can cause
/// spawn failures when the SDK root cannot be inferred from the shim path.
/// The hint points the user at the config option that lets them pin an
/// exact SDK path.
#[cfg(target_os = "windows")]
pub(crate) fn windows_hint() -> &'static str {
    "\n\nHint: If your Flutter is installed via a package manager (Chocolatey, scoop, winget) \
     or in a non-standard location, set `[flutter] sdk_path = \"C:\\\\path\\\\to\\\\flutter\"` \
     in `.fdemon/config.toml`."
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn windows_hint() -> &'static str {
    ""
}
