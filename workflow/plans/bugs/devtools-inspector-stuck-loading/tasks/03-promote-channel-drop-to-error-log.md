## Task: Promote Silent Channel Drops to Error Log

**Objective**: Stop silently swallowing message-channel send failures in `process.rs`. Surface them via `error!` log and (where feasible) recover with a synthetic failure message so the UI doesn't stay stuck on "Loading widget tree" forever.

**Depends on**: 01-add-diagnostic-instrumentation

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/process.rs` (lines 61-90, `hydrate_fetch_widget_tree` and the surrounding action-dispatch logic): Replace any silent `let _ = try_send(...)` / dropped-receiver branches with `error!` traces. Where possible, also send a fallback `WidgetTreeFetchFailed { error: "Internal channel closed" }` so the UI clears the `loading` flag.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/actions/inspector/mod.rs:123`: The `msg_tx.send(msg).await` call inside the spawn task — if its error path is silent, lift the same fix there too.

### Details

The hydration path silently discards actions when `vm_request_handle` is `None`:

```rust
// Today — silent drop
if hydrate_result.is_none() {
    let _ = msg_tx.try_send(Message::WidgetTreeFetchFailed { /* ... */ });
}
```

If `try_send` fails (channel full, receiver dropped), `loading` never clears. Promote to:

```rust
match msg_tx.try_send(Message::WidgetTreeFetchFailed { /* ... */ }) {
    Ok(()) => debug!("Inspector: dispatched fallback WidgetTreeFetchFailed after hydration drop"),
    Err(e) => error!(error = ?e, "Inspector: failed to dispatch fallback WidgetTreeFetchFailed — inspector may stay stuck"),
}
```

The spawn task at `actions/inspector/mod.rs:123` (`msg_tx.send(msg).await`) should also have its error path lifted:

```rust
if let Err(e) = msg_tx.send(msg).await {
    error!(error = ?e, "Inspector: failed to send terminal message — receiver dropped");
}
```

### Acceptance Criteria

1. Every `try_send` / `send().await` in the inspector dispatch path logs an `error!` on failure (no silent `let _ = ...`).
2. Where the dropped message would have cleared a loading flag, attempt a fallback path or document in the log why one isn't feasible.
3. `cargo clippy --workspace --all-targets -- -D warnings` passes — no `unused_must_use` warnings from these calls.

### Testing

- No new unit tests required; this is a narrow log-promotion change.
- Manual: induce a channel-close scenario in development by adding a temporary `panic!` after `engine.shutdown()` and trigger a widget-tree fetch right before shutdown; verify the error log line appears.

### Notes

- Do not introduce error chains that ignore the underlying `mpsc::error::SendError`/`TrySendError` cause. Always include `error = ?e` in the trace.
- This task is small but high-value: it's the last-resort observability for the "task spawned but message lost" hypothesis.
