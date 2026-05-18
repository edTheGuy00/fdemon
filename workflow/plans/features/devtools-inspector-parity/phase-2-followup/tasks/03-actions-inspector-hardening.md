## Task: Harden `spawn_fetch_inspector_properties` — error logging + total-timeout

**Objective**: Replace the five `let _ = msg_tx.send(...).await` sites in error paths with proper `tracing::error!` logging (M3), and replace the per-RPC `tokio::time::timeout` calls with a single outer total-timeout wrapper that matches the documented "total budget" contract (M5).

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/inspector/mod.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs` — `Message::DevToolsInspectorProperties*` variants (for the failure-message construction)
- `crates/fdemon-daemon/src/vm_service/extensions/properties.rs` — `parse_properties_response`, `split_widget_and_render_properties` (read for behavior, not modified)
- `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md` — M3, M5 findings

### Details

#### 1. Replace `let _ = msg_tx.send(...).await` with `tracing::error!` logging (M3)

**Affected sites** (review identified 5; verify with `grep -n 'let _ = msg_tx' crates/fdemon-app/src/actions/inspector/mod.rs`):
- ~line 535 — failure path after isolate resolution
- ~line 566 — failure path after `call_extension` initial fetch
- ~line 581 — timeout path on initial fetch
- ~line 602 — failure path on parse error
- ~line 652 — timeout / failure path inside sub-fetch loop

**Pattern (apply uniformly to all sites):**

```rust
// BEFORE
let _ = msg_tx.send(Message::DevToolsInspectorPropertiesFetchFailed {
    session_id,
    node_id: node_id.clone(),
    err: DevToolsError { message: format!("..."), hint: "...".into() },
}).await;

// AFTER
if let Err(send_err) = msg_tx.send(Message::DevToolsInspectorPropertiesFetchFailed {
    session_id,
    node_id: node_id.clone(),
    err: DevToolsError { message: format!("..."), hint: "...".into() },
}).await {
    tracing::error!(
        session_id = %session_id,
        node_id = %node_id,
        error = %send_err,
        "failed to dispatch DevToolsInspectorPropertiesFetchFailed; receiver dropped"
    );
}
```

For timeout sends, change the message variant and log key accordingly:

```rust
if let Err(send_err) = msg_tx.send(Message::DevToolsInspectorPropertiesFetchTimeout {
    session_id,
    node_id: node_id.clone(),
}).await {
    tracing::error!(
        session_id = %session_id,
        node_id = %node_id,
        error = %send_err,
        "failed to dispatch DevToolsInspectorPropertiesFetchTimeout; receiver dropped"
    );
}
```

This mirrors the success-path pattern at ~line 690 in the same file, and the error-handling pattern used in `spawn_fetch_layout_data` at lines 421, 441, 468.

**Verify after change:** `grep -n 'let _ = msg_tx' crates/fdemon-app/src/actions/inspector/mod.rs` returns zero matches.

#### 2. Replace per-RPC timeouts with a single outer total-timeout wrapper (M5)

**Background:**

Current code applies `tokio::time::timeout(PROPERTIES_FETCH_TIMEOUT, ...)` separately around:
- The initial `getProperties` call (~line 554-558)
- Each sub-fetch call inside the loop (~line 637-641)

The doc at lines 31-38 of the same file claims `PROPERTIES_FETCH_TIMEOUT` is the **total time budget**. With N render-object properties, the actual worst-case wall-clock is `(1+N) × 10s`, contradicting the doc.

**Fix:** wrap the entire async block of the spawn task in a single outer timeout. Per cross-cutting constraint #3 in TASKS.md, this matches the doc rather than relaxing it.

**Implementation sketch:**

```rust
pub fn spawn_fetch_inspector_properties(
    msg_tx: mpsc::Sender<Message>,
    handle: VmRequestHandle,
    session_id: Uuid,
    node_id: String,
    object_group: String,
) {
    tokio::spawn(async move {
        let task_result = tokio::time::timeout(
            PROPERTIES_FETCH_TIMEOUT,
            do_fetch_properties(
                handle.clone(),
                node_id.clone(),
                object_group.clone(),
            ),
        )
        .await;

        match task_result {
            Ok(Ok((widget_props, render_props))) => {
                // existing success-path send
                if let Err(send_err) = msg_tx.send(Message::DevToolsInspectorPropertiesFetched {
                    session_id,
                    node_id: node_id.clone(),
                    widget_properties: widget_props,
                    render_properties: render_props,
                }).await {
                    tracing::error!(/* ... */);
                }
            }
            Ok(Err(err)) => {
                // existing failure-path send
                if let Err(send_err) = msg_tx.send(Message::DevToolsInspectorPropertiesFetchFailed {
                    session_id,
                    node_id: node_id.clone(),
                    err,
                }).await {
                    tracing::error!(/* ... */);
                }
            }
            Err(_) => {
                // outer timeout fired
                if let Err(send_err) = msg_tx.send(Message::DevToolsInspectorPropertiesFetchTimeout {
                    session_id,
                    node_id: node_id.clone(),
                }).await {
                    tracing::error!(/* ... */);
                }
            }
        }
    });
}

/// Extracted async body — runs ONE round of `getProperties` for the widget,
/// then sequential sub-`getProperties` for each render-object node. No
/// internal timeout; the outer wrapper enforces total budget.
async fn do_fetch_properties(
    handle: VmRequestHandle,
    node_id: String,
    object_group: String,
) -> Result<(Vec<DiagnosticsNode>, Vec<DiagnosticsNode>), DevToolsError> {
    // 1. Resolve isolate
    let isolate_id = resolve_flutter_ui_isolate(&handle).await
        .map_err(|e| DevToolsError {
            message: format!("isolate resolution failed: {e}"),
            hint: "Press [r] to retry.".into(),
        })?;

    // 2. Initial getProperties call (no internal timeout — outer wrapper handles it)
    let raw = handle.call_extension(
        ext::GET_PROPERTIES,
        Some(args_for_get_properties(&isolate_id, &node_id, &object_group)),
    ).await
        .map_err(|e| DevToolsError {
            message: format!("initial getProperties failed: {e}"),
            hint: "Press [r] to retry.".into(),
        })?;

    let all_props = parse_properties_response(&raw)
        .map_err(|e| DevToolsError {
            message: format!("failed to parse properties response: {e}"),
            hint: "Press [r] to retry.".into(),
        })?;

    let (widget_props, render_props_initial) = split_widget_and_render_properties(all_props);

    // 3. Recursive sub-fetch for each render-object node
    let mut render_props_all = render_props_initial.clone();
    for parent in &render_props_initial {
        let Some(value_id) = parent.value_id.as_ref() else { continue; };
        match handle.call_extension(
            ext::GET_PROPERTIES,
            Some(args_for_get_properties(&isolate_id, value_id, &object_group)),
        ).await {
            Ok(raw_sub) => match parse_properties_response(&raw_sub) {
                Ok(subs) => render_props_all.extend(subs),
                Err(e) => tracing::debug!("sub-fetch parse error for {}: {}", value_id, e),
            },
            Err(e) => tracing::debug!("sub-fetch RPC error for {}: {}", value_id, e),
        }
    }

    Ok((widget_props, render_props_all))
}
```

(The exact field names and helpers may differ — match the existing code style. The structural change is: extract async body, no internal `tokio::time::timeout`, single outer wrapper.)

#### 3. Update the doc comment on `PROPERTIES_FETCH_TIMEOUT`

**Current doc (lines 31-38, approximately):**
```rust
/// Total time budget for fetching all properties (widget + recursive
/// render-object sub-fetches). Applies per-RPC...
const PROPERTIES_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
```

**New doc:**
```rust
/// Total time budget for `spawn_fetch_inspector_properties`, covering:
///   - Initial `ext.flutter.inspector.getProperties` call for the widget node.
///   - All recursive sub-`getProperties` calls (one per render-object property).
///
/// The total elapsed wall-clock time across all these RPCs is bounded by
/// this single value. Individual RPCs do NOT have their own timeouts —
/// the outer `tokio::time::timeout` wrapper in `spawn_fetch_inspector_properties`
/// is the only timeout in the pipeline.
const PROPERTIES_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
```

### Acceptance Criteria

1. `grep -n 'let _ = msg_tx' crates/fdemon-app/src/actions/inspector/mod.rs` returns zero matches.
2. All five former `let _ = msg_tx.send` sites now use `if let Err(e) = ... { tracing::error!(...) }` with structured fields (`session_id`, `node_id`, `error`).
3. `spawn_fetch_inspector_properties` has exactly one `tokio::time::timeout(PROPERTIES_FETCH_TIMEOUT, ...)` call (the outer wrapper). No `tokio::time::timeout` calls inside the inner async body.
4. The doc comment on `PROPERTIES_FETCH_TIMEOUT` describes the total-budget semantics and matches the code.
5. Existing tests for the timeout path (`spawn_properties_emits_timeout_when_first_call_hangs`) and error path (`spawn_properties_emits_failed_when_first_call_errors`) still pass after adaptation. New behavior: a hang during the sub-fetch loop now emits `Timeout` (previously also emitted `Timeout` per the per-RPC timeout, so behavior is similar at the message level — only worst-case wall-clock differs).
6. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Existing tests should continue to pass with minimal adaptation (the `tokio::time::timeout` wrapper site moves, but the externally-observable timeout behavior — a hung mock RPC produces a `Timeout` message — is unchanged).

Add this test to verify the new total-budget semantics:

```rust
#[tokio::test]
async fn spawn_properties_total_budget_is_bounded() {
    use std::time::Instant;
    // Use a mock handle whose call_extension always sleeps just under the
    // timeout, so that 2+ sub-fetches in sequence would exceed PROPERTIES_FETCH_TIMEOUT
    // if the per-RPC pattern was still in place.
    //
    // Difficult to fully exercise without VmRequestHandle test infra; this test
    // documents the contract. Skip if the test harness can't support it,
    // but add a `// TODO:` referencing the test-infra debt item.
    let _ = Instant::now();
}
```

If the test infrastructure cannot inject a controlled-latency mock (the same limitation that blocked 3 of 5 spec'd tests for the original `spawn_fetch_inspector_properties`), document this in the completion summary and add a `// TODO(test-infra):` comment in the file referencing the limitation.

### Notes

- The `tracing::error!` calls use the structured-fields pattern (`%session_id`, `%node_id`, `error = %send_err`). This is the project convention per `docs/CODE_STANDARDS.md` Logging section.
- The choice between "tighten code to match doc" (this task) and "relax doc to match code + cap sub-fetch count" was made per cross-cutting constraint #3 in TASKS.md. Implementor should NOT introduce a `MAX_RENDER_SUB_FETCHES` constant — the outer timeout is the bound.
- The extracted `do_fetch_properties` helper function is internal (private). It does not need its own doc comment beyond the brief one shown above.
- After the refactor, the `actions/inspector/mod.rs` file may be slightly shorter or marginally longer. File-size violation m1 (~907 lines) is NOT in scope for this task — defer to a separate cleanup pass.
- The `tokio::time::timeout` outer wrapper produces an `Err(Elapsed)` on timeout — pattern-match on the outer `Err(_)` to distinguish "timeout" from inner errors. Do NOT swallow the inner result; pass through the success/failure variants normally.

---

## Completion Summary

**Status:** Not Started
