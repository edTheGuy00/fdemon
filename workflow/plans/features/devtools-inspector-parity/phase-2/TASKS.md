# Phase 2 — Render Object & Flex Explorer Tabs — Task Index

## Overview

Phase 2 of the DevTools Inspector parity feature. Populates the two stub tabs in the Inspector Details view:

- **Render Object tab** — calls a new `ext.flutter.inspector.getProperties` VM Service extension to fetch widget properties + (recursively) render-object properties, then renders them as a key/value table.
- **Flex Explorer tab** — extends `extract_layout_info` to parse per-child flex data (`size`, `constraints`, `parentData`, `flexFactor`, `flexFit`) from the existing `getLayoutExplorerNode` response and renders an ASCII flex diagram with axis arrows and per-child boxes.

Phase 1 already scaffolded the InspectorState fields (`properties`, `render_properties`, `properties_loading`, `properties_error`) and the empty tab modules. Phase 2 fills the implementation. **Tab visibility remains unconditional in this phase** — per Phase 1's cross-cutting constraint #6, every widget shows all three tabs. Phase 3 introduces conditional visibility.

Cf. parent plan at `workflow/plans/features/devtools-inspector-parity/PLAN.md` §5.3 and §6 Phase 2.

**Total Tasks:** 10
**Estimated Hours:** 26–38 hours

## Task Dependency Graph

```
                Wave 1 — Foundation (parallel)
   ┌──────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
   │ 01-core-flex-and-    │ │ 02-daemon-           │ │ 03-app-properties-   │
   │  property-types      │ │  properties-rpc-     │ │  action-and-state    │
   │ (fdemon-core/        │ │  primitives          │ │ (handler/mod.rs,     │
   │  widget_tree.rs)     │ │ (extensions/mod.rs + │ │  message.rs,         │
   │                      │ │  extensions/         │ │  process.rs,         │
   │                      │ │  properties.rs NEW)  │ │  state.rs)           │
   └─────────┬────────────┘ └──────────┬───────────┘ └──────────┬───────────┘
             │                         │                         │
             ▼                         ▼                         ▼
                Wave 2 — Wiring (parallel)
   ┌──────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
   │ 04-daemon-flex-      │ │ 05-app-spawn-        │ │ 06-app-handlers-     │
   │  extraction          │ │  properties-task     │ │  and-open-details    │
   │ (extensions/         │ │ (actions/mod.rs +    │ │ (handler/update.rs + │
   │  layout.rs)          │ │  actions/inspector/  │ │  handler/devtools/   │
   │ depends: 01          │ │   mod.rs)            │ │   inspector.rs)      │
   │                      │ │ depends: 02, 03      │ │ depends: 03          │
   └──────────┬───────────┘ └──────────┬───────────┘ └──────────┬───────────┘
              │                        │                        │
              ▼                        ▼                        ▼
                Wave 3 — TUI rendering (parallel)
   ┌──────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
   │ 09-tui-flex-         │ │ 08-tui-properties-   │ │ 07-tui-render-       │
   │  explorer-tab        │ │  tab-population      │ │  object-tab          │
   │ (details/            │ │ (details/            │ │ (details/            │
   │  flex_explorer_      │ │  properties_tab.rs)  │ │  render_object_      │
   │  tab.rs)             │ │ depends: 06          │ │  tab.rs)             │
   │ depends: 04          │ │                      │ │ depends: 06          │
   └──────────┬───────────┘ └──────────┬───────────┘ └──────────┬───────────┘
              └────────────────────────┴────────────────────────┘
                                       ▼
                          Wave 4 — Docs
                  ┌─────────────────────────────┐
                  │ 10-docs-update              │
                  │ (Agent: doc_maintainer)     │
                  │ ARCHITECTURE.md             │
                  │ depends: 01–09              │
                  └─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-core-flex-and-property-types](tasks/01-core-flex-and-property-types.md) | Not Started | — | 3–4h | `crates/fdemon-core/src/widget_tree.rs` |
| 02 | [02-daemon-properties-rpc-primitives](tasks/02-daemon-properties-rpc-primitives.md) | Not Started | — | 2–3h | `crates/fdemon-daemon/src/vm_service/extensions/mod.rs`, `crates/fdemon-daemon/src/vm_service/extensions/properties.rs` **NEW** |
| 03 | [03-app-properties-action-and-state](tasks/03-app-properties-action-and-state.md) | Not Started | — | 3–4h | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/process.rs`, `crates/fdemon-app/src/state.rs` |
| 04 | [04-daemon-flex-extraction](tasks/04-daemon-flex-extraction.md) | Not Started | 01 | 3–5h | `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` |
| 05 | [05-app-spawn-properties-task](tasks/05-app-spawn-properties-task.md) | Not Started | 02, 03 | 3–4h | `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/actions/inspector/mod.rs` |
| 06 | [06-app-handlers-and-open-details](tasks/06-app-handlers-and-open-details.md) | Not Started | 03 | 3–4h | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/devtools/inspector.rs` |
| 07 | [07-tui-render-object-tab](tasks/07-tui-render-object-tab.md) | Not Started | 06 | 3–4h | `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` |
| 08 | [08-tui-properties-tab-population](tasks/08-tui-properties-tab-population.md) | Not Started | 06 | 2–3h | `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` |
| 09 | [09-tui-flex-explorer-tab](tasks/09-tui-flex-explorer-tab.md) | Not Started | 04 | 3–5h | `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` |
| 10 | [10-docs-update](tasks/10-docs-update.md) | Not Started | 01–09 | 1–2h | `docs/ARCHITECTURE.md` |

## Wave Schedule

| Wave | Tasks | Notes |
|------|-------|-------|
| W1 | 01, 02, 03 | All file-disjoint. Foundation: new types in `fdemon-core`, daemon RPC primitives, app action+message+state scaffolding. |
| W2 | 04, 05, 06 | 04 depends on 01 (FlexChild types). 05 depends on 02+03 (daemon RPC + UpdateAction). 06 depends on 03 (Message variants + new state fields). All three write disjoint files. |
| W3 | 07, 08, 09 | All three TUI tabs are file-disjoint. 07 + 08 consume `inspector.properties` / `inspector.render_properties` populated by 06. 09 consumes `LayoutInfo.children` populated by 04. |
| W4 | 10 | Documentation update after all implementation lands. |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|--------------------------|
| 01-core-flex-and-property-types | `crates/fdemon-core/src/widget_tree.rs` | `tmp/devtools/packages/devtools_app/lib/src/screens/inspector/inspector_data_models.dart` (reference for flex enum values) |
| 02-daemon-properties-rpc-primitives | `crates/fdemon-daemon/src/vm_service/extensions/mod.rs`, `crates/fdemon-daemon/src/vm_service/extensions/properties.rs` **NEW** | `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` (pattern reference), `crates/fdemon-core/src/widget_tree.rs` (DiagnosticsNode + `is_render_object_property()`) |
| 03-app-properties-action-and-state | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/process.rs`, `crates/fdemon-app/src/state.rs` | `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` (existing `FetchLayoutData` shape — pattern reference) |
| 04-daemon-flex-extraction | `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` | `crates/fdemon-core/src/widget_tree.rs` (FlexChild + enums from 01), `tmp/devtools/.../inspector_data_models.dart` |
| 05-app-spawn-properties-task | `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/actions/inspector/mod.rs` | `crates/fdemon-app/src/handler/mod.rs` (UpdateAction from 03), `crates/fdemon-app/src/message.rs` (Messages from 03), `crates/fdemon-daemon/src/vm_service/extensions/properties.rs` (from 02) |
| 06-app-handlers-and-open-details | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/devtools/inspector.rs` | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/handler/mod.rs` |
| 07-tui-render-object-tab | `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` | `crates/fdemon-app/src/state.rs` (InspectorState fields), `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` (palette + tab plumbing), `crates/fdemon-core/src/widget_tree.rs` (`DiagnosticsNode.level`, `is_render_object_property`) |
| 08-tui-properties-tab-population | `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` | `crates/fdemon-app/src/state.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs` (existing helpers) |
| 09-tui-flex-explorer-tab | `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` | `crates/fdemon-core/src/widget_tree.rs` (FlexChild + enums + LayoutInfo), `crates/fdemon-app/src/state.rs` |
| 10-docs-update | `docs/ARCHITECTURE.md` | All implementation task files; `~/.claude/skills/doc-standards/schemas.md`; current ARCHITECTURE.md DevTools Subsystem section |

### Overlap Matrix

(Pairs evaluated only between tasks in the same wave — i.e. tasks with no dependency between them.)

| Wave | Task Pair | Shared Write Files | Isolation Strategy |
|------|-----------|--------------------|--------------------|
| W1 | 01 + 02 | None | Parallel (worktree) |
| W1 | 01 + 03 | None | Parallel (worktree) |
| W1 | 02 + 03 | None | Parallel (worktree) |
| W2 | 04 + 05 | None | Parallel (worktree) |
| W2 | 04 + 06 | None | Parallel (worktree) |
| W2 | 05 + 06 | None | Parallel (worktree) |
| W3 | 07 + 08 | None | Parallel (worktree) |
| W3 | 07 + 09 | None | Parallel (worktree) |
| W3 | 08 + 09 | None | Parallel (worktree) |

No write-file collisions detected within any wave. All wave-peer tasks can be dispatched concurrently in isolated worktrees.

## Cross-Cutting Constraints

1. **Reuse the existing `INSPECTOR_OBJECT_GROUP = "fdemon-inspector-1"` for `getProperties` calls.** Per parent PLAN §7.2, the object group is shared across all inspector RPCs for the lifetime of the inspector view; disposal happens at session end. The `getProperties` spawn task (task 05) must NOT create a new object group; reuse the constant defined in `crates/fdemon-app/src/actions/inspector/mod.rs:31`.

2. **Property fetch caching keyed by `details_node_id`.** Task 03 adds two new fields to `InspectorState`:
   - `last_fetched_properties_node_id: Option<String>` — cache key; equals `details_node_id` on success.
   - `pending_properties_node_id: Option<String>` — in-flight tracker for stale-guard against rapid Enter→Esc→Enter cycles.
   Both fields are cleared by `reset()` and `reset_details_and_groups()` (`crates/fdemon-app/src/state.rs:434–443` and `:458–467`). Task 06's `handle_open_details` skips re-dispatch when `last_fetched_properties_node_id == Some(details_node_id) && properties_error.is_none()`. Mirrors the existing `last_fetched_node_id` / `pending_node_id` pattern used for layout (`state.rs:259–283`).

3. **Stale-response guard in `handle_properties_fetched`.** Task 06's handler must compare `pending_properties_node_id` against the response's matching id (or `details_node_id` at receive time) and discard the response if the user closed Details or switched nodes mid-flight. Mirrors `handle_layout_data_fetched` at `crates/fdemon-app/src/handler/devtools/inspector.rs:301–339`.

4. **`getProperties` recursion lives in the spawn task, not the daemon.** Task 02's daemon helpers expose:
   - `parse_diagnostics_array(raw_json) -> Result<Vec<DiagnosticsNode>, …>` — deserializes a getProperties response array.
   - `split_widget_and_render(props) -> (Vec<DiagnosticsNode>, Vec<DiagnosticsNode>)` — partitions by `is_render_object_property()`.
   The recursive second call (for each render-object node, fetch its sub-properties via another `getProperties`) lives in task 05's `spawn_fetch_inspector_properties` background task — that's where the `VmRequestHandle`, timeout, and message back-channel are owned. This mirrors the existing layout pattern (`fetch_layout_data` lives in the spawn task, not in the daemon's `WidgetInspector` struct).

5. **`handle_open_details` dispatches both `FetchLayoutData` AND `FetchInspectorProperties`.** Phase 1's `handle_open_details` (`inspector.rs:544–557`) already conditionally dispatches `FetchLayoutData`. Phase 2 must extend it to also dispatch `FetchInspectorProperties` (subject to its own caching predicate from constraint #2). If `UpdateResult` does not support multiple actions directly, the implementor of task 06 must either:
   - (a) Use the existing batched-action pattern (grep `crates/fdemon-app/src/handler/` for sites that return multiple actions, or look at how `Message::RequestLayoutData` chains via a dedicated message); OR
   - (b) Introduce a new chain message (`Message::RequestInspectorProperties`) that `handle_open_details` returns, which then routes to its own `FetchInspectorProperties` action.
   Task 06's completion summary must document which approach was chosen and why.

6. **Sanitize `DiagnosticsNode.property_type`.** Phase 1.5 wired `strip_ansi_codes()` into most `DiagnosticsNode` and `LayoutInfo` string fields via `deserialize_sanitized_string` and `deserialize_sanitized_option_string` (per `crates/fdemon-core/src/widget_tree.rs:1007–1025`). The `property_type` field at `widget_tree.rs:97–102` currently uses plain `#[serde(default, rename = "propertyType")]` with no sanitization. Task 01 adds `deserialize_with = "deserialize_sanitized_option_string"`.

7. **Tab visibility remains unconditional in Phase 2.** Cycling `Tab` / `Shift+Tab` continues to show all three tabs regardless of widget type. Conditional visibility (Container → 1 tab; non-flex render widget → 2 tabs; Row/Column/Flex → 3 tabs) is Phase 3 work.

8. **Render Object tab field ordering follows DevTools.** DevTools sorts properties with `level == "fine"` to the end (default badge), filters out `level == "hidden"`, and otherwise preserves the order returned by the RPC. Task 07 must implement this sort. Reference: `tmp/devtools/packages/devtools_app/lib/src/screens/inspector/widget_properties/properties_view.dart:313–333` (`_filterAndSortPropertiesByLevel`).

9. **ASCII Flex Explorer simplification.** Per parent PLAN §7.1, the TUI flex visualizer does NOT attempt proportional rectangles or animations. Each child renders as a fixed-height box labeled with its actual dimensions; the visual hierarchy is communicated through labels, not pixel-area. Task 09's renderer must follow this constraint — equal-size stacked boxes, never proportional.

10. **No new key bindings.** Phase 2 introduces no new keys. `Tab` / `Shift+Tab` already cycle tabs (Phase 1). `Esc` already closes details (Phase 1). The Phase 2 work is entirely about populating the existing details-tab scaffold.

## Success Criteria

Phase 2 is complete when:

- [ ] Selecting a widget and pressing `Enter` issues an `ext.flutter.inspector.getProperties` RPC (visible in `--vm-service-debug` logs if enabled) and populates `inspector.properties` + `inspector.render_properties`.
- [ ] For widgets with a `RenderObject` sub-property (e.g. `Column`, `Padding`), a second `getProperties` RPC is issued to fetch render-object sub-properties; these append to `render_properties`.
- [ ] The Render Object tab renders a key/value table from `render_properties` showing fields such as `needsCompositing`, `creator`, `parentData`, `constraints`, `layer`, `semantics node`, `size` (whichever the Flutter framework emits for the selected widget's render object).
- [ ] Properties with `level == "fine"` render with a muted "default" style and sort to the end of the list. Properties with `level == "hidden"` do not render.
- [ ] The Properties tab now shows a populated property list below the layout box-model preview, replacing the Phase 1 placeholder `"(properties will load here in Phase 2)"`.
- [ ] Selecting a `Column`/`Row`/`Flex` widget and opening the Flex Explorer tab renders an ASCII flex diagram with: axis arrows (main/cross), per-child boxes labeled with `w×h` + `flex:N` + `fit:tight/loose`, alignment labels (`mainAxisAlignment`/`crossAxisAlignment`), constraints footer, and a "Total Flex: N" hint.
- [ ] Flex children are correctly parsed from the existing `getLayoutExplorerNode` response (no new RPC) — the daemon-side `extract_layout_info` populates `LayoutInfo.children` and the new `direction`/`main_axis_alignment`/`cross_axis_alignment`/`main_axis_size` fields.
- [ ] Re-opening Details on the same widget within the same session does not re-issue `getProperties` (cache hit on `last_fetched_properties_node_id`).
- [ ] Refreshing the tree (`r`) or hot-restarting the session clears the properties cache + the details state. `expanded_groups`, `properties`, `render_properties`, `last_fetched_properties_node_id`, `pending_properties_node_id`, `properties_loading`, `properties_error`, `details_open`, `details_node_id`, `details_tab` all reset (this is the existing `reset_details_and_groups()` plus the two new cache fields from constraint #2).
- [ ] Stale responses (user closes Details or switches nodes mid-flight) are discarded by the `pending_properties_node_id` guard in `handle_properties_fetched` and do not mutate `properties`/`render_properties`.
- [ ] Fetch failures populate `properties_error` and render as a user-friendly error in the Render Object tab and the Properties property list area; fetch timeouts (>10s) populate the same error path with a "Press [r] to retry" hint.
- [ ] `DiagnosticsNode.property_type` passes through `strip_ansi_codes()` at deserialize time.
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass on the implementor's branch.
- [ ] No regression in existing Phase 1 / Phase 1.5 inspector tests (tree rendering, chain folding, mouse-region invariants, details navigation, settings persistence).
- [ ] `docs/ARCHITECTURE.md` DevTools Subsystem section is updated to list the new `getProperties` extension call, the new `FlexChild`/`FlexFit`/`Axis`/`MainAxisAlignment`/`CrossAxisAlignment`/`MainAxisSize` types in `fdemon-core`, and the two new `last_fetched_properties_node_id` / `pending_properties_node_id` cache fields on `InspectorState`.

## Keyboard Shortcuts (changed by Phase 2)

None. Phase 2 is rendering + data-fetch work; the key model from Phase 1 is unchanged.

## Notes

- Phase 2 ships as a single PR — all 10 tasks merged together. Cf. Phase 1's "single PR per phase" cadence.
- The two stub tabs from Phase 1 (`render_object_tab.rs`, `flex_explorer_tab.rs`) are fully rewritten by tasks 07 and 09. The triple-duplicated `render_centered_text` helper (Phase 1.5 deferred item n5) is naturally retired by these rewrites — task 07 + 09 do not need a placeholder centered-text helper because the new content always fills the tab area.
- The `WidgetInspector` daemon struct (`crates/fdemon-daemon/src/vm_service/extensions/inspector.rs:308`) is NOT used by the Phase 2 spawn task. Following Phase 1's `spawn_fetch_layout_data` pattern, the action task calls `handle.call_extension(ext::GET_PROPERTIES, args)` directly via the `VmRequestHandle`. The `properties.rs` module added by task 02 contains free functions, not methods on a struct.
- Reference DevTools source files (read-only) under `tmp/devtools/packages/devtools_app/lib/src/`. Key references are linked from individual task files' "Details" sections.
- The `last_fetched_node_id` field on `InspectorState` (for layout caching) is independent of the new `last_fetched_properties_node_id` (for properties caching). They cache different RPCs and may diverge — e.g. layout cached for node A while properties just loading for node B is a transient state we accept.
