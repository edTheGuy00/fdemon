# Phase 2: Settings Launch Tab Fixes - Task Index

## Overview

Fix the "Add New Configuration" navigation bug, add dart defines key-value editor modal and extra args fuzzy picker modal to the settings panel, and ensure all changes persist to `.fdemon/launch.toml`.

**Total Tasks:** 6
**Estimated Hours:** 14-19 hours

## Task Dependency Graph

```
┌──────────────────────────┐     ┌──────────────────────────┐
│  01-fix-add-config-bug   │     │  02-settings-modal-state │
│  (no deps)               │     │  (no deps)               │
└──────────┬───────────────┘     └─────┬──────┬──────┬──────┘
           │                           │      │      │
           │              ┌────────────┘      │      └────────────┐
           │              ▼                   ▼                   ▼
           │   ┌──────────────────┐  ┌───────────────────┐  ┌────────────────────┐
           │   │ 03-dart-defines  │  │ 04-extra-args     │  │ 05-render-settings │
           │   │ -modal           │  │ -modal             │  │ -modals            │
           │   └────────┬─────────┘  └────────┬──────────┘  └────────┬───────────┘
           │            │                     │                      │
           └────────────┼─────────────────────┼──────────────────────┘
                        │                     │
                        ▼                     ▼
                  ┌─────────────────────────────────┐
                  │  06-phase2-tests                 │
                  │  (depends on: 01, 03, 04, 05)   │
                  └─────────────────────────────────┘
```

**Execution waves:**
- **Wave 1** (parallel): 01-fix-add-config-bug, 02-settings-modal-state
- **Wave 2** (parallel, after 02): 03-dart-defines-modal, 04-extra-args-modal, 05-render-settings-modals
- **Wave 3** (after all): 06-phase2-tests

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-fix-add-config-bug](tasks/01-fix-add-config-bug.md) | Done | - | 2-3h | `settings_handlers.rs`, `settings_items.rs` |
| 2 | [02-settings-modal-state](tasks/02-settings-modal-state.md) | Done | - | 2-3h | `state.rs`, `message.rs`, `types.rs`, `update.rs` |
| 3 | [03-dart-defines-modal](tasks/03-dart-defines-modal.md) | Done | 2 | 3-4h | `settings_handlers.rs`, `keys.rs`, `settings.rs`, `update.rs` |
| 4 | [04-extra-args-modal](tasks/04-extra-args-modal.md) | Done | 2 | 2-3h | `settings_handlers.rs`, `keys.rs`, `settings.rs`, `update.rs` |
| 5 | [05-render-settings-modals](tasks/05-render-settings-modals.md) | Done | 2 | 2-3h | `widgets/settings_panel/mod.rs` |
| 6 | [06-phase2-tests](tasks/06-phase2-tests.md) | Done | 1, 3, 4, 5 | 2-3h | Cross-crate integration tests |

## Success Criteria

Phase 2 is complete when:

- [ ] "Add New Configuration" is navigable and creates a new config on Enter
- [ ] Dart defines editing opens the `DartDefinesModal` (key-value CRUD editor) matching the new session dialog UX
- [ ] Extra args editing opens a `FuzzyModal` with custom input support
- [ ] Changes to dart defines and extra args persist to `.fdemon/launch.toml`
- [ ] All existing settings tests pass (no regressions)
- [ ] All new tests pass
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Key Design Decisions

### DartDefinesModal over FuzzyModal for dart_defines

Research revealed that `DartDefinesModal` (key-value CRUD editor with master-detail layout) is far better suited for dart defines than `FuzzyModal` (read-only fuzzy picker). The `DartDefinesModal` supports:
- Add/edit/delete operations on key-value pairs
- Inline validation (empty key = save failure)
- Two-pane focus model (list + edit form)
- Already used in the new session dialog for the same purpose

### FuzzyModal with allows_custom for extra_args

Extra args are flat strings (not key-value), so `FuzzyModal` with `FuzzyModalType::ExtraArgs` (`allows_custom: true`) is appropriate. Users can type arbitrary args and press Enter to add them.

### Reuse over duplication

Both `DartDefinesModalState` and `FuzzyModalState` are reused as-is from the `new_session_dialog` module. Only the message variants and handler routing are new — the state machines and TUI widgets are shared.

## Notes

- `DartDefinesModalState` is at `crates/fdemon-app/src/new_session_dialog/state.rs:184-399`
- `FuzzyModalState` is at `crates/fdemon-app/src/new_session_dialog/state.rs:15-137`
- `DartDefinesModal` widget is at `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs`
- `FuzzyModal` widget is at `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs`
- `apply_launch_config_change()` is at `crates/fdemon-app/src/handler/settings.rs:159-204`
- `get_item_count_for_tab()` is at `crates/fdemon-app/src/handler/settings_handlers.rs:351-379`
- `get_selected_item()` is at `crates/fdemon-app/src/settings_items.rs:27-54`
- `Message::LaunchConfigCreate` handler is at `crates/fdemon-app/src/handler/update.rs:710-726`
