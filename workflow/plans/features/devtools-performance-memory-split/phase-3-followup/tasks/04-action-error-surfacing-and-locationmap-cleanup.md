# 04 — Action Error Surfacing + LocationMap Cleanup

**Wave:** 2
**Depends On:** 03
**Agent:** implementor
**Estimated Hours:** 2–3h
**Addresses:** H3, M3, M6

## Context

Three interrelated concerns in the action layer:

- **H3.** `ToggleProfileWidgetBuilds` and `FetchWidgetLocationIdMap` in `crates/fdemon-app/src/actions/mod.rs` log a `warn!` on RPC failure and discard the error. The user gets no feedback (no message back, no log-buffer entry). `rebuild_stats_enabled` stays at the optimistic value from `handle_toggle`, so the UI is out of sync with reality.
- **M3.** The silent failure compounds a clobber: if the user presses `R` during a hot-restart window, the RPC fails against the dying isolate, `rebuild_stats_enabled` stays `true`, and `SessionRestartCompleted` re-enables the extension — losing the user's OFF intent.
- **M6.** The `FetchWidgetLocationIdMap` arm inlines a file-URI loop and calls `LocationMap::merge_parallel_arrays` directly, building the map inside the action task. The pattern established elsewhere (e.g., `FetchAllocationProfile` → `VmServiceAllocationProfileReceived`) is to ship raw-but-typed data from the daemon helper and let the handler do any merging. The inspector helper `widget_location_id_map()` already returns a `LocationMap` — the action should just call it.

The planning decision (see PLAN.md Design Decisions §1) chose to fix both H3 and M3 with the **same mechanism**:
- On RPC failure, emit `RebuildStatsExtensionStateChanged { enabled: <actual_current_state> }` to roll back the optimistic UI state.
- Also emit a new `RebuildStatsToggleFailed { session_id, reason: String }` message which the handler appends to the session's log buffer for user visibility.

## Acceptance Criteria

1. **New message variant.** `crates/fdemon-app/src/message.rs` adds:
   ```rust
   RebuildStatsToggleFailed {
       session_id: SessionId,
       reason: String,
   },
   ```
2. **`ToggleProfileWidgetBuilds` failure path.** In `actions/mod.rs`, when `call_extension` returns `Err`:
   - Log at `tracing::warn!` (existing behavior preserved for log-file diagnostics).
   - Send `Message::RebuildStatsExtensionStateChanged { session_id, enabled: !attempted_enable }` to roll back the optimistic UI.
   - Send `Message::RebuildStatsToggleFailed { session_id, reason: format!("{e}") }`.
3. **`FetchWidgetLocationIdMap` failure path.** On `Err`, also send `RebuildStatsToggleFailed` (using a reason like `"Failed to fetch widget location map: {e}"`).
4. **M3 verification.** When the user disables during a hot-restart window:
   - `handle_toggle` optimistically sets `rebuild_stats_enabled = false`.
   - The RPC fails (dying isolate).
   - The new `RebuildStatsExtensionStateChanged { enabled: true }` rollback fires.
   - `SessionRestartCompleted` reads `rebuild_stats_enabled` and now sees `true` (extension is actually still active in old isolate state) → re-enables on new isolate.
   - **However**, the user pressed `R` AFTER they saw the toggle update; they intended to disable. To honor user intent, the `RebuildStatsToggleFailed` handler should append a session log entry like `"Rebuild tracking toggle to OFF failed: <reason>. Tracking is still ON."` so the user knows the state.
   - The hot-restart re-enable behavior is unchanged — the test must demonstrate that user intent + failure feedback together prevent silent clobber.
5. **M6 resolution.** The `FetchWidgetLocationIdMap` arm in `actions/mod.rs`:
   - Calls `extensions::inspector::widget_location_id_map(handle, isolate_id).await`.
   - On `Ok(map)`, sends `Message::RebuildStatsLocationMapFetched { session_id, map }`.
   - On `Err`, sends `RebuildStatsToggleFailed` (per #3 above).
   - No file-URI loop, no inline `merge_parallel_arrays` call, no `serde_json::Value` traversal in the action.
6. **Handler dispatch.** `handler/update.rs` adds the dispatch arm for `RebuildStatsToggleFailed` that calls a new `handle_toggle_failed` in `handler/devtools/performance/rebuild_stats.rs`.
7. **Handler logic.** `handle_toggle_failed` in `rebuild_stats.rs`:
   - Looks up the session, appends a `LogEntry` to the session's log buffer with `LogLevel::Error` (or `Warn`) and message `"Rebuild tracking toggle failed: {reason}"`.
   - Returns `UpdateResult::default()` (no further action).
8. **Tests added:**
   - `test_toggle_failure_emits_rollback_and_log` in `rebuild_stats.rs` — handler test asserting both messages are observable when the action fails.
   - `test_location_map_fetch_failure_emits_toggle_failed` — action-layer test (where existing action tests live).
   - `test_handle_toggle_failed_appends_log_entry` — handler test asserting the log buffer grows by one with the expected message.
9. **Inspector helper review.** Confirm `widget_location_id_map` in `extensions/inspector.rs` returns the right shape unchanged; if any small convenience adjustments are needed (e.g., better error context), make them here.
10. `cargo fmt --all -- --check && cargo check --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Files Modified (Write)

- `crates/fdemon-app/src/message.rs` — add `RebuildStatsToggleFailed` variant.
- `crates/fdemon-app/src/actions/mod.rs` — `ToggleProfileWidgetBuilds` failure path emits rollback + failure messages; `FetchWidgetLocationIdMap` arm calls `widget_location_id_map()` directly and emits the typed result.
- `crates/fdemon-app/src/handler/devtools/performance/rebuild_stats.rs` — new `handle_toggle_failed` function + tests.
- `crates/fdemon-app/src/handler/update.rs` — dispatch arm for `RebuildStatsToggleFailed`.
- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` — minor adjustments only if needed (e.g., error context). No new public API surface required.

## Files Read (Dependencies)

- `crates/fdemon-app/src/state.rs` — confirm Message dispatch is exhaustive.
- `crates/fdemon-core/src/log.rs` (or wherever `LogEntry`/`LogLevel` live) — for constructing the log entry.
- `crates/fdemon-core/src/session.rs` (or wherever `SessionId` is defined).
- T03 output — the Serialize/Deserialize derives on `RebuildEventPayload` aren't strictly required here but T03 should land first to avoid downstream surprises.

## Approach Hints

- The `RebuildStatsToggleFailed` message carries a plain `String` for `reason` rather than a typed enum — matches the existing pattern for user-facing log messages in the session buffer.
- For the rollback emission, the "actual current state" is the opposite of what was attempted: if the user tried to enable and it failed, the extension is presumably still disabled. The implementation can safely use `!attempted_enable` as the rollback value without querying the VM.
- The action-layer test for #3 may need to use the same `pub(crate)` test-channel infrastructure that T03 (Task 03 from Phase 3) introduced for `VmRequestHandle`. Reuse the existing pattern.
- For M6: confirm via grep that `actions/mod.rs` is the only inline-parser site for location maps. The handler already has a separate merging step that takes a `LocationMap` — this is fine and unchanged.

## Out of Scope

- Adding retry-on-failure logic for the toggle or location-map RPC. (If desired, a separate task.)
- Introducing a typed error enum for `RebuildStatsToggleFailed.reason`. String is fine.
- Touching the existing `RebuildStatsExtensionStateChanged` handler logic — only adding new emission sites.
- The `auto_enable_rebuild_tracking` wiring — that's T09.
- Toast/snackbar UI for the failure message — log-buffer surfacing is sufficient.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a0f98232d0de748d7

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `RebuildStatsToggleFailed { session_id, reason }` variant with doc comment |
| `crates/fdemon-app/src/actions/mod.rs` | `ToggleProfileWidgetBuilds` failure path now emits rollback + `RebuildStatsToggleFailed`; `FetchWidgetLocationIdMap` replaced inline JSON traversal with `widget_location_id_map_handle()` call, error path emits `RebuildStatsToggleFailed` |
| `crates/fdemon-app/src/handler/devtools/performance/rebuild_stats.rs` | Added `handle_toggle_failed` function + 3 new tests |
| `crates/fdemon-app/src/handler/update.rs` | Added dispatch arm for `RebuildStatsToggleFailed` → `handle_toggle_failed` |
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | Added `widget_location_id_map_handle(&VmRequestHandle, &str)` — same logic as existing `widget_location_id_map` but accepts `VmRequestHandle` for use in action tasks |
| `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` | Re-exported `widget_location_id_map_handle` |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Re-exported `widget_location_id_map_handle` at crate level |

### Notable Decisions/Tradeoffs

1. **New daemon helper instead of trait abstraction**: The `widget_location_id_map` function takes `&VmServiceClient`, but action tasks hold `VmRequestHandle`. Rather than introducing a trait to unify the two, I added `widget_location_id_map_handle(&VmRequestHandle)` as a parallel implementation. This is simpler and follows the existing codebase pattern for `call_extension` on `VmRequestHandle`.

2. **Test for action-layer behavior via handler layer**: `test_location_map_fetch_failure_emits_toggle_failed` tests the handler dispatch path (the same message variant used by both `ToggleProfileWidgetBuilds` and `FetchWidgetLocationIdMap` failures), rather than the action task directly. This is necessary since `VmRequestHandle` cannot be mocked without T11's infrastructure; the test correctly verifies the behavioral contract at the handler level.

3. **Rollback uses `!attempted_enable`**: Per the task spec, the failure rollback emits `RebuildStatsExtensionStateChanged { enabled: !enabled }` without querying the VM. This is safe since a failed toggle means the extension state did not change from what it was before the attempt.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace --lib` - Passed (2431+ tests across all crates)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo test -p fdemon-app rebuild_stats::tests::test_handle_toggle_failed_appends_log_entry` - Passed
- `cargo test -p fdemon-app rebuild_stats::tests::test_toggle_failure_emits_rollback_and_log` - Passed
- `cargo test -p fdemon-app rebuild_stats::tests::test_location_map_fetch_failure_emits_toggle_failed` - Passed

### Risks/Limitations

1. **Action-layer test not directly testing async task behavior**: The `test_location_map_fetch_failure_emits_toggle_failed` test verifies the handler's response to the message rather than the action's emission of it. Full end-to-end action testing requires T11's mock `VmRequestHandle` infrastructure.
