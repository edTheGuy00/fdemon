# Action Items: Version-Check Banner

**Review Date:** 2026-05-21
**Verdict:** ⚠️ APPROVED WITH CONCERNS
**Blocking Issues:** 0
**Recommended Follow-ups:** 4 MAJOR, 7 MINOR, 11 NITPICK

The feature is shippable as-is. The items below are tracked for follow-up. None block merge.

---

## Critical Issues (Must Fix Before Merge)

None.

---

## Major Issues (Should Fix Soon)

### M1. Add privacy disclosure for outbound HTTPS

- **Source:** risks_tradeoffs_analyzer
- **File:** `docs/CONFIGURATION.md`, `README.md`
- **Problem:** `version_check = true` by default causes every TUI launch to issue an HTTPS GET to api.github.com, including `User-Agent: fdemon/<version>`. No user-facing disclosure exists. Industry norm for dev tools is default-on with explicit notice.
- **Required Action:** Add a "Privacy" subsection in `docs/CONFIGURATION.md#version_check` (and/or README) stating exactly what is transmitted: outbound request to `https://api.github.com/repos/edTheGuy00/fdemon/releases/latest`, `User-Agent: fdemon/<version>` header, one request per launch. Document the opt-out.
- **Acceptance:** A privacy-conscious user reading the README or CONFIGURATION.md before installing knows the tool phones home and how to disable it.

### M2. Add a 24h on-disk TTL cache to mitigate rate-limit risk

- **Source:** risks_tradeoffs_analyzer
- **File:** New: `crates/fdemon-app/src/version_check.rs` (or a new `cache` submodule)
- **Problem:** GitHub's unauthenticated rate limit is 60 req/hr per IP. Heavy users (frequent restarts) or shared NAT environments (corporate offices) will hit it. 403 responses collapse silently — a chronically rate-limited team appears to "have no updates" forever.
- **Suggested Action:** Persist `{ checked_at: u64, latest: Option<String> }` to `<config-dir>/fdemon/cache/version_check.json`. Skip the network call if `now - checked_at < 86400`. Last-write-wins; no locking needed.
- **Acceptance:** Running `fdemon` twice within 24 h triggers exactly one HTTP request; the second uses the cache.

### M3. Resolve layer-boundary placement for `reqwest`

- **Source:** architecture_enforcer, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/version_check.rs`, `docs/REVIEW_FOCUS.md` or `docs/ARCHITECTURE.md`
- **Problem:** `fdemon-app` is the TEA/orchestration layer per docs; all network I/O lives in `fdemon-daemon`. `reqwest` in `fdemon-app` sets precedent that "fdemon-app may do I/O" and risks erosion of the boundary.
- **Required Action — choose one:**
  - **(a)** Move `version_check.rs` to `fdemon-daemon` (or a new `fdemon-net` crate). Keep `Message::NewVersionAvailable` and `spawn_version_check` in `fdemon-app`.
  - **(b)** Add an "Approved Optimizations / Exceptions" entry to `docs/REVIEW_FOCUS.md` enshrining `fdemon-app::version_check` as the sole permitted network I/O in `fdemon-app`, and explaining why (no Flutter-protocol knowledge; forcing it into `fdemon-daemon` would saddle that crate with a TLS stack it doesn't otherwise need).
- **Acceptance:** Future contributors adding network I/O to `fdemon-app` are caught at review time by an explicit precedent (either the layer is restored, or the exception is documented and bounded).

### M4. Handle late-arriving `NewVersionAvailable` lifecycle

- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/update.rs:360`
- **Problem:** Auto-launch path skips NSD entirely. If the version-check task completes 2 s after auto-launch, `startup_notice` is set on a `Normal` ui_mode state. The render gate correctly hides it then, but when the user later presses `n` to open NSD mid-session, the banner surfaces well outside the "startup window."
- **Suggested Action — choose one:**
  - **(a)** Gate the handler arm: `if matches!(state.ui_mode, UiMode::Startup | UiMode::NewSessionDialog) { state.startup_notice = Some(...); }`. Drops late messages.
  - **(b)** Document in CONFIGURATION.md that the banner appears on the *first* NSD opening after a version check completes, regardless of timing.
- **Acceptance:** Either the behavior matches the documented "startup-screen only" scope, or the docs match the actual behavior.

---

## Minor Issues (Consider Fixing)

### m1. Cap HTTP response body size

- **Source:** security_reviewer
- **File:** `crates/fdemon-app/src/version_check.rs:40`
- **Action:** Before `.json()`, check `response.content_length()` and reject responses > 512 KB.

### m2. Rewrite `parse_semver` without `Vec` allocation

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/version_check.rs`
- **Action:** Use `split('.').next()` chaining; reject trailing components with `if parts.next().is_some() { return None; }`. Eliminates the `collect-then-index` anti-pattern flagged in CODE_STANDARDS.md.

### m3. Rename misleading test

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/spawn.rs:787`
- **Action:** Rename `spawn_version_check_sends_message_on_some` to `new_version_available_message_round_trips_through_channel`; add a comment that `spawn_version_check` itself is covered only by manual smoke testing.

### m4. Add integration test for network code path

- **Source:** code_quality_inspector, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/version_check.rs` (tests module)
- **Action:** Add `wiremock` or `mockito` dev-dep; stub `RELEASES_ENDPOINT`; cover 200 newer/older, 404, 403 (rate-limited), malformed JSON. Endpoint URL must be injectable.

### m5. Update `behavior_settings_auto_launch_defaults_false` to assert `version_check`

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/config/types.rs:1478`
- **Action:** Add `assert!(s.version_check);` to the existing defaults test.

### m6. Document the pre-release tag policy

- **Source:** risks_tradeoffs_analyzer
- **File:** `CONTRIBUTING.md` or `docs/RELEASING.md` (if exists)
- **Action:** Add a release-engineering rule: pre-release tags must be published with `prerelease: true` on GitHub. Verify the assumption that `releases/latest` excludes those. OR upgrade `parse_semver` to extract the leading numeric triple from `vX.Y.Z-suffix`.

### m7. Comment on unjoined `tokio::spawn`

- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/spawn.rs:357`
- **Action:** Add a comment to `spawn_version_check` noting that the JoinHandle is intentionally dropped, tokio runtime drop aborts orphaned tasks on shutdown, and the 3 s timeout bounds the lifetime.

---

## Nitpicks

- **N1.** Narrow `pub mod version_check` → `pub(crate) mod version_check` (and the function's `pub` → `pub(crate)`).
- **N2.** Comment in `src/headless/runner.rs` documenting intentional absence of `spawn_version_check`.
- **N3.** Rename `REQUEST_TIMEOUT` → `VERSION_CHECK_TIMEOUT` to match `TOOL_CHECK_TIMEOUT` naming.
- **N4.** Make `tracing::debug!` logging uniform across all `None`-producing branches in `version_check.rs` (all or none).
- **N5.** Extract a `split_notice_area` helper to dedupe banner layout between `render_regions_impl` and `Widget::render`.
- **N6.** `.redirect(reqwest::redirect::Policy::none())` on the client builder; the GitHub releases endpoint does not redirect.
- **N7.** Add a doc comment to `check_for_newer_release` stating that the returned `String` is digit-and-dot only by virtue of `parse_semver` — preserves the sanitisation invariant against future refactors.
- **N8.** Hardcoded `edTheGuy00/fdemon` URL appears in three files; consider `option_env!("FDEMON_GITHUB_REPO")` or a centralized constant.
- **N9.** Include a URL or "see CHANGELOG" hint in the banner copy.
- **N10.** Add `[behavior] version_check_timeout_secs` config knob.
- **N11.** Use `#[derive(Deserialize)] struct ReleaseResponse { tag_name: String }` instead of `serde_json::Value` — bounds parse cost, catches schema drift, drops the body of the response we don't care about.

---

## Re-review Checklist

This is a follow-up triage list, not a re-review gate. Items M1–M4 should land before the next release; minors can be batched.

- [ ] M1: Privacy disclosure added to user-facing docs
- [ ] M2: 24h on-disk TTL cache
- [ ] M3: Layer-boundary decision documented or `version_check.rs` moved
- [ ] M4: Late-arrival lifecycle resolved (gate or doc)
- [ ] m1–m7: Minor cleanups batched in a single follow-up PR
- [ ] N1–N11: Address opportunistically
- [ ] Manual smoke checklist from task 04 executed (force-newer-version, opt-out, offline)
