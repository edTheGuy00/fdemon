# Task 03 — Immediate Timeline Fetch on Unpause

**Status:** Not Started
**Wave:** 1
**Agent:** implementor
**Estimated Effort:** 1–2 hours
**Depends On:** —

## Problem

`spawn_timeline_polling` (`crates/fdemon-app/src/actions/performance.rs`) polls at 1 Hz. When the user enters the Performance panel, the task is unpaused via `timeline_pause_tx.send(false)`. But the polling loop always waits for the next 1-Hz tick before fetching, so users see "Waiting for timeline events…" for ~1 second on every panel entry.

The allocation-polling task (`spawn_allocation_polling`, same file, lines 328–385) already does the right thing: it uses `tokio::select!` and on `pause_rx.changed -> false` immediately runs one fetch before entering the tick loop. Mirror that pattern for timeline polling.

## Files (Write)

- `crates/fdemon-app/src/actions/performance.rs`

## Files (Read)

- `crates/fdemon-daemon/src/vm_service/timeline.rs` — `fetch_timeline_chunk` signature unchanged
- `crates/fdemon-daemon/src/vm_service/request_api.rs` — `VmRequestApi` trait (introduced in T11 of Phase 3-followup)

## Approach Hints

### Current structure (simplified)

```rust
pub async fn spawn_timeline_polling(...) {
    let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));
    let mut last_poll_micros = seed_timeline_watermark(&handle).await;
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => break,
            _ = pause_rx.changed() => {
                if *pause_rx.borrow() {
                    // paused — continue and wait for next change
                }
            }
            _ = interval.tick() => {
                if *pause_rx.borrow() { continue; }
                // ... fetch and dispatch ...
            }
        }
    }
}
```

The problem: the `pause_rx.changed()` arm has no fetch logic. After resume, the loop continues and waits up to 1 s for the next `interval.tick()`.

### Proposed structure

Extract the fetch-and-dispatch into a small helper, then call it both on tick and on resume:

```rust
async fn run_one_fetch_cycle<H: VmRequestApi>(
    handle: &H,
    msg_tx: &MessageSender,
    session_id: SessionId,
    last_poll_micros: &mut u64,
) -> Result<(), VmServiceError> {
    let chunk = fetch_timeline_chunk(handle, *last_poll_micros).await?;
    let now_post_fetch = get_vm_timeline_micros(handle).await
        .unwrap_or_else(|_| (*last_poll_micros).saturating_add(1));
    *last_poll_micros = now_post_fetch;
    if !chunk.is_empty() {
        msg_tx.send(Message::TimelineEventsBatchReceived {
            session_id,
            events: chunk,
            metadata: vec![], // T04 will wire up metadata
        }).await?;
    }
    Ok(())
}

// Main loop
loop {
    tokio::select! {
        _ = shutdown_rx.changed() => break,
        Ok(()) = pause_rx.changed() => {
            if !*pause_rx.borrow() {
                // Just resumed — fetch immediately
                tracing::debug!("timeline immediate fetch on unpause");
                if let Err(e) = run_one_fetch_cycle(&handle, &msg_tx, session_id, &mut last_poll_micros).await {
                    tracing::debug!("timeline immediate fetch failed: {e}");
                }
            }
        }
        _ = interval.tick() => {
            if *pause_rx.borrow() { continue; }
            if let Err(e) = run_one_fetch_cycle(&handle, &msg_tx, session_id, &mut last_poll_micros).await {
                tracing::debug!("timeline tick fetch failed: {e}");
            }
        }
    }
}
```

### Test plan

Reuse the `VmRequestApi` mock from T11 of Phase 3-followup (`MockVmRequestApi`, `MockResponse`, `MockCall`). New tests:

- `test_timeline_immediate_fetch_on_unpause` — start paused, unpause, assert at least one `getVMTimeline` call within ~50 ms (well under the 1 s tick).
- `test_timeline_immediate_fetch_failure_logs_and_continues` — mock `getVMTimeline` to return `Err`; assert task does not crash; next tick still happens.

## Acceptance Criteria

1. **Immediate fetch on unpause** — When `pause_rx.changed -> false` fires, exactly one `fetch_timeline_chunk` call happens before waiting for the next `interval.tick()`. Verified by mock call count + timing assertion.
2. **No double-fetch on rapid pause/unpause** — Toggling pause `false → true → false` within < 100 ms triggers exactly one immediate fetch on the final resume (not two stacked).
3. **Backward compat** — Existing tests `test_timeline_pause_stops_rpcs`, `test_timeline_resume_restarts`, `test_timeline_shutdown_exits_within_100ms` still pass.
4. **Pause state honored on tick** — When paused, the tick arm continues to skip fetches. (Sanity check; should be unchanged.)
5. **Quality gate** — `cargo fmt --all -- --check`, `cargo check -p fdemon-app --all-targets`, `cargo test -p fdemon-app`, `cargo clippy -p fdemon-app --all-targets -- -D warnings` all pass.
6. **Cold-start placeholder window** — When manually tested (`cargo run -- <flutter-app>`), entering the Performance panel surfaces timeline events within ~150 ms. Tail log shows `"timeline immediate fetch on unpause"` debug entry.

## Notes

- The added `metadata: vec![]` field in the `TimelineEventsBatchReceived` message is a forward-compatible placeholder; T04 will populate it from `parse_vm_timeline_with_metadata`. If T04 hasn't landed yet, leave the field empty — the handler just won't have thread names to label rows with.
- If the message variant doesn't yet carry `metadata` (it currently doesn't — that's a T04 change), then this task can ship with `events`-only and T04 extends the variant. Confirm at implementation time which task touches `message.rs` first.
- The `tokio::time::pause()` testing pattern from T11's mock setup is the right approach for the timing assertions.
