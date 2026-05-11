## Task: Add Diagnostic Instrumentation Across Inspector Fetch Path

**Objective**: Add `info!`/`warn!` traces across the entire inspector fetch flow so the next runtime log capture clearly shows isolate selection, readiness polling, RPC call/response, and any silent drops.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/inspector/mod.rs`: Add `info!` at task entry, `info!` after isolate resolved (with isolate id + name), `info!` after readiness poll completes, `info!` before/after RPC call, `info!` before final message dispatch, `warn!` if `msg_tx.send` errors.
- `crates/fdemon-app/src/actions/inspector/widget_tree.rs`: Add `info!` on poll-loop entry (with budget config), `debug!` per attempt, `warn!` on exhaustion, `info!` on success.
- `crates/fdemon-daemon/src/vm_service/client.rs` (line 150-157 `main_isolate_id`): Add `info!` listing every isolate from `getVM` (id, name, system flag) before selection; `info!` after selection.
- `crates/fdemon-app/src/process.rs` (lines 61-90): Add `info!` on hydration path entry; `warn!` if `vm_request_handle` is `None`; `warn!` if `try_send` for fallback message fails.
- `crates/fdemon-app/src/handler/update.rs` (lines 1877-1907 `Message::RequestWidgetTree`): Add `info!` showing whether debounce blocked the request and current `last_fetch_time` value.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`: To read `InspectorState` fields for instrumentation.

### Details

The captured log file `tmp/fdemon-1778501860563-42351.log` shows zero inspector-related INFO entries during a confirmed DevTools session. This task changes that. Every meaningful branch in the fetch flow must produce an INFO log so we can trace which hypothesis fires.

Example trace points (target shape):

```rust
// actions/inspector/mod.rs in spawn_fetch_widget_tree task entry
info!(
    session_id = %session_id,
    tree_max_depth = ?tree_max_depth,
    fetch_timeout_secs = fetch_timeout_secs,
    "Inspector: fetch_widget_tree task started"
);

// after isolate resolution
info!(
    session_id = %session_id,
    isolate_id = %isolate_id,
    "Inspector: resolved isolate"
);

// vm_service/client.rs in main_isolate_id
info!(
    isolates_count = isolates.len(),
    isolates = ?isolates.iter().map(|i| (&i.id, &i.name, i.is_system_isolate)).collect::<Vec<_>>(),
    "VM Service: listing isolates from getVM"
);
```

Use `tracing` macros only — never `println!`/`eprintln!` (TUI owns stdout).

### Acceptance Criteria

1. Re-running fdemon (`cargo run -- example/app2`), entering DevTools, observing the loading state for ≥ 5 seconds, pressing `r` twice, and quitting produces a log with at least these entries:
   - `Inspector: fetch_widget_tree task started`
   - `VM Service: listing isolates from getVM`
   - Either `Inspector: resolved isolate` or `VM Service: no non-system isolates available`
   - At least one readiness-poll-related INFO or WARN line
   - Either `Inspector: RPC call X completed` or a `WARN` about timeout/error
   - For the `r` retries: either `Inspector: refresh requested` or `Inspector: RequestWidgetTree debounced`
2. No new `println!` or `eprintln!` introduced.
3. All log keys use structured `tracing` fields (no string interpolation).
4. `cargo check --workspace --all-targets` passes.

### Testing

- No new tests required — this is observability scaffolding.
- Manual verification: capture a new log and visually inspect that every flow branch has a trace.

### Notes

- Be cautious about logging the full widget-tree payload — it may be huge. Only log size / root type / child count.
- Per CLAUDE.md, log files live in the system temp directory.
- After the issue is fixed and verified in production, a follow-up task may downgrade these to `debug!` to reduce noise. That's out of scope here.
