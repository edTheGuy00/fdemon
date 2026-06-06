//! # JDK Home Resolution and Flutter JDK Configuration
//!
//! Provides two utilities:
//!
//! - [`resolve_jdk_home`] — best-effort resolution of the JDK installation
//!   directory from `JAVA_HOME` or by walking from the `java` binary on `PATH`.
//! - [`configure_flutter_jdk_dir`] — runs `flutter config --jdk-dir=<dir>` so
//!   the Flutter CLI uses the specified JDK.
//!
//! The per-OS *guided-install command string* (what a user should run to install
//! a JDK on their platform) is a display concern that lives in app-land (task 05)
//! — not here.

use std::path::{Path, PathBuf};

use fdemon_core::{Error, Result};

use super::process_stream::run_streaming;

// ── Public API ────────────────────────────────────────────────────────────────

/// Best-effort resolution of the JDK home directory.
///
/// Resolution order:
/// 1. `$JAVA_HOME` environment variable, if set and non-empty.
/// 2. Walk from the `java` binary found via `which`: the JDK home is typically
///    `<java_binary>/../../` (i.e. the grandparent of the bin dir).
///
/// Returns `None` when neither heuristic succeeds (JDK not found on this system).
pub fn resolve_jdk_home() -> Option<PathBuf> {
    // 1. $JAVA_HOME
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let path = PathBuf::from(&java_home);
        if !java_home.is_empty() && path.is_dir() {
            tracing::debug!("JDK home from JAVA_HOME: {}", path.display());
            return Some(path);
        }
    }

    // 2. Walk from `which java`.
    if let Some(path) = java_home_from_which() {
        tracing::debug!("JDK home resolved via `which java`: {}", path.display());
        return Some(path);
    }

    None
}

/// Run `flutter config --jdk-dir=<jdk_dir>` so the Flutter CLI uses the
/// specified JDK when building Android targets.
///
/// Output from the `flutter config` command is forwarded to the `tracing` debug
/// log. The function is considered successful when `flutter config` exits with
/// status 0.
///
/// # Arguments
///
/// * `flutter` — Path to the `flutter` binary (or batch file on Windows).
/// * `jdk_dir` — The JDK home directory to configure.
///
/// # Errors
///
/// Returns an error when `jdk_dir` contains newline or control characters, when
/// `flutter config` cannot be spawned, or when it exits non-zero.
/// The `--jdk-dir` value is passed as a single `argv` element to `Command::args`
/// (exec-style, no shell), so there is no shell-injection vector here; the
/// validation is defense-in-depth to catch malformed paths early.
/// The call site decides whether to surface the failure.
pub async fn configure_flutter_jdk_dir(flutter: &Path, jdk_dir: &Path) -> Result<()> {
    // Defense-in-depth: reject newlines and control characters in the JDK path.
    // The path is passed via argv (exec-style), not a shell, so this is not a
    // shell-injection mitigation — it guards against garbled paths reaching the
    // Flutter CLI.
    validate_jdk_dir(jdk_dir)?;

    let flutter_str = flutter.to_string_lossy().to_string();
    let jdk_arg = format!("--jdk-dir={}", jdk_dir.display());

    let status = run_streaming(&flutter_str, &["config", &jdk_arg], None, |line| {
        tracing::debug!("flutter config: {line}");
    })
    .await?;

    if !status.success() {
        return Err(Error::process(format!(
            "`flutter config --jdk-dir={}` exited with {status}",
            jdk_dir.display()
        )));
    }

    Ok(())
}

// ── Validation helpers ────────────────────────────────────────────────────────

/// Validate that `jdk_dir` does not contain newline or control characters.
///
/// This is a defense-in-depth guard: `configure_flutter_jdk_dir` passes the
/// path via argv (exec-style `Command::args`, no shell), so there is no
/// shell-injection vector. The check exists to catch malformed or truncated
/// paths before they reach the Flutter CLI.
fn validate_jdk_dir(jdk_dir: &Path) -> Result<()> {
    let s = jdk_dir.to_string_lossy();
    if s.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
        return Err(Error::config(
            "JDK directory path contains a newline or control character. \
             Refusing to pass a malformed path to `flutter config --jdk-dir`.",
        ));
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Attempt to resolve the JDK home by locating the `java` binary via `which`
/// and walking two directories up (bin → java_home).
///
/// Typical layouts:
/// - Linux: `/usr/lib/jvm/java-21-openjdk/bin/java` → `/usr/lib/jvm/java-21-openjdk`
/// - macOS: `/usr/bin/java` (stub) → skip; `/Library/Java/JavaVirtualMachines/…/Contents/Home/bin/java`
/// - FVM/asdf: `~/.jdks/corretto-21/bin/java` → `~/.jdks/corretto-21`
///
/// The `<bin>/..` parent is the JDK home if it contains a `release` file
/// (canonical JDK layout marker) or a `lib` subdirectory.
fn java_home_from_which() -> Option<PathBuf> {
    let java_bin = which::which("java").ok()?;

    // Resolve symlinks so we get the real binary path.
    let real_bin = std::fs::canonicalize(&java_bin).unwrap_or(java_bin);

    // Parent of the binary is the `bin/` directory; parent of that is the JDK home.
    let bin_dir = real_bin.parent()?;
    let jdk_home = bin_dir.parent()?;

    // Sanity check: a JDK home should have a `release` file or a `lib/` dir.
    if jdk_home.join("release").is_file() || jdk_home.join("lib").is_dir() {
        return Some(jdk_home.to_owned());
    }

    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// When `JAVA_HOME` is set to a valid directory, `resolve_jdk_home` must
    /// return that directory.
    #[test]
    #[serial_test::serial]
    fn test_resolve_jdk_home_honors_java_home() {
        let tmp = tempfile::TempDir::new().unwrap();

        // Temporarily override JAVA_HOME for this test.
        // Use serial_test if test isolation becomes an issue; for now, we
        // use a scoped override.
        std::env::set_var("JAVA_HOME", tmp.path());
        let result = resolve_jdk_home();
        std::env::remove_var("JAVA_HOME");

        assert_eq!(
            result.as_deref(),
            Some(tmp.path()),
            "resolve_jdk_home must return JAVA_HOME when it points to a valid dir"
        );
    }

    /// When `JAVA_HOME` points to a non-existent directory, the variable is
    /// ignored and we fall through to the next strategy.
    #[test]
    #[serial_test::serial]
    fn test_resolve_jdk_home_ignores_nonexistent_java_home() {
        let nonexistent = "/this/path/does/not/exist/fdemon_test";
        std::env::set_var("JAVA_HOME", nonexistent);
        // We can't assert `None` here because `which java` might still succeed.
        // Just assert no panic and result is well-typed.
        let _result = resolve_jdk_home();
        std::env::remove_var("JAVA_HOME");
    }

    /// `resolve_jdk_home` must not panic even when no JDK is configured.
    #[test]
    #[serial_test::serial]
    fn test_resolve_jdk_home_does_not_panic_when_absent() {
        // Remove JAVA_HOME if set; if `which java` also fails, we get None.
        std::env::remove_var("JAVA_HOME");
        let _ = resolve_jdk_home(); // must not panic
    }

    // ── validate_jdk_dir tests ────────────────────────────────────────────────

    #[test]
    fn test_validate_jdk_dir_accepts_normal_path() {
        assert!(validate_jdk_dir(Path::new("/usr/lib/jvm/java-21-openjdk")).is_ok());
        assert!(validate_jdk_dir(Path::new("/home/user/.jdks/corretto-21")).is_ok());
        assert!(validate_jdk_dir(Path::new(
            "/Library/Java/JavaVirtualMachines/jdk-21.jdk/Contents/Home"
        ))
        .is_ok());
    }

    #[test]
    fn test_validate_jdk_dir_rejects_newline() {
        let path = PathBuf::from("/usr/lib/jvm/java-21\n/etc/evil");
        assert!(validate_jdk_dir(&path).is_err(), "newline must be rejected");
        let err = validate_jdk_dir(&path).unwrap_err();
        assert!(
            err.to_string().contains("newline") || err.to_string().contains("control"),
            "error message must mention newline or control character"
        );
    }

    #[test]
    fn test_validate_jdk_dir_rejects_carriage_return() {
        let path = PathBuf::from("/usr/lib/jvm/java-21\r/etc/evil");
        assert!(
            validate_jdk_dir(&path).is_err(),
            "carriage return must be rejected"
        );
    }

    #[test]
    fn test_validate_jdk_dir_rejects_control_char() {
        // ASCII 0x01 (SOH) — a control character that should never appear in a path.
        let path = PathBuf::from("/usr/lib/jvm/java\x01-21");
        assert!(
            validate_jdk_dir(&path).is_err(),
            "control character must be rejected"
        );
    }

    #[test]
    fn test_validate_jdk_dir_accepts_path_with_spaces() {
        // Spaces are valid in JDK paths.
        assert!(validate_jdk_dir(Path::new("/Program Files/Java/jdk-21")).is_ok());
    }
}
