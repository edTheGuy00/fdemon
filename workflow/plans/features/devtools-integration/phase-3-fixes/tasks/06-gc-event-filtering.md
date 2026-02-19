## Task: Add GC Event Filtering to Prevent Scavenge Drowning

**Objective**: Filter or separate GC events by type so that frequent minor (Scavenge) collections don't push out informative major (MarkSweep) events from the ring buffer.

**Depends on**: 05-session-module-split

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-app/src/session/performance.rs`: Add filtering logic or separate buffers
- `crates/fdemon-app/src/handler/update.rs`: Apply filter when storing GC events
- `crates/fdemon-core/src/performance.rs`: Potentially add `GcType` enum or filter helper

### Details

#### Problem

The Dart VM emits GC events for both young-generation (Scavenge) and old-generation (MarkSweep/MarkCompact) collections. Scavenge events are extremely frequent at high allocation rates (multiple per second), while MarkSweep events are rare but more informative (they indicate memory pressure and have significant pause times).

The current 100-slot `gc_history: RingBuffer<GcEvent>` stores all GC events equally. Under high allocation, it fills entirely with Scavenge events, pushing out MarkSweep entries before they can be displayed in Phase 4's TUI panel.

#### Approach Options

**Option A: Filter to major GC only** (simplest)

Only store MarkSweep/MarkCompact events, discard Scavenge events:

```rust
// In handler/update.rs, VmServiceGcEvent handler
if gc_event.gc_type != "Scavenge" {
    handle.session.performance.gc_history.push(gc_event);
}
```

Pros: Simple, ring buffer stays small, major GCs always preserved.
Cons: Loses minor GC frequency data (could be useful for allocation rate estimation).

**Option B: Separate ring buffers by GC type**

```rust
pub struct PerformanceState {
    pub memory_history: RingBuffer<MemoryUsage>,
    pub major_gc_history: RingBuffer<GcEvent>,  // MarkSweep/MarkCompact, 50 slots
    pub minor_gc_history: RingBuffer<GcEvent>,  // Scavenge, 20 slots
    pub frame_history: RingBuffer<FrameTiming>,
    // ...
}
```

Pros: Preserves both types, ring buffer sizes tuned independently.
Cons: More complex, two buffers to manage, Phase 4 TUI needs to merge for display.

**Option C: Priority ring buffer** (deferred)

A ring buffer that reserves N slots for high-priority items and evicts low-priority items first. Too complex for this fix.

**Recommended: Option A** for simplicity. If Phase 4 needs minor GC data, switch to Option B then.

#### Implementation

1. **Add `is_major_gc()` helper** to `GcEvent` in `fdemon-core/src/performance.rs`:

```rust
impl GcEvent {
    /// Returns true if this is a major GC event (MarkSweep, MarkCompact).
    /// Minor GC (Scavenge) events are more frequent but less informative.
    pub fn is_major_gc(&self) -> bool {
        self.gc_type != "Scavenge"
    }
}
```

2. **Filter in the handler** at `handler/update.rs` (the `VmServiceGcEvent` handler):

```rust
Message::VmServiceGcEvent { session_id, gc_event } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Only store major GC events to prevent Scavenge drowning
        if gc_event.is_major_gc() {
            handle.session.performance.gc_history.push(gc_event);
        }
    }
    UpdateResult::none()
}
```

3. **Optionally reduce buffer size** from 100 to 50 since major GCs are rarer.

### Acceptance Criteria

1. Scavenge GC events are not stored in `gc_history`
2. MarkSweep and MarkCompact events are stored normally
3. `GcEvent` has an `is_major_gc()` method
4. Existing GC-related tests updated
5. New test: verify Scavenge events are filtered out
6. New test: verify MarkSweep events are stored
7. `cargo test --workspace` passes
8. `cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[test]
fn test_scavenge_gc_events_filtered() {
    // Setup: session with empty gc_history
    // Action: send VmServiceGcEvent with gc_type = "Scavenge"
    // Assert: gc_history remains empty
}

#[test]
fn test_major_gc_events_stored() {
    // Setup: session with empty gc_history
    // Action: send VmServiceGcEvent with gc_type = "MarkSweep"
    // Assert: gc_history has 1 entry
}

#[test]
fn test_is_major_gc() {
    assert!(!GcEvent { gc_type: "Scavenge".into(), .. }.is_major_gc());
    assert!(GcEvent { gc_type: "MarkSweep".into(), .. }.is_major_gc());
    assert!(GcEvent { gc_type: "MarkCompact".into(), .. }.is_major_gc());
}
```

### Notes

- This is a minor enhancement, not a blocking fix — it prevents data quality issues in the Phase 4 TUI
- The `gc_type` field comes from the Dart VM Service event; values are `"Scavenge"`, `"MarkSweep"`, `"MarkCompact"`
- If Phase 4 needs minor GC frequency data, upgrade to Option B (separate buffers) at that time
- Consider logging filtered Scavenge events at `trace!` level for debugging
