## Task: Add `UpdateAction::PersistSettings` and Engine Dispatch

**Objective**: Introduce a new `UpdateAction::PersistSettings { settings, project_path }` variant that handlers can return to defer `save_settings(...)` off the TEA event-loop thread, following the existing `UpdateAction::AutoSaveConfig` precedent. Adds the message variants needed for success/failure handshake. Consumed by task 08.

**Depends on**: —

**Estimated Time**: 1–1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mod.rs` — add `PersistSettings` variant to `UpdateAction` enum.
- `crates/fdemon-app/src/actions/mod.rs` — add dispatch arm in `handle_action` that spawns a blocking task to call `save_settings`, then sends a `Message` on completion/failure.
- `crates/fdemon-app/src/message.rs` — add `SettingsPersisted` / `SettingsPersistFailed` Message variants.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/settings.rs` — `save_settings(project_path: &Path, settings: &Settings) -> Result<()>` (lines 526–554).
- `crates/fdemon-app/src/actions/mod.rs` — existing `AutoSaveConfig` arm (line 162) as the reference pattern.
- `crates/fdemon-app/src/handler/mod.rs` — existing `UpdateAction::AutoSaveConfig { configs }` variant (line 201).

### Review Items Resolved

- **M3 (infrastructure)** — handler-side switch happens in task 08.

### Details

#### Add the action variant

In `crates/fdemon-app/src/handler/mod.rs`, adjacent to `UpdateAction::AutoSaveConfig`:

```rust
/// Persist the current settings to `.fdemon/config.toml` on a background
/// task. Used to keep the TEA event loop unblocked when a settings toggle
/// (e.g. `Shift+H` in the Inspector tab) flips a persisted boolean.
///
/// Emits [`Message::SettingsPersisted`] on success, or
/// [`Message::SettingsPersistFailed`] on failure.
PersistSettings {
    settings: Settings,
    project_path: PathBuf,
},
```

#### Add the message variants

In `crates/fdemon-app/src/message.rs`, near the other settings-related variants:

```rust
/// Confirmation that a `UpdateAction::PersistSettings` completed successfully.
SettingsPersisted,

/// A `UpdateAction::PersistSettings` write failed.
/// `error` carries the formatted error string for logging/UI surfacing.
SettingsPersistFailed { error: String },
```

These variants must be added to the exhaustive match in `handler/update.rs` as `UpdateResult::none()` stubs initially. Future tasks (or a follow-up) may surface them via toast/log; for now, log at `warn!` level on failure inside the dispatch arm and accept the no-op match.

#### Add the dispatch arm

In `crates/fdemon-app/src/actions/mod.rs`, mirror the `AutoSaveConfig` pattern (around line 162). Use `tokio::task::spawn_blocking` since `save_settings` is synchronous std I/O:

```rust
UpdateAction::PersistSettings { settings, project_path } => {
    let tx = msg_tx.clone();
    tokio::task::spawn_blocking(move || {
        match crate::config::settings::save_settings(&project_path, &settings) {
            Ok(()) => {
                let _ = tx.send(Message::SettingsPersisted);
            }
            Err(e) => {
                let msg = format!("save_settings failed: {e}");
                tracing::warn!("{msg}");
                let _ = tx.send(Message::SettingsPersistFailed { error: msg });
            }
        }
    });
}
```

(Adapt to the actual `handle_action` signature — check whether it gets `msg_tx` directly or via a context struct.)

### Acceptance Criteria

1. `UpdateAction::PersistSettings` exists with the documented fields.
2. `Message::SettingsPersisted` and `Message::SettingsPersistFailed { error }` exist with doc comments.
3. `handle_action` in `actions/mod.rs` dispatches `PersistSettings` via `tokio::task::spawn_blocking` and sends the appropriate `Message` on completion.
4. `handler/update.rs` exhaustive match covers the two new Message variants (stub arms returning `UpdateResult::none()` are acceptable for now; a `tracing::warn!` on failure is fine).
5. `cargo check --workspace --all-targets` passes (compilation only — no handler call sites switch yet).
6. `cargo clippy --workspace --all-targets -- -D warnings` passes.
7. `cargo test --workspace` continues to pass.
8. New unit test (in `actions/mod.rs` or wherever the dispatch logic is testable): `persist_settings_action_sends_persisted_message_on_success`. May be deferred to task 08 if the test rig for `handle_action` isn't easily set up here — note explicitly if so.

### Testing

The action dispatch is async + spawn_blocking, which is awkward to unit-test. Acceptable approaches:
- Build a test harness that constructs an `UpdateAction::PersistSettings` pointing at a `tempfile::TempDir`, drives `handle_action` in a tokio runtime, and asserts the resulting `Message`.
- Or defer the integration test to task 08 once a real handler call site exists.

The variant + dispatch arm presence is verified by `cargo check`.

### Notes

- This task **does not** change any existing call site of `save_settings`. Task 08 owns the migration of `handle_toggle_hide_implementation` and `handler/settings_handlers.rs` to use the new variant.
- The handshake (Persisted/PersistFailed) is intentionally minimal in Phase 1.5. Phase 2+ may add a toast/status-bar surface; the variants exist as the seam.
- Worktree note: this task is parallel-safe with tasks 01, 03, 04 (no shared write files).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a53831211ce0294ab

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Added `UpdateAction::PersistSettings { settings, project_path }` variant adjacent to `AutoSaveConfig` |
| `crates/fdemon-app/src/message.rs` | Added `Message::SettingsPersisted` and `Message::SettingsPersistFailed { error }` variants with doc comments |
| `crates/fdemon-app/src/actions/mod.rs` | Added `PersistSettings` dispatch arm using `tokio::spawn` + `spawn_blocking` pattern; added two `#[tokio::test]` tests |
| `crates/fdemon-app/src/handler/update.rs` | Added `SettingsPersisted` (no-op) and `SettingsPersistFailed` (`warn!` + no-op) match arms |
| `crates/fdemon-tui/src/runner.rs` | Added `PersistSettings { .. }` to the non-runner variants list in `handle_runner_actions` |

### Notable Decisions/Tradeoffs

1. **`spawn_blocking` wrapped in `tokio::spawn`**: Used the same pattern as `ScanInstalledSdks` (async outer task, blocking inner), allowing `.await` on the sender rather than `blocking_send`. This keeps the style consistent with the rest of `handle_action`.

2. **`JoinError` arm**: Added an extra match arm for the `Err(JoinErr)` case when the blocking task panics, sending `SettingsPersistFailed` with the panic message. This is defensive and mirrors best practice for `spawn_blocking`.

3. **Two unit tests instead of one**: Added both success and failure tests for full coverage of the dispatch arm. The failure test uses a non-existent path to trigger an I/O error.

4. **runner.rs exhaustive match**: The `UpdateAction` enum match in `fdemon-tui/src/runner.rs` is exhaustive by design; added `PersistSettings { .. }` to the non-runner variants list as required.

### Testing Performed

- `cargo check --workspace --all-targets` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo test --workspace` — Passed (all test results ok, no failures)
- New tests: `persist_settings_action_sends_persisted_message_on_success` — Passed
- New tests: `persist_settings_action_sends_failed_message_on_error` — Passed

### Risks/Limitations

1. **No call sites yet**: This task intentionally does not wire up any call sites. Task 08 owns the migration of `handle_toggle_hide_implementation` and `handler/settings_handlers.rs` to use `UpdateAction::PersistSettings`.

2. **No UI surface**: `SettingsPersisted` and `SettingsPersistFailed` are no-op stubs in Phase 1.5. Future phases can add toast/status-bar surfacing without further infrastructure changes.
