//! Performance monitoring state — memory, GC, and frame timing.

use fdemon_core::performance::{
    AllocationProfile, FrameTiming, GcEvent, MemorySample, MemoryUsage, PerformanceStats,
    RingBuffer,
};

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
/// Memory sample buffer size: 120 samples at 500ms polling = 60 seconds of history.
pub(crate) const DEFAULT_MEMORY_SAMPLE_SIZE: usize = 120;

/// Column by which the class allocation table is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllocationSortColumn {
    /// Sort by total allocated bytes (descending).
    #[default]
    BySize,
    /// Sort by total instance count (descending).
    ByInstances,
}

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

    /// Rich memory samples for time-series chart (populated by VM service polling).
    ///
    /// Each entry contains a full breakdown (Dart heap, native, raster cache, RSS)
    /// at 500ms polling. The buffer holds 120 samples = 60 seconds of history.
    /// Runs in parallel with `memory_history` — the older `memory_history` is kept
    /// as a fallback when rich sample data is unavailable.
    pub memory_samples: RingBuffer<MemorySample>,

    /// Index of the currently selected frame in `frame_history`.
    ///
    /// `None` means no frame is selected (normal scroll mode).
    /// When set, the frame bar chart highlights the frame at this index and
    /// the detail panel shows per-phase breakdown if available.
    pub selected_frame: Option<usize>,

    /// Latest allocation profile snapshot from `getAllocationProfile`.
    ///
    /// `None` until the first profile is fetched or when monitoring is inactive.
    /// Replaced on each fetch — only the most recent snapshot is retained.
    pub allocation_profile: Option<AllocationProfile>,

    /// Column by which the class allocation table is sorted.
    pub allocation_sort: AllocationSortColumn,
}

impl Default for PerformanceState {
    fn default() -> Self {
        Self {
            memory_history: RingBuffer::new(DEFAULT_MEMORY_HISTORY_SIZE),
            gc_history: RingBuffer::new(DEFAULT_GC_HISTORY_SIZE),
            frame_history: RingBuffer::new(DEFAULT_FRAME_HISTORY_SIZE),
            stats: PerformanceStats::default(),
            monitoring_active: false,
            memory_samples: RingBuffer::new(DEFAULT_MEMORY_SAMPLE_SIZE),
            selected_frame: None,
            allocation_profile: None,
            allocation_sort: AllocationSortColumn::default(),
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
    /// The `memory_samples` buffer always uses [`DEFAULT_MEMORY_SAMPLE_SIZE`].
    pub fn with_memory_history_size(memory_history_size: usize) -> Self {
        Self {
            memory_history: RingBuffer::new(memory_history_size),
            gc_history: RingBuffer::new(DEFAULT_GC_HISTORY_SIZE),
            frame_history: RingBuffer::new(DEFAULT_FRAME_HISTORY_SIZE),
            stats: PerformanceStats::default(),
            monitoring_active: false,
            memory_samples: RingBuffer::new(DEFAULT_MEMORY_SAMPLE_SIZE),
            selected_frame: None,
            allocation_profile: None,
            allocation_sort: AllocationSortColumn::default(),
        }
    }
}

impl PerformanceState {
    /// Compute the index of the previous frame without mutating state.
    ///
    /// Returns `None` when the frame history is empty.
    /// When no frame is selected, returns the index of the most recent frame (`len - 1`).
    /// When already at index 0, clamps and returns `Some(0)`.
    pub fn compute_prev_frame_index(&self) -> Option<usize> {
        let len = self.frame_history.len();
        if len == 0 {
            return None;
        }
        Some(match self.selected_frame {
            Some(i) if i > 0 => i - 1,
            Some(_) => 0,    // already at first frame, stay
            None => len - 1, // nothing selected, select most recent
        })
    }

    /// Compute the index of the next frame without mutating state.
    ///
    /// Returns `None` when the frame history is empty.
    /// When no frame is selected, returns the index of the most recent frame (`len - 1`).
    /// When already at the last frame, clamps and returns `Some(i)`.
    pub fn compute_next_frame_index(&self) -> Option<usize> {
        let len = self.frame_history.len();
        if len == 0 {
            return None;
        }
        Some(match self.selected_frame {
            Some(i) if i + 1 < len => i + 1,
            Some(i) => i,    // already at last frame, stay
            None => len - 1, // nothing selected, select most recent
        })
    }

    /// Select the next frame (Right arrow). Clamps at the end when already at the last frame.
    ///
    /// When no frame is selected, selects the most recent frame (index `len - 1`).
    pub fn select_next_frame(&mut self) {
        self.selected_frame = self.compute_next_frame_index();
    }

    /// Select the previous frame (Left arrow). Clamps at the start when already at index 0.
    ///
    /// When no frame is selected, selects the most recent frame (index `len - 1`).
    pub fn select_prev_frame(&mut self) {
        self.selected_frame = self.compute_prev_frame_index();
    }

    /// Deselect any selected frame (Esc). Returns to normal scroll mode.
    pub fn deselect_frame(&mut self) {
        self.selected_frame = None;
    }

    /// Get the currently selected frame timing, if any.
    ///
    /// Returns `None` if no frame is selected or if the index is out of bounds.
    pub fn selected_frame_timing(&self) -> Option<&FrameTiming> {
        self.selected_frame
            .and_then(|i| self.frame_history.iter().nth(i))
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::performance::FrameTiming;

    // ── Test helper ─────────────────────────────────────────────────────────

    /// Push `count` synthetic frame timings into `state.frame_history`.
    ///
    /// Frames are numbered 1..=count with 10ms elapsed each.
    fn push_test_frames(state: &mut PerformanceState, count: u64) {
        for i in 1..=count {
            state.frame_history.push(FrameTiming {
                number: i,
                build_micros: 5_000,
                raster_micros: 5_000,
                elapsed_micros: 10_000,
                timestamp: chrono::Local::now(),
                phases: None,
                shader_compilation: false,
            });
        }
    }

    // ── Frame selection: select_next_frame ──────────────────────────────────

    #[test]
    fn test_select_next_frame_from_none_selects_most_recent() {
        let mut state = PerformanceState::default();
        push_test_frames(&mut state, 5);
        state.select_next_frame();
        assert_eq!(state.selected_frame, Some(4)); // 0-based index of 5th frame
    }

    #[test]
    fn test_select_next_frame_increments() {
        let mut state = PerformanceState::default();
        push_test_frames(&mut state, 5);
        state.selected_frame = Some(2);
        state.select_next_frame();
        assert_eq!(state.selected_frame, Some(3));
    }

    #[test]
    fn test_select_next_frame_clamps_at_end() {
        let mut state = PerformanceState::default();
        push_test_frames(&mut state, 5);
        state.selected_frame = Some(4);
        state.select_next_frame();
        assert_eq!(state.selected_frame, Some(4)); // already at last, stays clamped
    }

    #[test]
    fn test_select_next_frame_empty_history_noop() {
        let mut state = PerformanceState::default();
        state.select_next_frame();
        assert_eq!(state.selected_frame, None);
    }

    // ── Frame selection: select_prev_frame ──────────────────────────────────

    #[test]
    fn test_select_prev_frame_from_none_selects_most_recent() {
        let mut state = PerformanceState::default();
        push_test_frames(&mut state, 5);
        state.select_prev_frame();
        assert_eq!(state.selected_frame, Some(4)); // most recent when None
    }

    #[test]
    fn test_select_prev_frame_decrements() {
        let mut state = PerformanceState::default();
        push_test_frames(&mut state, 5);
        state.selected_frame = Some(3);
        state.select_prev_frame();
        assert_eq!(state.selected_frame, Some(2));
    }

    #[test]
    fn test_select_prev_frame_clamps_at_start() {
        let mut state = PerformanceState::default();
        push_test_frames(&mut state, 5);
        state.selected_frame = Some(0);
        state.select_prev_frame();
        assert_eq!(state.selected_frame, Some(0)); // already at start, stays clamped
    }

    #[test]
    fn test_select_prev_frame_empty_history_noop() {
        let mut state = PerformanceState::default();
        state.select_prev_frame();
        assert_eq!(state.selected_frame, None);
    }

    // ── Pure computation: compute_prev_frame_index ──────────────────────────

    #[test]
    fn test_compute_prev_frame_index_from_middle() {
        let mut perf = PerformanceState::default();
        push_test_frames(&mut perf, 10);
        perf.selected_frame = Some(5);
        assert_eq!(perf.compute_prev_frame_index(), Some(4));
    }

    #[test]
    fn test_compute_prev_frame_index_at_start() {
        let mut perf = PerformanceState::default();
        push_test_frames(&mut perf, 10);
        perf.selected_frame = Some(0);
        assert_eq!(perf.compute_prev_frame_index(), Some(0)); // clamp at 0
    }

    #[test]
    fn test_compute_prev_frame_index_none_selects_newest() {
        let mut perf = PerformanceState::default();
        push_test_frames(&mut perf, 10);
        perf.selected_frame = None;
        assert_eq!(perf.compute_prev_frame_index(), Some(9));
    }

    #[test]
    fn test_compute_prev_frame_index_empty_returns_none() {
        let perf = PerformanceState::default();
        assert_eq!(perf.compute_prev_frame_index(), None);
    }

    // ── Pure computation: compute_next_frame_index ──────────────────────────

    #[test]
    fn test_compute_next_frame_index_from_middle() {
        let mut perf = PerformanceState::default();
        push_test_frames(&mut perf, 10);
        perf.selected_frame = Some(5);
        assert_eq!(perf.compute_next_frame_index(), Some(6));
    }

    #[test]
    fn test_compute_next_frame_index_at_end() {
        let mut perf = PerformanceState::default();
        push_test_frames(&mut perf, 10);
        perf.selected_frame = Some(9);
        assert_eq!(perf.compute_next_frame_index(), Some(9)); // clamp at end
    }

    #[test]
    fn test_compute_next_frame_index_none_selects_newest() {
        let mut perf = PerformanceState::default();
        push_test_frames(&mut perf, 10);
        perf.selected_frame = None;
        assert_eq!(perf.compute_next_frame_index(), Some(9));
    }

    #[test]
    fn test_compute_next_frame_index_empty_returns_none() {
        let perf = PerformanceState::default();
        assert_eq!(perf.compute_next_frame_index(), None);
    }

    // ── Frame selection: deselect_frame ────────────────────────────────────

    #[test]
    fn test_deselect_frame_clears_selection() {
        let mut state = PerformanceState::default();
        state.selected_frame = Some(3);
        state.deselect_frame();
        assert_eq!(state.selected_frame, None);
    }

    #[test]
    fn test_deselect_frame_when_none_is_noop() {
        let mut state = PerformanceState::default();
        state.deselect_frame();
        assert_eq!(state.selected_frame, None);
    }

    // ── Frame selection: selected_frame_timing ─────────────────────────────

    #[test]
    fn test_selected_frame_timing_returns_correct_frame() {
        let mut state = PerformanceState::default();
        push_test_frames(&mut state, 3);
        state.selected_frame = Some(1);
        let timing = state.selected_frame_timing().unwrap();
        // push_test_frames assigns number = i (1-based), so index 1 → number 2
        assert_eq!(timing.number, 2);
    }

    #[test]
    fn test_selected_frame_timing_returns_none_when_no_selection() {
        let mut state = PerformanceState::default();
        push_test_frames(&mut state, 3);
        assert!(state.selected_frame_timing().is_none());
    }

    #[test]
    fn test_selected_frame_timing_returns_none_on_empty_history() {
        let state = PerformanceState::default();
        assert!(state.selected_frame_timing().is_none());
    }

    // ── Memory samples ring buffer ──────────────────────────────────────────

    #[test]
    fn test_memory_samples_ring_buffer_default_capacity() {
        let state = PerformanceState::default();
        assert_eq!(state.memory_samples.capacity(), DEFAULT_MEMORY_SAMPLE_SIZE);
    }

    #[test]
    fn test_memory_samples_ring_buffer_default_capacity_is_120() {
        assert_eq!(DEFAULT_MEMORY_SAMPLE_SIZE, 120);
    }

    // ── AllocationSortColumn defaults ──────────────────────────────────────

    #[test]
    fn test_allocation_sort_default_is_by_size() {
        let state = PerformanceState::default();
        assert_eq!(state.allocation_sort, AllocationSortColumn::BySize);
    }

    #[test]
    fn test_allocation_sort_column_default_trait() {
        assert_eq!(
            AllocationSortColumn::default(),
            AllocationSortColumn::BySize
        );
    }

    // ── Constructor: with_memory_history_size ──────────────────────────────

    #[test]
    fn test_with_memory_history_size_sets_memory_history_capacity() {
        let state = PerformanceState::with_memory_history_size(30);
        assert_eq!(state.memory_history.capacity(), 30);
    }

    #[test]
    fn test_with_memory_history_size_memory_samples_uses_default() {
        let state = PerformanceState::with_memory_history_size(30);
        assert_eq!(state.memory_samples.capacity(), DEFAULT_MEMORY_SAMPLE_SIZE);
    }

    #[test]
    fn test_with_memory_history_size_selected_frame_is_none() {
        let state = PerformanceState::with_memory_history_size(30);
        assert!(state.selected_frame.is_none());
    }

    #[test]
    fn test_with_memory_history_size_allocation_profile_is_none() {
        let state = PerformanceState::with_memory_history_size(30);
        assert!(state.allocation_profile.is_none());
    }

    #[test]
    fn test_with_memory_history_size_allocation_sort_is_by_size() {
        let state = PerformanceState::with_memory_history_size(30);
        assert_eq!(state.allocation_sort, AllocationSortColumn::BySize);
    }

    // ── Default constructor ──────────────────────────────────────────────────

    #[test]
    fn test_default_selected_frame_is_none() {
        let state = PerformanceState::default();
        assert!(state.selected_frame.is_none());
    }

    #[test]
    fn test_default_allocation_profile_is_none() {
        let state = PerformanceState::default();
        assert!(state.allocation_profile.is_none());
    }
}
