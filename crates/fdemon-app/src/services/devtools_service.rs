//! DevTools telemetry access for external consumers
//!
//! This module provides the [`DevToolsService`] trait so remote-control
//! consumers (e.g. an MCP server giving AI agents diagnosis of a running
//! Flutter app) can read per-session DevTools telemetry — frame timings,
//! memory samples, HTTP profile entries, and the widget tree — without
//! direct access to the Engine and without any TUI interaction.
//!
//! ## How collection is gated (and how this service works around it)
//!
//! - **Frame timings** arrive passively on the VM Service Extension stream
//!   whenever the VM is connected (the per-session event forwarder always
//!   parses `Flutter.Frame` events). No TUI interaction is required; recent
//!   frames and jank stats are available as soon as the app is running in
//!   debug/profile mode.
//! - **Memory samples** come from the performance polling task, which the TUI
//!   only spawns/unpauses while DevTools mode is active. Call
//!   [`DevToolsService::start_monitoring`] to start (and keep) it running
//!   headlessly.
//! - **Network requests** come from the network polling task, which the TUI
//!   only starts when the Network panel is opened. `start_monitoring` starts
//!   it too (when the `ext.dart.io.*` extensions are available).
//! - **The widget tree** is fetched on demand. Reads come from the cache the
//!   Engine syncs from the inspector view-state; trigger a fresh fetch with
//!   [`DevToolsService::fetch_widget_tree`]. The tree is only available for
//!   the **currently selected** session — fetch results for background
//!   sessions are discarded by the TEA handler.
//!
//! Following the [`super::session_service`] precedent: cheap reads come from
//! [`SharedState`] snapshots (synced by the Engine after each TEA cycle);
//! actions are dispatched as [`Message`]s through the Engine's message
//! channel, reusing the same handler machinery as the keyboard user.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use super::state_service::SharedState;
use crate::message::Message;
use crate::session::SessionId;
use fdemon_core::network::HttpProfileEntry;
use fdemon_core::performance::{FrameTiming, MemorySample, PerformanceStats};
use fdemon_core::prelude::*;
use fdemon_core::DiagnosticsNode;

// ─────────────────────────────────────────────────────────────────────────────
// Snapshot caps (sync cost control)
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum frame timings copied into each [`DevToolsSessionSnapshot`].
///
/// 300 frames ≈ 5 seconds at 60 FPS — enough for jank diagnosis while keeping
/// the per-TEA-cycle clone cost negligible (the full per-session ring buffer
/// holds 1 800 frames).
pub const DEVTOOLS_SNAPSHOT_MAX_FRAMES: usize = 300;

/// Maximum memory samples copied into each [`DevToolsSessionSnapshot`].
///
/// 120 samples ≈ 1 minute at the default 500 ms memory poll interval.
pub const DEVTOOLS_SNAPSHOT_MAX_MEMORY_SAMPLES: usize = 120;

/// Maximum HTTP profile entries copied into each [`DevToolsSessionSnapshot`].
///
/// 100 most-recent requests; the per-session buffer holds up to 500.
pub const DEVTOOLS_SNAPSHOT_MAX_NETWORK_ENTRIES: usize = 100;

/// Poll interval used by [`SharedDevToolsService::fetch_widget_tree`] while
/// waiting for the Engine to sync a fresh tree into [`SharedState`].
const WIDGET_TREE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Freshness window for the cached widget tree.
///
/// Mirrors the 2-second `RequestWidgetTree` cooldown in
/// `InspectorState::is_fetch_debounced`: a dispatch inside this window would
/// be debounced by the handler anyway, so a cached tree younger than this is
/// returned immediately.
const WIDGET_TREE_FRESHNESS: Duration = Duration::from_secs(2);

// ─────────────────────────────────────────────────────────────────────────────
// Snapshot types
// ─────────────────────────────────────────────────────────────────────────────

/// Point-in-time view of one session's DevTools telemetry.
///
/// Synced from the per-session ring buffers (`session.performance`,
/// `session.memory`, `session.network`) into [`SharedState::devtools`] by the
/// Engine after each message-processing cycle. Telemetry vectors are capped by
/// the `DEVTOOLS_SNAPSHOT_MAX_*` constants (most recent entries are kept).
#[derive(Debug, Clone)]
pub struct DevToolsSessionSnapshot {
    pub session_id: SessionId,
    /// Whether the VM Service WebSocket is connected for this session.
    pub vm_connected: bool,
    /// Whether the performance polling task (memory sampling) is running.
    ///
    /// Note: the task may still be *paused*; use
    /// [`DevToolsService::start_monitoring`] to ensure it is running and
    /// unpaused.
    pub perf_monitoring_active: bool,
    /// Whether the network polling task is running.
    pub network_monitoring_active: bool,
    /// Whether the `ext.dart.io.*` network extensions are available
    /// (`None` = not probed yet, `Some(false)` = release mode).
    pub network_extensions_available: Option<bool>,
    /// Aggregated frame statistics (FPS, jank count, avg/p95/max frame time).
    pub stats: PerformanceStats,
    /// Most recent frame timings (oldest first), capped at
    /// [`DEVTOOLS_SNAPSHOT_MAX_FRAMES`].
    pub recent_frames: Vec<FrameTiming>,
    /// Most recent memory samples (oldest first), capped at
    /// [`DEVTOOLS_SNAPSHOT_MAX_MEMORY_SAMPLES`].
    pub memory_samples: Vec<MemorySample>,
    /// Most recent HTTP request summaries (oldest first), capped at
    /// [`DEVTOOLS_SNAPSHOT_MAX_NETWORK_ENTRIES`].
    pub network_requests: Vec<HttpProfileEntry>,
}

/// Recent frame timings together with the aggregated jank statistics.
#[derive(Debug, Clone)]
pub struct PerformanceFramesSnapshot {
    /// Most recent frame timings (oldest first).
    pub frames: Vec<FrameTiming>,
    /// Aggregated statistics over the session's full frame history.
    pub stats: PerformanceStats,
}

/// Cached widget tree for the currently selected session.
///
/// Synced from `AppState::devtools_view_state.inspector` by the Engine. The
/// tree is wrapped in an [`Arc`] and re-cloned into [`SharedState`] only when
/// a fetch starts/completes, so steady-state sync cost is one pointer compare.
#[derive(Debug, Clone)]
pub struct WidgetTreeSnapshot {
    /// The session the inspector view-state currently tracks (the selected one).
    pub session_id: SessionId,
    /// When the most recent fetch was *started* (`None` = never fetched).
    pub fetched_at: Option<Instant>,
    /// Whether a fetch is currently in flight.
    pub loading: bool,
    /// Error message from the most recent failed fetch.
    pub error: Option<String>,
    /// Root of the widget tree, if a fetch has completed successfully.
    pub root: Option<Arc<DiagnosticsNode>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Service trait
// ─────────────────────────────────────────────────────────────────────────────

/// Per-session DevTools telemetry reads and monitoring control
///
/// Both TUI-side consumers and external embedders (MCP handlers) use this
/// trait. All methods are headless-usable: none require the TUI user to enter
/// DevTools mode.
#[trait_variant::make(DevToolsService: Send)]
pub trait LocalDevToolsService {
    /// Telemetry snapshots for all active sessions.
    async fn list_snapshots(&self) -> Vec<DevToolsSessionSnapshot>;

    /// Telemetry snapshot for one session (`None` = unknown session id).
    async fn devtools_snapshot(&self, session_id: SessionId) -> Option<DevToolsSessionSnapshot>;

    /// Recent frame timings plus aggregated jank stats for one session.
    ///
    /// Frames are collected passively while the VM Service is connected, so
    /// this works without `start_monitoring`.
    async fn performance_frames(&self, session_id: SessionId) -> Option<PerformanceFramesSnapshot>;

    /// Recent memory samples for one session (oldest first).
    ///
    /// Empty until [`Self::start_monitoring`] (or the TUI user entering
    /// DevTools mode) starts the memory polling task.
    async fn memory_samples(&self, session_id: SessionId) -> Option<Vec<MemorySample>>;

    /// Recent HTTP request summaries for one session (oldest first).
    ///
    /// Empty until [`Self::start_monitoring`] (or the TUI user opening the
    /// Network panel) starts the network polling task.
    async fn network_requests(&self, session_id: SessionId) -> Option<Vec<HttpProfileEntry>>;

    /// Start (and keep) DevTools telemetry collection for a session.
    ///
    /// Dispatches [`Message::StartDevToolsMonitoring`] through the Engine's
    /// message channel. Success means the request was queued; the polling
    /// tasks spawn asynchronously (immediately when the VM is connected,
    /// otherwise on the next `VmServiceConnected`). While active, the TUI's
    /// pause-on-DevTools-exit paths leave polling running.
    async fn start_monitoring(&self, session_id: SessionId) -> Result<()>;

    /// Stop service-level telemetry collection for a session.
    ///
    /// Polling is paused unless the TUI user is actively viewing that session
    /// in DevTools mode.
    async fn stop_monitoring(&self, session_id: SessionId) -> Result<()>;

    /// The cached widget tree, if it belongs to `session_id`.
    ///
    /// The inspector cache tracks the **selected** session only; `None` is
    /// returned for background sessions or when no fetch has completed yet.
    async fn cached_widget_tree(&self, session_id: SessionId) -> Option<Arc<DiagnosticsNode>>;

    /// Trigger a widget tree fetch without waiting for the result.
    ///
    /// Dispatches [`Message::RequestWidgetTree`]; the handler debounces
    /// requests within 2 seconds of the previous fetch. Read the result later
    /// via [`Self::cached_widget_tree`].
    async fn request_widget_tree(&self, session_id: SessionId) -> Result<()>;

    /// Fetch the widget tree and await the result.
    ///
    /// Contract:
    /// 1. If a cached tree for `session_id` is younger than the handler's
    ///    2-second fetch cooldown, it is returned immediately (a dispatch
    ///    would be debounced anyway).
    /// 2. Otherwise [`Message::RequestWidgetTree`] is dispatched and the
    ///    synced cache is polled every 50 ms until a fresh tree (or a fetch
    ///    error) appears, or `timeout` elapses.
    ///
    /// Requirements: the session must be the **selected** session with a
    /// connected VM Service running in debug mode — otherwise the fetch
    /// result never reaches the cache and this returns a timeout error.
    async fn fetch_widget_tree(
        &self,
        session_id: SessionId,
        timeout: Duration,
    ) -> Result<Arc<DiagnosticsNode>>;
}

// ─────────────────────────────────────────────────────────────────────────────
// SharedState-backed implementation
// ─────────────────────────────────────────────────────────────────────────────

/// Implementation backed by [`SharedState`] snapshots and the Engine's
/// message channel. Cheap to construct and `Send + 'static`, so it can be
/// moved into spawned tokio tasks.
pub struct SharedDevToolsService {
    state: Arc<SharedState>,
    msg_tx: mpsc::Sender<Message>,
}

impl SharedDevToolsService {
    pub fn new(state: Arc<SharedState>, msg_tx: mpsc::Sender<Message>) -> Self {
        Self { state, msg_tx }
    }

    /// Read the widget-tree cache if it tracks `session_id`.
    async fn widget_tree_slot(&self, session_id: SessionId) -> Option<WidgetTreeSnapshot> {
        self.state
            .widget_tree
            .read()
            .await
            .as_ref()
            .filter(|slot| slot.session_id == session_id)
            .cloned()
    }

    /// Read the synced telemetry snapshot for one session.
    ///
    /// Inherent helper so trait methods can share it without ambiguous
    /// `self.devtools_snapshot(..)` calls (both trait variants would apply).
    async fn snapshot_for(&self, session_id: SessionId) -> Option<DevToolsSessionSnapshot> {
        self.state
            .devtools
            .read()
            .await
            .iter()
            .find(|s| s.session_id == session_id)
            .cloned()
    }

    /// Dispatch a message through the Engine's channel.
    async fn dispatch(&self, message: Message, what: &'static str) -> Result<()> {
        self.msg_tx
            .send(message)
            .await
            .map_err(|_| Error::channel_send(what))
    }
}

impl DevToolsService for SharedDevToolsService {
    async fn list_snapshots(&self) -> Vec<DevToolsSessionSnapshot> {
        self.state.devtools.read().await.clone()
    }

    async fn devtools_snapshot(&self, session_id: SessionId) -> Option<DevToolsSessionSnapshot> {
        self.snapshot_for(session_id).await
    }

    async fn performance_frames(&self, session_id: SessionId) -> Option<PerformanceFramesSnapshot> {
        self.snapshot_for(session_id)
            .await
            .map(|s| PerformanceFramesSnapshot {
                frames: s.recent_frames,
                stats: s.stats,
            })
    }

    async fn memory_samples(&self, session_id: SessionId) -> Option<Vec<MemorySample>> {
        self.snapshot_for(session_id)
            .await
            .map(|s| s.memory_samples)
    }

    async fn network_requests(&self, session_id: SessionId) -> Option<Vec<HttpProfileEntry>> {
        self.snapshot_for(session_id)
            .await
            .map(|s| s.network_requests)
    }

    async fn start_monitoring(&self, session_id: SessionId) -> Result<()> {
        self.dispatch(
            Message::StartDevToolsMonitoring { session_id },
            "start devtools monitoring command",
        )
        .await
    }

    async fn stop_monitoring(&self, session_id: SessionId) -> Result<()> {
        self.dispatch(
            Message::StopDevToolsMonitoring { session_id },
            "stop devtools monitoring command",
        )
        .await
    }

    async fn cached_widget_tree(&self, session_id: SessionId) -> Option<Arc<DiagnosticsNode>> {
        self.widget_tree_slot(session_id).await.and_then(|s| s.root)
    }

    async fn request_widget_tree(&self, session_id: SessionId) -> Result<()> {
        self.dispatch(
            Message::RequestWidgetTree { session_id },
            "request widget tree command",
        )
        .await
    }

    async fn fetch_widget_tree(
        &self,
        session_id: SessionId,
        timeout: Duration,
    ) -> Result<Arc<DiagnosticsNode>> {
        // Fast path: a fetch completed within the handler's cooldown window —
        // a new dispatch would be debounced, so return the cached tree.
        let baseline = self.widget_tree_slot(session_id).await;
        if let Some(ref slot) = baseline {
            if !slot.loading {
                if let (Some(at), Some(root)) = (slot.fetched_at, slot.root.as_ref()) {
                    if at.elapsed() < WIDGET_TREE_FRESHNESS {
                        return Ok(root.clone());
                    }
                }
            }
        }
        let baseline_fetched_at = baseline.and_then(|s| s.fetched_at);

        self.dispatch(
            Message::RequestWidgetTree { session_id },
            "request widget tree command",
        )
        .await?;

        let deadline = Instant::now() + timeout;
        loop {
            tokio::time::sleep(WIDGET_TREE_POLL_INTERVAL).await;

            if let Some(slot) = self.widget_tree_slot(session_id).await {
                // A new fetch has started (fetched_at advanced) and finished
                // (loading cleared): report its outcome.
                if !slot.loading && slot.fetched_at != baseline_fetched_at {
                    if let Some(error) = slot.error {
                        return Err(Error::vm_service(format!(
                            "widget tree fetch failed: {error}"
                        )));
                    }
                    if let Some(root) = slot.root {
                        return Ok(root);
                    }
                }
            }

            if Instant::now() >= deadline {
                return Err(Error::vm_service(format!(
                    "widget tree fetch timed out after {timeout:?} — the session must be \
                     the selected one with a connected VM Service in debug mode"
                )));
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // Import only the Send-variant trait: `SharedDevToolsService` implements
    // both variants (Local via the trait_variant blanket impl), so importing
    // both would make plain method-call syntax ambiguous.
    use super::{
        DevToolsService, DevToolsSessionSnapshot, SharedDevToolsService, WidgetTreeSnapshot,
    };
    use crate::message::Message;
    use crate::services::SharedState;
    use crate::session::SessionId;
    use fdemon_core::performance::{FrameTiming, MemorySample, PerformanceStats};
    use fdemon_core::DiagnosticsNode;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc;

    fn frame(number: u64) -> FrameTiming {
        FrameTiming {
            number,
            build_micros: 5_000,
            raster_micros: 5_000,
            elapsed_micros: 10_000,
            timestamp: chrono::Local::now(),
            phases: None,
            shader_compilation: false,
        }
    }

    fn memory_sample() -> MemorySample {
        MemorySample {
            dart_heap: 1024,
            dart_native: 64,
            raster_cache: 0,
            allocated: 2048,
            rss: 0,
            timestamp: chrono::Local::now(),
        }
    }

    fn snapshot(session_id: SessionId) -> DevToolsSessionSnapshot {
        DevToolsSessionSnapshot {
            session_id,
            vm_connected: true,
            perf_monitoring_active: true,
            network_monitoring_active: false,
            network_extensions_available: Some(true),
            stats: PerformanceStats::default(),
            recent_frames: vec![frame(1), frame(2)],
            memory_samples: vec![memory_sample()],
            network_requests: Vec::new(),
        }
    }

    fn create_service() -> (
        SharedDevToolsService,
        mpsc::Receiver<Message>,
        Arc<SharedState>,
    ) {
        let state = Arc::new(SharedState::new(100));
        let (tx, rx) = mpsc::channel(10);
        let service = SharedDevToolsService::new(state.clone(), tx);
        (service, rx, state)
    }

    fn tree_root(description: &str) -> Arc<DiagnosticsNode> {
        Arc::new(DiagnosticsNode {
            description: description.to_string(),
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn test_list_snapshots_empty_by_default() {
        let (service, _rx, _state) = create_service();
        assert!(service.list_snapshots().await.is_empty());
    }

    #[tokio::test]
    async fn test_devtools_snapshot_returns_synced_data() {
        let (service, _rx, state) = create_service();
        *state.devtools.write().await = vec![snapshot(1), snapshot(2)];

        let snap = service.devtools_snapshot(2).await.unwrap();
        assert_eq!(snap.session_id, 2);
        assert!(snap.vm_connected);
        assert_eq!(snap.recent_frames.len(), 2);

        assert!(service.devtools_snapshot(99).await.is_none());
    }

    #[tokio::test]
    async fn test_performance_frames_returns_frames_and_stats() {
        let (service, _rx, state) = create_service();
        let mut snap = snapshot(1);
        snap.stats.jank_count = 3;
        *state.devtools.write().await = vec![snap];

        let perf = service.performance_frames(1).await.unwrap();
        assert_eq!(perf.frames.len(), 2);
        assert_eq!(perf.frames[0].number, 1);
        assert_eq!(perf.stats.jank_count, 3);

        assert!(service.performance_frames(99).await.is_none());
    }

    #[tokio::test]
    async fn test_memory_samples_and_network_requests_reads() {
        let (service, _rx, state) = create_service();
        *state.devtools.write().await = vec![snapshot(7)];

        assert_eq!(service.memory_samples(7).await.unwrap().len(), 1);
        assert!(service.network_requests(7).await.unwrap().is_empty());
        assert!(service.memory_samples(8).await.is_none());
        assert!(service.network_requests(8).await.is_none());
    }

    #[tokio::test]
    async fn test_start_monitoring_sends_message() {
        let (service, mut rx, _state) = create_service();

        service.start_monitoring(5).await.unwrap();

        match rx.try_recv().unwrap() {
            Message::StartDevToolsMonitoring { session_id } => assert_eq!(session_id, 5),
            other => panic!("expected StartDevToolsMonitoring, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_stop_monitoring_sends_message() {
        let (service, mut rx, _state) = create_service();

        service.stop_monitoring(5).await.unwrap();

        match rx.try_recv().unwrap() {
            Message::StopDevToolsMonitoring { session_id } => assert_eq!(session_id, 5),
            other => panic!("expected StopDevToolsMonitoring, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_dispatch_with_closed_channel_returns_error() {
        let (service, rx, _state) = create_service();
        drop(rx);

        assert!(service.start_monitoring(1).await.is_err());
        assert!(service.stop_monitoring(1).await.is_err());
        assert!(service.request_widget_tree(1).await.is_err());
    }

    #[tokio::test]
    async fn test_request_widget_tree_sends_message() {
        let (service, mut rx, _state) = create_service();

        service.request_widget_tree(3).await.unwrap();

        match rx.try_recv().unwrap() {
            Message::RequestWidgetTree { session_id } => assert_eq!(session_id, 3),
            other => panic!("expected RequestWidgetTree, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cached_widget_tree_filters_by_session() {
        let (service, _rx, state) = create_service();
        *state.widget_tree.write().await = Some(WidgetTreeSnapshot {
            session_id: 1,
            fetched_at: Some(Instant::now()),
            loading: false,
            error: None,
            root: Some(tree_root("RootWidget")),
        });

        let root = service.cached_widget_tree(1).await.unwrap();
        assert_eq!(root.description, "RootWidget");
        assert!(
            service.cached_widget_tree(2).await.is_none(),
            "cache for session 1 must not be returned for session 2"
        );
    }

    #[tokio::test]
    async fn test_fetch_widget_tree_fast_path_returns_fresh_cache_without_dispatch() {
        let (service, mut rx, state) = create_service();
        *state.widget_tree.write().await = Some(WidgetTreeSnapshot {
            session_id: 1,
            fetched_at: Some(Instant::now()), // fresh — inside cooldown window
            loading: false,
            error: None,
            root: Some(tree_root("FreshRoot")),
        });

        let root = service
            .fetch_widget_tree(1, Duration::from_millis(500))
            .await
            .unwrap();

        assert_eq!(root.description, "FreshRoot");
        assert!(
            rx.try_recv().is_err(),
            "fresh cache must short-circuit without dispatching RequestWidgetTree"
        );
    }

    #[tokio::test]
    async fn test_fetch_widget_tree_awaits_synced_result() {
        let (service, mut rx, state) = create_service();

        // Simulate the Engine: consume the RequestWidgetTree dispatch, then
        // sync a completed fetch into SharedState.
        let responder_state = state.clone();
        let responder = tokio::spawn(async move {
            let msg = rx.recv().await.expect("dispatch expected");
            assert!(matches!(msg, Message::RequestWidgetTree { session_id: 4 }));
            *responder_state.widget_tree.write().await = Some(WidgetTreeSnapshot {
                session_id: 4,
                fetched_at: Some(Instant::now()),
                loading: false,
                error: None,
                root: Some(tree_root("FetchedRoot")),
            });
        });

        let root = service
            .fetch_widget_tree(4, Duration::from_secs(2))
            .await
            .unwrap();

        assert_eq!(root.description, "FetchedRoot");
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn test_fetch_widget_tree_surfaces_fetch_error() {
        let (service, mut rx, state) = create_service();

        let responder_state = state.clone();
        let responder = tokio::spawn(async move {
            let _ = rx.recv().await;
            *responder_state.widget_tree.write().await = Some(WidgetTreeSnapshot {
                session_id: 4,
                fetched_at: Some(Instant::now()),
                loading: false,
                error: Some("isolate not found".to_string()),
                root: None,
            });
        });

        let err = service
            .fetch_widget_tree(4, Duration::from_secs(2))
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("isolate not found"),
            "error must carry the fetch failure reason, got: {err}"
        );
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn test_fetch_widget_tree_times_out_when_no_result_arrives() {
        let (service, mut rx, _state) = create_service();

        // Consume the dispatch but never sync a result.
        let responder = tokio::spawn(async move {
            let _ = rx.recv().await;
            // Keep the receiver alive long enough for the fetch to time out.
            tokio::time::sleep(Duration::from_millis(300)).await;
        });

        let err = service
            .fetch_widget_tree(1, Duration::from_millis(150))
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("timed out"),
            "expected timeout error, got: {err}"
        );
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn test_devtools_service_usable_from_spawned_task() {
        let (service, mut rx, _state) = create_service();

        let handle = tokio::spawn(async move { service.start_monitoring(1).await });
        handle.await.unwrap().unwrap();

        assert!(matches!(
            rx.try_recv().unwrap(),
            Message::StartDevToolsMonitoring { .. }
        ));
    }
}
