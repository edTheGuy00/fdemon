# Task 02 — Timeline Parser: B/E Pair Reconstruction and Tree Building

**Status:** Not Started
**Wave:** 1
**Agent:** implementor
**Estimated Effort:** 2–3 hours
**Depends On:** —

## Problem

The current `parse_vm_timeline` emits flat `TimelineEvent` records with `phase: TimelinePhase::{Begin, End, Complete, Instant, Other}`. For the Gantt-style Timeline Events view (T05), we need:

1. **Duration reconstruction** — Begin/End pairs must be matched and converted into events with `dur` populated.
2. **Tree nesting** — Within the same `tid`, an event whose `(ts, ts+dur)` interval contains another event's interval becomes its parent.
3. **Per-thread grouping** — Events organized by `tid` (thread ID).
4. **Metadata access** — `ph: "M"` thread-name metadata events are currently dropped at parse time. We need them surfaced so the handler can populate `timeline_thread_name_map` with human-readable names like `"io.flutter.raster"`.

## Files (Write)

- `crates/fdemon-core/src/timeline.rs`

## Files (Read)

- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — verify consumer expectations
- `crates/fdemon-app/src/actions/performance.rs` — verify polling task interaction

## Approach Hints

### New types

```rust
/// A single event node in a per-thread tree. Begin/End pairs are reconciled
/// into a single node with `dur = Some(end_ts - start_ts)`. Complete (`X`)
/// events become nodes directly. Instant (`i`) events become zero-duration
/// nodes. Children are nested by interval containment within the same `tid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineNode {
    pub name: String,
    pub category: Option<String>,
    pub ts: i64,
    pub dur: Option<i64>,
    pub phase: TimelinePhase,
    pub thread: TimelineThread,
    pub children: Vec<TimelineNode>,
}

/// A per-thread track containing the root-level events for one `tid`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TimelineTrack {
    pub tid: i64,
    pub name: Option<String>,
    pub thread: TimelineThread,
    pub root_events: Vec<TimelineNode>,
}

/// Thread-metadata extracted from `ph: "M"` events. Used by the handler
/// to populate `PerformanceState::timeline_thread_name_map`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadMetadata {
    pub tid: i64,
    pub name: String,
}
```

### New functions

```rust
/// Reconstructs duration events from a sorted slice of `TimelineEvent`s
/// belonging to a single `tid`. Algorithm:
/// 1. Walk events in ts order.
/// 2. `Begin` -> push onto stack with start_ts.
/// 3. `End`   -> pop matching stack entry (by name + nearest unclosed),
///               finalize as `dur = end_ts - start_ts`.
/// 4. `Complete` -> emit directly with existing dur.
/// 5. `Instant`  -> emit as zero-dur leaf.
/// 6. Unmatched begins (still on stack at end) are emitted with `dur = None`
///    and a debug log entry.
/// 7. After flattening, nest by interval containment: parent contains child
///    iff parent.ts <= child.ts AND parent.ts + parent.dur >= child.ts + child.dur.
pub fn pair_be_events(events: &[TimelineEvent]) -> Vec<TimelineNode> { ... }

/// Like `parse_vm_timeline` but also returns metadata events. Existing
/// `parse_vm_timeline` continues to filter `ph:"M"` from the event stream
/// (no breaking change for current consumers); this new function returns
/// both event stream and metadata stream for consumers that need names.
pub fn parse_vm_timeline_with_metadata(
    json: &serde_json::Value,
) -> (Vec<TimelineEvent>, Vec<ThreadMetadata>) { ... }

/// Convenience: build full per-thread tracks from a batch of events.
/// Groups by `tid`, calls `pair_be_events` per group.
pub fn build_tracks(events: &[TimelineEvent]) -> BTreeMap<i64, TimelineTrack> { ... }
```

### Algorithm details

**B/E matching is stack-based per `tid`.** When an `End` event arrives with name `X`:

1. Look at the top of the stack for `tid`. If it's a `Begin` with matching `name`, pop and emit a `TimelineNode` with `dur = end.ts - begin.ts`.
2. If names don't match — log debug `"unmatched timeline End: name={...} stack_top={...}"` and pop anyway (defensive).

**Nesting by interval containment** runs after flattening. Sort flattened nodes by `(ts asc, dur desc)`; then for each node, find the nearest unclosed predecessor whose `[ts, ts+dur]` strictly contains the current node's `[ts, ts+dur]`. Equal-ts ties resolve by larger dur becoming the parent.

**Metadata parsing.** In `parse_vm_timeline_with_metadata`, when an event has `ph == "M"` and `name == "thread_name"`, extract `args.name` as the thread label. Push `ThreadMetadata { tid, name }` to the metadata vec.

### Constants

```rust
/// Maximum stack depth for unmatched Begin events, prevents OOM on malformed
/// streams. Events beyond this depth are emitted with `dur = None`.
const MAX_BE_STACK_DEPTH: usize = 256;
```

## Acceptance Criteria

1. **B/E pairing happy path** — `pair_be_events` correctly matches `Begin{name=A, ts=100}, Begin{name=B, ts=150}, End{name=B, ts=180}, End{name=A, ts=200}` into a single root `A` with `dur=100` and child `B` with `dur=30`.
2. **Complete events pass through** — A standalone `X`-phase event with `dur=50` becomes a `TimelineNode` with `dur=Some(50)` without modification.
3. **Instant events become zero-dur leaves** — An `i`-phase event becomes a `TimelineNode` with `dur=None` (or `Some(0)`, document the choice).
4. **Unmatched Begin tolerance** — An unmatched `Begin` at end-of-batch emits a `TimelineNode` with `dur=None` and logs a debug entry. Doesn't crash.
5. **Mismatched B/E names** — `Begin{name=A}, End{name=B}` pops the stack defensively, logs debug, emits A with `dur` to end-ts. Doesn't crash.
6. **Nesting** — Three events: outer `[100,200]`, middle `[120,180]`, inner `[140,160]` build a 3-level tree (outer.children = [middle], middle.children = [inner]).
7. **Per-tid isolation** — Events on `tid=1` do not appear as children of events on `tid=2` even if intervals overlap.
8. **Metadata extraction** — `parse_vm_timeline_with_metadata` returns `ThreadMetadata { tid: 45067, name: "io.flutter.raster" }` for a `ph="M" name="thread_name" args.name="io.flutter.raster"` event.
9. **Backward compat** — Existing `parse_vm_timeline` signature and behavior unchanged (still filters `M` events). All existing parser tests pass without modification.
10. **Serde round-trip** — `TimelineTrack` and `TimelineNode` round-trip through `serde_json` cleanly. New test `track_serde_round_trip`.
11. **Quality gate** — `cargo fmt --all -- --check`, `cargo check -p fdemon-core --all-targets`, `cargo test -p fdemon-core`, `cargo clippy -p fdemon-core --all-targets -- -D warnings` all pass.

## Notes

- `BTreeMap<i64, TimelineTrack>` chosen over `HashMap` for stable iteration order in the renderer (threads listed in `tid` ascending — matches DevTools convention).
- Nesting uses **interval containment** rather than **stack depth at emit time** to be robust to out-of-order events within a `tid`. The polling task receives events in microbatches; events within a batch are sorted by `ts`, but consecutive batches may interleave at the boundaries.
- The `MAX_BE_STACK_DEPTH` guard is defensive; in practice Flutter generates ≤ 10 levels.
- Test coverage should include the `classify_thread_tester_special_case` flow (`io.flutter.test..ui`) — verify it still returns `TimelineThread::Ui` after the parser changes.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/timeline.rs` | Added `TimelineNode`, `TimelineTrack`, `ThreadMetadata` types; `MAX_BE_STACK_DEPTH` constant; `pair_be_events()`, `parse_vm_timeline_with_metadata()`, `build_tracks()` functions; `#[derive(Default)]` on `TimelineThread`; 15+ new unit tests covering all acceptance criteria |

### Notable Decisions/Tradeoffs

1. **`TimelineThread::Other` as derive default**: Clippy required using `#[derive(Default)]` with `#[default]` on `Other` rather than a manual impl. This is cleaner and avoids the `derivable_impls` warning.
2. **`ts: i64` in `TimelineNode`**: Used `i64` (from `u64` in `TimelineEvent`) to allow signed arithmetic in nesting containment checks without explicit casts. The cast happens once at node creation (`event.ts as i64`).
3. **`pair_be_events` takes `&[TimelineEvent]` not `&mut`**: Read-only slice to keep the API functional; `build_tracks` sorts a `Vec<&TimelineEvent>` reference group before collecting owned copies for the call.
4. **`track.name = None` in `build_tracks`**: The raw thread-name string is not stored on `TimelineEvent` (only the classified `TimelineThread` enum is). Callers fill in `track.name` from a `thread_name_map` if they have one. This avoids forcing `build_tracks` to take a `HashMap` parameter it may not always need.
5. **Stack-based nesting in `nest_by_containment`**: Uses a LIFO frame stack instead of O(n²) search. After sorting by `(ts asc, dur desc)` a single pass suffices; frames that don't contain the current node are closed and pushed to their parent before the current node is pushed.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check -p fdemon-core --all-targets` — Passed
- `cargo test -p fdemon-core` — Passed (511 tests)
- `cargo clippy -p fdemon-core --all-targets -- -D warnings` — Passed
- `cargo test --workspace` — Passed (all crates, no regressions)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **`ts as i64` cast**: If the VM ever emits a `ts` larger than `i64::MAX` (≈9.2×10¹⁸ µs ≈ 292,000 years), the cast truncates. In practice Dart VM timestamps are sub-second relative values well within `u64` range; this is not a practical risk.
2. **`build_tracks` does not populate `track.name`**: Intentional — callers with a `thread_name_map` (e.g., the performance handler) must fill it in. Documented in the function's doc comment.
