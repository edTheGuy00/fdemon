# 09 — Auto-Enable Rebuild Tracking Wiring + Typo Fix

**Wave:** 3
**Depends On:** 04
**Agent:** implementor
**Estimated Hours:** 1.5–2h
**Addresses:** M2, L9

## Context

Two unrelated cleanups in the session lifecycle layer:

- **M2.** `auto_enable_rebuild_tracking` (defined in `crates/fdemon-app/src/config/types.rs:430–431`) is declared, defaulted, parsed from TOML, and documented in CONFIGURATION.md — but no production code path reads it. The doc comment promises "auto-enable[s] rebuild tracking on session start" but setting `true` in `.fdemon/config.toml` is a no-op.
- **L9.** code_quality_inspector flagged a possible `/` instead of `//` comment typo at `crates/fdemon-app/src/handler/session_lifecycle.rs:177`. The build passes (so it's either a misreading by the agent or a compiler-tolerated single-`/` interpreted as a division operator with no left operand). Worth verifying and cleaning up either way.

Per PLAN.md Design Decision §3, M2 is wired (not deleted): on `VmServiceConnected`, if `settings.devtools.auto_enable_rebuild_tracking == true` and `rebuild_stats_enabled == false`, queue `UpdateAction::ToggleProfileWidgetBuilds { enabled: true, vm_handle: None }`.

## Acceptance Criteria

1. **M2 wired.** In `crates/fdemon-app/src/handler/update.rs`, the `VmServiceConnected` arm:
   - Reads `state.settings.devtools.auto_enable_rebuild_tracking`.
   - If `true` AND the session's `rebuild_stats_enabled` is `false`, returns `UpdateAction::ToggleProfileWidgetBuilds { session_id, enabled: true, vm_handle: None }` (hydrated in `process.rs` as usual).
   - If the action already exists from the existing arm (e.g., if `SessionRestartCompleted` is handled in the same path), order the auto-enable so it does NOT race with the hot-restart re-enable. The hot-restart re-enable should still win on `SessionRestartCompleted` (existing T04 behavior).
2. **M2 test.** `test_auto_enable_rebuild_tracking_queues_toggle_on_vm_service_connected`:
   - Constructs an `AppState` with `auto_enable_rebuild_tracking = true`.
   - Dispatches `Message::VmServiceConnected { ... }`.
   - Asserts the returned `UpdateAction` includes `ToggleProfileWidgetBuilds { enabled: true, ... }`.
3. **M2 negative test.** `test_auto_enable_skipped_when_already_enabled`:
   - Session's `rebuild_stats_enabled = true`.
   - `VmServiceConnected` fires.
   - Asserts no `ToggleProfileWidgetBuilds` action is queued (idempotent).
4. **M2 doc clarification.** Update the doc comment in `config/types.rs` near `auto_enable_rebuild_tracking` to specify the trigger point: "When `true`, fdemon queues `ext.flutter.profileWidgetBuilds = true` on `VmServiceConnected`. Hot-restart preservation (re-enable if previously on) is independent of this setting."
5. **L9 investigated.**
   - Read `crates/fdemon-app/src/handler/session_lifecycle.rs:177` and verify whether the line is actually `/` (single slash) or `//` (proper comment).
   - If it's actually a typo, fix it.
   - If it's a false positive, leave as-is and note in the completion summary.
6. `cargo fmt --all -- --check && cargo check -p fdemon-app && cargo test -p fdemon-app && cargo clippy -p fdemon-app --all-targets -- -D warnings` all pass.

## Files Modified (Write)

- `crates/fdemon-app/src/handler/update.rs` — `VmServiceConnected` arm gains auto-enable check; tests in the same module.
- `crates/fdemon-app/src/handler/session_lifecycle.rs` — L9 fix if real.
- `crates/fdemon-app/src/config/types.rs` — doc-comment clarification on `auto_enable_rebuild_tracking`.

## Files Read (Dependencies)

- T04 outputs — `handler/update.rs` is also edited by T04 for the `RebuildStatsToggleFailed` dispatch arm. T09 follows T04 sequentially per the wave plan.
- `crates/fdemon-app/src/handler/update.rs:222–264` (after T04 lands) — confirm the existing `SessionRestartCompleted` re-enable behavior remains unchanged and the new auto-enable wiring does not race with it.
- `crates/fdemon-app/src/session/performance.rs` — confirm `rebuild_stats_enabled` field name.

## Approach Hints

- The auto-enable check is similar in shape to the existing `SessionRestartCompleted` re-enable: read state, gate on a boolean, return an action. Likely a 3–4 line addition.
- For ordering: `VmServiceConnected` runs on first connect; `SessionRestartCompleted` runs after hot-restart. They are mutually exclusive events for any given moment, so race is impossible in practice — but document the ordering in a comment for future readers.
- For L9: a quick `grep -n '^/[^/]' crates/fdemon-app/src/handler/session_lifecycle.rs` will surface single-slash lines if any. If the line is purely a comment-formatting issue, the fix is one character.
- The doc-comment update should also mention that on `SessionRestartCompleted`, the previous-state preservation (Phase 3 hot-restart re-enable) takes precedence over `auto_enable_rebuild_tracking`. This makes the interaction explicit.

## Out of Scope

- Adding an opposite setting (e.g., `auto_disable_rebuild_tracking_on_release_mode`). Defer.
- Changing the default value of `auto_enable_rebuild_tracking` from `false`.
- Wiring the setting to fire on other events (e.g., `SessionStarted` before VM Service is ready). `VmServiceConnected` is the right hook because that's when the toggle can actually succeed.
- Surfacing the auto-enable result via a log entry. T04's `RebuildStatsToggleFailed` will surface failures; success is silent (consistent with manual toggle).

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/update.rs` | `VmServiceConnected` arm reads `auto_enable_rebuild_tracking` setting; if `true` and `rebuild_stats_enabled` is `false` (always true after the per-connect reset), queues `ToggleProfileWidgetBuilds { enabled: true, vm_handle: None }` in `extra_actions`. Comment block updated from "two things" to "three things". |
| `crates/fdemon-app/src/handler/tests.rs` | Three new tests: `test_auto_enable_rebuild_tracking_queues_toggle_on_vm_service_connected` (positive), `test_auto_enable_skipped_when_already_enabled` (documents the post-reset behaviour), `test_auto_enable_not_queued_when_setting_is_false` (default/negative). |
| `crates/fdemon-app/src/config/types.rs` | Doc comment on `auto_enable_rebuild_tracking` expanded to specify trigger point (`VmServiceConnected`), note the independence from hot-restart re-enable (`SessionRestartCompleted`), and clarify the hot-restart path takes precedence because they fire at different lifecycle points. |

### Notable Decisions/Tradeoffs

1. **extra_actions slot**: The `VmServiceConnected` arm already uses `action` for `StartPerformanceMonitoring` and `message` for `TriggerDevToolsServeFallback`. The auto-enable goes into `extra_actions` rather than replacing any existing slot, preserving all three concurrent effects cleanly.
2. **Post-reset idempotency**: `PerformanceState` is unconditionally reset by the handler before the gate check, so `rebuild_stats_enabled` is always `false` at check time. The gate against `!rebuild_stats_enabled` is still correct and forward-safe; if the reset were ever removed or conditioned, the gate would prevent a double-toggle.
3. **L9 investigation**: Line 177 of `session_lifecycle.rs` is a proper `//` comment ("// Timeline polls at 1 Hz..."). The `grep -n '^/[^/]'` search returned no results — the L9 flag was a false positive by the code-quality agent.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (2441 tests, 0 failed)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Auto-enable fires on every reconnect**: Because `PerformanceState` is reset on `VmServiceConnected`, the auto-enable will fire on every VM reconnect (not just first connect). This is consistent with the existing `auto_repaint_rainbow` / `auto_performance_overlay` behaviour and is the intended design per PLAN.md §3.
