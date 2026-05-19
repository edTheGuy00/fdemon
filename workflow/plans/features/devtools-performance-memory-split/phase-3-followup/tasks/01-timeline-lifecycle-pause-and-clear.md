# 01 — Timeline Lifecycle: Pause and Clear on Performance Leave

**Wave:** 1
**Depends On:** —
**Agent:** implementor
**Estimated Hours:** 1–2h
**Addresses:** C1, C2

## Context

Phase 3 added a 1 Hz `getVMTimeline` polling task and a ring buffer of up to 1000 timeline events. The reviewer identified two related gaps in lifecycle handling:

- **C1.** `handle_exit_devtools_mode` (`crates/fdemon-app/src/handler/devtools/mod.rs:355–393`) signals `perf_pause_tx`, `alloc_pause_tx`, and `network_pause_tx` when the user presses `Esc` to leave DevTools — but does **not** signal `timeline_pause_tx`. The 1 Hz `getVMTimeline` loop keeps firing for the entire session while the user views Logs.
- **C2.** The `timeline_events` buffer is never cleared on Performance-panel leave. Per PLAN.md §7.5 and `TASKS.md:114`, leaving the panel should drop accumulated events so the next entry shows fresh data. Currently up to ~100–200 KB per session is retained, and re-entry surfaces stale events until 1000 new ones overwrite them.

Both issues share the same call sites (`handle_exit_devtools_mode` for the Esc path, `handle_switch_panel` for the `i`/`m`/`n` paths). One task that touches one file.

## Acceptance Criteria

1. `handle_exit_devtools_mode` signals `timeline_pause_tx.send(true)` alongside the existing three pause sends.
2. On every code path that pauses the Performance panel (Esc-exit AND panel-switch away from Performance), the session's `timeline_events` buffer is cleared and `timeline_events_scroll_offset` is reset to 0.
3. New test `test_exit_devtools_pauses_timeline` mirrors the structure of `test_exit_devtools_pauses_network` (or equivalent existing perf-pause test).
4. New test `test_leaving_performance_clears_timeline_buffer` asserts:
   - Buffer is populated before pause.
   - Buffer is empty and scroll offset is 0 immediately after `handle_switch_panel` (Performance → another panel) AND after `handle_exit_devtools_mode` (DevTools → Logs).
5. Existing tests for perf/alloc/network pause continue to pass (no regressions in shared dispatch logic).
6. `cargo fmt --all -- --check && cargo check -p fdemon-app && cargo test -p fdemon-app && cargo clippy -p fdemon-app --all-targets -- -D warnings` all pass.

## Files Modified (Write)

- `crates/fdemon-app/src/handler/devtools/mod.rs` — add timeline pause + buffer clear to both `handle_exit_devtools_mode` and the Performance-leave branch of `handle_switch_panel`; add the two new tests in the existing `#[cfg(test)] mod tests` block.

## Files Read (Dependencies)

- `crates/fdemon-app/src/session/handle.rs` — verify `timeline_pause_tx` field name.
- `crates/fdemon-app/src/session/performance.rs` — verify `timeline_events` and `timeline_events_scroll_offset` field names.
- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — read-only cross-check that buffer is the same one written to by `handle_batch_received`.

## Approach Hints

- The existing pause logic is a clear pattern: `if let Some(tx) = &handle.session.<x>_pause_tx { let _ = tx.send(true); }`. Add a fourth analogous block.
- The buffer clear should happen in the **handler** (same place that signals pause), not in the spawned task. The handler has direct mutable access to `PerformanceState` via `session_manager.get_mut(session_id)`.
- For the panel-switch path, only clear when transitioning **away from** Performance — not when entering Performance from another panel.
- The test for `test_leaving_performance_clears_timeline_buffer` should: (a) construct an `AppState` with one session, (b) populate `session.performance.timeline_events` with a couple of fake events and set `timeline_events_scroll_offset = 5`, (c) invoke the panel-switch or exit handler, (d) assert both fields are at their default values.

## Out of Scope

- Pausing the `Flutter.RebuiltWidgets` event forwarder — that's H1 (T05).
- Clearing the `rebuild_stats_frames` buffer on panel-leave — not a Phase 3 acceptance criterion; if desired, that's a separate followup.
- Any change to the 1000-event buffer cap or default size.
