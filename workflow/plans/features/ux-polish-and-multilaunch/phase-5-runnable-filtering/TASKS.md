# Phase 5: Runnable-Device Filtering — Task Index

## Overview

Stop offering devices the Flutter toolchain won't actually run for this project, eliminating the "No supported devices found" failure that surfaces *after* a confirmed launch. `flutter devices --machine` already emits `isSupported` (and a `capabilities` object) per device, but fdemon parses neither. This phase captures `is_supported` on the `Device` struct (serde `default = true` for backward/daemon-mode compatibility), excludes explicitly-unsupported connected devices from the new-session dialog's Connected tab — keeping the multi-select (Phase 1) checked-set, cursor, and click-regions all aligned — and adds an actionable empty state for the "devices found but none runnable" case.

**Total Tasks:** 3
**Estimated Hours:** 4–6h

## Task Dependency Graph

```
                ┌──────────────────────────────────┐
                │ 01-device-supportability-flag      │  (foundation: daemon)
                │   add is_supported + capabilities  │
                └─────────────────┬──────────────────┘
                  ┌───────────────┴────────────────┐
                  ▼                                 ▼
   ┌──────────────────────────────┐   ┌──────────────────────────────┐
   │ 02-filter-and-skip-unsupported│   │ 03-empty-state-messaging      │
   │   (fdemon-app)               │   │   (fdemon-tui)                │
   └──────────────────────────────┘   └──────────────────────────────┘
          (depends 01)                        (depends 01)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-device-supportability-flag](tasks/01-device-supportability-flag.md) | ✅ Done | - | 1–2h | `fdemon-daemon/src/devices.rs` |
| 2 | [02-filter-and-skip-unsupported](tasks/02-filter-and-skip-unsupported.md) | ✅ Done | 1 | 2–3h | `fdemon-app/src/new_session_dialog/device_groups.rs`, `target_selector_state.rs` |
| 3 | [03-empty-state-messaging](tasks/03-empty-state-messaging.md) | ✅ Done | 1 | 1–2h | `fdemon-tui/src/widgets/new_session_dialog/device_list.rs`, `target_selector.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-device-supportability-flag | `crates/fdemon-daemon/src/devices.rs` | - |
| 02-filter-and-skip-unsupported | `crates/fdemon-app/src/new_session_dialog/device_groups.rs`, `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | `crates/fdemon-daemon/src/devices.rs` |
| 03-empty-state-messaging | `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | `crates/fdemon-app/src/new_session_dialog/device_groups.rs`, `crates/fdemon-daemon/src/devices.rs` |

### Overlap Matrix

<!-- Read-only overlap on devices.rs / device_groups.rs is fine; only write overlap forces sequencing. -->

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None (01 writes daemon struct; 02 reads it) | Sequential (02 depends on 01) |
| 01 + 03 | None (01 writes daemon struct; 03 reads it) | Sequential (03 depends on 01) |
| 02 + 03 | None (different crates, different files) | Parallel (worktree) |

**Waves:** Wave 1 = `01` (must land first — the `is_supported` field is consumed by both downstream tasks). Wave 2 = `02` + `03` in parallel worktrees (no shared write files; 02 is fdemon-app, 03 is fdemon-tui).

> **Critical design note for task 02 ↔ 03:** The TUI render path `connected_device_list_render_with_regions` (`device_list.rs:209`) independently calls `group_connected_devices(list.devices)` — the **same** function the app-layer state uses in `compute_flat_list` and `selected_connected_device`. Task 02 must place the `is_supported` filter **inside `group_connected_devices`** (the single shared chokepoint) so that the flat-list indices, the cursor (`selected_index`), the multi-select checked-set, and the widget's per-row click regions stay aligned. If the filter were applied in only one layer, indices would diverge and clicks/highlights would target the wrong row. Task 03 then renders against the already-filtered list and only adds messaging — it must **not** add a second, independent filter.

## Success Criteria

Phase 5 is complete when:

- [x] `Device` captures `is_supported: bool` (serde `default = true`); absent/abbreviated payloads (and daemon `device.added` events using the leaner `DeviceInfo`) are unaffected and remain visible.
- [x] Explicitly-unsupported connected devices (`isSupported: false`) are excluded from the Connected tab; cursor navigation, scroll, and mouse click-regions remain correct (no off-by-one).
- [x] Multi-select (Phase 1) cannot check an unsupported device via `Space` and `a`/select-all skips them; `checked_devices()` never returns an unsupported device.
- [x] When all discovered connected devices are filtered out, the dialog shows an actionable empty state ("Devices found but none runnable for this project — check enabled platforms") distinct from the truly-empty "No connected devices" state.
- [x] Parsing is unit-tested for present-true, present-false, and absent `isSupported`; filtering and multi-select-skip logic are unit-tested.
- [x] `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Keyboard Shortcuts

No new keybindings. Existing Phase 1 keys (`Space` toggle, `a` select-all, `Enter` launch) gain an implicit guard: they skip unsupported devices.

## Notes

- **Scope:** Connected tab only (matches Phase 1). Bootable simulators/AVDs are not affected by `isSupported`.
- **Default = exclude, not dim.** The plan picks hard-exclude (with an explaining empty state) over "dim + non-selectable." This keeps the list to runnable targets and avoids a new disabled-row rendering path. The actionable empty state is what preserves discoverability of *why* a device is missing. (Revisit "dim" only if users report a runnable device disappearing.)
- **Relation to Phase 1:** Phase 1 already shipped two `TODO(phase-5)` markers at `target_selector_state.rs:389` (`toggle_checked_cursor`) and `:407` (`toggle_select_all`) — task 02 resolves both.
- **`capabilities` (hot-reload/restart flags):** captured in task 01 as an optional struct for *future* UI (greying out hot-reload where unsupported — a Future Enhancement). Not consumed by the dialog in this phase; parse-and-store only.
- **No core-doc update required:** this adds a struct field and a dialog filter, not a new module, layer dependency, or convention — so no `doc_maintainer` task. (If `capabilities` later drives UI, document then.)
