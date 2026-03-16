## Task: Add Shared Source Message Variants

**Objective**: Add `Message::SharedSourceLog`, `Message::SharedSourceStarted`, and `Message::SharedSourceStopped` variants for the TEA message bus so shared sources can communicate through the standard event loop.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/message.rs`: Add three new `Message` variants
- `crates/fdemon-app/src/handler/mod.rs`: Add placeholder match arms (if needed for exhaustive match)

### Details

#### 1. `SharedSourceLog`

```rust
/// Log event from a shared custom source (not bound to a specific session).
///
/// The TEA handler broadcasts this to all active sessions, applying per-session
/// tag filtering. Contrast with `NativeLog` which targets a single session.
SharedSourceLog {
    /// The native log event (tag = source name, level, message).
    event: fdemon_daemon::NativeLogEvent,
},
```

#### 2. `SharedSourceStarted`

```rust
/// A shared custom source process has been spawned successfully.
///
/// The TEA handler stores the handle on `AppState.shared_source_handles`
/// (not per-session). Sent by the forwarding task in `spawn_pre_app_sources`
/// or `spawn_custom_sources` for sources with `shared = true`.
SharedSourceStarted {
    /// Source name (matches config `name` field).
    name: String,
    /// Shutdown sender for graceful stop.
    shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
    /// Task handle for abort fallback. Wrapped in `Arc<Mutex<Option<>>>>`
    /// so the spawning task can deposit the handle after `tokio::spawn`.
    task_handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Whether this source was started before the Flutter app.
    start_before_app: bool,
},
```

#### 3. `SharedSourceStopped`

```rust
/// A shared custom source process has exited.
///
/// The TEA handler removes the handle from `AppState.shared_source_handles`
/// and logs a warning to all active sessions.
SharedSourceStopped {
    /// Source name.
    name: String,
},
```

### Acceptance Criteria

1. Three new `Message` variants defined with documentation
2. `Message` enum still derives/implements `Clone` (all new fields are `Clone`-compatible)
3. All existing match arms compile (add placeholder `_ =>` or explicit arms as needed)
4. All existing tests pass

### Testing

- No behavioral tests needed — this is a type definition task
- Compilation is the test (exhaustive matches will catch missing arms)

### Notes

- `SharedSourceStarted` uses the same `Arc<Mutex<Option<JoinHandle>>>` pattern as `CustomSourceStarted` for the task handle deposit — this is the established pattern in the codebase
- `SharedSourceLog` intentionally omits `session_id` — the handler decides which sessions receive the log

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `SharedSourceLog`, `SharedSourceStarted`, `SharedSourceStopped` variants to `Message` enum |
| `crates/fdemon-app/src/handler/update.rs` | Added placeholder match arms for three new variants |
| `crates/fdemon-app/src/session/handle.rs` | Added `#[derive(Debug)]` to `SharedSourceHandle` (pre-existing broken state from prior task) |
| `crates/fdemon-app/src/actions/native_logs.rs` | Added `shared: false` to test helper `make_source_config` (pre-existing missing field from prior task) |
| `crates/fdemon-app/src/handler/tests.rs` | Added `shared: false` to two `CustomSourceConfig` struct literals (pre-existing missing field from prior task) |
| `crates/fdemon-app/src/state.rs` | Removed spurious `mut` on `rx` in test (pre-existing warning from prior task) |

### Notable Decisions/Tradeoffs

1. **Placeholder match arms in `update.rs`**: Added `Message::SharedSourceLog { .. } => UpdateResult::none()` etc. rather than `_ =>` to keep exhaustive matching explicit. Full implementations come in later tasks.
2. **Pre-existing breakage fixed**: The `config/types.rs` `shared` field and `state.rs` `shared_source_handles` were already in place from prior uncommitted task work but with compile errors. Fixed `#[derive(Debug)]` on `SharedSourceHandle`, `shared: false` in three struct literals, and an unused `mut` warning. These were blocking `cargo test` but not `cargo check`.
3. **`SharedTaskHandle` type alias reuse**: Used the existing `SharedTaskHandle` type alias in `message.rs` for the `task_handle` field of `SharedSourceStarted`, consistent with `CustomSourceStarted` and `NativeLogCaptureStarted`.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1657 tests, 0 failed, 4 ignored)
- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Placeholder handlers**: The three new match arms return `UpdateResult::none()` — no actual behavior until the next tasks implement the handler logic.
