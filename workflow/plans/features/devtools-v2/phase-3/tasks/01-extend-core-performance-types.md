## Task: Extend Core Performance Types

**Objective**: Add `MemorySample` and `FramePhases` structs to `fdemon-core`, and extend `FrameTiming` with optional phase breakdown and shader compilation fields. These types form the data model for Phase 3's frame bar chart and memory time-series chart.

**Depends on**: None

### Scope

- `crates/fdemon-core/src/performance.rs`: Add new structs, extend `FrameTiming`
- `crates/fdemon-core/src/lib.rs`: Export new types

### Details

#### Add `FramePhases` struct

Add after the existing `FrameTiming` struct (around line 155):

```rust
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
```

Add helper methods:

```rust
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
```

#### Extend `FrameTiming`

Add two new fields to the existing struct:

```rust
pub struct FrameTiming {
    pub number: u64,
    pub build_micros: u64,
    pub raster_micros: u64,
    pub elapsed_micros: u64,
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub phases: Option<FramePhases>,      // NEW — detailed phase breakdown
    pub shader_compilation: bool,          // NEW — shader compile detected in this frame
}
```

**Breaking change mitigation**: The two new fields must be added to all existing construction sites:
- `crates/fdemon-daemon/src/vm_service/timeline.rs` — the `parse_frame_timing` function
- `crates/fdemon-app/src/handler/tests.rs` — test helper that constructs `FrameTiming`

In both places, set `phases: None` and `shader_compilation: false` as defaults.

Add helper method on `FrameTiming`:

```rust
impl FrameTiming {
    // ... existing methods ...

    /// Whether this frame involved shader compilation.
    /// Checks both the top-level flag and the phases detail.
    pub fn has_shader_compilation(&self) -> bool {
        self.shader_compilation
            || self.phases.as_ref().is_some_and(|p| p.shader_compilation)
    }
}
```

#### Add `MemorySample` struct

Add after `MemoryUsage` (around line 50):

```rust
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
```

Add helper methods:

```rust
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
```

#### Export from lib.rs

Add `FramePhases` and `MemorySample` to the existing performance re-export block:

```rust
pub use performance::{
    AllocationProfile, ClassHeapStats, FramePhases, FrameTiming, GcEvent, MemorySample,
    MemoryUsage, PerformanceStats, RingBuffer, FRAME_BUDGET_120FPS_MICROS,
    FRAME_BUDGET_60FPS_MICROS,
};
```

### Acceptance Criteria

1. `FramePhases` struct exists with `build_micros`, `layout_micros`, `paint_micros`, `raster_micros`, `shader_compilation` fields
2. `FramePhases::ui_micros()` returns sum of build + layout + paint
3. `FrameTiming` has `phases: Option<FramePhases>` and `shader_compilation: bool` fields
4. `FrameTiming::has_shader_compilation()` checks both fields
5. `MemorySample` struct exists with `dart_heap`, `dart_native`, `raster_cache`, `allocated`, `rss`, `timestamp` fields
6. `MemorySample::from_memory_usage()` converts from existing `MemoryUsage` type
7. All new types exported from `fdemon-core` lib.rs
8. All existing tests in `fdemon-core` pass (with updates for new `FrameTiming` fields)
9. `cargo check -p fdemon-daemon` passes (update `parse_frame_timing` construction site)
10. `cargo check -p fdemon-app` passes (update test construction sites)

### Testing

Add tests in `performance.rs` inline test module:

```rust
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
```

### Notes

- **Breaking change strategy**: Adding non-`Option` field `shader_compilation: bool` to `FrameTiming` requires updating all construction sites. There are exactly 3: `parse_frame_timing()` in `timeline.rs`, test helpers in `handler/tests.rs`, and tests in `performance.rs` itself. Set `shader_compilation: false` at each site.
- **No `ClassAllocation` struct**: The plan mentions `ClassAllocation`, but `ClassHeapStats` + `AllocationProfile` already model this data. Task 02 reuses these existing types.
- **`MemorySample` coexists with `MemoryUsage`**: The existing `MemoryUsage` type and its ring buffer are not removed. `MemorySample` is a richer superset used by the new memory chart. The `from_memory_usage()` bridge allows progressive migration.
- **RSS availability**: RSS may not be available from all VM service versions. The `rss: u64` field uses `0` as the "unavailable" sentinel rather than `Option<u64>`, since the chart rendering logic is simpler with a numeric zero (no line drawn when all samples are 0).

---

## Completion Summary

**Status:** Not started
