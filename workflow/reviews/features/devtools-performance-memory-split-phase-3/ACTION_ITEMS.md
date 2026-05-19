# Action Items — Phase 3 (Rebuild Stats + Timeline Events)

**Review Date:** 2026-05-19
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3 critical, 3 high

## Critical Issues (Must Fix)

### C1. `timeline_pause_tx` not signaled on `Esc` from DevTools
- **Sources:** logic_reasoning_checker, security_reviewer, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/devtools/mod.rs:355–393` (`handle_exit_devtools_mode`)
- **Problem:** The function pauses `perf_pause_tx`, `alloc_pause_tx`, and `network_pause_tx` but not `timeline_pause_tx`. The 1 Hz `getVMTimeline` poll loop keeps running for the entire session when the user exits DevTools via Esc.
- **Required Action:** Send `true` on `timeline_pause_tx` alongside the existing three pauses.
- **Acceptance:** New test `test_exit_devtools_pauses_timeline` mirrors the pattern of `test_exit_devtools_pauses_network`. Manual: tail fdemon log, enter DevTools → Performance, press Esc to return to Logs, observe `timeline poll paused` within one tick.
- **Violates:** `TASKS.md:110` success criterion.

### C2. Timeline event buffer not cleared when leaving the Performance panel
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/devtools/mod.rs` — `handle_switch_panel` (panel-switch path) and `handle_exit_devtools_mode` (Esc path)
- **Problem:** PLAN.md §7.5 mitigation and `TASKS.md:114` require the buffer to clear on leave. Currently up to 1000 events × ~100–200 B remain resident, and stale events show on re-entry.
- **Required Action:** Wherever timeline polling is paused, also call `handle.session.performance.timeline_events.clear()` and reset `timeline_events_scroll_offset = 0`.
- **Acceptance:** New test `test_leaving_performance_clears_timeline_buffer` asserts buffer is empty after pause-on-leave.

### C3. `docs/CONFIGURATION.md` documents the wrong TOML key names for `inspector_readiness_poll_*`
- **Source:** risks_tradeoffs_analyzer (already a Task 07 CONCERN)
- **File:** `docs/CONFIGURATION.md` lines ~345–348 and the surrounding example block
- **Problem:** Doc shows `readiness_poll_attempts`, `readiness_poll_interval_ms`, `readiness_poll_call_timeout_ms`. Actual fields require `inspector_` prefix. `DevToolsSettings` does not `deny_unknown_fields`, so wrong keys silently fall through to defaults — proven by existing test `test_old_readiness_poll_key_does_not_silently_override_default` (`crates/fdemon-app/src/config/types.rs:1985`).
- **Required Action:** Rename the three keys in CONFIGURATION.md (rows and example TOML block) to use `inspector_` prefix. Consider adding a doc-test that deserializes the example block to a `DevToolsSettings` and asserts non-default values land in the right fields.
- **Acceptance:** Manual: copy-paste the example block into `.fdemon/config.toml` and verify the new values take effect (e.g., set `inspector_readiness_poll_attempts = 99` and observe in tracing).

## High Priority (Should Fix)

### H1. Rebuild-stats handler runs at 60 fps regardless of panel visibility
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/actions/vm_service.rs:170–199`
- **Problem:** Once `profileWidgetBuilds` is ON, every frame allocates payloads, MPSC-sends, and churns the ring buffer even when the user is on Logs/Inspector/Memory/Network. CPU/heap drain proportional to app rebuild rate.
- **Suggested Action:** Either (a) panel-gate the forwarder — early-return when `active_panel != Performance`; or (b) short-circuit in `rebuild_stats::handle_event` to update only `rebuild_stats_totals` (cheap) when not on the RebuildStats tab. Option (a) is simpler; option (b) preserves totals while skipping snapshot work.

### H2. `classify_thread` docstring contradicts implementation
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-core/src/timeline.rs:14–23` (docstring) vs `:191–193` (code)
- **Problem:** Docstring promises an exclusion guard for `.flutter.test..ui`; code uses a bare `.contains(".ui")`. The test for the tester thread passes by coincidence.
- **Suggested Action:** Decide: (a) match upstream — add `&& !thread_name.contains(".flutter.test..ui")` then re-classify the tester thread by checking `.contains(".ui")` afterwards (mirrors upstream Dart fallback), or (b) keep current code and rewrite the docstring to accurately describe the simple containment match plus the deliberate "tester thread is Ui because its name contains `.ui`" rationale.

### H3. Silent RPC failures in `ToggleProfileWidgetBuilds` and `FetchWidgetLocationIdMap`
- **Sources:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/actions/mod.rs:~1036–1115`
- **Problem:** On RPC error, only `tracing::warn!`. No `RebuildStatsExtensionStateChanged` is emitted to roll back state or surface failure to the UI. Compounds M3 (hot-restart clobber).
- **Suggested Action:** Add a new `Message::RebuildStatsToggleFailed { reason: String }` (and/or reuse `RebuildStatsExtensionStateChanged` with the actual current state) so the handler can revert optimistic UI and append a session-log entry. Same pattern for the location-map fetch failure.

## Medium Priority

### M1. Tab cycle should skip hidden `RebuildStats` at state level (not just renderer)
- **File:** `crates/fdemon-app/src/handler/devtools/performance/details.rs:18–23`; `state.rs:198–204`
- **Action:** Make `handle_perf_cycle_details_tab` consult `rebuild_stats_enabled` (or `visible_tabs()`) and skip the hidden tab when computing next.

### M2. `auto_enable_rebuild_tracking` is dead config
- **File:** `crates/fdemon-app/src/config/types.rs:430–431`
- **Action:** Wire it on `VmServiceConnected`/`SessionStarted` to queue `ToggleProfileWidgetBuilds { enabled: true }` when true. Or delete the setting and remove from CONFIGURATION.md.

### M3. Hot-restart re-enable can clobber user toggle-OFF
- **Files:** `handler/update.rs:222–264`; `actions/mod.rs:1045–1051`
- **Action:** Fix via H3 (failure emits state-sync message), or record `rebuild_stats_target_enabled` separately so user intent survives RPC failure.

### M4. `R` is a silent no-op in DevTools-mode non-RebuildStats contexts
- **File:** `crates/fdemon-app/src/handler/keys.rs:544–555`
- **Action:** Relax the early-return so `R` outside the RebuildStats tab falls through to global `HotRestart`. Update the test at `:2167` to assert HotRestart fallback in DevTools mode.

### M5. `enable_frame_tracking` uses string literal instead of `ext::PROFILE_WIDGET_BUILDS`
- **File:** `crates/fdemon-daemon/src/vm_service/timeline.rs:147`
- **Action:** Replace `"ext.flutter.profileWidgetBuilds"` with `crate::vm_service::extensions::ext::PROFILE_WIDGET_BUILDS`.

### M6. `FetchWidgetLocationIdMap` parsing belongs in the daemon helper, not the action task
- **File:** `crates/fdemon-app/src/actions/mod.rs:~1085–1115`
- **Action:** Call `inspector::widget_location_id_map()` (already returns a `LocationMap`) and ship the result via Message. Remove the inline file-URI/merge loop from `actions/mod.rs`.

### M7. Missing integration test for `spawn_timeline_polling`
- **File:** `crates/fdemon-app/src/actions/performance.rs`
- **Action:** Extract a trait abstraction over `VmRequestHandle::request`/`call_extension` and add tests asserting pause-stops-RPCs, resume-restarts, shutdown-exits-within-100ms. This unblocks `spawn_performance_polling` testing too.

### M8. Watermark advancement off-by-one in timeline polling
- **File:** `crates/fdemon-app/src/actions/performance.rs:603`
- **Action:** Capture `now_micros` after `fetch_timeline_chunk` returns (or use `max(ts)` from the response) to avoid dropping events with `ts ∈ [now_pre_fetch, fetch_complete]`.

### M9. Duplicated text helpers (`truncate_with_ellipsis`, `pad_right`, `pad_left`)
- **Files:** `rebuild_stats_tab.rs:303–333`, `timeline_events_tab.rs:301–327`, `widgets/new_session_dialog/mod.rs:65`
- **Action:** Extract to a shared module (e.g., `widgets/text_helpers.rs` or under `widgets/devtools/performance/details/`).

## Minor (Consider Fixing)

- **L1** Name the `200` ms timeline-poll floor constant (`TIMELINE_POLL_MIN_MS`) with derivation comment in `actions/performance.rs:503`.
- **L2** Add `since_micros.min(i64::MAX as u64) as i64` guard in `vm_service/timeline.rs:221–223`.
- **L3** Downgrade `Flutter.RebuiltWidgets` parse-error from `warn!` to `debug!` (60fps log flood risk).
- **L4** Replace manual `Rect` arithmetic for placeholder centering in both new tabs with `Layout::vertical` + `Constraint::Min(0)` absorbers. CODE_STANDARDS Principle 2.
- **L5** Name the `line_count = 3u16` magic number in placeholder rendering.
- **L6** Replace the three `Option<bool>.map(|e| e.to_string())` tests in `extensions/performance.rs:81–125` with real round-trip tests (or delete — they exercise stdlib).
- **L7** Document or `tracing::debug!` the silent `unwrap_or` for `ph`/`tid` in `timeline.rs:113,115`.
- **L8** Derive `Serialize`/`Deserialize` on `RebuildEventPayload` for consistency with sibling types.
- **L9** Verify `session_lifecycle.rs:177` comment — code_quality_inspector flagged a possible `/` instead of `//`; build passes so it's either misread or compiler-tolerant. Confirm and clean up if real.
- **L10** Consider `try_send` for `RebuildStatsEventReceived` so a slow handler doesn't head-of-line-block the entire VM Service stream.
- **L11** On first-tick seed RPC failure, retry (or cap initial `extent` to ~2 s) rather than starting `last_poll_micros = 0`.

## Re-review Checklist

After addressing critical and high items, the following must pass:

- [ ] C1 fixed; new test `test_exit_devtools_pauses_timeline` passes
- [ ] C2 fixed; new test `test_leaving_performance_clears_timeline_buffer` passes
- [ ] C3 fixed; doc-test optional but recommended
- [ ] H1 mitigated (panel gate or handler short-circuit)
- [ ] H2 resolved (code or doc updated to match)
- [ ] H3 mitigated (failure message routed to UI/log)
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual acceptance walkthrough at `TASKS.md:131–143` passes end-to-end
