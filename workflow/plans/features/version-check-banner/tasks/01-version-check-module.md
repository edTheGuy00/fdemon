## Task: Add HTTP client and version_check module

**Objective**: Add `reqwest` (rustls-tls) as a workspace dependency and create a new `fdemon-app::version_check` module that, when called, queries GitHub for the latest fdemon release and returns `Some(tag)` if it is newer than the current crate version.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `Cargo.toml` (workspace): Add `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }` under `[workspace.dependencies]` to centralize the version pin.
- `crates/fdemon-app/Cargo.toml`: Add `reqwest.workspace = true` under `[dependencies]`.
- `crates/fdemon-app/src/version_check.rs` (NEW): Async function `check_for_newer_release() -> Option<String>` plus private helpers `parse_semver` and a numeric-triple comparator.
- `crates/fdemon-app/src/lib.rs`: Add `pub mod version_check;`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/actions/ready_check.rs`: Reference for in-tree timeout / network-error handling style.
- `Cargo.toml` (workspace) line 7: Read `version = "0.5.4"` to confirm `CARGO_PKG_VERSION` value (the module will use `env!("CARGO_PKG_VERSION")` at compile time).

### Details

**Module shape** (`crates/fdemon-app/src/version_check.rs`):

```rust
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

const RELEASES_ENDPOINT: &str =
    "https://api.github.com/repos/edTheGuy00/fdemon/releases/latest";
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
        tracing::debug!(
            "Version check: GitHub returned {}",
            response.status()
        );
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
```

**Why `reqwest` and not the raw-TCP pattern in `ready_check.rs`:** `ready_check.rs` only handles plaintext HTTP. The GitHub releases endpoint is HTTPS-only, and there is no TLS crate in the dependency tree today. `reqwest` with `rustls-tls` adds vendored TLS without pulling in system OpenSSL.

**Why `serde_json::Value` and not a typed struct:** the response has dozens of fields and the schema is owned by GitHub. Reading the single `tag_name` field via `Value` avoids defining a 30-field struct that we'd never use.

### Acceptance Criteria

1. `cargo build -p fdemon-app` succeeds.
2. `cargo test -p fdemon-app version_check` runs the unit tests and they pass.
3. `parse_semver("0.5.4")` returns `Some((0, 5, 4))`; `parse_semver("v0.5.4")` returns `None` (leading `v` is rejected — callers must strip it first); `parse_semver("0.5")` returns `None`; `parse_semver("0.5.4-beta")` returns `None`.
4. The triple-comparison `(0, 6, 0) > (0, 5, 4)` is `true`; `(0, 5, 4) > (0, 5, 4)` is `false`; `(0, 5, 4) > (0, 6, 0)` is `false`.
5. No code anywhere yet calls `check_for_newer_release` — it is a leaf module that compiles and tests cleanly in isolation. Wiring happens in task 04.

### Testing

Unit tests live inline at the bottom of `version_check.rs`:

```rust
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
        assert!(!((0, 5, 4) > (0, 5, 4)));
        assert!(!((0, 5, 4) > (0, 6, 0)));
    }
}
```

**Deliberately not tested:** the live network path. `check_for_newer_release` is an integration concern best validated by manual smoke test on the implementation branch. Network tests are flaky in CI and rate-limit-prone against `api.github.com`.

### Notes

- The `User-Agent` header is required by GitHub's API — requests without UA get a `403 Forbidden`.
- 3-second `timeout` covers both connect and total request time in `reqwest`.
- `reqwest::Client::builder()` is cheap to construct per-call here — only one call per process. No need to share a `Client`.
- Workspace `Cargo.toml` change is small and additive; doesn't conflict with any other task's `Cargo.toml` edits in this plan.

---

## Completion Summary

**Status:** Done
**Branch:** feat/version-check-banner

### Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` (workspace) | Added `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }` under `[workspace.dependencies]` |
| `crates/fdemon-app/Cargo.toml` | Added `reqwest.workspace = true` under `[dependencies]` |
| `crates/fdemon-app/src/version_check.rs` | New module: `check_for_newer_release()` async function + `parse_semver()` helper + 6 unit tests |
| `crates/fdemon-app/src/lib.rs` | Added `pub mod version_check;` |

### Notable Decisions/Tradeoffs

1. **Clippy `nonminimal_bool` fix in tests**: The task spec used `assert!(!((0,5,4) > (0,5,4)))` but clippy -D warnings rejects negated boolean comparisons. Changed to equivalent `assert!((0,5,4) <= (0,5,4))` — semantically identical, clippy-clean.
2. **`reqwest` 0.12 pinned in workspace**: Using `default-features = false` with `rustls-tls` + `json` avoids pulling in native OpenSSL and limits feature surface to what the module needs.

### Testing Performed

- `cargo build -p fdemon-app` — Passed
- `cargo test -p fdemon-app version_check` — Passed (6 tests)
- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **No live network test**: `check_for_newer_release` is not covered by unit tests — only `parse_semver` and tuple comparison are tested. This matches the task spec: "Deliberately not tested: the live network path."
