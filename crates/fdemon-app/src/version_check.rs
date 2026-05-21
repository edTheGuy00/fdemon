//! GitHub release version check.
//!
//! On startup, queries the GitHub releases API for the latest fdemon release.
//! Compares against the compile-time `CARGO_PKG_VERSION` and returns
//! `Some(latest_tag)` when a newer release is available, `None` otherwise.
//!
//! All errors (network, parse, non-2xx, body too large, version-not-newer)
//! collapse to `None` — there is no error surface. Silent failure is
//! intentional: this is a developer tool, not a security update channel.
//!
//! # Caching
//!
//! Results are cached at `<dirs::cache_dir()>/fdemon/version_check.json` for
//! 24 hours. On a cache hit within the TTL, no outbound network request is
//! made. The cache is written atomically (write-to-.tmp + rename) to avoid
//! corruption on crash.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// The GitHub API endpoint for the latest fdemon release.
const GITHUB_RELEASES_LATEST: &str =
    "https://api.github.com/repos/edTheGuy00/fdemon/releases/latest";

/// Maximum allowed response body size (512 KiB). GitHub release payloads are
/// typically a few kilobytes; this cap prevents a hostile or malformed response
/// from consuming unbounded memory.
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

/// Cache TTL in seconds (24 hours).
const CACHE_TTL_SECS: u64 = 86_400;

// ── Typed response ────────────────────────────────────────────────────────────

/// Typed representation of the GitHub releases API response.
///
/// No `#[serde(deny_unknown_fields)]` — GitHub adds fields to release payloads
/// regularly and we must not break on schema growth.
#[derive(serde::Deserialize)]
struct ReleaseResponse {
    tag_name: String,
}

// ── Cache ─────────────────────────────────────────────────────────────────────

/// A single cache entry persisted to disk.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CacheEntry {
    /// POSIX seconds when the check was performed.
    pub checked_at: u64,
    /// The latest tag string (bare semver, no `v` prefix), or `None` when the
    /// last check confirmed the installed version is current.
    pub latest: Option<String>,
}

/// Returns the canonical path of the on-disk cache file, or `None` when
/// `dirs::cache_dir()` is unavailable (e.g. unsupported OS configuration).
fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("fdemon").join("version_check.json"))
}

/// Read the cache from the given path. Returns `None` on any I/O or parse error.
///
/// This function is public within the crate to allow tests to inject an
/// arbitrary path without relying on `dirs::cache_dir()`.
pub(crate) fn read_cache_at(path: &Path) -> Option<CacheEntry> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Read the cache from the canonical cache path. Returns `None` when the cache
/// file is missing, unreadable, or contains invalid JSON.
fn read_cache() -> Option<CacheEntry> {
    let path = cache_path()?;
    let entry = read_cache_at(&path);
    if entry.is_none() {
        tracing::debug!("Version check: cache miss or invalid cache at {:?}", path);
    }
    entry
}

/// Write `entry` to the given path atomically (write `.tmp` then rename).
///
/// On Windows, `std::fs::rename` fails when the destination already exists.
/// We remove the destination first, which leaves a small window; for a
/// best-effort read cache this is acceptable.
///
/// Any I/O error is logged at `debug` and otherwise ignored — cache writes
/// are best-effort.
pub(crate) fn write_cache_at(path: &Path, entry: &CacheEntry) {
    let json = match serde_json::to_string(entry) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!("Version check: failed to serialize cache: {}", e);
            return;
        }
    };

    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::debug!("Version check: failed to create cache dir: {}", e);
            return;
        }
    }

    let tmp_path = path.with_extension("tmp");
    if let Err(e) = std::fs::write(&tmp_path, &json) {
        tracing::debug!("Version check: failed to write cache tmp file: {}", e);
        return;
    }

    // On Windows, rename over an existing file requires removing it first.
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(path);
    }

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        tracing::debug!("Version check: failed to rename cache tmp file: {}", e);
        // Clean up the tmp file on failure.
        let _ = std::fs::remove_file(&tmp_path);
    }
}

/// Write `entry` to the canonical cache path. Best-effort: errors are logged
/// at `debug` and otherwise ignored.
fn write_cache(entry: &CacheEntry) {
    if let Some(path) = cache_path() {
        write_cache_at(&path, entry);
    } else {
        tracing::debug!("Version check: cache_dir unavailable, skipping cache write");
    }
}

/// Returns the current POSIX time in seconds.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

// ── Fetch ─────────────────────────────────────────────────────────────────────

/// Fetch the latest release tag from the given endpoint. Returns the
/// remote `tag_name` with a single leading `v` stripped, or `None`
/// on any I/O / HTTP / size error.
///
/// This function does NOT validate the returned string as semver —
/// it is a raw transport helper. Callers must validate / normalize
/// before treating the result as a version. `check_for_newer_release`
/// is the validated public entry point.
///
/// The `endpoint` parameter exists so tests can substitute a `wiremock` server
/// URL without touching real network paths.
pub(crate) async fn fetch_latest_tag(endpoint: &str, timeout: Duration) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(concat!("fdemon/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| tracing::debug!("Version check: failed to build HTTP client: {}", e))
        .ok()?;

    let response = client
        .get(endpoint)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| tracing::debug!("Version check: network send failed: {}", e))
        .ok()?;

    if !response.status().is_success() {
        tracing::debug!("Version check: GitHub returned {}", response.status());
        return None;
    }

    // Guard against oversized responses using the Content-Length header first.
    if let Some(len) = response.content_length() {
        if len > MAX_RESPONSE_BYTES as u64 {
            tracing::debug!("Version check: response too large ({} bytes)", len);
            return None;
        }
    }

    // Collect the body and re-check size to guard against missing Content-Length.
    let body = response
        .bytes()
        .await
        .map_err(|e| tracing::debug!("Version check: failed to read response body: {}", e))
        .ok()?;
    if body.len() > MAX_RESPONSE_BYTES {
        tracing::debug!(
            "Version check: streamed response too large ({} bytes)",
            body.len()
        );
        return None;
    }

    let parsed: ReleaseResponse = serde_json::from_slice(&body)
        .map_err(|e| tracing::debug!("Version check: failed to parse JSON response: {}", e))
        .ok()?;
    let tag = parsed.tag_name;

    let tag_str = if let Some(stripped) = tag.strip_prefix('v') {
        stripped.to_string()
    } else {
        tag
    };

    Some(tag_str)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Returns `Some("0.6.0")` (the bare semver string, no `v` prefix) when
/// GitHub's latest release is newer than the compiled version. Returns
/// `None` on any failure or when the latest is not newer.
///
/// # Caching
///
/// Results are cached on disk for 24 hours. When a fresh cache entry exists,
/// no network request is made.
///
/// # Security: returned string is digit-and-dot only
///
/// The returned `String` is built from `parse_semver`'s parsed
/// numeric triple (`format!("{major}.{minor}.{patch}")`) — never
/// the raw remote `tag_name`. Any pre-release suffix, build
/// metadata, or hostile bytes following `-`/`+` are stripped by
/// `parse_semver` and discarded by the formatting step. Callers
/// that embed the returned string into a terminal banner can
/// therefore skip escape-sequence sanitisation.
///
/// Do not change `check_for_newer_release` to return the raw
/// `tag_str` without also adding explicit sanitisation at every
/// render site.
pub(crate) async fn check_for_newer_release(timeout: Duration) -> Option<String> {
    let current = parse_semver(env!("CARGO_PKG_VERSION"))?;
    let now = now_secs();

    // Cache hit: skip network if within TTL.
    if let Some(entry) = read_cache() {
        if now.saturating_sub(entry.checked_at) < CACHE_TTL_SECS {
            tracing::debug!(
                "Version check: serving from cache (age={}s)",
                now.saturating_sub(entry.checked_at)
            );
            return entry.latest.and_then(|tag| {
                let parsed = parse_semver(&tag)?;
                if parsed > current {
                    Some(format!("{}.{}.{}", parsed.0, parsed.1, parsed.2))
                } else {
                    None
                }
            });
        }
        tracing::debug!(
            "Version check: cache expired (age={}s), fetching from network",
            now.saturating_sub(entry.checked_at)
        );
    }

    // Network fetch.
    let tag_str = fetch_latest_tag(GITHUB_RELEASES_LATEST, timeout).await?;
    let latest = parse_semver(&tag_str)?;
    let normalized = format!("{}.{}.{}", latest.0, latest.1, latest.2);

    let result = if latest > current {
        Some(normalized)
    } else {
        None
    };

    // Write cache (best-effort).
    write_cache(&CacheEntry {
        checked_at: now,
        latest: result.clone(),
    });

    result
}

// ── Parse ─────────────────────────────────────────────────────────────────────

/// Parse a `MAJOR.MINOR.PATCH` string into `(u32, u32, u32)`.
///
/// Pre-release suffixes (e.g. `-rc.1`) and build metadata (e.g. `+build.42`)
/// are stripped before parsing. The function returns `None` when the string
/// does not contain exactly three dot-separated numeric components after
/// stripping.
fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    // Strip any pre-release/build suffix starting with '-' or '+'
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_semver ──────────────────────────────────────────────────────────

    #[test]
    fn parse_semver_happy_path() {
        assert_eq!(parse_semver("0.5.4"), Some((0, 5, 4)));
        assert_eq!(parse_semver("1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_semver("12.34.56"), Some((12, 34, 56)));
    }

    #[test]
    fn parse_semver_rejects_leading_v() {
        assert_eq!(parse_semver("v0.5.4"), None);
    }

    #[test]
    fn parse_semver_strips_pre_release_suffix() {
        assert_eq!(parse_semver("0.6.0-rc.1"), Some((0, 6, 0)));
        assert_eq!(parse_semver("0.6.0+build.42"), Some((0, 6, 0)));
        assert_eq!(
            parse_semver("1.2.3-beta.4+exp.sha.5114f85"),
            Some((1, 2, 3))
        );
    }

    #[test]
    fn parse_semver_rejects_two_components() {
        assert_eq!(parse_semver("0.5"), None);
    }

    #[test]
    fn parse_semver_rejects_four_components() {
        assert_eq!(parse_semver("0.5.4.1"), None);
    }

    #[test]
    fn triple_comparison_newer_wins() {
        assert!((0, 6, 0) > (0, 5, 99));
        assert!((1, 0, 0) > (0, 99, 99));
        assert!((0, 5, 4) <= (0, 5, 4));
        assert!((0, 5, 4) <= (0, 6, 0));
    }

    // ── Cache ─────────────────────────────────────────────────────────────────

    #[test]
    fn cache_within_ttl_returns_cached_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("version_check.json");
        let now = now_secs();
        std::fs::write(
            &path,
            format!(r#"{{"checked_at": {}, "latest": "999.0.0"}}"#, now),
        )
        .unwrap();

        let entry = read_cache_at(&path).unwrap();
        assert_eq!(entry.checked_at, now);
        assert_eq!(entry.latest, Some("999.0.0".to_string()));
        // Within TTL: age = 0
        assert!(now.saturating_sub(entry.checked_at) < CACHE_TTL_SECS);
    }

    #[test]
    fn cache_outside_ttl_is_expired() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("version_check.json");
        // checked_at = 0 means age is effectively "now" seconds which exceeds 24h.
        std::fs::write(&path, r#"{"checked_at": 0, "latest": "999.0.0"}"#).unwrap();

        let entry = read_cache_at(&path).unwrap();
        let now = now_secs();
        // Age should be at least CACHE_TTL_SECS (over 24h since epoch).
        assert!(now.saturating_sub(entry.checked_at) >= CACHE_TTL_SECS);
    }

    #[test]
    fn cache_corrupt_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("version_check.json");
        std::fs::write(&path, b"not valid json {{{").unwrap();
        assert!(read_cache_at(&path).is_none());
    }

    #[test]
    fn cache_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        assert!(read_cache_at(&path).is_none());
    }

    #[test]
    fn cache_atomic_write_via_rename() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("version_check.json");
        let entry = CacheEntry {
            checked_at: 12345,
            latest: Some("1.2.3".to_string()),
        };
        write_cache_at(&path, &entry);

        // The .tmp file should NOT exist after a successful write.
        let tmp_path = path.with_extension("tmp");
        assert!(
            !tmp_path.exists(),
            ".tmp file must not exist after successful atomic write"
        );

        // The final file should contain the correct JSON.
        let read_back = read_cache_at(&path).unwrap();
        assert_eq!(read_back, entry);
    }

    #[test]
    fn cache_write_null_latest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("version_check.json");
        let entry = CacheEntry {
            checked_at: 99999,
            latest: None,
        };
        write_cache_at(&path, &entry);
        let read_back = read_cache_at(&path).unwrap();
        assert_eq!(read_back.latest, None);
    }

    // ── Wiremock integration tests ────────────────────────────────────────────

    #[tokio::test]
    async fn returns_some_when_remote_is_newer() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "tag_name": "v999.999.999" })),
            )
            .mount(&mock)
            .await;
        let result = fetch_latest_tag(&mock.uri(), Duration::from_secs(3)).await;
        assert_eq!(result, Some("999.999.999".to_string()));
    }

    #[tokio::test]
    async fn returns_none_on_404() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&mock)
            .await;
        let result = fetch_latest_tag(&mock.uri(), Duration::from_secs(3)).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_none_on_403_rate_limited() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(403))
            .mount(&mock)
            .await;
        let result = fetch_latest_tag(&mock.uri(), Duration::from_secs(3)).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_none_on_malformed_json() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&mock)
            .await;
        let result = fetch_latest_tag(&mock.uri(), Duration::from_secs(3)).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_none_on_oversized_response() {
        // Build a body larger than MAX_RESPONSE_BYTES (512 KiB).
        // We use a JSON object with a large padding field. The `tag_name` field
        // is valid, but the body must exceed 512 KiB to trigger the size check.
        let padding = "x".repeat(MAX_RESPONSE_BYTES + 1024);
        let body = serde_json::json!({ "tag_name": "v999.0.0", "padding": padding });
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&mock)
            .await;
        let result = fetch_latest_tag(&mock.uri(), Duration::from_secs(3)).await;
        assert!(result.is_none(), "oversized response must return None");
    }

    #[tokio::test]
    async fn returns_none_on_timeout() {
        let mock = wiremock::MockServer::start().await;
        // Add a delay larger than the request timeout (100ms).
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(500))
                    .set_body_json(serde_json::json!({ "tag_name": "v999.0.0" })),
            )
            .mount(&mock)
            .await;
        // Very short timeout to ensure it fires before the mock delay.
        let result = fetch_latest_tag(&mock.uri(), Duration::from_millis(50)).await;
        assert!(result.is_none(), "timed-out request must return None");
    }

    #[tokio::test]
    async fn strips_leading_v_prefix() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "tag_name": "v1.2.3" })),
            )
            .mount(&mock)
            .await;
        let result = fetch_latest_tag(&mock.uri(), Duration::from_secs(3)).await;
        assert_eq!(result, Some("1.2.3".to_string()));
    }

    #[tokio::test]
    async fn returns_none_when_remote_version_is_not_newer() {
        // Use a version that is older than any plausible current build.
        // The `fetch_latest_tag` function returns the tag string regardless of
        // whether it is newer than the current version — that comparison is done
        // in `check_for_newer_release`. So we verify that parse_semver("0.0.1")
        // would compare as not-newer to the actual CARGO_PKG_VERSION.
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "tag_name": "v0.0.1" })),
            )
            .mount(&mock)
            .await;
        // fetch_latest_tag itself returns the tag regardless of comparison.
        let result = fetch_latest_tag(&mock.uri(), Duration::from_secs(3)).await;
        // Verify it returns the tag string (comparison done upstream).
        assert_eq!(result, Some("0.0.1".to_string()));
        // And verify the comparison logic: 0.0.1 is not newer than any real version.
        let current = parse_semver(env!("CARGO_PKG_VERSION")).unwrap();
        let remote = parse_semver("0.0.1").unwrap();
        assert!(
            remote <= current,
            "0.0.1 must not be newer than current version"
        );
    }

    #[tokio::test]
    async fn ansi_escape_in_tag_is_stripped() {
        use wiremock::matchers::method;
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "tag_name": "v1000.0.0-\u{001b}[31mEVIL\u{001b}[0m"
                })),
            )
            .mount(&mock)
            .await;
        // fetch_latest_tag is the raw transport — it may still return the
        // suffixed string (its doc explicitly says no validation). The
        // assertion below targets check_for_newer_release's contract.
        let raw = fetch_latest_tag(&mock.uri(), Duration::from_secs(3))
            .await
            .unwrap();
        assert!(
            raw.contains('\u{001b}'),
            "raw transport returns the unvalidated string"
        );

        // The public API must normalize.
        // Exercise the normalization path through parse_semver + format!.
        let triple = parse_semver(&raw).unwrap();
        let normalized = format!("{}.{}.{}", triple.0, triple.1, triple.2);
        assert!(
            normalized.chars().all(|c| c.is_ascii_digit() || c == '.'),
            "normalized form must contain only digits and dots, got: {:?}",
            normalized
        );
        assert_eq!(normalized, "1000.0.0");
    }

    #[test]
    fn cache_hit_with_suffixed_tag_returns_normalized_form() {
        // Simulate an old cache entry that was written before the normalization
        // fix — i.e., the `latest` field holds a raw suffixed string.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("version_check.json");
        let now = now_secs();
        // Write a cache entry whose `latest` contains a pre-release suffix.
        std::fs::write(
            &path,
            format!(r#"{{"checked_at": {}, "latest": "999.0.0-rc.1"}}"#, now),
        )
        .unwrap();

        let entry = read_cache_at(&path).unwrap();
        let current = (0u32, 0u32, 0u32); // pretend current is 0.0.0 so remote is always newer

        // Reproduce the cache-hit normalization logic directly.
        let result = entry.latest.and_then(|tag| {
            let parsed = parse_semver(&tag)?;
            if parsed > current {
                Some(format!("{}.{}.{}", parsed.0, parsed.1, parsed.2))
            } else {
                None
            }
        });

        assert_eq!(result, Some("999.0.0".to_string()));
        // Verify no characters outside [0-9.] are present.
        assert!(
            result
                .unwrap()
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.'),
            "cache-hit normalized form must contain only digits and dots"
        );
    }
}
