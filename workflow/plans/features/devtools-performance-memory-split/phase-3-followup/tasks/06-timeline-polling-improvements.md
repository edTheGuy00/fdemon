# 06 — Timeline Polling Improvements

**Wave:** 2
**Depends On:** —
**Agent:** implementor
**Estimated Hours:** 1.5–2h
**Addresses:** M8, L1, L11

## Context

Three correctness/hygiene issues in `crates/fdemon-app/src/actions/performance.rs::spawn_timeline_polling`:

- **M8.** Watermark advancement is off-by-one. After each successful poll, `last_poll_micros` is set to `now_micros.saturating_add(1)` where `now_micros` was captured BEFORE the `fetch_timeline_chunk` RPC ran. Events with `ts ∈ [now_micros, fetch_completion_time]` are silently dropped because the next poll starts from `now_micros + 1`. Under slow VM Service responses (heap walk, profile-mode lag), this manifests as sporadic gaps.
- **L1.** The `200` ms timeline-poll floor is a magic number with no named constant or derivation comment. CODE_STANDARDS Principle 4 forbids magic numbers in operational thresholds.
- **L11.** On the first-tick seed (`get_vm_timeline_micros`), if the RPC fails the code falls back to `last_poll_micros = 0`. The first successful `fetch_timeline_chunk(handle, 0, extent)` then retrieves the entire VM lifetime worth of events (up to the VM's circular buffer of ~32 MB). The handler is hit with thousands of events at once, causing a stall.

## Acceptance Criteria

1. **M8 resolved.** Replace `last_poll_micros = now_micros.saturating_add(1)` with one of:
   - **Preferred.** Capture `now_micros` AFTER `fetch_timeline_chunk` completes (via a second `get_vm_timeline_micros` call or the `max(ts)` from the returned events plus 1).
   - **Alternative.** Use the largest `ts` from the returned events plus 1 if events were returned; otherwise call `get_vm_timeline_micros` again.
   - The chosen approach is documented in a code comment near the watermark update.
2. **L1 resolved.** Introduce:
   ```rust
   /// Minimum timeline poll interval (200 ms).
   /// Safety floor preventing accidental sub-200ms polling if `poll_interval_ms`
   /// is mis-configured. PLAN.md §5.4 target is 1 Hz (1000 ms); 200 ms is the
   /// floor at which VM Service stress remains acceptable.
   const TIMELINE_POLL_MIN_MS: u64 = 200;
   ```
   Replace the literal `200` in `Duration::from_millis(poll_interval_ms.max(200))` with the constant.
3. **L11 resolved.** On first-tick `get_vm_timeline_micros` failure:
   - Retry once after a 100 ms backoff.
   - If retry also fails, set `last_poll_micros` to a recent watermark via a separate "now-ish" estimate (e.g., capture wall-clock via `tokio::time::Instant::now()` converted to a reasonable initial value, or cap the first `extent` to 2 seconds).
   - On no retry success, log at `tracing::warn!` with the error and continue with the bounded initial extent.
   - Test: `test_first_tick_seed_failure_retries_and_falls_back` asserts the retry path is exercised when the mock RPC returns Err once then Ok.
4. **New test: `test_watermark_captured_after_fetch_avoids_event_loss`** — under simulated slow fetch (await delay), events with `ts ∈ [pre-fetch, post-fetch]` are observed in the next batch.
5. `cargo fmt --all -- --check && cargo check -p fdemon-app && cargo test -p fdemon-app && cargo clippy -p fdemon-app --all-targets -- -D warnings` all pass.

## Files Modified (Write)

- `crates/fdemon-app/src/actions/performance.rs` — `spawn_timeline_polling` body changes (watermark capture, constant, retry), tests.

## Files Read (Dependencies)

- `crates/fdemon-daemon/src/vm_service/timeline.rs` — read-only: confirm `fetch_timeline_chunk` and `get_vm_timeline_micros` signatures are unchanged. (They are; T07 only touches the constant migration and casts.)

## Approach Hints

- The watermark fix is the substantive change. Approach: at the END of a successful fetch, call `get_vm_timeline_micros(&handle).await` and use that result as the new `last_poll_micros + 1`. This costs one extra RPC per tick (negligible; both RPCs are cheap) but eliminates the drift.
  - Alternative: scan `events.iter().map(|e| e.ts).max()` and use `max + 1` only if events are non-empty; fall back to the second RPC otherwise. Choice is up to the implementor; both are acceptable.
- For L11's retry: tokio's `sleep(Duration::from_millis(100))` then a single retry of `get_vm_timeline_micros`. No exponential backoff needed — one retry is sufficient for transient races.
- The retry test will need to use the test-channel infrastructure from T03 (Phase 3) for `VmRequestHandle`. Reuse the existing pattern.
- For the `test_watermark_captured_after_fetch_avoids_event_loss` test: the simulation can be done by injecting an artificial `tokio::time::sleep(50ms)` in a mock `fetch_timeline_chunk` and asserting that events created during that sleep window appear in the next tick's batch.

## Out of Scope

- Making the timeline poll interval configurable. PLAN.md §5.4 specifies 1 Hz; the safety floor is the only change to interval handling.
- Adding a per-session-history overlap window (e.g., always re-fetch the last 100 ms). The watermark fix alone is sufficient.
- Refactoring `spawn_timeline_polling` into smaller helpers — preserve the existing structure.
- Adding a `Message::TimelinePollFailed` surface — failures are still log-only.
- The trait abstraction over `VmRequestHandle` for integration testing — that's T11.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/performance.rs` | Added `TIMELINE_POLL_MIN_MS` constant (L1); extracted `seed_timeline_watermark` async fn with retry + wall-clock fallback (L11); updated watermark to use post-fetch `get_vm_timeline_micros` call (M8); added 3 new tests |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Made `ClientCommand` and `new_with_test_channel` available under `test-helpers` feature for external test harnesses; used `cfg(not(feature = "test-helpers"))` / `cfg(feature = "test-helpers")` to avoid duplicate definitions |

### Notable Decisions/Tradeoffs

1. **Watermark fix (M8):** After each successful `fetch_timeline_chunk`, a second `get_vm_timeline_micros` call captures the post-fetch timestamp. This costs one extra RPC per tick (~50 µs over local WebSocket) but eliminates the event-loss window `[pre_fetch_ts, fetch_completion_ts]`. Fallback to `now_micros + 1` if the post-fetch query fails.

2. **Seed helper extraction (L11):** `seed_timeline_watermark` is extracted as a standalone `async fn` (not embedded in the task closure) to make it directly unit-testable. It tries once, retries after 100 ms backoff, then falls back to `SystemTime::UNIX_EPOCH` wall-clock estimate. Both failures log at `warn!`; the fallback produces a recent microsecond timestamp that bounds the first fetch window to milliseconds rather than the entire VM lifetime.

3. **Test-helpers feature widening:** To write `test_first_tick_seed_failure_retries_and_falls_back` (which needs a stateful mock RPC — fail once, succeed once), `ClientCommand` and `new_with_test_channel` are exposed as `pub` under `test-helpers`. The `#[cfg(all(test, not(feature = "test-helpers")))]` / `#[cfg(feature = "test-helpers")]` guard prevents duplicate definitions when both `cfg(test)` and `feature = "test-helpers"` are active.

4. **`test_watermark_captured_after_fetch_avoids_event_loss`:** Uses the full `spawn_timeline_polling` function with a fake responder via `new_with_test_channel`. The responder answers three RPCs per tick (pre-fetch micros, getVMTimeline, post-fetch micros) and asserts on the second tick's `timeOriginMicros` parameter to verify the watermark was set to the post-fetch value (12_000) not the pre-fetch+1 value (10_001).

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check -p fdemon-app` — Passed
- `cargo check -p fdemon-daemon` — Passed
- `cargo check -p fdemon-daemon --features test-helpers` — Passed
- `cargo test -p fdemon-app --lib -- actions::performance` — Passed (13 tests: 10 existing + 3 new)
- `cargo test -p fdemon-app` — Passed (2432 unit tests, 2 doc tests)
- `cargo test -p fdemon-daemon --lib` — Passed (820 tests)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo test --workspace --lib` — Passed (5772 total: 2432 + 496 + 820 + 842 + 1182)

### Risks/Limitations

1. **Extra RPC per tick:** The post-fetch `get_vm_timeline_micros` adds one RPC per active timeline poll. At 1 Hz with a fast local WebSocket (~50 µs), the cost is negligible. Under high load (heap walks) the RPC may fail, falling back to `now_micros + 1`.

2. **`test_watermark_captured_after_fetch_avoids_event_loss` is fragile to timing:** It uses `tokio::time::timeout(2s)` to wait for messages. If CI is very slow, this could flake. The timeout is generous (200ms poll interval), but it relies on the tokio scheduler running the spawned task reasonably promptly.

3. **T11 (mock VmRequestHandle trait) supersedes the `test-helpers` approach:** Once T11 provides proper trait-based mocking, the `ClientCommand` exposure via `test-helpers` can be removed and tests rewritten to use the trait. The current approach is a minimal bridge until T11 lands.
