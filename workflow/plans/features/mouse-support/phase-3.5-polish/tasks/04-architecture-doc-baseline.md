# Task 04: ARCHITECTURE.md baseline update for Phase 3 region registry

**Status:** Done
**Estimated Hours:** 0.5h
**Depends On:** —
**Crate / Area:** docs
**Agent:** doc_maintainer

## Goal

Update `docs/ARCHITECTURE.md` to document the new mouse-support infrastructure that landed in Phase 3 of `feat/mouse-support`. This is the first half of review item 18; the second half (`MouseRegionGuard` from Wave 4) lands as Task 11.

After this task, `ARCHITECTURE.md` should describe:

1. The `mouse_regions` module in `fdemon-app` — `MouseRect`, `MouseAction`, `MouseRegionEntry`, `MouseRegions`, `MouseRegionsBuilder`, `MouseRegionsCell`. Domain types only — no `ratatui` dependency.
2. The `AppState::mouse_regions: MouseRegionsCell` field with its TEA-exception annotation pattern.
3. The `MouseCtx` threading pattern in `fdemon-tui::render::view` — how `Cell::take` / clear / build / `Cell::set` are sequenced, and how widgets receive `Option<&mut MouseCtx<'_>>` for region recording.
4. The hit-test path in `handler/mouse/normal.rs::handle_press` — how the registry is consulted, the busy gate, and the put-back pair.

## Files Modified (Write)

- `docs/ARCHITECTURE.md`

## Files Read

- `crates/fdemon-app/src/mouse_regions.rs` — read for accurate type signatures and module description
- `crates/fdemon-app/src/state.rs` — confirm `mouse_regions` field placement and exception comment
- `crates/fdemon-app/src/handler/mouse/normal.rs` — read for hit-test description
- `crates/fdemon-tui/src/render/mod.rs` — read for `MouseCtx` and the take/clear/build/set sequence
- `crates/fdemon-tui/src/widgets/mod.rs` — read for `MouseCtx` re-export
- `docs/ARCHITECTURE.md` — read existing structure to find the right insertion points (Module Reference, Key Patterns, Key Types)

## Implementation Steps

1. **Read the existing `ARCHITECTURE.md` structure.** Identify the "Module Reference" section for `fdemon-app` and `fdemon-tui`, the "Key Patterns" list, and the "Key Types" enumeration if present.

2. **Add a `mouse_regions` entry under `fdemon-app`'s Module Reference.** Brief description (1–3 sentences):
   - Per-frame click-region registry with a z-index-aware hit-test.
   - Exposes `MouseRect` (a `ratatui`-free coordinate type), `MouseAction` (`Emit(Box<Message>)` and `EmitWithCoord(fn(u16, u16) -> Message)`), `MouseRegions`, `MouseRegionsBuilder`, and `MouseRegionsCell` (a thin newtype wrapping `Cell<MouseRegions>` to satisfy `#[derive(Debug)]` on `AppState`).
   - Cite that `fdemon-app` does NOT depend on `ratatui` — `MouseRect` is a local type, with conversion handled at the `fdemon-tui` boundary.

3. **Add a "Mouse Region Registry" sub-section under "Key Patterns".** Describe the per-frame lifecycle (≤8 bullet points):
   - `render::view()` calls `state.mouse_regions.take()` at frame start, leaving `Default::default()` in the cell.
   - `regions.clear()` resets the entry list while preserving `Vec` capacity.
   - `MouseCtx::new(regions.builder())` constructs the per-frame thread-through; widgets receive `Option<&mut MouseCtx<'_>>` and call `ctx.click(...)`, `ctx.click_at_z(...)`, or `ctx.click_left_middle(...)`.
   - At frame end, `state.mouse_regions.set(regions)` puts the populated registry back.
   - `handler/mouse/normal.rs::handle_press` performs the same `take`/hit-test/`set` pattern when a click arrives, so the registry is restored before the synchronous TEA loop yields.
   - The `tag_filter_visible` and per-`UiMode` gates live at the dispatcher (`handler/mouse/mod.rs::handle_press`) — they decide whether to consult the registry at all, which means a click in Settings mode is silently dropped before reaching the hit-test even though the registry is populated.
   - The busy gate (`HotReload`/`HotRestart`/`StopApp` blocked when `any_session_busy()`) lives at the per-mode handler — gating at click time rather than registration time, mirroring `handler/keys.rs`.
   - **Note:** Wave 4 (Task 09 / 11) introduces `MouseRegionGuard<'_>` which replaces the manual `take`/`set` pairs with an RAII type for panic-safety. This sub-section will be amended in Task 11.

4. **Update the "Key Types" / data-flow diagram if present.** If `ARCHITECTURE.md` enumerates the major types per crate (`AppState`, `Message`, `LogEntry`, etc.), add `MouseRegions` and `MouseAction`. If a TEA data-flow diagram shows the message bus, note that `Cell<MouseRegions>` is an approved render-hint exception (cross-link to `docs/REVIEW_FOCUS.md`).

5. **Cross-reference `docs/REVIEW_FOCUS.md`.** Where `ARCHITECTURE.md` mentions render-hint exceptions, add a parenthetical: *"see `docs/REVIEW_FOCUS.md` 'Approved TEA Exception → Current usage' for the canonical list."*

## Acceptance Criteria

- [ ] `docs/ARCHITECTURE.md` describes the `mouse_regions` module under `fdemon-app`
- [ ] `docs/ARCHITECTURE.md` describes the `MouseCtx` threading pattern under `fdemon-tui` or "Key Patterns"
- [ ] `docs/ARCHITECTURE.md` notes that `fdemon-app` does not depend on `ratatui` (the `MouseRect` boundary)
- [ ] `docs/ARCHITECTURE.md` cross-references `docs/REVIEW_FOCUS.md` for the TEA-exception canonical list
- [ ] `docs/ARCHITECTURE.md` notes that the take/set pattern is the *current* mechanism and will be wrapped by `MouseRegionGuard` in Wave 4 (forward-looking pointer for the reader)
- [ ] No source code changes
- [ ] Existing `ARCHITECTURE.md` structure is preserved (don't rewrite sections that don't relate to mouse support)
- [ ] Document is internally consistent — no contradictory descriptions of the same component

## Notes

- This task is routed to `doc_maintainer` per the planner-skill rules: `docs/ARCHITECTURE.md` is a managed doc.
- Stay descriptive, not prescriptive. Document what the code currently does, not what it should do.
- Do **not** describe `MouseRegionGuard` in this task — that lands in Task 11 after Wave 4 implements it. A single forward pointer ("Wave 4 will introduce `MouseRegionGuard`") is fine but no API description.
- Keep the new content tight — match the surrounding section's voice and density. Three to six paragraphs total is appropriate for the registry pattern; one short module-reference bullet for `mouse_regions`.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Added `mouse_regions.rs`, `handler/mouse/`, and `input_mouse.rs` rows to the `fdemon-app` Module Reference table; updated `render/mod.rs` row in `fdemon-tui` to mention `MouseCtx`; added "Mouse Region Registry" subsection to Key Patterns covering types, per-frame lifecycle, per-click lifecycle, gate checks, TEA exception note, and Wave 4 forward pointer; added `mouse_regions: MouseRegionsCell` to AppState description in Key Types |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES/NO/N/A: N/A

### Notable Decisions/Tradeoffs

1. **Handler/mouse rows added to Module Reference**: `handler/mouse/` and `input_mouse.rs` were not previously listed in the fdemon-app table even though they are significant modules. Added them alongside `mouse_regions.rs` since the task context made clear they are part of the same feature boundary.
2. **Forward pointer phrasing**: Described `MouseRegionGuard` only as a named future mechanism with no API details, per the task's explicit instruction. The sentence references "Wave 4 (phase-3.5 Task 09 / Task 11)" so future readers can locate the follow-up task.
3. **REVIEW_FOCUS.md cross-reference**: Cross-referenced via the TEA exception note (`docs/REVIEW_FOCUS.md` "Approved TEA Exception → Current usage") as required by acceptance criteria, without linking to prohibited docs directly in managed doc content.
