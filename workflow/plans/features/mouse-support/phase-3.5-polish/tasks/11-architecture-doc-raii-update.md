# Task 11: ARCHITECTURE.md update for `MouseRegionGuard`

**Status:** Not Started
**Estimated Hours:** 0.25h
**Depends On:** 9
**Crate / Area:** docs
**Agent:** doc_maintainer

## Goal

Augment `docs/ARCHITECTURE.md` to describe the new `MouseRegionGuard<'a>` RAII type that Task 09 introduced. This is the second half of review item 18; the first half (Phase-3 baseline) landed in Task 04.

After this task, `ARCHITECTURE.md` should:

1. Describe `MouseRegionGuard<'a>` as the canonical accessor for the registry, replacing the manual `Cell::take` / `Cell::set` pair documented in Task 04.
2. Update the "Mouse Region Registry" sub-section under "Key Patterns" to reflect that `take_guard()` is the recommended path; mention `take()`/`set()` only as low-level primitives.
3. Note the panic-safety guarantee — a widget panic between guard construction and `Drop` no longer leaves the registry empty.

## Files Modified (Write)

- `docs/ARCHITECTURE.md`

## Files Read

- `crates/fdemon-app/src/mouse_regions.rs` — read post-Task-09 state for the `MouseRegionGuard` type signature, its `Deref{Mut}` / `Drop` impls, and the `take_guard()` accessor
- `crates/fdemon-tui/src/render/mod.rs` — read post-Task-09 state for the actual guard usage in `view()`
- `crates/fdemon-app/src/handler/mouse/normal.rs` — read post-Task-09 state for the guard usage in `handle_press`
- `docs/ARCHITECTURE.md` — read the existing "Mouse Region Registry" sub-section that Task 04 added, to find the right amendment points

## Implementation Steps

1. **Locate the "Mouse Region Registry" sub-section** that Task 04 added under "Key Patterns" in `docs/ARCHITECTURE.md`.

2. **Update the per-frame lifecycle bullets** to use `take_guard()` as the canonical pattern. Where Task 04 wrote:
   > - `render::view()` calls `state.mouse_regions.take()` at frame start, leaving `Default::default()` in the cell.
   > - …
   > - At frame end, `state.mouse_regions.set(regions)` puts the populated registry back.

   Replace with:
   > - `render::view()` calls `state.mouse_regions.take_guard()` at frame start. The guard takes ownership of the inner `MouseRegions` (leaving `Default::default()` in the cell for the duration of the frame) and exposes it via `Deref` / `DerefMut`.
   > - `regions.clear()` resets the entry list while preserving `Vec` capacity.
   > - `MouseCtx::new(regions.builder())` constructs the per-frame thread-through; widgets receive `Option<&mut MouseCtx<'_>>` and call `ctx.click(...)`, `ctx.click_at_z(...)`, or `ctx.click_left_middle(...)`.
   > - When the guard goes out of scope at the end of `view()`, its `Drop` impl puts the populated registry back into the cell — no explicit `set()` call is required.
   > - The same pattern is used in `handler/mouse/normal.rs::handle_press`: a `take_guard()` borrowing the cell wraps the hit-test, ensuring the registry is restored even if the hit-test path panics.

3. **Add or update a "Panic safety" note.** Where the section discusses the `Cell` exception, add:
   > **Panic safety:** Prior to `MouseRegionGuard`, a widget panic between `Cell::take()` and `Cell::set()` would silently leave the registry permanently empty (replaced with `Default::default()`), disabling mouse interaction for the rest of the session with no diagnostic. The guard's `Drop` impl restores the registry on stack unwind, eliminating this failure mode. The lower-level `MouseRegionsCell::{take, set}` methods remain available for tests but should not appear in production code.

4. **Forward-pointer cleanup.** In Task 04's text, there was a sentence saying *"Wave 4 will introduce `MouseRegionGuard`"*. Remove it (it's no longer forward-looking) and replace with a back-reference if the doc structure benefits from one.

5. **Cross-reference the new type from `Key Types` (if present).** Add `MouseRegionGuard<'a>` to any enumerated list of mouse-support types. Mark it as the canonical accessor; mark `MouseRegionsCell::{take, set}` as low-level primitives.

## Acceptance Criteria

- [ ] `docs/ARCHITECTURE.md` describes `MouseRegionGuard<'a>` with: (a) construction via `MouseRegionsCell::take_guard()`, (b) `Deref`/`DerefMut` access, (c) `Drop`-based put-back, (d) panic-safety guarantee
- [ ] The "Mouse Region Registry" sub-section in "Key Patterns" no longer says "Wave 4 will introduce" — it describes `MouseRegionGuard` as the current canonical pattern
- [ ] `MouseRegionsCell::{take, set}` are described as low-level primitives, with a note that production code should use `take_guard` instead
- [ ] No source code changes
- [ ] Existing `ARCHITECTURE.md` structure is preserved (don't rewrite unrelated sections)

## Notes

- This task is routed to `doc_maintainer` per the planner-skill rules: `docs/ARCHITECTURE.md` is a managed doc.
- Task 04 already established the section structure; this task is an amendment, not a rewrite. Keep edits surgical.
- The "Panic safety" note is the most important addition because it documents a non-obvious property that future maintainers will need to know when adding take/set call sites.
- Do not deprecate `MouseRegionsCell::{take, set}` — they remain part of the test-friendly API and may have legitimate non-render use cases. Just steer production callers toward `take_guard`.
