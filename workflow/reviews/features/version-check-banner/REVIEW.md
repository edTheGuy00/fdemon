# Code Review: Version-Check Banner

**Review Date:** 2026-05-21
**Branch:** `feat/version-check-banner`
**Diff Base:** `823031a..HEAD` (7 commits)
**Plan:** [workflow/plans/features/version-check-banner/PLAN.md](../../../plans/features/version-check-banner/PLAN.md)

## Verdict: ⚠️ APPROVED WITH CONCERNS

The implementation is correct, the happy path works, all tests pass, and the workspace passes the full quality gate (`fmt`, `check`, `test`, `clippy`). The TEA pattern is followed cleanly, the migration-nudge machinery is fully removed, and the opt-out guard is wired at both TUI entry points. Several substantive concerns — none blocking — are surfaced below and tracked in [ACTION_ITEMS.md](ACTION_ITEMS.md).

## Reviewer Verdicts (5 agents)

| Agent | Verdict | Notable Findings |
|-------|---------|------------------|
| `architecture_enforcer` | ✅ PASS (1 warning, 2 suggestions) | `reqwest` in `fdemon-app` vs `fdemon-daemon` layer; `pub` could be `pub(crate)` |
| `code_quality_inspector` | ✅ APPROVED with minor | `parse_semver` uses collect-then-index; spawn test misnamed |
| `logic_reasoning_checker` | ✅ PASS (1 warning) | Late-arriving `NewVersionAvailable` can surface banner on a later NSD open |
| `risks_tradeoffs_analyzer` | ⚠️ CONCERNS | Privacy framing, no on-disk cache (rate-limit risk), layer boundary |
| `security_reviewer` | ✅ PASS (0 critical, 1 medium, 3 low) | No response body size cap; default redirect follow |

## Acceptance Criteria

| Criterion | Met |
|-----------|-----|
| No "Cache-driven auto-launch" banner | ✅ `grep` confirms deletion |
| Newer GitHub release → banner copy correct | ✅ Format matches: `⬆ New version available: v<latest> (current v<current>)` |
| Current ≥ latest → no banner | ✅ Comparator returns `None` |
| Network/parse failure → silent | ✅ All `?` paths collapse to `None` |
| `[behavior] version_check = false` → no outbound HTTP | ✅ Guard at both TUI call sites |
| Headless skips check entirely | ✅ No call in `src/headless/runner.rs` |
| Render time not blocked | ✅ `tokio::spawn` returns immediately |
| `cargo test --workspace` passes | ✅ All crates pass |
| `cargo clippy --workspace` clean | ✅ |
| All references to deleted symbols removed | ✅ `show_migration_banner`, `emit_migration_nudge`, `NudgeMode` all gone |

`has_cached_last_device` was intentionally retained — it has live callers in `startup.rs` and `headless/runner.rs` for the auto-launch cache-gate (unrelated to migration nudges). The plan-author conflated it; the implementor made the correct call.

## Consolidated Findings

### 🟠 MAJOR (track as follow-up)

#### M1. Privacy framing absent from user-facing docs
**[Source: risks_tradeoffs_analyzer]**

`version_check = true` by default means every TUI launch issues `GET https://api.github.com/.../releases/latest` with `User-Agent: fdemon/<version>`. The README has no Privacy section, and `docs/CONFIGURATION.md#version_check` does not disclose what is sent. Industry norm for dev tools (`npm`, `brew`, `cargo`) is default-on but with explicit disclosure.

**Required action:** Add a one-paragraph Privacy section to `docs/CONFIGURATION.md` (or README) naming exactly what is transmitted: source IP (inherent), `User-Agent: fdemon/<version>`, and one `GET` request per launch. Document the opt-out flag prominently.

#### M2. No on-disk cache → GitHub 60/hr unauthenticated rate-limit pressure
**[Source: risks_tradeoffs_analyzer]**

Per-launch checks with no cache means heavy users (rapid restarts) or shared NAT environments (corporate offices, multiple developers behind one IP) will exhaust the 60 req/hr rate limit. A 403 response collapses silently to "no banner," so a chronically rate-limited team appears to "have no updates" forever.

**Suggested action:** 24-hour TTL on `~/.fdemon/cache/version_check.json`. ~20 LOC, last-write-wins, no locking needed. The plan rejected this as "complexity" — that judgment does not match the actual implementation cost vs the failure mode it prevents.

#### M3. Layer boundary: `reqwest` is the first network I/O in `fdemon-app`
**[Source: architecture_enforcer, risks_tradeoffs_analyzer]**

`fdemon-app` is documented as the TEA/orchestration layer; all network I/O historically lives in `fdemon-daemon` (VM Service WebSocket, native log capture). Placing `reqwest` in `fdemon-app` is the first violation of that boundary and sets precedent that "fdemon-app may do I/O." Counter-argument: `version_check.rs` has zero Flutter-protocol knowledge and forcing it into `fdemon-daemon` would saddle that crate with a TLS stack it does not otherwise need.

**Suggested action:** Either (a) move `version_check.rs` to `fdemon-daemon` (or a new `fdemon-net` crate), keeping `Message::NewVersionAvailable` and `spawn_version_check` in `fdemon-app`; OR (b) add an entry under "Approved Optimizations" in `docs/REVIEW_FOCUS.md` documenting that `fdemon-app::version_check` is the sole permitted network I/O at this layer, so future reviewers can enforce the boundary against drift.

#### M4. Late-arriving `NewVersionAvailable` can surface banner on later NSD opens
**[Source: logic_reasoning_checker]**

Handler arm at `update.rs:360` unconditionally sets `state.startup_notice = Some(...)` regardless of `ui_mode`. Scenario: user auto-launches → no NSD shown → 2 s later the version-check task completes and sets `startup_notice`. The render gate hides it (no NSD on screen). User later presses `n` to open a new-session dialog mid-session → banner appears, well outside the "startup-screen" window the plan describes.

**Suggested action:** Gate the handler arm on `matches!(state.ui_mode, UiMode::Startup | UiMode::NewSessionDialog)` so late messages are dropped, OR explicitly document this behavior is intentional in the plan/CONFIGURATION docs.

### 🟡 MINOR

#### m1. Response body has no size limit
**[Source: security_reviewer]**

`response.json().await.ok()?` buffers the full body before parsing. The 3 s timeout bounds wall-clock time but not bytes — a slow trickle could allocate several MB before the timeout. Suggested fix: check `response.content_length()` and reject > 512 KB before calling `.json()`.

#### m2. `parse_semver` uses collect-then-index instead of iterator chaining
**[Source: code_quality_inspector]**

`CODE_STANDARDS.md` flags "collect-then-iterate" as anti-pattern. Current `Vec<&str>` allocation is unnecessary:
```rust
let mut parts = s.split('.');
let major = parts.next()?.parse().ok()?;
let minor = parts.next()?.parse().ok()?;
let patch = parts.next()?.parse().ok()?;
if parts.next().is_some() { return None; }
Some((major, minor, patch))
```

#### m3. Test name lies: `spawn_version_check_sends_message_on_some` does not call `spawn_version_check`
**[Source: code_quality_inspector]**

The test constructs a channel, manually sends `Message::NewVersionAvailable`, and reads it back. It tests `tokio::sync::mpsc`, not the function it claims to test. Rename to `new_version_available_message_round_trips_through_channel` and add a comment noting that `spawn_version_check` itself is only covered by manual smoke testing.

#### m4. Network code path has no integration test
**[Source: code_quality_inspector, risks_tradeoffs_analyzer]**

`check_for_newer_release` has zero unit tests — only `parse_semver` and the tuple comparator (which tests Rust's built-in `PartialOrd`, not fdemon logic) are exercised. Header construction, leading-`v` strip, 404/403/malformed-JSON handling, and `tag_name` field extraction all run unguarded. Combined with the silent-failure model, regressions are invisible. Suggested: `wiremock`/`mockito` test with stub endpoint.

#### m5. `behavior_settings_auto_launch_defaults_false` not updated to cover `version_check`
**[Source: code_quality_inspector]**

Existing default-values test asserts `auto_launch` and `confirm_quit` but not the new `version_check` field. Trivial one-line addition.

#### m6. Pre-release tag landmine
**[Source: risks_tradeoffs_analyzer]**

`parse_semver` rejects `0.6.0-rc.1`. The day a release engineer publishes a pre-release tag, every user worldwide may silently stop seeing update notifications until a final tag is cut. GitHub's `releases/latest` endpoint typically excludes pre-releases (`prerelease: true`), but verify this assumption holds. Either document the release-process rule in `CONTRIBUTING.md` or upgrade `parse_semver` to tolerate suffixes.

#### m7. `tokio::spawn` task is not joined or aborted on shutdown
**[Source: risks_tradeoffs_analyzer]**

Mirrors the pre-existing pattern of `spawn_tool_availability_check`. Today the tokio runtime drop aborts the orphaned task on process exit, so blast radius is zero. But the pattern compounds with each new background task. Add at minimum a comment noting the implicit contract.

### 🔵 NITPICKS

- **N1.** `pub mod version_check` and `pub async fn check_for_newer_release` have no external callers — narrow to `pub(crate)`. [Source: architecture_enforcer]
- **N2.** Add a comment in `src/headless/runner.rs` documenting the intentional absence of `spawn_version_check`. [Source: architecture_enforcer]
- **N3.** `REQUEST_TIMEOUT` would be clearer as `VERSION_CHECK_TIMEOUT` to match `TOOL_CHECK_TIMEOUT` naming convention. [Source: code_quality_inspector]
- **N4.** `tracing::debug!` fires on non-2xx status but is absent from all other `None` branches in `version_check.rs` — either remove the lone debug line or add equivalents for consistency. [Source: code_quality_inspector]
- **N5.** Banner layout duplication between `render_regions_impl` and `Widget::render` in `new_session_dialog/mod.rs`. Pre-existing structural issue; this PR replicates it for the new notice. [Source: code_quality_inspector]
- **N6.** Default redirect-following enabled on the `reqwest::Client`. The GitHub releases API does not redirect; consider `Policy::limited(3)` or `Policy::none()`. [Source: security_reviewer]
- **N7.** The sanitisation invariant (`latest_str` is `[0-9.]`-only because `parse_semver` must succeed) is implicit. A future refactor could remove it inadvertently. Add a doc comment to `check_for_newer_release` stating it as a contract. [Source: security_reviewer]
- **N8.** Hardcoded `edTheGuy00/fdemon` URL appears in `version_check.rs`, `README.md`, `CONTRIBUTING.md`. A repo move/rename requires N edits. Low priority. [Source: risks_tradeoffs_analyzer]
- **N9.** Banner copy omits a URL or "see CHANGELOG" hint — user sees there's a new version but no path to upgrade. [Source: risks_tradeoffs_analyzer]
- **N10.** The 3 s timeout is hardcoded; users on slow connections may never see banners. A `[behavior] version_check_timeout_secs` config knob would be a courtesy. [Source: risks_tradeoffs_analyzer]
- **N11.** Consider using a typed `#[derive(Deserialize)] struct ReleaseResponse { tag_name: String }` instead of `serde_json::Value`. Bounds parse cost, catches schema drift, allows `serde(deny_unknown_fields = false)` to handle GitHub adding new fields without breaking the client. [Source: risks_tradeoffs_analyzer, security_reviewer]

## Documentation Freshness

| Doc | Updated? | Notes |
|-----|----------|-------|
| `docs/ARCHITECTURE.md` | ✅ Task 05b | Added `version_check.rs` module entry, startup-task enumeration, `reqwest` to fdemon-app dep line |
| `docs/CONFIGURATION.md` | ✅ Task 05a | New `version_check` row + subsection; deleted stale post-v0.5.0 callout |
| `docs/CODE_STANDARDS.md` | ⏭ Not needed | No new conventions introduced |
| `docs/DEVELOPMENT.md` | ⏭ Not needed | Build/test commands unchanged; new dep is a normal Cargo entry |
| `docs/REVIEW_FOCUS.md` | ⚠ See M3 | Should be updated if the `fdemon-app` layer-exception path is chosen for `reqwest` |
| README | ⚠ See M1 | Should disclose the outbound HTTPS call for user transparency |

## Strengths

- Clean TEA integration: new `Message` variant → pure handler arm → state mutation → render.
- `StartupNotice` as an enum (rather than `String`) is forward-thinking — future notices add a variant, not a new state field.
- Silent-failure model prevents user-visible churn when GitHub is unreachable.
- Opt-out config key is well-named, well-defaulted (default `true` mirrors `confirm_quit` pattern), and gates both call sites.
- Migration-nudge cleanup is in the same commit set — no straggler tech debt.
- Headless correctly skips the check (CI-noise avoidance).
- `parse_semver` strict rejection of malformed versions doubles as a sanitisation gate against ANSI escape injection.

## Files Modified (16 source + 2 docs)

See `workflow/plans/features/version-check-banner/PLAN.md` for the full inventory. Aggregate scope: 1 new module, 6 modified `fdemon-app` files, 4 modified `fdemon-tui` files, 1 modified binary file, 2 doc updates, 1 workspace Cargo.toml change.
