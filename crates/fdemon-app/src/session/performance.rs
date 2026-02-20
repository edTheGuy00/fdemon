//! Performance monitoring state — memory, GC, and frame timing.

use fdemon_core::performance::{FrameTiming, GcEvent, MemoryUsage, PerformanceStats, RingBuffer};

/// Default number of memory snapshots to keep (at 2s interval = 2 minutes).
pub(crate) const DEFAULT_MEMORY_HISTORY_SIZE: usize = 60;
/// Default number of major GC events to keep.
///
/// Only major GC events (MarkSweep, MarkCompact) are stored — Scavenge events
/// are filtered out in the handler. Major GCs are rare, so 50 slots provides
/// ample history without wasting memory.
pub(crate) const DEFAULT_GC_HISTORY_SIZE: usize = 50;
/// Default number of frame timings to keep.
pub(crate) const DEFAULT_FRAME_HISTORY_SIZE: usize = 300;

/// Performance monitoring state for a session.
///
/// Holds rolling ring-buffer history for memory snapshots, GC events, and
/// frame timings, plus aggregated statistics for display.
#[derive(Debug, Clone)]
pub struct PerformanceState {
    /// Rolling history of memory snapshots.
    pub memory_history: RingBuffer<MemoryUsage>,
    /// Rolling history of GC events.
    pub gc_history: RingBuffer<GcEvent>,
    /// Rolling history of frame timings (populated by Task 06).
    pub frame_history: RingBuffer<FrameTiming>,
    /// Aggregated performance statistics (updated periodically).
    pub stats: PerformanceStats,
    /// Whether performance monitoring is active.
    pub monitoring_active: bool,
}

impl Default for PerformanceState {
    fn default() -> Self {
        Self {
            memory_history: RingBuffer::new(DEFAULT_MEMORY_HISTORY_SIZE),
            gc_history: RingBuffer::new(DEFAULT_GC_HISTORY_SIZE),
            frame_history: RingBuffer::new(DEFAULT_FRAME_HISTORY_SIZE),
            stats: PerformanceStats::default(),
            monitoring_active: false,
        }
    }
}

impl PerformanceState {
    /// Create a new [`PerformanceState`] with a configurable memory history size.
    ///
    /// The `memory_history_size` parameter controls how many memory snapshots to
    /// retain (ring buffer capacity). At the default 2-second poll interval,
    /// `60` snapshots covers 2 minutes of history.
    ///
    /// GC and frame history sizes use fixed defaults — only memory is configurable
    /// for now (see `DEFAULT_GC_HISTORY_SIZE` and `DEFAULT_FRAME_HISTORY_SIZE`).
    pub fn with_memory_history_size(memory_history_size: usize) -> Self {
        Self {
            memory_history: RingBuffer::new(memory_history_size),
            gc_history: RingBuffer::new(DEFAULT_GC_HISTORY_SIZE),
            frame_history: RingBuffer::new(DEFAULT_FRAME_HISTORY_SIZE),
            stats: PerformanceStats::default(),
            monitoring_active: false,
        }
    }
}

/// How often to recompute aggregated stats (every N frames).
///
/// At 60 FPS this produces ~6 stats updates per second — fast enough for a
/// TUI that renders at ~30 FPS. The 2-second memory poll cycle recomputes
/// stats as a backstop for when frame events are sparse.
pub(crate) const STATS_RECOMPUTE_INTERVAL: usize = 10;

/// Time window for FPS calculation (1 second).
const FPS_WINDOW: std::time::Duration = std::time::Duration::from_secs(1);

impl PerformanceState {
    /// Recompute aggregated performance statistics from the ring buffers.
    ///
    /// Called every [`STATS_RECOMPUTE_INTERVAL`] frames to avoid per-frame
    /// allocation overhead, and also from the memory-snapshot handler as a
    /// 2-second backstop.
    pub fn recompute_stats(&mut self) {
        self.stats = Self::compute_stats(&self.frame_history);
    }

    /// Compute performance statistics from frame history.
    ///
    /// Returns [`PerformanceStats::default()`] when no frames are available.
    pub fn compute_stats(frames: &RingBuffer<FrameTiming>) -> PerformanceStats {
        if frames.is_empty() {
            return PerformanceStats::default();
        }

        let frame_times: Vec<f64> = frames.iter().map(|f| f.elapsed_ms()).collect();

        let buffered_frames = frames.len() as u64;

        // FPS: compute actual frames-per-second rate from recent frame timings
        let fps = Self::calculate_fps(frames);

        // Jank count: frames exceeding 60fps budget
        let jank_count = frames.iter().filter(|f| f.is_janky()).count() as u32;

        // Average frame time (frame_times is non-empty because frames.is_empty() returned above)
        let avg_frame_ms = Some(frame_times.iter().sum::<f64>() / frame_times.len() as f64);

        // P95 frame time
        let p95_frame_ms = Self::percentile(&frame_times, 95.0);

        // Max frame time
        let max_frame_ms = frame_times.iter().copied().reduce(f64::max);

        PerformanceStats {
            fps,
            jank_count,
            avg_frame_ms,
            p95_frame_ms,
            max_frame_ms,
            buffered_frames,
        }
    }

    /// Calculate FPS from recent frame timings.
    ///
    /// Computes the actual frames-per-second rate using the timestamps of frames
    /// within the last [`FPS_WINDOW`] (1 second). Returns `None` when the app
    /// is idle or backgrounded (fewer than 2 frames in the last second).
    pub fn calculate_fps(frames: &RingBuffer<FrameTiming>) -> Option<f64> {
        if frames.len() < 2 {
            return None;
        }

        let now = chrono::Local::now();
        let window_start =
            now - chrono::Duration::from_std(FPS_WINDOW).unwrap_or(chrono::Duration::seconds(1));

        let recent: Vec<_> = frames
            .iter()
            .filter(|f| f.timestamp >= window_start)
            .collect();

        if recent.len() < 2 {
            // Fewer than 2 frames in the last second — app is idle or backgrounded.
            return None;
        }

        // Compute actual elapsed time between first and last frame in window
        let earliest = recent.iter().map(|f| f.timestamp).min()?;
        let latest = recent.iter().map(|f| f.timestamp).max()?;
        let elapsed_secs = (latest - earliest).num_milliseconds() as f64 / 1000.0;

        if elapsed_secs <= 0.0 {
            return None;
        }

        // FPS = (frame_count - 1) / elapsed_time
        // Subtract 1 because N frames span N-1 intervals
        Some((recent.len() - 1) as f64 / elapsed_secs)
    }

    /// Calculate the Nth percentile from a slice of values.
    ///
    /// Creates a sorted copy of the input — acceptable for ring buffer sizes
    /// (~300 items). Returns `None` for empty input.
    pub fn percentile(values: &[f64], pct: f64) -> Option<f64> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let index = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        Some(sorted[index.min(sorted.len() - 1)])
    }
}
