# Bugfix Plan: DevTools Inspector Stuck on "Loading widget tree"

## TL;DR

The Inspector panel shows "Loading widget tree..." forever; pressing `r` to refresh does nothing. Reference browser DevTools fetches the same widget tree instantly from the same VM Service URL, so the bug is in our VM Service inspector flow. A runtime log capture shows **zero inspector-related INFO entries** during a full DevTools session — instrumentation is missing, root cause cannot be confirmed from logs alone. The fix is sequenced as: (1) add diagnostic instrumentation, (2) capture a fresh log to confirm hypothesis, (3) apply targeted fixes (debounce-clear on failure, shorter/optional readiness poll, Flutter UI isolate resolution).

## Bug Reports

### Bug 1: Inspector stuck at "Loading widget tree", `r` does nothing

**Symptom:** Open the Inspector tab in DevTools — text reads "Loading widget tree..." indefinitely. Pressing `r` produces no visible change. Browser DevTools (`http://127.0.0.1:PORT/.../devtools/inspector?uri=ws://...`) loads the same widget tree instantly against the same Flutter session.

**Expected:** Widget tree renders within ~1 second of opening the Inspector. Pressing `r` re-fetches and re-renders.

**Root Cause Analysis (hypotheses, ranked):**

1. **🔴 Most likely — Readiness poller eats the timeout budget.** `poll_widget_tree_ready` runs up to **8 × 500 ms** with a **2 s per-call timeout** inside the outer `fetch_timeout_secs` wrap. The outer timeout can fire **during the poll loop** before the actual `try_fetch_widget_tree` call executes. Browser DevTools does not poll — it just calls the RPC and lets the framework return an error if not ready.

2. **🟠 Likely — `main_isolate_id` picks the wrong isolate.** `crates/fdemon-daemon/src/vm_service/client.rs:153-157` returns the **first non-system isolate by array position**, not the Flutter UI isolate specifically. Inspector extensions (`ext.flutter.inspector.*`) are only registered on the Flutter root isolate. If a Dart-only background isolate appears first in `getVM`, every inspector RPC returns `-32601 Method not found`. That error is classified as transient → fallback runs → fallback also fails → `WidgetTreeFetchFailed`.

3. **🟡 Possible — `r` refresh is debounce-blocked for 2 s after a failed fetch.** `record_fetch_start()` (`state.rs:292-300`) stamps `last_fetch_time` at fetch **start**. The failure/timeout handlers (`handler/devtools/inspector.rs:88, 233`) clear the `loading` flag but **do not clear `last_fetch_time`**, so the 2 s `is_fetch_debounced()` window blocks `r` retries. Reinforces "nothing happens when I press r."

4. **🟡 Possible — Spawn task's `msg_tx.send` failure is silently dropped.** `actions/inspector/mod.rs:123` `.await`s the send; if the receiver is dropped or the channel is full, the message disappears with no UI fallback (`loading=true` stays set forever).

5. **🔵 Speculative — Inspector code path emits only `debug!`/`trace!` logs.** The captured log file `/Users/ed/Dev/zabin/flutter-demon/tmp/fdemon-1778501860563-42351.log` shows **zero inspector trace lines** between VM Service connect (19:18:08) and quit (19:19:08), even though the user did enter DevTools (proven by Network monitoring start at 19:19:04). Either the inspector path has no `info!` logs, or the path was never executed at all.

**Affected Files (research evidence):**

- `crates/fdemon-app/src/actions/inspector/mod.rs:57-98` — `spawn_fetch_widget_tree` task; outer timeout wrap at line 60.
- `crates/fdemon-app/src/actions/inspector/widget_tree.rs:22-103` — readiness poller, 8 × 500 ms, 2 s per call.
- `crates/fdemon-app/src/actions/inspector/widget_tree.rs:125-154` — `try_fetch_widget_tree` with primary `getRootWidgetTree` + fallback `getRootWidgetSummaryTree`.
- `crates/fdemon-daemon/src/vm_service/client.rs:150-157` — `main_isolate_id` (first non-system isolate).
- `crates/fdemon-app/src/handler/update.rs:1877-1907` — `Message::RequestWidgetTree` handler; debounce check.
- `crates/fdemon-app/src/state.rs:292-300` — `is_fetch_debounced` / `record_fetch_start`.
- `crates/fdemon-app/src/handler/devtools/inspector.rs:20-242` — terminal message handlers; do not clear `last_fetch_time` on failure/timeout.
- `crates/fdemon-app/src/process.rs:61-90` — action hydration; silent drop if `vm_request_handle` absent.

---

## Affected Modules

- `crates/fdemon-app/src/actions/inspector/` — Add INFO-level instrumentation; revise readiness poll budget; optionally bypass readiness poll on `r` refresh.
- `crates/fdemon-daemon/src/vm_service/client.rs` — Resolve **Flutter UI isolate** by checking `Isolate.extensionRPCs` for `ext.flutter.inspector.*`; cache resolved id.
- `crates/fdemon-app/src/handler/devtools/inspector.rs` — Clear `last_fetch_time` on failure / timeout so `r` retry is not silently debounced.
- `crates/fdemon-app/src/state.rs` — Add `clear_fetch_debounce()` helper on `InspectorState`.
- `crates/fdemon-app/src/process.rs` — Promote silent message-channel drops to `error!` log.

---

## Phases

### Phase 1: Diagnostic Instrumentation (Bug 1, prerequisite) - Critical

Add INFO-level tracing across the inspector fetch path so we can confirm the root-cause hypothesis with a fresh log capture before changing behavior.

**Steps:**
1. Add `info!`/`warn!` traces in `spawn_fetch_widget_tree` at: task entry, isolate resolved (id + name), readiness poll start / each attempt result, RPC method call, response received, message dispatched.
2. Add `info!` trace in `main_isolate_id` listing **all** isolates returned by `getVM` (id + name + system flag) before selecting one.
3. Add `warn!` in `process.rs:88` on `try_send` failure (currently silent).
4. Add `info!` in `Message::RequestWidgetTree` handler indicating when debounce blocks a retry.

**Measurable Outcomes:**
- Re-run `cargo run -- <flutter-project>`, enter DevTools, observe "Loading widget tree...", press `r` twice, quit. The log must show at least one `Fetching widget tree` entry plus debounce/isolate trace lines.
- A maintainer reading the log can determine which of the 5 hypotheses fired.

### Phase 2: Debounce + Failure-Path Fixes (Bug 1, hypotheses 3 + 4) - Critical

The cheapest and safest fixes; do these even if Phase 1 logs point elsewhere.

**Steps:**
1. In `handler/devtools/inspector.rs`: `handle_widget_tree_fetch_failed` and `handle_widget_tree_fetch_timeout` must call a new `inspector_state.clear_fetch_debounce()`.
2. Add `clear_fetch_debounce()` on `InspectorState` (sets `last_fetch_time = None` or equivalent sentinel).
3. Promote `try_send` drop in `process.rs:88-90` to `error!` and record a `WidgetTreeFetchFailed` synthetic message via a non-channel fallback path (or surface in the UI as "Inspector unavailable, restart session").

**Measurable Outcomes:**
- After a fetch failure, `r` immediately triggers a new fetch (no 2 s silent block).
- New unit tests covering `clear_fetch_debounce()` and the failure/timeout handlers verify the debounce is cleared.

### Phase 3: Isolate Resolution (Bug 1, hypothesis 2) - Critical

Resolve the **Flutter UI isolate** rather than blindly picking the first non-system isolate.

**Steps:**
1. Replace `main_isolate_id` with `resolve_flutter_ui_isolate(handle)` that:
   - Calls `getVM` to list isolates.
   - For each non-system isolate, calls `getIsolate` and inspects `extensionRPCs` for an entry starting with `ext.flutter.`.
   - Returns the first isolate that has Flutter extensions registered. Falls back to current behavior if none found.
2. Cache the resolved isolate id on `VmServiceHandle` so we don't repeat the lookup on every fetch.
3. Add `info!` trace listing every isolate's extension RPC count to aid future debugging.

**Measurable Outcomes:**
- On a project with background isolates, the inspector resolves to the UI isolate (verified in the log).
- Inspector fetch succeeds where it previously failed with `-32601`.

### Phase 4: Readiness Poll Refactor (Bug 1, hypothesis 1) - Major

Shrink the readiness poll's footprint or skip it on `r` refresh.

**Steps:**
1. Reduce default poll budget to **2 × 250 ms** with a 1 s per-call timeout (configurable via `[devtools.inspector] readiness_poll_attempts` / `readiness_poll_interval_ms`).
2. Bypass readiness polling on `r` refresh: if the Inspector was previously rendered (i.e. `inspector.root.is_some()` at any point in the session), assume the framework is ready and skip the poll.
3. If `isWidgetTreeReady` returns `false` for the full short budget, log `warn!` and proceed with the fetch anyway — let the RPC error speak for itself instead of timing out silently.

**Measurable Outcomes:**
- First inspector open completes in ≤ 1.5 s on a warm Flutter session.
- `r` refresh fires the RPC within ~100 ms.
- New unit tests for the short readiness budget and the bypass-on-refresh logic.

### Phase 5: Verification + Documentation - Minor

Cross-reference doc updates and end-to-end verification.

**Steps:**
1. Cross-check the fix against `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` to ensure the RPC argument shapes match Flutter's current schema (`groupName` vs `objectGroup` etc.).
2. Update `docs/ARCHITECTURE.md` "DevTools Subsystem" section if isolate-resolution flow changed materially.
3. Manual verification: warm session, cold session, session with multiple isolates, Flutter web vs. native. Document results in the task completion summary.

---

## Edge Cases & Risks

### First-open warm-up race
- **Risk:** Skipping the readiness poll could surface a `-32601` "extension not yet registered" on the very first open immediately after `flutter run` launches before Flutter framework runs.
- **Mitigation:** Listen to `Isolate.Runnable` event (already streamed via VM service) and gate the first fetch on it. Otherwise rely on the short residual poll budget (Phase 4).

### Multiple isolates
- **Risk:** Some apps (especially `compute()` users or worker-isolate libraries) have multiple non-system isolates. Picking the wrong one breaks inspector.
- **Mitigation:** `extensionRPCs` lookup in Phase 3 is the authoritative selector — only the UI isolate registers `ext.flutter.*`.

### Cached isolate becomes stale
- **Risk:** After hot restart the isolate ID changes; cached value becomes stale.
- **Mitigation:** Invalidate the cache on `Isolate.Kill` / `Service.IsolateExit` events, or on every `vm_service_connected` notification.

### Log noise
- **Risk:** Phase 1 instrumentation adds noise to user logs.
- **Mitigation:** Keep traces at `info!` for the duration of this bug fix; downgrade to `debug!` once the issue is verified fixed.

---

## Task Dependency Graph

```
01-add-diagnostic-instrumentation        [Phase 1]
       │
       ▼
02-clear-fetch-debounce-on-failure       [Phase 2, parallel safe after 01]
03-promote-channel-drop-to-error-log     [Phase 2, parallel with 02]
       │
       ▼
04-resolve-flutter-ui-isolate            [Phase 3, depends on 01]
       │
       ▼
05-shrink-readiness-poll-budget          [Phase 4, depends on 04]
06-bypass-readiness-poll-on-refresh      [Phase 4, parallel with 05]
       │
       ▼
07-tests-inspector-handlers              [Phase 5]
08-update-architecture-doc               [Phase 5, doc_maintainer]
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Re-running fdemon and tracing inspector flow produces a clear log showing isolate selection, readiness poll attempts, RPC call/response, and any drop / timeout.
- [ ] Maintainers can determine which hypothesis fired from a single log file.

### Phase 2 Complete When:
- [ ] After fetch failure / timeout, `r` immediately triggers a new fetch.
- [ ] Unit tests verify `clear_fetch_debounce()` is called from failure/timeout paths.
- [ ] Channel-drop path logs an `error!` and surfaces user-visible state.

### Phase 3 Complete When:
- [ ] Inspector resolves to the Flutter UI isolate even when background isolates exist.
- [ ] Resolved isolate id cached on `VmServiceHandle`.
- [ ] Cache invalidated on hot restart / isolate exit.

### Phase 4 Complete When:
- [ ] First inspector open ≤ 1.5 s on warm Flutter session.
- [ ] `r` refresh fires RPC ≤ 100 ms.
- [ ] Config keys `readiness_poll_attempts` / `readiness_poll_interval_ms` honored.

### Phase 5 Complete When:
- [ ] `docs/ARCHITECTURE.md` reflects new isolate-resolution flow.
- [ ] Manual verification across cold/warm sessions and multi-isolate apps documented.

---

## Milestone Deliverable

The Inspector panel renders the widget tree within ~1.5 s of being opened on any normal Flutter project, with a working `r` refresh and a clear error message (not a stuck loading spinner) when the framework is genuinely unavailable.
