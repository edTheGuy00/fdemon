# Action Items: browser-devtools-dds-registration

**Review Date:** 2026-05-12
**Verdict:** ❌ REJECTED
**Blocking Issues:** 5
**Major Issues:** 5
**Minor Issues:** 10

---

## Critical Issues (Must Fix)

### 1. Orphaned `devtools.serve` RPC response — fallback path is silently broken

- **Source:** bug_fix_reviewer, logic_reasoning_checker
- **Files:**
  - `crates/fdemon-daemon/src/protocol.rs:183-220` (orphaned helper)
  - `crates/fdemon-app/src/process.rs:403-415` (response demuxer with type mismatch)
  - `crates/fdemon-app/src/actions/mod.rs:861-863` (misleading comment)
  - `crates/fdemon-app/src/handler/session.rs:289-294` (string request_id producer)
- **Problem:** `maybe_serve_devtools` sends `request_id: Some("devtools-serve-{session_id}")` — a string. `process.rs::route_session_daemon_response` uses `id.as_u64()` which returns `None` for string IDs, so the response is never demuxed. Even if it were, no code converts `DaemonMessage::Response` → `Message::DevToolsServed`/`DevToolsServeFailed`. `parse_devtools_serve_response` has zero callers in production. Result: on SDKs that don't fire `app.devTools`, the eager RPC succeeds, response is silently discarded, `devtools_serve_pending` stays `true` forever, user is permanently stuck with the "still starting" toast.
- **Required Action:** Pick one:
  - **(a) Wire it up:** In `process.rs::route_session_daemon_response` (or equivalent point), correlate string request IDs matching `devtools-serve-*`, parse with `parse_devtools_serve_response(result, error)`, send the resulting `Message::DevToolsServed`/`DevToolsServeFailed` through the message channel. Update `RequestTracker` or add a parallel string-keyed tracker if needed.
  - **(b) Remove it:** Delete `parse_devtools_serve_response`, the unused tests for it, the `request_id` field on `DaemonCommand::ServeDevTools` (use auto-generated numeric ID via tracker if any value retained), the eager-dispatch logic in `maybe_serve_devtools` if it can't serve a response. Update comments in `actions/mod.rs:861-863` and `session.rs:251-269` to reflect "primary path only". Update RESEARCH.md / TASKS.md to note this design decision.
- **Acceptance:** Either (a) a new test sends a `devtools.serve` response and verifies `Message::DevToolsServed` is produced; or (b) no production code path mentions the RPC fallback, comments are consistent, and the toast message is updated to reflect primary-path-only support.

### 2. No URL-scheme validation before browser open (SECURITY HIGH)

- **Source:** security_reviewer
- **Files:**
  - `crates/fdemon-app/src/actions/network.rs:414-449` (opener invocation)
  - `crates/fdemon-app/src/handler/devtools/mod.rs:417-438` (URL construction)
  - `crates/fdemon-daemon/src/protocol.rs:156-158` (parse `app.devTools` `uri` field)
- **Problem:** The `uri` field from the `app.devTools` event flows verbatim into `Command::new("open"/"xdg-open"/"cmd").arg(url)`. A malicious Flutter project or compromised pub dependency can set `uri = "file:///Users/victim/.ssh/id_rsa"` or `uri = "javascript:alert(1)"`. The OS opener invokes the platform's default handler — on macOS, `file://` opens in a viewer; on Linux, behavior is MIME-handler dependent; on Windows, `cmd /C start "" <url>` accepts any URL-like argument.
- **Required Action:** Add scheme allow-list at `open_url_in_browser` in `actions/network.rs`:
  ```rust
  if !url.starts_with("http://") && !url.starts_with("https://") {
      tracing::warn!(url = %url, "Refusing to open non-HTTP DevTools URL");
      // Optional: also push a toast
      return Err(io::Error::new(io::ErrorKind::InvalidInput, "non-http(s) URL"));
  }
  ```
- **Acceptance:** New test verifies that a `file://` or `javascript:` URL is rejected with an error. Manual test: build a Flutter app that prints `[{"event":"app.devTools","params":{"appId":"x","uri":"file:///etc/passwd"}}]` to its `--machine` stdout; verify fdemon does NOT open the file.

### 3. Unvalidated `host` field interpolated into URL (SECURITY HIGH)

- **Source:** security_reviewer
- **File:** `crates/fdemon-daemon/src/protocol.rs:202-208`
- **Problem:** `format!("http://{}:{}", h, p)` interpolates daemon-supplied `host` without character validation. `host = "127.0.0.1@evil.com"` produces a URL browsers parse as targeting `evil.com`. `host = "127.0.0.1#"` truncates path. Only non-empty check exists.
- **Required Action:** Before constructing `base_url`, validate `host` matches a hostname/IP character set. Example using a simple regex or character check:
  ```rust
  let valid_host = h.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
  if !valid_host { /* return DevToolsServeFailed with reason "invalid host" */ }
  ```
- **Acceptance:** New unit test asserts that `host = "127.0.0.1@evil.com"` produces `DevToolsServeFailed`, not `DevToolsServed`. Note: only relevant if Critical #1 path (a) is chosen.

### 4. Lifecycle transitions don't reset `devtools_endpoint` / `devtools_serve_pending`

- **Source:** logic_reasoning_checker, architecture_enforcer
- **Files:**
  - `crates/fdemon-app/src/handler/session.rs:188-234` (`AppStop` arm)
  - `crates/fdemon-app/src/handler/session.rs:95-168` (`handle_session_exited`)
  - `crates/fdemon-app/src/handler/update.rs:1671-1726` (`VmServiceDisconnected`)
- **Problem:** Three lifecycle handlers reset `app_id`, `ws_uri`, `vm_connected`, perf/network handles — but leave `devtools_endpoint` and `devtools_serve_pending` alone. Consequences:
  - After hot restart (`AppStop → AppStart`), Flutter cycles its DevTools server; the stored `base_url` points at a stale port. `B` opens a broken page with no toast (served-endpoint branch suppresses toasts).
  - If `devtools_serve_pending = true` when daemon dies, the flag stays `true` forever — `maybe_serve_devtools` refuses to re-dispatch.
- **Required Action:** In all three handlers, add:
  ```rust
  session.session.devtools_endpoint = None;
  session.session.devtools_serve_pending = false;
  ```
- **Acceptance:** New unit tests:
  - `app_stop_resets_devtools_endpoint`
  - `handle_session_exited_resets_devtools_state`
  - `vm_service_disconnected_clears_pending_flag`

### 5. Silent failure on `B` when `ws_uri.is_none()`

- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/devtools/mod.rs:406-409`
- **Problem:** Early-return with only `warn!` log when `ws_uri` is None. No toast, no user feedback. Inconsistent with the toast-on-failure policy used 25 lines below.
- **Required Action:** Push a toast in this branch:
  ```rust
  let Some(ref ws_uri) = session_handle.session.ws_uri else {
      tracing::warn!("Cannot open browser DevTools: no VM Service URI available");
      state.push_toast(ToastLevel::Warn, "VM Service not ready yet — try again once the app finishes launching.");
      return UpdateResult::none();
  };
  ```
- **Acceptance:** New unit test `no_ws_uri_emits_toast` verifies the toast is queued.

---

## Major Issues (Should Fix)

### 6. Eager-serve gated on `ui_mode == DevTools` blocks fallback

- **Source:** bug_fix_reviewer, logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/update.rs:1540-1563`
- **Problem:** When user is in DevTools mode at `VmServiceConnected` time, `maybe_serve_devtools` is skipped. Comment claims `app.devTools` fires first, but this is unverified across all SDK versions.
- **Suggested Action:** Run `maybe_serve_devtools` in both branches. Idempotence guards (`devtools_endpoint.is_some()`) already prevent redundant dispatch. Returns either `StartPerformanceMonitoring` + a deferred `Message::SendDevToolsServe` (using `UpdateResult::message_and_action`), or restructure to dispatch both.

### 7. Duplicated `percent_encode_uri`

- **Source:** code_quality_inspector
- **Files:** `crates/fdemon-app/src/session/session.rs:37-51` + `crates/fdemon-app/src/handler/devtools/mod.rs:494`
- **Suggested Action:** Move to `fdemon-core` (or a `crate::util` module in `fdemon-app`) and import from both call sites. Comment acknowledging the duplication should not exist in shipped code.

### 8. Magic number `4` in toast width / off-by-two with icon

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-tui/src/render/mod.rs:452`
- **Suggested Action:** Define `const ICON_WIDTH: u16 = 4;` with a derivation comment. Use consistently in both `max_text_chars` calculation and `text_width` formula. Reconcile with `icon.chars().count()` returning 2.

### 9. `base_url` logged at info! level — auth token leak (SECURITY MEDIUM)

- **Source:** security_reviewer
- **Files:**
  - `crates/fdemon-app/src/handler/update.rs:1898-1901`
  - `crates/fdemon-app/src/handler/devtools/mod.rs:419`
- **Suggested Action:** Either change both call sites to `debug!`, or redact path segment for `info!` output (`scheme://host:port/...`).

### 10. `Message::DevToolsServeFailed` handler logs but emits no toast

- **Source:** architecture_enforcer
- **File:** `crates/fdemon-app/src/handler/update.rs:1912`
- **Suggested Action:** Either `state.push_toast(ToastLevel::Warn, reason)` in this arm, or store the failure reason on session state for contextual display.

---

## Minor Issues (Consider Fixing)

11. **`let _ = m` should be `drop(m)` + promote `debug!` to `warn!`** — `handler/daemon.rs:130, 236`. Makes intent explicit. [Source: code_quality_inspector]

12. **Single `/` comment typos** in `handler/session.rs:266, 272` and mirrors. [Source: code_quality_inspector]

13. **`DevToolsEndpoint::served_at` is dead** — add `#[allow(dead_code)]` with TODO or remove. [Source: code_quality_inspector]

14. **`TOAST_TTL_SECS` derivation comment says "≈4 seconds" but constant is 5** — fix arithmetic or explain choice. [Source: code_quality_inspector]

15. **`render_toasts` has no unit test** — add smoke test with empty/Warn/Info slices. [Source: code_quality_inspector]

16. **Bridge collision should use `UpdateResult::message_and_action`** — `handler/daemon.rs:121, 229`. Infrastructure exists. [Source: logic_reasoning_checker]

17. **`find_by_app_id` miss is silent** — add `warn!` log when non-empty `app_id` fails lookup. [Source: logic_reasoning_checker, bug_fix_reviewer]

18. **`browser` config field used as executable path** — add allow-list or document as security-sensitive. [Source: security_reviewer]

19. **`build_local_devtools_url` doesn't validate `ws_uri` scheme** — defense-in-depth. Subsumed by Critical #2's primary fix. [Source: security_reviewer]

20. **Consider `UpdateAction::PushToast` variant** for strict TEA purity — handlers wouldn't need `&mut AppState` just to push toasts. Long-term refactor only. [Source: architecture_enforcer]

---

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All 5 critical issues resolved
- [ ] All 5 major issues resolved or explicitly justified in commit messages
- [ ] Minor issues addressed where they don't introduce risk
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] New tests for:
  - [ ] RPC response routing (or removal documented if Critical #1b chosen)
  - [ ] URL scheme rejection
  - [ ] Host character validation (if Critical #1a chosen)
  - [ ] Lifecycle reset (AppStop, Exited, VmServiceDisconnected)
  - [ ] `ws_uri=None` toast emission
- [ ] Manual verification on real Flutter project (modern SDK + ideally an older one)
- [ ] RESEARCH.md and TASKS.md updated to reflect any design changes
