//! # Assertion Helpers
//!
//! Assertion functions for SDK detection results and a parser for the
//! newline-delimited JSON (NDJSON) output produced by headless mode.
//!
//! All assertion functions panic with a descriptive message on failure so that
//! test output is self-explanatory without digging into raw `assert_eq!` diffs.

use std::path::Path;

use fdemon_core::prelude::*;
use fdemon_daemon::flutter_sdk::{FlutterSdk, SdkSource};

// ─────────────────────────────────────────────────────────────────────────────
// SDK Assertion Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Assert that `sdk` was resolved from `expected_source`.
///
/// Compares the `source` field via [`PartialEq`].  On failure the panic
/// message includes both the actual and expected sources.
///
/// # Panics
///
/// Panics when `sdk.source != *expected_source`.
pub fn assert_sdk_source(sdk: &FlutterSdk, expected_source: &SdkSource) {
    assert_eq!(
        &sdk.source, expected_source,
        "SDK source mismatch — got {:?}, expected {:?}",
        sdk.source, expected_source,
    );
}

/// Assert that `sdk.root` equals `expected_root`.
///
/// The comparison is performed on canonicalized paths so that platform-specific
/// symlink chains (e.g. macOS `/var` → `/private/var`) do not cause false
/// negatives.  If canonicalization fails for either path the raw paths are
/// compared instead.
///
/// # Panics
///
/// Panics when the canonicalized (or raw) SDK root does not match
/// `expected_root`.
pub fn assert_sdk_root(sdk: &FlutterSdk, expected_root: &Path) {
    let actual = std::fs::canonicalize(&sdk.root).unwrap_or_else(|_| sdk.root.clone());
    let expected =
        std::fs::canonicalize(expected_root).unwrap_or_else(|_| expected_root.to_path_buf());

    assert_eq!(
        actual,
        expected,
        "SDK root mismatch — got {}, expected {}",
        actual.display(),
        expected.display(),
    );
}

/// Assert that `result` is an `Err` containing [`Error::FlutterNotFound`].
///
/// # Panics
///
/// Panics when `result` is `Ok(_)` or when the error is not
/// [`Error::FlutterNotFound`].
pub fn assert_sdk_not_found(result: &Result<FlutterSdk>) {
    match result {
        Ok(sdk) => panic!(
            "Expected FlutterNotFound error but got an SDK: source={:?}, root={}",
            sdk.source,
            sdk.root.display()
        ),
        Err(Error::FlutterNotFound) => {}
        Err(other) => panic!("Expected FlutterNotFound error but got a different error: {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Headless NDJSON Event Parser
// ─────────────────────────────────────────────────────────────────────────────

/// A single parsed NDJSON event from headless mode stdout.
///
/// Headless mode emits one JSON object per line.  Each object has at minimum an
/// `"event"` string field and a `"timestamp"` field.
#[derive(Debug)]
pub struct HeadlessEvent {
    /// The event name, e.g. `"daemon_connected"`, `"app_started"`, `"error"`.
    pub event: String,
    /// Optional `"message"` field (present on `log` and `error` events).
    pub message: Option<String>,
    /// Optional `"fatal"` field (present on `error` events).
    pub fatal: Option<bool>,
}

/// Assert that no fatal error event was emitted in the collected headless
/// events.
///
/// A fatal error indicates that fdemon could not start normally — most
/// commonly because it failed to locate the Flutter SDK.  Any test that
/// expects successful SDK detection should call this helper after
/// [`parse_headless_events`].
///
/// # Panics
///
/// Panics when at least one `{"event":"error","fatal":true}` object is found
/// in `events`.  The panic message includes the full list of offending events
/// so the cause can be diagnosed from the test output alone.
pub fn assert_no_fatal_sdk_error(events: &[HeadlessEvent]) {
    let fatal_errors: Vec<_> = events
        .iter()
        .filter(|e| e.event == "error" && e.fatal == Some(true))
        .collect();
    assert!(
        fatal_errors.is_empty(),
        "Unexpected fatal errors in headless output: {:?}",
        fatal_errors
    );
}

/// Parse NDJSON output from headless mode into a vector of [`HeadlessEvent`]s.
///
/// Each non-blank line is expected to be a self-contained JSON object.  Lines
/// that cannot be parsed as JSON are silently skipped so that interleaved
/// non-JSON output (such as tracing log lines written to stderr when redirected
/// to stdout) does not cause the test to fail prematurely.
///
/// # Arguments
///
/// * `stdout` — The complete stdout captured from a headless mode run.  May
///   contain multiple newline-separated JSON objects.
///
/// # Returns
///
/// A `Vec<HeadlessEvent>` in the same order as the lines in `stdout`.
pub fn parse_headless_events(stdout: &str) -> Vec<HeadlessEvent> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            let obj = value.as_object()?;

            let event = obj.get("event")?.as_str()?.to_string();
            let message = obj
                .get("message")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let fatal = obj.get("fatal").and_then(|v| v.as_bool());

            Some(HeadlessEvent {
                event,
                message,
                fatal,
            })
        })
        .collect()
}
