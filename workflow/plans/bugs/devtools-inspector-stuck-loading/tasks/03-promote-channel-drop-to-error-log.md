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

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ade93b5d77da4c5ef

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/process.rs` | Promoted all three hydration-fallback `try_send` calls (`FetchWidgetTree`, `FetchLayoutData`, `FetchHttpRequestDetail`) from silent `let _ =` / `warn!` to `match` with `debug!` on success and `error!(error = ?e, ...)` on channel failure |
| `crates/fdemon-app/src/actions/inspector/mod.rs` | Promoted terminal `msg_tx.send(msg).await` error in `spawn_fetch_widget_tree` from `warn!` to `error!(error = ?e, ...)`; promoted all `let _ = msg_tx.send(...).await` calls in `spawn_toggle_overlay` and `spawn_fetch_layout_data` (5 call sites: isolate-id failure, timeout, extension-call failure, parse failure, success path) to `if let Err(e) = ... { error!(error = ?e, ...) }` |
| `crates/fdemon-app/src/handler/update.rs` | Reformatted by `cargo fmt` (pre-existing style drift, no logic change) |

### Notable Decisions/Tradeoffs

1. **`FetchWidgetTree` fallback upgraded from `warn!` to `error!`**: The task specified `error!` throughout. The pre-existing `warn!` in `process.rs` lines 95-99 was also a `try_send` failure path (not the send itself), so it was replaced with the `match`+`debug!/error!` pattern as specified.
2. **All `spawn_fetch_layout_data` send paths covered**: There were 5 distinct `let _ = msg_tx.send(...).await` sites in that function (isolate failure, timeout, extension failure, parse failure, success). All are now instrumented with `error!` on failure.
3. **No new unit tests added**: Task explicitly stated none required for this narrow log-promotion change.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check -p fdemon-app` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (no warnings)
- `cargo test -p fdemon-app --lib` — Passed (2168 tests, 0 failed)

### Risks/Limitations

1. **`spawn_toggle_overlay` success path coverage**: The success `send` at the end of `spawn_toggle_overlay` was also a silent `let _ =` — it is now instrumented. This matches acceptance criterion 1 ("every send in the inspector dispatch path").
2. **`error!` severity on success-path send failures**: A channel-closed error on the success path (e.g., `LayoutDataFetched`) is also `error!`-level — appropriate since it means the UI will stay in a loading state with no recovery path.
