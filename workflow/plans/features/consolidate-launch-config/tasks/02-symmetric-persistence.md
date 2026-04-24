# Task 02 — Save `last_device` / `last_config` on manual NewSessionDialog launches

**Agent:** implementor
**Plan:** [../PLAN.md](../PLAN.md) (§2, "Who persists what, when" — the asymmetry row)

## Problem (one-liner)

`save_last_selection` currently has exactly one call site: `crates/fdemon-app/src/handler/update.rs:926-930`, fired from the `Message::AutoLaunchResult::Ok` handler. Manual NewSessionDialog launches via `handle_launch` in `crates/fdemon-app/src/handler/new_session/launch_context.rs:404-577` do **not** persist the user's choice.

Effect: the `settings.local.toml` cache only ever reflects the last *auto*-launch. If a user picks a device from the dialog, the cache stays stale — so next time auto-launch fires without a `launch.toml` auto_start config (Task 01's new Priority 2), it uses the old auto-launch choice, not the user's real most-recent selection.

## Desired behavior

When `handle_launch` successfully creates a session from the NewSessionDialog, persist the selected device and config (if any) to `settings.local.toml` via the same `save_last_selection` helper used by the auto-launch path.

## Acceptance criteria

1. After a successful dialog-based launch, `settings.local.toml` contains `last_device = "<selected device id>"` and `last_config = "<selected config name>"` (or the `last_config` field is cleared if the user launched without selecting a config).
2. Persistence failures (disk full, permission denied) do NOT block the launch — log a warning and continue, matching the auto-launch path's error handling.
3. Launches that fail to spawn (e.g. `flutter run` exits immediately) do NOT update the cache. Only successful session creation triggers persistence.
4. Integration test: dispatching `handle_launch` with a mocked filesystem results in a call to `save_last_selection` with the expected args.

## Files modified (write)

- `crates/fdemon-app/src/handler/new_session/launch_context.rs` — add the `save_last_selection` call at the success branch of `handle_launch`.

## Files read (context only)

- `crates/fdemon-app/src/config/settings.rs` — `save_last_selection` signature and error type.
- `crates/fdemon-app/src/handler/update.rs:926-930` — copy the call pattern used by the auto-launch path.

## Implementation notes

- Find the exact success branch in `handle_launch` (around the `SpawnSession` UpdateAction return) and add the save call there.
- `save_last_selection` should receive the device ID and, if present, the config name. If the user launched from the dialog without a config (ad-hoc device selection), pass `None` for the config name — `save_last_selection` already handles this case.
- The persistence is synchronous disk I/O via temp-file-rename (see `settings.rs:797`). For a single small TOML file this is microseconds on modern disks — no need to move it async. But DO wrap the call in a `match` that logs but swallows errors so a disk issue can't prevent the launch.
- If there's a path where `handle_launch` spawns multiple sessions (multi-device select), call `save_last_selection` only for the primary / last-picked device. Follow the existing convention — don't invent a new one.

## Out of scope

- Changing the `save_last_selection` function itself. If you find you need to add a parameter or return value, stop and escalate to the planner — Task 03 also reads this function and we don't want a signature change in Wave 1.
- Adding a "remember my choice" checkbox to the dialog UI. Today the behavior is always-save; a user opt-out is a follow-up.

## Verification

```bash
cargo test -p fdemon-app handler::new_session::
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

Manual smoke: use the dialog to pick iPhone Air in `example/app3` (after removing its `auto_start` from launch.toml so the dialog actually appears). Confirm `example/app3/.fdemon/settings.local.toml` afterward contains the iPhone's simulator UUID.
