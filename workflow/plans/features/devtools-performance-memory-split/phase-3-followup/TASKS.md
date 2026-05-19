# Phase 3-Followup — Review-Driven Fixes — Task Index

## Overview

Twelve tasks address the 3 Critical + 3 High + 9 Medium + 11 Minor findings from the Phase 3 review ([`../../../../reviews/features/devtools-performance-memory-split-phase-3/REVIEW.md`](../../../../reviews/features/devtools-performance-memory-split-phase-3/REVIEW.md)). See [`PLAN.md`](PLAN.md) for the rationale, finding↔task mapping, and design decisions.

- **Wave 1 (parallel × 3):** Critical lifecycle fix (T01), docs fix (T02), core parser hygiene (T03). Disjoint files.
- **Wave 2 (parallel × 4):** Four action-layer concerns split by file: error surfacing/locationmap cleanup (T04), forwarder gate (T05), timeline polling improvements (T06), daemon hygiene (T07). All disjoint within the wave.
- **Wave 3 (parallel × 3, T09 depends on T04):** Tab cycle + R-key fallthrough (T08), auto-enable wiring + typo (T09), TUI text helpers (T10).
- **Wave 4 (sequential after T06):** Mock VmRequestHandle for polling integration tests (T11).
- **Wave 5 (sequential after all impl):** Doc updates via `doc_maintainer` (T12).

**Total Tasks:** 12
**Estimated Hours:** 18–28 hours

## Task Dependency Graph

```
Wave 1 (parallel)
┌──────────────────────────────────────┐ ┌──────────────────────────────────┐ ┌──────────────────────────────────┐
│ 01 timeline-lifecycle-pause-and-     │ │ 02 fix-inspector-readiness-      │ │ 03 core-parser-hygiene           │
│    clear (C1+C2)                     │ │    config-doc (C3)               │ │    (H2+L7+L8)                    │
│   handler/devtools/mod.rs            │ │   docs/CONFIGURATION.md          │ │   fdemon-core/{timeline,         │
│                                      │ │                                  │ │   rebuild_stats}.rs              │
└──────────────────────────────────────┘ └──────────────────────────────────┘ └────────────────┬─────────────────┘
                                                                                               │
Wave 2 (parallel — all depend on T03 for parser changes; T04 depends on T03 for serde derives)
┌──────────────────────────────────────┐ ┌──────────────────────────────────┐ ┌──────────────────────────────────┐ ┌──────────────────────────────────┐
│ 04 action-error-surfacing-and-       │ │ 05 rebuild-forwarder-panel-gate- │ │ 06 timeline-polling-improvements │ │ 07 daemon-vm-service-hygiene     │
│    locationmap-cleanup               │ │    and-logging (H1+L3+L10)       │ │    (M8+L1+L11)                   │ │    (M5+L2+L6)                    │
│    (H3+M3+M6)                        │ │   actions/vm_service.rs          │ │   actions/performance.rs         │ │   vm_service/timeline.rs +       │
│   actions/mod, message, handler/     │ │                                  │ │                                  │ │   extensions/performance.rs      │
│   {update, devtools/performance/     │ │                                  │ │                                  │ │                                  │
│   rebuild_stats}, extensions/        │ │                                  │ │                                  │ │                                  │
│   inspector                          │ │                                  │ │                                  │ │                                  │
└────────────────┬─────────────────────┘ └──────────────────────────────────┘ └────────────────┬─────────────────┘ └──────────────────────────────────┘
                 │                                                                               │
Wave 3 (parallel — T09 depends on T04 for handler/update.rs)
┌──────────────────────────────────────┐ ┌──────────────────────────────────┐ ┌──────────────────────────────────┐
│ 08 ux-tab-cycle-skip-and-r-key-      │ │ 09 auto-enable-rebuild-wiring-   │ │ 10 tui-text-helpers-and-         │
│    fallthrough (M1+M4)               │ │    and-typo (M2+L9)              │ │    centering (M9+L4+L5)          │
│   state.rs, details.rs, keys.rs      │ │   session_lifecycle, update,     │ │   new details/text_helpers.rs +  │
│                                      │ │   config/types                   │ │   both tab files                 │
└──────────────────────────────────────┘ └──────────────────────────────────┘ └──────────────────────────────────┘
                                                                                               │
Wave 4 (sequential after T06 — both write actions/performance.rs)
┌──────────────────────────────────────────────────────────────────────────────────────────────┘
│ 11 mock-vmrequesthandle-for-polling-tests (M7)
│   New trait abstraction in fdemon-daemon/vm_service, refactor of spawn_timeline_polling tests
└──────────────────────────────────────────────────────────────────────────────────────────────┐
                                                                                               │
Wave 5 (sequential after all impl)                                                             ▼
┌──────────────────────────────────────────────────────────────────────────────────────────────┐
│ 12 update-arch-and-review-focus-docs                                                         │
│   docs/ARCHITECTURE.md + docs/REVIEW_FOCUS.md   [doc_maintainer]                             │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Wave |
|---|------|--------|------------|------------|-------|------|
| 01 | [timeline-lifecycle-pause-and-clear](tasks/01-timeline-lifecycle-pause-and-clear.md) | Not Started | — | 1–2h | implementor | 1 |
| 02 | [fix-inspector-readiness-config-doc](tasks/02-fix-inspector-readiness-config-doc.md) | Not Started | — | 0.5h | implementor | 1 |
| 03 | [core-parser-hygiene](tasks/03-core-parser-hygiene.md) | Not Started | — | 1.5–2.5h | implementor | 1 |
| 04 | [action-error-surfacing-and-locationmap-cleanup](tasks/04-action-error-surfacing-and-locationmap-cleanup.md) | Not Started | 03 | 2–3h | implementor | 2 |
| 05 | [rebuild-forwarder-panel-gate-and-logging](tasks/05-rebuild-forwarder-panel-gate-and-logging.md) | Not Started | — | 1.5–2h | implementor | 2 |
| 06 | [timeline-polling-improvements](tasks/06-timeline-polling-improvements.md) | Not Started | — | 1.5–2h | implementor | 2 |
| 07 | [daemon-vm-service-hygiene](tasks/07-daemon-vm-service-hygiene.md) | Not Started | — | 1–1.5h | implementor | 2 |
| 08 | [ux-tab-cycle-skip-and-r-key-fallthrough](tasks/08-ux-tab-cycle-skip-and-r-key-fallthrough.md) | Not Started | — | 1.5–2h | implementor | 3 |
| 09 | [auto-enable-rebuild-wiring-and-typo](tasks/09-auto-enable-rebuild-wiring-and-typo.md) | Not Started | 04 | 1.5–2h | implementor | 3 |
| 10 | [tui-text-helpers-and-centering](tasks/10-tui-text-helpers-and-centering.md) | Not Started | — | 1–2h | implementor | 3 |
| 11 | [mock-vmrequesthandle-for-polling-tests](tasks/11-mock-vmrequesthandle-for-polling-tests.md) | Not Started | 06 | 3–5h | implementor | 4 |
| 12 | [update-arch-and-review-focus-docs](tasks/12-update-arch-and-review-focus-docs.md) | Not Started | 01,04,05,08,09 | 2–3h | doc_maintainer | 5 |

## File Overlap Analysis

> The orchestrator uses this section to decide isolation strategy per wave. Read-only overlap is fine — only **write-file** overlap forces sequential execution.

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| **01** timeline-lifecycle-pause-and-clear | `crates/fdemon-app/src/handler/devtools/mod.rs` (add `timeline_pause_tx.send(true)` + `timeline_events.clear()` + `timeline_events_scroll_offset = 0` in `handle_exit_devtools_mode` AND `handle_switch_panel` when leaving Performance; extend existing pause tests with timeline assertions; new `test_leaving_performance_clears_timeline_buffer`) | `crates/fdemon-app/src/session/{handle,performance}.rs` (verify field names), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (cross-check buffer write paths) |
| **02** fix-inspector-readiness-config-doc | `docs/CONFIGURATION.md` (rename rows + example TOML keys for the three inspector-readiness keys to use `inspector_` prefix; add intra-doc test note recommending users verify keys via tracing) | `crates/fdemon-app/src/config/types.rs:402,410,417` (verify actual serde field names), `crates/fdemon-app/src/config/types.rs:1985` (existing regression test that confirms the old-name silent-default behavior) |
| **03** core-parser-hygiene | `crates/fdemon-core/src/timeline.rs` (H2: reconcile docstring vs `classify_thread` code — chosen path: rewrite docstring to accurately describe simple `.contains(".ui")` behavior + add inline rationale for tester thread; L7: add `tracing::debug!` calls in `parse_vm_timeline` when `ph` or `tid` default; add doc comment explaining asymmetry with `name`/`ts`), `crates/fdemon-core/src/rebuild_stats.rs` (L8: add `Serialize, Deserialize` derives on `RebuildEventPayload`; verify `serde_json::Value` round-trip in a new test) | `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (read-only — confirm parser changes don't break existing consumers) |
| **04** action-error-surfacing-and-locationmap-cleanup | `crates/fdemon-app/src/message.rs` (NEW variant `RebuildStatsToggleFailed { session_id: SessionId, reason: String }`), `crates/fdemon-app/src/actions/mod.rs` (H3: `ToggleProfileWidgetBuilds` arm on Err sends BOTH `RebuildStatsExtensionStateChanged { enabled: <opposite of attempted> }` AND `RebuildStatsToggleFailed`; `FetchWidgetLocationIdMap` arm on Err sends `RebuildStatsToggleFailed`; M6: delete inline file-URI/merge loop, call `inspector::widget_location_id_map()` directly and forward the returned `LocationMap`), `crates/fdemon-app/src/handler/devtools/performance/rebuild_stats.rs` (new `handle_toggle_failed` function appends a log entry to the session's log buffer with the reason), `crates/fdemon-app/src/handler/update.rs` (dispatch the new `RebuildStatsToggleFailed` arm), `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` (M6: confirm `widget_location_id_map` already returns the right shape; add no-op or extend if a final-merge convenience method is needed) | T03 outputs (the `RebuildEventPayload` Serialize derive isn't a hard dep here but T03 should land first), `crates/fdemon-app/src/state.rs` (Message dispatch table is exhaustive) |
| **05** rebuild-forwarder-panel-gate-and-logging | `crates/fdemon-app/src/actions/vm_service.rs` (H1: in the `Flutter.RebuiltWidgets` branch, look up the session's active DevTools panel via `state.session_manager.get(session_id)?.active_devtools_panel` — if not `Performance`, early-return without parsing or sending; L3: downgrade `tracing::warn!` for parse-error to `tracing::debug!` to avoid log flood at 60 fps; L10: change `msg_tx.send(...).await` to `msg_tx.try_send(...)` with a `tracing::debug!` on `TrySendError::Full` so a slow handler doesn't head-of-line-block the VM stream) | `crates/fdemon-app/src/session/devtools.rs` or wherever active-panel is tracked (verify field name) |
| **06** timeline-polling-improvements | `crates/fdemon-app/src/actions/performance.rs` (M8: in `spawn_timeline_polling`, capture `now_micros` AFTER `fetch_timeline_chunk` completes — use either a second `get_vm_timeline_micros` call post-fetch OR the max `ts` from the returned events; L1: introduce `const TIMELINE_POLL_MIN_MS: u64 = 200;` with a derivation comment; replace the literal `200`; L11: on first-tick seed `get_vm_timeline_micros` failure, retry once with a short backoff rather than starting `last_poll_micros = 0`) | `crates/fdemon-daemon/src/vm_service/timeline.rs` (read-only — verify `fetch_timeline_chunk` signature unchanged) |
| **07** daemon-vm-service-hygiene | `crates/fdemon-daemon/src/vm_service/timeline.rs` (M5: replace string literal `"ext.flutter.profileWidgetBuilds"` at line ~147 with `crate::vm_service::extensions::ext::PROFILE_WIDGET_BUILDS`; L2: add `since_micros.min(i64::MAX as u64) as i64` guard in `fetch_timeline_chunk`'s u64→i64 casts; remove the "in practice safe" caveat from the doc comment), `crates/fdemon-daemon/src/vm_service/extensions/performance.rs` (L6: replace the three no-op `Option<bool>.map(\|e\| e.to_string())` tests with either real round-trip tests using a mock RPC OR delete them and add a justification comment that round-tripping is covered by `toggle_bool_extension`'s tests) | `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` (read-only — `ext::PROFILE_WIDGET_BUILDS` constant) |
| **08** ux-tab-cycle-skip-and-r-key-fallthrough | `crates/fdemon-app/src/state.rs` (M1: extend `PerfDetailsTab::next` to accept a `rebuild_stats_enabled: bool` parameter and skip `RebuildStats` when false; OR add a `next_visible(rebuild_stats_enabled)` method alongside the existing `next`), `crates/fdemon-app/src/handler/devtools/performance/details.rs` (M1: `handle_perf_cycle_details_tab` reads `rebuild_stats_enabled` from the session's `PerformanceState` and passes it to `next_visible`), `crates/fdemon-app/src/handler/keys.rs` (M4: relax the early-return for `Char('R')` — only return `Some(ToggleRebuildStats)` when actually on RebuildStats; otherwise fall through to the main match where `Char('R')` maps to `HotRestart`; rename test `test_capital_r_on_frame_analysis_tab_triggers_hot_restart` body to actually assert `Some(Message::HotRestart)`; add similar regression tests for Inspector/Memory/Network panels asserting HotRestart fallthrough) | `crates/fdemon-app/src/session/performance.rs` (read-only — `rebuild_stats_enabled` field) |
| **09** auto-enable-rebuild-wiring-and-typo | `crates/fdemon-app/src/handler/update.rs` (M2: in the `VmServiceConnected` arm, check `settings.devtools.auto_enable_rebuild_tracking`; if true AND `rebuild_stats_enabled == false`, return `UpdateAction::ToggleProfileWidgetBuilds { enabled: true, vm_handle: None }`), `crates/fdemon-app/src/handler/session_lifecycle.rs` (L9: verify line 177 — if it's actually `/` instead of `//`, fix it; add a regression test that `auto_enable_rebuild_tracking = true` causes `ToggleProfileWidgetBuilds` to be queued on first VM connect; cross-check the existing hot-restart re-enable still wins on session-restart paths), `crates/fdemon-app/src/config/types.rs` (clarify doc-comment on `auto_enable_rebuild_tracking` to state that wiring fires on `VmServiceConnected`) | T04 outputs (handler/update.rs surface — both tasks edit this file; T09 follows T04), `crates/fdemon-app/src/handler/update.rs:222-264` (existing `SessionRestartCompleted` arm — the new `VmServiceConnected` wiring must be ordered so hot-restart re-enable still wins) |
| **10** tui-text-helpers-and-centering | `crates/fdemon-tui/src/widgets/devtools/performance/details/text_helpers.rs` (NEW — `pub(super) fn truncate_with_ellipsis`, `pad_right`, `pad_left` extracted from the two tab files; module-level `//!` doc; unit tests for each helper inc. unicode edge cases), `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` (declare `pub(super) mod text_helpers;`), `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` (delete local helpers; import from `text_helpers`; L4: replace manual `Rect` arithmetic in `render_disabled_placeholder` and `render_empty_placeholder` with `Layout::vertical` + `Constraint::Min(0)` absorbers; L5: name the `line_count = 3u16` magic number as `const PLACEHOLDER_LINE_COUNT: u16 = 3;` with doc comment), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` (same as above) | `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs:65` (read-only — the third near-identical implementation; do NOT consolidate with this one in scope — that's a separate refactor) |
| **11** mock-vmrequesthandle-for-polling-tests | `crates/fdemon-daemon/src/vm_service/client.rs` (extract `VmRequestHandle::request` and `VmRequestHandle::call_extension` into a `pub(crate) trait VmRequestApi` so test code can mock it; gate the trait behind `#[cfg(any(test, feature = "test-util"))]` or expose internally), `crates/fdemon-app/src/actions/performance.rs` (M7: rewrite `spawn_timeline_polling` to accept `impl VmRequestApi` instead of concrete `VmRequestHandle`; add three integration tests `test_timeline_pause_stops_rpcs`, `test_timeline_resume_restarts`, `test_timeline_shutdown_exits_within_100ms`), `crates/fdemon-app/src/actions/performance.rs` tests module (mock impl of `VmRequestApi` returning canned responses) | T06 outputs (both tasks write `actions/performance.rs`; T11 follows T06) |
| **12** update-arch-and-review-focus-docs | `docs/ARCHITECTURE.md` ("DevTools Subsystem" / "Performance Panel Interactivity": document the new `RebuildStatsToggleFailed` message + failure-feedback flow; the panel-gate optimization in `forward_vm_events`; the `auto_enable_rebuild_tracking` wiring path on `VmServiceConnected`; the `VmRequestApi` trait abstraction and test pattern; the M6 architectural cleanup that moves location-map parsing entirely to the daemon helper), `docs/REVIEW_FOCUS.md` (add entry for the new `text_helpers` module's `pub(super)` boundary; cross-reference the panel-gate as an approved early-return optimization in the forwarder; if T08 ends up adding a new TEA exception via `next_visible(...)`, document it) | T01–T11 completion summaries (all impl tasks must have landed) |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 01 + 02 | None | 1 | **Parallel (worktree)** — T01 in handler/devtools/mod.rs; T02 in docs/CONFIGURATION.md. |
| 01 + 03 | None | 1 | **Parallel (worktree)** — T01 in fdemon-app; T03 in fdemon-core. |
| 02 + 03 | None | 1 | **Parallel (worktree)** — docs vs core. |
| 03 + 04 | None | — | **Sequential by dependency** — T04 consumes T03's serde derive (L8) on `RebuildEventPayload` if any new code paths serialize it; safer to run T03 first. |
| 04 + 05 | None | 2 | **Parallel (worktree)** — T04 in actions/mod.rs + message.rs + handler/{update, devtools/performance/rebuild_stats}.rs + extensions/inspector.rs; T05 in actions/vm_service.rs. Disjoint. |
| 04 + 06 | None | 2 | **Parallel (worktree)** — actions/mod.rs ≠ actions/performance.rs. |
| 04 + 07 | None | 2 | **Parallel (worktree)** — different crates (app vs daemon) and different files. |
| 05 + 06 | None | 2 | **Parallel (worktree)** — actions/vm_service.rs ≠ actions/performance.rs. |
| 05 + 07 | None | 2 | **Parallel (worktree)** — different crates. |
| 06 + 07 | None | 2 | **Parallel (worktree)** — different crates. |
| 04 + 09 | `crates/fdemon-app/src/handler/update.rs` | — | **Sequential** — T04 adds `RebuildStatsToggleFailed` arm; T09 adds auto-enable wiring on `VmServiceConnected` arm. Line-disjoint but same file; orchestrator serializes. |
| 08 + 09 | None | 3 | **Parallel (worktree)** — T08 in state.rs + details.rs + keys.rs; T09 in session_lifecycle.rs + update.rs + config/types.rs. Disjoint. |
| 08 + 10 | None | 3 | **Parallel (worktree)** — T08 in handler/keys.rs; T10 in TUI widget tabs. |
| 09 + 10 | None | 3 | **Parallel (worktree)** — handler vs TUI. |
| 06 + 11 | `crates/fdemon-app/src/actions/performance.rs` | — | **Sequential** — T11 wraps T06's `spawn_timeline_polling` in a trait abstraction; T11 must run after T06 to layer on top of T06's watermark/constant changes. |
| 11 + 12 | None | — | T12 is docs-only. |

## Success Criteria

Phase 3-followup is complete when:

- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **C1 verified:** From DevTools → Performance, press Esc to return to Logs — tail fdemon log file: `timeline poll paused` appears within one tick. New test `test_exit_devtools_pauses_timeline` mirrors the existing `test_exit_devtools_pauses_network`.
- [ ] **C2 verified:** From Performance, switch to any other panel (`i`/`m`/`n`) or press Esc — `timeline_events` buffer is empty when re-entering. New test `test_leaving_performance_clears_timeline_buffer`.
- [ ] **C3 verified:** Copy-paste the `[devtools]` example block from CONFIGURATION.md into `.fdemon/config.toml` with non-default values for the three inspector-readiness keys — values take effect at runtime (visible in tracing).
- [ ] **H1 verified:** Toggle `R` ON in Performance, switch to Logs, observe per-frame `Flutter.RebuiltWidgets` events are NOT parsed (tracing-level evidence: no `RebuildStatsEventReceived` log entries while away from Performance). Switching back resumes parsing.
- [ ] **H2 verified:** `classify_thread` docstring in `fdemon-core/timeline.rs` accurately describes the implementation. No contradictions.
- [ ] **H3 verified:** Press `R` against a profile-mode app (where extension is unavailable) — session log buffer shows a "Rebuild tracking toggle failed: …" entry. The Rebuild Stats tab does NOT appear (toggle state is correctly reverted).
- [ ] **M1 verified:** With rebuild tracking OFF, pressing `]` from FrameAnalysis goes directly to TimelineEvents (no apparent dead-press). State machine cycles only visible tabs.
- [ ] **M2 verified:** Setting `auto_enable_rebuild_tracking = true` in `.fdemon/config.toml` causes the Rebuild Stats tab to appear automatically within ~1s of a session connecting.
- [ ] **M3 verified:** Press `R` to disable rebuild tracking during a hot restart cycle — user's OFF intent is preserved (tab does NOT re-appear on `SessionRestartCompleted`).
- [ ] **M4 verified:** Pressing `R` in DevTools mode but outside the RebuildStats tab (e.g., Inspector, Memory, Performance/FrameChart) triggers `HotRestart`. New regression tests assert HotRestart fallthrough in each non-RebuildStats context.
- [ ] **M5 verified:** `cargo clippy` continues to pass; grep for `"ext.flutter.profileWidgetBuilds"` in `fdemon-daemon` returns only the constant definition.
- [ ] **M6 verified:** `actions/mod.rs::FetchWidgetLocationIdMap` arm has no inline file-URI loop; it calls `inspector::widget_location_id_map()` and forwards the result.
- [ ] **M7 verified:** Three new integration tests pass: `test_timeline_pause_stops_rpcs`, `test_timeline_resume_restarts`, `test_timeline_shutdown_exits_within_100ms`. Tests use the new `VmRequestApi` trait.
- [ ] **M8 verified:** Under simulated slow `fetch_timeline_chunk` (200ms+), no timeline events with `ts ∈ [start_of_fetch, end_of_fetch]` are dropped.
- [ ] **M9 verified:** `truncate_with_ellipsis`/`pad_right`/`pad_left` are defined exactly once in `details/text_helpers.rs`; both tab files import from there.
- [ ] **L1–L11 verified** per their per-task acceptance criteria (see individual task files).
- [ ] **doc updates verified:** `docs/ARCHITECTURE.md` Performance Panel section documents the new failure-feedback flow, panel-gate, auto-enable wiring, M6 cleanup. `docs/REVIEW_FOCUS.md` reflects new patterns. No content-boundary violations.

## Phase Acceptance Test Plan

After all 12 tasks merge, run the manual smoke sequence:

1. `cargo run -- ~/Dev/some-flutter-app` in a 200×30 iTerm split. Wait for attach.
2. **C2/H1 check:** `d` → DevTools, `p` → Performance, `]` to Details, press `R` to enable rebuild tracking. Within ~1s, table populates. Press `n` to switch to Network. Tail fdemon log: confirm no `RebuildStatsEventReceived` entries. Switch back to Performance — buffer reflects only events from the new active window (not stale entries from before the leave).
3. **C1 check:** From Performance, press Esc to return to Logs. Tail log: `timeline poll paused` appears within one tick.
4. **C3 check:** Add `[devtools]` block to `.fdemon/config.toml` with `inspector_readiness_poll_attempts = 99` (and similar for the other two `inspector_*` keys). Restart fdemon. Tail log on session start: confirm the new attempt count is in effect.
5. **H2 check:** `cargo doc -p fdemon-core --open` — verify the `classify_thread` docstring under `timeline` module accurately describes the implementation (no contradicting "exclusion guard" language).
6. **H3 check:** Switch a Flutter app to release/profile mode where `profileWidgetBuilds` is unavailable. Open Performance → press `R`. Verify a "Rebuild tracking toggle failed" log entry appears in the session's Logs view. Tab does not appear.
7. **M1 check:** With rebuild tracking OFF, press `]` repeatedly from Performance/Details — confirm it cycles `FrameAnalysis → TimelineEvents → FrameAnalysis` (skipping RebuildStats).
8. **M2 check:** Set `auto_enable_rebuild_tracking = true`. Restart fdemon, attach app. Rebuild Stats tab appears within ~1s of session connect without manual `R`.
9. **M3 check:** Toggle rebuild tracking ON. Trigger hot restart (`R` from Logs). Within the restart window, also press `R` in DevTools to disable. After `SessionRestartCompleted`, verify tab does NOT re-appear (user's last-intent wins).
10. **M4 check:** From DevTools, navigate to Inspector / Memory / Network / Performance-with-FrameChart-focused — pressing `R` triggers hot restart in each. (Pressing `R` while focused on the RebuildStats Details tab still toggles tracking — Phase 3 behavior preserved.)
11. **M9 check:** `cargo check --workspace` — confirm both tabs compile after the helper extraction. Visually inspect the disabled and empty placeholders in both tabs at small terminal sizes (e.g., 80×20) — text remains centered, no overflow.

## Notes

- **C1 and C2 are the same file (`handler/devtools/mod.rs`) and should be one task (T01)** — they share the same panel-leave hooks, and bundling reduces test duplication.
- **H1 implementation choice:** option (a) panel-gate the forwarder (in `actions/vm_service.rs`) was chosen over option (b) short-circuit in `handle_event` because (a) eliminates parsing cost entirely, while (b) only skips snapshot work. (a) also keeps file ownership clean — the forwarder owns "what to dispatch", the handler owns "what to do with it".
- **T04's `RebuildStatsToggleFailed` message** carries a `String` reason rather than a typed error enum, matching the existing pattern for user-facing log entries in the session's log buffer.
- **T08's `R`-key relaxation:** the existing test `test_capital_r_on_frame_analysis_tab_triggers_hot_restart` has a misleading name (it currently only asserts `!ToggleRebuildStats`). T08 makes that name accurate by changing the assertion to `Some(Message::HotRestart)`.
- **T11 introduces `VmRequestApi`** as the first trait-abstraction over `VmRequestHandle`. This unlocks integration testing for `spawn_timeline_polling`, `spawn_performance_polling`, `spawn_allocation_polling`, and `spawn_network_polling` — though only the timeline one is in this phase's scope. The other three are intentionally left to follow-up work to keep T11 focused.
- **T12 is intentionally NOT consolidated with the impl tasks** — the doc_maintainer's content boundaries differ from the implementor's, and the orchestrator routes them differently.
- **No new keyboard shortcuts.** All Phase 3 `f`/`R` semantics preserved.
- **No layout-threshold value changes** other than naming the existing `3u16` as `PLACEHOLDER_LINE_COUNT` (T10).
