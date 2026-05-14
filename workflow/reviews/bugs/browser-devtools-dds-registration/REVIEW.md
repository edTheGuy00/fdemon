# Bug Fix Review: browser-devtools-dds-registration

**Review Date:** 2026-05-12
**Reviewer:** Code Review Orchestrator
**Bug Task:** `workflow/plans/bugs/browser-devtools-dds-registration/BUG.md` + 10 task files
**Files Changed:** 19 files, +1542/-27 lines, 9 commits (e711dd6..dcd92d5)

---

## Executive Summary

**Overall Verdict:** ❌ REJECTED

The primary `app.devTools` event path works correctly and fixes the bug on modern Flutter (≥3.24). However, two reviewers independently identified that `parse_devtools_serve_response` is orphaned dead code — the `devtools.serve` RPC fallback path that the implementation advertises as "belt-and-suspenders" is structurally broken because string request IDs cannot be matched against the numeric `RequestTracker`, and the `Response` arm in `handler/daemon.rs` never invokes the parser. Additionally, security review found HIGH-severity URL-scheme injection vulnerabilities: a malicious Flutter project can inject `javascript:`, `file://`, or `data:` URIs through the `app.devTools` event and have them passed verbatim to the OS browser opener. Lifecycle state-leak issues compound these: `devtools_endpoint` and `devtools_serve_pending` survive `AppStop`/`VmServiceDisconnected`, so after a hot restart users get a stale URL silently.

---

## Bug Context

### Original Problem
GitHub issue #42 — Pressing `B` in DevTools mode opens `http://<DDS-host>/devtools/?uri=<ws_uri>`, returning literal "No DevTools instance is registered with the Dart Development Service (DDS)" on Flutter ≥3.16, because newer DDS no longer bundles DevTools — it must be separately registered.

### Root Cause
Two-fold: (1) BUG.md's assumed JSON-RPC contract was wrong (correct method is `devtools.serve` not `daemon.devtools.serve`; correct async event is `app.devTools` under `app` domain, not `daemon.devtools`); (2) fdemon never listened for `app.devTools` and never dispatched `devtools.serve`.

### Fix Approach
- Listen for `app.devTools` event → store base URL on `Session.devtools_endpoint`.
- Eagerly dispatch `devtools.serve` RPC on `VmServiceConnected` as a fallback (intent — implementation is broken, see Critical #1).
- On `B` keypress: prefer served URL, fall back to legacy URL with a recovery toast.

---

## Subagent Review Summaries

### Bug Fix Reviewer
**Verdict:** 🟠 APPROVED WITH CONCERNS

**Root Cause Addressed:** Yes (primary path); No (fallback path is broken)

Identified the orphaned-RPC-response issue independently. Confirmed all acceptance criteria pass for the primary `app.devTools` event path. Flagged that on Flutter SDKs that don't emit `app.devTools` (older versions, certain build modes), the fallback is silently broken.

### Architecture Enforcer
**Verdict:** 🟠 WARNING

**Layer compliance:** PASS (no cross-layer violations). **TEA pattern:** WARNING — `handle_open_browser_devtools` signature changed from `&AppState` to `&mut AppState` to enable inline `push_toast()` calls; consistent with project convention but expands handler impurity. `DevToolsServeFailed` handler doesn't emit a toast (silent failure to surface to user). `devtools_serve_pending` not reset on `VmServiceDisconnected`.

### Code Quality Inspector
**Verdict:** 🟠 NEEDS WORK

**Quality Scores:**
| Metric | Score |
|--------|-------|
| Language Idioms | ⭐⭐⭐⭐ |
| Error Handling | ⭐⭐⭐⭐ |
| Testing | ⭐⭐⭐⭐ |
| Documentation | ⭐⭐⭐ |
| Maintainability | ⭐⭐⭐ |

**Key Findings:** Duplicated `percent_encode_uri` between `session/session.rs` and `handler/devtools/mod.rs` (acknowledged but unresolved). Magic number `4` in toast width with off-by-two against actual icon width. `let _ = m` discards live messages in bridge collision arm. Comment typos using `/` instead of `//`. `served_at` field is dead weight.

### Logic & Reasoning Checker
**Verdict:** ❌ FAIL

**Critical Findings:**
1. `parse_devtools_serve_response` is orphaned dead code; RPC response never reaches `Message::DevToolsServed`. The acknowledgment comment at `actions/mod.rs:861-863` is incorrect.
2. `handle_open_browser_devtools` silently returns `UpdateResult::none()` when `ws_uri.is_none()` — no toast, no user feedback.
3. `devtools_endpoint`/`devtools_serve_pending` never reset on `AppStop`/`Exited`/`VmServiceDisconnected`. Hot restart leaves stale endpoint; user sees broken browser tab with no in-app explanation.
4. `find_by_app_id` returning `None` for non-empty `app_id` silently drops the message (no log, no toast, no fallback).
5. `VmServiceReconnected` does not retrigger `maybe_serve_devtools`; combined with stale endpoint, blocks re-attempts.

### Security Reviewer
**Verdict:** 🟠 CONCERNS — 2 HIGH, 2 MEDIUM, 1 LOW

**Security Findings:**
| Finding | Category | Severity |
|---------|----------|----------|
| No URL-scheme validation; daemon-controlled URI passed verbatim to `open`/`xdg-open` | Injection / Input Validation | HIGH |
| `devtools.serve` `host` field interpolated into `format!("http://{}:{}",..)` without sanitization | URL Injection | HIGH |
| `base_url` logged at `info!` level — auth tokens (Flutter 3.24+ DDS-integrated) leak to log files | Credential Exposure | MEDIUM |
| `browser` config field used as `Command::new(...)` without sanitization | Command Injection (local) | MEDIUM |
| `ws_uri` scheme not validated for `ws://`/`wss://` before `replacen` substitution | Defense-in-Depth | LOW |

### Documentation Freshness
**Status:** ✅ Up to date

| Doc | Updated? | Reason |
|-----|----------|--------|
| ARCHITECTURE.md | Yes | New "Browser DevTools URL (Served Endpoint)" subsection added (commit dcd92d5) |
| KEYBINDINGS.md | Yes | `B` behavior note added (commit a1c98ff) |
| CODE_STANDARDS.md | No | No new patterns established |
| DEVELOPMENT.md | No | No new build steps |

---

## Consolidated Issues

### 🔴 Critical Issues (Must Fix)

1. **[Source: bug_fix_reviewer, logic_reasoning_checker] `devtools.serve` RPC response is orphaned — fallback path is silently broken**
   - **File:** `crates/fdemon-daemon/src/protocol.rs:183-220` (definition) + `crates/fdemon-app/src/process.rs:403-415` (response demuxer) + `crates/fdemon-app/src/actions/mod.rs:861-863` (incorrect comment)
   - **Problem:** `maybe_serve_devtools` dispatches `DaemonCommand::ServeDevTools { request_id: Some("devtools-serve-{session_id}") }` — a **string** request id. The response demuxer in `process.rs` uses `id.as_u64()` which returns `None` for string IDs. Even if it routed via `RequestTracker`, nothing converts `DaemonMessage::Response` into `Message::DevToolsServed`/`DevToolsServeFailed`. `parse_devtools_serve_response` has zero production callers. On Flutter SDKs that don't emit `app.devTools`, the eager-serve dispatches successfully, the response arrives, and is silently discarded. `devtools_serve_pending` stays `true` permanently; the user is stuck with the "still starting" toast forever.
   - **Required Action:** Either (a) wire `parse_devtools_serve_response` into `process.rs::route_session_daemon_response` (correlate string request_id, parse response, emit `Message::DevToolsServed`/`DevToolsServeFailed`), or (b) remove the orphaned function and revise comments and documentation to match — `devtools.serve` is deliberately unused, primary path only.

2. **[Source: security_reviewer] No URL-scheme validation — daemon-controlled URI passed verbatim to browser opener**
   - **File:** `crates/fdemon-app/src/actions/network.rs:414-449` + `crates/fdemon-app/src/handler/devtools/mod.rs:417-438`
   - **Problem:** `app.devTools` event's `uri` field flows through `DevToolsEndpoint::base_url` → `endpoint.url(ws_uri)` → `OpenBrowserDevTools::url` → `Command::new("open"/"xdg-open"/"cmd").arg(url)` with no scheme check. A malicious Flutter project (or compromised SDK / dev dependency) can set `uri = "file:///Users/victim/.ssh/id_rsa"` or `uri = "javascript:..."` and have it opened in the system's default handler. On macOS, `open file://...` opens the file in its default viewer. On Linux, `xdg-open` behavior is handler-dependent.
   - **Required Action:** Validate `url.starts_with("http://") || url.starts_with("https://")` before invoking the OS opener. Reject with a `warn!` log and a toast on rejection. Best location: `open_url_in_browser` in `actions/network.rs`.

3. **[Source: security_reviewer] `devtools.serve` RPC `host` field interpolated unvalidated into URL**
   - **File:** `crates/fdemon-daemon/src/protocol.rs:202-208`
   - **Problem:** `format!("http://{}:{}", h, p)` uses raw daemon-supplied `host` string. `host = "127.0.0.1@evil.com"` produces a URL that browsers parse as targeting `evil.com`. `host = "127.0.0.1#"` truncates the path. No hostname-character allowlist.
   - **Required Action:** Validate `host` matches `^[a-zA-Z0-9\.\-\[\]:]+$` (covers IPv4/IPv6/hostnames) before interpolation. Combined with Critical #2's scheme check, this closes both the construction and opener surfaces. Note: this fix is only relevant if Critical #1 is resolved by wiring up the response path.

4. **[Source: logic_reasoning_checker] `devtools_endpoint` and `devtools_serve_pending` not reset on lifecycle transitions**
   - **File:** `crates/fdemon-app/src/handler/session.rs:188-234` (`AppStop`), `:95-168` (`handle_session_exited`), `crates/fdemon-app/src/handler/update.rs:1671-1726` (`VmServiceDisconnected`)
   - **Problem:** Three lifecycle handlers clear `app_id`, `ws_uri`, `vm_connected`, perf/network handles, but retain `devtools_endpoint` and `devtools_serve_pending`. After a hot restart (`AppStop → AppStart`), the old DevTools server may be cycled to a new port — but the stale endpoint persists. Pressing `B` opens a stale URL with no toast (served-endpoint branch suppresses toasts), leaving the user with a broken browser tab and no in-app indication of cause. If `devtools_serve_pending = true` when a daemon dies mid-flight, the flag is permanently stuck `true`, blocking any future fallback.
   - **Required Action:** Reset both fields to `None`/`false` in `AppStop`, `handle_session_exited`, and `VmServiceDisconnected` handlers.

5. **[Source: logic_reasoning_checker] `handle_open_browser_devtools` silent failure when `ws_uri` is None**
   - **File:** `crates/fdemon-app/src/handler/devtools/mod.rs:406-409`
   - **Problem:** Pressing `B` before VM Service is ready returns `UpdateResult::none()` with only a `warn!` log. No toast, no user feedback. Contradicts the otherwise consistent toast-on-fallback policy in the same handler 25 lines below. RESEARCH.md required a recovery toast for this scenario.
   - **Required Action:** Push a toast like `"VM Service not ready yet — wait for the app to finish launching, then press B again."` in this branch.

### 🟠 Major Issues (Should Fix)

6. **[Source: bug_fix_reviewer, logic_reasoning_checker] Eager-serve gated on `ui_mode == DevTools` blocks fallback for DevTools-first users**
   - **File:** `crates/fdemon-app/src/handler/update.rs:1540-1563`
   - **Problem:** When `state.ui_mode == UiMode::DevTools` at the moment `VmServiceConnected` fires, the handler returns `StartPerformanceMonitoring` and `maybe_serve_devtools` is never reached. The load-bearing comment ("the `app.devTools` primary event fires before `VmServiceConnected` in modern Flutter, so `devtools_endpoint` is already set") is an unverified assumption. On older Flutter (1.22-3.23) the fallback RPC is the *only* path; users in DevTools mode at connect time never get a URL.
   - **Recommended Action:** Run `maybe_serve_devtools` in both branches. The existing `devtools_endpoint.is_some()` idempotence guard already prevents redundant dispatch when the primary path works.

7. **[Source: code_quality_inspector] Duplicated `percent_encode_uri` between session.rs and handler/devtools/mod.rs**
   - **File:** `crates/fdemon-app/src/session/session.rs:37-51` + `crates/fdemon-app/src/handler/devtools/mod.rs:494`
   - **Problem:** Byte-for-byte identical implementations. The session.rs version's own comment notes the duplication. Any RFC 3986 conformance bug must be fixed in two places.
   - **Recommended Action:** Move `percent_encode_uri` to `fdemon-core` (zero-dependency) or a `crate::util` module in `fdemon-app` and import from both sites.

8. **[Source: code_quality_inspector] Magic number `4` in toast width with off-by-two against actual icon width**
   - **File:** `crates/fdemon-tui/src/render/mod.rs:452`
   - **Problem:** `area.width.saturating_sub(HORIZONTAL_PADDING * 2 + 4)` uses the literal `4`. The comment says "leave 4 chars for padding and a leading icon" but icons (`"⚠ "`, `"ℹ "`) are 2 chars each. Truncation threshold (4) and `text_width` calc (`icon.chars().count()` = 2) differ by 2, potentially causing toast overflow on terminals where emoji render double-width. CODE_STANDARDS.md Principle 4 explicitly forbids magic numbers in layout code.
   - **Recommended Action:** Define `const ICON_WIDTH: u16 = 4;` with a derivation comment; use it consistently in both the truncation threshold and `text_width` formula.

9. **[Source: security_reviewer] `base_url` logged at info! — auth tokens leak to log output**
   - **File:** `crates/fdemon-app/src/handler/update.rs:1898-1901` + `crates/fdemon-app/src/handler/devtools/mod.rs:419`
   - **Problem:** Flutter 3.24+ DDS-integrated DevTools URLs embed an auth token as a path segment (e.g., `http://127.0.0.1:59123/tbrR0DzW2j8=/devtools`). This token grants VM Service access. Logging the full URL at `info!` level writes the token to any tracing subscriber sink (file, journald, terminal captures) in plaintext.
   - **Recommended Action:** Lower to `debug!`, or redact the path segment in info-level output (`scheme://host:port/<redacted>`).

10. **[Source: architecture_enforcer] `Message::DevToolsServeFailed` handler logs but emits no toast**
    - **File:** `crates/fdemon-app/src/handler/update.rs:1912`
    - **Problem:** Failure is logged at `warn!` level but never surfaced to the user. The user only learns about failure when they press `B` and see the "not registered" toast — but by then the fallback may be misleading (says "Update Flutter" when the SDK is fine but RPC failed for a different reason).
    - **Recommended Action:** Either (a) `state.push_toast(ToastLevel::Warn, reason)` in this arm, or (b) store the reason on session state for contextual display in the DevTools panel header.

### 🟡 Minor Issues

11. **[Source: code_quality_inspector] `let _ = m` discards live `Message` value in bridge collision arm** — `handler/daemon.rs:130, 236`. Use `drop(m)` to make intent explicit; promote log from `debug!` to `warn!`.

12. **[Source: code_quality_inspector] Single `/` comment typos** in `handler/session.rs:266, 272`, plus mirrors in `daemon.rs` and `update.rs`. Should be `//` or `///`.

13. **[Source: code_quality_inspector] `DevToolsEndpoint::served_at: Instant` is dead** — `session/session.rs:70-73`. Field doc admits it's unused. Add `#[allow(dead_code)]` with tracking comment or remove.

14. **[Source: code_quality_inspector] `TOAST_TTL_SECS` comment says "≈4 seconds" but constant is 5** — `state.rs`. Fix derivation comment.

15. **[Source: code_quality_inspector] `render_toasts` has no unit test** — `crates/fdemon-tui/src/render/mod.rs`. Smoke-test with empty/Warn/Info slices would cover truncation and positioning.

16. **[Source: logic_reasoning_checker] Bridge collision should use `UpdateResult::message_and_action`** — `handler/daemon.rs:121, 229`. The infrastructure exists; current code drops `m` unnecessarily.

17. **[Source: logic_reasoning_checker] `find_by_app_id` miss is silent** — `handler/daemon.rs:71-79`. Add a `warn!` log when non-empty `app_id` fails lookup.

18. **[Source: security_reviewer] `browser` config field used as executable path without sanitization** — `actions/network.rs:415-418`. Project-local `.fdemon/config.toml` could set `browser = "/tmp/evil.sh"`. Document as security-sensitive or add allow-list.

19. **[Source: security_reviewer] `build_local_devtools_url` doesn't validate `ws_uri` scheme** — `handler/devtools/mod.rs:481-491`. Defense-in-depth gap; subsumed by Critical #2's primary fix.

20. **[Source: architecture_enforcer] `handle_open_browser_devtools` signature change from `&AppState` to `&mut AppState`** — documented design decision; consistent with project pattern but expands handler impurity. Consider `UpdateAction::PushToast` variant for stricter purity if revisiting.

---

## Regression Analysis

**Affected Code Paths:**
- `B` keypress flow (`handle_open_browser_devtools`)
- Daemon event ingestion (`handler/daemon.rs` Stdout + Message arms)
- VM Service connection lifecycle (`update.rs::VmServiceConnected`/`Reconnected`/`Disconnected`)
- App lifecycle (`AppStop`, `handle_session_exited`)

**Potential Side Effects:**
| Change | Possible Side Effect | Mitigated? |
|--------|---------------------|------------|
| Bridge `app.devTools` → `Message::DevToolsServed` | Empty `app_id` falls back to local session_id; non-empty miss silently drops | Partial — local-sid fallback is fine; miss is unhandled |
| `maybe_serve_devtools` sets `pending = true` | If RPC dies / response orphaned (Critical #1), flag stuck `true` forever | **No** |
| Eager-serve gated on `ui_mode == DevTools` | DevTools-first users never trigger fallback | **No** |
| Overwrite `devtools_endpoint` on every event | Stale re-emit overwrites fresh endpoint | **No** (no `served_at` check) |
| Lifecycle transitions don't reset endpoint/pending | Stale endpoint reused after restart; pending stuck after disconnect | **No** |
| Tick-driven `expire_toasts` | Headless mode never ticks; toasts would persist (DevTools flow is TUI-only, OK) | Acceptable |

**Test Coverage for Regression:**
- ✅ Existing tests still pass (2148+ tests, 0 failures)
- ✅ New tests added for happy path and idempotence
- ❌ No test exercises the orphaned RPC response path (would have caught Critical #1)
- ❌ No test exercises hot-restart cleanup (would have caught Critical #4)
- ❌ No test exercises ws_uri=None branch toast emission (would have caught Critical #5)

---

## Review Checklist

- [x] **Root Cause Fixed**: Primary path addresses the root cause correctly
- [ ] **No Regressions**: Stale endpoint after hot restart is a regression vector
- [ ] **Complete Fix**: RPC fallback path is dead code; not all affected paths handled
- [ ] **Tests Added**: Happy path well-covered; failure modes uncovered
- [ ] **Error Handling**: Multiple silent-failure paths
- [ ] **Security**: HIGH-severity URL injection surface

---

## ❌ REJECTION NOTICE

**Rejection Reason:** Two independent reviewers flagged a structural break in the advertised fallback path (orphaned RPC response handling), and security review found HIGH-severity URL-scheme injection. The primary path works on modern Flutter, but the fix is materially incomplete and introduces a security regression compared to the (broken-but-static) legacy URL it replaces.

### Blocking Issues Summary

| # | Issue | Severity | File | Required Fix |
|---|-------|----------|------|--------------|
| 1 | Orphaned `parse_devtools_serve_response` | Critical | `process.rs`, `protocol.rs` | Wire response into Message bridge, or remove + document |
| 2 | No URL-scheme validation before browser open | Critical (HIGH security) | `actions/network.rs` | Allow-list `http://`/`https://` schemes |
| 3 | Unvalidated `host` interpolated into URL | Critical (HIGH security) | `protocol.rs:202-208` | Character allowlist on `host` |
| 4 | Endpoint/pending leak across lifecycle | Critical | `session.rs`, `update.rs` | Reset on `AppStop`/`Exited`/`VmServiceDisconnected` |
| 5 | Silent failure when `ws_uri.is_none()` | Critical | `devtools/mod.rs:406-409` | Push toast in this branch |

### Re-review Instructions

After addressing the 5 critical issues:
1. Add tests covering the formerly-orphaned RPC response path, lifecycle cleanup, and `ws_uri=None` toast emission
2. Run `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
3. Request re-review

**Estimated Rework Effort:** Medium (1-2 days)

---

## Conclusion

**Fix Validity:** Partial. The primary `app.devTools` event path is well-implemented and fixes the bug for the common modern-Flutter case (≥3.24). The fallback path is structurally broken, leaving older SDKs and edge cases worse off than before. The security regression (daemon-controlled URI → browser opener with no validation) cannot ship.

**Next Steps:**
1. Resolve all 5 critical issues — start with the security gates (Critical #2, #3) since they unblock merge of the working primary path
2. Decide and document: wire RPC fallback (Critical #1a) OR remove it (Critical #1b)
3. Add lifecycle-cleanup tests covering hot-restart and disconnect scenarios
4. Address the 5 major issues during rework — most are small (constant naming, log redaction, gate removal)

**Re-review Required:** Yes
