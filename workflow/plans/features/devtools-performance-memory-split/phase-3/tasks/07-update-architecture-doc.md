## Task: Update Documentation for Phase 3 (Rebuild Stats + Timeline Events)

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` "Performance Panel Interactivity" section to reflect Phase 3's new state, RPCs, messages, sub-handlers, polling task, and conditional tab visibility. Phase 2's section already documents `PerfDetailsTab::{FrameAnalysis, RebuildStats, TimelineEvents}` and the dual-pane layout; Phase 3 fills in what the previously-stubbed tabs now do and what plumbing they introduce.

**Depends on**: T04 (state shape + spawn + handler structure), T05 (UI rendering + visibility), T06 (key bindings + footer contract).

**Estimated Time**: 2–4 hours

### Scope

**Files Modified (Write):**

- `docs/ARCHITECTURE.md` — targeted edits within the "Performance Panel Interactivity" section (or wherever Phase 2's documentation landed; cross-reference the Phase 2 doc-update task at `phase-2/tasks/07-update-architecture-doc.md` and the phase-2-followup `03-fix-architecture-doc-errors.md` to find the right anchor).

**Files Read (Dependencies):**

- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- T04 completion summary — final `PerformanceState` field names + `Message` variant names + spawn function name.
- T05 completion summary — final tab visibility behavior + render-hint usage.
- T06 completion summary — final letter shortcuts.
- `crates/fdemon-app/src/session/performance.rs` — verify field names match doc claims.
- `crates/fdemon-app/src/message.rs` — verify variant names match doc claims.
- `crates/fdemon-app/src/handler/devtools/performance/{rebuild_stats, timeline}.rs` — verify handler function names.
- `crates/fdemon-daemon/src/vm_service/extensions/{performance, inspector}.rs` — verify RPC function names.
- `crates/fdemon-app/src/actions/performance.rs` — verify `spawn_timeline_polling` signature.

### Change Context

Phase 3 introduces:

1. **New `PerformanceState` fields** (10 new fields, 1 new enum):
   - Rebuild stats: `rebuild_stats_enabled`, `rebuild_stats_location_map`, `rebuild_stats_totals`, `rebuild_stats_frames`, `rebuild_stats_scroll_offset`, `rebuild_stats_selected_row`.
   - Timeline events: `timeline_events`, `timeline_events_scroll_offset`, `timeline_thread_name_map`, `timeline_events_filter`.
   - Enum: `TimelineFilter { All, Ui, Raster }`.
2. **New `SessionHandle` fields:** `timeline_shutdown_tx`, `timeline_pause_tx`, `timeline_task_handle` (mirror of perf_*).
3. **New `Message` variants** (7): `RebuildStatsEventReceived`, `ToggleRebuildStats`, `RebuildStatsExtensionStateChanged`, `RebuildStatsLocationMapFetched`, `TimelineEventsBatchReceived`, `TimelineEventsCycleFilter`, `VmServiceTimelineMonitoringStarted`.
4. **New sub-handler modules** under `crates/fdemon-app/src/handler/devtools/performance/`: `rebuild_stats.rs`, `timeline.rs`.
5. **New polling task:** `actions::performance::spawn_timeline_polling` at 1 Hz; gated on Performance panel being active via `timeline_pause_tx`.
6. **New event dispatch branch** in `actions/vm_service.rs::forward_vm_events` for `Flutter.RebuiltWidgets`.
7. **Hot-restart re-enable** of `ext.flutter.profileWidgetBuilds` on `SessionRestartCompleted` when previously enabled.
8. **New VM Service surfaces:**
   - `extensions::ext::PROFILE_WIDGET_BUILDS` constant.
   - `extensions::ext::WIDGET_LOCATION_ID_MAP` constant.
   - `extensions::performance::{set_profile_widget_builds, get_profile_widget_builds}` (NEW module).
   - `extensions::inspector::widget_location_id_map` (NEW function).
   - `timeline::{get_vm_timeline_micros, fetch_timeline_chunk}` (NEW functions in existing module).
9. **Conditional tab visibility:** `RebuildStats` tab is dynamically shown/hidden in `widgets/devtools/performance/details/mod.rs` based on `state.rebuild_stats_enabled`.
10. **New config keys** (in `[devtools]` block of `.fdemon/config.toml`): `auto_enable_rebuild_tracking`, `rebuild_stats_frame_window`, `timeline_event_buffer_size`.
11. **New letter shortcuts** (architecturally relevant to mention): `f` (Timeline filter cycle), `R` (Rebuild tracking toggle, shadows global `R`).
12. **Core type additions** (already documented in core-type sections): `fdemon_core::rebuild_stats::{Location, LocationMap, RebuildLocation, RebuildStatsSnapshot, RebuildEventPayload, parse_rebuilt_widgets_event}`, `fdemon_core::timeline::{TimelineThread, TimelinePhase, TimelineEvent, parse_vm_timeline}`.

### Acceptance Criteria

1. The "Performance Panel Interactivity" section (or equivalent) describes the dual-pane layout's three details tabs with their **current Phase-3 functional state** — no remaining "stubs / coming soon" language from Phase 2.
2. The `PerformanceState` field list reflects the 10 new fields and the `TimelineFilter` enum.
3. The `Message` flow diagram (if present) lists the 7 new variants and their dispatch targets.
4. The handler module tree shows the two new sub-modules under `handler/devtools/performance/`.
5. The "VM Service" or equivalent section lists the new RPC wrappers and the new event-stream branch.
6. The "Session Lifecycle" or equivalent section documents the timeline polling task's start/stop/pause and the hot-restart re-enable of `profileWidgetBuilds`.
7. Conditional `RebuildStats` tab visibility is explicitly called out (it's an architectural decision worth surfacing, not just a render detail).
8. The new config keys are listed in whatever "Configuration" or `DevToolsSettings` reference section already exists.
9. Cross-references between sections remain valid (no broken anchors).
10. No content boundary violations — only ARCHITECTURE.md is edited; no CODE_STANDARDS.md or DEVELOPMENT.md edits in this task (those would each get their own doc task if needed).
11. The doc validates against `~/.claude/skills/doc-standards/schemas.md`.
12. All field names, function names, message variant names, and config key names match the actual code as of T04–T06 completion (no drift).

### Notes

- **Follow content boundaries strictly** — see `~/.claude/skills/doc-standards/schemas.md`. Architectural facts only; no implementation tutorials, no rationales for design decisions (those live in PLAN.md / completion summaries).
- **Make targeted edits, do not rewrite entire documents.** The Phase 2 section structure exists — extend it, don't replace it.
- **Verify against source after T04+T05+T06 land.** Field and variant names may have minor adjustments during implementation — this task is the final reconciliation between docs and code.
- **`docs/KEYBINDINGS.md` is owned by T06**, not this task — keep ARCHITECTURE.md focused on system shape, not key tables.
- **`docs/CONFIGURATION.md`** — if it exists and lists `[devtools]` keys, T07 ALSO updates it to add the 3 new keys (it's unmanaged, not under doc_maintainer strict control, but updating it here keeps the docs together). Check first; if absent, skip.
- **Phase 2 introduced a `display_refresh_rate` field** documented as "Phase 3 may extend to parse `Display.Refresh` events" — Phase 3 did NOT do this (deferred per PLAN.md §7.4). Either remove that forward-pointer or rephrase to "still deferred — see PLAN.md §7.4".
- **`details_pane_visible_height` render-hint** — Phase 2 added the field; Phase 3 first consumer. Note this in the render-hint section if it documents Cell-based hints inventory (per phase-2-followup `04-consolidated-minor-cleanup.md` m1).
- **No new `// EXCEPTION:` annotations to document beyond what Phase 2 already covers** — Phase 3 reuses the existing render-hint pattern; no new TEA exceptions are introduced.
