# Phase 3-Followup — Review-Driven Fixes

## Overview

This phase addresses all 26 findings from the Phase 3 review at [`../../../../reviews/features/devtools-performance-memory-split-phase-3/REVIEW.md`](../../../../reviews/features/devtools-performance-memory-split-phase-3/REVIEW.md) and [`ACTION_ITEMS.md`](../../../../reviews/features/devtools-performance-memory-split-phase-3/ACTION_ITEMS.md).

**Breakdown:** 3 critical, 3 high, 9 medium, 11 minor → 12 tasks across 5 waves.

The Phase 3 implementation itself (rebuild stats + timeline events) shipped fully functional with green CI; this followup tightens correctness, UX, and architecture loose ends surfaced by the multi-agent review.

## Finding → Task Mapping

### Critical (block release)

| ID | Finding | Task |
|----|---------|------|
| C1 | `timeline_pause_tx` not signaled on Esc-from-DevTools — 1 Hz poll runs indefinitely | [T01](tasks/01-timeline-lifecycle-pause-and-clear.md) |
| C2 | Timeline event buffer never cleared on Performance-leave — stale data + memory retention | [T01](tasks/01-timeline-lifecycle-pause-and-clear.md) |
| C3 | `docs/CONFIGURATION.md` documents wrong TOML key names (missing `inspector_` prefix) | [T02](tasks/02-fix-inspector-readiness-config-doc.md) |

### High

| ID | Finding | Task |
|----|---------|------|
| H1 | `Flutter.RebuiltWidgets` events parsed at 60 fps regardless of panel visibility | [T05](tasks/05-rebuild-forwarder-panel-gate-and-logging.md) |
| H2 | `classify_thread` docstring contradicts implementation (test passes by coincidence) | [T03](tasks/03-core-parser-hygiene.md) |
| H3 | Silent RPC failures in `ToggleProfileWidgetBuilds` / `FetchWidgetLocationIdMap` | [T04](tasks/04-action-error-surfacing-and-locationmap-cleanup.md) |

### Medium

| ID | Finding | Task |
|----|---------|------|
| M1 | Tab cycle doesn't skip hidden RebuildStats in state machine | [T08](tasks/08-ux-tab-cycle-skip-and-r-key-fallthrough.md) |
| M2 | `auto_enable_rebuild_tracking` config plumbed but never read | [T09](tasks/09-auto-enable-rebuild-wiring-and-typo.md) |
| M3 | Hot-restart re-enable can clobber user's toggle-OFF during restart window | [T04](tasks/04-action-error-surfacing-and-locationmap-cleanup.md) |
| M4 | `R` silent no-op in DevTools-mode non-RebuildStats contexts | [T08](tasks/08-ux-tab-cycle-skip-and-r-key-fallthrough.md) |
| M5 | `enable_frame_tracking` uses string literal instead of `ext::PROFILE_WIDGET_BUILDS` | [T07](tasks/07-daemon-vm-service-hygiene.md) |
| M6 | `FetchWidgetLocationIdMap` parsing inline in `actions/mod.rs` instead of inspector helper | [T04](tasks/04-action-error-surfacing-and-locationmap-cleanup.md) |
| M7 | Missing integration test for `spawn_timeline_polling` pause/resume/shutdown | [T11](tasks/11-mock-vmrequesthandle-for-polling-tests.md) |
| M8 | Watermark advancement off-by-one in `spawn_timeline_polling` | [T06](tasks/06-timeline-polling-improvements.md) |
| M9 | Duplicated `truncate_with_ellipsis`/`pad_*` helpers between two tab files | [T10](tasks/10-tui-text-helpers-and-centering.md) |

### Minor

| ID | Finding | Task |
|----|---------|------|
| L1 | Magic number `200` ms timeline-poll floor needs named constant | [T06](tasks/06-timeline-polling-improvements.md) |
| L2 | `u64 → i64` cast in `fetch_timeline_chunk` lacks ceiling guard | [T07](tasks/07-daemon-vm-service-hygiene.md) |
| L3 | Per-frame `Flutter.RebuiltWidgets` parse-error at `warn!` level — log flood risk | [T05](tasks/05-rebuild-forwarder-panel-gate-and-logging.md) |
| L4 | Manual `Rect` arithmetic for placeholder centering — CODE_STANDARDS Principle 2 violation | [T10](tasks/10-tui-text-helpers-and-centering.md) |
| L5 | `line_count = 3u16` magic number in placeholder rendering | [T10](tasks/10-tui-text-helpers-and-centering.md) |
| L6 | Three of five `set_profile_widget_builds` tests test only stdlib `Option.map` | [T07](tasks/07-daemon-vm-service-hygiene.md) |
| L7 | `timeline.rs` silent `unwrap_or` on `ph`/`tid` is asymmetric vs `name`/`ts` errors | [T03](tasks/03-core-parser-hygiene.md) |
| L8 | `RebuildEventPayload` missing `Serialize`/`Deserialize` derives | [T03](tasks/03-core-parser-hygiene.md) |
| L9 | Possible `/` instead of `//` comment typo in `session_lifecycle.rs:177` | [T09](tasks/09-auto-enable-rebuild-wiring-and-typo.md) |
| L10 | Forwarder uses `msg_tx.send().await` — head-of-line blocking risk under backpressure | [T05](tasks/05-rebuild-forwarder-panel-gate-and-logging.md) |
| L11 | First-tick seed-RPC failure → `last_poll_micros = 0` → first batch retrieves VM lifetime | [T06](tasks/06-timeline-polling-improvements.md) |

## Wave Strategy

```
Wave 1 (parallel × 3 worktrees)
  ├─ T01 timeline-lifecycle-pause-and-clear      (C1+C2)        handler/devtools/mod.rs
  ├─ T02 fix-inspector-readiness-config-doc      (C3)           docs/CONFIGURATION.md
  └─ T03 core-parser-hygiene                     (H2+L7+L8)     fdemon-core/{timeline,rebuild_stats}.rs

Wave 2 (parallel × 4 worktrees)
  ├─ T04 action-error-surfacing-and-locationmap  (H3+M3+M6)     actions/mod, message, handler/{update, rebuild_stats}, extensions/inspector
  ├─ T05 rebuild-forwarder-panel-gate            (H1+L3+L10)    actions/vm_service.rs
  ├─ T06 timeline-polling-improvements           (M8+L1+L11)    actions/performance.rs
  └─ T07 daemon-vm-service-hygiene               (M5+L2+L6)     vm_service/timeline.rs, extensions/performance.rs

Wave 3 (parallel × 3 worktrees — T09 depends on T04)
  ├─ T08 ux-tab-cycle-skip-and-r-key-fallthrough (M1+M4)        state, details, keys
  ├─ T09 auto-enable-rebuild-wiring-and-typo     (M2+L9)        session_lifecycle, update, config/types
  └─ T10 tui-text-helpers-and-centering          (M9+L4+L5)     new details/text_helpers.rs + both tab files

Wave 4 (sequential after Wave 2 T06)
  └─ T11 mock-vmrequesthandle-for-polling-tests  (M7)           actions/performance.rs + new test infra

Wave 5 (sequential after all implementation)
  └─ T12 update-arch-and-review-focus-docs       (doc updates)  docs/ARCHITECTURE.md, docs/REVIEW_FOCUS.md  [doc_maintainer]
```

## Design Decisions

These three decisions were approved at the planning stage and are baked into the task specs.

### 1. Action-layer failure feedback (T04)

When `ToggleProfileWidgetBuilds` or `FetchWidgetLocationIdMap` RPC fails:

- **Emit `RebuildStatsExtensionStateChanged { enabled: <actual_current> }`** to roll back optimistic UI state.
- **Also emit a new `RebuildStatsToggleFailed { reason: String }` message** that the handler appends to the session log buffer so the user has visible feedback.

This combined approach (option C from planning) addresses both H3 (silent failure) and M3 (hot-restart clobber): the optimistic-rollback message keeps `rebuild_stats_enabled` in sync with the real extension state, so the `SessionRestartCompleted` handler in T04 will read the correct value.

### 2. `R`-key fallthrough in DevTools mode (T08)

In `handler/keys.rs`, relax the early-return so `R` outside the `(Performance + Details + RebuildStats)` context **falls through to the global `Char('R')` arm** and emits `Message::HotRestart`. Preserves muscle memory; matches reviewer recommendation.

The regression test that currently asserts `!ToggleRebuildStats` in non-RebuildStats DevTools contexts must be updated to assert `HotRestart` instead. The test name `test_capital_r_on_frame_analysis_tab_triggers_hot_restart` will then accurately describe what it tests.

### 3. `auto_enable_rebuild_tracking` is wired (T09)

The config field stays. When `auto_enable_rebuild_tracking == true` and a session transitions to a state where rebuild tracking is meaningful (`VmServiceConnected` — the same trigger that starts timeline monitoring), queue `UpdateAction::ToggleProfileWidgetBuilds { enabled: true }`. Idempotent — if already enabled (e.g., from hot-restart re-enable), the toggle is a no-op on the Dart side.

The doc-comment in `config/types.rs:430` already promises this behavior; deletion would require user-facing changelog and reverse-removal from CONFIGURATION.md, so wiring is the cheaper path.

## Out of Scope

- **No new keybindings.** All Phase 3 `f`/`R` semantics preserved (M4 just restores fallthrough for non-RebuildStats contexts).
- **No new dependencies.** T11's mock infrastructure is internal Rust traits only.
- **No layout-threshold value changes** other than L5 (naming the existing `3u16` constant).
- **No protobuf / Perfetto migration.** Phase 3's `getVMTimeline` JSON choice stands per PLAN.md §7.5.
- **No Dart-side changes.** All fixes are fdemon-side.

## Total Estimated Effort

| Wave | Tasks | Hours |
|------|-------|-------|
| Wave 1 | T01, T02, T03 | 3–5h |
| Wave 2 | T04, T05, T06, T07 | 6–9h |
| Wave 3 | T08, T09, T10 | 4–6h |
| Wave 4 | T11 | 3–5h |
| Wave 5 | T12 | 2–3h |
| **Total** | **12 tasks** | **18–28 hours** |

## References

- Review document: `workflow/reviews/features/devtools-performance-memory-split-phase-3/REVIEW.md`
- Action items: `workflow/reviews/features/devtools-performance-memory-split-phase-3/ACTION_ITEMS.md`
- Phase 3 implementation: `workflow/plans/features/devtools-performance-memory-split/phase-3/TASKS.md`
- Original feature plan: `workflow/plans/features/devtools-performance-memory-split/PLAN.md`
