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

/// Returns `true` if the given stderr text indicates a Windows path-resolution
/// error — the kind that the `windows_hint()` advice can actually fix.
///
/// Matches phrases produced by `cmd.exe`, the NT loader, and `CreateProcessW`
/// when a binary or path cannot be resolved.
pub(crate) fn is_path_resolution_error(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("cannot find the path")
        || lower.contains("system cannot find")
        || lower.contains("not recognized as an internal")
        || lower.contains("no such file or directory") // Unix counterpart, harmless on Windows
}

/// Strip ANSI escape sequences from a string. Useful when embedding a child
/// process's stderr into a user-facing error message — the TUI does not
/// interpret raw ANSI in error text, so leftover escapes appear as literal
/// noise.
pub(crate) fn strip_ansi(input: &str) -> String {
    // Minimal CSI-only stripper: handle ESC [ ... letter sequences.
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            for inner in chars.by_ref() {
                if inner.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_path_resolution_error_matches_cmd_messages() {
        assert!(is_path_resolution_error(
            "The system cannot find the path specified."
        ));
        assert!(is_path_resolution_error(
            "'flutter' is not recognized as an internal or external command"
        ));
        assert!(!is_path_resolution_error(
            "flutter doctor: please accept the Android licenses"
        ));
        assert!(!is_path_resolution_error(""));
    }

    #[test]
    fn test_strip_ansi_removes_color_codes() {
        assert_eq!(strip_ansi("\x1b[31merror\x1b[0m: bad"), "error: bad");
        assert_eq!(strip_ansi("plain text"), "plain text");
        assert_eq!(strip_ansi(""), "");
    }
}
