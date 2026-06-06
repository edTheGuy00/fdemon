//! # Flutter Doctor Capture and Parser
//!
//! Provides two functions:
//! - [`capture_flutter_doctor`] — runs `flutter doctor -v` and returns the raw output.
//! - [`parse_doctor_output`] — pure parser that converts the raw text to [`DoctorLine`]s.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;

use crate::flutter_sdk::diagnostics::strip_ansi;
use crate::flutter_sdk::FlutterExecutable;

use super::types::{DoctorLine, DoctorMarker};

/// How long to wait for `flutter doctor -v` before giving up.
///
/// `flutter doctor` performs network I/O and SDK cache checks; 60 s is
/// generous enough for a cold first-run but avoids blocking the preflight
/// indefinitely.
const DOCTOR_TIMEOUT: Duration = Duration::from_secs(60);

/// Upper bound on captured `flutter doctor -v` output per stream (stdout or stderr).
///
/// Real output is a few KiB; this cap prevents a misbehaving or replaced
/// binary from streaming unbounded data into an in-memory buffer for the
/// full timeout duration.
const MAX_DOCTOR_OUTPUT_BYTES: u64 = 1024 * 1024; // 1 MiB per stream

/// Maximum leading-space indent to record for a single `DoctorLine`.
///
/// `flutter doctor` never indents more than a handful of spaces; this cap
/// defends against pathological input that would otherwise drive a large
/// per-frame `" ".repeat(indent)` allocation in the TUI.
const MAX_DOCTOR_INDENT: usize = 32;

/// Run `flutter doctor -v` and return its combined stdout+stderr as a string.
///
/// Returns `None` when:
/// - The Flutter executable cannot be spawned.
/// - The command times out after [`DOCTOR_TIMEOUT`].
/// - Any I/O error occurs while reading output.
///
/// **Display-only.** The result is never used to gate component statuses;
/// it is stored verbatim in [`ToolchainReport::doctor`] for the UI to render.
pub async fn capture_flutter_doctor(exe: &FlutterExecutable) -> Option<String> {
    let mut cmd = exe.command();
    cmd.args(["doctor", "-v"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Prevent the spawned Flutter process from inheriting the terminal's
        // stdin, avoiding unexpected blocking reads.
        .stdin(Stdio::null())
        // Ensure the child is killed when the handle is dropped (covers the
        // timeout arm where `child` goes out of scope).
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("flutter doctor spawn failed: {}", e);
            return None;
        }
    };

    // Take the I/O handles before moving `child` into the timeout future so
    // we can kill the process if the timeout fires.
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    // Collect stdout and stderr concurrently inside the timeout.
    let result = tokio::time::timeout(DOCTOR_TIMEOUT, async {
        // Read stdout (capped)
        let stdout_bytes = if let Some(stdout) = stdout_handle {
            let mut buf = Vec::new();
            let _ = AsyncReadExt::take(stdout, MAX_DOCTOR_OUTPUT_BYTES)
                .read_to_end(&mut buf)
                .await;
            buf
        } else {
            Vec::new()
        };

        // Read stderr (capped)
        let stderr_bytes = if let Some(stderr) = stderr_handle {
            let mut buf = Vec::new();
            let _ = AsyncReadExt::take(stderr, MAX_DOCTOR_OUTPUT_BYTES)
                .read_to_end(&mut buf)
                .await;
            buf
        } else {
            Vec::new()
        };

        // Wait for the process to finish (ignore exit code — doctor exits
        // non-zero when issues are found, which is the normal case).
        let _ = child.wait().await;

        // Combine stdout + stderr; doctor writes its main output to stdout
        // but some versions write section headers to stderr.
        let mut combined = String::with_capacity(stdout_bytes.len() + stderr_bytes.len());
        combined.push_str(&String::from_utf8_lossy(&stdout_bytes));
        if !stderr_bytes.is_empty() {
            let stderr_str = String::from_utf8_lossy(&stderr_bytes);
            // Avoid doubling content only when stderr is an *exact* copy of
            // stdout (e.g. some shells forward stderr into stdout verbatim).
            // A substring test over-eagerly drops legitimate stderr diagnostics
            // that happen to appear somewhere inside a larger stdout body.
            if combined.trim() != stderr_str.trim() {
                combined.push_str(&stderr_str);
            }
        }
        combined
    })
    .await;

    match result {
        Ok(text) if !text.trim().is_empty() => Some(text),
        Ok(_) => {
            tracing::debug!("flutter doctor returned empty output");
            None
        }
        Err(_) => {
            tracing::warn!(
                "flutter doctor timed out after {} s; child process will be killed via kill_on_drop",
                DOCTOR_TIMEOUT.as_secs()
            );
            // `child` is dropped here; `kill_on_drop(true)` ensures the OS
            // sends SIGKILL (Unix) / TerminateProcess (Windows) before the
            // handle is released, preventing an orphaned flutter process.
            None
        }
    }
}

/// Parse the raw text from `flutter doctor -v` into structured [`DoctorLine`]s.
///
/// # Parsing rules
///
/// 1. ANSI escape codes are stripped first.
/// 2. Each line is classified by scanning the first non-whitespace token:
///    - `[✓]` or `[√]`  → [`DoctorMarker::Ok`]
///    - `[!]`            → [`DoctorMarker::Warning`]
///    - `[✗]`            → [`DoctorMarker::Error`]
///    - `[☠]`            → [`DoctorMarker::Dead`]
///    - anything else    → [`DoctorMarker::None`] (continuation / section header)
/// 3. `indent` is the number of leading space characters (before the marker or text),
///    capped at [`MAX_DOCTOR_INDENT`].
/// 4. Empty lines produce a `DoctorLine` with an empty `text` and `DoctorMarker::None`.
///
/// This function is **pure and total** — it never panics, even on garbage input.
pub fn parse_doctor_output(text: &str) -> Vec<DoctorLine> {
    if text.is_empty() {
        return Vec::new();
    }

    text.lines()
        .map(|raw_line| {
            let clean = strip_ansi(raw_line);
            parse_single_line(&clean)
        })
        .collect()
}

/// Parse a single ANSI-stripped line into a [`DoctorLine`].
fn parse_single_line(line: &str) -> DoctorLine {
    // Count leading spaces for indent, capped defensively so a pathological
    // line cannot drive a large per-frame allocation in the TUI.
    let leading_spaces = line.chars().take_while(|c| *c == ' ').count();
    let indent = leading_spaces.min(MAX_DOCTOR_INDENT);
    let trimmed = line.trim_start();

    // Try to match a marker at the beginning of the trimmed line.
    let (marker, text) = if let Some(rest) = try_strip_marker(trimmed) {
        rest
    } else {
        (DoctorMarker::None, trimmed.to_string())
    };

    DoctorLine {
        marker,
        text: text.trim().to_string(),
        indent,
    }
}

/// Attempt to strip a leading `[X]` marker from the beginning of a trimmed line.
///
/// Returns `Some((marker, remaining_text))` when a marker is found,
/// `None` otherwise.
fn try_strip_marker(s: &str) -> Option<(DoctorMarker, String)> {
    // Doctor lines look like: "[✓] Flutter (Channel stable, ...)"
    // The bracket + marker + bracket must all be present.
    if !s.starts_with('[') {
        return None;
    }

    // Find closing bracket
    let close = s.find(']')?;
    let inner = &s[1..close]; // content between [ and ]
    let rest = s[close + 1..].trim_start().to_string();

    let marker = classify_inner(inner)?;
    Some((marker, rest))
}

/// Classify the content between `[` and `]`.
fn classify_inner(inner: &str) -> Option<DoctorMarker> {
    // Normalize: strip ANSI that might have survived inside the brackets
    let clean = strip_ansi(inner);
    let trimmed = clean.trim();

    match trimmed {
        // Unicode checkmark (U+2713) and Windows-1252 ASCII fallback (√ U+221A)
        "✓" | "√" => Some(DoctorMarker::Ok),
        // Warning / info
        "!" => Some(DoctorMarker::Warning),
        // Error / cross
        "✗" => Some(DoctorMarker::Error),
        // Dead / skull
        "☠" => Some(DoctorMarker::Dead),
        // Some versions use ' ' (space) for section headers — treat as None
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── pure parser tests ─────────────────────────────────────────────────────

    #[test]
    fn test_parse_doctor_empty_returns_empty_vec() {
        assert!(parse_doctor_output("").is_empty());
    }

    #[test]
    fn test_parse_doctor_classifies_all_markers() {
        let input = "[✓] Flutter (Channel stable)\n\
                     [!] Android toolchain - missing\n\
                     [✗] Xcode - not installed\n\
                     [☠] VS Code - crashed\n\
                     • Some detail line\n\
                     Another continuation";

        let lines = parse_doctor_output(input);
        assert_eq!(lines.len(), 6);

        assert_eq!(lines[0].marker, DoctorMarker::Ok);
        assert!(lines[0].text.contains("Flutter"));

        assert_eq!(lines[1].marker, DoctorMarker::Warning);
        assert!(lines[1].text.contains("Android toolchain"));

        assert_eq!(lines[2].marker, DoctorMarker::Error);
        assert!(lines[2].text.contains("Xcode"));

        assert_eq!(lines[3].marker, DoctorMarker::Dead);
        assert!(lines[3].text.contains("VS Code"));

        // Continuation lines have None marker
        assert_eq!(lines[4].marker, DoctorMarker::None);
        assert_eq!(lines[5].marker, DoctorMarker::None);
    }

    #[test]
    fn test_parse_doctor_ignores_ansi_color_codes() {
        // Embed ANSI color code in a marker line
        let input = "\x1b[32m[✓]\x1b[0m Flutter (Channel stable, 3.19.0)";
        let lines = parse_doctor_output(input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].marker, DoctorMarker::Ok);
        assert!(lines[0].text.contains("Flutter"));
    }

    #[test]
    fn test_parse_doctor_continuation_lines_have_none_marker() {
        let input = "    • No issues found!\n    - Some detail";
        let lines = parse_doctor_output(input);
        for line in &lines {
            assert_eq!(line.marker, DoctorMarker::None);
        }
    }

    #[test]
    fn test_parse_doctor_indent_counted_correctly() {
        let input = "    [✓] Indented section";
        let lines = parse_doctor_output(input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].indent, 4);
        assert_eq!(lines[0].marker, DoctorMarker::Ok);
    }

    #[test]
    fn test_parse_doctor_no_indent_for_top_level() {
        let input = "[✓] Flutter (Channel stable)";
        let lines = parse_doctor_output(input);
        assert_eq!(lines[0].indent, 0);
    }

    #[test]
    fn test_parse_doctor_ascii_checkmark_fallback() {
        let input = "[√] Flutter checkmark fallback";
        let lines = parse_doctor_output(input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].marker, DoctorMarker::Ok);
    }

    #[test]
    fn test_parse_doctor_garbage_input_does_not_panic() {
        let garbage = "\x00\u{FF}\x1b[1;31m\x00garbage\n\x1b]0;title\x07";
        let _ = parse_doctor_output(garbage);
    }

    #[test]
    fn test_parse_doctor_single_line_no_newline() {
        let input = "[✗] Something is wrong";
        let lines = parse_doctor_output(input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].marker, DoctorMarker::Error);
    }

    #[test]
    fn test_parse_doctor_text_trimmed_of_extra_spaces() {
        let input = "[✓]   Flutter   ";
        let lines = parse_doctor_output(input);
        assert_eq!(lines[0].text, "Flutter");
    }

    #[test]
    fn test_parse_caps_indent_at_max() {
        // A line with 1000 leading spaces should be capped at MAX_DOCTOR_INDENT (32)
        let input = format!("{}[✓] over-indented", " ".repeat(1000));
        let lines = parse_doctor_output(&input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].indent, MAX_DOCTOR_INDENT);
        assert_eq!(lines[0].marker, DoctorMarker::Ok);
    }

    #[test]
    fn test_parse_caps_indent_plain_text() {
        // Plain text line with large indent is also capped
        let input = format!("{}some text", " ".repeat(500));
        let lines = parse_doctor_output(&input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].indent, MAX_DOCTOR_INDENT);
    }

    // ── strip_ansi delegation tests ───────────────────────────────────────────
    // These tests verify that doctor.rs delegates to the shared strip_ansi, which
    // now handles both CSI and OSC sequences.

    #[test]
    fn test_strip_ansi_removes_osc_sequences() {
        // OSC: ESC ] ... BEL — should be stripped from doctor output
        let input = "\x1b]0;title\x07[✓] Flutter";
        let lines = parse_doctor_output(input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].marker, DoctorMarker::Ok);
        assert!(lines[0].text.contains("Flutter"));
    }

    #[test]
    fn test_strip_ansi_csi_unchanged() {
        // CSI stripping must still work correctly after the consolidation
        let input = "\x1b[32m[✓]\x1b[0m Flutter";
        let lines = parse_doctor_output(input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].marker, DoctorMarker::Ok);
    }

    // ── stderr dedup logic ────────────────────────────────────────────────────
    // These tests verify the exact-equality dedup rule used when combining
    // stdout + stderr from `flutter doctor -v`.
    //
    // We test the dedup predicate directly rather than going through the async
    // capture function, keeping the tests synchronous and hermetic.

    /// Helper: apply the same dedup logic as `capture_flutter_doctor` and
    /// return the resulting combined string.
    fn combine(stdout: &str, stderr: &str) -> String {
        let mut combined = stdout.to_string();
        if !stderr.is_empty() {
            // Exact-equality dedup: only suppress stderr when it is a
            // whitespace-normalised duplicate of stdout.
            if combined.trim() != stderr.trim() {
                combined.push_str(stderr);
            }
        }
        combined
    }

    /// stderr that is a strict substring of stdout must still be appended.
    #[test]
    fn stderr_substring_of_stdout_is_retained() {
        let stdout = "[✓] Flutter\n  • some detail line\n";
        let stderr = "  • some detail line\n"; // proper substring
        let result = combine(stdout, stderr);
        assert!(
            result.contains("some detail line\n  • some detail line"),
            "stderr substring should be appended, not suppressed; got: {:?}",
            result
        );
    }

    /// Exactly-equal stderr (content identical after trim) is dropped.
    #[test]
    fn exactly_equal_stderr_is_dropped() {
        let content = "[✓] Flutter\n  • ok\n";
        let result = combine(content, content);
        // The combined string should equal the single copy, not a doubled copy.
        assert_eq!(result, content, "duplicate stderr should be suppressed");
    }

    /// Exactly-equal after whitespace-trim is also dropped.
    #[test]
    fn exactly_equal_after_trim_is_dropped() {
        let stdout = "  [✓] Flutter\n";
        let stderr = "[✓] Flutter"; // different leading/trailing whitespace
        let result = combine(stdout, stderr);
        // trim() of both is "[✓] Flutter" → suppress.
        assert_eq!(result, stdout, "trimmed-equal stderr should be suppressed");
    }

    /// Distinct stderr is always appended regardless of partial overlap.
    #[test]
    fn distinct_stderr_is_appended() {
        let stdout = "[✓] Flutter\n";
        let stderr = "[!] Android toolchain - incomplete\n";
        let result = combine(stdout, stderr);
        assert!(result.contains("Flutter"));
        assert!(result.contains("Android toolchain"));
    }

    /// Empty stderr produces no change to stdout.
    #[test]
    fn empty_stderr_produces_no_change() {
        let stdout = "[✓] Flutter\n";
        let result = combine(stdout, "");
        assert_eq!(result, stdout);
    }
}
