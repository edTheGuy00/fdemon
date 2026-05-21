## Task: Fix the three critical handler-layer bugs from the Phase 1 review

**Objective:** Resolve all three Critical findings in [`REVIEW.md`](../../../../../reviews/features/devtools-performance-memory-split-phase-1/REVIEW.md) and add the missing regression tests (m7, m8). Restore correct alloc-polling behaviour for the `default_panel = "memory"` cold path (C1), route Memory-panel mouse-wheel events to the right state (C2), and commit the already-applied in-flight fix for `performance.monitoring_active` (C3).

**Depends on:** None (Wave 1)

**Agent:** implementor

**Estimated Time:** 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/update.rs` — extend the alloc-unpause guard at the `VmServicePerformanceMonitoringStarted` handler (currently around line 1920) AND verify the in-flight C3 fix (both `performance.monitoring_active = true` and `memory.monitoring_active = true` are set there).
- `crates/fdemon-app/src/handler/mouse/devtools.rs` — add a new `handle_memory_scroll` function and wire `DevToolsPanel::Memory` to it instead of `handle_performance_scroll`.
- `crates/fdemon-app/src/handler/tests.rs` — extend `test_performance_monitoring_started_stores_shutdown_tx` (or add adjacent tests) with the three missing regressions: C3 (both flags flip), m7 (Perf↔Memory switch leaves `alloc_pause_tx` untouched), m8 (`default_panel = "memory"` cold path actually unpauses alloc). C2 regression tests live in `mouse/devtools.rs`'s own `#[cfg(test)] mod tests` block.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/devtools/memory.rs` — confirms which `Mem*` messages the new `handle_memory_scroll` should emit.
- `crates/fdemon-app/src/message.rs` — variant names for `Mem*` messages.
- `crates/fdemon-app/src/state.rs` — `DevToolsPanel::Memory` and the existing structure of `devtools_view_state`.

### Background

The user reported a live regression during the Phase 1 review: the Performance tab was stuck on "performance monitoring starting…". Root cause was that `Message::VmServiceMemorySnapshot` used to set `performance.monitoring_active = true` as a side-effect; T02 of Phase 1 moved both the data push *and* the flag write to `memory.*` but nothing now writes `performance.monitoring_active = true`. **This was fixed in-flight before the orchestrator finished review consolidation** by adding `handle.session.performance.monitoring_active = true` and `handle.session.memory.monitoring_active = true` inside the `VmServicePerformanceMonitoringStarted` handler. The fix and a one-line test extension live in the working tree at the time this task begins.

The logic reviewer independently found two more bugs in the same general area:

- **C1** — at `update.rs:1920` the alloc-unpause guard inside the same `VmServicePerformanceMonitoringStarted` handler only matches `DevToolsPanel::Performance`. `default_panel = "memory"` users never get an unpause for the cold-start path because `handle_enter_devtools_mode` queued the `StartPerformanceMonitoring` action *before* `alloc_pause_tx` existed, and the catch-all unpause inside this handler skips Memory. Alloc polling stays paused indefinitely.
- **C2** — `handler/mouse/devtools.rs:99` routes `DevToolsPanel::Memory` wheel events to `handle_performance_scroll`, emitting `Message::PerfScrollUp/Down/PageUp/PageDown`. The handlers behind those messages mutate `session.performance.frame_chart_scroll_offset` — completely unrelated to memory state. T01 of Phase 1 left a "T03 will replace this" comment that T03 missed.

Three reviewers flagged C2 independently (architecture, logic, risks). The fix is to add a `handle_memory_scroll` that mirrors `handle_performance_scroll`'s structure but emits `MemScroll*` / `MemPage*` messages — the handlers behind those already exist in `handler/devtools/memory.rs`.

### Details

#### 1. `handler/update.rs` — extend the alloc-unpause guard (C1) + verify C3

The current handler (around line 1875–1921) reads:

```rust
Message::VmServicePerformanceMonitoringStarted {
    session_id, perf_shutdown_tx, perf_task_handle, alloc_pause_tx, perf_pause_tx,
} => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // ...store senders...
        handle.perf_pause_tx = Some(perf_pause_tx);

        // C3 in-flight fix — verify these two lines are present (they should be from
        // the working-tree patch that landed during review consolidation):
        handle.session.performance.monitoring_active = true;
        handle.session.memory.monitoring_active = true;

        // Adjust initial pause values based on current UI state.
        if state.ui_mode == UiMode::DevTools {
            if let Some(ref tx) = handle.perf_pause_tx {
                let _ = tx.send(false);
            }
            // ← BUG C1: this guard only matches Performance
            if state.devtools_view_state.active_panel == DevToolsPanel::Performance {
                if let Some(ref tx) = handle.alloc_pause_tx {
                    let _ = tx.send(false);
                }
            }
        }
    }
    UpdateResult::none()
}
```

**Required edit:** change the panel guard from:

```rust
if state.devtools_view_state.active_panel == DevToolsPanel::Performance {
```

to:

```rust
if matches!(
    state.devtools_view_state.active_panel,
    DevToolsPanel::Performance | DevToolsPanel::Memory,
) {
```

This aligns the cold-start unpause path with the documented invariant (`docs/ARCHITECTURE.md:908`: "Entering either the Performance tab or the Memory tab sends `false` (unpause).") and with `handle_enter_devtools_mode` and `handle_switch_panel`, which both treat the two panels equivalently.

**Comment update:** replace the existing comment "Unpause allocation polling only if the Performance panel is active." with one that names both panels.

If for any reason the C3 fix lines (`performance.monitoring_active = true; memory.monitoring_active = true;`) are *not* already in `update.rs`, add them inside this handler just before the pause-value adjustment block. The in-flight commit should already contain them — verify with `git diff a094416..HEAD -- crates/fdemon-app/src/handler/update.rs`.

#### 2. `handler/mouse/devtools.rs` — add `handle_memory_scroll`, retire the placeholder (C2)

The current `handle_scroll` function (lines 93–102) reads:

```rust
pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    match state.devtools_view_state.active_panel {
        DevToolsPanel::Inspector => handle_inspector_scroll(dir, mods),
        DevToolsPanel::Performance => handle_performance_scroll(dir, mods),
        // Memory panel uses the same scroll behaviour as Performance (row scroll,
        // Shift for page step) until T03 introduces dedicated memory scroll logic.
        DevToolsPanel::Memory => handle_performance_scroll(dir, mods),
        DevToolsPanel::Network => handle_network_scroll(state, dir, mods),
    }
}
```

**Required edits:**

1. Replace the two-line `Memory` arm comment + delegation with `DevToolsPanel::Memory => handle_memory_scroll(dir, mods),`.
2. Add a new module-private function below `handle_performance_scroll` whose body mirrors it but emits `MemScroll*` / `MemPage*`:

   ```rust
   fn handle_memory_scroll(dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
       // Shift+wheel → page step (mirrors handle_performance_scroll).
       if mods.is_shift_only() {
           return match dir {
               ScrollDir::Up => Some(Message::MemPageUp),
               ScrollDir::Down => Some(Message::MemPageDown),
               ScrollDir::Left | ScrollDir::Right => None,
           };
       }

       // Ctrl or Alt → no-op for parity with the other panels.
       if mods.ctrl || mods.alt {
           return None;
       }

       match dir {
           ScrollDir::Up => Some(Message::MemScrollUp),
           ScrollDir::Down => Some(Message::MemScrollDown),
           ScrollDir::Left | ScrollDir::Right => None,
       }
   }
   ```

3. Update the module-level `//!` docstring at the top of the file. The current line 4 reads "Scroll dispatches by `state.devtools_view_state.active_panel`:" followed by a bullet for Inspector, Performance, Network — but no Memory entry (Phase 1 missed it). Add a Memory bullet matching the Performance one's style: "Memory → memory chart / alloc-table scroll (Up/Down → `MemScrollUp`/`MemScrollDown`; Shift+Up/Down → `MemPageUp`/`MemPageDown`; Ctrl/Alt → no-op; horizontal wheel → no-op). The keyboard handler routes by `focused_section`."

#### 3. `handler/mouse/devtools.rs` tests — C2 regression coverage

The existing test module already has thorough coverage for the Performance panel's wheel behaviour. Add the symmetric Memory tests immediately after the Performance block (around line 247):

| Test name | Assertion |
|---|---|
| `memory_wheel_up_emits_mem_scroll_up` | `handle_scroll(state, ScrollDir::Up, NONE)` → `Some(Message::MemScrollUp)` |
| `memory_wheel_down_emits_mem_scroll_down` | `handle_scroll(state, ScrollDir::Down, NONE)` → `Some(Message::MemScrollDown)` |
| `memory_shift_wheel_up_emits_mem_page_up` | `handle_scroll(state, ScrollDir::Up, shift_only)` → `Some(Message::MemPageUp)` |
| `memory_shift_wheel_down_emits_mem_page_down` | `handle_scroll(state, ScrollDir::Down, shift_only)` → `Some(Message::MemPageDown)` |
| `memory_ctrl_modifier_returns_none` | Ctrl+Up/Down both `None` |
| `memory_alt_modifier_returns_none` | Alt+Up/Down both `None` |

Plus one regression test that locks in the routing fix:

| Test name | Assertion |
|---|---|
| `memory_wheel_does_not_emit_perf_scroll_messages` | For each direction × each modifier, the resulting `Option<Message>` is **never** a `Message::PerfScroll*` or `Message::PerfPage*` variant. |

The existing `horizontal_wheel_no_op_in_every_panel` test already includes `DevToolsPanel::Memory` in its panel list — that test now passes for the right reason rather than accidentally.

#### 4. `handler/tests.rs` — C1 + C3 + m7 + m8 regression coverage

Three places to edit:

**(a) Extend `test_performance_monitoring_started_stores_shutdown_tx` (C3)** — the in-flight patch already added two assertions for `performance.monitoring_active == true` and `memory.monitoring_active == true`. Verify they are present. The test now also implicitly covers C1's "Performance active" branch; rename it to `test_performance_monitoring_started_stores_handles_and_flips_flags` if the wording feels misleading.

**(b) Add `test_lazy_start_memory_default_unpauses_alloc` (C1, m8)** — symmetric to `test_monitoring_started_handler_adjusts_alloc_for_performance_panel` at handler/tests.rs:9912 but with `active_panel = DevToolsPanel::Memory`. Required assertions after dispatching the message:

```rust
let alloc_rx_value = *alloc_pause_rx.borrow();
assert!(
    !alloc_rx_value,
    "alloc_pause_tx must be sent `false` when monitoring starts with Memory as the active panel"
);
```

Also assert `state.session_manager.get(session_id).unwrap().session.memory.monitoring_active == true` for completeness.

**(c) Add `test_switch_performance_to_memory_does_not_pause_alloc` and the symmetric reverse (m7)** — protect the central refactor claim that swapping between Performance and Memory does not toggle alloc polling.

Place these near `test_monitoring_started_handler_adjusts_alloc_for_performance_panel` (handler/tests.rs:9912). Setup pattern:

1. Create a session, dispatch `VmServicePerformanceMonitoringStarted` with `active_panel = Performance` (so `alloc_pause_tx` is initially sent `false`).
2. Record `*alloc_pause_rx.borrow_and_update()` (consume the initial change notification).
3. Dispatch `Message::SwitchDevToolsPanel(DevToolsPanel::Memory)`.
4. Assert `!alloc_pause_rx.has_changed().unwrap()` OR — if the watch channel API doesn't expose has_changed cleanly in this codebase — assert that `*alloc_pause_rx.borrow()` is still `false` (the sender coalesces equal values, so the receiver should not have a new change to report).
5. Repeat symmetrically for `Memory → Performance`.

If `has_changed()` returns `true` after the switch, the refactor's central correctness claim is violated.

#### 5. Quality gate

Run the verification suite before reporting Done. See `docs/DEVELOPMENT.md`:

```bash
cargo fmt --all -- --check && \
  cargo check --workspace --all-targets && \
  cargo test --workspace && \
  cargo clippy --workspace --all-targets -- -D warnings
```

All four must pass.

### Acceptance Criteria

- [ ] `cargo check --workspace --all-targets` succeeds with zero warnings.
- [ ] `cargo test --workspace` passes; the new tests in (3) and (4) all pass.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] Setting `default_panel = "memory"` (or programmatically setting `active_panel = Memory` before dispatching `VmServicePerformanceMonitoringStarted`) results in `alloc_pause_tx.borrow() == false`.
- [ ] Dispatching `VmServicePerformanceMonitoringStarted` results in **both** `session.performance.monitoring_active == true` and `session.memory.monitoring_active == true`.
- [ ] Mouse wheel + Shift+wheel + Ctrl/Alt modifiers on `DevToolsPanel::Memory` produce the expected `Mem*` messages or `None`; no `Perf*` variant is reachable.
- [ ] No test in `handler/mouse/devtools.rs` or `handler/tests.rs` asserts `Memory` wheel events produce `Perf*` messages (the previous transitional behaviour is gone).
- [ ] `Memory` arm comment "until T03 introduces dedicated memory scroll logic" is removed.
- [ ] Tab-bar / module-level docstring in `mouse/devtools.rs` now mentions the Memory routing.

### Module Structure

No new modules. All changes land in pre-existing files. `handle_memory_scroll` is a new module-private function alongside the existing `handle_inspector_scroll` / `handle_performance_scroll` / `handle_network_scroll` siblings.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a98e237b761c667e9

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/update.rs` | C1 fix: extended alloc-unpause guard from `DevToolsPanel::Performance` to `DevToolsPanel::Performance \| DevToolsPanel::Memory` in the `VmServicePerformanceMonitoringStarted` handler. C3 was already present (both monitoring_active flags). |
| `crates/fdemon-app/src/handler/mouse/devtools.rs` | C2 fix: added `handle_memory_scroll` function emitting `MemScroll*`/`MemPage*` messages; updated `handle_scroll` to route `DevToolsPanel::Memory` to the new function; removed the "until T03" placeholder comment; updated module-level docstring to include Memory routing. Added 7 regression tests for C2 in the `tests` module. |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | m7 fix: added `entering_from_non_alloc` guard to both `Performance` and `Memory` arms of `handle_switch_panel` to avoid sending a spurious `alloc_pause_tx` value when switching between the two alloc-enabled panels. |
| `crates/fdemon-app/src/handler/tests.rs` | Added 3 regression tests: `test_lazy_start_memory_default_unpauses_alloc` (C1/m8), `test_switch_performance_to_memory_does_not_retoggle_alloc` (m7), `test_switch_memory_to_performance_does_not_retoggle_alloc` (m7 symmetric). |

### Notable Decisions/Tradeoffs

1. **handle_switch_panel m7 fix**: The m7 tests required that switching between Performance and Memory does not re-send on `alloc_pause_tx`. The existing code sent `false` unconditionally in each arm. The fix adds an `entering_from_non_alloc` guard — only send `false` when entering from a non-alloc panel. This is semantically correct: if alloc was already unpaused, there is no need to send again, and doing so triggers a spurious change notification on the watch channel. The `leaving_alloc_panel` guard already handled the pause side correctly.

2. **C3 verification**: The C3 fix (both `performance.monitoring_active = true` and `memory.monitoring_active = true`) was already present from the in-flight patch. The existing test `test_performance_monitoring_started_stores_shutdown_tx` already asserts both flags, so no additional C3-specific test was needed.

### Testing Performed

- `cargo fmt --all -- --check` - PASS
- `cargo check --workspace --all-targets` - PASS (0 warnings)
- `cargo test --workspace` - PASS (2379 fdemon-app unit tests, all others pass; 0 failures across workspace)
- `cargo clippy --workspace --all-targets -- -D warnings` - PASS (0 warnings)

### Risks/Limitations

1. **m7 guard semantics**: The `entering_from_non_alloc` guard assumes that if you come from a Memory or Performance panel, `alloc_pause_tx` is already `false`. This invariant holds because (a) the `leaving_alloc_panel` guard only sends `true` when leaving to a non-alloc panel, and (b) the `VmServicePerformanceMonitoringStarted` handler now unpauses alloc for both panels. If a future code path leaves `alloc_pause_tx = true` while on an alloc panel, the guard would incorrectly skip the unpause. No such path currently exists.
