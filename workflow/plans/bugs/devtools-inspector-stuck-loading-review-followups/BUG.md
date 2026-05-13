# Bugfix Plan: DevTools Inspector Stuck Loading — Review Followups

## TL;DR

The 6-agent review of `fix/devtools-improvements` (see `workflow/reviews/bugs/devtools-inspector-stuck-loading/`) surfaced 4 critical, 4 major, and ~6 minor issues. The critical issues block merge to main: a new perf regression from the fallback isolate-resolution path, a doc/code mismatch on the unused `FetchTrigger::AutoRehydrate` variant, a dropped BUG.md commitment (cache invalidation on `IsolateExit`), and VM Service auth tokens being logged in plain text. This plan addresses them in three phases.

## Bug Reports

### Bug 1: Fallback isolate resolution doesn't cache → N+1 RPC pattern on every fetch during Flutter app warm-up

**Symptom:** During the warm-up window (extensions not yet registered), `resolve_flutter_ui_isolate` re-runs `getVM` + N×`getIsolate` on every widget tree fetch.

**Expected:** Resolution result is cached on every path; hot restart invalidates.

**Root Cause Analysis:**
1. `crates/fdemon-daemon/src/vm_service/client.rs:311-317` returns `first.id.clone()` without writing to `isolate_id_cache`.
2. The method's own doc comment (lines 229-230) claims all paths cache — implementation contradicts doc.
3. Original `main_isolate_id` heuristic always cached; new path made the *common* case slower than the code it replaced.

**Affected Files:**
- `crates/fdemon-daemon/src/vm_service/client.rs` — add cache write to fallback path

---

### Bug 2: `FetchTrigger::AutoRehydrate` is dead code with a doc/code mismatch

**Symptom:** ARCHITECTURE.md claims `AutoRehydrate` bypasses the readiness poll like `Refresh`. The code only bypasses for `Refresh`. The variant has no construction sites anywhere.

**Expected:** No dead enum variants in the public API; doc and code agree.

**Root Cause Analysis:**
1. Variant defined in `handler/mod.rs:91-95` with a doc comment that contradicts ARCHITECTURE.md:933.
2. Re-exported from `lib.rs` as part of the crate's public API despite no external use.
3. Guard at `actions/inspector/mod.rs:93` only matches `Refresh`.

**Affected Files:**
- `crates/fdemon-app/src/handler/mod.rs`
- `crates/fdemon-app/src/actions/inspector/mod.rs`
- `crates/fdemon-app/src/lib.rs`
- `docs/ARCHITECTURE.md`

---

### Bug 3: `IsolateExit` does not invalidate the isolate cache (dropped BUG.md commitment)

**Symptom:** After an uncaught Dart exception kills the root isolate (or DAP `terminate` without restart), the cached isolate ID points to a dead isolate. Subsequent fetches produce confusing "method not found" / "isolate not found" errors.

**Expected:** `IsolateExit` invalidates the cache so the next fetch re-resolves.

**Root Cause Analysis:**
1. `crates/fdemon-app/src/handler/devtools/debug.rs:311-317` updates `DebugState` on `IsolateEvent::IsolateExit` but never calls `invalidate_isolate_cache()`.
2. BUG.md "Edge Cases & Risks" promised this invalidation; implementation dropped it silently.
3. With `FetchTrigger::Refresh` now bypassing the readiness poll, the stale-cache state is reached faster.

**Affected Files:**
- `crates/fdemon-app/src/handler/devtools/debug.rs`

---

### Bug 4: VM Service WebSocket auth token logged in plain text

**Symptom:** Log files contain entries like `Connecting to VM Service at ws://127.0.0.1:PORT/AUTH_TOKEN/ws`. The `AUTH_TOKEN` is a credential — anyone with read access to the log file can connect to the Dart VM and execute arbitrary service RPCs (hot reload, read heap, invoke service extensions).

**Expected:** Auth token redacted in log output. The `port` and `host` are sufficient context for debugging.

**Root Cause Analysis:**
1. `crates/fdemon-daemon/src/vm_service/client.rs:515` — `info!("Connecting to VM Service at {}", ws_uri)`.
2. `crates/fdemon-app/src/actions/vm_service.rs:54-57` — timeout `warn!` interpolates raw URI.
3. No redaction helper exists in the codebase.
4. A previous review (`workflow/reviews/bugs/browser-devtools-dds-registration/REVIEW.md`) flagged this category as Medium severity.

**Affected Files:**
- `crates/fdemon-daemon/src/vm_service/client.rs`
- `crates/fdemon-daemon/src/vm_service/` (new redact helper)
- `crates/fdemon-app/src/actions/vm_service.rs`

---

### Bug 5: `has_ever_rendered_tree` survives hot restart → `Refresh` trigger races with framework re-init

**Symptom:** After hot restart, pressing `r` emits `FetchTrigger::Refresh` (skipping the readiness poll) while the new framework is still warming up. User sees a one-cycle error flicker before the next press succeeds.

**Expected:** Hot restart resets the sticky flag so the next fetch uses `Initial` (full poll budget).

**Root Cause Analysis:**
1. `Message::SessionRestartCompleted` handler (`handler/update.rs:222-238`) invalidates the isolate cache but doesn't touch `inspector.has_ever_rendered_tree`.
2. Flag docstring (`state.rs:250-261`) says "Only cleared when the entire session is destroyed" — but hot restart creates a new isolate and framework state, invalidating the "framework is warm" invariant.

**Affected Files:**
- `crates/fdemon-app/src/handler/update.rs`
- `crates/fdemon-app/src/state.rs` (docstring update)

---

### Bug 6: Three auto-fetch sites set `loading = true` directly, leaving `last_fetch_time = None`

**Symptom:** Three sites in `handler/devtools/mod.rs` (lines 159, 221, 323) set `inspector.loading = true` without calling `record_fetch_start()`. `is_fetch_debounced()` returns `true` while `loading=true`, and if a spawned task's terminal message is ever lost, `loading` stays `true` permanently — re-introducing the bug this PR was supposed to fix.

**Expected:** All paths that mark the inspector as fetching use `record_fetch_start()` so the invariant is enforced centrally.

**Root Cause Analysis:**
1. `record_fetch_start()` (`state.rs:351-354`) sets both flags atomically.
2. The three direct assignments diverge from this canonical invariant.

**Affected Files:**
- `crates/fdemon-app/src/handler/devtools/mod.rs`

---

### Bug 7: New `readiness_poll_*` config keys lack bounds validation

**Symptom:** A typo or paste error in `.fdemon/config.toml` (e.g., `readiness_poll_attempts = 4294967295`) saturates the Tokio runtime for up to `inspector_fetch_timeout_secs` seconds.

**Expected:** Values clamped at config application; warn on clamp.

**Root Cause Analysis:**
1. `crates/fdemon-app/src/config/types.rs:396-417` deserializes raw `u32`/`u64` with no clamp.
2. Sibling key `fetch_timeout_secs.max(5)` shows the existing defensive pattern; not extended to new keys.

**Affected Files:**
- `crates/fdemon-app/src/handler/devtools/mod.rs` (where settings are read into `ReadinessPollConfig`)

---

### Bug 8: New config keys are flat under `[devtools]` without the `inspector_` prefix used by siblings

**Symptom:** `readiness_poll_attempts` (no prefix) sits next to `inspector_fetch_timeout_secs` (prefixed). Future `network` or `performance` readiness logic would collide.

**Expected:** Inspector-scoped keys carry the `inspector_` prefix matching existing convention.

**Root Cause Analysis:**
1. `crates/fdemon-app/src/config/types.rs` introduced flat keys.
2. Pre-release rename costs nothing; post-release would require a migration shim.

**Affected Files:**
- `crates/fdemon-app/src/config/types.rs`
- `crates/fdemon-app/src/config/settings.rs`
- All call sites that read the keys

---

## Phases

### Phase 1: Critical fixes (block merge to main)

**Tasks:** 01, 02, 03, 04, 05

**Goal:** Resolve the four issues that introduce regressions or violate the BUG.md plan. Phase 1 must merge before `fix/devtools-improvements` lands on main.

**Measurable Outcomes:**
- Fallback path writes to cache (unit test asserts `cached_isolate_id().is_some()`)
- `FetchTrigger::AutoRehydrate` no longer exists in the codebase
- `IsolateExit` invalidates the cache (unit test asserts post-exit `cached_isolate_id() == None`)
- No log site emits raw `ws_uri` (greppable invariant + unit test on the redact helper)

---

### Phase 2: Major fixes (before next release)

**Tasks:** 06, 07, 08, 09

**Goal:** Close behavioral gaps that don't block merge but should ship before the next release. Includes the pre-release config-key rename (cheap now, expensive later).

**Measurable Outcomes:**
- Hot restart clears `has_ever_rendered_tree`; next `r` uses `Initial`
- All `loading = true` writes go through `record_fetch_start()`
- Out-of-bounds config values are clamped with a `warn!`
- Config keys renamed to `inspector_readiness_poll_*`

---

### Phase 3: Minor cleanups (post-release acceptable)

**Tasks:** 10, 11, 12

**Goal:** Address style, API hygiene, and observability gaps. Lower priority; can ship in a follow-up PR.

**Measurable Outcomes:**
- `FetchTrigger` narrowed to `pub(crate)`; redundant `clear_isolate_cache` removed
- Magic strings replaced with named constants; tracing style consistent
- `info!` Inspector sites carry `TODO(stabilization)` markers tied to a tracking task

---

## Edge Cases & Risks

### Cache write race on fallback path
- **Risk:** Two concurrent calls to `resolve_flutter_ui_isolate` during warm-up could both run the full scan, both find no `ext.flutter.*`, and both attempt to write the cache. Writes are last-writer-wins.
- **Mitigation:** Acceptable — both writes set the same value (the first non-system isolate id is deterministic from the `getVM` enumeration). The cache lock serializes the writes.

### Hot-restart sequencing for `has_ever_rendered_tree`
- **Risk:** Resetting the flag synchronously in `SessionRestartCompleted` is correct only if `SessionRestartCompleted` fires *after* the new isolate is live. If it races with a `RequestWidgetTree` from the user, the next fetch could be `Refresh` against a still-warming framework.
- **Mitigation:** The existing `try_fetch_widget_tree` transient-error fallback (summary tree) already handles this. No additional sequencing needed.

### Config key rename + user config files
- **Risk:** Users who already have `readiness_poll_attempts` in their `.fdemon/config.toml` will silently fall back to defaults after the rename.
- **Mitigation:** Branch hasn't been released. No users have these keys yet. No shim needed.

### AutoRehydrate removal
- **Risk:** Future work may need a third trigger variant; removing prematurely costs the cognitive load of re-adding it.
- **Mitigation:** YAGNI per user direction. Reintroduce in the same PR that adds its first caller; the type system + test suite will guard the next change.

---

## Further Considerations

1. **Pre-existing issues out of scope.** UTF-8 panic risk in `client.rs:1009` and `let _ =` in `send_close` (`client.rs:1082-1084`) pre-date this branch. Track as a separate hygiene pass, not in this followup plan.

2. **9-arg `spawn_fetch_widget_tree` refactor** is deferred per the reviewer's recommendation — defer until the next arg-add forces it.

3. **`info!` → `debug!` downgrade** is intentionally not done in this plan. Instrumentation is still load-bearing for verifying the bug stays fixed in the field. Phase 3 task 12 only adds tracking markers; the actual downgrade should be a separate task after one release cycle of observation.

---

## Task Dependency Graph

```
Phase 1
├── 01-cache-fallback-isolate-resolution
├── 02-remove-autorehydrate-variant
│   └── 03-update-architecture-autorehydrate (doc_maintainer)
├── 04-invalidate-cache-on-isolate-exit
└── 05-redact-vm-service-uri-in-logs
        (writes vm_service/client.rs → sequential with 01)

Phase 2 (after Phase 1)
├── 06-clear-render-flag-on-hot-restart
├── 07-use-record-fetch-start-at-auto-fetch-sites
│   └── 08-clamp-readiness-poll-config
│         (writes handler/devtools/mod.rs → sequential with 07)
└── 09-rename-readiness-poll-config-keys
        (writes config/types.rs + dispatch sites)

Phase 3 (after Phase 2)
├── 10-api-hygiene-cleanup
├── 11-code-style-sweep
└── 12-observability-followups
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `resolve_flutter_ui_isolate` writes the cache on both success and fallback paths; unit test added
- [ ] `FetchTrigger::AutoRehydrate` removed from `handler/mod.rs`, `lib.rs` export, and `ARCHITECTURE.md`
- [ ] `IsolateEvent::IsolateExit` arm in `debug.rs` calls `invalidate_isolate_cache()`; unit test added
- [ ] No `info!`/`warn!`/`error!` log emits raw `ws_uri`; redaction helper added with unit tests
- [ ] All CI quality gates pass

### Phase 2 Complete When:
- [ ] `SessionRestartCompleted` resets `has_ever_rendered_tree`; unit test asserts post-restart trigger is `Initial`
- [ ] Three direct `inspector.loading = true` sites replaced with `inspector.record_fetch_start()`
- [ ] `readiness_poll_*` values clamped to bounded ranges with `warn!` on clamp
- [ ] Config keys renamed to `inspector_readiness_poll_*`; sample config updated

### Phase 3 Complete When:
- [ ] `FetchTrigger` is `pub(crate)`; not re-exported from `lib.rs`
- [ ] `clear_isolate_cache` removed; callers updated to `invalidate_isolate_cache`
- [ ] Magic strings `"fdemon-inspector-1"` and `"devtools-layout"` extracted as named constants
- [ ] Tracing calls in `widget_tree.rs` use structured fields consistently
- [ ] 5 non-conforming test names renamed
- [ ] `TODO(stabilization)` markers placed at 34 `Inspector:` log sites

---

## Milestone Deliverable

`fix/devtools-improvements` becomes mergeable to main after Phase 1. Phase 2 wraps before the next release tag. Phase 3 cleans up minor debt at convenience.
