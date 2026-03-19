## Task: Set `MissedTickBehavior::Skip` on All Polling Intervals

**Objective**: Prevent RPC burst cascades after slow VM Service calls by setting `MissedTickBehavior::Skip` on all three polling intervals (memory, allocation, network).

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/performance.rs`: Add `MissedTickBehavior::Skip` to `memory_tick` and `alloc_tick` intervals (~lines 115-116)
- `crates/fdemon-app/src/actions/network.rs`: Add `MissedTickBehavior::Skip` to `poll_tick` interval (~lines 154-155)

**Files Read (Dependencies):**
- None (self-contained change)

### Details

#### The Problem

All three `tokio::time::interval` calls use the default `MissedTickBehavior::Burst`. When a VM Service RPC takes longer than the tick interval (common in profile mode where frame budgets are tighter and the VM is under AOT constraints), the Burst behavior fires all missed ticks immediately on recovery. This creates back-to-back RPC storms that amplify the lag.

Example: if `performance_refresh_ms = 500ms` and a `getMemoryUsage` call takes 1.5 seconds, Burst mode fires 2 extra ticks immediately after recovery — 3 RPCs in rapid succession instead of 1.

#### The Fix

Add `.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip)` after each interval creation. `Skip` drops missed ticks entirely and resumes at the next natural boundary — exactly the right behavior for a polling loop where stale data is worthless.

**In `actions/performance.rs` (~lines 115-116):**

```rust
let mut memory_tick = tokio::time::interval(memory_interval);
memory_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

let mut alloc_tick = tokio::time::interval(alloc_interval);
alloc_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
```

**In `actions/network.rs` (~lines 154-155):**

```rust
let mut poll_tick = tokio::time::interval(tokio::time::Duration::from_millis(poll_interval_ms));
poll_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
```

#### Behavioral Change

| Scenario | Before (Burst) | After (Skip) |
|----------|----------------|--------------|
| Normal tick (RPC completes within interval) | Next tick fires on schedule | Same — no change |
| RPC takes 2x the interval | 2 ticks fire immediately on recovery | 1 tick fires at next boundary |
| RPC takes 5x the interval | 5 ticks fire back-to-back | 1 tick fires at next boundary |

### Acceptance Criteria

1. `MissedTickBehavior::Skip` is set on all three interval timers (`memory_tick`, `alloc_tick`, `poll_tick`)
2. No burst recovery occurs after slow VM calls — polling resumes at the next natural interval boundary
3. All existing tests pass: `cargo test -p fdemon-app`
4. `cargo clippy --workspace -- -D warnings` — no new warnings

### Testing

This change is difficult to unit test directly (tokio interval behavior is runtime-dependent), but correctness can be verified by:

1. **Code inspection**: Confirm each `tokio::time::interval()` call is immediately followed by `.set_missed_tick_behavior(MissedTickBehavior::Skip)`
2. **Existing test suite**: `cargo test -p fdemon-app` — all 1,511 tests pass (none depend on Burst behavior)
3. **Manual verification**: With the Phase 1 reproduction config, profile mode no longer shows burst-recovery jank after a slow frame

### Notes

- This is a minimal, surgical change — 3 lines added across 2 files.
- `MissedTickBehavior::Skip` is the standard choice for polling loops. `Delay` (which shifts the schedule) would also work but changes the observable polling cadence. `Skip` is simpler and more predictable.
- The first tick of `tokio::time::interval` fires immediately (at creation time). `Skip` does not affect this — it only governs recovery after a missed tick.
- This change has no interaction with the mode-aware scaling in later tasks. It's a standalone improvement that benefits all modes.

---

## Completion Summary

**Status:** Done
**Branch:** fix/profile-mode-lag-25

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/performance.rs` | Added `set_missed_tick_behavior(MissedTickBehavior::Skip)` on `memory_tick` (line 120) and `alloc_tick` (line 123) immediately after interval creation |
| `crates/fdemon-app/src/actions/network.rs` | Added `set_missed_tick_behavior(MissedTickBehavior::Skip)` on `poll_tick` (line 156) immediately after interval creation |

### Notable Decisions/Tradeoffs

1. **Placement**: Each `.set_missed_tick_behavior()` call is placed on the line immediately following the corresponding `tokio::time::interval()` call, matching the pattern shown in the task spec and making the relationship visually obvious.
2. **No functional change for normal operation**: When RPCs complete within their tick interval, `Skip` and `Burst` are identical — only recovery-after-slowness behavior differs.

### Testing Performed

- `cargo fmt --all` - Passed (no formatting changes needed)
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1797 tests, 0 failures)
- `cargo clippy --workspace -- -D warnings` - Passed (no new warnings)

### Risks/Limitations

1. **Timing correctness**: The first tick of `tokio::time::interval` fires immediately at creation regardless of `MissedTickBehavior`. This is unaffected by this change — only missed-tick recovery behavior changes.
2. **Untestable directly**: Tokio interval behavior is runtime-dependent; correctness is verified by code inspection and the existing test suite (which has no tests depending on Burst recovery behavior).
