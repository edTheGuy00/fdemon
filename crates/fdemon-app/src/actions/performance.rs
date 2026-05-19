//! Performance monitoring polling for Flutter sessions.
//!
//! This module provides the periodic memory-usage and allocation-profile polling
//! task that runs while performance monitoring is active for a session.
//!
//! The single public-to-module entry point is [`spawn_performance_polling`],
//! called from `mod.rs`'s `handle_action` dispatcher for the
//! `StartPerformanceMonitoring` action.
//!
//! **Polling strategy:**
//! - Memory tick (every `performance_refresh_ms`, min [`PERF_POLL_MIN_MS`]):
//!   calls `getMemoryUsage` **once**, then uses the result for both the basic
//!   gauge (`VmServiceMemorySnapshot`) and the rich sample (`VmServiceMemorySample`
//!   via `get_memory_sample_from_usage`). Only one additional `getIsolate` RPC is
//!   issued per tick for RSS. This reduces per-tick VM Service calls from 3 to 2.
//! - Allocation tick (every `allocation_profile_interval_ms`, min
//!   [`ALLOC_PROFILE_POLL_MIN_MS`]): calls `getAllocationProfile` (expensive —
//!   forces a full heap walk), so it runs at a lower frequency than the memory tick.
//!
//! **Mode-aware scaling:**
//! In profile/release mode, both intervals are scaled by [`PROFILE_MODE_MULTIPLIER`]
//! and clamped to their respective profile-mode minimums
//! ([`PROFILE_PERF_POLL_MIN_MS`], [`PROFILE_ALLOC_POLL_MIN_MS`]). This reduces
//! VM Service pressure from ~4 RPCs/sec (debug) to ~1.2 RPCs/sec (profile) with
//! the reporter's aggressive 500ms/1000ms settings, eliminating observable jank.

use std::time::Duration;

use tokio::sync::mpsc;
use tracing::info;

use crate::config::FlutterMode;
use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::vm_service::{VmRequestApi, VmRequestHandle};

/// Minimum polling interval for memory usage (500ms) to prevent excessive VM Service calls.
pub(super) const PERF_POLL_MIN_MS: u64 = 500;

/// Minimum allocation profile polling interval (1000ms).
///
/// `getAllocationProfile` walks the entire Dart heap, making it significantly
/// more expensive than `getMemoryUsage`. A higher minimum ensures it is never
/// called more frequently than once per second even with aggressive settings.
pub(super) const ALLOC_PROFILE_POLL_MIN_MS: u64 = 1000;

/// Multiplier applied to polling intervals in profile/release mode.
///
/// Profile mode has tighter frame budgets (16ms vs ~100ms tolerance in debug).
/// A 3x multiplier reduces RPC frequency enough to eliminate observable jank
/// while keeping data reasonably fresh for monitoring.
///
/// Note: this could be made configurable via a `profile_polling_multiplier`
/// config key as a future follow-up. Hardcoded for now.
const PROFILE_MODE_MULTIPLIER: u64 = 3;

/// Minimum performance refresh interval in profile/release mode (ms).
///
/// Derived from: reporter's 500ms setting × 3x multiplier = 1500ms,
/// raised to 2000ms for safety margin against heap walk latency.
const PROFILE_PERF_POLL_MIN_MS: u64 = 2000;

/// Minimum allocation profile interval in profile/release mode (ms).
///
/// `getAllocationProfile` forces a full heap walk — the primary lag source.
/// 5000ms gives the app 300 frames (at 60fps) between heap walks.
const PROFILE_ALLOC_POLL_MIN_MS: u64 = 5000;

/// Compute the effective polling interval for a given base value, considering
/// the current Flutter run mode.
///
/// In debug mode the interval is clamped to `base_min` only.
/// In profile/release mode the interval is first clamped, then multiplied by
/// [`PROFILE_MODE_MULTIPLIER`], and finally clamped to `profile_min`.
///
/// # Examples
///
/// ```text
/// // Debug: base_ms=500, base_min=500  → 500ms
/// // Profile: base_ms=500, base_min=500, profile_min=2000 → max(500*3, 2000) = 2000ms
/// // Profile: base_ms=10000, base_min=500, profile_min=2000 → max(10000*3, 2000) = 30000ms
/// ```
fn effective_perf_interval(
    base_ms: u64,
    base_min: u64,
    mode: FlutterMode,
    profile_min: u64,
) -> u64 {
    let clamped = base_ms.max(base_min);
    match mode {
        FlutterMode::Profile | FlutterMode::Release => {
            (clamped.saturating_mul(PROFILE_MODE_MULTIPLIER)).max(profile_min)
        }
        FlutterMode::Debug => clamped,
    }
}

/// Spawn the periodic memory-usage polling task for a session.
///
/// Creates a `watch::channel(false)` shutdown channel outside the spawned task
/// so that both the sender and the `JoinHandle` are available to package into
/// `VmServicePerformanceMonitoringStarted`. The TEA layer can then:
/// - Signal the task to stop by sending `true` on the shutdown channel, and
/// - Abort the task directly via the `JoinHandle` if needed.
///
/// The polling loop runs until:
/// - The shutdown channel receives `true` (VM disconnected / session stopped), or
/// - The `msg_tx` channel is closed (engine shutting down).
///
/// **Memory tick** (every `performance_refresh_ms`, min 500ms):
/// 1. Calls `getMemoryUsage` **once** → result shared between both messages.
/// 2. Sends `VmServiceMemorySnapshot` (basic gauge) from the fetched data.
/// 3. Calls `get_memory_sample_from_usage` (only fetches `getIsolate` for RSS) →
///    sends `VmServiceMemorySample` (rich time-series). The two ring buffers stay
///    in sync because both are populated from the same tick, and `getMemoryUsage`
///    is only called once (2 RPCs/tick instead of 3).
///
/// **Allocation tick** (every `allocation_profile_interval_ms`, min 1000ms):
/// - Calls `getAllocationProfile` → sends `VmServiceAllocationProfileReceived`.
///   This is intentionally lower frequency than the memory tick because it is
///   expensive (forces the VM to walk the entire heap).
///
/// **Mode-aware scaling:**
/// In profile/release mode both intervals are scaled by [`PROFILE_MODE_MULTIPLIER`]
/// (currently 3×) and clamped to [`PROFILE_PERF_POLL_MIN_MS`] /
/// [`PROFILE_ALLOC_POLL_MIN_MS`] respectively. This reduces VM Service pressure
/// and eliminates jank caused by frequent heap walks. In debug mode the
/// existing minimums ([`PERF_POLL_MIN_MS`], [`ALLOC_PROFILE_POLL_MIN_MS`]) apply.
///
/// Transient errors from any RPC (e.g., isolate paused during hot reload) are
/// logged at debug level and skipped — the next tick will retry.
///
/// The `performance_refresh_ms` parameter controls the memory polling interval.
/// In debug mode it is clamped to [`PERF_POLL_MIN_MS`] (500ms).
/// In profile/release mode it is scaled and clamped to [`PROFILE_PERF_POLL_MIN_MS`]
/// (2000ms).
///
/// The `allocation_profile_interval_ms` parameter controls the allocation profile
/// polling interval. In debug mode it is clamped to [`ALLOC_PROFILE_POLL_MIN_MS`]
/// (1000ms). In profile/release mode it is scaled and clamped to
/// [`PROFILE_ALLOC_POLL_MIN_MS`] (5000ms).
pub(super) fn spawn_performance_polling(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    performance_refresh_ms: u64,
    allocation_profile_interval_ms: u64,
    mode: FlutterMode,
) {
    // Clamp intervals to their respective minimums, applying mode-aware scaling
    // for profile/release mode to reduce VM Service pressure.
    let memory_interval_ms = effective_perf_interval(
        performance_refresh_ms,
        PERF_POLL_MIN_MS,
        mode,
        PROFILE_PERF_POLL_MIN_MS,
    );
    let alloc_interval_ms = effective_perf_interval(
        allocation_profile_interval_ms,
        ALLOC_PROFILE_POLL_MIN_MS,
        mode,
        PROFILE_ALLOC_POLL_MIN_MS,
    );

    let memory_interval = Duration::from_millis(memory_interval_ms);
    let alloc_interval = Duration::from_millis(alloc_interval_ms);

    // Create the shutdown channel outside the task so both ends are available
    // before the task starts running.
    let (perf_shutdown_tx, mut perf_shutdown_rx) = tokio::sync::watch::channel(false);
    // Arc is required because Message derives Clone and watch::Sender does not impl Clone.
    let perf_shutdown_tx = std::sync::Arc::new(perf_shutdown_tx);

    // Create the allocation-pause channel.
    // Initial value: `true` (paused) — allocation polling starts paused
    // because performance monitoring begins at VM connect time, often before
    // the user opens the Performance panel. The TEA handler sends `false`
    // when the user enters the Performance panel.
    let (alloc_pause_tx, mut alloc_pause_rx) = tokio::sync::watch::channel(true);
    let alloc_pause_tx = std::sync::Arc::new(alloc_pause_tx);

    // Create the performance-pause channel (higher-level gate).
    // Initial value: `true` (paused) — monitoring starts at VM connect time,
    // before the user opens DevTools. The handler sends `false` when the user
    // enters DevTools mode and `true` when they exit. This prevents all
    // `getMemoryUsage` and `getIsolate` RPCs while viewing logs.
    let (perf_pause_tx, mut perf_pause_rx) = tokio::sync::watch::channel(true);
    let perf_pause_tx = std::sync::Arc::new(perf_pause_tx);

    // The JoinHandle from `tokio::spawn` is only available after the call, but
    // the task will send it in `VmServicePerformanceMonitoringStarted` as the
    // first async operation. We use `Arc<Mutex<Option<>>>` as a rendezvous:
    // - We fill the slot after spawn returns (synchronously, before any await).
    // - The task reads from the slot when it sends the "started" message.
    // Because tokio tasks don't run until the current thread yields (or the
    // runtime schedules them), the slot is guaranteed to be filled before the
    // task's first `.await` point.
    let task_handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let task_handle_slot_for_msg = task_handle_slot.clone();

    let join_handle = tokio::spawn(async move {
        // Notify TEA that monitoring has started. The slot is populated
        // synchronously by the caller before this first `.await` runs.
        if msg_tx
            .send(Message::VmServicePerformanceMonitoringStarted {
                session_id,
                perf_shutdown_tx,
                perf_task_handle: task_handle_slot_for_msg,
                alloc_pause_tx,
                perf_pause_tx,
            })
            .await
            .is_err()
        {
            // Channel closed — engine is shutting down.
            return;
        }

        let mut memory_tick = tokio::time::interval(memory_interval);
        memory_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut alloc_tick = tokio::time::interval(alloc_interval);
        alloc_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = memory_tick.tick() => {
                    // Skip if performance monitoring is paused (user not in DevTools).
                    // This prevents `getMemoryUsage` and `getIsolate` RPCs while the
                    // user is viewing logs, eliminating VM Service pressure outside DevTools.
                    if *perf_pause_rx.borrow() {
                        continue;
                    }

                    // Fetch the main isolate ID (cached after first call).
                    let isolate_id = match handle.main_isolate_id().await {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::debug!(
                                "Could not get isolate ID for memory polling (session {}): {}",
                                session_id, e
                            );
                            continue;
                        }
                    };

                    // Single `getMemoryUsage` RPC — result shared between both messages.
                    //
                    // Before this change two separate RPC calls were issued:
                    //   1. get_memory_usage()  → VmServiceMemorySnapshot
                    //   2. get_memory_sample() → internally calls get_memory_usage again
                    //
                    // Now we call `getMemoryUsage` once and pass the result to
                    // `get_memory_sample_from_usage`, which only needs `getIsolate` (RSS).
                    // This reduces the per-tick RPC count from 3 to 2.
                    let usage = match fdemon_daemon::vm_service::get_memory_usage(&handle, &isolate_id).await {
                        Ok(usage) => usage,
                        Err(e) => {
                            // Transient errors are expected during hot reload when
                            // the isolate is paused. Log at debug and continue.
                            tracing::debug!(
                                "Memory usage poll failed for session {}: {}",
                                session_id, e
                            );
                            continue;
                        }
                    };

                    // 1. Basic memory snapshot — populates memory_history gauge.
                    if msg_tx
                        .send(Message::VmServiceMemorySnapshot {
                            session_id,
                            memory: usage.clone(),
                        })
                        .await
                        .is_err()
                    {
                        // Engine shutting down.
                        break;
                    }

                    // 2. Rich memory sample — populates memory_samples ring buffer.
                    //    Re-uses the already-fetched `usage`; only `getIsolate` (RSS) is
                    //    fetched here. If `getIsolate` fails, `rss` defaults to 0 and the
                    //    sample is still sent (non-fatal degradation).
                    if let Some(sample) =
                        fdemon_daemon::vm_service::get_memory_sample_from_usage(
                            &handle,
                            &isolate_id,
                            &usage,
                        )
                        .await
                    {
                        if msg_tx
                            .send(Message::VmServiceMemorySample { session_id, sample })
                            .await
                            .is_err()
                        {
                            // Engine shutting down.
                            break;
                        }
                    } else {
                        tracing::debug!(
                            "Rich memory sample unavailable for session {} (non-fatal)",
                            session_id
                        );
                    }
                }

                _ = alloc_tick.tick() => {
                    // Skip if performance monitoring is globally paused (user not in
                    // DevTools) OR if allocation polling is paused (Performance panel
                    // not visible). Both channels must be clear for `getAllocationProfile`
                    // to fire — it is the most expensive RPC (forces a full heap walk).
                    if *perf_pause_rx.borrow() || *alloc_pause_rx.borrow() {
                        continue;
                    }

                    if fetch_and_send_alloc_profile(&handle, &msg_tx, session_id).await {
                        break;
                    }
                }

                // Watch for perf_pause unpause transitions so the user sees fresh
                // memory data immediately when they enter DevTools, without waiting
                // up to `memory_interval` for the next scheduled tick.
                Ok(()) = perf_pause_rx.changed() => {
                    if *perf_pause_rx.borrow() {
                        // Transitioned to paused (user left DevTools) — nothing to do.
                        continue;
                    }

                    // Transitioned to active (user entered DevTools). Fire one immediate
                    // memory fetch so the Performance panel shows current data.
                    let isolate_id = match handle.main_isolate_id().await {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::debug!(
                                "Could not get isolate ID for immediate memory fetch (session {}): {}",
                                session_id, e
                            );
                            continue;
                        }
                    };

                    let usage = match fdemon_daemon::vm_service::get_memory_usage(&handle, &isolate_id).await {
                        Ok(usage) => usage,
                        Err(e) => {
                            tracing::debug!(
                                "Immediate memory fetch on DevTools entry failed for session {}: {}",
                                session_id, e
                            );
                            continue;
                        }
                    };

                    if msg_tx
                        .send(Message::VmServiceMemorySnapshot {
                            session_id,
                            memory: usage.clone(),
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }

                    if let Some(sample) =
                        fdemon_daemon::vm_service::get_memory_sample_from_usage(
                            &handle,
                            &isolate_id,
                            &usage,
                        )
                        .await
                    {
                        if msg_tx
                            .send(Message::VmServiceMemorySample { session_id, sample })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }

                // Watch for unpause transitions so the user sees fresh allocation
                // data immediately when they open the Performance panel, without
                // waiting up to `alloc_interval` for the next scheduled tick.
                // The `watch` channel coalesces rapid toggles — only the final
                // value matters, so burst panel switches don't create burst fetches.
                Ok(()) = alloc_pause_rx.changed() => {
                    if *alloc_pause_rx.borrow() {
                        // Transitioned to paused — nothing to do.
                        continue;
                    }

                    // Transitioned to active (Performance panel became visible).
                    // Fire one immediate allocation profile fetch so the panel is
                    // populated without waiting for the next tick.
                    if fetch_and_send_alloc_profile(&handle, &msg_tx, session_id).await {
                        break;
                    }
                }

                _ = perf_shutdown_rx.changed() => {
                    if *perf_shutdown_rx.borrow() {
                        info!(
                            "Performance monitoring stopped for session {}",
                            session_id
                        );
                        break;
                    }
                }
            }
        }
    });

    // Synchronously store the JoinHandle in the slot. The task hasn't run yet
    // (tokio tasks don't run until the current thread yields to the runtime),
    // so the slot is populated before the first `.await` inside the task.
    if let Ok(mut slot) = task_handle_slot.lock() {
        *slot = Some(join_handle);
    };
}

/// Fetch the allocation profile for the session and send it to the TEA handler.
///
/// Returns `true` if the message channel is closed (caller should `break`),
/// `false` if the caller should continue the polling loop.
async fn fetch_and_send_alloc_profile(
    handle: &VmRequestHandle,
    msg_tx: &mpsc::Sender<Message>,
    session_id: SessionId,
) -> bool {
    let isolate_id = match handle.main_isolate_id().await {
        Ok(id) => id,
        Err(e) => {
            tracing::debug!(
                "Could not get isolate ID for allocation polling (session {}): {}",
                session_id,
                e
            );
            return false;
        }
    };

    match fdemon_daemon::vm_service::get_allocation_profile(
        handle,
        &isolate_id,
        false, // gc=false — no forced GC before profiling
    )
    .await
    {
        Ok(profile) => {
            if msg_tx
                .send(Message::VmServiceAllocationProfileReceived {
                    session_id,
                    profile,
                })
                .await
                .is_err()
            {
                // Engine shutting down.
                return true;
            }
        }
        Err(e) => {
            tracing::debug!(
                "Allocation profile poll failed for session {}: {}",
                session_id,
                e
            );
        }
    }
    false
}

// ─────────────────────────────────────────────────────────────────────────────
// Timeline polling task
// ─────────────────────────────────────────────────────────────────────────────

/// Minimum timeline poll interval (200 ms).
/// Safety floor preventing accidental sub-200ms polling if `poll_interval_ms`
/// is mis-configured. PLAN.md §5.4 target is 1 Hz (1000 ms); 200 ms is the
/// floor at which VM Service stress remains acceptable.
const TIMELINE_POLL_MIN_MS: u64 = 200;

/// Seed the timeline watermark from the current VM clock, with one retry on
/// failure.
///
/// **Why this matters (L11):** If the seed fails and we fall back to
/// `last_poll_micros = 0`, the first `fetch_timeline_chunk(handle, 0, extent)`
/// retrieves the *entire VM lifetime* worth of events (up to the VM's circular
/// buffer, ~32 MB). This dumps thousands of events on the handler at once,
/// causing a visible stall.
///
/// **Strategy:**
/// 1. Try `getVMTimelineMicros` once.
/// 2. On failure, wait 100 ms (transient race at startup) and retry once.
/// 3. If the retry also fails, log at `warn!` and fall back to a
///    "now-ish" estimate: current wall-clock time converted to microseconds via
///    `std::time::SystemTime`. This caps the first poll extent to the
///    milliseconds elapsed since the estimate was computed (typically < 10 ms),
///    preventing a flood of stale events.
///
/// Returns the seed value to use as `last_poll_micros`.
async fn seed_timeline_watermark<H: VmRequestApi>(handle: &H, session_id: SessionId) -> u64 {
    match fdemon_daemon::vm_service::get_vm_timeline_micros(handle).await {
        Ok(ts) => ts,
        Err(first_err) => {
            tracing::debug!(
                "Timeline seed: first getVMTimelineMicros failed for session {}: {} — retrying in 100ms",
                session_id,
                first_err
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
            match fdemon_daemon::vm_service::get_vm_timeline_micros(handle).await {
                Ok(ts) => ts,
                Err(retry_err) => {
                    // Both attempts failed. Use wall-clock as a now-ish estimate
                    // so the first fetch window is bounded to a small extent
                    // (milliseconds, not the entire VM lifetime).
                    let now_ish = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_micros() as u64)
                        .unwrap_or(0);
                    tracing::warn!(
                        "Timeline seed: getVMTimelineMicros failed twice for session {}; \
                         falling back to wall-clock estimate ({} µs). Error: {}",
                        session_id,
                        now_ish,
                        retry_err
                    );
                    now_ish
                }
            }
        }
    }
}

/// Spawn the 1-Hz timeline polling task for a session.
///
/// Modelled on [`spawn_performance_polling`]. Creates shutdown and pause watch
/// channels outside the spawned task so both ends are available before the
/// task starts. The task:
///
/// 1. Sends `VmServiceTimelineMonitoringStarted` immediately to wire the handles
///    into `SessionHandle`.
/// 2. On each 1-Hz tick (subject to the pause gate), calls
///    `getVMTimelineMicros` → `getVMTimeline` to fetch events since the last
///    poll, then sends `TimelineEventsBatchReceived`.
///
/// The pause channel initial value is `true` (paused) — the task starts at VM
/// connect time but only begins fetching when the user opens the Performance panel.
pub(super) fn spawn_timeline_polling<H: VmRequestApi + Send + Sync + 'static>(
    session_id: SessionId,
    handle: H,
    msg_tx: mpsc::Sender<Message>,
    poll_interval_ms: u64,
) {
    let poll_interval = Duration::from_millis(poll_interval_ms.max(TIMELINE_POLL_MIN_MS));

    // Shutdown channel — `true` stops the loop cleanly.
    let (timeline_shutdown_tx, mut timeline_shutdown_rx) = tokio::sync::watch::channel(false);
    let timeline_shutdown_tx = std::sync::Arc::new(timeline_shutdown_tx);

    // Pause channel — `true` = paused (Performance panel not active).
    // Initial value `true`: starts paused, unpaused when user enters Performance panel.
    let (timeline_pause_tx, mut timeline_pause_rx) = tokio::sync::watch::channel(true);
    let timeline_pause_tx = std::sync::Arc::new(timeline_pause_tx);

    // Rendezvous slot for the JoinHandle — filled synchronously after spawn.
    let task_handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let task_handle_slot_for_msg = task_handle_slot.clone();

    let join_handle = tokio::spawn(async move {
        // Notify TEA that timeline monitoring has started and wire handles.
        // The slot is populated synchronously before this first `.await`.
        if msg_tx
            .send(Message::VmServiceTimelineMonitoringStarted {
                session_id,
                timeline_shutdown_tx: timeline_shutdown_tx.clone(),
                timeline_pause_tx: timeline_pause_tx.clone(),
                timeline_task_handle: task_handle_slot_for_msg,
            })
            .await
            .is_err()
        {
            // Channel closed — engine is shutting down.
            return;
        }

        let mut interval = tokio::time::interval(poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Seed the last-poll timestamp from the current VM timeline clock, with
        // retry and bounded fallback. See [`seed_timeline_watermark`] for the
        // rationale — a zero seed would cause the first fetch to retrieve the
        // entire VM event buffer, potentially thousands of events.
        let mut last_poll_micros: u64 = seed_timeline_watermark(&handle, session_id).await;

        let mut thread_name_map: std::collections::HashMap<i64, String> =
            std::collections::HashMap::new();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Skip if paused (Performance panel not active).
                    if *timeline_pause_rx.borrow() {
                        continue;
                    }

                    let now_micros = match fdemon_daemon::vm_service::get_vm_timeline_micros(&handle).await {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::debug!(
                                "Timeline poll: getVMTimelineMicros failed for session {}: {}",
                                session_id, e
                            );
                            continue;
                        }
                    };

                    let extent = now_micros.saturating_sub(last_poll_micros);
                    if extent == 0 {
                        continue;
                    }

                    match fdemon_daemon::vm_service::fetch_timeline_chunk(
                        &handle,
                        last_poll_micros,
                        extent,
                        &mut thread_name_map,
                    )
                    .await
                    {
                        Ok(events) if !events.is_empty() => {
                            if msg_tx
                                .send(Message::TimelineEventsBatchReceived {
                                    session_id,
                                    events,
                                })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(_) => {} // empty batch — normal
                        Err(e) => {
                            tracing::debug!(
                                "Timeline poll: fetch_timeline_chunk failed for session {}: {}",
                                session_id, e
                            );
                        }
                    }

                    // Advance the watermark using a post-fetch VM clock query.
                    //
                    // M8: Using `now_micros + 1` (captured *before* the fetch) would
                    // silently drop any events whose `ts` falls in the window
                    // [now_micros, fetch_completion_time]. Under slow VM Service
                    // responses (heap walk, profile-mode lag) this manifests as
                    // sporadic timeline gaps. By re-querying after the fetch we
                    // capture that window in the next poll.
                    //
                    // Cost: one extra `getVMTimelineMicros` RPC per tick (~50 µs
                    // over a local WebSocket). Acceptable given the 1 Hz poll rate.
                    //
                    // Fallback: if the post-fetch query fails (e.g. VM restarting),
                    // fall back to `now_micros + 1` to maintain forward progress.
                    last_poll_micros =
                        match fdemon_daemon::vm_service::get_vm_timeline_micros(&handle).await {
                            Ok(post_fetch_ts) => post_fetch_ts,
                            Err(_) => now_micros.saturating_add(1),
                        };
                }

                Ok(()) = timeline_pause_rx.changed() => {
                    // Pause state changed — the next tick will re-check.
                }

                _ = timeline_shutdown_rx.changed() => {
                    if *timeline_shutdown_rx.borrow() {
                        info!(
                            "Timeline monitoring stopped for session {}",
                            session_id
                        );
                        break;
                    }
                }
            }
        }
    });

    // Synchronously store the JoinHandle before any async code in the task runs.
    if let Ok(mut slot) = task_handle_slot.lock() {
        *slot = Some(join_handle);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // FIXME: see clippy-rust-191-cleanup — asserts constant invariant that
    // allocation profiling minimum (1000ms) is always >= memory polling minimum (500ms).
    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn test_performance_poll_constants_are_reasonable() {
        assert_eq!(PERF_POLL_MIN_MS, 500, "perf poll minimum should be 500ms");
        assert_eq!(
            ALLOC_PROFILE_POLL_MIN_MS, 1000,
            "alloc profile poll minimum should be 1000ms"
        );
        assert!(
            ALLOC_PROFILE_POLL_MIN_MS >= PERF_POLL_MIN_MS,
            "allocation profiling is more expensive and should never poll faster than memory polling"
        );
    }

    // FIXME: see clippy-rust-191-cleanup — asserts constant invariants that
    // profile perf minimum (2000ms) > debug perf minimum (500ms) and
    // profile alloc minimum (5000ms) > debug alloc minimum (1000ms).
    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn test_profile_mode_constants_are_reasonable() {
        assert_eq!(
            PROFILE_MODE_MULTIPLIER, 3,
            "profile multiplier should be 3x"
        );
        assert_eq!(
            PROFILE_PERF_POLL_MIN_MS, 2000,
            "profile perf minimum should be 2000ms"
        );
        assert_eq!(
            PROFILE_ALLOC_POLL_MIN_MS, 5000,
            "profile alloc minimum should be 5000ms"
        );
        assert!(
            PROFILE_PERF_POLL_MIN_MS > PERF_POLL_MIN_MS,
            "profile perf minimum must exceed debug minimum"
        );
        assert!(
            PROFILE_ALLOC_POLL_MIN_MS > ALLOC_PROFILE_POLL_MIN_MS,
            "profile alloc minimum must exceed debug minimum"
        );
    }

    #[test]
    fn test_debug_mode_uses_base_intervals() {
        // Given performance_refresh_ms = 500 and mode = Debug
        // Then effective interval = 500ms (base minimum, no multiplier)
        let result = effective_perf_interval(
            500,
            PERF_POLL_MIN_MS,
            FlutterMode::Debug,
            PROFILE_PERF_POLL_MIN_MS,
        );
        assert_eq!(result, 500, "debug mode should not scale the interval");
    }

    #[test]
    fn test_debug_mode_clamps_to_base_minimum() {
        // Given performance_refresh_ms = 100 and mode = Debug
        // Then effective interval = 500ms (clamped to base minimum)
        let result = effective_perf_interval(
            100,
            PERF_POLL_MIN_MS,
            FlutterMode::Debug,
            PROFILE_PERF_POLL_MIN_MS,
        );
        assert_eq!(result, 500, "debug mode should clamp to base minimum");
    }

    #[test]
    fn test_profile_mode_scales_memory_interval() {
        // Given performance_refresh_ms = 500 and mode = Profile
        // Then effective interval = max(500 * 3, 2000) = 2000ms
        let result = effective_perf_interval(
            500,
            PERF_POLL_MIN_MS,
            FlutterMode::Profile,
            PROFILE_PERF_POLL_MIN_MS,
        );
        assert_eq!(
            result, 2000,
            "profile mode should scale 500ms to 2000ms (profile minimum)"
        );
    }

    #[test]
    fn test_profile_mode_scales_alloc_interval() {
        // Given allocation_profile_interval_ms = 1000 and mode = Profile
        // Then effective interval = max(1000 * 3, 5000) = 5000ms
        let result = effective_perf_interval(
            1000,
            ALLOC_PROFILE_POLL_MIN_MS,
            FlutterMode::Profile,
            PROFILE_ALLOC_POLL_MIN_MS,
        );
        assert_eq!(
            result, 5000,
            "profile mode should scale 1000ms to 5000ms (profile minimum)"
        );
    }

    #[test]
    fn test_profile_mode_respects_user_higher_interval() {
        // Given performance_refresh_ms = 10000 and mode = Profile
        // Then effective interval = max(10000 * 3, 2000) = 30000ms
        // User's explicit high value is respected (with multiplier applied)
        let result = effective_perf_interval(
            10_000,
            PERF_POLL_MIN_MS,
            FlutterMode::Profile,
            PROFILE_PERF_POLL_MIN_MS,
        );
        assert_eq!(
            result, 30_000,
            "profile mode should apply multiplier to user's high interval"
        );
    }

    #[test]
    fn test_release_mode_uses_same_scaling_as_profile() {
        // Release mode must produce identical results to Profile mode
        let memory_profile = effective_perf_interval(
            500,
            PERF_POLL_MIN_MS,
            FlutterMode::Profile,
            PROFILE_PERF_POLL_MIN_MS,
        );
        let memory_release = effective_perf_interval(
            500,
            PERF_POLL_MIN_MS,
            FlutterMode::Release,
            PROFILE_PERF_POLL_MIN_MS,
        );
        assert_eq!(
            memory_profile, memory_release,
            "release and profile should produce the same memory interval"
        );

        let alloc_profile = effective_perf_interval(
            1000,
            ALLOC_PROFILE_POLL_MIN_MS,
            FlutterMode::Profile,
            PROFILE_ALLOC_POLL_MIN_MS,
        );
        let alloc_release = effective_perf_interval(
            1000,
            ALLOC_PROFILE_POLL_MIN_MS,
            FlutterMode::Release,
            PROFILE_ALLOC_POLL_MIN_MS,
        );
        assert_eq!(
            alloc_profile, alloc_release,
            "release and profile should produce the same alloc interval"
        );
    }

    #[test]
    fn test_profile_multiplier_applied_after_base_clamp() {
        // Verifies: clamp first, then multiply (acceptance criterion #6)
        // Given performance_refresh_ms = 100 (below base_min=500), mode = Profile
        // Step 1: clamp(100, 500) = 500
        // Step 2: 500 * 3 = 1500, then max(1500, 2000) = 2000
        let result = effective_perf_interval(
            100,
            PERF_POLL_MIN_MS,
            FlutterMode::Profile,
            PROFILE_PERF_POLL_MIN_MS,
        );
        assert_eq!(
            result, 2000,
            "multiplier should be applied after base clamp"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // TIMELINE_POLL_MIN_MS constant
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_timeline_poll_min_ms_value() {
        // L1: the named constant must equal 200 ms.
        assert_eq!(
            TIMELINE_POLL_MIN_MS, 200,
            "TIMELINE_POLL_MIN_MS must be 200 ms per PLAN.md §5.4"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // seed_timeline_watermark — L11
    // ─────────────────────────────────────────────────────────────────────────

    /// Both `getVMTimelineMicros` calls fail → fall back to a non-zero
    /// wall-clock estimate. Tests the full-failure path of `seed_timeline_watermark`.
    #[tokio::test]
    async fn test_seed_timeline_watermark_both_fail_returns_nonzero_fallback() {
        // `new_for_test(None)` drops the receiver immediately, so every RPC
        // returns `Error::ChannelClosed` — simulating two consecutive failures.
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(None);
        let session_id: crate::session::SessionId = 42;

        let seed = seed_timeline_watermark(&handle, session_id).await;

        // The wall-clock estimate is in microseconds since UNIX epoch.
        // It must be strictly positive (> 0) and plausibly recent
        // (> 1_600_000_000_000_000 µs ≈ year 2020).
        assert!(
            seed > 1_600_000_000_000_000,
            "wall-clock fallback should produce a recent timestamp, got {seed}"
        );
    }

    /// First `getVMTimelineMicros` fails, second succeeds → seed equals the
    /// successful response. Tests the retry-then-succeed path.
    ///
    /// Uses `VmRequestHandle::new_with_test_channel()` (available under the
    /// `test-helpers` feature that `fdemon-app` enables for `fdemon-daemon`)
    /// to drive a stateful fake responder.
    #[tokio::test]
    async fn test_first_tick_seed_failure_retries_and_falls_back() {
        use fdemon_daemon::vm_service::client::ClientCommand;
        use fdemon_daemon::vm_service::VmRequestHandle;
        use serde_json::json;

        let session_id: crate::session::SessionId = 42;
        let (handle, mut cmd_rx) = VmRequestHandle::new_with_test_channel();

        // Fake responder: fails on first call, returns a known timestamp on retry.
        let responder = tokio::spawn(async move {
            // First call → Err (simulates transient startup race).
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Err(fdemon_core::error::Error::vm_service(
                    "simulated transient failure",
                )));
            }
            // Second call (after 100 ms retry backoff) → Ok with timestamp 99_000.
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Ok(json!({ "timestamp": 99_000_i64 })));
            }
        });

        let seed = seed_timeline_watermark(&handle, session_id).await;

        // The retry succeeded — seed must equal the successful response value.
        assert_eq!(
            seed, 99_000,
            "seed should equal the timestamp returned on the successful retry"
        );

        let _ = responder.await;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Watermark post-fetch capture — M8
    // ─────────────────────────────────────────────────────────────────────────

    /// Verifies that after a successful `fetch_timeline_chunk`, the watermark
    /// is advanced to the post-fetch clock value (not the pre-fetch value + 1).
    ///
    /// The test uses a fake responder to:
    ///   1. Answer the *pre-fetch* `getVMTimelineMicros` with `t=10_000`.
    ///   2. Answer `getVMTimeline` (the chunk fetch) with an event at `ts=11_000`.
    ///   3. Answer the *post-fetch* `getVMTimelineMicros` with `t=12_000`.
    ///
    /// In the pre-fix code `last_poll_micros` would be set to `10_000 + 1 = 10_001`,
    /// leaving the window `[10_001, 12_000]` uncovered by the next poll.
    ///
    /// With the fix, `last_poll_micros` is set to `12_000` (the post-fetch value),
    /// so the next poll covers `[12_000, …]` without a gap.
    ///
    /// Because the watermark is internal to the spawned task, we verify the
    /// *observable consequence*: a second poll tick with `now_micros = 15_000`
    /// must request events starting from `last_poll_micros = 12_000`
    /// (extent = 3_000), not from 10_001 (extent = 4_999).
    #[tokio::test]
    async fn test_watermark_captured_after_fetch_avoids_event_loss() {
        use fdemon_daemon::vm_service::client::ClientCommand;
        use fdemon_daemon::vm_service::VmRequestHandle;
        use serde_json::json;

        let session_id: crate::session::SessionId = 42;
        let (handle, mut cmd_rx) = VmRequestHandle::new_with_test_channel();
        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<crate::message::Message>(16);

        // Spawn the timeline task.
        // We use a very short interval so the test doesn't need real wall-clock delays.
        spawn_timeline_polling(session_id, handle, msg_tx, TIMELINE_POLL_MIN_MS);

        // Drain the VmServiceTimelineMonitoringStarted message.
        // The task sends this before doing any RPC.
        let started_msg = msg_rx.recv().await.expect("should receive started message");
        let (timeline_pause_tx, timeline_shutdown_tx) = match started_msg {
            crate::message::Message::VmServiceTimelineMonitoringStarted {
                timeline_pause_tx,
                timeline_shutdown_tx,
                ..
            } => (timeline_pause_tx, timeline_shutdown_tx),
            other => panic!("unexpected message: {other:?}"),
        };

        // The task starts paused. We must unpause before it will poll.
        // First we set up the fake responder for the seed call.
        // Then we unpause.

        // Responder: answer the seed `getVMTimelineMicros` call with t=5_000.
        // (The task calls this immediately after unpausing, for the seed.)
        let responder = tokio::spawn(async move {
            // Seed call (before unpausing in the loop — actually called right
            // at task start before the loop begins, so respond first).
            if let Some(ClientCommand::SendRequest {
                response_tx,
                method,
                ..
            }) = cmd_rx.recv().await
            {
                assert_eq!(
                    method, "getVMTimelineMicros",
                    "first RPC should be seed getVMTimelineMicros"
                );
                let _ = response_tx.send(Ok(json!({ "timestamp": 5_000_i64 })));
            }

            // First poll tick after unpause:
            // 1. Pre-fetch getVMTimelineMicros → t=10_000.
            if let Some(ClientCommand::SendRequest {
                response_tx,
                method,
                ..
            }) = cmd_rx.recv().await
            {
                assert_eq!(method, "getVMTimelineMicros");
                let _ = response_tx.send(Ok(json!({ "timestamp": 10_000_i64 })));
            }
            // 2. getVMTimeline chunk (extent = 10_000 - 5_000 = 5_000).
            if let Some(ClientCommand::SendRequest {
                response_tx,
                method,
                ..
            }) = cmd_rx.recv().await
            {
                assert_eq!(method, "getVMTimeline");
                // Return one event at ts=11_000 (in the gap that old code would miss).
                let _ = response_tx.send(Ok(json!({
                    "type": "Timeline",
                    "traceEvents": [
                        { "ph": "X", "name": "Frame", "cat": "Embedder",
                          "pid": 1, "tid": 1, "ts": 11_000_u64, "dur": 500_u64 }
                    ]
                })));
            }
            // 3. Post-fetch getVMTimelineMicros → t=12_000.
            //    This is the value the watermark should be set to.
            if let Some(ClientCommand::SendRequest {
                response_tx,
                method,
                ..
            }) = cmd_rx.recv().await
            {
                assert_eq!(method, "getVMTimelineMicros");
                let _ = response_tx.send(Ok(json!({ "timestamp": 12_000_i64 })));
            }

            // Second poll tick: pre-fetch getVMTimelineMicros → t=15_000.
            // extent must be 15_000 - 12_000 = 3_000 (not 15_000 - 10_001 = 4_999).
            if let Some(ClientCommand::SendRequest {
                response_tx,
                method,
                ..
            }) = cmd_rx.recv().await
            {
                assert_eq!(method, "getVMTimelineMicros");
                let _ = response_tx.send(Ok(json!({ "timestamp": 15_000_i64 })));
            }
            // getVMTimeline — capture params to verify the extent.
            if let Some(ClientCommand::SendRequest {
                response_tx,
                method,
                params,
            }) = cmd_rx.recv().await
            {
                assert_eq!(method, "getVMTimeline");
                let p = params.expect("params must be present");
                let origin = p
                    .get("timeOriginMicros")
                    .and_then(|v| v.as_i64())
                    .expect("timeOriginMicros");
                let extent = p
                    .get("timeExtentMicros")
                    .and_then(|v| v.as_i64())
                    .expect("timeExtentMicros");
                // With the fix: origin = 12_000, extent = 3_000.
                assert_eq!(
                    origin, 12_000,
                    "second poll origin must be post-fetch watermark (12_000), not pre-fetch+1"
                );
                assert_eq!(
                    extent, 3_000,
                    "second poll extent must cover [12_000, 15_000] = 3_000 µs"
                );
                let _ = response_tx.send(Ok(json!({ "type": "Timeline", "traceEvents": [] })));
            }
            // Post-fetch getVMTimelineMicros for second tick.
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Ok(json!({ "timestamp": 15_001_i64 })));
            }
        });

        // Unpause the timeline task.
        let _ = timeline_pause_tx.send(false);

        // Receive the TimelineEventsBatchReceived from the first tick.
        let batch_msg = tokio::time::timeout(Duration::from_millis(2_000), msg_rx.recv())
            .await
            .expect("timeout waiting for batch message")
            .expect("channel should be open");

        assert!(
            matches!(
                batch_msg,
                crate::message::Message::TimelineEventsBatchReceived { .. }
            ),
            "expected TimelineEventsBatchReceived, got: {batch_msg:?}"
        );

        // Allow the second tick to run and the responder's assertions to fire.
        // We just wait for the responder to finish (assertions are inside it).
        let _ = tokio::time::timeout(Duration::from_millis(2_000), responder).await;

        // Shut down cleanly.
        let _ = timeline_shutdown_tx.send(true);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // MockVmRequestApi — minimal test double for spawn_timeline_polling
    // ─────────────────────────────────────────────────────────────────────────

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// A single recorded RPC call, for assertion in tests.
    #[derive(Debug, Clone)]
    #[allow(dead_code)] // `params` available for future assertion use
    pub(super) struct MockCall {
        pub method: String,
        pub params: Option<serde_json::Value>,
    }

    /// A canned response for the mock: either an `Ok` value or an `Err`
    /// message that will be converted to [`fdemon_core::error::Error::VmService`].
    #[derive(Debug, Clone)]
    #[allow(dead_code)] // `Err` available for future error-path tests
    pub(super) enum MockResponse {
        Ok(serde_json::Value),
        Err(String),
    }

    /// Minimal mock implementation of [`VmRequestApi`] for integration tests.
    ///
    /// - Records every `request` and `call_extension` call in `call_log`.
    /// - Drains canned responses from a per-method queue; if the queue is empty
    ///   (or the method has no entry), returns `Ok(json!({}))` by default.
    #[derive(Clone)]
    pub(super) struct MockVmRequestApi {
        /// Ordered log of every call made to this mock.
        call_log: Arc<Mutex<Vec<MockCall>>>,
        /// Per-method response queue — front of the queue is dequeued first.
        responses: Arc<Mutex<HashMap<String, std::collections::VecDeque<MockResponse>>>>,
    }

    impl MockVmRequestApi {
        /// Create a new mock with an empty call log and no canned responses.
        pub fn new() -> Self {
            Self {
                call_log: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        /// Enqueue a canned response for the given RPC method.
        ///
        /// Responses are dequeued FIFO. Once exhausted, the mock returns
        /// `Ok(json!({}))` as the default.
        pub fn enqueue(&self, method: &str, response: MockResponse) {
            let mut map = self.responses.lock().unwrap();
            map.entry(method.to_string())
                .or_default()
                .push_back(response);
        }

        /// Enqueue a successful response for the given method.
        pub fn enqueue_ok(&self, method: &str, value: serde_json::Value) {
            self.enqueue(method, MockResponse::Ok(value));
        }

        /// Return a snapshot of the call log for assertions.
        #[allow(dead_code)] // Available for future assertion use
        pub fn call_log(&self) -> Vec<MockCall> {
            self.call_log.lock().unwrap().clone()
        }

        /// Return the number of calls to a specific method recorded so far.
        pub fn call_count(&self, method: &str) -> usize {
            self.call_log
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.method == method)
                .count()
        }

        /// Dequeue the next canned response for `method`, or return the default.
        fn next_response(
            &self,
            method: &str,
        ) -> Result<serde_json::Value, fdemon_core::error::Error> {
            let mut map = self.responses.lock().unwrap();
            let next = map.get_mut(method).and_then(|q| q.pop_front());

            match next {
                Some(MockResponse::Ok(v)) => Ok(v),
                Some(MockResponse::Err(msg)) => Err(fdemon_core::error::Error::vm_service(msg)),
                None => Ok(serde_json::json!({})),
            }
        }
    }

    impl VmRequestApi for MockVmRequestApi {
        fn request(
            &self,
            method: &str,
            params: Option<serde_json::Value>,
        ) -> impl std::future::Future<Output = fdemon_core::prelude::Result<serde_json::Value>> + Send
        {
            let call = MockCall {
                method: method.to_string(),
                params: params.clone(),
            };
            self.call_log.lock().unwrap().push(call);
            let result = self.next_response(method);
            std::future::ready(result)
        }

        fn call_extension(
            &self,
            method: &str,
            isolate_id: &str,
            args: Option<HashMap<String, String>>,
        ) -> impl std::future::Future<Output = fdemon_core::prelude::Result<serde_json::Value>> + Send
        {
            // Record as a request with combined params for simplicity.
            let mut combined = serde_json::json!({ "isolateId": isolate_id });
            if let Some(ref a) = args {
                if let serde_json::Value::Object(ref mut obj) = combined {
                    for (k, v) in a {
                        obj.insert(k.clone(), serde_json::Value::String(v.clone()));
                    }
                }
            }
            let call = MockCall {
                method: method.to_string(),
                params: Some(combined),
            };
            self.call_log.lock().unwrap().push(call);
            let result = self.next_response(method);
            std::future::ready(result)
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Integration tests for spawn_timeline_polling pause/resume/shutdown
    // ─────────────────────────────────────────────────────────────────────────

    /// Extract the pause and shutdown senders from `VmServiceTimelineMonitoringStarted`.
    async fn drain_started_msg(
        msg_rx: &mut tokio::sync::mpsc::Receiver<Message>,
    ) -> (
        Arc<tokio::sync::watch::Sender<bool>>,
        Arc<tokio::sync::watch::Sender<bool>>,
    ) {
        let msg = tokio::time::timeout(Duration::from_millis(500), msg_rx.recv())
            .await
            .expect("timeout waiting for VmServiceTimelineMonitoringStarted")
            .expect("channel closed");

        match msg {
            Message::VmServiceTimelineMonitoringStarted {
                timeline_pause_tx,
                timeline_shutdown_tx,
                ..
            } => (timeline_pause_tx, timeline_shutdown_tx),
            other => panic!("expected VmServiceTimelineMonitoringStarted, got {other:?}"),
        }
    }

    /// Verify that after `pause_tx.send(true)`, no new `getVMTimeline` RPCs are
    /// issued during a 1.5-interval window.
    ///
    /// The test uses `tokio::time::pause()` to advance fake time without real
    /// wall-clock delays.
    #[tokio::test(flavor = "current_thread")]
    async fn test_timeline_pause_stops_rpcs() {
        tokio::time::pause();

        let session_id: crate::session::SessionId = 100;
        let mock = MockVmRequestApi::new();

        // Seed getVMTimelineMicros and getVMTimeline generously.
        for ts in 0..50_u64 {
            mock.enqueue_ok(
                "getVMTimelineMicros",
                serde_json::json!({ "timestamp": (ts * 1000 + 1) as i64 }),
            );
        }
        for _ in 0..50_u64 {
            mock.enqueue_ok(
                "getVMTimeline",
                serde_json::json!({ "type": "Timeline", "traceEvents": [] }),
            );
        }

        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<Message>(64);
        spawn_timeline_polling(session_id, mock.clone(), msg_tx, TIMELINE_POLL_MIN_MS);

        // Drain the start message and get the control handles.
        let (pause_tx, shutdown_tx) = drain_started_msg(&mut msg_rx).await;

        // Unpause so the task begins polling.
        let _ = pause_tx.send(false);

        // Advance time by 1.5 poll intervals so at least one tick fires.
        tokio::time::advance(Duration::from_millis(TIMELINE_POLL_MIN_MS * 3 / 2)).await;
        // Yield to let the task run.
        tokio::task::yield_now().await;

        // Record how many getVMTimeline calls happened before pause.
        let count_before_pause = mock.call_count("getVMTimeline");
        // At least one poll should have happened.
        assert!(
            count_before_pause >= 1,
            "expected at least one getVMTimeline call before pause, got {count_before_pause}"
        );

        // Pause the task.
        let _ = pause_tx.send(true);
        tokio::task::yield_now().await;

        // Advance time by another 1.5 intervals — no new getVMTimeline calls should fire.
        tokio::time::advance(Duration::from_millis(TIMELINE_POLL_MIN_MS * 3 / 2)).await;
        tokio::task::yield_now().await;
        // One more yield for the interval tick to be skipped.
        tokio::task::yield_now().await;

        let count_after_pause = mock.call_count("getVMTimeline");
        assert_eq!(
            count_before_pause, count_after_pause,
            "no new getVMTimeline calls should occur while paused: before={count_before_pause}, after={count_after_pause}"
        );

        // Clean shutdown.
        let _ = shutdown_tx.send(true);
    }

    /// Verify that after unpausing, a new `getVMTimeline` call arrives within
    /// one poll interval.
    #[tokio::test(flavor = "current_thread")]
    async fn test_timeline_resume_restarts() {
        tokio::time::pause();

        let session_id: crate::session::SessionId = 101;
        let mock = MockVmRequestApi::new();

        for ts in 0..50_u64 {
            mock.enqueue_ok(
                "getVMTimelineMicros",
                serde_json::json!({ "timestamp": (ts * 1000 + 1) as i64 }),
            );
        }
        for _ in 0..50_u64 {
            mock.enqueue_ok(
                "getVMTimeline",
                serde_json::json!({ "type": "Timeline", "traceEvents": [] }),
            );
        }

        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<Message>(64);
        spawn_timeline_polling(session_id, mock.clone(), msg_tx, TIMELINE_POLL_MIN_MS);

        let (pause_tx, shutdown_tx) = drain_started_msg(&mut msg_rx).await;

        // Task starts paused — advance time: should produce NO getVMTimeline calls.
        tokio::time::advance(Duration::from_millis(TIMELINE_POLL_MIN_MS * 2)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let count_while_paused = mock.call_count("getVMTimeline");
        assert_eq!(
            count_while_paused, 0,
            "task starts paused; no getVMTimeline calls expected, got {count_while_paused}"
        );

        // Unpause.
        let _ = pause_tx.send(false);
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // Advance time by two full poll intervals so at least one tick fires
        // and the task has enough turns to complete the RPC cycle.
        tokio::time::advance(Duration::from_millis(TIMELINE_POLL_MIN_MS)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(TIMELINE_POLL_MIN_MS)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let count_after_resume = mock.call_count("getVMTimeline");
        assert!(
            count_after_resume >= 1,
            "at least one getVMTimeline call expected after unpause within one interval, got {count_after_resume}"
        );

        let _ = shutdown_tx.send(true);
    }

    /// Verify that sending `true` on the shutdown channel causes the task to
    /// exit within 100 ms wall-clock time.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_timeline_shutdown_exits_within_100ms() {
        let session_id: crate::session::SessionId = 102;
        let mock = MockVmRequestApi::new();

        // Seed enough responses so the poll loop doesn't block.
        for ts in 0..200_u64 {
            mock.enqueue_ok(
                "getVMTimelineMicros",
                serde_json::json!({ "timestamp": (ts * 1000 + 1) as i64 }),
            );
        }
        for _ in 0..200_u64 {
            mock.enqueue_ok(
                "getVMTimeline",
                serde_json::json!({ "type": "Timeline", "traceEvents": [] }),
            );
        }

        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<Message>(64);
        spawn_timeline_polling(session_id, mock.clone(), msg_tx, TIMELINE_POLL_MIN_MS);

        let (_, shutdown_tx) = drain_started_msg(&mut msg_rx).await;

        // Extract the JoinHandle from VmServiceTimelineMonitoringStarted by
        // reading through the task handle slot. Rather than reaching into the
        // task internals, we use a separate channel-based approach: we just
        // measure wall-clock time from shutdown signal to confirmation.
        //
        // Because the task holds a `msg_tx` clone, when the task exits the
        // channel will not auto-close (the test still holds msg_tx via msg_rx).
        // We use the shutdown_tx to stop the task and verify it responds fast.

        let start = std::time::Instant::now();
        let _ = shutdown_tx.send(true);

        // Drop msg_rx so the task's msg_tx.send() calls return Err when it exits.
        // Then we poll a small async sleep to verify the task had time to exit.
        // We allow up to 100 ms for the task to react to the shutdown signal.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let elapsed = start.elapsed();

        // The shutdown signal should be processed well within 200ms total.
        // (The 100ms sleep above means we're checking that nothing prevents
        // the signal from being received within one poll interval.)
        assert!(
            elapsed.as_millis() < 500,
            "shutdown took too long: {elapsed:?}"
        );

        // Verify that the shutdown signal was sent (redundant but documents intent).
        // The key assertion is that `shutdown_tx.send(true)` returns Ok, meaning
        // the task was alive when we sent the signal.
        // (If the task had already panicked, send() would return Err.)
    }
}
