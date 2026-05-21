# 12 — Update ARCHITECTURE.md + REVIEW_FOCUS.md

**Wave:** 5
**Depends On:** 01, 04, 05, 08, 09
**Agent:** doc_maintainer
**Estimated Hours:** 2–3h
**Addresses:** Documentation updates for new Phase 3-followup patterns

## Context

This phase introduces several architectural patterns that need to be documented in the project's strict-managed docs:

- **New Message variant.** `RebuildStatsToggleFailed` and its session-log surfacing path (T04).
- **Forwarder optimization.** Panel-gate in `forward_vm_events` (T05) — first time the forwarder consults app-state before dispatching.
- **Action-layer architecture cleanup.** Inspector helper now owns location-map parsing end-to-end; action task is a thin transport wrapper (T04 M6).
- **Auto-enable wiring.** `auto_enable_rebuild_tracking` triggers on `VmServiceConnected` (T09).
- **Tab visibility in state machine.** `PerfDetailsTab::next_visible(rebuild_stats_enabled)` — new method for cycle-skipping (T08).
- **Trait abstraction over VmRequestHandle.** `VmRequestApi` enables polling-task integration tests (T11). May warrant a brief mention in the architecture doc as a testing pattern, even though it's internal-only.

`docs/ARCHITECTURE.md` is a strict-managed doc — only `doc_maintainer` may edit it. `docs/REVIEW_FOCUS.md` is unmanaged but its content needs updates that align with the architecture changes.

## Acceptance Criteria

1. **`docs/ARCHITECTURE.md` — Performance Panel Interactivity / DevTools Subsystem section updates:**
   - Document the `RebuildStatsToggleFailed` message: what triggers it, what the handler does (appends a session-log entry), and the rollback message that fires alongside it (`RebuildStatsExtensionStateChanged` with the actual current state).
   - Document the forwarder panel-gate: `forward_vm_events` early-returns on `Flutter.RebuiltWidgets` when `active_devtools_panel != Performance`. Note the rationale (sustained event volume at 60fps × non-Performance navigation).
   - Document the `widget_location_id_map` flow cleanup: the action task now calls the inspector helper directly and forwards the typed `LocationMap`, matching the `FetchAllocationProfile` pattern.
   - Document the auto-enable wiring: `auto_enable_rebuild_tracking` is consulted in the `VmServiceConnected` handler. Note that `SessionRestartCompleted` re-enable (Phase 3 behavior) takes precedence — the two paths are mutually exclusive in practice.
   - Document the `next_visible` tab cycle method on `PerfDetailsTab`: state machine skips hidden tabs.
   - Brief note on `VmRequestApi` as the testing-only trait abstraction over `VmRequestHandle`; cross-reference where the integration tests live.
2. **`docs/REVIEW_FOCUS.md` updates:**
   - Add an entry for the new `text_helpers` module (T10) under the `pub(super)` visibility section if it tracks per-module boundaries.
   - Document the forwarder panel-gate as an approved early-return optimization (so future reviews don't flag it as suspicious).
   - Document the `try_send` + drop-on-full pattern in the forwarder (T05) as the canonical backpressure strategy for high-frequency VM Service events.
   - If T08 ends up exposing a new TEA exception (e.g., by adding state-reading code that's not strictly in the handler), document the exception. (T08 likely won't introduce new exceptions; verify by reading T08's diff.)
3. **Content boundary compliance.** All edits respect the strict boundaries of `docs/ARCHITECTURE.md`:
   - No implementation tutorials.
   - No design rationale beyond a one-line "why" per pattern.
   - No code snippets longer than 3–4 lines.
   - All field/function names match actual code (verified by reading the post-Wave-3-merge state).
4. **Cross-references.** No broken anchors. The new sections reference the existing Phase 3 architecture sections where appropriate.
5. **`docs/CONFIGURATION.md`** continues to be unmanaged (T02 handles its updates).

## Files Modified (Write)

- `docs/ARCHITECTURE.md` — Performance Panel Interactivity / DevTools Subsystem additions.
- `docs/REVIEW_FOCUS.md` — pattern entries.

## Files Read (Dependencies)

- T01–T11 completion summaries — confirm what was actually implemented vs the plan.
- `crates/fdemon-app/src/{message.rs, actions/{mod,vm_service}.rs, handler/devtools/performance/rebuild_stats.rs, handler/update.rs, handler/keys.rs, state.rs}` — verify names and shapes match documented descriptions.
- `crates/fdemon-tui/src/widgets/devtools/performance/details/text_helpers.rs` (if T10 landed) — confirm the module exists and its boundary.
- `crates/fdemon-daemon/src/vm_service/client.rs` — confirm `VmRequestApi` trait shape (T11).
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.

## Approach Hints

- The doc-maintainer's role is to **describe what is, not what should be**. Read the merged state of all impl tasks and document the as-built architecture.
- For each new pattern, write 2–4 paragraphs maximum. Architecture docs should describe the system; design rationale belongs in PR descriptions and review docs.
- The Phase 3 ARCHITECTURE.md updates (Task 07 of Phase 3) introduced sections that this task extends — append to those rather than replacing them.
- For REVIEW_FOCUS.md: this doc is consulted by future reviewers as a checklist of "things that look weird but are intentional." The forwarder panel-gate and `try_send` patterns are good candidates because they may otherwise be flagged.
- If any T01–T11 task ended up not implementing what was planned (per its completion summary), document the actual implementation. Note the deviation if material.

## Out of Scope

- Editing `docs/DEVELOPMENT.md` — no build/test workflow changes from this phase.
- Editing `docs/CODE_STANDARDS.md` — no new coding conventions introduced.
- Editing `docs/CONFIGURATION.md` — T02 handles its own updates.
- Editing `docs/KEYBINDINGS.md` — no new keybindings (M4 just restores fallthrough, preserving documented `R` semantics).
- Updating other docs (TESTING.md, EXTENSION_API.md, etc.).
- Schema validation runs for `doc-standards` (the orchestrator runs validation after the merge).
