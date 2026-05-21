//! GitHub release version check.
//!
//! On startup, queries the GitHub releases API for the latest fdemon release.
//! Compares against the compile-time `CARGO_PKG_VERSION` and returns
//! `Some(latest_tag)` when a newer release is available, `None` otherwise.
//!
//! All errors (network, parse, non-2xx, version-not-newer) collapse to
//! `None` — there is no error surface. Silent failure is intentional:
//! this is a developer tool, not a security update channel.

use std::time::Duration;

const RELEASES_ENDPOINT: &str = "https://api.github.com/repos/edTheGuy00/fdemon/releases/latest";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// Returns `Some("0.6.0")` (the bare semver string, no `v` prefix) when
/// GitHub's latest release is newer than the compiled version. Returns
/// `None` on any failure or when the latest is not newer.
pub async fn check_for_newer_release() -> Option<String> {
    let current = parse_semver(env!("CARGO_PKG_VERSION"))?;

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("fdemon/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

    let response = client
        .get(RELEASES_ENDPOINT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        tracing::debug!("Version check: GitHub returned {}", response.status());
        return None;
    }

    let body: serde_json::Value = response.json().await.ok()?;
    let tag = body.get("tag_name")?.as_str()?;

    let latest_str = tag.strip_prefix('v').unwrap_or(tag);
    let latest = parse_semver(latest_str)?;

    if latest > current {
        Some(latest_str.to_string())
    } else {
        None
    }
}

/// Parse a `MAJOR.MINOR.PATCH` string into `(u32, u32, u32)`.
///
/// Returns `None` for anything that does not match exactly three
/// dot-separated numeric components. Pre-release suffixes (e.g.
/// `1.0.0-beta`) are intentionally rejected — fdemon does not ship
/// pre-release tags today, and treating them as "not newer" is the
/// safe default if that ever changes.
fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    let patch = parts[2].parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_semver_rejects_pre_release() {
        assert_eq!(parse_semver("0.5.4-beta"), None);
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
}
