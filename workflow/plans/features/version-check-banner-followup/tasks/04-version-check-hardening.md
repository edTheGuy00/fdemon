## Task: Version-check hardening — cache, body cap, pre-release tolerance, integration tests, typed struct

**Objective**: Atomic refactor of `crates/fdemon-app/src/version_check.rs` to address six review findings together: add a per-user on-disk cache (M2), cap response body size (m1), tolerate pre-release tag suffixes (m6), rewrite `parse_semver` with iterator chaining (m2), introduce a typed `ReleaseResponse` struct (N11), and add `wiremock` integration tests for the eight-case matrix (m4). Plus several smaller nits in the same file: `REQUEST_TIMEOUT` rename (N3), uniform `tracing::debug!` (N4), no-redirect client policy (N6), sanitisation-invariant doc comment (N7), and a single-line orphan-task comment in `spawn.rs` (m7).

This task **must be atomic** — splitting it leaves the workspace in a half-refactored state and the cache code, body cap, and tests are all tightly coupled.

**Depends on**: None (in Wave 1)

**Agent:** implementor

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**

- `crates/fdemon-app/src/version_check.rs`: Substantial refactor. New module structure:
  - Public surface: `pub async fn check_for_newer_release(timeout: Duration) -> Option<String>` (returns the same opaque tag string as before, callers don't see cache plumbing).
  - Internal: split into `mod cache`, `mod fetch`, `mod parse` sub-modules **only if** the file would exceed 500 lines per `CODE_STANDARDS.md`. Single file is fine if it stays under that threshold.
  - Cache: `read_cache() -> Option<CacheEntry>` and `write_cache(entry: &CacheEntry)` using `dirs::cache_dir()?.join("fdemon").join("version_check.json")`. `CacheEntry { checked_at: u64, latest: Option<String> }`.
  - Fetch: `fetch_latest_tag(endpoint: &str, timeout: Duration) -> Option<String>` — injectable endpoint for tests. Caps body at 512 KB via `response.content_length()` check, then `response.text_with_charset()` (or just `response.bytes()` truncated). Builds `reqwest::Client` with `.redirect(reqwest::redirect::Policy::none())`.
  - Parse: `parse_semver(s: &str) -> Option<(u32, u32, u32)>` rewritten with iterator chaining; tolerates a `-<suffix>` tail on the patch component.
  - Typed response: `#[derive(Deserialize)] struct ReleaseResponse { tag_name: String }`.
  - Constant rename: `REQUEST_TIMEOUT` → removed (timeout now a parameter); `RELEASES_ENDPOINT` → `GITHUB_RELEASES_LATEST` (single source of truth).
  - Uniform `tracing::debug!` on every `None` branch (network error, parse error, status non-2xx, body too large, cache miss, etc.) — see Decision 4 in PLAN.md.

- `crates/fdemon-app/src/spawn.rs`:
  - Update `spawn_version_check` signature to `pub fn spawn_version_check(msg_tx: mpsc::Sender<Message>, timeout: Duration)`. Passes the timeout through to `check_for_newer_release(timeout)`.
  - Add a comment near the `tokio::spawn` site explaining that the JoinHandle is intentionally dropped (m7).
  - Rename the misleading test `spawn_version_check_sends_message_on_some` to `new_version_available_message_round_trips_through_channel` and add a comment noting that the actual spawn-with-network path is covered by `version_check`'s integration tests, not by this test.

- `crates/fdemon-app/Cargo.toml`: Add `wiremock = { workspace = true }` under `[dev-dependencies]`.

- `Cargo.toml` (workspace): Add `wiremock = "0.6"` under `[workspace.dependencies]` to keep the version pinned centrally.

- `crates/fdemon-tui/src/runner.rs`: Both `spawn_version_check` call sites (currently `runner.rs:78` and `:203`) pass `engine.settings.behavior.version_check_timeout_secs` converted to `Duration`. The conversion `Duration::from_secs(secs as u64)` happens at the call site — keep `spawn.rs` typed in `Duration` so the unit-conversion is visible to readers of the call site.

**Files Read (Dependencies):**

- `crates/fdemon-app/src/config/types.rs`: read the new `version_check_timeout_secs` field (added by task 05). **Task 04 must compile and test against the field NOT YET EXISTING** — see Notes below for the sequencing trick.

### Details

**Cache file format** (JSON):

```json
{
  "checked_at": 1716253200,
  "latest": "0.6.0"
}
```

- `checked_at`: POSIX seconds.
- `latest`: `Option<String>` — `null` when the last check confirmed current is up-to-date; otherwise the tag-without-`v` string. This lets us cache the "no update" answer too, avoiding 24h of pointless requests for current users.

**Cache read flow** (entering `check_for_newer_release`):

1. `read_cache()` → `Some(entry)`.
2. If `now - entry.checked_at < 86400`, return `entry.latest` directly. Skip the network call entirely.
3. Otherwise, fall through to network fetch.

**Cache write flow** (after a successful fetch):

1. Build a `CacheEntry { checked_at: now, latest: Some(tag) }` (or `latest: None` when the fetched tag is not newer than current).
2. Serialize, write atomically via `std::fs::rename` from a `.tmp` sibling. Best-effort: any IO error is logged at `debug` and otherwise ignored.

**Why atomic write:** prevents corrupt-on-crash. `fs2` is already a workspace dep but we don't need locking — race-write on the same file collapses to "last writer wins," and both writers' payloads are equivalent within the 24h TTL window.

**Body size cap** (in `fetch_latest_tag`):

```rust
if let Some(len) = response.content_length() {
    if len > MAX_RESPONSE_BYTES {
        tracing::debug!("Version check: response too large ({} bytes)", len);
        return None;
    }
}
// reqwest::Response::json reads the whole body — bound it via .bytes() then parse
let body = response.bytes().await.ok()?;
if body.len() > MAX_RESPONSE_BYTES {
    tracing::debug!("Version check: streamed response too large ({} bytes)", body.len());
    return None;
}
let parsed: ReleaseResponse = serde_json::from_slice(&body).ok()?;
```

Where `MAX_RESPONSE_BYTES: usize = 512 * 1024;`.

**Pre-release tolerance** (`parse_semver`):

```rust
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
```

Tests to update: `parse_semver_rejects_pre_release` becomes `parse_semver_strips_pre_release_suffix` asserting `parse_semver("0.6.0-rc.1") == Some((0, 6, 0))` and `parse_semver("0.6.0+build.42") == Some((0, 6, 0))`.

**`tracing::debug!` uniformity** — every `None` branch logs a one-line reason at `debug` level. Pattern: `tracing::debug!("Version check: <reason>");` (no end-of-message punctuation, prefix uniform).

**Typed response struct**:

```rust
#[derive(serde::Deserialize)]
struct ReleaseResponse {
    tag_name: String,
}
```

No `#[serde(deny_unknown_fields)]` — GitHub adds fields to release payloads regularly and we don't want to break on schema growth. `tag_name` is the only field we read.

**Sanitisation invariant doc comment** (on `check_for_newer_release`):

```rust
/// Returns `Some("0.6.0")` (the bare semver string, no `v` prefix) when
/// GitHub's latest release is newer than the compiled version. Returns
/// `None` on any failure or when the latest is not newer.
///
/// # Security: returned string is digit-and-dot only
///
/// The returned `String` is the output of `parse_semver`'s numeric-triple
/// validation. Any tag containing characters outside `[0-9.]` (including
/// ANSI/control sequences from a hostile or malformed response) fails the
/// parse step and returns `None`. Callers that embed the returned string
/// into a terminal banner can therefore skip escape-sequence sanitisation.
/// Do not change `parse_semver` to be more permissive without also adding
/// explicit sanitisation at the render site.
pub(crate) async fn check_for_newer_release(timeout: Duration) -> Option<String> { ... }
```

(`pub(crate)` is task 05's job — keep `pub` in task 04 so the visibility change is isolated.)

**Wiremock integration tests** (under `#[cfg(test)] mod tests` in `version_check.rs`):

```rust
#[tokio::test]
async fn returns_some_when_remote_is_newer() {
    let mock = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({ "tag_name": "v999.999.999" })))
        .mount(&mock).await;
    let result = fetch_latest_tag(&mock.uri(), Duration::from_secs(3)).await;
    assert_eq!(result, Some("999.999.999".to_string()));
}

#[tokio::test]
async fn returns_none_when_remote_is_older_or_equal() { ... }

#[tokio::test]
async fn returns_none_on_404() { ... }

#[tokio::test]
async fn returns_none_on_403_rate_limited() { ... }

#[tokio::test]
async fn returns_none_on_malformed_json() { ... }

#[tokio::test]
async fn returns_none_on_oversized_response() { ... }

#[tokio::test]
async fn returns_none_on_timeout() { ... }

#[tokio::test]
async fn strips_leading_v_prefix() { ... }
```

Note: these test `fetch_latest_tag` (the inner function) directly, not the cache wrapper. The cache logic gets its own tests (cache-hit-returns-cached, cache-miss-fetches, cache-write-after-fetch).

### Acceptance Criteria

1. `cargo build --workspace` succeeds.
2. `cargo test -p fdemon-app version_check` runs ≥ 14 tests (6 existing parse tests + 8 integration cases above + cache tests).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. Running `fdemon` twice within 24h shows only one outbound HTTPS request (verified via `lsof -i -p <pid>` or by removing the cache file and observing the next launch refetch).
5. The cache file at `<dirs::cache_dir()>/fdemon/version_check.json` is created on first run and contains valid JSON.
6. `grep -n "REQUEST_TIMEOUT" crates/fdemon-app/src/version_check.rs` returns no match (renamed to function parameter).
7. `grep -n "spawn_version_check_sends_message_on_some" crates/fdemon-app/src/spawn.rs` returns no match (renamed).
8. `parse_semver("0.6.0-rc.1")` returns `Some((0, 6, 0))` — assertion in the test module.
9. Hostile-response test: a wiremock mock that returns a 1 MB JSON body causes `fetch_latest_tag` to return `None` and emit a `tracing::debug!` line — verifiable by capturing tracing events.

### Testing

See the Wiremock matrix above. Additionally for the cache:

```rust
#[test]
fn cache_within_ttl_returns_cached_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("version_check.json");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    std::fs::write(&path, format!(r#"{{"checked_at": {}, "latest": "999.0.0"}}"#, now)).unwrap();
    // override cache_path() for test via injection or env var
    let result = read_cache_at(&path).filter_fresh(now);
    assert_eq!(result, Some(CacheEntry { checked_at: now, latest: Some("999.0.0".into()) }));
}

#[test]
fn cache_outside_ttl_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("version_check.json");
    // older than 24h
    std::fs::write(&path, r#"{"checked_at": 0, "latest": "999.0.0"}"#).unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let result = read_cache_at(&path).filter_fresh(now);
    assert_eq!(result, None);
}

#[test]
fn cache_corrupt_file_returns_none() { ... }
#[test]
fn cache_missing_file_returns_none() { ... }
#[test]
fn cache_atomic_write_via_rename() { ... } // confirm .tmp is created then renamed
```

The cache-path injection trick: a private `fn cache_path() -> Option<PathBuf>` that production code calls (using `dirs::cache_dir()`); tests call a variant `read_cache_at(&path)` / `write_cache_at(&path, ...)` directly. Avoids `std::env::set_var` in tests.

### Notes

- **Sequencing with task 05**: task 04 ships `spawn_version_check(msg_tx, timeout: Duration)`. The call sites in `runner.rs` need a value to pass — for this task, hardcode `Duration::from_secs(3)` at both call sites. Task 05 replaces those hardcoded values with `engine.settings.behavior.version_check_timeout_secs`. Both tasks compile cleanly on their own.
- The cache logic and the body-size cap and the typed struct **must land together** — the typed struct simplifies the body-cap implementation (`serde_json::from_slice(&bytes)` after the size check), and the cache wraps the whole fetch path.
- Atomic rename for cache write: `std::fs::write` to `.tmp`, then `std::fs::rename` to the target. On Windows, `rename` over an existing file fails — use `std::fs::remove_file` + `rename` if Windows support matters. **Verify on Windows CI** — this is a known footgun.
- `wiremock` is the same lib used by `reqwest`'s own tests; it's the standard choice. Pinned at `0.6` (current as of 2026-05).
- The `pub` → `pub(crate)` visibility narrowing is intentionally left for task 05 to keep this task's diff focused.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/version-check-banner-followup

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale>

### Testing Performed

- `cargo build --workspace` — Pending
- `cargo test -p fdemon-app version_check` — Pending
- `cargo clippy --workspace --all-targets -- -D warnings` — Pending
- Cache TTL behavior verified manually — Pending
- Windows atomic-rename verified on CI — Pending

### Risks/Limitations

1. **<Risk>**: <Description>
