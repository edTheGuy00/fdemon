# Version-Check Banner — Review Follow-ups

## Goal

Address the 4 MAJOR and several MINOR/NITPICK findings from the multi-agent review of `feat/version-check-banner`. See `workflow/reviews/features/version-check-banner/{REVIEW.md,ACTION_ITEMS.md}` for the full triage.

The version-check feature is shipped and working; this plan is hardening.

## What's being addressed

| ID | Category | Summary |
|----|----------|---------|
| **M1** | Privacy | No user-facing disclosure that fdemon phones home to api.github.com on every launch |
| **M2** | Reliability | No on-disk cache → 60/hr GitHub rate-limit pressure for heavy users and shared NAT |
| **M3** | Architecture | `reqwest` in `fdemon-app` is the first network I/O in that layer — boundary should be moved or formally documented |
| **M4** | Lifecycle | Handler arm sets `startup_notice` regardless of `ui_mode`, so a slow check can surface the banner on a later NSD open mid-session |
| m1 | Security | Response body has no size cap |
| m2 | Quality | `parse_semver` uses collect-then-index |
| m3 | Tests | `spawn_version_check_sends_message_on_some` doesn't call the function |
| m4 | Tests | No integration test for the network code path |
| m5 | Tests | `behavior_settings_auto_launch_defaults_false` doesn't assert `version_check` |
| m6 | Reliability | Pre-release tags (`v0.6.0-rc.1`) silently break update notifications |
| m7 | Quality | Unjoined `tokio::spawn` lacks a comment |
| N1 | Visibility | `pub mod version_check` could be `pub(crate)` |
| N2 | Quality | Intentional absence of `spawn_version_check` in headless is undocumented |
| N3 | Naming | `REQUEST_TIMEOUT` → `VERSION_CHECK_TIMEOUT` |
| N4 | Logging | `tracing::debug!` only on one `None` branch |
| N5 | Quality | Banner layout duplicated between `render_regions_impl` and `Widget::render` |
| N6 | Security | Default redirect-following enabled |
| N7 | Documentation | Sanitisation invariant (digit-and-dot only) is implicit |
| N8 | Quality | Hardcoded `edTheGuy00/fdemon` URL in three files |
| N9 | UX | Banner has no URL or "how to update" hint |
| N10 | Config | 3 s timeout is hardcoded; should be `[behavior] version_check_timeout_secs` |
| N11 | Quality | `serde_json::Value` could be a typed struct |

## Non-goals

- No new product features beyond what the review surfaced.
- No re-litigating settled design decisions (silent failure model, opt-out key name, banner copy verb).
- No telemetry beyond what already exists (the version-check call is the only outbound traffic; we are not adding more).

## Design decisions

### Decision 1 — Layer-boundary: document the exception, do not move `version_check.rs`

`fdemon-daemon` has no HTTP client today (its network code is `tokio-tungstenite` for VM Service WebSockets). Moving `version_check.rs` into `fdemon-daemon` would saddle that crate with a TLS stack it does not otherwise need, and `version_check.rs` has zero Flutter-protocol knowledge — putting it in the Flutter daemon crate is semantically wrong.

**Chosen:** Document `fdemon-app::version_check` as the sole permitted network I/O in the `fdemon-app` layer via an "Approved Exception" entry in `docs/REVIEW_FOCUS.md`. Future reviewers can enforce the boundary against drift without further architectural churn.

**Rejected:** New `fdemon-net` crate (overengineering for a single ~100-line module).
**Rejected:** Move to `fdemon-daemon` (taints the daemon's dep tree).

### Decision 2 — Cache: per-user file under `dirs::cache_dir()`, 24h TTL

The `dirs` crate is already a workspace dependency. Cache path: `dirs::cache_dir()?.join("fdemon").join("version_check.json")`. Format: `{ "checked_at": u64, "latest": Option<String> }` (POSIX seconds + the last known newer-release tag, or `null` if the last check showed current is up-to-date).

**Why per-user, not per-project**: the check is about the fdemon binary, not the Flutter project. Two `fdemon` runs in different directories should share a cache.

**Concurrency:** last-write-wins. Two simultaneous fdemons each hitting the cache race-write the file. Last writer's bytes survive. Acceptable for this data — every entry contains the same "result" within a 24 h window, and a corrupt read collapses silently to "no cache, do a fresh check."

**Why 24h:** matches the standard convention for dev tools (`brew`, `npm`). Long enough to absorb burst usage, short enough that a freshly cut release is noticed within a day.

**Opt-out interaction:** when `[behavior] version_check = false`, the cache is never read or written. The guard stays at the spawn site.

### Decision 3 — Late-arrival gate: drop the message if NSD already dismissed

Add a guard in the handler arm: if `state.ui_mode` is not `Startup` or `NewSessionDialog` when `Message::NewVersionAvailable` arrives, drop it (do not set `startup_notice`). This matches the "startup-screen only" scope from the original plan.

The existing helper `AppState::is_new_session_dialog_visible()` at `state.rs:1707` returns true for both `Startup` and `NewSessionDialog`. Reuse it.

**Rejected:** track the JoinHandle and abort on `hide_new_session_dialog`. Heavier, doesn't add value — the message-drop approach is simpler and tokio cleanup is already fine.

### Decision 4 — Configurable timeout via `[behavior] version_check_timeout_secs`

Add `version_check_timeout_secs: u8` to `BehaviorSettings` with default `3`. `u8` (range 0–255) is sufficient and prevents nonsense values without a `Range` validator. The constant `VERSION_CHECK_TIMEOUT` is removed; `check_for_newer_release` takes the timeout as a parameter.

### Decision 5 — Pre-release tag handling: tolerate the suffix

Upgrade `parse_semver` so `0.6.0-rc.1` parses to `(0, 6, 0)`. Compare on the numeric triple only. This means a stable user could be notified about an RC if GitHub serves `latest = v0.6.0-rc.1` — which is a release-engineering decision (GitHub's `releases/latest` excludes prereleases by default if `prerelease: true` is set on the release).

**Why permissive parsing:** the alternative (`CONTRIBUTING.md` mandating `prerelease: true`) requires release-engineering discipline that is one bug away from silently breaking the check globally. Tolerant parsing is more robust.

### Decision 6 — Typed response struct, not `serde_json::Value`

```rust
#[derive(serde::Deserialize)]
struct ReleaseResponse {
    tag_name: String,
}
```

`serde(deny_unknown_fields)` is **not** set — GitHub adds fields to release payloads regularly, and we don't want to break on schema growth.

Benefits:
- Smaller heap footprint than `Value` (only one field deserialized).
- Catches `tag_name` schema drift at compile time.
- Reads better than `.get("tag_name")?.as_str()?`.

### Decision 7 — `wiremock` as dev-dep for integration tests

`wiremock = "0.6"` adds ~5 dependencies but only at test time. Run a local HTTP stub on a random port; inject the endpoint URL into a refactored `check_for_newer_release(endpoint: &str, timeout: Duration)`. The public `pub async fn check_for_newer_release()` becomes a thin wrapper that supplies the hardcoded URL and the configured timeout.

**Test matrix:**
- 200 with `{ "tag_name": "0.6.0" }` and current `0.5.4` → `Some("0.6.0")`
- 200 with `{ "tag_name": "v0.6.0" }` → strips `v`, returns `Some("0.6.0")`
- 200 with `{ "tag_name": "0.5.4" }` (equal) → `None`
- 200 with `{ "tag_name": "0.6.0-rc.1" }` → with tolerant parser, `Some("0.6.0-rc.1")`
- 200 with malformed JSON → `None`
- 404 → `None`
- 403 (rate-limited) → `None`
- Timeout (sleep > 3 s in handler) → `None`

### Decision 8 — Centralize the GitHub URL

Move `https://api.github.com/repos/edTheGuy00/fdemon/releases/latest` into a single constant `GITHUB_RELEASES_LATEST` and reference it from one location. The hardcoded references in `README.md` / `CONTRIBUTING.md` are separately maintained — leave them alone (they document the human-facing URL).

### Decision 9 — Bundle most minor/nitpick polish into the `version_check.rs` hardening task

Items that all touch `version_check.rs` belong together — splitting them creates artificial PR churn. The hardening task subsumes: M2, m1, m2, m4, m6, m7, N3, N4, N6, N7, N11. The remaining nits that touch other files batch into a single polish task.

## File-level change inventory

### New files

| File | Purpose |
|---|---|
| `workflow/plans/features/version-check-banner-followup/PLAN.md` | This file |
| `workflow/plans/features/version-check-banner-followup/TASKS.md` | Task index |
| `workflow/plans/features/version-check-banner-followup/tasks/01-…06.md` | Six task files |
| (eventual) `<dirs::cache_dir()>/fdemon/version_check.json` | Runtime cache artifact (not in repo) |

### Modified files (across all tasks)

| File | Tasks |
|---|---|
| `crates/fdemon-app/src/version_check.rs` | 04 (refactor + cache + tests + parser + struct), 05 (visibility, URL constant) |
| `crates/fdemon-app/src/spawn.rs` | 04 (test rename + comment) |
| `crates/fdemon-app/src/handler/update.rs` | 03 (late-arrival gate + test) |
| `crates/fdemon-app/src/config/types.rs` | 05 (timeout field + default test) |
| `crates/fdemon-app/src/lib.rs` | 05 (visibility) |
| `crates/fdemon-app/Cargo.toml` | 04 (wiremock dev-dep) |
| `crates/fdemon-tui/src/runner.rs` | 04 (pass timeout to spawn) |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | 05 (layout helper + URL hint) |
| `src/headless/runner.rs` | 05 (intentional-absence comment) |
| `Cargo.toml` (workspace) | 04 (wiremock if pinned centrally) |
| `docs/CONFIGURATION.md` | 01 (privacy disclosure), 05 (timeout doc) |
| `docs/REVIEW_FOCUS.md` | 02 (Approved Exception entry) |
| `README.md` | 01 (privacy disclosure) |
| `docs/ARCHITECTURE.md` | 06 (cache artifact + updated startup sequence) |

## Open items deferred

These ACTION_ITEMS entries are not in this plan, by design:

- **N9** Banner copy URL — included in task 05 (small UX add)
- **N5** Banner layout helper — included in task 05 (cleanup that pairs with N9)
- Anything below NITPICK that isn't listed in tasks 01–06 above

The deferred items are not lost — they remain in `workflow/reviews/features/version-check-banner/ACTION_ITEMS.md` for future polish.
