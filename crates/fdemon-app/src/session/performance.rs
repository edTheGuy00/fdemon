//! Performance monitoring state — frame timing and aggregated statistics.
//!
//! Memory monitoring state has moved to [`super::memory`].

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, VecDeque};

use fdemon_core::performance::{FrameTiming, PerformanceStats, RingBuffer};
use fdemon_core::rebuild_stats::{LocationMap, RebuildStatsSnapshot};
use fdemon_core::timeline::TimelineTrack;
use serde::{Deserialize, Serialize};

use crate::state::PerfDetailsTab;

// ── TimelineEventCursor ───────────────────────────────────────────────────────

/// Identifies a single event in the timeline tree.
///
/// Stable across batches as long as the event survives the ring-buffer eviction
/// policy (oldest root events are evicted first, by `ts`). When eviction removes
/// the pointed-to event, the selection is cleared and a debug log entry is
/// emitted.
///
/// The triple `(tid, depth, ts)` uniquely identifies an event because:
/// - `tid` identifies the thread track.
/// - `depth` is the nesting level within the root event subtree.
/// - `ts` (start timestamp in micros) disambiguates siblings at the same depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimelineEventCursor {
    /// Thread id of the track that owns this event.
    pub tid: i64,
    /// Nesting depth (0 = root event, 1 = direct child, …).
    pub depth: u8,
    /// Event start timestamp in microseconds. Disambiguates siblings.
    pub ts: i64,
}

// ── SelectionDirection ────────────────────────────────────────────────────────

/// Direction for moving the timeline event selection cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionDirection {
    /// Move to the previous sibling at the same depth; wraps to last if at first.
    PrevSibling,
    /// Move to the next sibling at the same depth; wraps to first if at last.
    NextSibling,
    /// Move to the parent event. If at depth 0, move to the previous thread's
    /// first root event.
    ParentOrUpThread,
    /// Move to the first child. If no children, move to the next thread's first
    /// root event.
    FirstChildOrDownThread,
}

// ── TimelineFilter ────────────────────────────────────────────────────────────

/// Filter controlling which timeline threads are shown in the Timeline Events tab.
///
/// Cycles `All → Ui → Raster → All` when the user presses `f` on the Timeline
/// Events tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimelineFilter {
    /// Show events from all threads.
    #[default]
    All,
    /// Show only Flutter UI thread events.
    Ui,
    /// Show only Flutter Raster (GPU) thread events.
    Raster,
}

impl TimelineFilter {
    /// Cycle to the next filter value: `All → Ui → Raster → All`.
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Ui,
            Self::Ui => Self::Raster,
            Self::Raster => Self::All,
        }
    }
}

/// 30 seconds at 60 FPS — enables meaningful scroll-back.
pub(crate) const DEFAULT_FRAME_HISTORY_SIZE: usize = 1800;

/// Maximum number of entries in [`PerformanceState::frame_anchor_map`].
///
/// 2 000 frames ≈ 33 seconds at 60 FPS — enough to cover all frame history
/// while keeping memory overhead in the tens of KB.  Oldest frame numbers
/// (smallest keys) are evicted first when the map reaches this cap.
pub(crate) const FRAME_ANCHOR_MAP_CAP: usize = 2_000;

/// Active section within the Performance DevTools panel.
///
/// Used for `Tab`/`Shift+Tab` navigation between the two sub-sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerfSection {
    /// Frame timing bar chart (default section on open).
    #[default]
    FrameChart,
    /// The details pane (frame analysis, rebuild stats, timeline events).
    Details,
}

impl PerfSection {
    /// Return the next section in Tab order — wraps `FrameChart → Details → FrameChart`.
    ///
    /// # Caution: 2-variant assumption
    ///
    /// This implementation assumes `PerfSection` has exactly 2 variants
    /// (`FrameChart` and `Details`). The body returns the opposite variant
    /// unconditionally — correct for n=2, silently wrong if a third variant
    /// is added. If you add a variant, rewrite both `next` and `prev` to
    /// cycle through all variants explicitly.
    pub fn next(self) -> Self {
        match self {
            PerfSection::FrameChart => PerfSection::Details,
            PerfSection::Details => PerfSection::FrameChart,
        }
    }

    /// Return the previous section in Tab order — wraps the other way.
    ///
    /// # Caution: 2-variant assumption
    ///
    /// This implementation assumes `PerfSection` has exactly 2 variants
    /// (`FrameChart` and `Details`). The body returns the opposite variant
    /// unconditionally — correct for n=2, silently wrong if a third variant
    /// is added. If you add a variant, rewrite both `next` and `prev` to
    /// cycle through all variants explicitly.
    pub fn prev(self) -> Self {
        match self {
            PerfSection::FrameChart => PerfSection::Details,
            PerfSection::Details => PerfSection::FrameChart,
        }
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
    ///
    /// **Invariant:** flipped in lockstep with [`super::memory::MemoryState::monitoring_active`].
    /// Both flags are set true in the `VmServicePerformanceMonitoringStarted` arm
    /// and reset to false on `VmServiceConnected` (full struct replacement). If a
    /// future change diverges these lifecycles, document the rationale here.
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
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3 and
    // docs/REVIEW_FOCUS.md "Approved TEA Exception → Current usage".
    pub frame_chart_visible_width: Cell<usize>,

    /// Which tab is active within the Performance Details pane.
    ///
    /// Defaults to `PerfDetailsTab::FrameAnalysis`. Cycled by `]`/`[` when
    /// `focused_section == PerfSection::Details`. The renderer reads this
    /// value to dispatch to the correct tab module.
    pub details_tab: PerfDetailsTab,

    /// Render-hint: visible height (in rows) of the Details pane content area
    /// from the last rendered frame.
    ///
    /// Defaults to `0`, signalling "not yet rendered — use fallback". Phase 3
    /// consumes this for Rebuild Stats / Timeline Events scrolling; Phase 2 sets
    /// it but does not read it.
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3 and
    // docs/REVIEW_FOCUS.md "Approved TEA Exception → Current usage".
    pub details_pane_visible_height: Cell<usize>,

    /// Refresh rate (Hz) used to compute the per-frame budget in
    /// [`fdemon_core::frame_hints::frame_hints`].
    ///
    /// Phase 2 hard-codes `60.0`. Phase 3 may parse the `Display.Refresh`
    /// Extension event stream to detect 90 / 120 Hz devices. A conservative
    /// 60 Hz default never reports a non-janky frame as janky.
    pub display_refresh_rate: f64,

    // ── Phase 3: Rebuild Stats ────────────────────────────────────────────────
    /// Whether the `ext.flutter.profileWidgetBuilds` extension is currently on.
    ///
    /// Drives Rebuild Stats tab visibility; persisted across hot restart so
    /// `session_lifecycle::handle_session_restart_completed` can re-enable it.
    pub rebuild_stats_enabled: bool,

    /// Persistent location map: incrementally merged from
    /// `Flutter.RebuiltWidgets` events and the one-shot
    /// `widgetLocationIdMap` fallback.
    pub rebuild_stats_location_map: LocationMap,

    /// Lifetime accumulator (since extension was last enabled): location id →
    /// total rebuild count across all observed frames. Cleared on disable.
    pub rebuild_stats_totals: HashMap<u32, u32>,

    /// Per-frame snapshot ring buffer (newest at the back). Capped by
    /// `Settings::devtools::rebuild_stats_frame_window` (default 30).
    pub rebuild_stats_frames: VecDeque<RebuildStatsSnapshot>,

    /// Scroll offset for the Rebuild Stats table.
    pub rebuild_stats_scroll_offset: usize,

    /// Currently-selected row in the Rebuild Stats table (j/k navigation).
    pub rebuild_stats_selected_row: Option<usize>,

    // ── Phase 4: Timeline Events (thread-grouped tree) ────────────────────────
    /// Per-thread event trees, keyed by `tid`. `BTreeMap` iteration order is
    /// `tid` ascending so the renderer produces stable thread-row ordering.
    ///
    /// Total node count across all tracks is capped by
    /// `Settings::devtools::timeline_event_buffer_size` (default 1000). Eviction
    /// drops the oldest root events globally (by `ts`) until under the cap.
    pub timeline_tracks: BTreeMap<i64, TimelineTrack>,

    /// Render-hint write-back: actual visible thread-row count last frame.
    ///
    /// Written by the Gantt renderer (T05); read by the scroll handler.
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3.
    pub timeline_visible_row_count: Cell<usize>,

    /// Scroll offset measured in thread rows (not individual events).
    ///
    /// Reset to 0 on filter change or panel exit.
    pub timeline_thread_scroll_offset: usize,

    /// `tid → thread_name` cache, populated from `ph:"M"` metadata events.
    /// Persists across polls within a session. Used by the Gantt renderer to
    /// label thread rows.
    pub timeline_thread_name_map: HashMap<i64, String>,

    /// Current filter selection — `All`, `Ui`, or `Raster`.
    pub timeline_events_filter: TimelineFilter,

    // ── Phase 5: Frame-anchored timeline viewport ─────────────────────────────
    /// The frame *number* (from `FrameTiming.number`) currently anchored in
    /// the Timeline Events Gantt viewport.
    ///
    /// `None` — no anchor; show the "Select a frame…" placeholder.
    /// `Some(N)` — anchor to the frame with `FrameTiming.number == N`.
    ///
    /// Set by `handle_apply_frame_anchor` after the 200 ms debounce fires.
    /// Reset to `None` when leaving the Performance panel.
    pub committed_frame_anchor: Option<u64>,

    /// Monotonic counter incremented each time the frame selection changes.
    ///
    /// `ApplyFrameAnchor` messages carry the generation at which they were
    /// spawned; handlers silently drop messages whose generation is older than
    /// the current value (stale debounce firings).
    pub frame_anchor_generation: u64,

    /// Persistent map of `frame_number → (vm_ts_start, vm_ts_end)` populated by
    /// the timeline ingest handler. Survives `timeline_tracks` eviction so that
    /// anchoring on an older frame still works even after its raw events have
    /// dropped out of the event buffer. Capped at [`FRAME_ANCHOR_MAP_CAP`] entries
    /// (oldest frame numbers evicted first when full).
    pub frame_anchor_map: BTreeMap<u64, (u64, u64)>,

    // ── Phase 5: Timeline event selection ────────────────────────────────────
    /// Currently selected event in the Timeline Events Gantt.
    ///
    /// `None` — no event is selected (normal pan/zoom mode).
    /// `Some(cursor)` — the event identified by `cursor` is highlighted.
    ///
    /// Cleared when:
    /// - The user presses `Esc` (with popup closed).
    /// - The identified event is evicted from the ring buffer.
    /// - The user switches away from the Timeline Events tab.
    pub timeline_selected_event: Option<TimelineEventCursor>,

    /// Whether the event details popup is currently open.
    ///
    /// When `true`, the popup overlays the Gantt and intercepts `Esc`
    /// (to close the popup) before the selection-clear arm fires.
    ///
    /// Default: `false`.
    pub timeline_details_popup_open: bool,

    // ── Phase 5: Pan/zoom viewport state ─────────────────────────────────────
    /// Manual viewport start in microseconds.
    ///
    /// Honored only when `timeline_follow_latest == false`. When `follow_latest`
    /// is true, the active viewport is computed from the frame anchor (mode 2)
    /// or the live-edge fallback (mode 3) per PLAN D2.
    pub timeline_viewport_start_micros: u64,

    /// Viewport width in microseconds.
    ///
    /// Default: [`TIMELINE_VIEWPORT_MICROS`] (5 s). Bounded by
    /// [`TIMELINE_VIEWPORT_MIN_MICROS`] (100 ms) ..= [`TIMELINE_VIEWPORT_MAX_MICROS`] (60 s).
    /// This field is always updated together with `timeline_viewport_start_micros`
    /// by the zoom handler; both fields are logically irrelevant when
    /// `timeline_follow_latest == true`.
    pub timeline_viewport_width_micros: u64,

    /// When `true` (default), `compute_active_viewport` returns the live-edge or
    /// frame-anchored window (PLAN D2 modes 2 & 3).
    ///
    /// When `false`, the Gantt is pinned to the manual
    /// `[viewport_start_micros, viewport_start_micros + viewport_width_micros]`
    /// window (PLAN D2 mode 1). The Gantt renders a "PAUSED" indicator while in
    /// this state; press `g` or `End` to resume follow-latest.
    ///
    /// Panning or zooming sets this to `false`. Pressing `g`/`End` sets it to
    /// `true` and resets `viewport_width_micros` to the default 5 s.
    pub timeline_follow_latest: bool,
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
            details_tab: PerfDetailsTab::default(),
            details_pane_visible_height: Cell::new(0),
            display_refresh_rate: 60.0,
            // Phase 3: Rebuild Stats — all start empty/disabled
            rebuild_stats_enabled: false,
            rebuild_stats_location_map: LocationMap::default(),
            rebuild_stats_totals: HashMap::new(),
            rebuild_stats_frames: VecDeque::new(),
            rebuild_stats_scroll_offset: 0,
            rebuild_stats_selected_row: None,
            // Phase 4: Timeline Events (thread-grouped tree) — all start empty
            timeline_tracks: BTreeMap::new(),
            timeline_visible_row_count: Cell::new(0),
            timeline_thread_scroll_offset: 0,
            timeline_thread_name_map: HashMap::new(),
            timeline_events_filter: TimelineFilter::All,
            // Phase 5: Frame-anchored viewport — start unanchored
            committed_frame_anchor: None,
            frame_anchor_generation: 0,
            frame_anchor_map: BTreeMap::new(),
            // Phase 5: Timeline event selection — start with nothing selected
            timeline_selected_event: None,
            timeline_details_popup_open: false,
            // Phase 5: Pan/zoom viewport — start in follow-latest mode
            timeline_viewport_start_micros: 0,
            // Default width = 5 s (matches TIMELINE_VIEWPORT_MICROS in the TUI crate;
            // cannot import the TUI constant here due to layer boundaries).
            timeline_viewport_width_micros: 5_000_000,
            timeline_follow_latest: true,
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
    fn perf_section_next_cycles_between_frame_chart_and_details() {
        assert_eq!(PerfSection::FrameChart.next(), PerfSection::Details);
        assert_eq!(PerfSection::Details.next(), PerfSection::FrameChart);
    }

    #[test]
    fn perf_section_prev_cycles_between_frame_chart_and_details() {
        assert_eq!(PerfSection::FrameChart.prev(), PerfSection::Details);
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
    fn performance_state_defaults_phase_2_fields() {
        let s = PerformanceState::default();
        assert_eq!(s.details_tab, PerfDetailsTab::FrameAnalysis);
        assert_eq!(s.details_pane_visible_height.get(), 0);
        assert_eq!(s.display_refresh_rate, 60.0);
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

    // ── Phase 3: TimelineFilter ──────────────────────────────────────────────

    #[test]
    fn timeline_filter_default_is_all() {
        assert_eq!(TimelineFilter::default(), TimelineFilter::All);
    }

    #[test]
    fn timeline_filter_next_cycles_all_ui_raster_all() {
        assert_eq!(TimelineFilter::All.next(), TimelineFilter::Ui);
        assert_eq!(TimelineFilter::Ui.next(), TimelineFilter::Raster);
        assert_eq!(TimelineFilter::Raster.next(), TimelineFilter::All);
    }

    // ── Phase 3: PerformanceState defaults ──────────────────────────────────

    #[test]
    fn performance_state_phase3_rebuild_stats_defaults() {
        let s = PerformanceState::default();
        assert!(!s.rebuild_stats_enabled);
        assert!(s.rebuild_stats_location_map.by_id.is_empty());
        assert!(s.rebuild_stats_totals.is_empty());
        assert!(s.rebuild_stats_frames.is_empty());
        assert_eq!(s.rebuild_stats_scroll_offset, 0);
        assert!(s.rebuild_stats_selected_row.is_none());
    }

    // ── Phase 4: Timeline tree state defaults ────────────────────────────────

    #[test]
    fn performance_state_phase4_timeline_tree_defaults() {
        let s = PerformanceState::default();
        assert!(
            s.timeline_tracks.is_empty(),
            "timeline_tracks should start empty"
        );
        assert_eq!(s.timeline_visible_row_count.get(), 0);
        assert_eq!(s.timeline_thread_scroll_offset, 0);
        assert!(s.timeline_thread_name_map.is_empty());
        assert_eq!(s.timeline_events_filter, TimelineFilter::All);
    }

    // ── Phase 5: Pan/zoom viewport defaults ──────────────────────────────────

    #[test]
    fn performance_state_phase5_viewport_defaults() {
        let s = PerformanceState::default();
        // New Phase 5 fields — verify defaults without redeclaring Phase 4 fields
        assert_eq!(
            s.timeline_viewport_start_micros, 0,
            "viewport start should default to 0"
        );
        assert_eq!(
            s.timeline_viewport_width_micros, 5_000_000,
            "viewport width should default to 5s (5_000_000 µs)"
        );
        assert!(
            s.timeline_follow_latest,
            "follow_latest should default to true"
        );
        // Verify Phase 4 frame-anchor fields are NOT redeclared (still present)
        assert!(s.committed_frame_anchor.is_none());
        assert_eq!(s.frame_anchor_generation, 0);
        assert!(s.frame_anchor_map.is_empty());
    }
}
