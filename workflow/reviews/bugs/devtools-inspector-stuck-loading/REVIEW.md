# Bug Fix Review: DevTools Inspector Stuck on "Loading widget tree"

**Review Date:** 2026-05-12
**Reviewer:** Code Review Orchestrator
**Bug Plan:** `workflow/plans/bugs/devtools-inspector-stuck-loading/` (8 tasks)
**Files Changed:** 15 (~1,572 insertions, ~71 deletions)
**Diff Base:** `fb0fdbe0a20fb7f7ab8a96e412bde198cac21fa7`
**Branch:** `fix/devtools-improvements`

---

## Executive Summary

**Overall Verdict:** ⚠️ NEEDS WORK

The fix correctly identifies and addresses all six declared root causes — debounce-blocking after failure, runaway readiness-poll budget, silent channel drops, heuristic isolate selection, missing instrumentation, and unnecessary poll on `r` refresh. Implementation is internally consistent, test coverage is solid (25 new tests, 2190 total passing), and the architecture is respected. However, six reviewers surfaced **one performance regression** (fallback isolate-resolution path skips the cache, defeating the optimization on the common warm-up window), **one latent correctness bug** (`AutoRehydrate` documented as bypassing the poll but the code only bypasses for `Refresh`), and **one dropped commitment from BUG.md** (`IsolateExit` was supposed to invalidate the cache but doesn't). Security review also flagged that the VM Service auth token is logged in plain text via the `ws_uri`. None block the bug's primary symptom from being fixed; all should be resolved before merging to `main`.

---

## Bug Context

### Original Problem
DevTools Inspector hangs on "Loading widget tree…" on first open. `r` to retry is silently no-op'd. Reproducible in multi-isolate Flutter apps and after fetch failures.

### Root Causes Identified
1. `main_isolate_id` heuristic ("first non-system isolate") could pick a non-UI isolate in multi-isolate apps.
2. Readiness poll budget (8 × ~2 s = ~20 s) could exceed `fetch_timeout_secs`.
3. After fetch failure/timeout, 2 s debounce silently blocked retries.
4. `r` refresh after first render still ran the readiness poll (wasted budget).
5. `let _ = msg_tx.send(...)` silently dropped channel-send failures, leaving UI stuck.
6. Missing instrumentation made the failure mode invisible in logs.

### Fix Approach
- Added `info!`/`warn!` traces across the entire fetch path (task 01).
- Added `InspectorState::clear_fetch_debounce()`, called from failure/timeout handlers (task 02).
- Promoted silent channel drops to `error!` with synthetic failure fallback (task 03).
- Added `VmRequestHandle::resolve_flutter_ui_isolate()` — enumerates `extensionRPCs` looking for `ext.flutter.*` (task 04).
- Reduced poll budget to 2 × (1 s + 250 ms) = 2.5 s worst case, with three new config keys (task 05).
- Added `FetchTrigger` enum (`Initial`/`Refresh`/`AutoRehydrate`); `Refresh` skips the readiness poll; gated by sticky `has_ever_rendered_tree` flag (task 06).
- 7 new integration-style tests covering open/success/failure/timeout/refresh/multi-isolate paths (task 07).
- ARCHITECTURE.md updated with the new isolate-resolution and poll model (task 08).

---

## Changes Overview

### Files Changed

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/client.rs` | `resolve_flutter_ui_isolate` + `clear_isolate_cache` + new instrumentation (+429 lines) |
| `crates/fdemon-app/src/actions/inspector/mod.rs` | `spawn_fetch_widget_tree` rewired with trigger + config + `error!` on send drops (+176) |
| `crates/fdemon-app/src/actions/inspector/widget_tree.rs` | `ReadinessPollConfig` + bounded poll + tests (+273) |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | `clear_fetch_debounce()` on failure/timeout; record-render on success (+224) |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Initial trigger at auto-fetch sites; reads config into `ReadinessPollConfig` (+22) |
| `crates/fdemon-app/src/handler/mod.rs` | `FetchTrigger` enum; expanded `UpdateAction::FetchWidgetTree` fields (+41) |
| `crates/fdemon-app/src/handler/tests.rs` | Trigger-selection tests (+180) |
| `crates/fdemon-app/src/handler/update.rs` | Debounce/refresh instrumentation + trigger selection (+50) |
| `crates/fdemon-app/src/process.rs` | Hydration path + `error!` on channel drops (+73) |
| `crates/fdemon-app/src/state.rs` | `clear_fetch_debounce`, `has_ever_rendered_tree` (+38) |
| `crates/fdemon-app/src/config/types.rs` | 3 new TOML keys (+87) |
| `crates/fdemon-app/src/config/settings.rs` | Config-file template additions (+8) |
| `crates/fdemon-app/src/actions/mod.rs` | Trigger plumbing (+8) |
| `crates/fdemon-app/src/lib.rs` | `FetchTrigger` re-export (+2/-1) |
| `docs/ARCHITECTURE.md` | Inspector Widget Tree Fetch subsection (+32/-1) |

---

## Subagent Review Summaries

### Bug Fix Reviewer
**Verdict:** ⚠️ CONCERNS
**Root Cause Addressed:** Yes (all 6)
**Regression Risk:** Medium

All six declared root causes are correctly diagnosed and addressed. However, the fallback path in `resolve_flutter_ui_isolate` re-runs the full `getVM` + N×`getIsolate` enumeration on every call when no `ext.flutter.*` extensions are registered (the exact warm-up window the PR targets), and three auto-fetch sites set `loading = true` directly without calling `record_fetch_start()`, leaving `last_fetch_time = None` and creating a brittle invariant.

### Architecture Enforcer
**Verdict:** ✅ PASS with 1 warning + 2 suggestions

Layer boundaries are clean throughout. TEA purity is preserved: `update()` returns `UpdateAction` carrying all parameters; the actual async fetch executes in the actions layer. `fdemon-daemon` adds `resolve_flutter_ui_isolate` without leaking app-layer types. `FetchTrigger` placement in `handler/mod.rs` (next to `UpdateAction`) is architecturally sounder than the plan's `actions/inspector/mod.rs` suggestion. One warning: `FetchTrigger::AutoRehydrate` is defined but never produced — dead code.

### Code Quality Inspector
**Verdict:** ⚠️ NEEDS WORK

**Quality Scores:**

| Metric | Score |
|--------|-------|
| Language Idioms | ⭐⭐⭐⭐ |
| Error Handling | ⭐⭐⭐ |
| Testing | ⭐⭐⭐⭐ |
| Documentation | ⭐⭐⭐ |
| Maintainability | ⭐⭐⭐ |

Two major issues: the fallback-path cache miss (correctness/perf) and the `AutoRehydrate` doc/code mismatch. Minor: inconsistent tracing style (some structured, some format-string), magic strings (`"fdemon-inspector-1"`, `"devtools-layout"`), test naming convention violations, redundant `clear_isolate_cache` alias for `invalidate_isolate_cache`, and a `let _ =` without an explanatory comment.

### Logic & Reasoning Checker
**Verdict:** ⚠️ WARNING

Logic is internally consistent for happy paths. Three concerns: (1) `has_ever_rendered_tree` is sticky for session lifetime but `SessionRestartCompleted` invalidates the isolate cache without clearing the flag — post-restart `r` will pick `Refresh` (skip poll) while the framework is re-warming; (2) three auto-fetch sites set `loading = true` directly without `record_fetch_start()`, divergent from the canonical invariant; (3) `try_send` fallback in `process.rs` is explicitly acknowledged as "may stay stuck" if the channel is full.

### Risks & Tradeoffs Analyzer
**Verdict:** ⚠️ CONCERNS

| Risk | Severity | Mitigated? |
|------|----------|------------|
| `IsolateExit` does not invalidate the isolate cache (promised in BUG.md, dropped) | High | No |
| `has_ever_rendered_tree` not cleared on hot restart | Medium | No |
| Fallback isolate never cached (RPC chattiness) | Medium | Partially — intentional |
| `info!` instrumentation has no sunset plan | Medium | No |
| 3 new config keys lack bounds/clamping | Low | No |
| Config keys flat `[devtools]` (no `inspector_` prefix) | Low | No |
| `spawn_fetch_widget_tree` 9-arg + `#[allow(clippy::too_many_arguments)]` | Low | No |

### Security Reviewer
**Verdict:** ⚠️ CONCERNS
**Critical Findings:** 0

| Finding | Category | Severity |
|---------|----------|----------|
| VM Service auth token logged in plain text via `ws_uri` | Credential Exposure | Medium |
| No upper-bound validation on `readiness_poll_attempts` (u32::MAX would saturate Tokio runtime) | Input Validation / DoS | Medium |
| `&raw[..raw.len().min(120)]` can panic on non-ASCII UTF-8 (pre-existing) | Panic Safety | Low |
| Isolate ID logged at `info!` (defense-in-depth) | Information Exposure | Low |
| `resubscribe_streams` registers tracker entries with dropped receivers (pre-existing) | Resource Management | Low |

### Documentation Freshness
**Status:** ⚠️ Partial — task 08 updated ARCHITECTURE.md, but it contains a doc/code mismatch

| Doc | Updated? | Issue |
|-----|----------|-------|
| ARCHITECTURE.md | Yes | But: claims `AutoRehydrate` "follows the same bypass logic as Refresh" — the code does NOT bypass for `AutoRehydrate` |
| CODE_STANDARDS.md | N/A | No new patterns |
| DEVELOPMENT.md | N/A | No new build steps |

---

## Consolidated Issues

Findings from multiple agents referencing the same code have been deduplicated, with all source agents credited.

### 🔴 Critical Issues (Must Fix)

#### 1. `resolve_flutter_ui_isolate` fallback path does not cache → performance regression on warm-up

**[Source: bug_fix_reviewer, code_quality_inspector, risks_tradeoffs_analyzer]**

- **File:** `crates/fdemon-daemon/src/vm_service/client.rs:311-317`
- **Problem:** When no isolate has `ext.flutter.*` extensions registered (the exact state during Flutter app warm-up — the bug this PR is targeting), the fallback returns `first.id.clone()` without writing to `isolate_id_cache`. The method's own doc comment at line 229-230 states "The resolved ID is stored in the same `isolate_id_cache` as `main_isolate_id`", contradicting the implementation. Every widget tree fetch during the warm-up window runs the full `getVM` + N×`getIsolate` RPC sequence — making the very scenario this PR fixes *slower* than the original `main_isolate_id` heuristic.
- **Required Action:** Either (a) cache the fallback value to match the doc, or (b) update the doc to reflect intentional retry-on-eventual-registration semantics AND add a regression-bounded retry (e.g., short TTL or "max 3 retries before caching"). The implementor's completion note states (a) is intentional, but the regression cost is not justified for the common single-isolate case.

#### 2. `FetchTrigger::AutoRehydrate` is documented to bypass the readiness poll, but the code does not

**[Source: code_quality_inspector, architecture_enforcer, logic_reasoning_checker, risks_tradeoffs_analyzer]**

- **File:** `crates/fdemon-app/src/actions/inspector/mod.rs:93`, `docs/ARCHITECTURE.md:933`, `crates/fdemon-app/src/handler/mod.rs:91-95`
- **Problem:** ARCHITECTURE.md describes `AutoRehydrate` as "follows the same bypass logic as `Refresh`". The code checks `if trigger != FetchTrigger::Refresh { /* run full poll */ }` — meaning `AutoRehydrate` runs the *full* poll. The variant is currently dead code (no construction sites), so this is latent — but the first caller to emit `AutoRehydrate` will get the opposite of the documented behavior.
- **Required Action:** Either change the condition to `if trigger == FetchTrigger::Initial` (matching the doc), or remove `AutoRehydrate` until a caller is wired and update the doc accordingly. YAGNI argues for removal.

#### 3. `IsolateExit` does not invalidate the isolate cache (dropped BUG.md commitment)

**[Source: risks_tradeoffs_analyzer]**

- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs:311-317`
- **Problem:** `BUG.md` "Edge Cases & Risks" promises: "Invalidate the cache on `Isolate.Kill` / `Service.IsolateExit` events". `handle_isolate_event` updates `DebugState` but never calls `clear_isolate_cache()` / `invalidate_isolate_cache()`. After an uncaught Dart exception kills the root isolate (or DAP `terminate` request), the cached ID persists and every subsequent fetch RPCs against a dead isolate, producing confusing "method not found" / "isolate not found" errors. Hot restart is covered (`update.rs:222-238`), but isolate exit without restart is not.
- **Required Action:** Add `vm_handle.invalidate_isolate_cache().await` to the `IsolateEvent::IsolateExit` arm.

#### 4. VM Service auth token logged in plain text

**[Source: security_reviewer]**

- **Files:**
  - `crates/fdemon-daemon/src/vm_service/client.rs:515` (`info!("Connecting to VM Service at {}", ws_uri)`)
  - `crates/fdemon-app/src/actions/vm_service.rs:54-57` (timeout `warn!`)
- **Problem:** The Dart VM Service WebSocket URI has the form `ws://127.0.0.1:PORT/AUTH_TOKEN/ws`. Logging the full URI exposes the auth token to anyone reading log files; with that token, an attacker on the same machine can execute arbitrary VM Service RPCs (hot reload, read heap, invoke service extensions). A previous review (`workflow/reviews/bugs/browser-devtools-dds-registration/REVIEW.md:79`) flagged this class of issue.
- **Required Action:** Add a `redact_vm_service_token(&ws_uri) -> String` helper that strips the path component, and apply at all log sites that emit the URI. Alternatively, demote to `debug!`.

---

### 🟠 Major Issues (Should Fix)

#### 5. `has_ever_rendered_tree` is not cleared on hot restart → wrong trigger semantics post-restart

**[Source: logic_reasoning_checker, risks_tradeoffs_analyzer]**

- **File:** `crates/fdemon-app/src/handler/update.rs:222-238` (`SessionRestartCompleted` handler), `crates/fdemon-app/src/state.rs:250-261`
- **Problem:** The flag is documented as "Only cleared when the entire session is destroyed", but hot restart creates a new isolate with a fresh framework state. Because the flag survives, the next `r` press emits `FetchTrigger::Refresh` (skip poll) while the new framework may still be initializing. The `getRootWidgetSummaryTree` transient-error fallback partially mitigates this, but produces one cycle of user-visible error flicker.
- **Recommended Action:** Clear `has_ever_rendered_tree = false` in `SessionRestartCompleted`, alongside the existing `invalidate_isolate_cache()` call. One-line fix.

#### 6. Three auto-fetch sites set `loading = true` directly instead of calling `record_fetch_start()`

**[Source: bug_fix_reviewer, logic_reasoning_checker]**

- **File:** `crates/fdemon-app/src/handler/devtools/mod.rs:159, 221, 323`
- **Problem:** `record_fetch_start()` sets *both* `loading = true` AND `last_fetch_time = Some(now)`. The three auto-fetch sites set only `loading = true`, leaving `last_fetch_time = None`. This creates two divergent invariants. If the spawned task's terminal message is ever lost (a known acknowledged failure mode of `try_send` in `process.rs`), `loading` stays `true` forever — re-introducing the bug this PR was supposed to fix. The bug_fix_reviewer also notes this causes `RequestWidgetTree` to be debounce-blocked on subsequent presses because `is_fetch_debounced()` returns true while `loading=true`.
- **Recommended Action:** Replace the three direct assignments with `inspector.record_fetch_start()` calls. Centralizes the invariant.

#### 7. No bounds validation on `readiness_poll_*` config keys

**[Source: security_reviewer, risks_tradeoffs_analyzer]**

- **File:** `crates/fdemon-app/src/config/types.rs:402-417`, `crates/fdemon-app/src/handler/devtools/mod.rs`
- **Problem:** `readiness_poll_attempts: u32` accepts `u32::MAX`; `readiness_poll_call_timeout_ms: u64` accepts `u64::MAX`. A typo or misconfigured value can saturate the Tokio runtime for up to `inspector_fetch_timeout_secs`. The existing `fetch_timeout_secs.max(5)` pattern was not extended to the new keys.
- **Recommended Action:** Clamp at `ReadinessPollConfig` construction: e.g., `attempts ∈ [0, 20]`, `interval_ms ∈ [10, 5000]`, `call_timeout_ms ∈ [100, 10000]`. Emit `warn!` on clamp.

#### 8. `FetchTrigger::AutoRehydrate` is dead code

**[Source: architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer]**

- **File:** `crates/fdemon-app/src/handler/mod.rs:91-95`
- **Problem:** Defined, documented, exported, never constructed. Compounds with critical issue #2 (the doc/code mismatch makes it a footgun for the first caller).
- **Recommended Action:** Remove the variant until a concrete use case is implemented. Reintroduce in the same PR that adds its first caller. Aligns with YAGNI.

---

### 🟡 Minor Issues (Consider Fixing)

| # | Issue | Source |
|---|-------|--------|
| 9 | `FetchTrigger` is `pub` from `fdemon-app/lib.rs` but has no external consumer → tighten to `pub(crate)` | architecture_enforcer |
| 10 | `clear_isolate_cache` is a redundant public alias for `invalidate_isolate_cache` (test-only caller) → remove or deprecate | code_quality_inspector |
| 11 | Magic strings `"fdemon-inspector-1"` and `"devtools-layout"` → extract as `const INSPECTOR_OBJECT_GROUP` / `LAYOUT_OBJECT_GROUP` | code_quality_inspector |
| 12 | `widget_tree.rs` mixes structured-field `tracing::debug!` with format-string-style calls (lines 96-101, 118-123, 143-149) → convert to structured fields | code_quality_inspector |
| 13 | Test names violate `test_<function>_<scenario>_<expected>` convention (5 tests in `widget_tree.rs`) | code_quality_inspector |
| 14 | `isolate_id_cache` doc-comment at `client.rs:66` still says "Cached main isolate ID" — now shared with `resolve_flutter_ui_isolate` | bug_fix_reviewer |
| 15 | `info!` instrumentation has no sunset plan → add `TODO(stabilization)` markers and a tracking task | risks_tradeoffs_analyzer |
| 16 | Config keys `readiness_poll_*` lack `inspector_` prefix used by sibling keys like `inspector_fetch_timeout_secs` → rename for consistency before release | risks_tradeoffs_analyzer |
| 17 | `spawn_fetch_widget_tree` carries 9 args with `#[allow(clippy::too_many_arguments)]` → refactor to `FetchWidgetTreeOptions` struct when next arg is added | code_quality_inspector, risks_tradeoffs_analyzer |
| 18 | `reset()` in `state.rs` doesn't reset `has_ever_rendered_tree` — add inline comment explaining the intentional exclusion | bug_fix_reviewer, logic_reasoning_checker |
| 19 | UTF-8 panic risk in `&raw[..raw.len().min(120)]` at `client.rs:1009` (pre-existing) → use char-boundary-safe truncation | security_reviewer |
| 20 | `let _ =` in `send_close` (`client.rs:1082-1084`) lacks explanatory comment matching the style at `disconnect()` line 595 (pre-existing) | code_quality_inspector |
| 21 | `try_fetch_widget_tree` doc-comment lists "method not found → permanent fallback" as case 2 and "transient error" as case 3, but the code merges both via `is_transient_error` — misleading | code_quality_inspector |
| 22 | When multiple isolates have `ext.flutter.*`, the resolver picks the first found silently. Add `warn!` when ambiguity exists | risks_tradeoffs_analyzer |

---

## Regression Analysis

**Affected Code Paths:**
- Inspector fetch lifecycle: `Message::RequestWidgetTree` → `UpdateAction::FetchWidgetTree` → `spawn_fetch_widget_tree` → `poll_widget_tree_ready` → `try_fetch_widget_tree`
- VM Service isolate resolution: `resolve_flutter_ui_isolate` (new) coexists with `main_isolate_id` via shared cache
- Hot restart: `SessionRestartCompleted` invalidates the cache
- Failure paths: `clear_fetch_debounce()` resets `last_fetch_time` on failure/timeout

**Potential Side Effects:**

| Change | Possible Side Effect | Mitigated? |
|--------|---------------------|------------|
| 2.5 s poll budget vs 20 s | Slow devices / cold-start may surface RPC errors that the longer poll would have masked | Yes — warn-on-exhaust + RPC fallback to summary tree |
| `Refresh` bypasses poll | Post-hot-restart `r` may race with framework re-init | **No** — see Major Issue #5 |
| Cache shared between `main_isolate_id` and `resolve_flutter_ui_isolate` | First caller wins; performance poller could poison cache with wrong isolate | Documented but no test coverage |
| Fallback isolate never cached | RPC chattiness on non-Flutter VMs and during warm-up | **No** — see Critical Issue #1 |
| `error!` instead of silent drop on channel send failure | Increased log volume | Acceptable |
| `info!` instrumentation | Log file growth ~7-10 lines per fetch | **No sunset plan** — see Minor #15 |

**Test Coverage for Regression:**
- [x] Existing tests still pass (2190 unit tests, 0 failed)
- [x] New tests added (25 across 3 files)
- [ ] **Gap:** No test for `IsolateExit` → cache invalidation
- [ ] **Gap:** No test for hot-restart + `Refresh` trigger interaction
- [ ] **Gap:** No test for fallback-path cache behavior

---

## Review Checklist

- [x] **Root Cause Fixed**: All 6 declared root causes are addressed
- [ ] **No Regressions**: Fallback-path cache miss creates a new perf regression (Critical #1)
- [ ] **Complete Fix**: Missing `IsolateExit` invalidation (Critical #3); doc/code mismatch on `AutoRehydrate` (Critical #2)
- [x] **Tests Added**: 25 new tests; matrix coverage adequate for primary scenarios
- [x] **Error Handling**: Failure paths now log `error!` instead of silent drop
- [ ] **Security**: Auth-token-in-log issue not addressed (Critical #4)

---

## Actionable Items

### Required for Approval

1. [ ] **Resolve the fallback-path cache miss in `resolve_flutter_ui_isolate`**
   - Files: `crates/fdemon-daemon/src/vm_service/client.rs:311-317`
   - Either cache the fallback (and update the doc) or implement bounded retries before falling through
2. [ ] **Fix the `AutoRehydrate` doc/code mismatch**
   - Files: `crates/fdemon-app/src/actions/inspector/mod.rs:93`, `docs/ARCHITECTURE.md:933`
   - Either make `AutoRehydrate` bypass the poll (match docs) or remove the variant
3. [ ] **Invalidate the isolate cache on `IsolateExit`**
   - Files: `crates/fdemon-app/src/handler/devtools/debug.rs:311`
   - One-line addition fulfilling the BUG.md commitment
4. [ ] **Redact the VM Service auth token from log output**
   - Files: `crates/fdemon-daemon/src/vm_service/client.rs:515`, `crates/fdemon-app/src/actions/vm_service.rs:54-57`
   - Add a redaction helper and apply at all sites that emit `ws_uri`

### Recommended

5. [ ] **Clear `has_ever_rendered_tree` on hot restart** (`handler/update.rs:222-238`)
6. [ ] **Replace direct `loading = true` assignments with `record_fetch_start()`** (`handler/devtools/mod.rs:159, 221, 323`)
7. [ ] **Clamp the new `readiness_poll_*` config keys** to reasonable bounds
8. [ ] **Rename `readiness_poll_*` keys to `inspector_readiness_poll_*`** for naming consistency (cheaper now than after release)
9. [ ] **Remove `FetchTrigger::AutoRehydrate`** until a caller is wired
10. [ ] Address minor issues #9–22 opportunistically

---

## Conclusion

**Fix Validity:** The implementation correctly addresses the six declared root causes and the primary user-visible symptom ("Loading widget tree forever") should be eliminated for the common case. The instrumentation, test coverage, and architectural decisions are sound.

**However**, four issues should not ship as-is:
- The fallback-path cache miss creates a *new* performance regression in the same scenario the PR targets.
- The `AutoRehydrate` doc/code mismatch is a footgun for the next contributor.
- The dropped `IsolateExit` commitment from BUG.md leaves a known stale-cache failure mode unfixed.
- Auth tokens in logs is a long-standing class of issue this PR didn't introduce but adds new instances of.

The recommended Major issues (#5–8) are quick wins (1–10 line changes each) that meaningfully improve robustness and consistency.

**Next Steps:**
1. Address the four Critical issues; re-run validation
2. Apply the four Major fixes (small, well-bounded)
3. Triage the minor items into either this PR or a follow-up cleanup task
4. Plan the `info!` → `debug!` downgrade for the next release cycle

**Blocking Issues Count:** 4 Critical, 4 Major
**Re-review Required:** Yes (after Critical fixes)
