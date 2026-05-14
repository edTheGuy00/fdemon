# Action Items: DevTools Inspector Stuck Loading

**Review Date:** 2026-05-12
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 4 Critical + 4 Major

---

## Critical Issues (Must Fix)

### 1. `resolve_flutter_ui_isolate` fallback path does not cache → performance regression on warm-up
- **Source:** bug_fix_reviewer, code_quality_inspector, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-daemon/src/vm_service/client.rs:311-317`
- **Problem:** When no isolate has `ext.flutter.*` extensions, the fallback returns `first.id.clone()` without writing to `isolate_id_cache`. Every widget tree fetch during warm-up re-runs the full `getVM` + N×`getIsolate` enumeration — making the very scenario this PR targets *slower* than the original `main_isolate_id` heuristic. The method's doc-comment (line 229-230) explicitly says all paths cache; the implementation contradicts the doc.
- **Required Action:** Either (a) cache the fallback value to match the documented behavior, or (b) update the doc to reflect intentional retry-on-eventual-registration semantics AND add a bounded retry mechanism (TTL or "max N retries before caching") so the warm-up window isn't unbounded.
- **Acceptance:** A unit test asserts `cached_isolate_id()` returns `Some(_)` after a fallback resolution (or that retries are bounded). The docstring matches the implementation.

### 2. `FetchTrigger::AutoRehydrate` documented to bypass the poll, but the code only bypasses for `Refresh`
- **Source:** code_quality_inspector, architecture_enforcer, logic_reasoning_checker, risks_tradeoffs_analyzer
- **Files:**
  - `crates/fdemon-app/src/actions/inspector/mod.rs:93` (`if trigger != FetchTrigger::Refresh`)
  - `docs/ARCHITECTURE.md:933` (claims "follows the same bypass logic as `Refresh`")
  - `crates/fdemon-app/src/handler/mod.rs:91-95` (variant definition)
- **Problem:** ARCHITECTURE.md describes `AutoRehydrate` as bypassing the poll like `Refresh`, but the code does the opposite. Currently dormant (no caller), but the first emitter will get the inverse of the documented behavior.
- **Required Action:** Change the condition to `if trigger == FetchTrigger::Initial` (match docs), OR remove `AutoRehydrate` until a caller is wired and revert the doc change. YAGNI argues for removal.
- **Acceptance:** Code and doc agree; if kept, a unit test asserts `AutoRehydrate` skips the poll.

### 3. `IsolateExit` does not invalidate the isolate cache (dropped BUG.md commitment)
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs:311-317` (`handle_isolate_event`)
- **Problem:** `BUG.md` "Edge Cases & Risks" promised cache invalidation on `Isolate.Kill` / `Service.IsolateExit`. The handler updates `DebugState` but never calls `invalidate_isolate_cache()`. After an uncaught Dart exception or DAP `terminate` kills the root isolate (without a hot restart), the cached ID points to a dead isolate, and subsequent fetches produce "method not found" / "isolate not found" errors. With `FetchTrigger::Refresh` now bypassing the readiness poll, the bad state is reached faster.
- **Required Action:** Add `vm_handle.invalidate_isolate_cache().await` to the `IsolateEvent::IsolateExit` arm.
- **Acceptance:** Unit test simulates `IsolateExit` and asserts `cached_isolate_id()` returns `None`.

### 4. VM Service auth token logged in plain text
- **Source:** security_reviewer
- **Files:**
  - `crates/fdemon-daemon/src/vm_service/client.rs:515` — `info!("Connecting to VM Service at {}", ws_uri)`
  - `crates/fdemon-app/src/actions/vm_service.rs:54-57` — timeout `warn!`
- **Problem:** Dart VM Service URIs include an auth token in the path component: `ws://127.0.0.1:PORT/AUTH_TOKEN/ws`. Logging the full URI exposes the token to anyone reading log files; with that token, a local actor can execute arbitrary RPCs (hot reload, read heap, invoke service extensions). Prior review (`workflow/reviews/bugs/browser-devtools-dds-registration/REVIEW.md:79`) flagged this category.
- **Required Action:** Add a `redact_vm_service_token(uri: &str) -> String` helper that strips the path component. Apply at all `info!`/`warn!`/`error!` sites that emit the URI. Alternatively, demote URI logging to `debug!`.
- **Acceptance:** Greppable assertion: no `info!`/`warn!`/`error!` call site emits `ws_uri` directly. Unit test covers redaction.

---

## Major Issues (Should Fix)

### 5. `has_ever_rendered_tree` is not cleared on hot restart
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **Files:**
  - `crates/fdemon-app/src/handler/update.rs:222-238` (`SessionRestartCompleted` handler)
  - `crates/fdemon-app/src/state.rs:250-261` (flag definition + docstring)
- **Problem:** Hot restart creates a new isolate with a fresh framework state, but the sticky flag survives — so the next `r` emits `FetchTrigger::Refresh` and skips the readiness poll while the framework is still warming up. The `getRootWidgetSummaryTree` transient-error fallback partially masks this, but produces a one-cycle error flicker.
- **Suggested Action:** Clear `has_ever_rendered_tree = false` in the `SessionRestartCompleted` handler, alongside the existing `invalidate_isolate_cache()` call. Also update the docstring on the flag to enumerate hot restart as an intentional reset point.

### 6. Three auto-fetch sites set `loading = true` directly instead of calling `record_fetch_start()`
- **Source:** bug_fix_reviewer, logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/devtools/mod.rs:159, 221, 323`
- **Problem:** `record_fetch_start()` sets both `loading = true` AND `last_fetch_time = Some(now)`. The three direct assignments set only `loading`, leaving `last_fetch_time = None`. This creates a divergent invariant: if a spawned task's terminal message is ever lost (a known acknowledged failure mode of `try_send` in `process.rs`), `loading` stays `true` indefinitely — re-introducing the bug this PR was supposed to fix. The bug_fix_reviewer also notes that `is_fetch_debounced()` returns `true` while `loading=true`, blocking a follow-up `RequestWidgetTree` from the same loop iteration.
- **Suggested Action:** Replace each direct assignment with `inspector.record_fetch_start()`.

### 7. No bounds validation on `readiness_poll_*` config keys
- **Source:** security_reviewer (Medium), risks_tradeoffs_analyzer (Low)
- **Files:**
  - `crates/fdemon-app/src/config/types.rs:402-417`
  - `crates/fdemon-app/src/handler/devtools/mod.rs` (where `ReadinessPollConfig` is constructed)
- **Problem:** `readiness_poll_attempts: u32` accepts `u32::MAX`; `readiness_poll_call_timeout_ms: u64` accepts `u64::MAX`. A typo can saturate the Tokio runtime. The existing `fetch_timeout_secs.max(5)` defensive pattern wasn't extended.
- **Suggested Action:** Clamp at `ReadinessPollConfig` construction: e.g., `attempts ∈ [0, 20]`, `interval_ms ∈ [10, 5000]`, `call_timeout_ms ∈ [100, 10000]`. Emit `warn!` on clamp.

### 8. `FetchTrigger::AutoRehydrate` is dead code
- **Source:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/mod.rs:91-95`
- **Problem:** Defined, documented, re-exported from `lib.rs`, never constructed. Compounds with Critical Issue #2.
- **Suggested Action:** Remove the variant. Reintroduce in the same PR that adds its first caller (YAGNI).

---

## Minor Issues (Consider Fixing)

9. **`FetchTrigger` is `pub` but has no external consumer** — tighten to `pub(crate)`, remove from `lib.rs` re-export. *(architecture_enforcer)*

10. **`clear_isolate_cache` is a redundant public alias for `invalidate_isolate_cache`** — only test code uses it; remove or `#[deprecated]`. *(code_quality_inspector)*

11. **Magic strings `"fdemon-inspector-1"` and `"devtools-layout"`** — extract as `const INSPECTOR_OBJECT_GROUP` / `LAYOUT_OBJECT_GROUP` in `actions/inspector/mod.rs`. *(code_quality_inspector)*

12. **Inconsistent tracing style in `widget_tree.rs`** — lines 96-101, 118-123, 143-149 use format-string interpolation; other calls in the same function use structured fields. Convert to structured. *(code_quality_inspector)*

13. **Test naming violations** in `widget_tree.rs`: `readiness_poll_config_defaults_match_spec`, `readiness_poll_config_custom_values`, `poll_exhaustion_returns_ok_not_error`, `poll_with_zero_attempts_returns_immediately`, `poll_respects_custom_attempts_and_interval` — should follow `test_<function>_<scenario>_<expected>`. *(code_quality_inspector)*

14. **`isolate_id_cache` docstring** at `client.rs:66` still says "Cached main isolate ID" — now shared with `resolve_flutter_ui_isolate`. Update to reflect dual use. *(bug_fix_reviewer)*

15. **`info!` instrumentation has no sunset plan** — add `// TODO(stabilization)` markers at the 34 `Inspector: ...` log sites and file a follow-up tracking task to downgrade to `debug!` after one release cycle. *(risks_tradeoffs_analyzer)*

16. **Config keys `readiness_poll_*` lack `inspector_` prefix** used by sibling keys (`inspector_fetch_timeout_secs`). Rename pre-release to avoid a future migration shim. *(risks_tradeoffs_analyzer)*

17. **`spawn_fetch_widget_tree` takes 9 args with `#[allow(clippy::too_many_arguments)]`** — refactor to `FetchWidgetTreeOptions` struct when the next arg is added. *(code_quality_inspector, risks_tradeoffs_analyzer)*

18. **`InspectorState::reset()` silently skips `has_ever_rendered_tree`** — add a one-line inline comment explaining the intentional exclusion. *(bug_fix_reviewer, logic_reasoning_checker)*

19. **UTF-8 panic risk at `client.rs:1009`** (`&raw[..raw.len().min(120)]`) — pre-existing, but trivially fixable with `.chars().take(120).collect()`. *(security_reviewer)*

20. **`let _ =` in `send_close`** (`client.rs:1082-1084`) lacks explanatory comment matching the `disconnect()` style at line 595 (pre-existing). *(code_quality_inspector)*

21. **`try_fetch_widget_tree` docstring** lists "method not found" and "transient error" as distinct cases 2 and 3, but the code merges them via `is_transient_error`. Update the comment. *(code_quality_inspector)*

22. **Multi-Flutter-isolate ambiguity is silent** — `resolve_flutter_ui_isolate` picks the first match. Add `warn!` when more than one candidate has `ext.flutter.*` so misselection is observable. *(risks_tradeoffs_analyzer)*

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All 4 critical issues resolved (cache fallback, AutoRehydrate, IsolateExit, auth-token logging)
- [ ] All 4 major issues resolved or explicitly deferred with rationale
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes (with new tests added for critical fixes)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] New unit tests cover: fallback-path cache behavior, `IsolateExit` invalidation, `AutoRehydrate` behavior (or removal), redacted log output
- [ ] ARCHITECTURE.md and code agree on `FetchTrigger` semantics
