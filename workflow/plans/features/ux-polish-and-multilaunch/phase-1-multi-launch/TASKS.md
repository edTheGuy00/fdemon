# Phase 1: Multi-Device Launch Picker — Task Index

## Overview

Let the user check multiple **connected** devices in the new-session dialog and launch them all with one confirm, exploiting the existing 9-session `SessionManager`. Selection lives in `TargetSelectorState` independent of the cursor; `Space` toggles, `a` selects-all/clears, and the existing Launch action fans out to N sessions via `UpdateResult::actions_vec`. Zero checked → unchanged single-device behavior (no regression). Connected tab only (Bootable multi-select is out of scope).

**Total Tasks:** 4
**Estimated Hours:** 8–12h

## Task Dependency Graph

```
                  ┌──────────────────────────────┐
                  │ 01-multi-select-state         │  (foundation)
                  └───────────────┬──────────────┘
            ┌─────────────────────┼─────────────────────┐
            ▼                     ▼                     ▼
┌────────────────────┐ ┌────────────────────┐ ┌────────────────────┐
│ 02-keys-and-       │ │ 03-launch-n-       │ │ 04-render-         │
│    messages        │ │    sessions        │ │    checkboxes      │
└────────────────────┘ └────────────────────┘ └────────────────────┘
        (depends 01)        (depends 01)            (depends 01)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-multi-select-state](tasks/01-multi-select-state.md) | Not Started | - | 2–3h | `target_selector_state.rs` |
| 2 | [02-keys-and-messages](tasks/02-keys-and-messages.md) | Not Started | 1 | 2–3h | `message.rs`, `handler/keys.rs`, `handler/new_session/target_selector.rs`, `handler/update.rs` |
| 3 | [03-launch-n-sessions](tasks/03-launch-n-sessions.md) | Not Started | 1 | 3–4h | `handler/new_session/launch_context.rs`, `new_session_dialog/state.rs` |
| 4 | [04-render-checkboxes](tasks/04-render-checkboxes.md) | Not Started | 1 | 2–3h | `widgets/new_session_dialog/device_list.rs`, `widgets/new_session_dialog/target_selector.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-multi-select-state | `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | `crates/fdemon-app/src/new_session_dialog/device_groups.rs` |
| 02-keys-and-messages | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/new_session/target_selector.rs`, `crates/fdemon-app/src/handler/update.rs` | `target_selector_state.rs` |
| 03-launch-n-sessions | `crates/fdemon-app/src/handler/new_session/launch_context.rs`, `crates/fdemon-app/src/new_session_dialog/state.rs` | `target_selector_state.rs`, `session_manager.rs`, `new_session_dialog/types.rs` |
| 04-render-checkboxes | `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | `target_selector_state.rs` |

### Overlap Matrix

<!-- Read-only overlap on target_selector_state.rs is fine; only write overlap forces sequencing. -->

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None (01 writes state; 02 reads it) | Sequential (02 depends on 01) |
| 01 + 03 | None | Sequential (03 depends on 01) |
| 01 + 04 | None | Sequential (04 depends on 01) |
| 02 + 03 | None | Parallel (worktree) |
| 02 + 04 | None | Parallel (worktree) |
| 03 + 04 | None | Parallel (worktree) |

**Waves:** Wave 1 = `01`. Wave 2 = `02`, `03`, `04` in parallel worktrees (no shared write files; all only read `target_selector_state.rs`). Note `handler/new_session/target_selector.rs` (task 02) and `widgets/new_session_dialog/target_selector.rs` (task 04) are **different files in different crates** — no conflict.

## Success Criteria

Phase 1 is complete when:

- [ ] User can check ≥2 connected devices (`Space`) and launch all of them with one confirm.
- [ ] `a` toggles select-all / clear-all across the current Connected list.
- [ ] Zero devices checked → launch uses the cursor device exactly as today (no regression).
- [ ] Devices already running a session are skipped; over-capacity (>9) launches up to the cap and reports "launched X of Y".
- [ ] Checkboxes render per connected device; checked count is visible; footer hint reflects the new keys.
- [ ] Selection set prunes ids that disappear on device-list refresh.
- [ ] All new state/handler logic has unit tests; `cargo test --workspace` and `cargo clippy --workspace` pass.

## Keyboard Shortcuts

| Key | Context | Action |
|-----|---------|--------|
| `Space` | New-session dialog, TargetSelector pane, Connected tab | Toggle selection of the cursor device |
| `a` | New-session dialog, TargetSelector pane, Connected tab | Select all / clear all |
| `Enter`/Launch | New-session dialog | Launch all checked (or cursor device if none checked) |

## Notes

- **Scope:** Connected tab only. On the Bootable tab, `Space`/`a` are no-ops; multi-boot is deferred (plan Future Enhancements).
- **Selection identity:** key the checked set by device `id` so it survives list refreshes and pane/tab switches.
- **Relation to Phase 5 (runnable filtering, separate unit):** once Phase 5 lands, the checked set must skip `is_supported == false` devices. Phase 1 does not hard-depend on Phase 5; if Phase 5 ships first, task 01 should consult the supportability flag when toggling/selecting-all.
- **Shared launch context:** all N sessions use the single right-pane launch context (mode/flavor/dart-defines/config); only the device varies.
- A `docs/KEYBINDINGS.md` update (Space/`a` in the dialog) is unmanaged-doc work — fold into task 02 or do as a trailing edit.
