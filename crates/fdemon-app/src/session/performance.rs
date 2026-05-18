//! Performance monitoring state — frame timing and aggregated statistics.
//!
//! Memory monitoring state has moved to [`super::memory`].

use std::cell::Cell;

use fdemon_core::performance::{FrameTiming, PerformanceStats, RingBuffer};

/// 30 seconds at 60 FPS — enables meaningful scroll-back.
pub(crate) const DEFAULT_FRAME_HISTORY_SIZE: usize = 1800;

/// Active section within the Performance DevTools panel.
///
/// Used for `Tab`/`Shift+Tab` navigation between the two sub-sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerfSection {
    /// Frame timing bar chart (default section on open).
    #[default]
    FrameChart,
    /// Phase 2 anchor — the details pane. In Phase 1 Tab does not enter this
    /// section (`next()` and `prev()` return `FrameChart` unconditionally) so
    /// the variant is reserved but unreachable via user interaction.
    Details,
}

impl PerfSection {
    /// Return the next section in Tab order.
    ///
    /// Phase 1: Tab is a visible no-op — `next()` always returns `FrameChart`
    /// until Phase 2 introduces real content for `Details`.
    pub fn next(self) -> Self {
        // Phase 2 will reintroduce cycling when Details has real content.
        // For Phase 1: Tab is a visible no-op.
        PerfSection::FrameChart
    }

    /// Return the previous section in Tab order.
    ///
    /// Phase 1: `prev()` always returns `FrameChart` (mirrors `next()`).
    pub fn prev(self) -> Self {
        self.next()
    }
}

/// Performance monitoring state for a session.
///
/// Holds frame timing history and aggregated statistics for the frame chart.
/// Memory monitoring state (heap snapshots, GC events, allocation profile) has
/// moved to [`super::memory::MemoryState`].
#[derive(Debug, Clone)]
pub struct PerformanceState {
    /// Rolling history of frame timings.
    pub frame_history: RingBuffer<FrameTiming>,
    /// Aggregated performance statistics (updated periodically).
    pub stats: PerformanceStats,
    /// Whether performance monitoring is active.
    pub monitoring_active: bool,

    /// Index of the currently selected frame in `frame_history`.
    ///
    /// `None` means no frame is selected (normal scroll mode).
    /// When set, the frame bar chart highlights the frame at this index and
    /// the detail panel shows per-phase breakdown if available.
    pub selected_frame: Option<usize>,

    // ── Navigation / scroll state ─────────────────────────────────────────────
    /// Which sub-section of the Performance panel currently has keyboard focus.
    pub focused_section: PerfSection,

    /// How many frames the frame chart has been scrolled back from the live edge.
    ///
    /// `0` means the chart is at the live edge (newest frames visible).
    pub frame_chart_scroll_offset: usize,

    /// Render-hint: visible width (in columns) of the frame chart from the last rendered frame.
    ///
    /// Defaults to `0`, signalling "not yet rendered — use fallback".
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md "Region Registry Pattern" and Principle 3.
    pub frame_chart_visible_width: Cell<usize>,
}

impl Default for PerformanceState {
    fn default() -> Self {
        Self {
            frame_history: RingBuffer::new(DEFAULT_FRAME_HISTORY_SIZE),
            stats: PerformanceStats::default(),
            monitoring_active: false,
            selected_frame: None,
            focused_section: PerfSection::default(),
            frame_chart_scroll_offset: 0,
            frame_chart_visible_width: Cell::new(0),
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

    // ── PerfSection navigation ───────────────────────────────────────────────

    #[test]
    fn perf_section_next_is_noop_in_phase_1() {
        // Phase 1: Tab is a visible no-op — next() always returns FrameChart.
        assert_eq!(PerfSection::FrameChart.next(), PerfSection::FrameChart);
        assert_eq!(PerfSection::Details.next(), PerfSection::FrameChart);
    }

    #[test]
    fn perf_section_prev_is_noop_in_phase_1() {
        // Phase 1: prev() mirrors next() — always returns FrameChart.
        assert_eq!(PerfSection::FrameChart.prev(), PerfSection::FrameChart);
        assert_eq!(PerfSection::Details.prev(), PerfSection::FrameChart);
    }

    #[test]
    fn perf_section_default_is_frame_chart() {
        assert_eq!(PerfSection::default(), PerfSection::FrameChart);
    }

    // ── PerformanceState new fields: defaults ────────────────────────────────

    #[test]
    fn performance_state_defaults() {
        let s = PerformanceState::default();
        assert_eq!(s.focused_section, PerfSection::FrameChart);
        assert_eq!(s.frame_chart_scroll_offset, 0);
        assert_eq!(s.frame_chart_visible_width.get(), 0);
    }

    #[test]
    fn performance_state_frame_history_capacity_is_1800() {
        let s = PerformanceState::default();
        assert_eq!(s.frame_history.capacity(), DEFAULT_FRAME_HISTORY_SIZE);
        assert_eq!(DEFAULT_FRAME_HISTORY_SIZE, 1800);
    }

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
        let mut state = PerformanceState {
            selected_frame: Some(3),
            ..Default::default()
        };
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

    // ── Default constructor ──────────────────────────────────────────────────

    #[test]
    fn test_default_selected_frame_is_none() {
        let state = PerformanceState::default();
        assert!(state.selected_frame.is_none());
    }

    #[test]
    fn test_default_monitoring_active_is_false() {
        let state = PerformanceState::default();
        assert!(!state.monitoring_active);
    }
}
