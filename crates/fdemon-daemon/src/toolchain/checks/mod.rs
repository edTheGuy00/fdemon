//! # Toolchain Component Probes
//!
//! One `async fn check_*` per toolchain component, each returning a
//! [`ComponentCheck`]. All probes are read-only; they never install, download,
//! or modify system state.
//!
//! Android-specific probes live in the [`android`] submodule and are
//! re-exported here so callers in `toolchain/mod.rs` see them via
//! `checks::check_android_*` and `checks::android_sdk_root`.
//!
//! OS-level prerequisites live in the [`prerequisites`] submodule and are
//! re-exported here as `checks::check_prerequisites`.
//!
//! See also: [`android_sdk_root`] — resolves the Android SDK root from env vars
//! and OS-specific default locations.

mod android;
mod prerequisites;

pub use android::{
    android_sdk_root, check_android_build_tools, check_android_cmdline_tools,
    check_android_licenses, check_android_platform, check_android_platform_tools,
    sdkmanager_bin_name,
};
pub use prerequisites::check_prerequisites;

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::flutter_sdk::{diagnostics::strip_ansi, find_flutter_sdk};

use super::types::{ComponentCheck, ComponentKind, ComponentStatus};

#[cfg(test)]
use super::types::HostPlatform;

/// Cap stored probe detail so a misbehaving tool's first line cannot bloat the report.
const MAX_DETAIL_LEN: usize = 256;

/// Strip ANSI escape sequences and truncate to [`MAX_DETAIL_LEN`] characters.
///
/// Applied to `detail` strings that originate from external process output.
/// Code-authored static strings are **not** passed through this function.
pub(super) fn strip_and_truncate(s: &str) -> String {
    let cleaned = strip_ansi(s);
    if cleaned.len() <= MAX_DETAIL_LEN {
        cleaned
    } else {
        // Truncate at a character boundary
        cleaned
            .char_indices()
            .nth(MAX_DETAIL_LEN)
            .map_or(cleaned.clone(), |(i, _)| cleaned[..i].to_string())
    }
}

/// Timeout for lightweight `--version` style tool probes.
pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for the JDK probe, which may be slower on first run.
const JDK_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

// ─── Flutter SDK ─────────────────────────────────────────────────────────────

/// Check for the Flutter SDK.
///
/// Runs the full 12-strategy SDK locator. On success, returns `Ok` with a
/// detail string containing the version and discovery source. On failure,
/// classifies as `Missing` or `Partial` depending on the error.
pub async fn check_flutter(
    project_path: &Path,
    explicit_path: Option<&Path>,
) -> (
    ComponentCheck,
    Option<crate::flutter_sdk::FlutterExecutable>,
) {
    match find_flutter_sdk(project_path, explicit_path) {
        Ok(sdk) => {
            let detail = format!("{} ({})", sdk.version, sdk.source);
            (
                ComponentCheck {
                    kind: ComponentKind::FlutterSdk,
                    status: ComponentStatus::Ok,
                    detail,
                },
                Some(sdk.executable),
            )
        }
        Err(e) => {
            use fdemon_core::error::Error;
            let (status, detail) = match &e {
                Error::FlutterNotFound => (
                    ComponentStatus::Missing,
                    "Flutter SDK not found. Ensure 'flutter' is in your PATH.".to_string(),
                ),
                Error::FlutterSdkInvalid { path, reason } => (
                    ComponentStatus::Partial,
                    format!("SDK at {} is invalid: {}", path.display(), reason),
                ),
                other => (ComponentStatus::Error, other.to_string()),
            };
            (
                ComponentCheck {
                    kind: ComponentKind::FlutterSdk,
                    status,
                    detail,
                },
                None,
            )
        }
    }
}

// ─── Git ─────────────────────────────────────────────────────────────────────

/// Check for `git` on PATH.
///
/// Runs `git --version` and parses the version string. Returns `Ok` when git
/// is found and responsive, `Missing` otherwise.
pub async fn check_git() -> ComponentCheck {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("git")
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let version = strip_and_truncate(raw.trim());
            ComponentCheck {
                kind: ComponentKind::Git,
                status: ComponentStatus::Ok,
                detail: version,
            }
        }
        Ok(Ok(output)) => {
            let raw_stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = strip_and_truncate(raw_stderr.trim());
            ComponentCheck {
                kind: ComponentKind::Git,
                status: ComponentStatus::Error,
                detail: if stderr.is_empty() {
                    format!("git exited with status {}", output.status)
                } else {
                    stderr
                },
            }
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::Git,
            status: ComponentStatus::Missing,
            detail: "git not found on PATH".to_string(),
        },
        Ok(Err(e)) => ComponentCheck {
            kind: ComponentKind::Git,
            status: ComponentStatus::Error,
            detail: format!("git probe failed: {e}"),
        },
        Err(_) => ComponentCheck {
            kind: ComponentKind::Git,
            status: ComponentStatus::Error,
            detail: "git --version timed out".to_string(),
        },
    }
}

// ─── JDK ─────────────────────────────────────────────────────────────────────

/// Check for a Java Development Kit.
///
/// Runs `java -version` (which writes to **stderr**) and parses the major version.
/// - Major version `>= 17` → `Ok`
/// - Present but `< 17` → `Partial` (detail names the version)
/// - Not found → `Missing`
pub async fn check_jdk() -> ComponentCheck {
    let result = tokio::time::timeout(JDK_PROBE_TIMEOUT, async {
        Command::new("java")
            .arg("-version")
            // `java -version` outputs to stderr, not stdout
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) => {
            // `java -version` exits with 0 whether or not stderr has output.
            let stderr_text = String::from_utf8_lossy(&output.stderr);
            parse_jdk_output(&stderr_text)
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Missing,
            detail: "java not found on PATH".to_string(),
        },
        Ok(Err(e)) => ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Error,
            detail: format!("java probe failed: {e}"),
        },
        Err(_) => ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Error,
            detail: "java -version timed out".to_string(),
        },
    }
}

/// Parse the stderr output of `java -version` into a [`ComponentCheck`].
///
/// Handles both the modern `openjdk version "17.0.2" ...` format and the
/// older `java version "1.8.0_291"` format.
fn parse_jdk_output(text: &str) -> ComponentCheck {
    // Look for a version string like `"17.0.2"` or `"1.8.0_291"`
    // The first line typically has: openjdk version "X.Y.Z" ...
    // Strip ANSI codes that some JVM distributions emit on their version output.
    let raw_first_line = text.lines().next().unwrap_or("");
    let first_line = strip_ansi(raw_first_line);
    let first_line = first_line.trim();

    if first_line.is_empty() {
        return ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Missing,
            detail: "java not found on PATH".to_string(),
        };
    }

    // Extract the quoted version string
    let version_str = extract_quoted_version(first_line);

    match version_str {
        Some(v) => {
            let major = parse_java_major_version(&v);
            match major {
                Some(maj) if maj >= 17 => ComponentCheck {
                    kind: ComponentKind::Jdk,
                    status: ComponentStatus::Ok,
                    detail: format!("Java {v} (major {maj})"),
                },
                Some(maj) => ComponentCheck {
                    kind: ComponentKind::Jdk,
                    status: ComponentStatus::Partial,
                    detail: format!("Java {v} (major {maj}) — Android requires JDK 17 or newer"),
                },
                // m5 fix: unparseable major version is not a confirmed-good JDK.
                None => ComponentCheck {
                    kind: ComponentKind::Jdk,
                    status: ComponentStatus::Partial,
                    detail: format!("Java {v} (could not determine major version)"),
                },
            }
        }
        None => ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Error,
            detail: strip_and_truncate(&format!("could not parse java version from: {first_line}")),
        },
    }
}

/// Extract the version string from inside double-quotes in a `java -version` line.
fn extract_quoted_version(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

/// Parse the major Java version from a version string.
///
/// Handles both:
/// - Modern: `"17.0.2"` → 17
/// - Legacy: `"1.8.0_291"` → 8 (the second component when first is `1`)
fn parse_java_major_version(v: &str) -> Option<u32> {
    let mut parts = v.split('.');
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        // Legacy version format: 1.X.Y → major is X
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── JDK parsing ───────────────────────────────────────────────────────────

    #[test]
    fn test_parse_jdk_modern_version_17() {
        let text = r#"openjdk version "17.0.9" 2023-10-17
OpenJDK Runtime Environment (build 17.0.9+9)
"#;
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(check.detail.contains("17.0.9"));
    }

    #[test]
    fn test_parse_jdk_modern_version_21() {
        let text = r#"openjdk version "21.0.1" 2023-10-17
OpenJDK Runtime Environment (build 21.0.1+12)
"#;
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Ok);
    }

    #[test]
    fn test_parse_jdk_legacy_version_8_yields_partial() {
        let text = r#"java version "1.8.0_291"
Java(TM) SE Runtime Environment (build 1.8.0_291-b10)
"#;
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Partial);
        assert!(check.detail.contains("1.8.0_291"));
    }

    #[test]
    fn test_parse_jdk_version_11_yields_partial() {
        let text = r#"openjdk version "11.0.20" 2023-07-18
OpenJDK Runtime Environment (build 11.0.20+8)
"#;
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Partial);
    }

    #[test]
    fn test_parse_jdk_empty_output_yields_missing() {
        let check = parse_jdk_output("");
        assert_eq!(check.status, ComponentStatus::Missing);
    }

    #[test]
    fn test_extract_quoted_version_basic() {
        assert_eq!(
            extract_quoted_version(r#"openjdk version "17.0.9" 2023"#),
            Some("17.0.9".to_string())
        );
    }

    #[test]
    fn test_extract_quoted_version_none_when_no_quotes() {
        assert_eq!(extract_quoted_version("no quotes here"), None);
    }

    #[test]
    fn test_parse_java_major_modern() {
        assert_eq!(parse_java_major_version("17.0.9"), Some(17));
        assert_eq!(parse_java_major_version("21.0.1"), Some(21));
    }

    #[test]
    fn test_parse_java_major_legacy() {
        assert_eq!(parse_java_major_version("1.8.0_291"), Some(8));
        assert_eq!(parse_java_major_version("1.11.0"), Some(11));
    }

    // ── process-spawning checks (no panic guarantee) ───────────────────────

    #[tokio::test]
    async fn test_check_git_present_or_missing_never_panics() {
        let _ = check_git().await;
    }

    #[tokio::test]
    async fn test_check_jdk_present_or_missing_never_panics() {
        let _ = check_jdk().await;
    }

    #[tokio::test]
    async fn test_check_prerequisites_never_panics() {
        let platform = HostPlatform::detect();
        let _ = check_prerequisites(&platform).await;
    }

    // ── m5: JDK unparseable major is not Ok ───────────────────────────────────

    /// A bare `"1"` version string has no parseable major (since the first component
    /// is `1` — legacy format — but there is no second component). This must not
    /// classify as `Ok`.
    #[test]
    fn test_parse_jdk_unparseable_major_is_not_ok() {
        let text = "java version \"1\"\n";
        let check = parse_jdk_output(text);
        assert_ne!(
            check.status,
            ComponentStatus::Ok,
            "unparseable major version must not be Ok; got {:?}",
            check.status
        );
        assert!(
            check.status == ComponentStatus::Partial || check.status == ComponentStatus::Error,
            "expected Partial or Error, got {:?}",
            check.status
        );
    }

    /// Regression guard — Java 17 must still be classified as Ok.
    #[test]
    fn test_parse_jdk_modern_17_is_ok() {
        let text = "openjdk version \"17.0.9\" 2023-10-17\nOpenJDK Runtime Environment\n";
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(check.detail.contains("17.0.9"));
    }

    /// Java 8 (legacy `1.8.x` format) must yield Partial.
    #[test]
    fn test_parse_jdk_legacy_8_is_partial() {
        let text = "java version \"1.8.0_291\"\nJava(TM) SE Runtime Environment\n";
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Partial);
        assert!(check.detail.contains("1.8.0_291"));
    }

    // ── n12: ANSI stripping and length-bounding ───────────────────────────────

    /// `strip_and_truncate` must remove embedded ANSI codes and cap the result at
    /// `MAX_DETAIL_LEN` characters.
    #[test]
    fn test_detail_strips_ansi_and_truncates() {
        // Build a string with a CSI color code + a very long suffix
        let long_suffix = "x".repeat(MAX_DETAIL_LEN + 50);
        let input = format!("\x1b[31merror\x1b[0m: {long_suffix}");
        let result = strip_and_truncate(&input);
        // ANSI codes stripped
        assert!(!result.contains('\x1b'), "ANSI escape survived stripping");
        // Length bounded
        assert!(
            result.len() <= MAX_DETAIL_LEN,
            "detail len {} exceeds MAX_DETAIL_LEN {}",
            result.len(),
            MAX_DETAIL_LEN
        );
        // Visible content preserved
        assert!(
            result.starts_with("error:"),
            "content was mangled: {result:?}"
        );
    }

    /// `strip_and_truncate` must leave short strings that contain no ANSI untouched.
    #[test]
    fn test_detail_passthrough_for_clean_short_string() {
        let input = "git version 2.43.0";
        assert_eq!(strip_and_truncate(input), input);
    }

    /// `parse_jdk_output` strips ANSI from the java -version first line before
    /// version extraction.
    #[test]
    fn test_parse_jdk_strips_ansi_from_version_line() {
        // Simulate a JVM that emits color codes around the version line
        let text = "\x1b[32mopenjdk version \"17.0.9\" 2023-10-17\x1b[0m\n";
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(check.detail.contains("17.0.9"));
    }
}
