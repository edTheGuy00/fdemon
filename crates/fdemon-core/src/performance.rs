//! # Performance & Memory Domain Types
//!
//! Domain data types for representing memory usage, GC events, frame timing,
//! allocation profiles, and a generic ring buffer for rolling history storage.
//!
//! These types are the shared vocabulary between:
//! - `fdemon-daemon` (parsing VM Service responses)
//! - `fdemon-app` (aggregation, session state)

use std::cmp::Reverse;
use std::collections::VecDeque;

// ── MemoryUsage ──────────────────────────────────────────────────────────────

/// Heap memory usage snapshot from the Dart VM.
///
/// Returned by `getMemoryUsage(isolateId)`. All values are in bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryUsage {
    /// Current heap usage in bytes (amount currently allocated).
    pub heap_usage: u64,
    /// Total heap capacity in bytes (amount the VM has reserved from the OS).
    pub heap_capacity: u64,
    /// External memory usage in bytes (e.g., images, native buffers managed
    /// by Dart objects with C finalizers).
    pub external_usage: u64,
    /// Timestamp when this snapshot was taken.
    pub timestamp: chrono::DateTime<chrono::Local>,
}

impl MemoryUsage {
    /// Heap utilization as a percentage (0.0–1.0).
    pub fn utilization(&self) -> f64 {
        if self.heap_capacity == 0 {
            return 0.0;
        }
        self.heap_usage as f64 / self.heap_capacity as f64
    }

    /// Total memory (heap + external) in bytes.
    pub fn total(&self) -> u64 {
        self.heap_usage + self.external_usage
    }

    /// Format bytes as human-readable string (e.g., "12.5 MB").
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * 1024;
        const GB: u64 = 1024 * 1024 * 1024;
        match bytes {
            b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
            b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
            b if b >= KB => format!("{:.1} KB", b as f64 / KB as f64),
            b => format!("{} B", b),
        }
    }
}

// ── MemorySample ─────────────────────────────────────────────────────────────

/// Rich memory snapshot for time-series charting.
///
/// Extends `MemoryUsage` with per-category breakdown (Dart heap, native,
/// raster cache) and RSS. Collected by combining `getMemoryUsage` with
/// `getIsolate` data from the VM service.
#[derive(Debug, Clone)]
pub struct MemorySample {
    /// Dart/Flutter heap objects (bytes).
    pub dart_heap: u64,
    /// Native memory outside Dart heap — decoded images, file I/O buffers (bytes).
    pub dart_native: u64,
    /// Raster cache layers/pictures (bytes). 0 if unavailable.
    pub raster_cache: u64,
    /// Total Dart heap capacity (bytes).
    pub allocated: u64,
    /// Resident set size (bytes). 0 if unavailable.
    pub rss: u64,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

impl MemorySample {
    /// Total memory tracked (heap + native + raster).
    pub fn total_usage(&self) -> u64 {
        self.dart_heap + self.dart_native + self.raster_cache
    }

    /// Construct from an existing `MemoryUsage` with defaults for unavailable fields.
    ///
    /// Used as a migration bridge: converts the simpler `MemoryUsage` into a
    /// `MemorySample` with `raster_cache` and `rss` set to 0.
    pub fn from_memory_usage(usage: &MemoryUsage) -> Self {
        Self {
            dart_heap: usage.heap_usage,
            dart_native: usage.external_usage,
            raster_cache: 0,
            allocated: usage.heap_capacity,
            rss: 0,
            timestamp: usage.timestamp,
        }
    }
}

// ── GcEvent ──────────────────────────────────────────────────────────────────

/// A garbage collection event from the VM Service GC stream.
#[derive(Debug, Clone)]
pub struct GcEvent {
    /// Type of GC operation performed (e.g., "Scavenge", "MarkSweep", "MarkCompact").
    pub gc_type: String,
    /// Reason the GC was triggered.
    pub reason: Option<String>,
    /// Isolate that performed the GC.
    pub isolate_id: Option<String>,
    /// Timestamp of the GC event.
    pub timestamp: chrono::DateTime<chrono::Local>,
}

impl GcEvent {
    /// Returns `true` if this is a major GC event (MarkSweep, MarkCompact).
    ///
    /// The Dart VM emits two categories of GC events:
    /// - **Minor GC** (`Scavenge`): Young-generation collection. Very frequent at high
    ///   allocation rates (multiple per second) but low pause time.
    /// - **Major GC** (`MarkSweep`, `MarkCompact`): Old-generation collection. Rare but
    ///   has significant pause times and indicates real memory pressure.
    ///
    /// Only major GC events are stored in `gc_history` to prevent Scavenge events from
    /// filling the ring buffer and pushing out the more informative major GC entries.
    pub fn is_major_gc(&self) -> bool {
        self.gc_type != "Scavenge"
    }
}

// ── ClassHeapStats ───────────────────────────────────────────────────────────

/// Heap allocation statistics for a single class.
#[derive(Debug, Clone)]
pub struct ClassHeapStats {
    /// Fully qualified class name (e.g., "dart:core/String").
    pub class_name: String,
    /// Library URI that defines the class.
    pub library_uri: Option<String>,
    /// Number of instances in new space.
    pub new_space_instances: u64,
    /// Bytes occupied in new space.
    pub new_space_size: u64,
    /// Number of instances in old space.
    pub old_space_instances: u64,
    /// Bytes occupied in old space.
    pub old_space_size: u64,
}

impl ClassHeapStats {
    /// Total bytes across new + old space.
    pub fn total_size(&self) -> u64 {
        self.new_space_size + self.old_space_size
    }

    /// Total instance count across new + old space.
    pub fn total_instances(&self) -> u64 {
        self.new_space_instances + self.old_space_instances
    }
}

// ── AllocationProfile ────────────────────────────────────────────────────────

/// Allocation profile summary from `getAllocationProfile`.
#[derive(Debug, Clone)]
pub struct AllocationProfile {
    /// Allocation statistics per class.
    pub members: Vec<ClassHeapStats>,
    /// Timestamp of the profile snapshot.
    pub timestamp: chrono::DateTime<chrono::Local>,
}

impl AllocationProfile {
    /// Return classes sorted by total size (descending).
    pub fn top_by_size(&self, limit: usize) -> Vec<&ClassHeapStats> {
        let mut sorted: Vec<_> = self.members.iter().collect();
        sorted.sort_by_key(|s| Reverse(s.total_size()));
        sorted.truncate(limit);
        sorted
    }
}

// ── FrameTiming ──────────────────────────────────────────────────────────────

/// Budget for a single frame at 60 FPS (16.667ms).
pub const FRAME_BUDGET_60FPS_MICROS: u64 = 16_667;

/// Budget for a single frame at 120 FPS (8.333ms).
pub const FRAME_BUDGET_120FPS_MICROS: u64 = 8_333;

/// Breakdown of a single frame into build/layout/paint/raster phases.
///
/// Not always available — requires timeline event data from the VM service.
/// When unavailable, `FrameTiming.phases` is `None` and only the aggregate
/// `build_micros` / `raster_micros` split is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramePhases {
    pub build_micros: u64,
    pub layout_micros: u64,
    pub paint_micros: u64,
    pub raster_micros: u64,
    pub shader_compilation: bool,
}

impl FramePhases {
    /// Total UI thread time (build + layout + paint).
    pub fn ui_micros(&self) -> u64 {
        self.build_micros + self.layout_micros + self.paint_micros
    }

    /// Total frame time (UI + raster).
    pub fn total_micros(&self) -> u64 {
        self.ui_micros() + self.raster_micros
    }

    pub fn ui_ms(&self) -> f64 {
        self.ui_micros() as f64 / 1000.0
    }

    pub fn raster_ms(&self) -> f64 {
        self.raster_micros as f64 / 1000.0
    }
}

/// Timing data for a single Flutter UI frame.
///
/// Flutter posts `Flutter.Frame` events via `developer.postEvent` on the
/// Extension stream. Each event carries the build and raster durations.
#[derive(Debug, Clone)]
pub struct FrameTiming {
    /// Frame number (monotonically increasing).
    pub number: u64,
    /// Duration of the build phase (widget tree construction) in microseconds.
    pub build_micros: u64,
    /// Duration of the raster phase (GPU painting) in microseconds.
    pub raster_micros: u64,
    /// Total elapsed frame time in microseconds.
    pub elapsed_micros: u64,
    /// Timestamp of the frame event.
    pub timestamp: chrono::DateTime<chrono::Local>,
    /// Detailed phase breakdown (build/layout/paint/raster). `None` when
    /// timeline event data is unavailable from the VM service.
    pub phases: Option<FramePhases>,
    /// Whether shader compilation was detected in this frame.
    pub shader_compilation: bool,
}

impl FrameTiming {
    /// Whether this frame exceeded the 60 FPS budget (janky).
    pub fn is_janky(&self) -> bool {
        self.elapsed_micros > FRAME_BUDGET_60FPS_MICROS
    }

    /// Frame duration in milliseconds.
    pub fn elapsed_ms(&self) -> f64 {
        self.elapsed_micros as f64 / 1000.0
    }

    /// Build duration in milliseconds.
    pub fn build_ms(&self) -> f64 {
        self.build_micros as f64 / 1000.0
    }

    /// Raster duration in milliseconds.
    pub fn raster_ms(&self) -> f64 {
        self.raster_micros as f64 / 1000.0
    }

    /// Whether this frame involved shader compilation.
    /// Checks both the top-level flag and the phases detail.
    pub fn has_shader_compilation(&self) -> bool {
        self.shader_compilation || self.phases.as_ref().is_some_and(|p| p.shader_compilation)
    }
}

// ── PerformanceStats ─────────────────────────────────────────────────────────

/// Aggregated performance metrics for display.
#[derive(Debug, Clone, Default)]
pub struct PerformanceStats {
    /// Current FPS (frames per second), calculated from recent frame timings.
    pub fps: Option<f64>,
    /// Number of janky frames in the recent window.
    pub jank_count: u32,
    /// Average frame time in milliseconds over the recent window.
    pub avg_frame_ms: Option<f64>,
    /// 95th percentile frame time in milliseconds.
    pub p95_frame_ms: Option<f64>,
    /// Worst (max) frame time in milliseconds.
    pub max_frame_ms: Option<f64>,
    /// Number of frame timing samples currently in the ring buffer.
    pub buffered_frames: u64,
}

impl PerformanceStats {
    /// Whether the FPS data is stale (no recent frames in the last second).
    ///
    /// Returns `true` when `fps` is `None`, which happens when the app is idle
    /// or backgrounded (no animation → no `Flutter.Frame` events).
    /// Phase 4's TUI can show "idle" or "–" when this returns `true`.
    pub fn is_stale(&self) -> bool {
        self.fps.is_none()
    }
}

// ── RingBuffer<T> ────────────────────────────────────────────────────────────

/// A fixed-capacity circular buffer that overwrites the oldest entries
/// when full. Used for rolling performance history.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    buf: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// Create a new ring buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a value, evicting the oldest if at capacity.
    pub fn push(&mut self, value: T) {
        if self.buf.len() == self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(value);
    }

    /// Number of items currently stored.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Returns true if the buffer is at capacity and the next push will evict.
    pub fn is_full(&self) -> bool {
        self.buf.len() == self.capacity
    }

    /// Maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Iterate over items from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf.iter()
    }

    /// Get the most recently pushed item.
    pub fn latest(&self) -> Option<&T> {
        self.buf.back()
    }

    /// Get the oldest item.
    pub fn oldest(&self) -> Option<&T> {
        self.buf.front()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── GcEvent ─────────────────────────────────────
    #[test]
    fn test_is_major_gc_scavenge_returns_false() {
        let gc = GcEvent {
            gc_type: "Scavenge".into(),
            reason: None,
            isolate_id: None,
            timestamp: chrono::Local::now(),
        };
        assert!(!gc.is_major_gc(), "Scavenge should not be a major GC");
    }

    #[test]
    fn test_is_major_gc_mark_sweep_returns_true() {
        let gc = GcEvent {
            gc_type: "MarkSweep".into(),
            reason: None,
            isolate_id: None,
            timestamp: chrono::Local::now(),
        };
        assert!(gc.is_major_gc(), "MarkSweep should be a major GC");
    }

    #[test]
    fn test_is_major_gc_mark_compact_returns_true() {
        let gc = GcEvent {
            gc_type: "MarkCompact".into(),
            reason: None,
            isolate_id: None,
            timestamp: chrono::Local::now(),
        };
        assert!(gc.is_major_gc(), "MarkCompact should be a major GC");
    }

    #[test]
    fn test_is_major_gc_unknown_type_returns_true() {
        // Unknown GC types are treated as major to err on the side of preserving data.
        let gc = GcEvent {
            gc_type: "UnknownGcType".into(),
            reason: None,
            isolate_id: None,
            timestamp: chrono::Local::now(),
        };
        assert!(
            gc.is_major_gc(),
            "Unknown GC types should be treated as major"
        );
    }

    // ── MemoryUsage ─────────────────────────────────
    #[test]
    fn test_memory_utilization() {
        let mem = MemoryUsage {
            heap_usage: 50_000_000,
            heap_capacity: 100_000_000,
            external_usage: 10_000_000,
            timestamp: chrono::Local::now(),
        };
        assert!((mem.utilization() - 0.5).abs() < f64::EPSILON);
        assert_eq!(mem.total(), 60_000_000);
    }

    #[test]
    fn test_memory_utilization_zero_capacity() {
        let mem = MemoryUsage {
            heap_usage: 0,
            heap_capacity: 0,
            external_usage: 0,
            timestamp: chrono::Local::now(),
        };
        assert!((mem.utilization() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(MemoryUsage::format_bytes(500), "500 B");
        assert_eq!(MemoryUsage::format_bytes(1536), "1.5 KB");
        assert_eq!(MemoryUsage::format_bytes(52_428_800), "50.0 MB");
        assert_eq!(MemoryUsage::format_bytes(1_610_612_736), "1.5 GB");
    }

    // ── ClassHeapStats ──────────────────────────────
    #[test]
    fn test_class_heap_stats_totals() {
        let stats = ClassHeapStats {
            class_name: "String".into(),
            library_uri: Some("dart:core".into()),
            new_space_instances: 100,
            new_space_size: 4000,
            old_space_instances: 50,
            old_space_size: 6000,
        };
        assert_eq!(stats.total_size(), 10_000);
        assert_eq!(stats.total_instances(), 150);
    }

    // ── FrameTiming ─────────────────────────────────
    #[test]
    fn test_frame_timing_janky() {
        let frame = FrameTiming {
            number: 1,
            build_micros: 8000,
            raster_micros: 10000,
            elapsed_micros: 18000,
            timestamp: chrono::Local::now(),
            phases: None,
            shader_compilation: false,
        };
        assert!(frame.is_janky()); // 18ms > 16.667ms
    }

    #[test]
    fn test_frame_timing_smooth() {
        let frame = FrameTiming {
            number: 2,
            build_micros: 5000,
            raster_micros: 5000,
            elapsed_micros: 10000,
            timestamp: chrono::Local::now(),
            phases: None,
            shader_compilation: false,
        };
        assert!(!frame.is_janky()); // 10ms < 16.667ms
    }

    // ── RingBuffer ──────────────────────────────────
    #[test]
    fn test_ring_buffer_basic() {
        let mut buf = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.latest(), Some(&3));
        assert_eq!(buf.oldest(), Some(&1));
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut buf = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        buf.push(4);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.oldest(), Some(&2)); // 1 was evicted
        assert_eq!(buf.latest(), Some(&4));
        let items: Vec<_> = buf.iter().copied().collect();
        assert_eq!(items, vec![2, 3, 4]);
    }

    #[test]
    fn test_ring_buffer_is_full() {
        let mut buf = RingBuffer::new(3);
        assert!(!buf.is_full(), "empty buffer should not be full");
        buf.push(1);
        assert!(!buf.is_full(), "partially filled buffer should not be full");
        buf.push(2);
        assert!(!buf.is_full(), "partially filled buffer should not be full");
        buf.push(3);
        assert!(buf.is_full(), "buffer at capacity should be full");
        buf.push(4); // evicts 1
        assert!(
            buf.is_full(),
            "buffer should remain full after eviction push"
        );
    }

    #[test]
    fn test_ring_buffer_empty() {
        let buf: RingBuffer<i32> = RingBuffer::new(5);
        assert!(buf.is_empty());
        assert_eq!(buf.latest(), None);
    }

    #[test]
    fn test_ring_buffer_clear() {
        let mut buf = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.clear();
        assert!(buf.is_empty());
    }

    // ── FramePhases ─────────────────────────────────
    #[test]
    fn test_frame_phases_ui_micros() {
        let phases = FramePhases {
            build_micros: 3_000,
            layout_micros: 1_000,
            paint_micros: 2_000,
            raster_micros: 4_000,
            shader_compilation: false,
        };
        assert_eq!(phases.ui_micros(), 6_000);
        assert_eq!(phases.total_micros(), 10_000);
    }

    #[test]
    fn test_frame_phases_ms_conversion() {
        let phases = FramePhases {
            build_micros: 5_000,
            layout_micros: 0,
            paint_micros: 0,
            raster_micros: 3_000,
            shader_compilation: false,
        };
        assert!((phases.ui_ms() - 5.0).abs() < f64::EPSILON);
        assert!((phases.raster_ms() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_frame_timing_has_shader_compilation_top_level() {
        let timing = FrameTiming {
            number: 1,
            build_micros: 5_000,
            raster_micros: 5_000,
            elapsed_micros: 10_000,
            timestamp: chrono::Local::now(),
            phases: None,
            shader_compilation: true,
        };
        assert!(timing.has_shader_compilation());
    }

    #[test]
    fn test_frame_timing_has_shader_compilation_from_phases() {
        let timing = FrameTiming {
            number: 1,
            build_micros: 5_000,
            raster_micros: 5_000,
            elapsed_micros: 10_000,
            timestamp: chrono::Local::now(),
            phases: Some(FramePhases {
                build_micros: 3_000,
                layout_micros: 1_000,
                paint_micros: 1_000,
                raster_micros: 5_000,
                shader_compilation: true,
            }),
            shader_compilation: false,
        };
        assert!(timing.has_shader_compilation());
    }

    #[test]
    fn test_frame_timing_no_shader_compilation() {
        let timing = FrameTiming {
            number: 1,
            build_micros: 5_000,
            raster_micros: 5_000,
            elapsed_micros: 10_000,
            timestamp: chrono::Local::now(),
            phases: None,
            shader_compilation: false,
        };
        assert!(!timing.has_shader_compilation());
    }

    // ── MemorySample ────────────────────────────────
    #[test]
    fn test_memory_sample_total_usage() {
        let sample = MemorySample {
            dart_heap: 10_000_000,
            dart_native: 5_000_000,
            raster_cache: 2_000_000,
            allocated: 20_000_000,
            rss: 50_000_000,
            timestamp: chrono::Local::now(),
        };
        assert_eq!(sample.total_usage(), 17_000_000);
    }

    #[test]
    fn test_memory_sample_from_memory_usage() {
        let usage = MemoryUsage {
            heap_usage: 10_000_000,
            heap_capacity: 20_000_000,
            external_usage: 5_000_000,
            timestamp: chrono::Local::now(),
        };
        let sample = MemorySample::from_memory_usage(&usage);
        assert_eq!(sample.dart_heap, 10_000_000);
        assert_eq!(sample.dart_native, 5_000_000);
        assert_eq!(sample.raster_cache, 0);
        assert_eq!(sample.allocated, 20_000_000);
        assert_eq!(sample.rss, 0);
    }

    #[test]
    fn test_memory_sample_in_ring_buffer() {
        let mut buf = RingBuffer::new(3);
        for i in 0..5u64 {
            buf.push(MemorySample {
                dart_heap: i * 1_000_000,
                dart_native: 0,
                raster_cache: 0,
                allocated: 0,
                rss: 0,
                timestamp: chrono::Local::now(),
            });
        }
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.oldest().unwrap().dart_heap, 2_000_000);
        assert_eq!(buf.latest().unwrap().dart_heap, 4_000_000);
    }
}
