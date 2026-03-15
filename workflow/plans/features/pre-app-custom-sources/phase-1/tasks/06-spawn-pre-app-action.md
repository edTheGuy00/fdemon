## Task: `spawn_pre_app_sources()` Action Implementation

**Objective**: Implement the async action that spawns pre-app custom sources, runs their readiness checks, sends progress messages, and fires `PreAppSourcesReady` when all sources are ready or timed out.

**Depends on**: Task 03 (message + action types), Task 04 (ready check execution), Task 05 (launch flow)

### Scope

- `crates/fdemon-app/src/actions/native_logs.rs`: Add `spawn_pre_app_sources()` function
- `crates/fdemon-app/src/actions/mod.rs`: Wire up `SpawnPreAppSources` dispatch to the new function

### Details

#### 1. Wire Up Action Dispatch

In `actions/mod.rs`, replace the stub from Task 03:

```rust
UpdateAction::SpawnPreAppSources {
    session_id,
    device,
    config,
    settings,
    project_path,
} => {
    native_logs::spawn_pre_app_sources(
        session_id,
        device,
        config,
        &settings,
        &project_path,
        &msg_tx,
    );
}
```

#### 2. Implement `spawn_pre_app_sources()`

Add to `crates/fdemon-app/src/actions/native_logs.rs`:

```rust
/// Spawn pre-app custom sources and run their readiness checks.
///
/// For each source with `start_before_app = true`:
/// 1. Spawns the `CustomLogCapture` process immediately (logs flow to the session in real time)
/// 2. Sends `CustomSourceStarted` so handles are tracked on `SessionHandle`
/// 3. Collects readiness futures for sources that have a `ready_check`
///
/// After all sources are spawned, waits for all readiness checks to complete
/// (each with its own timeout). Sends progress messages throughout.
/// Finally sends `PreAppSourcesReady` to release the Flutter launch gate.
pub fn spawn_pre_app_sources(
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
    settings: &NativeLogsSettings,
    project_path: &std::path::Path,
    msg_tx: &mpsc::Sender<Message>,
) {
    let pre_app_sources: Vec<_> = settings
        .custom_sources
        .iter()
        .filter(|s| s.start_before_app)
        .cloned()
        .collect();

    if pre_app_sources.is_empty() {
        // No pre-app sources — send ready immediately
        let tx = msg_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(Message::PreAppSourcesReady {
                session_id,
                device,
                config,
            }).await;
        });
        return;
    }

    let settings = settings.clone();
    let project_path = project_path.to_path_buf();
    let msg_tx = msg_tx.clone();

    tokio::spawn(async move {
        let mut readiness_futures = Vec::new();

        for source_config in &pre_app_sources {
            // Validate
            if let Err(e) = source_config.validate() {
                tracing::warn!(
                    "Skipping invalid pre-app source for session {}: {}",
                    session_id, e
                );
                continue;
            }

            // Send progress: starting
            let _ = msg_tx.send(Message::PreAppSourceProgress {
                session_id,
                message: format!("Starting pre-app source '{}'...", source_config.name),
            }).await;

            // Build daemon-layer config
            let working_dir = source_config.working_dir.clone()
                .or_else(|| project_path.to_str().map(|s| s.to_string()));

            let mut daemon_config = DaemonCustomSourceConfig {
                name: source_config.name.clone(),
                command: source_config.command.clone(),
                args: source_config.args.clone(),
                format: source_config.format,
                working_dir,
                env: source_config.env.clone(),
                exclude_tags: settings.exclude_tags.clone(),
                include_tags: settings.include_tags.clone(),
                ready_pattern: None, // Set below if stdout check
            };

            // Prepare stdout readiness channel if needed
            let ready_rx = if let Some(ReadyCheck::Stdout { ref pattern, .. }) = source_config.ready_check {
                daemon_config.ready_pattern = Some(pattern.clone());
                let (tx, rx) = tokio::sync::oneshot::channel();
                // ready_tx will be passed to spawn_with_readiness
                Some((tx, rx))
            } else {
                None
            };

            let (ready_tx_opt, ready_rx_opt) = match ready_rx {
                Some((tx, rx)) => (Some(tx), Some(rx)),
                None => (None, None),
            };

            // Spawn the capture process
            let capture = create_custom_log_capture(daemon_config);

            // Use spawn_with_readiness if we have a ready_tx, otherwise regular spawn
            let native_handle = if ready_tx_opt.is_some() {
                // Downcast to CustomLogCapture to call spawn_with_readiness
                // Since create_custom_log_capture returns Box<dyn NativeLogCapture>,
                // we need to restructure slightly. Instead, create CustomLogCapture directly.
                drop(capture); // Don't use the boxed version
                let daemon_config_for_spawn = /* reconstruct or clone */;
                let custom_capture = CustomLogCapture::new(daemon_config_for_spawn);
                custom_capture.spawn_with_readiness(ready_tx_opt)
            } else {
                capture.spawn()
            };
```

**Important implementation note:** The above pseudocode shows a challenge — `create_custom_log_capture()` returns `Box<dyn NativeLogCapture>`, but we need `CustomLogCapture` directly to call `spawn_with_readiness()`. Solution: **construct `CustomLogCapture` directly** instead of using the factory for pre-app sources:

```rust
            // Construct CustomLogCapture directly (not via factory) to access
            // spawn_with_readiness()
            let custom_capture = fdemon_daemon::native_logs::custom::CustomLogCapture::new(
                daemon_config
            );
            let native_handle = match custom_capture.spawn_with_readiness(ready_tx_opt) {
                Some(h) => h,
                None => continue,
            };
```

Then continue with the forwarding task (same pattern as existing `spawn_custom_sources`):

```rust
            // Wrap handles for the CustomSourceStarted message
            let shutdown_tx = std::sync::Arc::new(native_handle.shutdown_tx);
            let task_handle = std::sync::Arc::new(
                tokio::sync::Mutex::new(Some(native_handle.task_handle))
            );

            let source_name = source_config.name.clone();
            let fwd_tx = msg_tx.clone();
            let shutdown_tx_clone = shutdown_tx.clone();
            let task_handle_clone = task_handle.clone();

            // Spawn forwarding task (same as existing spawn_custom_sources pattern)
            tokio::spawn({
                let source_name = source_name.clone();
                let fwd_tx = fwd_tx.clone();
                async move {
                    // Send CustomSourceStarted so handles are tracked
                    let _ = fwd_tx.send(Message::CustomSourceStarted {
                        session_id,
                        name: source_name.clone(),
                        shutdown_tx: shutdown_tx_clone,
                        task_handle: task_handle_clone,
                    }).await;

                    // Forward events
                    let mut event_rx = native_handle.event_rx;
                    while let Some(event) = event_rx.recv().await {
                        if fwd_tx.send(Message::NativeLog {
                            session_id,
                            event,
                        }).await.is_err() {
                            break;
                        }
                    }

                    let _ = fwd_tx.send(Message::CustomSourceStopped {
                        session_id,
                        name: source_name,
                    }).await;
                }
            });

            // Collect readiness future if this source has a ready_check
            if let Some(ref check) = source_config.ready_check {
                let check = check.clone();
                let name = source_config.name.clone();
                let progress_tx = msg_tx.clone();

                readiness_futures.push(async move {
                    // Send progress: waiting for readiness
                    let check_desc = describe_ready_check(&check);
                    let _ = progress_tx.send(Message::PreAppSourceProgress {
                        session_id,
                        message: format!(
                            "Waiting for '{}' readiness ({})...",
                            name, check_desc
                        ),
                    }).await;

                    let result = super::ready_check::run_ready_check(
                        &check, &name, ready_rx_opt,
                    ).await;

                    match &result {
                        ReadyCheckResult::Ready(elapsed) => {
                            let _ = progress_tx.send(Message::PreAppSourceProgress {
                                session_id,
                                message: format!(
                                    "Pre-app source '{}' ready ({:.1}s)",
                                    name, elapsed.as_secs_f64()
                                ),
                            }).await;
                        }
                        ReadyCheckResult::TimedOut(_) => {
                            let _ = progress_tx.send(Message::PreAppSourceTimedOut {
                                session_id,
                                source_name: name.clone(),
                            }).await;
                        }
                        ReadyCheckResult::Failed(reason) => {
                            let _ = progress_tx.send(Message::PreAppSourceProgress {
                                session_id,
                                message: format!(
                                    "Pre-app source '{}' readiness check failed: {}",
                                    name, reason
                                ),
                            }).await;
                        }
                    }

                    (name, result)
                });
            }
        }

        // Wait for all readiness checks to complete (each has its own timeout)
        let results = futures::future::join_all(readiness_futures).await;

        // Log summary
        if !results.is_empty() {
            let ready_count = results.iter().filter(|(_, r)| r.is_ready()).count();
            let _ = msg_tx.send(Message::PreAppSourceProgress {
                session_id,
                message: format!(
                    "Pre-app sources: {}/{} ready. Launching Flutter...",
                    ready_count, results.len()
                ),
            }).await;
        }

        // Release the gate
        let _ = msg_tx.send(Message::PreAppSourcesReady {
            session_id,
            device,
            config,
        }).await;
    });
}

/// Human-readable description of a ready check for progress messages.
fn describe_ready_check(check: &ReadyCheck) -> String {
    match check {
        ReadyCheck::Http { url, .. } => format!("http: {}", url),
        ReadyCheck::Tcp { host, port, .. } => format!("tcp: {}:{}", host, port),
        ReadyCheck::Command { command, .. } => format!("command: {}", command),
        ReadyCheck::Stdout { pattern, .. } => format!("stdout: /{}/", pattern),
        ReadyCheck::Delay { seconds } => format!("delay: {}s", seconds),
    }
}
```

#### 3. Import Requirements

Add to `native_logs.rs` imports:

```rust
use crate::config::ReadyCheck;
use crate::actions::ready_check::ReadyCheckResult;
use fdemon_daemon::native_logs::custom::CustomLogCapture;
```

Check if `futures` crate is available for `join_all`. If not, use `tokio::join!` or a manual approach. Alternatively, since we're collecting into a `Vec` of futures, `futures::future::join_all` is the cleanest API. The `futures` crate may already be in the dependency tree.

### Acceptance Criteria

1. Pre-app sources are spawned and their stdout is visible in the session log immediately
2. `CustomSourceStarted` messages are sent so handles are tracked on `SessionHandle`
3. Readiness checks run concurrently (not sequentially) for all pre-app sources
4. Progress messages are sent at each stage: starting, waiting, ready/timed out/failed
5. `PreAppSourcesReady` is sent after ALL readiness checks complete (or individually time out)
6. Sources without `ready_check` are spawned but don't block the gate
7. `PreAppSourceTimedOut` is sent for individual timeouts (informational)
8. If the only pre-app sources are fire-and-forget (no `ready_check`), `PreAppSourcesReady` fires immediately after spawning
9. `cargo check -p fdemon-app` passes
10. `cargo test -p fdemon-app` passes

### Testing

This function is heavily async and depends on process spawning, so integration-style tests are most appropriate. Unit tests should cover:

```rust
#[test]
fn test_describe_ready_check_http() {
    let check = ReadyCheck::Http {
        url: "http://localhost:8080/health".to_string(),
        interval_ms: 500,
        timeout_s: 30,
    };
    assert_eq!(describe_ready_check(&check), "http: http://localhost:8080/health");
}

#[test]
fn test_describe_ready_check_tcp() {
    let check = ReadyCheck::Tcp {
        host: "localhost".to_string(),
        port: 3000,
        interval_ms: 500,
        timeout_s: 30,
    };
    assert_eq!(describe_ready_check(&check), "tcp: localhost:3000");
}

// ... other variants
```

For the spawn orchestration, consider a test with a mock `echo` command and a TCP check against a local listener (similar to Task 04 tests).

### Notes

- **`futures` crate**: Check if it's in `fdemon-app/Cargo.toml`. If not, either add it (it's lightweight) or use a manual `Vec<JoinHandle>` approach with `tokio::spawn` per check and `join_all` from tokio.
- **`CustomLogCapture` visibility**: The `spawn_with_readiness` method needs to be accessible from `fdemon-app`. Since `fdemon-daemon` is a dependency of `fdemon-app`, this should work as long as `CustomLogCapture` and `spawn_with_readiness` are `pub`. Verify the import path.
- **`ready_rx` ownership**: The `oneshot::Receiver` is consumed by `run_ready_check()`. Since each source has at most one readiness check, there's no sharing issue. But the `ready_rx` must be moved into the readiness future, not the forwarding task — so the variable must be captured correctly in the async block.
- **Forwarding task pattern**: Reuse the exact pattern from the existing `spawn_custom_sources()` (lines 289-330 of `native_logs.rs`). The `CustomSourceStarted` / `NativeLog` / `CustomSourceStopped` message lifecycle is identical.
- **Error handling**: If `validate()` fails, skip the source with a warning. If `spawn()` returns `None`, skip. These are the same patterns as the existing `spawn_custom_sources()`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/native_logs.rs` | Added `spawn_pre_app_sources()`, `describe_ready_check()`, and 5 tests; updated imports to include `ReadyCheck`, `CustomLogCapture`, `ReadyCheckResult` |
| `crates/fdemon-app/src/actions/mod.rs` | Replaced stub `SpawnPreAppSources` dispatch with real call to `native_logs::spawn_pre_app_sources()` |
| `crates/fdemon-app/src/actions/ready_check.rs` | Removed `#![allow(dead_code)]` (module is now actively used); updated module doc comment |

### Notable Decisions/Tradeoffs

1. **`JoinSet` instead of `futures::future::join_all`**: `futures` is not in `fdemon-app`'s dependencies (only `futures-util` is in the workspace, used by `fdemon-daemon`). Used `tokio::task::JoinSet` to run readiness checks concurrently without adding a new dependency. The semantics are equivalent for this use case.

2. **`CustomLogCapture::new()` instead of factory function**: The task spec required calling `spawn_with_readiness()`, which is only on the concrete `CustomLogCapture` type (not `Box<dyn NativeLogCapture>`). Constructing `CustomLogCapture` directly is correct — the factory `create_custom_log_capture()` is still used by `spawn_custom_sources()` where readiness signaling is not needed.

3. **`TimedOut(Duration)` field actually used**: Removing `#![allow(dead_code)]` from `ready_check.rs` exposed a `dead_code` warning because the `Duration` payload inside `TimedOut` was never read. Fixed by logging the elapsed time at `warn!` level in the `TimedOut` match arm — this is useful operational information (not just suppressing the lint).

4. **Sources without `ready_check` don't block the gate**: Implemented per spec — fire-and-forget sources are spawned but their forwarding tasks run independently. Only sources with a `ready_check` contribute to `sources_with_checks` and block `PreAppSourcesReady`. If zero sources have a `ready_check`, `PreAppSourcesReady` fires after all spawns complete synchronously.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1637 tests, +5 new `describe_ready_check` tests)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **Forwarding task races `CustomSourceStarted`**: The forwarding task sends `CustomSourceStarted` at start. If the engine shuts down before receiving it, the task exits cleanly. This is identical to the existing `spawn_custom_sources()` pattern so the risk is already understood.

2. **`ready_rx` capture in readiness future**: The `oneshot::Receiver` is moved into the `JoinSet` future (via `spawn`), not the forwarding task. This is correct: both tasks need access to the same `native_handle`, so `event_rx` goes to the forwarding task and `ready_rx` goes to the readiness future. The ownership is properly split before either task is spawned.
