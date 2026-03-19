## Task: Scale Polling Intervals by `FlutterMode`

**Objective**: Apply a multiplier to all polling intervals when running in profile or release mode, reducing VM Service pressure from ~8 RPCs/sec to <= 2 RPCs/sec with the reporter's aggressive config.

**Depends on**: 03-thread-flutter-mode

**Estimated Time**: 1-1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/performance.rs`: Use the `mode` parameter (from task 03) to scale `memory_interval` and `alloc_interval` in profile/release modes; add profile-mode minimum constants
- `crates/fdemon-app/src/actions/network.rs`: Use the `mode` parameter to scale `poll_interval_ms` in profile/release modes; add profile-mode minimum constant

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs`: `FlutterMode` enum for matching

### Details

#### Current Interval Constants

| Constant | File | Value | Purpose |
|----------|------|-------|---------|
| `PERF_POLL_MIN_MS` | `actions/performance.rs:28` | 500 | Floor for `performance_refresh_ms` |
| `ALLOC_PROFILE_POLL_MIN_MS` | `actions/performance.rs:35` | 1000 | Floor for `allocation_profile_interval_ms` |
| `NETWORK_POLL_MIN_MS` | `actions/network.rs:32` | 500 | Floor for `network_poll_interval_ms` |

#### New Constants for Profile/Release Mode

Add named constants with derivation comments:

```rust
// In actions/performance.rs:

/// Multiplier applied to polling intervals in profile/release mode.
/// Profile mode has tighter frame budgets (16ms vs ~100ms tolerance in debug).
/// A 3x multiplier reduces RPC frequency enough to eliminate observable jank
/// while keeping data reasonably fresh for monitoring.
const PROFILE_MODE_MULTIPLIER: u64 = 3;

/// Minimum performance refresh interval in profile/release mode (ms).
/// Derived from: reporter's 500ms setting × 3x multiplier = 1500ms,
/// raised to 2000ms for safety margin against heap walk latency.
const PROFILE_PERF_POLL_MIN_MS: u64 = 2000;

/// Minimum allocation profile interval in profile/release mode (ms).
/// getAllocationProfile forces a full heap walk — the primary lag source.
/// 5000ms gives the app 300 frames (at 60fps) between heap walks.
const PROFILE_ALLOC_POLL_MIN_MS: u64 = 5000;

// In actions/network.rs:

/// Multiplier applied to network poll interval in profile/release mode.
const PROFILE_MODE_MULTIPLIER: u64 = 3;

/// Minimum network poll interval in profile/release mode (ms).
/// Network polling is less expensive than memory/alloc polling,
/// but still adds VM Service round-trip latency.
const PROFILE_NETWORK_POLL_MIN_MS: u64 = 3000;
```

#### Scaling Logic

In `spawn_performance_polling`, after the existing interval clamping (~lines 77-79), apply mode-aware scaling:

```rust
// Existing clamping
let memory_interval_ms = performance_refresh_ms.max(PERF_POLL_MIN_MS);
let alloc_interval_ms = allocation_profile_interval_ms.max(ALLOC_PROFILE_POLL_MIN_MS);

// Mode-aware scaling (NEW)
let (memory_interval_ms, alloc_interval_ms) = match mode {
    FlutterMode::Profile | FlutterMode::Release => {
        let mem = (memory_interval_ms * PROFILE_MODE_MULTIPLIER).max(PROFILE_PERF_POLL_MIN_MS);
        let alloc = (alloc_interval_ms * PROFILE_MODE_MULTIPLIER).max(PROFILE_ALLOC_POLL_MIN_MS);
        (mem, alloc)
    }
    FlutterMode::Debug => (memory_interval_ms, alloc_interval_ms),
};

let memory_interval = Duration::from_millis(memory_interval_ms);
let alloc_interval = Duration::from_millis(alloc_interval_ms);
```

Same pattern in `spawn_network_monitoring` (~line 58):

```rust
let poll_interval_ms = poll_interval_ms.max(NETWORK_POLL_MIN_MS);

// Mode-aware scaling (NEW)
let poll_interval_ms = match mode {
    FlutterMode::Profile | FlutterMode::Release => {
        (poll_interval_ms * PROFILE_MODE_MULTIPLIER).max(PROFILE_NETWORK_POLL_MIN_MS)
    }
    FlutterMode::Debug => poll_interval_ms,
};
```

#### RPC Reduction with Reporter's Config

| Source | Debug RPCs/sec | Profile RPCs/sec (after this task) |
|--------|---------------|-----------------------------------|
| Memory snapshot + sample | 4.0 (2 per 500ms tick) | 1.0 (2 per 2000ms tick) |
| Allocation profile | 1.0 (1 per 1000ms tick) | 0.2 (1 per 5000ms tick) |
| Network poll | 1.0 (1 per 1000ms tick) | 0.33 (1 per 3000ms tick) |
| **Total** | **6.0** (after task 01 dedup) | **1.53** |

Note: the "before" column already accounts for task 01's dedup (3→2 RPCs per memory tick). Without dedup, debug was 8.0 RPCs/sec.

### Acceptance Criteria

1. In `Debug` mode, intervals are unchanged from current behavior (existing minimums apply)
2. In `Profile` mode, `memory_interval` is >= 2000ms even when `performance_refresh_ms = 500`
3. In `Profile` mode, `alloc_interval` is >= 5000ms even when `allocation_profile_interval_ms = 1000`
4. In `Profile` mode, `network_poll_interval` is >= 3000ms even when `network_poll_interval_ms = 500`
5. In `Release` mode, same scaling as Profile mode
6. The multiplier is applied AFTER the base minimum clamp (so `500ms × 3 = 1500ms`, then raised to 2000ms minimum)
7. All existing tests pass: `cargo test --workspace`
8. New unit tests verify interval calculation for each mode

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_mode_uses_base_intervals() {
        // Given performance_refresh_ms = 500 and mode = Debug
        // Then effective interval = 500ms (base minimum, no multiplier)
    }

    #[test]
    fn test_profile_mode_scales_memory_interval() {
        // Given performance_refresh_ms = 500 and mode = Profile
        // Then effective interval = max(500 * 3, 2000) = 2000ms
    }

    #[test]
    fn test_profile_mode_scales_alloc_interval() {
        // Given allocation_profile_interval_ms = 1000 and mode = Profile
        // Then effective interval = max(1000 * 3, 5000) = 5000ms
    }

    #[test]
    fn test_profile_mode_respects_user_higher_interval() {
        // Given performance_refresh_ms = 10000 and mode = Profile
        // Then effective interval = max(10000 * 3, 2000) = 30000ms
        // User's explicit high value is respected (with multiplier)
    }

    #[test]
    fn test_release_mode_uses_same_scaling_as_profile() {
        // Given mode = Release
        // Then same intervals as Profile
    }

    #[test]
    fn test_profile_network_interval_scales() {
        // Given network_poll_interval_ms = 1000 and mode = Profile
        // Then effective interval = max(1000 * 3, 3000) = 3000ms
    }
}
```

To make interval calculations testable, consider extracting the clamping+scaling logic into a pure function:

```rust
fn effective_interval(base_ms: u64, base_min: u64, mode: FlutterMode, profile_multiplier: u64, profile_min: u64) -> u64 {
    let clamped = base_ms.max(base_min);
    match mode {
        FlutterMode::Profile | FlutterMode::Release => (clamped * profile_multiplier).max(profile_min),
        FlutterMode::Debug => clamped,
    }
}
```

### Notes

- `PROFILE_MODE_MULTIPLIER = 3` is a reasonable starting point. The BUG.md's "Further Considerations" suggests a `profile_polling_multiplier` config key — defer this to a follow-up. Hardcode the multiplier for now with a comment noting it could be configurable.
- `Release` mode gets the same treatment as `Profile`. In practice, release builds rarely connect to fdemon (no VM Service), but if they do (via `--enable-vm-service`), the same pressure concerns apply.
- The constants follow the project's `SCREAMING_SNAKE_CASE` convention per `CODE_STANDARDS.md`.
- The scaling is applied in the spawn functions (`actions/performance.rs`, `actions/network.rs`), not in the handler or config layer. This keeps the interval logic co-located with the timer creation and avoids leaking mode awareness into the TEA update function.

---

## Completion Summary

**Status:** Not Started
