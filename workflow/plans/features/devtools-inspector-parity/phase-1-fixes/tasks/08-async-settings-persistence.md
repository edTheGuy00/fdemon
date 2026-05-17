## Task: Switch `save_settings` Handler Call Sites to `UpdateAction::PersistSettings`

**Objective**: Remove all synchronous `save_settings(&path, &settings)` calls from TEA handlers. Replace each with `UpdateAction::PersistSettings { settings, project_path }` (added by task 02) so file I/O runs on a background tokio task. Also fix the fixed-temp-filename TOCTOU concern in `save_settings` itself.

**Depends on**: 02 (consumes `UpdateAction::PersistSettings` + `Message::SettingsPersisted/Failed`), 07 (same file `handler/devtools/inspector.rs` — sequential)

**Estimated Time**: 1–1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs` — `handle_toggle_hide_implementation` (~inspector.rs:585) returns `UpdateAction::PersistSettings`, no longer calls `save_settings` directly.
- `crates/fdemon-app/src/handler/settings_handlers.rs` — same migration for the settings-panel save handler (~line 173).
- `crates/fdemon-app/src/config/settings.rs` — use a unique temp filename (e.g. via the `tempfile` crate, already a dev-dep — check Cargo.toml).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/mod.rs` — `UpdateAction::PersistSettings` variant (task 02).
- `crates/fdemon-app/src/actions/mod.rs` — dispatch arm (task 02).

### Review Items Resolved

- **M3** — `save_settings()` runs synchronously on every Shift+H — TUI-loop stall risk
- **m7** — `save_settings()` uses fixed temp filename `.config.toml.tmp` (TOCTOU / readable intermediate)

### Details

#### M3 — Migrate `handle_toggle_hide_implementation`

Currently (inspector.rs ~585):
```rust
state.settings.devtools.hide_implementation_widgets =
    inspector.hide_implementation_widgets;
if let Err(e) = save_settings(&state.project_path, &state.settings) {
    tracing::warn!("save_settings failed: {e}");
}
UpdateResult::none()
```

After:
```rust
state.settings.devtools.hide_implementation_widgets =
    inspector.hide_implementation_widgets;
UpdateResult::action(UpdateAction::PersistSettings {
    settings: state.settings.clone(),
    project_path: state.project_path.clone(),
})
```

(Adjust to the actual `UpdateResult::action(...)` constructor — verify the existing API.)

#### M3 — Migrate `settings_handlers.rs`

The existing synchronous save in `crates/fdemon-app/src/handler/settings_handlers.rs:173` should follow the same migration. If multiple handler functions in this file save, migrate all of them. After migration, no `save_settings(...)` call sites should remain in any handler. Grep to confirm:

```bash
rg -n "save_settings\(" crates/fdemon-app/src/handler/
```

The only remaining call site should be the dispatch arm inside `crates/fdemon-app/src/actions/mod.rs` (added by task 02).

#### m7 — Unique temp filename in `save_settings`

Currently at `crates/fdemon-app/src/config/settings.rs:536`:
```rust
let temp_path = fdemon_dir.join(".config.toml.tmp");
```

Replace with a uniquely-named temp file in the same directory. **Preferred approach:** use the `tempfile` crate (already in dev-dependencies for tests — check if it should be promoted to a runtime dep):

```rust
use tempfile::Builder;

let temp_file = Builder::new()
    .prefix(".config.toml.")
    .suffix(".tmp")
    .tempfile_in(&fdemon_dir)
    .map_err(|e| /* wrap in Error */)?;
let temp_path = temp_file.path().to_path_buf();
// write to temp_path, then persist (rename) into place
temp_file.as_file().write_all(full_content.as_bytes())?;
temp_file.persist(&config_path).map_err(|e| e.error)?;
```

**Alternative (no new dependency):** append `std::process::id()` and a monotonic counter to the temp name. Less robust but avoids the dependency change. Implementor's call; document the choice in the completion summary.

### Acceptance Criteria

1. `handle_toggle_hide_implementation` returns `UpdateAction::PersistSettings { ... }` instead of calling `save_settings` synchronously.
2. All settings-save handlers in `settings_handlers.rs` migrate to the same pattern.
3. `grep -rn "save_settings(" crates/fdemon-app/src/handler/` returns zero hits after the migration.
4. `save_settings` in `config/settings.rs` uses a unique temp filename (either via `tempfile` crate or process-id + counter suffix).
5. New tests:
   - `handle_toggle_hide_implementation_returns_persist_settings_action`: invoke the handler, assert the returned `UpdateAction` is `PersistSettings` with the correct project_path + the new hide-impl value.
   - `save_settings_two_concurrent_writes_do_not_collide`: spawn two threads/tasks both calling `save_settings`; assert no panic, no leftover temp file, final file contents are well-formed TOML (the latter check is best-effort).
6. Existing tests on `handle_toggle_hide_implementation` continue to pass (they may need updating since the return value changes from `UpdateResult::none()` to `UpdateResult::action(PersistSettings)` — update them, not delete them).
7. `cargo test --workspace` passes.
8. `cargo clippy --workspace --all-targets -- -D warnings` passes.

### Testing

```rust
#[test]
fn handle_toggle_hide_implementation_returns_persist_settings_action() {
    let mut state = make_test_app_state();
    let initial = state.devtools_view_state.inspector.hide_implementation_widgets;
    let result = handle_toggle_hide_implementation(&mut state);
    let action = match result {
        UpdateResult { action: Some(a), .. } => a,
        _ => panic!("expected PersistSettings action"),
    };
    match action {
        UpdateAction::PersistSettings { settings, project_path } => {
            assert_eq!(settings.devtools.hide_implementation_widgets, !initial);
            assert_eq!(project_path, state.project_path);
        }
        other => panic!("expected PersistSettings, got {:?}", other),
    }
}
```

### Notes

- If the `tempfile` crate is currently only a dev-dependency in `Cargo.toml`, promoting it to a runtime dep is acceptable. It's small, widely-used, and the alternative (hand-rolled temp naming) is more fragile. Document the decision.
- This task is the final consumer of task 02's infrastructure. After landing, no handler should perform synchronous disk I/O.
- The `Message::SettingsPersistFailed` arm in `handler/update.rs` (added as a stub by task 02) may stay as a stub for Phase 1.5 — surfacing the failure to the user is a Phase-2-or-later UI concern. A `tracing::warn!` in the action dispatch arm is sufficient for now.
- Wave: W4. Sequential with task 07 (same file).

---

## Completion Summary

**Status:** Not Started
**Branch:** —

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
