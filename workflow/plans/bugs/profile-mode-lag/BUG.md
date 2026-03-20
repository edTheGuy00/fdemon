# Bugfix Plan: Profile Mode Lag (Issue #25)

## TL;DR

Users experience ~1-second freezes when running Flutter in profile mode via fdemon. Root cause: DevTools polling (`getAllocationProfile`, `getMemoryUsage`) fires unconditionally regardless of build mode or active panel, causing VM Service calls that pause the Dart isolate. The reporter's aggressive polling config (500ms perf, 1000ms alloc, 1000ms network) produces ~7 VM RPCs/second — including heap walks — that are imperceptible in debug mode but cause visible jank in profile mode where frame budgets are tighter. Fix: add mode-aware polling throttling and panel-gated monitoring.

## Bug Reports

### Bug 1: Profile Mode Lag from DevTools Polling

**Symptom:** ~1-second freezes in the Flutter app when running a profile build via fdemon. No freezes when using `flutter run` directly.

**Expected:** Profile mode should run with minimal interference from fdemon's monitoring, comparable to `flutter run` alone.

**Reporter Config (from issue #25):**
```toml
# Aggressive polling settings
performance_refresh_ms = 500          # minimum allowed (fires 2x/sec)
allocation_profile_interval_ms = 1000 # minimum allowed (heap walk 1x/sec)
network_poll_interval_ms = 1000       # minimum allowed (1x/sec)
network_auto_record = true
default_panel = "performance"
dap.enabled = true
mode = "profile"
```

**Root Cause Analysis:**

1. **`getAllocationProfile` is the primary suspect.** It forces the VM to walk the entire Dart heap (`performance.rs:14-16` comment acknowledges this). At the reporter's `1000ms` interval, this heap walk fires every second — matching the reported freeze cadence exactly.

2. **Double `getMemoryUsage` per tick.** Each `performance_refresh_ms` tick fires `getMemoryUsage` twice: once in the memory snapshot path and again inside `get_memory_sample` (`actions/performance.rs:133-180`). At 500ms intervals, that's 4 `getMemoryUsage` calls/second plus 2 `getIsolate` calls/second.

3. **No mode awareness.** Performance monitoring starts unconditionally on `VmServiceConnected` (`handler/update.rs:1307-1404`) regardless of build mode. Profile/release modes get identical polling pressure as debug mode.

4. **No panel gating.** Performance polling runs from VM connect until disconnect, even when the user is viewing logs, not DevTools. The allocation profiler runs even when the allocation table isn't visible.

5. **Network polling never stops.** Once the user enters the Network tab, `spawn_network_monitoring` runs indefinitely — switching to another tab doesn't pause it (`actions/network.rs:159-201`).

6. **`MissedTickBehavior::Burst` default.** If a VM call takes longer than the tick interval (likely during profile mode compilation), missed ticks fire back-to-back on recovery, creating RPC bursts.

**Total VM Service RPCs with reporter's config:**
| Source | Interval | Calls/tick | RPCs/sec |
|--------|----------|------------|----------|
| Memory snapshot | 500ms | 1x `getMemoryUsage` | 2.0 |
| Memory sample | 500ms | 1x `getMemoryUsage` + 1x `getIsolate` | 4.0 |
| Allocation profile | 1000ms | 1x `getAllocationProfile` | 1.0 |
| Network poll | 1000ms | 1x `ext.dart.io.getHttpProfile` | 1.0 |
| **Total** | | | **8.0 RPCs/sec** |

**Affected Files:**
- `crates/fdemon-app/src/actions/performance.rs` — dual-timer polling loop
- `crates/fdemon-app/src/actions/network.rs` — network polling loop
- `crates/fdemon-app/src/handler/update.rs:1307-1404` — unconditional monitoring trigger
- `crates/fdemon-daemon/src/vm_service/performance.rs` — duplicate `getMemoryUsage` call
- `crates/fdemon-app/src/config/types.rs` — interval defaults and minimums

---

## Affected Modules

- `crates/fdemon-app/src/actions/performance.rs`: Add mode-aware interval scaling, deduplicate `getMemoryUsage`, set `MissedTickBehavior::Skip`
- `crates/fdemon-app/src/actions/network.rs`: Add panel-aware pause/resume, set `MissedTickBehavior::Skip`
- `crates/fdemon-app/src/handler/update.rs`: Pass `FlutterMode` into `StartPerformanceMonitoring` action
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Send pause/resume signals when switching panels
- `crates/fdemon-daemon/src/vm_service/performance.rs`: Reuse `getMemoryUsage` result in `get_memory_sample`
- `crates/fdemon-app/src/config/types.rs`: Raise minimum intervals for profile/release modes
- `example/app3/.fdemon/launch.toml`: Add profile mode config for reproduction testing

---

## Phases

### Phase 1: Reproduction Setup

**Goal:** Add profile mode launch config to example app3 so the issue can be reproduced locally. Mirror the reporter's aggressive polling settings.

**Steps:**
1. **Add profile mode config to `example/app3/.fdemon/launch.toml`**
   - Add a new `[[configurations]]` entry with `mode = "profile"` and `auto_start = true`
   - Keep existing configs for comparison testing between debug and profile

2. **Add aggressive DevTools polling to `example/app3/.fdemon/config.toml`**
   - Set `performance_refresh_ms = 500`
   - Set `allocation_profile_interval_ms = 1000`
   - Set `network_poll_interval_ms = 1000`
   - Set `network_auto_record = true`
   - Set `default_panel = "performance"`
   - These mirror the reporter's config to maximize reproduction likelihood

**Measurable Outcomes:**
- Running `cargo run -- example/app3` launches in profile mode
- Lag is observable when DevTools performance panel is active
- Switching to debug config shows no lag with identical settings

---

### Phase 2: Core Fix — Mode-Aware Polling

**Goal:** Reduce VM Service pressure in profile/release modes by scaling intervals, deduplicating calls, and gating on panel visibility.

**Steps:**

1. **Deduplicate `getMemoryUsage` in performance polling**
   - In `actions/performance.rs`, call `getMemoryUsage` once per tick and reuse the result for both `MemorySnapshot` and `MemorySample`
   - Reduces RPCs per memory tick from 3 to 2

2. **Set `MissedTickBehavior::Skip` on all polling intervals**
   - In `actions/performance.rs` and `actions/network.rs`, set `.set_missed_tick_behavior(MissedTickBehavior::Skip)` on tokio intervals
   - Prevents RPC bursts after slow VM calls

3. **Pass `FlutterMode` through the monitoring chain**
   - Add `mode: FlutterMode` field to `UpdateAction::StartPerformanceMonitoring` and `StartNetworkMonitoring`
   - Thread it from `LaunchConfig` (or default `Debug`) through `update.rs` → `process.rs` → `actions/performance.rs`

4. **Scale intervals by mode**
   - In `actions/performance.rs`, when mode is `Profile` or `Release`, apply a multiplier (e.g., 3x) to both `performance_refresh_ms` and `allocation_profile_interval_ms`
   - Raise effective minimums: 2000ms for perf, 5000ms for alloc in profile mode
   - In `actions/network.rs`, apply same multiplier to `network_poll_interval_ms`

5. **Gate allocation profiling on panel visibility**
   - Only run the allocation timer when the Performance panel (allocation sub-tab) is actually visible
   - Add a channel or flag that `handle_switch_panel` toggles

**Measurable Outcomes:**
- Profile mode with reporter's config produces <= 2 RPCs/sec (down from 8)
- No visible lag in profile mode with default settings
- Debug mode behavior is unchanged

---

### Phase 3: Panel-Aware Monitoring Lifecycle

**Goal:** Stop polling entirely when the user isn't viewing DevTools panels.

**Steps:**

1. **Pause performance monitoring when not on DevTools**
   - When the user is in log view (not DevTools), suspend the performance polling task via a pause channel
   - Resume when they switch back to a DevTools panel

2. **Stop network monitoring when leaving Network tab**
   - Send a pause signal to the network polling task when switching away from the Network panel
   - Resume when switching back

3. **Lazy-start performance monitoring**
   - Instead of starting on `VmServiceConnected`, defer until the user first enters a DevTools panel
   - Keep frame timing (event-driven, no polling) always active since it's free

**Measurable Outcomes:**
- Zero VM Service polling RPCs when viewing logs only
- Monitoring resumes within one interval when switching to DevTools

---

## Edge Cases & Risks

### Backwards Compatibility
- **Risk:** Users relying on background performance data collection (e.g., memory history populated before opening DevTools)
- **Mitigation:** Phase 3's lazy-start could be gated behind a config flag, or keep a very slow background poll (e.g., 30s) for history seeding

### Mode Detection Without Launch Config
- **Risk:** When no `LaunchConfig` is present (bare device run), the mode is unknown — `FlutterProcess::spawn_with_device` doesn't set a mode flag
- **Mitigation:** Default to `Debug` when mode is unknown (current behavior), or query `ext.flutter.activeDevToolsServerAddress` / check for JIT vs AOT at runtime

### Allocation Profile Data Freshness
- **Risk:** Users opening the allocation table in profile mode may see stale data if interval is scaled to 5s+
- **Mitigation:** Add a manual "Refresh" action or fire one immediate poll when the allocation sub-tab gains focus

---

## Further Considerations

1. **Should we auto-detect profile mode at runtime?** The Dart VM Service could be queried for compilation mode, avoiding reliance on the launch config. This would handle cases where users manually pass `--profile` via `extra_args`.

2. **Should interval scaling be configurable?** A `profile_polling_multiplier` config key would let power users tune the tradeoff between data freshness and jank.

3. **DAP interaction?** The reporter has `dap.enabled = true`. Need to verify the DAP server isn't adding additional VM Service pressure on top of the polling.

---

## Task Dependency Graph

```
Phase 1
└── 01-add-profile-config-to-example

Phase 2
├── 02-dedup-memory-rpc
├── 03-missed-tick-skip
├── 04-thread-flutter-mode
│   └── 05-scale-intervals-by-mode (depends on: 04)
│       └── 06-gate-alloc-on-panel (depends on: 05)

Phase 3
├── 07-pause-perf-when-not-devtools (depends on: 05)
├── 08-pause-network-on-tab-switch
└── 09-lazy-start-monitoring (depends on: 07)
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `example/app3` has a profile mode launch config with aggressive polling settings
- [ ] Lag is reproducible by running `cargo run -- example/app3`

### Phase 2 Complete When:
- [ ] `getMemoryUsage` is called once per tick (not twice)
- [ ] `MissedTickBehavior::Skip` is set on all polling intervals
- [ ] Profile mode intervals are scaled up (effective 2000ms+ perf, 5000ms+ alloc)
- [ ] No visible lag in profile mode with reporter's original config
- [ ] Debug mode performance is unchanged

### Phase 3 Complete When:
- [ ] Zero polling RPCs when viewing logs (not DevTools)
- [ ] Network polling pauses when switching away from Network tab
- [ ] Monitoring lazy-starts on first DevTools panel visit

---

## Milestone Deliverable

Profile mode runs without fdemon-induced lag. DevTools polling is mode-aware and panel-gated, reducing VM Service pressure from ~8 RPCs/sec to ~1-2 RPCs/sec in profile mode (and zero when not viewing DevTools). Example app3 provides a ready-made reproduction config for regression testing.

---

## References

- [Issue #25 — Lags when building with profiling](https://github.com/edTheGuy00/fdemon/issues/25)
- Reporter: YRuzik
- `getAllocationProfile` heap walk cost: `crates/fdemon-app/src/actions/performance.rs:14-16`
- Dart VM Service Protocol: `getMemoryUsage`, `getAllocationProfile`, `getIsolate`
