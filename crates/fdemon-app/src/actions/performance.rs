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
//!   calls `getMemoryUsage` and `get_memory_sample`, populating both the basic
//!   gauge and the rich time-series ring buffer.
//! - Allocation tick (every `allocation_profile_interval_ms`, min
//!   [`ALLOC_PROFILE_POLL_MIN_MS`]): calls `getAllocationProfile` (expensive —
//!   forces a full heap walk), so it runs at a lower frequency than the memory tick.

use std::time::Duration;

use tokio::sync::mpsc;
use tracing::info;

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
/// 1. Calls `getMemoryUsage` → sends `VmServiceMemorySnapshot` (basic gauge).
/// 2. Calls `get_memory_sample` (combines `getMemoryUsage` + `getIsolate` RSS) →
///    sends `VmServiceMemorySample` (rich time-series). The two ring buffers stay
///    in sync because both are populated from the same tick.
///
/// **Allocation tick** (every `allocation_profile_interval_ms`, min 1000ms):
/// - Calls `getAllocationProfile` → sends `VmServiceAllocationProfileReceived`.
///   This is intentionally lower frequency than the memory tick because it is
///   expensive (forces the VM to walk the entire heap).
///
/// Transient errors from any RPC (e.g., isolate paused during hot reload) are
/// logged at debug level and skipped — the next tick will retry.
///
/// The `performance_refresh_ms` parameter controls the memory polling interval.
/// It is clamped to a minimum of [`PERF_POLL_MIN_MS`] (500ms).
///
/// The `allocation_profile_interval_ms` parameter controls the allocation profile
/// polling interval. It is clamped to a minimum of [`ALLOC_PROFILE_POLL_MIN_MS`]
/// (1000ms).
pub(super) fn spawn_performance_polling(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    performance_refresh_ms: u64,
    allocation_profile_interval_ms: u64,
) {
    // Clamp intervals to their respective minimums.
    let memory_interval = Duration::from_millis(performance_refresh_ms.max(PERF_POLL_MIN_MS));
    let alloc_interval =
        Duration::from_millis(allocation_profile_interval_ms.max(ALLOC_PROFILE_POLL_MIN_MS));

    // Create the shutdown channel outside the task so both ends are available
    // before the task starts running.
    let (perf_shutdown_tx, mut perf_shutdown_rx) = tokio::sync::watch::channel(false);
    // Arc is required because Message derives Clone and watch::Sender does not impl Clone.
    let perf_shutdown_tx = std::sync::Arc::new(perf_shutdown_tx);

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
            })
            .await
            .is_err()
        {
            // Channel closed — engine is shutting down.
            return;
        }

        let mut memory_tick = tokio::time::interval(memory_interval);
        let mut alloc_tick = tokio::time::interval(alloc_interval);

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

                    // 1. Basic memory snapshot (existing behaviour — populates memory_history).
                    match fdemon_daemon::vm_service::get_memory_usage(&handle, &isolate_id).await {
                        Ok(memory) => {
                            if msg_tx
                                .send(Message::VmServiceMemorySnapshot {
                                    session_id,
                                    memory,
                                })
                                .await
                                .is_err()
                            {
                                // Engine shutting down.
                                break;
                            }
                        }
                        Err(e) => {
                            // Transient errors are expected during hot reload when
                            // the isolate is paused. Log at debug and continue.
                            tracing::debug!(
                                "Memory usage poll failed for session {}: {}",
                                session_id, e
                            );
                            continue;
                        }
                    }

                    // 2. Rich memory sample (new — populates memory_samples ring buffer).
                    //    Shares the same tick as the basic snapshot so both ring buffers
                    //    stay in sync. If `get_memory_sample` fails (e.g. getIsolate
                    //    unavailable), the basic VmServiceMemorySnapshot still succeeded
                    //    above, so the gauge fallback remains functional.
                    if let Some(sample) =
                        fdemon_daemon::vm_service::get_memory_sample(&handle, &isolate_id).await
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
                    // Allocation profile polling (lower frequency than memory polling).
                    // `getAllocationProfile` is expensive — it forces the VM to walk the
                    // entire Dart heap. Transient failures are silently skipped.
                    let isolate_id = match handle.main_isolate_id().await {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::debug!(
                                "Could not get isolate ID for allocation polling (session {}): {}",
                                session_id, e
                            );
                            continue;
                        }
                    };

                    match fdemon_daemon::vm_service::get_allocation_profile(
                        &handle,
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
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Allocation profile poll failed for session {}: {}",
                                session_id, e
                            );
                        }
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
}
