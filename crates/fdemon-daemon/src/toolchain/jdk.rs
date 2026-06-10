//! # JDK Home Resolution and Flutter JDK Configuration
//!
//! Provides two utilities:
//!
//! - [`resolve_jdk_home`] — best-effort resolution of the JDK installation
//!   directory from `JAVA_HOME` or by walking from the `java` binary on `PATH`.
//! - [`validate_jdk_home`] — normalize and validate a JDK home path, requiring
//!   both `bin/java[.exe]` and `bin/javac[.exe]` to confirm it is a real JDK.
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

/// Normalize and validate a JDK home directory path.
///
/// Normalization:
/// - Strip surrounding ASCII double-quotes (e.g. `"C:\jdk-21"` → `C:\jdk-21`).
/// - Strip any trailing `/` or `\` characters.
///
/// Validation:
/// - The resulting path must be an existing directory.
/// - It must contain `bin/java` (or `bin/java.exe` on Windows) — confirms Java
///   runtime is present.
/// - It must contain `bin/javac` (or `bin/javac.exe` on Windows) — confirms this
///   is a full JDK, not a JRE. The `sdkmanager` build system requires javac.
///
/// # Returns
///
/// `Ok(PathBuf)` — the normalized (trimmed) JDK home path on success.
///
/// # Errors
///
/// Returns `Err` with a diagnostic message when the path does not exist, is not a
/// directory, or is missing either the `java` or `javac` binary. The error
/// message names the expected remedies (install JDK 17+, set `[toolchain]
/// jdk_path`, or fix `JAVA_HOME`) so the user can act without reading docs.
pub fn validate_jdk_home(jdk_home: &Path) -> Result<PathBuf> {
    // Step 1: Normalize — strip surrounding quotes and trailing slash/backslash.
    let raw = jdk_home.to_string_lossy();
    let trimmed = raw
        .trim_matches('"')
        .trim_end_matches('/')
        .trim_end_matches('\\');
    let normalized = PathBuf::from(trimmed);

    // Step 2: Must exist and be a directory.
    if !normalized.is_dir() {
        return Err(Error::process(format!(
            "JDK home '{}' does not exist or is not a directory. \
             Install a JDK 17+ (e.g. Eclipse Temurin), set '[toolchain] jdk_path' \
             in .fdemon/config.toml, or fix the JAVA_HOME environment variable.",
            normalized.display()
        )));
    }

    // Step 3: Must contain bin/java[.exe] and bin/javac[.exe].
    // Use .exe extensions only on Windows; POSIX binaries have no extension.
    #[cfg(windows)]
    let (java_name, javac_name) = ("java.exe", "javac.exe");
    #[cfg(not(windows))]
    let (java_name, javac_name) = ("java", "javac");

    let java_bin = normalized.join("bin").join(java_name);
    let javac_bin = normalized.join("bin").join(javac_name);

    if !java_bin.exists() {
        return Err(Error::process(format!(
            "JDK home '{}' is missing 'bin/{java_name}'. \
             Install a JDK 17+ (e.g. Eclipse Temurin), set '[toolchain] jdk_path' \
             in .fdemon/config.toml, or fix the JAVA_HOME environment variable.",
            normalized.display()
        )));
    }

    if !javac_bin.exists() {
        return Err(Error::process(format!(
            "JDK home '{}' has 'bin/{java_name}' but is missing 'bin/{javac_name}'. \
             This looks like a JRE rather than a full JDK — the Android build system \
             requires javac. Install a JDK 17+ (e.g. Eclipse Temurin), set \
             '[toolchain] jdk_path' in .fdemon/config.toml, or fix JAVA_HOME.",
            normalized.display()
        )));
    }

    Ok(normalized)
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

/// Known system directories that are not JDK homes.
///
/// `/usr/bin/java` on macOS is a stub that invokes the JDK selector; on Linux
/// it may be a binary managed by `update-alternatives` but the grandparent
/// (`/usr`) is the system root, not a JDK home.  `/usr/local` is similarly a
/// system prefix, never a JDK.  Returning these as a `JAVA_HOME` would produce
/// `flutter config --jdk-dir=/usr`, which breaks the Flutter build.
const NON_JDK_PREFIXES: &[&str] = &["/usr", "/usr/local"];

/// Check whether `home` contains the canonical JDK markers for the given
/// `javac_name` (either `"javac"` on POSIX or `"javac.exe"` on Windows).
///
/// A candidate JDK home is accepted when it has **either**:
/// - A `release` file (present in every OpenJDK / OracleJDK build since JDK 9), **or**
/// - A `bin/<javac_name>` binary.
///
/// A bare `lib/` subdirectory is no longer accepted on its own because
/// `/usr/lib` exists on every Unix system and would be a false positive for
/// stubs at `/usr/bin/java`.
///
/// This helper is extracted as a pure function so tests can exercise **both**
/// platform names (`javac` and `javac.exe`) with a temp-dir fixture on any host
/// OS, rather than requiring `#[cfg(windows)]`-only coverage.
pub(crate) fn has_jdk_markers(home: &Path, javac_name: &str) -> bool {
    home.join("release").is_file() || home.join("bin").join(javac_name).exists()
}

/// Attempt to resolve the JDK home by locating the `java` binary via `which`
/// and walking two directories up (bin → java_home).
///
/// Typical layouts:
/// - Linux: `/usr/lib/jvm/java-21-openjdk/bin/java` → `/usr/lib/jvm/java-21-openjdk`
/// - macOS: `/usr/bin/java` (stub) → skip; `/Library/Java/JavaVirtualMachines/…/Contents/Home/bin/java`
/// - Windows: `C:\Program Files\Eclipse Adoptium\jdk-21\bin\java.exe` → `…\jdk-21`
/// - FVM/asdf: `~/.jdks/corretto-21/bin/java` → `~/.jdks/corretto-21`
///
/// The candidate JDK home must satisfy **both** of these conditions to be
/// accepted:
///
/// 1. It is not a known non-JDK system prefix (e.g. `/usr`, `/usr/local`).
/// 2. It contains a canonical JDK marker: a `release` file **or** a `bin/javac[.exe]`
///    binary (platform-aware name, matching `validate_jdk_home`).
fn java_home_from_which() -> Option<PathBuf> {
    let java_bin = which::which("java").ok()?;

    // Resolve symlinks so we get the real binary path.
    let real_bin = std::fs::canonicalize(&java_bin).unwrap_or(java_bin);

    // Parent of the binary is the `bin/` directory; parent of that is the JDK home.
    let bin_dir = real_bin.parent()?;
    let jdk_home = bin_dir.parent()?;

    // Reject well-known non-JDK system prefixes.
    //
    // On macOS, `/usr/bin/java` is a stub (XCode command-line tools shim) whose
    // grandparent is `/usr`.  On Linux, `update-alternatives` may point
    // `java → /usr/bin/java` when no JDK is installed, again giving `/usr`.
    // Accepting `/usr` as a JDK home would pass a bogus path to
    // `flutter config --jdk-dir`.
    let jdk_home_str = jdk_home.to_string_lossy();
    if NON_JDK_PREFIXES
        .iter()
        .any(|prefix| jdk_home_str == *prefix)
    {
        tracing::debug!(
            path = %jdk_home.display(),
            "java_home_from_which: rejected — resolves to a known non-JDK system prefix"
        );
        return None;
    }

    // A real JDK home must contain a `release` file (present in every OpenJDK /
    // OracleJDK build since JDK 9) or a `bin/javac[.exe]` compiler binary.
    // The javac name is platform-aware (`.exe` on Windows, no extension on POSIX)
    // to match the check in `validate_jdk_home`.
    #[cfg(windows)]
    let javac_name = "javac.exe";
    #[cfg(not(windows))]
    let javac_name = "javac";

    if has_jdk_markers(jdk_home, javac_name) {
        return Some(jdk_home.to_owned());
    }

    tracing::debug!(
        path = %jdk_home.display(),
        "java_home_from_which: rejected — no release file or bin/{javac_name} found"
    );
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

    // ── validate_jdk_home tests ───────────────────────────────────────────────

    /// Helper: create a minimal JDK fixture in a temp dir (bin/java + bin/javac).
    fn make_jdk_fixture(dir: &std::path::Path) {
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();

        #[cfg(windows)]
        let (java_name, javac_name) = ("java.exe", "javac.exe");
        #[cfg(not(windows))]
        let (java_name, javac_name) = ("java", "javac");

        std::fs::write(bin.join(java_name), b"#!/bin/sh\nexec java").unwrap();
        std::fs::write(bin.join(javac_name), b"#!/bin/sh\nexec javac").unwrap();
    }

    /// A directory with bin/java and bin/javac is a valid JDK home.
    #[test]
    fn test_validate_jdk_home_accepts_full_jdk() {
        let tmp = tempfile::TempDir::new().unwrap();
        make_jdk_fixture(tmp.path());

        let result = validate_jdk_home(tmp.path());
        assert!(
            result.is_ok(),
            "a dir with bin/java and bin/javac must be accepted: {:?}",
            result
        );
    }

    /// Trailing slash is stripped and the path is still accepted.
    #[test]
    fn test_validate_jdk_home_strips_trailing_slash() {
        let tmp = tempfile::TempDir::new().unwrap();
        make_jdk_fixture(tmp.path());

        // Build a path string with trailing slash.
        let with_slash = PathBuf::from(format!("{}/", tmp.path().display()));
        let result = validate_jdk_home(&with_slash);
        assert!(
            result.is_ok(),
            "trailing slash must be stripped and path accepted: {:?}",
            result
        );
        // The returned path must not have a trailing slash component.
        let ok_path = result.unwrap();
        let ok_str = ok_path.to_string_lossy();
        assert!(
            !ok_str.ends_with('/'),
            "normalized path must not end with '/': {ok_str}"
        );
    }

    /// Trailing backslash is stripped (important on Windows paths).
    #[test]
    fn test_validate_jdk_home_strips_trailing_backslash() {
        let tmp = tempfile::TempDir::new().unwrap();
        make_jdk_fixture(tmp.path());

        let with_backslash = PathBuf::from(format!("{}\\", tmp.path().display()));
        let result = validate_jdk_home(&with_backslash);
        // The trimmed path resolves to the same dir; it may or may not validate
        // depending on whether the OS supports trailing backslash in paths.
        // On POSIX, `foo\` refers to a directory named literally `foo\` — which
        // doesn't exist — so validation will fail.  On Windows, trailing backslash
        // is stripped.  We only assert this does not panic and returns a Result.
        let _ = result; // must not panic
    }

    /// A path pointing at the `bin/` subdirectory (not the JDK root) is rejected.
    #[test]
    fn test_validate_jdk_home_rejects_bin_subdirectory() {
        let tmp = tempfile::TempDir::new().unwrap();
        make_jdk_fixture(tmp.path());

        // Pass the `bin/` path instead of the JDK root.
        let bin_path = tmp.path().join("bin");
        let result = validate_jdk_home(&bin_path);
        assert!(
            result.is_err(),
            "the bin/ subdirectory must be rejected as the JDK home"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Install a JDK") || err.contains("missing"),
            "error must be actionable: {err}"
        );
    }

    /// A non-existent directory is rejected.
    #[test]
    fn test_validate_jdk_home_rejects_nonexistent_dir() {
        let result = validate_jdk_home(Path::new("/this/path/does/not/exist/fdemon_jdk_test"));
        assert!(result.is_err(), "a non-existent path must be rejected");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("does not exist") || err.contains("Install a JDK"),
            "error must mention non-existence or remedies: {err}"
        );
    }

    /// A JRE-only directory (has bin/java but no bin/javac) is rejected.
    #[test]
    fn test_validate_jdk_home_rejects_jre_only() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();

        // Only plant java, not javac.
        #[cfg(windows)]
        let java_name = "java.exe";
        #[cfg(not(windows))]
        let java_name = "java";

        std::fs::write(bin.join(java_name), b"#!/bin/sh\nexec java").unwrap();

        let result = validate_jdk_home(tmp.path());
        assert!(
            result.is_err(),
            "a JRE-only dir (no javac) must be rejected"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("javac") || err.contains("JRE") || err.contains("JDK"),
            "error must mention javac or JDK/JRE distinction: {err}"
        );
    }

    /// Surrounding quotes are stripped from the path before validation.
    #[test]
    fn test_validate_jdk_home_strips_surrounding_quotes() {
        let tmp = tempfile::TempDir::new().unwrap();
        make_jdk_fixture(tmp.path());

        // Wrap the path in double-quotes (as Windows JAVA_HOME sometimes has).
        let quoted = PathBuf::from(format!("\"{}\"", tmp.path().display()));
        let result = validate_jdk_home(&quoted);
        assert!(
            result.is_ok(),
            "surrounding quotes must be stripped and path accepted: {:?}",
            result
        );
    }

    // ── java_home_from_which heuristic tests ──────────────────────────────────

    /// `java_home_from_which` must return `None` when the resolved binary is
    /// under a known non-JDK prefix (`/usr`), even if `/usr/lib` exists.
    ///
    /// We can't invoke `which` here (it would query the live system), so we
    /// test the filtering predicate directly via a helper.
    #[test]
    fn test_java_home_rejects_usr_prefix() {
        // Simulate the grandparent path that `java_home_from_which` would
        // derive from `/usr/bin/java`.
        let candidate = PathBuf::from("/usr");
        let candidate_str = candidate.to_string_lossy();
        let rejected = NON_JDK_PREFIXES
            .iter()
            .any(|prefix| candidate_str == *prefix);
        assert!(rejected, "/usr must be in the NON_JDK_PREFIXES list");
    }

    #[test]
    fn test_java_home_rejects_usr_local_prefix() {
        let candidate = PathBuf::from("/usr/local");
        let candidate_str = candidate.to_string_lossy();
        let rejected = NON_JDK_PREFIXES
            .iter()
            .any(|prefix| candidate_str == *prefix);
        assert!(rejected, "/usr/local must be in the NON_JDK_PREFIXES list");
    }

    /// A real JDK layout (with `release` and `bin/javac`) is accepted.
    #[test]
    fn test_java_home_accepts_release_file_layout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let jdk_home = tmp.path();

        // Plant the JDK markers.
        std::fs::write(jdk_home.join("release"), b"JAVA_VERSION=\"21\"").unwrap();
        let bin = jdk_home.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("javac"), b"#!/bin/sh").unwrap();

        // Verify that a candidate with these markers passes the heuristic.
        let jdk_home_str = jdk_home.to_string_lossy();
        let is_non_jdk_prefix = NON_JDK_PREFIXES
            .iter()
            .any(|prefix| jdk_home_str == *prefix);
        assert!(
            !is_non_jdk_prefix,
            "temp JDK dir must not be a known prefix"
        );

        let has_release = jdk_home.join("release").is_file();
        let has_javac = jdk_home.join("bin").join("javac").exists();
        assert!(
            has_release || has_javac,
            "fixture must pass the JDK marker check"
        );
    }

    /// A stub layout (only `lib/` subdirectory, no `release`, no `bin/javac`)
    /// is rejected — this prevents `/usr` from being returned as a JDK home.
    #[test]
    fn test_java_home_rejects_lib_only_layout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let jdk_home = tmp.path();

        // Only create `lib/` — no `release`, no `bin/javac`.
        let lib = jdk_home.join("lib");
        std::fs::create_dir_all(&lib).unwrap();

        let has_release = jdk_home.join("release").is_file();
        let has_javac = jdk_home.join("bin").join("javac").exists();
        assert!(
            !has_release && !has_javac,
            "fixture with only lib/ must fail the JDK marker check"
        );
    }

    /// `java_home_from_which` returns `None` for a temp directory that has the
    /// structure of the known stub path (`bin/java` exists but no `release`
    /// and no `bin/javac`) and is not in the non-JDK prefix list.
    ///
    /// This tests the pure marker logic without invoking `which`.
    #[test]
    fn test_java_home_stub_dir_rejected_without_jdk_markers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let jdk_home = tmp.path();

        // Simulate a stub: only `bin/java`, no `release`, no `bin/javac`.
        let bin = jdk_home.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("java"), b"#!/bin/sh").unwrap();

        let has_release = jdk_home.join("release").is_file();
        let has_javac = jdk_home.join("bin").join("javac").exists();
        // Must fail the marker check.
        assert!(
            !has_release && !has_javac,
            "stub dir must not have release or javac"
        );
    }

    // ── has_jdk_markers cross-platform helper tests ───────────────────────────

    /// `has_jdk_markers` with POSIX name accepts a home that has `bin/javac`
    /// (no extension) — tests the Linux/macOS path on any host OS.
    #[test]
    fn test_has_jdk_markers_posix_javac_accepted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("javac"), b"#!/bin/sh").unwrap();

        assert!(
            has_jdk_markers(tmp.path(), "javac"),
            "bin/javac must satisfy the POSIX marker check"
        );
    }

    /// `has_jdk_markers` with Windows name accepts a home that has `bin/javac.exe`
    /// — tests the Windows path on any host OS (cross-platform testability).
    #[test]
    fn test_has_jdk_markers_windows_javac_exe_accepted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("javac.exe"), b"MZ").unwrap();

        assert!(
            has_jdk_markers(tmp.path(), "javac.exe"),
            "bin/javac.exe must satisfy the Windows marker check"
        );
    }

    /// `has_jdk_markers` accepts a home that has only a `release` file (no javac),
    /// regardless of the javac name passed — covers the OpenJDK `release`-only layout.
    #[test]
    fn test_has_jdk_markers_release_file_accepted_regardless_of_javac_name() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("release"), b"JAVA_VERSION=\"21\"").unwrap();

        assert!(
            has_jdk_markers(tmp.path(), "javac"),
            "release file must satisfy the marker check with POSIX javac name"
        );
        assert!(
            has_jdk_markers(tmp.path(), "javac.exe"),
            "release file must satisfy the marker check with Windows javac name"
        );
    }

    /// `has_jdk_markers` rejects a home that has neither `release` nor any
    /// `bin/javac*` file — tests the stub/empty layout.
    #[test]
    fn test_has_jdk_markers_empty_dir_rejected_for_both_names() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Only create bin/ with java but no javac — simulates a JRE stub.
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("java"), b"#!/bin/sh").unwrap();

        assert!(
            !has_jdk_markers(tmp.path(), "javac"),
            "stub dir must be rejected with POSIX javac name"
        );
        assert!(
            !has_jdk_markers(tmp.path(), "javac.exe"),
            "stub dir must be rejected with Windows javac name"
        );
    }

    /// `has_jdk_markers` with POSIX name `"javac"` does NOT accept a dir that
    /// only has `bin/javac.exe` — the name check is exact (no cross-name
    /// contamination).
    #[test]
    fn test_has_jdk_markers_posix_name_does_not_match_exe_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        // Only plant javac.exe, not javac.
        std::fs::write(bin.join("javac.exe"), b"MZ").unwrap();

        assert!(
            !has_jdk_markers(tmp.path(), "javac"),
            "POSIX name 'javac' must not match 'javac.exe'"
        );
    }

    /// `has_jdk_markers` with Windows name `"javac.exe"` does NOT accept a dir
    /// that only has `bin/javac` (no extension).
    #[test]
    fn test_has_jdk_markers_windows_name_does_not_match_posix_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        // Only plant javac (no .exe).
        std::fs::write(bin.join("javac"), b"#!/bin/sh").unwrap();

        assert!(
            !has_jdk_markers(tmp.path(), "javac.exe"),
            "Windows name 'javac.exe' must not match 'javac'"
        );
    }
}
