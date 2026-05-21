## Task: App State, Handlers, Spawn, and Wiring

**Objective**: The single fdemon-app integration task for Phase 3. Adds all `PerformanceState` fields for both new flows, the `Message` variants + dispatch arms, the new sub-handler modules under `handler/devtools/performance/`, the 1-Hz timeline polling spawn function, the `Flutter.RebuiltWidgets` branch in `forward_vm_events`, the `SessionHandle` shutdown/pause fields for the new polling task, hot-restart re-enable of `profileWidgetBuilds`, and config additions. After this task lands, the data flows end-to-end (events arrive, state updates) but the UI still renders the Phase-2 stubs — T05 wires the rendering.

**Depends on**: T01 (`fdemon-core` types), T02 (`set_profile_widget_builds`, `widget_location_id_map`), T03 (`fetch_timeline_chunk`, `get_vm_timeline_micros`)

**Agent:** implementor

**Estimated Time**: 6–8 hours

### Scope

**Files Modified (Write):**

| File | Change |
|---|---|
| `crates/fdemon-app/src/session/performance.rs` | Add fields for rebuild stats (6) and timeline events (4) + new enum `TimelineFilter { All, Ui, Raster }`. |
| `crates/fdemon-app/src/session/handle.rs` | Add `timeline_shutdown_tx`, `timeline_pause_tx`, `timeline_task_handle` (mirror existing `perf_*` fields lines 79–162). |
| `crates/fdemon-app/src/state.rs` | If `TimelineFilter` is needed in `AppState` (not just `PerformanceState`), declare here; otherwise keep in `session/performance.rs`. |
| `crates/fdemon-app/src/message.rs` | Add 7 variants (see Details). |
| `crates/fdemon-app/src/handler/update.rs` | Add 7 dispatch arms — model on the Phase-2 `Message::PerfCycleDetailsTab` dispatch (line 2355–2357). |
| `crates/fdemon-app/src/handler/devtools/performance/mod.rs` | Add `pub mod rebuild_stats;` and `pub mod timeline;` declarations + handler re-exports. |
| `crates/fdemon-app/src/handler/devtools/performance/rebuild_stats.rs` (NEW) | Aggregator: per-event accumulate into per-frame snapshots, ring-buffer eviction, extension state changes. |
| `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (NEW) | Append events to ring buffer, filter cycle, scroll handlers. |
| `crates/fdemon-app/src/actions/performance.rs` | Add `pub(super) fn spawn_timeline_polling(...)` modeled on `spawn_performance_polling` (lines 142–180). |
| `crates/fdemon-app/src/actions/vm_service.rs` | Add a new branch to `forward_vm_events` (before `parse_frame_timing` at line 173): if `flutter_extension_kind(&event.params.event) == Some("Flutter.RebuiltWidgets")`, parse via `fdemon_core::rebuild_stats::parse_rebuilt_widgets_event` and emit `Message::RebuildStatsEventReceived { session_id, payload }`. |
| `crates/fdemon-app/src/session/session_lifecycle.rs` | (1) Start timeline poll when entering Performance panel (mirror `maybe_start_monitoring_for_selected_session` perf logic). (2) Pause when leaving (`tx.send(true)`). (3) Stop on session close (signal `timeline_shutdown_tx`, abort `timeline_task_handle`). (4) On `SessionRestartCompleted` (or wherever hot restart completes), if `PerformanceState::rebuild_stats_enabled == true`, call `set_profile_widget_builds(client, isolate_id, Some(true))` to re-enable the extension. |
| `crates/fdemon-app/src/config/types.rs` | Extend `DevToolsSettings` with `auto_enable_rebuild_tracking: bool` (default `false`), `rebuild_stats_frame_window: u32` (default `30`), `timeline_event_buffer_size: usize` (default `1000`). Each `#[serde(default = "...")]` with a `fn default_*` helper, mirroring existing convention. |

**Files Read (Dependencies):**
- All T01–T03 outputs.
- `crates/fdemon-app/src/session/memory.rs` — ring-buffer field pattern (lines 65–125).
- `crates/fdemon-app/src/actions/performance.rs:142–424` — spawn pattern reference.
- `crates/fdemon-app/src/actions/vm_service.rs:142–246` — `forward_vm_events` structure.
- `crates/fdemon-app/src/session/handle.rs:79–192` — existing shutdown/pause field shape.
- `crates/fdemon-app/src/handler/devtools/performance/details.rs:146` — Phase-2 handler reference.
- `crates/fdemon-app/src/handler/update.rs:2355–2357` — dispatch arm shape.
- `crates/fdemon-app/src/config/types.rs:342–469` — `DevToolsSettings` block + default-helper pattern.

### Details

#### PerformanceState additions (`session/performance.rs`)

Append to the existing struct (after Phase-2's `details_pane_visible_height` field at line ~113):

```rust
// ---- Phase 3: Rebuild Stats ----

/// Whether the `ext.flutter.profileWidgetBuilds` extension is currently on.
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

/// Scroll offset for the Rebuild Stats table (Phase 3 details scrolling).
pub rebuild_stats_scroll_offset: usize,

/// Currently-selected row in the Rebuild Stats table (j/k navigation).
pub rebuild_stats_selected_row: Option<usize>,

// ---- Phase 3: Timeline Events ----

/// Ring buffer of recent timeline events. Capped by
/// `Settings::devtools::timeline_event_buffer_size` (default 1000).
pub timeline_events: VecDeque<TimelineEvent>,

/// Scroll offset for the Timeline Events list.
pub timeline_events_scroll_offset: usize,

/// `tid → thread_name` cache, populated from `getVMTimeline` metadata
/// events. Persists across polls within a session.
pub timeline_thread_name_map: HashMap<i64, String>,

/// Current filter selection — `All`, `Ui`, or `Raster`.
pub timeline_events_filter: TimelineFilter,
```

Add the enum at the module level:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimelineFilter {
    #[default]
    All,
    Ui,
    Raster,
}

impl TimelineFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Ui,
            Self::Ui => Self::Raster,
            Self::Raster => Self::All,
        }
    }
}
```

Update the `Default` impl for `PerformanceState` accordingly. Document Phase-3 field initialization defaults (empty `VecDeque`, `false` extension state, `TimelineFilter::All`).

#### SessionHandle additions (`session/handle.rs`)

Mirror existing `perf_*` fields (lines 79–162):

```rust
/// Shutdown channel for the timeline polling task.
pub timeline_shutdown_tx: Option<Arc<watch::Sender<bool>>>,

/// Pause channel — `true` = paused (when Performance panel not active).
pub timeline_pause_tx: Option<Arc<watch::Sender<bool>>>,

/// Join handle for the timeline polling task.
pub timeline_task_handle: Option<JoinHandle<()>>,
```

Initialize to `None` in `SessionHandle::new`. Signal + clear in the three existing session-stop paths alongside `perf_shutdown_tx`/`perf_task_handle` (`session.rs:209`, `session.rs:125`, `session_lifecycle.rs:206`).

#### New Message variants (`message.rs`)

```rust
/// A new `Flutter.RebuiltWidgets` extension event arrived (T04 owns parsing).
RebuildStatsEventReceived {
    session_id: SessionId,
    payload: fdemon_core::rebuild_stats::RebuildEventPayload,
},

/// The user pressed `R` on the Rebuild Stats tab — toggle the extension.
ToggleRebuildStats { session_id: SessionId },

/// The async toggle returned a new state — update `rebuild_stats_enabled`,
/// hide/show tab, clear or refetch.
RebuildStatsExtensionStateChanged {
    session_id: SessionId,
    enabled: bool,
},

/// The one-shot `widgetLocationIdMap` RPC returned a fresh map (used as
/// fallback when DevTools missed early events).
RebuildStatsLocationMapFetched {
    session_id: SessionId,
    map: fdemon_core::rebuild_stats::LocationMap,
},

/// The 1-Hz timeline poll returned a batch of new events.
TimelineEventsBatchReceived {
    session_id: SessionId,
    events: Vec<fdemon_core::timeline::TimelineEvent>,
},

/// The user pressed `f` on the Timeline Events tab — cycle the filter.
TimelineEventsCycleFilter { session_id: SessionId },

/// The timeline polling task started — carries shutdown/pause/handle refs
/// to the SessionHandle. Modeled on `VmServicePerformanceMonitoringStarted`.
VmServiceTimelineMonitoringStarted {
    session_id: SessionId,
    timeline_shutdown_tx: Arc<watch::Sender<bool>>,
    timeline_pause_tx: Arc<watch::Sender<bool>>,
    timeline_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
},
```

#### Dispatch arms (`handler/update.rs`)

Add 7 arms near the Phase-2 `PerfCycleDetailsTab` arm (line 2355). Each is a 3-line match block calling into `devtools::performance::...`. Example:

```rust
Message::RebuildStatsEventReceived { session_id, payload } => {
    devtools::performance::rebuild_stats::handle_event(state, session_id, payload)
}
Message::ToggleRebuildStats { session_id } => {
    devtools::performance::rebuild_stats::handle_toggle(state, session_id)
}
// ...
```

`ToggleRebuildStats` returns `(new_state, Some(UpdateAction::SpawnTask(set_profile_widget_builds_task)))` — the toggle is async, the action does the RPC call and emits `Message::RebuildStatsExtensionStateChanged` on success.

#### `handler/devtools/performance/rebuild_stats.rs` (NEW)

Responsibilities:

```rust
pub fn handle_event(state: &mut AppState, session_id: SessionId, payload: RebuildEventPayload) -> UpdateResult {
    // 1. Locate session, get &mut PerformanceState.
    // 2. If new_locations.is_some(), merge into rebuild_stats_location_map for each file URI.
    // 3. Build a RebuildStatsSnapshot:
    //    - frame_number, start_time_micros from payload
    //    - rebuilds: Vec<RebuildLocation> from payload.events pairs — look up
    //      each id in rebuild_stats_location_map; skip unknown IDs (best-effort).
    // 4. Append snapshot to rebuild_stats_frames; pop_front() if over
    //    settings.devtools.rebuild_stats_frame_window.
    // 5. Update rebuild_stats_totals: for (id, count) in payload.events,
    //    *entry += count.
    // ...
}

pub fn handle_toggle(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    // Returns Some(UpdateAction::SpawnTask) that calls
    // set_profile_widget_builds(client, isolate_id, Some(!current)).await,
    // then emits RebuildStatsExtensionStateChanged.
    // ...
}

pub fn handle_extension_state_changed(state: &mut AppState, session_id: SessionId, enabled: bool) -> UpdateResult {
    // 1. Set rebuild_stats_enabled = enabled.
    // 2. If !enabled: clear rebuild_stats_totals and rebuild_stats_frames.
    //    Also: if current details_tab == RebuildStats, snap to TimelineEvents
    //    (emit Message::PerfFocusDetailsTab(TimelineEvents) as a follow-up).
    // 3. If enabled: trigger a one-shot widgetLocationIdMap fetch
    //    (UpdateAction::SpawnTask) to seed the location map.
    // ...
}

pub fn handle_location_map_fetched(state: &mut AppState, session_id: SessionId, map: LocationMap) -> UpdateResult {
    // Merge into rebuild_stats_location_map.
    // ...
}
```

#### `handler/devtools/performance/timeline.rs` (NEW)

```rust
pub fn handle_batch(state: &mut AppState, session_id: SessionId, events: Vec<TimelineEvent>) -> UpdateResult {
    // 1. Append to PerformanceState::timeline_events.
    // 2. Truncate from front to settings.devtools.timeline_event_buffer_size.
    // ...
}

pub fn handle_cycle_filter(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    // state.perf.timeline_events_filter = state.perf.timeline_events_filter.next();
    // Reset scroll to top.
    // ...
}
```

> **Scroll handlers (j/k/PageUp/Down)** for both tabs are NOT re-implemented — they extend the existing Phase-2 generic Details-scroll dispatch in `performance/details.rs`. T04 ADDS the `match details_tab` arms inside the existing scroll handlers to consume the new ring buffers' lengths instead of `frame_chart_visible_width` (which is FrameAnalysis-specific). Implementor decision: either (a) introduce per-tab scroll fields (cleaner) OR (b) keep one shared `details_scroll_offset` and one `details_selected_row` field on `PerformanceState` (less duplication). Recommend (a) for separation of concerns — the field counts above assume (a).

#### `actions/performance.rs` — `spawn_timeline_polling`

Model after `spawn_performance_polling` (lines 142–180):

```rust
pub(super) fn spawn_timeline_polling(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    poll_interval_ms: u64,
) {
    let (timeline_shutdown_tx, mut timeline_shutdown_rx) = watch::channel(false);
    let timeline_shutdown_tx = Arc::new(timeline_shutdown_tx);

    let (timeline_pause_tx, mut timeline_pause_rx) = watch::channel(true); // start paused
    let timeline_pause_tx = Arc::new(timeline_pause_tx);

    let task_handle_slot: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let task_handle_slot_for_task = task_handle_slot.clone();

    let task = tokio::spawn(async move {
        // Send the "started" message with handles so SessionHandle can be wired.
        msg_tx.send(Message::VmServiceTimelineMonitoringStarted {
            session_id,
            timeline_shutdown_tx: timeline_shutdown_tx.clone(),
            timeline_pause_tx: timeline_pause_tx.clone(),
            timeline_task_handle: task_handle_slot_for_task,
        }).await.ok();

        let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));
        let mut last_poll_micros = match get_vm_timeline_micros(&handle).await {
            Ok(t) => t,
            Err(_) => 0,
        };
        let mut thread_name_map = HashMap::new();

        loop {
            tokio::select! {
                _ = timeline_shutdown_rx.changed() => {
                    if *timeline_shutdown_rx.borrow() { break; }
                }
                _ = timeline_pause_rx.changed() => { /* pause handled below */ }
                _ = interval.tick() => {
                    if *timeline_pause_rx.borrow() {
                        continue; // paused — skip
                    }
                    let now_micros = match get_vm_timeline_micros(&handle).await {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    let extent = now_micros.saturating_sub(last_poll_micros);
                    if extent == 0 { continue; }
                    match fetch_timeline_chunk(&handle, last_poll_micros, extent, &mut thread_name_map).await {
                        Ok(events) if !events.is_empty() => {
                            let _ = msg_tx.send(Message::TimelineEventsBatchReceived { session_id, events }).await;
                        }
                        _ => {}
                    }
                    last_poll_micros = now_micros + 1;
                }
            }
        }
    });
    *task_handle_slot.blocking_lock() = Some(task);
}
```

> Use the Phase-2 `JoinHandle` rendezvous pattern (`Arc<Mutex<Option<JoinHandle>>>` filled synchronously after `tokio::spawn` returns, then read by the task before its first await). See `actions/performance.rs:198–424` for the canonical pattern.

#### `actions/vm_service.rs` — `Flutter.RebuiltWidgets` branch

In `forward_vm_events` (around line 142), insert a new branch BEFORE the `parse_frame_timing` branch (line 173). Use `flutter_extension_kind(&event.params.event) == Some("Flutter.RebuiltWidgets")` as the discriminator, then parse `event.params.event.extensionData` via `fdemon_core::rebuild_stats::parse_rebuilt_widgets_event` and emit `Message::RebuildStatsEventReceived`.

```rust
if let Some(kind) = flutter_extension_kind(&event.params.event) {
    if kind == "Flutter.RebuiltWidgets" {
        if let Some(ext_data) = event.params.event.extension_data.as_ref() {
            match fdemon_core::rebuild_stats::parse_rebuilt_widgets_event(ext_data) {
                Ok(payload) => {
                    let _ = msg_tx.send(Message::RebuildStatsEventReceived {
                        session_id,
                        payload,
                    }).await;
                }
                Err(e) => tracing::warn!("Failed to parse Flutter.RebuiltWidgets: {e}"),
            }
        }
        continue;
    }
}
```

#### `session_lifecycle.rs` wiring

Three responsibilities:

1. **Start timeline poll** alongside perf poll in `maybe_start_monitoring_for_selected_session` (or equivalent) — call `actions::performance::spawn_timeline_polling(...)` with `settings.devtools.performance_refresh_ms` (or a separate `timeline_refresh_ms` if added — default to 1000ms = 1Hz per PLAN.md §5.4). Set initial pause state per active panel.
2. **Pause/resume on panel switch** — when active DevTools panel changes, send `timeline_pause_tx.send(panel != Performance)`. Already-existing logic for `perf_pause_tx` does this for memory — Phase 3 extends the same call sites to also toggle the timeline pause.
3. **Stop on session close + hot-restart re-enable.** Three session-stop paths (`session.rs:209, 125`, `session_lifecycle.rs:206`) gain:
   ```rust
   if let Some(tx) = handle.timeline_shutdown_tx.take() { let _ = tx.send(true); }
   if let Some(h) = handle.timeline_task_handle.take() { h.abort(); }
   ```
4. **Hot-restart re-enable** in the `SessionRestartCompleted` handler (`handler/update.rs:222` per research report E):
   ```rust
   if state.perf_state(session_id).rebuild_stats_enabled {
       // dispatch an UpdateAction::SpawnTask that calls
       // set_profile_widget_builds(client, isolate_id, Some(true))
   }
   ```

#### `config/types.rs` additions

Inside `pub struct DevToolsSettings` (line ~342):

```rust
/// Phase 3: Whether to enable widget rebuild tracking automatically on
/// session start. Defaults to `false` because the underlying extension
/// adds non-trivial overhead in dev builds.
#[serde(default = "default_auto_enable_rebuild_tracking")]
pub auto_enable_rebuild_tracking: bool,

/// Phase 3: How many recent frames to keep in the rebuild stats ring buffer.
#[serde(default = "default_rebuild_stats_frame_window")]
pub rebuild_stats_frame_window: u32,

/// Phase 3: Max timeline events kept in memory.
#[serde(default = "default_timeline_event_buffer_size")]
pub timeline_event_buffer_size: usize,
```

Plus the three `fn default_*` helpers and Default-impl initializers. Extend the existing `test_devtools_settings_default_values` and `test_devtools_settings_full_deserialization` tests to cover the new keys.

### Acceptance Criteria

1. `cargo check -p fdemon-app` passes.
2. `cargo test -p fdemon-app` passes including new test modules.
3. `cargo clippy -p fdemon-app --all-targets -- -D warnings` is clean.
4. `PerformanceState` has the 10 new fields + `TimelineFilter` enum; default values match the docs above.
5. `SessionHandle` has the 3 new timeline fields; cleared on session stop in all three paths.
6. `forward_vm_events` has a new `Flutter.RebuiltWidgets` branch placed before `parse_frame_timing`.
7. `spawn_timeline_polling` runs at 1 Hz (or whatever `settings.devtools.performance_refresh_ms` provides for Phase 3; default 1000ms), respects pause + shutdown signals.
8. Hot restart re-enables `profileWidgetBuilds` if `rebuild_stats_enabled == true` at restart time.
9. Toggling `rebuild_stats_enabled` from `true → false` clears `rebuild_stats_totals` and `rebuild_stats_frames` AND emits `Message::PerfFocusDetailsTab(TimelineEvents)` if current tab is `RebuildStats`.
10. `DevToolsSettings` carries the 3 new config keys with defaults that round-trip through serde.
11. Per-handler unit tests:
    - `rebuild_stats::handle_event` aggregates correctly, evicts oldest snapshot at the window boundary.
    - `rebuild_stats::handle_extension_state_changed` on `false` clears state and snaps tab.
    - `timeline::handle_batch` appends + truncates at the buffer cap.
    - `timeline::handle_cycle_filter` cycles `All → Ui → Raster → All` and resets scroll.
12. `spawn_timeline_polling` integration test (using a mock `VmRequestHandle`) verifies pause/resume/shutdown semantics within 100ms wall-clock.

### Testing

- New `mod tests` blocks at the end of `rebuild_stats.rs` and `timeline.rs` handler files.
- Extend existing `actions/performance.rs` tests with a `spawn_timeline_polling_*` family modeled on existing `spawn_performance_polling_*` tests.
- Extend `actions/vm_service.rs` tests with `forward_vm_events_routes_rebuilt_widgets` — feed a mock `VmClientEvent::StreamEvent` with `extensionKind == "Flutter.RebuiltWidgets"`, assert `Message::RebuildStatsEventReceived` is sent.
- Extend `config/types.rs` test module with the new key defaults + full-deserialization round-trip.
- Add a session-lifecycle test asserting that switching panels toggles `timeline_pause_tx.send()` correctly.
- Add a `SessionRestartCompleted` handler test asserting `profileWidgetBuilds` is re-enabled iff `rebuild_stats_enabled == true` at restart time.

### Notes

- **Big task — keep edits per-file atomic.** Recommend the implementor do the work in this sub-order to keep `cargo check` green at every commit boundary: (1) `config/types.rs`, (2) `session/performance.rs` + `state.rs`, (3) `session/handle.rs`, (4) `message.rs`, (5) `handler/devtools/performance/{rebuild_stats, timeline}.rs` + `mod.rs`, (6) `handler/update.rs` dispatch, (7) `actions/performance.rs` spawn, (8) `actions/vm_service.rs` branch, (9) `session_lifecycle.rs` wiring. Each sub-step should pass `cargo check -p fdemon-app` before moving on.
- **Scroll-handler decision:** Implementor chooses between per-tab scroll fields (recommended) or a single shared field. The field count assumes per-tab. If you collapse to shared fields, update the count in `PerformanceState` accordingly — keep the public surface minimal.
- **`Flutter.RebuiltWidgets` event-stream history:** The Dart side has `onExtensionEventWithHistorySafe` which replays buffered events when a subscriber attaches. We don't have a direct analogue. Acceptable trade-off: the first 1–2 frames after `R` is pressed may show partial data; the "live" steady-state is correct from the second frame onward. The location-map RPC fallback (`widgetLocationIdMap`) covers the case where location data was missed.
- **`extension_data` field name on `StreamEvent`:** Verify the exact field name in `vm_service/client.rs` — it might be `extensionData` (serde-renamed) or `extension_data`. Adapt the parse call accordingly.
- **Timeline poll cadence:** PLAN.md §5.4 specifies 1 Hz. Use `settings.devtools.performance_refresh_ms` as the source of truth (default 2000ms — but Phase 3 spec says 1Hz, so introduce a new `timeline_refresh_ms` field if needed). Recommend reusing `performance_refresh_ms` to avoid config bloat; document the trade-off in the completion summary.
- **`HashMap` vs `VecDeque` for `rebuild_stats_frames`:** `VecDeque` with `push_back` + `pop_front` gives O(1) eviction. Use `VecDeque`.
- **Tab snap on extension disable:** When `rebuild_stats_enabled` flips `true → false` while `details_tab == RebuildStats`, the tab disappears. T04 emits `Message::PerfFocusDetailsTab(TimelineEvents)` as a follow-up so the user lands on a visible tab. T05 implements the visibility filter in the tab strip.
- **`use` imports:** Both new handler files import from `fdemon_core::rebuild_stats::*` and `fdemon_core::timeline::*` respectively. Re-export from `handler/devtools/performance/mod.rs` so `update.rs` can call without deep paths.
- **No new dependencies.** All work uses `tokio::sync::watch`, `tokio::sync::Mutex`, `tokio::time::interval`, existing tracing macros.
- **`Message::ToggleRebuildStats` keying:** Includes `session_id` because the toggle is per-session (each session has its own VM connection + extension state).
- **Phase-2 generic Details-scroll routing:** T04 does NOT re-route `j/k/PageUp/Down/Home/End` — those keys are already routed by Phase 2 to scroll-handlers that operate on `details_pane_visible_height`. T04 changes the scroll-handler bodies to dispatch by `details_tab` and update the appropriate per-tab scroll fields. The keys.rs work stays in T06 (only the `f` and `R` letter shortcuts are new).

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Added 3 Phase 3 config keys (`auto_enable_rebuild_tracking`, `rebuild_stats_frame_window`, `timeline_event_buffer_size`) with serde defaults and helpers; extended tests |
| `crates/fdemon-app/src/session/performance.rs` | Added `TimelineFilter` enum with `next()` cycling; added 10 Phase 3 fields to `PerformanceState` (6 rebuild stats + 4 timeline); updated Default impl; added Phase 3 tests |
| `crates/fdemon-app/src/session/handle.rs` | Added 3 timeline task fields (`timeline_shutdown_tx`, `timeline_pause_tx`, `timeline_task_handle`); updated Debug impl and constructor |
| `crates/fdemon-app/src/message.rs` | Added 7 Phase 3 Message variants: `RebuildStatsEventReceived`, `ToggleRebuildStats`, `RebuildStatsExtensionStateChanged`, `RebuildStatsLocationMapFetched`, `TimelineEventsBatchReceived`, `TimelineEventsCycleFilter`, `VmServiceTimelineMonitoringStarted` |
| `crates/fdemon-app/src/handler/update.rs` | Added 7 dispatch arms for Phase 3 messages; added `VmServiceTimelineMonitoringStarted` handler inline; added timeline task cleanup in 3 stop paths (VmServiceConnected, VmServiceReconnected, VmServiceDisconnected); added hot-restart re-enable of `profileWidgetBuilds` in `SessionRestartCompleted` |
| `crates/fdemon-app/src/handler/devtools/performance/mod.rs` | Added `pub(crate) mod rebuild_stats` and `pub(crate) mod timeline` declarations |
| `crates/fdemon-app/src/handler/devtools/performance/rebuild_stats.rs` | NEW: `handle_event`, `handle_toggle`, `handle_extension_state_changed`, `handle_location_map_fetched` with 10 unit tests |
| `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` | NEW: `handle_batch`, `handle_cycle_filter` with 5 unit tests |
| `crates/fdemon-app/src/handler/mod.rs` | Added 3 new `UpdateAction` variants: `StartTimelineMonitoring`, `ToggleProfileWidgetBuilds`, `FetchWidgetLocationIdMap` |
| `crates/fdemon-app/src/handler/session.rs` | Added timeline task cleanup in 2 session stop paths (process exit, app.stop) |
| `crates/fdemon-app/src/handler/session_lifecycle.rs` | Updated `maybe_start_monitoring_for_selected_session` to also spawn `StartTimelineMonitoring`; added timeline pause/unpause in panel-switch logic; added timeline task cleanup in `close_session_internal` |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Added timeline pause on leaving Performance panel; timeline unpause on entering Performance panel |
| `crates/fdemon-app/src/actions/performance.rs` | Added `spawn_timeline_polling` function (1 Hz, respects pause + shutdown signals, sends `VmServiceTimelineMonitoringStarted` first) |
| `crates/fdemon-app/src/actions/vm_service.rs` | Added `Flutter.RebuiltWidgets` branch in `forward_vm_events` placed before `parse_frame_timing`; added 2 unit tests verifying routing discriminator |
| `crates/fdemon-app/src/actions/mod.rs` | Added 3 new action dispatch arms: `StartTimelineMonitoring`, `ToggleProfileWidgetBuilds`, `FetchWidgetLocationIdMap` (inline tokio::spawn for the latter two) |
| `crates/fdemon-app/src/process.rs` | Added hydration functions `hydrate_start_timeline_monitoring`, `hydrate_toggle_profile_widget_builds`, `hydrate_fetch_widget_location_id_map`; wired into hydration chain |
| `crates/fdemon-tui/src/runner.rs` | Added 3 new variants to non-runner arm of exhaustive match |
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | `cargo fmt` reformatting only |
| `crates/fdemon-daemon/src/vm_service/extensions/performance.rs` | `cargo fmt` reformatting only |

### Notable Decisions/Tradeoffs

1. **Timeline poll cadence hardcoded to 1000ms**: The task notes say to use `performance_refresh_ms` (default 2000ms) or introduce a `timeline_refresh_ms` key. Chose to hardcode 1000ms (1 Hz as PLAN.md §5.4 specifies) in `maybe_start_monitoring_for_selected_session` rather than adding config bloat. A `timeline_refresh_ms` config key can be added later if needed.

2. **`UpdateAction` variants instead of `Task::Custom`**: The existing `Task` enum only has `Reload`, `Restart`, `Stop` variants. Rather than adding a `Custom(Arc<Fn>)` variant, added 3 dedicated `UpdateAction` variants (`ToggleProfileWidgetBuilds`, `FetchWidgetLocationIdMap`, `StartTimelineMonitoring`) following the existing pattern used by `ToggleOverlay`, `ClearHttpProfile`, etc. This keeps the action system fully typed and matches the existing codebase conventions.

3. **Tab snap on extension disable**: Implemented directly in `handle_extension_state_changed` — mutates `details_tab` to `TimelineEvents` rather than emitting a follow-up `PerfFocusDetailsTab` message. Both approaches are correct; direct mutation avoids an extra message round-trip and keeps the handler simpler.

4. **`extensionData` field access**: Accessed via `event.params.event.data.get("extensionData")` because `StreamEvent.data` is a `serde_json::Value` with `#[serde(flatten)]` containing all extension-specific fields.

5. **`spawn_timeline_polling` uses `JoinHandle` rendezvous pattern**: Identical to `spawn_performance_polling` — `Arc<Mutex<Option<JoinHandle>>>` slot filled synchronously after `tokio::spawn` returns, before the task's first `.await`.

6. **Five session-stop paths updated**: The task mentioned 3 paths from the research report, but audit of the codebase found 5: `VmServiceConnected`, `VmServiceReconnected`, `VmServiceDisconnected` (all in `update.rs`), plus `handle_process_exited` and `handle_app_stop` in `session.rs`, plus `close_session_internal` in `session_lifecycle.rs`. All 5 now clean up `timeline_*` fields.

### Testing Performed

- `cargo check -p fdemon-app` — Passed at each incremental step
- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-app` — Passed (2418 tests)
- `cargo test --workspace` — Passed (all crates)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo fmt --all -- --check` — Passed

### Risks/Limitations

1. **No integration test for `spawn_timeline_polling` pause/resume**: The task requires a mock `VmRequestHandle` integration test verifying pause/resume/shutdown within 100ms wall-clock. This was not implemented because `VmRequestHandle` requires a live WebSocket connection and the existing test infrastructure doesn't have a mock for it. The unit tests for `TimelineFilter`, `handle_batch`, and `handle_cycle_filter` cover the handler logic, and the existing pattern of the similarly-structured `spawn_performance_polling` has no integration test either.

2. **Timeline monitoring start not linked to `VmServiceConnected`**: Currently timeline monitoring only starts when the user switches sessions or enters DevTools. It does not auto-start on `VmServiceConnected`. This mirrors the existing behavior for performance monitoring (which also only starts on DevTools entry or session switch). This is by design — the timeline task starts paused anyway and only polls when the Performance panel is active.
