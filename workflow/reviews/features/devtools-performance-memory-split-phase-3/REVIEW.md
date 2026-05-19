# Phase 3 Review — DevTools Performance: Rebuild Stats + Timeline Events

**Reviewed:** 2026-05-19
**Diff base:** `4feb38e` (plan commit)
**HEAD:** `65d2286` on `feat/devtools-inspector-parity`
**Files changed:** 36 (+5,187 / -115)
**Overall verdict:** ⚠️ **NEEDS WORK**

## Verdict Breakdown

| Agent | Verdict |
|---|---|
| architecture_enforcer | ⚠️ CONCERNS (0 critical, 2 warnings) |
| code_quality_inspector | ⚠️ NEEDS WORK (1 logic-doc contradiction, DRY violation, magic numbers) |
| logic_reasoning_checker | ⚠️ CONCERN (1 high-priority gap, 4 warnings) |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS (3 HIGH severity, 5 MEDIUM) |
| security_reviewer | ⚠️ CONCERNS (1 HIGH, 3 MEDIUM) |

Per the skill's rubric, multiple agents returning CONCERN combined with `code_quality_inspector`'s explicit NEEDS WORK and three HIGH-severity items that contradict the phase's own acceptance criteria → **NEEDS WORK**.

## What works well

- Layer boundaries clean across all 4 crates. New `fdemon-core` modules (`rebuild_stats`, `timeline`) are dependency-free; daemon and app layers use them appropriately.
- TEA pattern preserved: all 7 new `Message` variants dispatch through `update()`; handler modules are pure state transitions; side effects returned via `UpdateAction`.
- Per-session DevTools state on `PerformanceState` and `SessionHandle` mirrors the `MemoryState`/`NetworkState` pattern from prior phases.
- Ring buffer choice (`VecDeque` with `push_back`/`pop_front`) is O(1); eviction loop `while len > cap` is guarded by `.max(1)` against zero-size config.
- Render-hint Cell write-back (`details_pane_visible_height.set(...)`) carries the required `// EXCEPTION:` annotation in both new tabs.
- Hot-restart re-enable of `profileWidgetBuilds` correctly invalidates the isolate cache before re-issuing the RPC.
- Five session-stop paths cleaned up (spec called for three) — defensive correctness.
- No credential exposure, no shell-injection surface, no cross-session channel leakage. WebSocket auth token redaction is reused correctly.

## Cross-cutting Findings

### 🔴 Critical (block release)

#### C1 — `timeline_pause_tx` not signaled on DevTools exit ⚠️ flagged by 3 agents
**Files:** `crates/fdemon-app/src/handler/devtools/mod.rs:355–393`
**Sources:** logic_reasoning_checker (W1), security_reviewer (HIGH), risks_tradeoffs_analyzer (R#2)

`handle_exit_devtools_mode` signals `perf_pause_tx`, `alloc_pause_tx`, and `network_pause_tx` but omits `timeline_pause_tx`. When the user presses Esc to leave DevTools entirely (rather than switching panels via `i`/`m`/`n`), the 1 Hz `getVMTimeline` polling loop keeps running for the entire session. Directly violates Phase 3 success criterion at `TASKS.md:110`:

> Switching away from the Performance panel (`i`/`m`/`n` or **Esc**) **stops** the 1-Hz timeline polling.

#### C2 — Timeline event buffer never cleared on Performance leave
**Files:** `crates/fdemon-app/src/handler/devtools/mod.rs` (panel-switch + exit paths)
**Source:** risks_tradeoffs_analyzer (R#2, HIGH)

Phase 3 success criterion at `TASKS.md:114`:

> Switching away from Performance clears the buffer (per PLAN.md §7.5 mitigation).

A grep for `timeline_events.clear()` returns no hits in `handler/`. The up-to-1000-event buffer remains resident across panel-switches, and stale events appear on re-entry until 1000 new events overwrite them. ~100–200 KB retained per session × up to 9 sessions.

#### C3 — `docs/CONFIGURATION.md` documents wrong TOML key names for inspector-readiness keys
**Files:** `docs/CONFIGURATION.md:345–348` (and surrounding example block)
**Source:** risks_tradeoffs_analyzer (R#3, HIGH); already logged as Task 07 CONCERN

The doc lists `readiness_poll_attempts`, `readiness_poll_interval_ms`, `readiness_poll_call_timeout_ms`. The actual serde field names in `crates/fdemon-app/src/config/types.rs:402,410,417` carry an `inspector_` prefix. `DevToolsSettings` does **not** use `deny_unknown_fields`, so the wrong key is silently ignored (proven by the regression test `test_old_readiness_poll_key_does_not_silently_override_default` at types.rs:1985). Any user copy-pasting from docs gets defaults with no error.

### 🟠 High

#### H1 — `Flutter.RebuiltWidgets` events processed regardless of panel visibility
**Files:** `crates/fdemon-app/src/actions/vm_service.rs:170–199`
**Source:** risks_tradeoffs_analyzer (R#1, HIGH)

Once `profileWidgetBuilds` is ON, events at 60 fps are parsed, allocated, MPSC-sent, and run through the handler even when the user is on Logs/Inspector/Memory/Network. Per-frame: payload alloc + Vec/HashMap construction + ring-buffer churn. The toggle does not auto-pause the extension itself, so fdemon-side gating is the only mitigation.

#### H2 — `classify_thread` docstring contradicts implementation
**Files:** `crates/fdemon-core/src/timeline.rs:14–23` (docstring) vs `:191–193` (code)
**Source:** code_quality_inspector (Major #1)

Docstring promises an exclusion guard for `.flutter.test..ui`; code applies a bare `.contains(".ui")`. The test for the tester thread passes only because `"io.flutter.test..ui"` happens to contain `".ui"` — accidental match, not by design. A reader trusting the docstring will be misled.

#### H3 — Silent RPC failures in `ToggleProfileWidgetBuilds` and `FetchWidgetLocationIdMap`
**Files:** `crates/fdemon-app/src/actions/mod.rs:~1036–1061, ~1085–1115`
**Sources:** logic_reasoning_checker (W2), risks_tradeoffs_analyzer (R#6, R#9)

On RPC error, only `tracing::warn!` fires. No `RebuildStatsExtensionStateChanged` is emitted to roll back optimistic UI state or surface the failure. User presses `R`, nothing happens, no feedback. Same pattern for the location-map fetch — if it fails, all subsequent events show empty `rebuilds` with no error indication.

### 🟡 Medium

#### M1 — Tab cycle does not skip hidden `RebuildStats` tab in state
**Files:** `crates/fdemon-app/src/handler/devtools/performance/details.rs:18–23`; `crates/fdemon-app/src/state.rs:198–204`
**Source:** logic_reasoning_checker (W3)

`PerfDetailsTab::next()` always cycles `FrameAnalysis → RebuildStats → TimelineEvents → FrameAnalysis`. When `rebuild_stats_enabled == false`, the renderer's `effective_tab()` falls through, so press 1 of `]` and press 2 produce the same visible result — one apparent "dead" press. Phase 3 manual acceptance plan at `TASKS.md:136` says: *"Verify it cycles `FrameAnalysis → TimelineEvents → FrameAnalysis` (RebuildStats is skipped)"*. State machine does not honor this.

#### M2 — `auto_enable_rebuild_tracking` config setting plumbed but never read
**Files:** `crates/fdemon-app/src/config/types.rs:430–431`
**Sources:** logic_reasoning_checker (W4), architecture_enforcer (Rec #3)

Declared, defaulted, parsed, documented in CONFIGURATION.md — but no production code path consults it. Setting to `true` in `.fdemon/config.toml` is a no-op. Either wire on `VmServiceConnected`/`SessionStarted` or remove.

#### M3 — Hot-restart re-enable can clobber user's toggle-OFF during restart window
**Files:** `crates/fdemon-app/src/handler/update.rs:222–264`; `crates/fdemon-app/src/actions/mod.rs:1045–1051`
**Source:** risks_tradeoffs_analyzer (R#4)

If user presses `R` to disable during the brief restart window, the toggle RPC fails (dying isolate), `RebuildStatsExtensionStateChanged` is never emitted (see H3), `rebuild_stats_enabled` stays `true`, and `SessionRestartCompleted` re-enables. User's OFF intent is silently lost.

#### M4 — `R` is a silent no-op in DevTools-mode non-RebuildStats contexts
**Files:** `crates/fdemon-app/src/handler/keys.rs:544–555`; test at `:2167–2173`
**Source:** risks_tradeoffs_analyzer (R#5); already logged as Task 06 CONCERN

In Inspector/Memory/Network or Performance with FrameChart/FrameAnalysis/TimelineEvents focused, `R` returns `None` instead of falling through to global `HotRestart`. Surprising for muscle memory. Two options: (a) relax the early-return so `R` falls through outside RebuildStats, or (b) explicit footer hint.

#### M5 — `enable_frame_tracking` uses string literal instead of `ext::PROFILE_WIDGET_BUILDS`
**Files:** `crates/fdemon-daemon/src/vm_service/timeline.rs:147`
**Source:** architecture_enforcer (Warning)

Phase 3 introduced the constant for exactly this purpose. Pre-existing call site was not migrated. Refactor liability — a rename of the extension wouldn't be caught.

#### M6 — `FetchWidgetLocationIdMap` parsing inline in `actions/mod.rs` instead of daemon helper
**Files:** `crates/fdemon-app/src/actions/mod.rs:~1085–1115`
**Source:** architecture_enforcer (Warning)

`inspector::widget_location_id_map()` exists and returns a `LocationMap`. The action task should call it and ship the result via Message; instead it inlines the file-URI loop and merge. Departs from the pattern used by `get_allocation_profile` → `VmServiceAllocationProfileReceived`.

#### M7 — Missing integration test for `spawn_timeline_polling` pause/resume/shutdown
**Files:** `crates/fdemon-app/src/actions/performance.rs`
**Sources:** code_quality_inspector (#4), risks_tradeoffs_analyzer (R-tech-debt #1); already logged as Task 04 CONCERN

Acceptance criterion 12 explicitly required this. Lack of mock `VmRequestHandle` infra is real, but a trait abstraction would also unblock the existing `spawn_performance_polling`.

#### M8 — Watermark advancement off-by-one in `spawn_timeline_polling`
**Files:** `crates/fdemon-app/src/actions/performance.rs:603`
**Source:** risks_tradeoffs_analyzer (R#8)

`last_poll_micros = now_micros.saturating_add(1)` where `now_micros` was captured **before** `fetch_timeline_chunk` ran. Events with `ts ∈ [now_micros, fetch_completion_time]` can be dropped under load. Capture `now_micros` after fetch completes (or use `max(ts)` from the response).

#### M9 — Duplicated text helpers (`truncate_with_ellipsis`, `pad_right`, `pad_left`)
**Files:** `rebuild_stats_tab.rs:303–333` and `timeline_events_tab.rs:301–327` (byte-identical)
**Source:** code_quality_inspector (Major #2)

A third near-identical implementation already exists in `widgets/new_session_dialog/mod.rs:65`. Extract to a shared module.

### 🔵 Minor

| ID | Source | Summary |
|---|---|---|
| L1 | code_quality_inspector (#3), security_reviewer | Magic number `200` ms timeline poll floor in `actions/performance.rs:503` — needs named `TIMELINE_POLL_MIN_MS` constant + derivation comment |
| L2 | security_reviewer | `u64 → i64` cast in `fdemon-daemon/vm_service/timeline.rs:221–223` lacks ceiling guard `since_micros.min(i64::MAX as u64) as i64` |
| L3 | security_reviewer | `Flutter.RebuiltWidgets` parse-error logs at `warn!` level — at 60 fps a pathological app floods logs. Downgrade to `debug!` or rate-limit |
| L4 | code_quality_inspector (#10), CODE_STANDARDS Principle 2 | Manual `Rect` arithmetic for placeholder centering in both tabs — use `Layout::vertical` with `Constraint::Min(0)` absorbers |
| L5 | code_quality_inspector (#11) | `line_count = 3u16` undocumented magic number in placeholder rendering |
| L6 | code_quality_inspector (#5) | Three of five tests for `set_profile_widget_builds` in `extensions/performance.rs` test only `Option<bool>.map(\|e\| e.to_string())` — std-library no-ops |
| L7 | code_quality_inspector (#6, #7) | `timeline.rs:113,115` — `ph` and `tid` default silently (`unwrap_or`); asymmetric vs `name`/`ts` which return errors. Document or warn |
| L8 | code_quality_inspector (#12) | `RebuildEventPayload` does not derive `Serialize`/`Deserialize`; inconsistent with sibling types |
| L9 | code_quality_inspector (#8) | `session_lifecycle.rs:177` possible comment typo (`/` instead of `//`). Verify — may be a misreading since the build passes |
| L10 | risks_tradeoffs_analyzer (R#7) | `Flutter.RebuiltWidgets` forwarder uses `msg_tx.send().await` — head-of-line blocking under backpressure. Consider `try_send` and drop |
| L11 | risks_tradeoffs_analyzer (R#10) | First-tick `last_poll_micros = 0` on seed-RPC failure → first batch retrieves entire VM lifetime |

## Documentation Freshness

- ARCHITECTURE.md and CONFIGURATION.md were updated in Task 07. Doc validation against schemas passed.
- KEYBINDINGS.md was updated in Task 06.
- The **wrong inspector-readiness key names** in CONFIGURATION.md (C3) are a doc-vs-code drift, not a freshness gap — fixing C3 is the action.

## Re-review Checklist

After addressing critical and high items, the following must pass:

- [ ] **C1** fixed and `test_exit_devtools_pauses_timeline` added
- [ ] **C2** fixed and `test_leaving_performance_clears_timeline_buffer` added
- [ ] **C3** fixed; ideally a doc-test that parses the example TOML block prevents regression
- [ ] **H1** mitigated (either panel-gate the forwarder or short-circuit in the handler)
- [ ] **H2** resolved (either implement the exclusion or rewrite the docstring)
- [ ] **H3** mitigated (emit failure message or surface to session log)
- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual acceptance walkthrough at `TASKS.md:131–143` passes end-to-end

## Files Reviewed (representative)

```
crates/fdemon-core/src/{rebuild_stats,timeline}.rs
crates/fdemon-daemon/src/vm_service/{timeline.rs, extensions/{mod,inspector,performance}.rs, client.rs}
crates/fdemon-app/src/{
  actions/{mod,performance,vm_service}.rs,
  config/types.rs,
  handler/{keys,update,session,session_lifecycle,mod}.rs,
  handler/devtools/{mod,performance/{mod,rebuild_stats,timeline,details}.rs},
  session/{performance,handle,mod}.rs,
  process.rs, message.rs
}
crates/fdemon-tui/src/widgets/devtools/{mod.rs, performance/details/{mod,rebuild_stats_tab,timeline_events_tab,frame_analysis_tab}.rs}
docs/{ARCHITECTURE,CONFIGURATION,KEYBINDINGS}.md
```
