## Task: Fix `selected_frame` stale index on ring buffer wrap

**Objective**: Prevent `selected_frame` from silently pointing to the wrong frame when `frame_history` wraps at capacity.

**Depends on**: None

**Source**: Review Major Issue #4 (Risks & Tradeoffs Analyzer)

### Scope

- `crates/fdemon-app/src/handler/update.rs:1350-1362`: Adjust `selected_frame` after push eviction
- `crates/fdemon-core/src/performance.rs`: Add `is_full()` or `capacity()` method to `RingBuffer`

### Details

#### Problem

`selected_frame: Option<usize>` is a bare positional index into `frame_history: RingBuffer<FrameTiming>` (capacity 300). When the buffer is full and a new frame arrives, `RingBuffer::push()` calls `pop_front()`, shifting all indices down by 1. The handler never adjusts `selected_frame`, so it drifts to a different frame.

**Timeline:**
1. Buffer has 300 frames. User selects frame at index 50.
2. New frame arrives → `push()` evicts index 0, everything shifts down by 1.
3. `selected_frame = Some(50)` now points to what was index 51 — a different frame.
4. This repeats every frame (~16ms at 60fps), compounding silently.

#### Fix

In the `VmServiceFrameTiming` handler, check whether the buffer was at capacity before pushing (meaning eviction occurred) and compensate:

```rust
Message::VmServiceFrameTiming { session_id, timing } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        let perf = &mut handle.session.performance;
        let was_full = perf.frame_history.is_full();

        perf.frame_history.push(timing);

        // When the buffer was at capacity, pop_front() evicted the oldest
        // entry and shifted all positions by -1. Compensate so that
        // selected_frame continues to refer to the same logical frame.
        if was_full {
            perf.selected_frame = perf.selected_frame.and_then(|i| i.checked_sub(1));
            // checked_sub returns None when i == 0, meaning the selected
            // frame was the oldest and was just evicted — clear selection.
        }

        let len = perf.frame_history.len();
        if len % crate::session::STATS_RECOMPUTE_INTERVAL == 0 {
            perf.recompute_stats();
        }
    }
    UpdateResult::none()
}
```

#### RingBuffer API Addition

Add an `is_full()` method to `RingBuffer` in `fdemon-core/src/performance.rs`:

```rust
impl<T> RingBuffer<T> {
    /// Returns true if the buffer is at capacity and the next push will evict.
    pub fn is_full(&self) -> bool {
        self.buf.len() == self.capacity
    }
}
```

Also add a `capacity()` accessor if not already present:

```rust
    pub fn capacity(&self) -> usize {
        self.capacity
    }
```

### Acceptance Criteria

1. When a frame is selected and the buffer wraps, `selected_frame` continues to point to the same logical frame
2. When the selected frame is the oldest in the buffer and gets evicted, `selected_frame` becomes `None`
3. The frame detail panel displays the correct frame data after buffer wraps
4. No regression in frame navigation (Left/Right keys still work correctly)
5. `RingBuffer::is_full()` is tested

### Testing

```rust
#[test]
fn test_selected_frame_decrements_on_buffer_wrap() {
    let mut state = test_state_with_performance();
    // Fill buffer to capacity
    for i in 0..300 {
        state.process(Message::VmServiceFrameTiming {
            session_id: id,
            timing: frame_timing(i),
        });
    }
    // Select frame at index 50
    state.process(Message::SelectPerformanceFrame { index: Some(50) });
    assert_eq!(selected_frame(&state), Some(50));

    // Push one more frame (causes eviction)
    state.process(Message::VmServiceFrameTiming {
        session_id: id,
        timing: frame_timing(300),
    });

    // selected_frame should decrement to 49
    assert_eq!(selected_frame(&state), Some(49));
}

#[test]
fn test_selected_frame_clears_when_evicted() {
    // Select frame at index 0 (oldest), push new frame
    // selected_frame should become None
}

#[test]
fn test_selected_frame_unchanged_when_buffer_not_full() {
    // Buffer has room, push doesn't evict
    // selected_frame should stay the same
}

#[test]
fn test_ring_buffer_is_full() {
    let mut buf = RingBuffer::new(3);
    assert!(!buf.is_full());
    buf.push(1); buf.push(2); buf.push(3);
    assert!(buf.is_full());
    buf.push(4); // evicts 1
    assert!(buf.is_full()); // still full
}
```

### Notes

- An alternative design is to anchor `selected_frame` to `FrameTiming::number` (the monotonically increasing frame number from the VM) instead of a positional index, resolving to position at render time. This eliminates the class of bug entirely but requires changing the `FrameChart` API. The index-adjustment approach is simpler and sufficient for the current capacity of 300 frames.
- At 60fps, the buffer wraps every ~5 seconds. The bug manifests quickly in normal usage when a user has a frame selected.
- The `checked_sub` returning `None` when `i == 0` is the correct behavior: the user's selected frame has been evicted, so the selection should clear.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/performance.rs` | Added `RingBuffer::is_full()` method and `test_ring_buffer_is_full` test |
| `crates/fdemon-app/src/handler/update.rs` | Modified `VmServiceFrameTiming` handler to capture `was_full` before push and decrement/clear `selected_frame` on eviction |
| `crates/fdemon-app/src/handler/tests.rs` | Added three new tests: `test_selected_frame_decrements_on_buffer_wrap`, `test_selected_frame_clears_when_evicted`, `test_selected_frame_unchanged_when_buffer_not_full` |

### Notable Decisions/Tradeoffs

1. **`perf` local borrow**: Refactored handler body to borrow `&mut handle.session.performance` as a local `perf` variable, which avoids the repeated `handle.session.performance.` prefix across the eviction logic and stats recompute — cleaner and matches the task's suggested code structure.
2. **`capacity()` accessor not added**: `capacity()` was already present in `RingBuffer` (line 347-349 pre-change), so no new accessor was needed.
3. **Test placement**: New tests were added immediately after `test_frame_timing_ignored_for_unknown_session` with a section comment, consistent with the existing Phase 3 section grouping in `handler/tests.rs`.

### Testing Performed

- `cargo check -p fdemon-core` - Passed
- `cargo test -p fdemon-core` - Passed (340 tests, +1 new: `test_ring_buffer_is_full`)
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (955 tests, +3 new selected_frame wrap tests)
- `cargo clippy -p fdemon-core -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt --all -- --check` - Passed

### Risks/Limitations

1. **Single-eviction-per-push assumption**: The fix assumes each `push()` evicts at most one entry (the oldest). This is guaranteed by the `RingBuffer` implementation (`pop_front()` is called at most once per `push()`), so the -1 adjustment is always correct.
