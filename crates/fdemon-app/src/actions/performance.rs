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
use fdemon_daemon::vm_service::VmRequestHandle;

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
                    // Skip if allocation polling is paused (Performance panel not visible).
                    // `getAllocationProfile` forces a full heap walk — the most expensive
                    // RPC. Running it only when the user is viewing the Performance panel
                    // eliminates jank in all other views (logs, inspector, network).
                    if *alloc_pause_rx.borrow() {
                        continue;
                    }

                    if fetch_and_send_alloc_profile(&handle, &msg_tx, session_id).await {
                        break;
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
                session_id, e
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
                session_id, e
            );
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
