# Task 09: `MouseRegionGuard` RAII wrapper for take/put-back panic-safety

**Status:** Not Started
**Estimated Hours:** 1.5h
**Depends On:** 1, 8
**Crate / Area:** `fdemon-app`, `fdemon-tui`

## Goal

Discharge review item 11: the manual `Cell::take` + `Cell::set` pairs in `render::view` (lines 108 and 336 of `render/mod.rs`) and `normal::handle_press` (lines 44 and 54 of `normal.rs`) are not panic-safe. If a widget panics between `take` and `set`, the registry is permanently empty (replaced with `Default::default()`) for the remainder of the session — mouse interaction silently disabled with no diagnostic.

Phase 4 will add many more take/set sites (log row click registration, frame bar, network row, dialog/overlay regions) — locking down panic-safety now is cheaper than retrofitting later.

Introduce a `MouseRegionGuard<'a>` RAII type that:
- Takes ownership of `MouseRegions` from a `&'a MouseRegionsCell` on construction.
- Exposes the inner `MouseRegions` via `Deref` and `DerefMut`.
- Puts the value back into the cell on `Drop`, even if a panic unwinds the stack.

The guard is the user-confirmed approach (chat decision: borrowing guard with `Deref{Mut}` over a closure-based `with_regions(|builder| ...)` API). Closure APIs nest call sites by one level of indentation and fight Rust's `?`-via-closure ergonomics; the borrowing guard preserves call-site syntax.

## Files Modified (Write)

- `crates/fdemon-app/src/mouse_regions.rs` — define `MouseRegionGuard<'a>` and add a `MouseRegionsCell::take_guard(&self) -> MouseRegionGuard<'_>` accessor
- `crates/fdemon-tui/src/render/mod.rs` — replace the `state.mouse_regions.take()` / `state.mouse_regions.set(regions)` pair with a `let mut guard = state.mouse_regions.take_guard();` binding scoped to the render body
- `crates/fdemon-app/src/handler/mouse/normal.rs` — replace the analogous take/set pair in `handle_press` with the same guard pattern

## Files Read

- `crates/fdemon-app/src/state.rs` — confirm `MouseRegionsCell` API surface (it exposes `take(&self)` and `set(&self, MouseRegions)`)

## Implementation Steps

### Part A — Define `MouseRegionGuard<'a>` in `mouse_regions.rs`

1. **Add the guard type** after the existing `MouseRegionsCell` definition:
   ```rust
   /// RAII guard that holds a `MouseRegions` taken from a `MouseRegionsCell`.
   ///
   /// On construction, calls `Cell::take()` (leaving `Default::default()` in the cell).
   /// On `Drop`, calls `Cell::set(regions)` to put the value back, even if a panic
   /// unwinds the stack. This guarantees that a widget panic between take and put-back
   /// cannot silently disable the mouse-region registry.
   ///
   /// Constructed via `MouseRegionsCell::take_guard()`. Use `&mut *guard` (or
   /// `guard.builder()`, etc.) to access the inner `MouseRegions`.
   pub struct MouseRegionGuard<'a> {
       cell: &'a MouseRegionsCell,
       // Always `Some` between construction and Drop. The `Option` is needed so
       // `Drop` can move the value back via `Cell::set` without unsafe.
       regions: Option<MouseRegions>,
   }

   impl<'a> MouseRegionGuard<'a> {
       fn new(cell: &'a MouseRegionsCell) -> Self {
           Self {
               cell,
               regions: Some(cell.take()),
           }
       }
   }

   impl Deref for MouseRegionGuard<'_> {
       type Target = MouseRegions;
       fn deref(&self) -> &MouseRegions {
           self.regions.as_ref().expect("guard is live until Drop")
       }
   }

   impl DerefMut for MouseRegionGuard<'_> {
       fn deref_mut(&mut self) -> &mut MouseRegions {
           self.regions.as_mut().expect("guard is live until Drop")
       }
   }

   impl Drop for MouseRegionGuard<'_> {
       fn drop(&mut self) {
           if let Some(regions) = self.regions.take() {
               self.cell.set(regions);
           }
       }
   }
   ```

2. **Add a `take_guard` accessor to `MouseRegionsCell`:**
   ```rust
   impl MouseRegionsCell {
       // ... existing methods ...

       /// Take the inner `MouseRegions` and return a panic-safe RAII guard that puts
       /// it back on `Drop`. Prefer this over the raw `take()` / `set()` pair.
       pub fn take_guard(&self) -> MouseRegionGuard<'_> {
           MouseRegionGuard::new(self)
       }
   }
   ```

3. **Re-export `MouseRegionGuard`** from `crates/fdemon-app/src/lib.rs` alongside the other Phase-3 mouse types:
   ```rust
   pub use mouse_regions::{
       MouseAction, MouseRect, MouseRegionEntry, MouseRegionGuard, MouseRegions,
       MouseRegionsBuilder, MouseRegionsCell,
   };
   ```

4. **Add unit tests in `mouse_regions.rs`:**
   - `guard_puts_regions_back_on_drop`: take a guard, mutate via `DerefMut`, drop it, assert the cell now holds the mutated value.
   - `guard_puts_regions_back_on_panic`: use `std::panic::catch_unwind` with a closure that constructs a guard and panics; after `catch_unwind` returns, assert the cell holds the (possibly partially-built) registry, NOT `Default::default()`.
   - `guard_deref_exposes_builder`: confirm `guard.builder()` works through `DerefMut`.

### Part B — Adopt the guard in `render::view`

5. **In `crates/fdemon-tui/src/render/mod.rs::view`,** locate the existing pattern around lines 108 and 336:
   ```rust
   // Take the registry at frame start
   let mut regions = state.mouse_regions.take();
   regions.clear();

   // ... render body that constructs MouseCtx::new(regions.builder()) ...

   // Put the populated registry back
   state.mouse_regions.set(regions);
   ```

   Replace with:
   ```rust
   // RAII guard puts the registry back on Drop, even if rendering panics.
   let mut regions = state.mouse_regions.take_guard();
   regions.clear();

   // ... render body — `regions.builder()` works via DerefMut ...

   // No explicit set() needed — guard's Drop runs at end of scope.
   ```

   The render body's `MouseCtx::new(regions.builder())` continues to work because `regions.builder()` is now a method-call-via-`DerefMut` against the inner `MouseRegions`.

6. **Verify the `view` function's borrow scoping still compiles.** If the existing inner-block scaffolding from Phase 3 Task 04 (the `{ let _mouse_ctx = ...; }` block) is no longer needed because the guard's Drop is the put-back, simplify or remove that scaffolding.

### Part C — Adopt the guard in `normal::handle_press`

7. **In `crates/fdemon-app/src/handler/mouse/normal.rs::handle_press`,** locate the take/set pair around lines 44 and 54:
   ```rust
   let regions = state.mouse_regions.take();
   let entry = regions.hit_test(x, y, button);
   let action_opt = entry.and_then(|e| match button {
       MouseButton::Left => e.on_left.clone(),
       MouseButton::Middle => e.on_middle.clone(),
       _ => None,
   });
   state.mouse_regions.set(regions);
   ```

   Replace with:
   ```rust
   // Guard puts the registry back on Drop, including on early-return paths below.
   let regions = state.mouse_regions.take_guard();
   let entry = regions.hit_test(x, y, button);
   let action_opt = entry.and_then(|e| match button {
       MouseButton::Left => e.on_left.clone(),
       MouseButton::Middle => e.on_middle.clone(),
       _ => None,
   });
   drop(regions); // explicit drop before subsequent state inspection (busy gate, etc.)
   ```

   The explicit `drop(regions)` is a small ergonomic note — it makes the put-back point obvious and matches the existing comment style in `render::view`. If the busy-gate code below the hit-test does not need the registry, dropping early is fine; otherwise let the guard live to end-of-scope.

## Acceptance Criteria

- [ ] `MouseRegionGuard<'a>` exists in `mouse_regions.rs` with `Deref`, `DerefMut`, and `Drop` impls
- [ ] `MouseRegionsCell::take_guard(&self) -> MouseRegionGuard<'_>` is the canonical accessor; existing `take`/`set` methods may remain for tests but should not be used in production code
- [ ] `crates/fdemon-app/src/lib.rs` re-exports `MouseRegionGuard`
- [ ] `crates/fdemon-tui/src/render/mod.rs::view` uses `take_guard()` instead of the manual take/set pair
- [ ] `crates/fdemon-app/src/handler/mouse/normal.rs::handle_press` uses `take_guard()` instead of the manual take/set pair
- [ ] New tests confirm panic-safety: `std::panic::catch_unwind` with a panicking closure that holds a guard does NOT leave the cell empty
- [ ] All existing 5,131 tests continue to pass
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] No new clippy lints fire (e.g., `clippy::let_underscore_must_use` if `drop(regions)` is used)

## Notes

- **Why depends on Task 01 and Task 08:**
  - Task 01 polishes `mouse_regions.rs` (TODO removal, Debug doc, EmitWithCoord doc) — Task 09 then builds on the cleaned-up file. Doing them in the other order means Task 01's edits land on top of the new guard code, increasing the merge surface.
  - Task 08 simplifies `normal::handle_press` by lifting the `tag_filter_visible` check to the dispatcher. Task 09 then replaces the (now-shorter) take/set pair. In the other order, Task 08 has to refactor a function that just got rewritten.
- **The `Option<MouseRegions>` inside the guard** is needed so `Drop::drop` (which takes `&mut self`) can move the value out for `Cell::set`. The alternative is `unsafe` code or a `MaybeUninit` dance — both worse.
- **`drop(regions)` in `normal::handle_press`** is optional but recommended. The orchestrator may inline the guard's lifetime to function scope without it; the explicit `drop` documents intent.
- **Do not introduce a panic-safety test that uses `std::process::abort` or similar** — `catch_unwind` is the right tool. Make sure the test compiles with `panic = unwind` (the project default) and gracefully skips with `panic = abort` if the project ever switches.
- **Keep the existing `MouseRegionsCell::{take, set}` public methods** for now — Task 11 / future phases can deprecate them once all production call sites have migrated to `take_guard`.
