## Task: Extend PerformanceState and Add Messages

**Objective**: Extend `PerformanceState` with fields needed for the new frame bar chart (frame selection) and memory chart (rich memory samples, class allocations), and add the corresponding `Message` variants for data flow.

**Depends on**: Task 01 (extend-core-performance-types)

### Scope

- `crates/fdemon-app/src/session/performance.rs`: Add new fields to `PerformanceState`
- `crates/fdemon-app/src/message.rs`: Add new message variants
- `crates/fdemon-app/src/state.rs`: Add `AllocationSortColumn` enum if needed
- `crates/fdemon-app/src/lib.rs`: Export new types if needed

### Details

#### Extend `PerformanceState`

Add new fields to the existing struct in `session/performance.rs`:

```rust
pub struct PerformanceState {
    // Existing fields (unchanged)
    pub memory_history: RingBuffer<MemoryUsage>,
    pub gc_history: RingBuffer<GcEvent>,
    pub frame_history: RingBuffer<FrameTiming>,
    pub stats: PerformanceStats,
    pub monitoring_active: bool,

    // NEW — rich memory samples for time-series chart
    pub memory_samples: RingBuffer<MemorySample>,

    // NEW — frame selection for bar chart interaction
    pub selected_frame: Option<usize>,

    // NEW — latest allocation profile snapshot
    pub allocation_profile: Option<AllocationProfile>,

    // NEW — sort order for class allocation table
    pub allocation_sort: AllocationSortColumn,
}
```

#### Add `AllocationSortColumn` enum

Add in `session/performance.rs` (or `state.rs` depending on project convention):

```rust
/// Column by which the class allocation table is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllocationSortColumn {
    #[default]
    BySize,
    ByInstances,
}
```

#### Update constructors

Update `PerformanceState::default()` and `PerformanceState::with_memory_history_size()`:

```rust
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
```

Add constant:
```rust
/// Memory sample buffer size: 120 samples at 500ms polling = 60 seconds of history.
pub const DEFAULT_MEMORY_SAMPLE_SIZE: usize = 120;
```

Update `with_memory_history_size()` to also accept/configure the sample buffer size, or add a separate constructor method.

#### Add new Message variants

Add to the `Message` enum in `message.rs`:

```rust
/// User selected/deselected a frame in the performance bar chart.
SelectPerformanceFrame {
    index: Option<usize>,
},

/// Rich memory sample received from VM service (for time-series chart).
VmServiceMemorySample {
    session_id: SessionId,
    sample: fdemon_core::performance::MemorySample,
},

/// Allocation profile snapshot received from VM service.
VmServiceAllocationProfileReceived {
    session_id: SessionId,
    profile: fdemon_core::performance::AllocationProfile,
},
```

#### Frame selection helpers

Add methods to `PerformanceState`:

```rust
impl PerformanceState {
    /// Select the next frame (Right arrow). Wraps at the end.
    pub fn select_next_frame(&mut self) {
        let len = self.frame_history.len();
        if len == 0 { return; }
        self.selected_frame = Some(match self.selected_frame {
            Some(i) if i + 1 < len => i + 1,
            Some(_) => len - 1, // clamp at end
            None => len - 1,    // select most recent
        });
    }

    /// Select the previous frame (Left arrow). Wraps at the beginning.
    pub fn select_prev_frame(&mut self) {
        let len = self.frame_history.len();
        if len == 0 { return; }
        self.selected_frame = Some(match self.selected_frame {
            Some(i) if i > 0 => i - 1,
            Some(_) => 0, // clamp at start
            None => len - 1, // select most recent
        });
    }

    /// Deselect any selected frame (Esc).
    pub fn deselect_frame(&mut self) {
        self.selected_frame = None;
    }

    /// Get the currently selected frame timing, if any.
    pub fn selected_frame_timing(&self) -> Option<&FrameTiming> {
        self.selected_frame
            .and_then(|i| self.frame_history.iter().nth(i))
    }
}
```

### Acceptance Criteria

1. `PerformanceState` has `memory_samples: RingBuffer<MemorySample>` with capacity 120
2. `PerformanceState` has `selected_frame: Option<usize>`
3. `PerformanceState` has `allocation_profile: Option<AllocationProfile>`
4. `PerformanceState` has `allocation_sort: AllocationSortColumn`
5. `AllocationSortColumn` enum with `BySize` (default), `ByInstances` variants
6. `select_next_frame()` / `select_prev_frame()` / `deselect_frame()` work correctly
7. `selected_frame_timing()` returns the correct `&FrameTiming` reference
8. `Message::SelectPerformanceFrame`, `Message::VmServiceMemorySample`, `Message::VmServiceAllocationProfileReceived` variants exist
9. `DEFAULT_MEMORY_SAMPLE_SIZE` constant = 120
10. Default and `with_memory_history_size` constructors updated
11. `cargo check -p fdemon-app` passes
12. `cargo test -p fdemon-app` passes (existing tests updated for new fields)

### Testing

Add tests in `session/performance.rs`:

```rust
#[test]
fn test_select_next_frame_from_none() {
    let mut state = PerformanceState::default();
    push_test_frames(&mut state, 5);
    state.select_next_frame();
    assert_eq!(state.selected_frame, Some(4)); // selects most recent
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
    assert_eq!(state.selected_frame, Some(4)); // clamped
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
    assert_eq!(state.selected_frame, Some(0)); // clamped
}

#[test]
fn test_deselect_frame() {
    let mut state = PerformanceState::default();
    state.selected_frame = Some(3);
    state.deselect_frame();
    assert_eq!(state.selected_frame, None);
}

#[test]
fn test_select_frame_empty_history_noop() {
    let mut state = PerformanceState::default();
    state.select_next_frame();
    assert_eq!(state.selected_frame, None);
}

#[test]
fn test_selected_frame_timing() {
    let mut state = PerformanceState::default();
    push_test_frames(&mut state, 3);
    state.selected_frame = Some(1);
    let timing = state.selected_frame_timing().unwrap();
    assert_eq!(timing.number, 2); // 1-indexed frame numbers from helper
}

#[test]
fn test_memory_samples_ring_buffer_default_capacity() {
    let state = PerformanceState::default();
    assert_eq!(state.memory_samples.capacity(), DEFAULT_MEMORY_SAMPLE_SIZE);
}

#[test]
fn test_allocation_sort_default_is_by_size() {
    let state = PerformanceState::default();
    assert_eq!(state.allocation_sort, AllocationSortColumn::BySize);
}

// Test helper
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
```

### Notes

- **`memory_history` is kept**: The existing `RingBuffer<MemoryUsage>` (`memory_history`) is NOT removed. It continues to be populated by the existing `VmServiceMemorySnapshot` handler and is used as a fallback when rich `MemorySample` data is unavailable. The new `memory_samples` buffer runs in parallel.
- **Frame selection invalidation**: When new frames arrive and `selected_frame` is `Some(i)`, the index stays valid because `RingBuffer` is append-only. However, if frames scroll out of the buffer, the selected index may reference a different frame. Task 04 (handler) addresses this by clearing selection when the buffer wraps.
- **`AllocationProfile` reuse**: Uses the existing `AllocationProfile` type (with `Vec<ClassHeapStats>`) rather than introducing a new `ClassAllocation` type. The `AllocationProfile::top_by_size()` method already provides sorted/truncated output for the table.
- **No handler logic**: This task only adds state fields and messages. The actual message handling is in Task 04.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/performance.rs` | Added `AllocationSortColumn` enum, `DEFAULT_MEMORY_SAMPLE_SIZE` constant, 4 new fields to `PerformanceState`, updated both constructors, added `select_next_frame` / `select_prev_frame` / `deselect_frame` / `selected_frame_timing` methods, added 22 new unit tests |
| `crates/fdemon-app/src/message.rs` | Added 3 new `Message` variants: `SelectPerformanceFrame`, `VmServiceMemorySample`, `VmServiceAllocationProfileReceived` |
| `crates/fdemon-app/src/session/mod.rs` | Re-exported `AllocationSortColumn` and `DEFAULT_MEMORY_SAMPLE_SIZE` from `performance` module |
| `crates/fdemon-app/src/lib.rs` | Exported `AllocationSortColumn` from crate root |
| `crates/fdemon-app/src/handler/update.rs` | Added stub match arms for the 3 new `Message` variants (returns `UpdateResult::none()`, handler logic deferred to Task 04) |

### Notable Decisions/Tradeoffs

1. **Stub handlers in update.rs**: Rust's exhaustive match requires every `Message` variant to be handled. Since Task 04 is responsible for the actual handler logic, I added minimal stubs (`UpdateResult::none()`) grouped under a clear comment noting they are placeholders. This keeps the codebase compiling without pre-implementing logic that belongs to a future task.

2. **AllocationSortColumn in performance.rs**: The task gave a choice between `session/performance.rs` and `state.rs`. Placed it in `performance.rs` to keep all allocation-related types co-located. It is re-exported via `session/mod.rs` and `lib.rs` for external access.

3. **DEFAULT_MEMORY_SAMPLE_SIZE exported as `pub`**: The constant is `pub` (not `pub(crate)`) consistent with how the task spec uses it and to allow future TUI/test code in other crates to reference it without hardcoding the value.

### Testing Performed

- `cargo fmt --all` — Passed (auto-formatted)
- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app` — Passed (930 tests, 0 failed; includes 22 new tests in `session/performance.rs`)
- `cargo clippy -p fdemon-app -- -D warnings` — Passed (0 warnings)
- `cargo check --workspace` — Passed (no regressions in fdemon-tui or binary crate)

### Risks/Limitations

1. **Stub handlers**: The three new message variants return `UpdateResult::none()` until Task 04 implements real logic. Any code sending these messages before Task 04 will silently have no effect.
2. **Frame index invalidation on buffer wrap**: As noted in the task spec, when `frame_history` is full and new frames push old ones out, `selected_frame` may point to a different (newer) frame. Task 04 is expected to handle clearing `selected_frame` when the buffer wraps.
