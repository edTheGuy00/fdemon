# Task 06: `mouse_regions.rs` Doc Polish

## Goal

Update documentation in `crates/fdemon-app/src/mouse_regions.rs` to reflect Phase 4 reality: registry sizing now scales with viewport, `hit_test` is O(N), `MouseAction::Emit(Box<Message>)` allocates per push, and the `expect()` calls in `MouseRegionGuard::deref/deref_mut` need SAFETY notes. Also document the same-z-last-pushed-wins contract on `MouseRegionsBuilder::click` and add a usage note on `MouseAction::as_emit()`.

## Background

Six minor review findings concentrate in this single file:

- **Minor #10:** `MouseAction::Emit(Box<Message>)` allocates per region push (~4,000 small allocations/sec at peak in log view). Module docstring claims "registry hot path is allocation-free at steady state" — no longer accurate.
- **Minor #13 (doc portion):** Glyph-after-row push order relies on last-pushed-wins-at-same-z. The contract is enforced only by registration order; document it explicitly on `MouseRegionsBuilder::click`.
- **Minor #19:** `MouseRegionGuard::deref` and `deref_mut` use `expect("MouseRegionGuard is live until Drop")`. The invariant is structurally sound but lacks a SAFETY: comment explaining why `regions` is always `Some` from construction until `drop()`.
- **Minor #20:** `MouseRegions::with_capacity()` docstring says "starts at 32, grows to ~32 entries (header + 9 tabs + 9 device rows + 6 settings rows)". Phase 4 widens the working set to viewport-bounded.
- **Minor #21:** `hit_test` is O(N) and there is no docstring noting that.
- **Minor #22:** `MouseAction::as_emit()` was added out-of-scope by Phase 4 task 07. Add a doc comment explaining why it's public (cross-crate test usage).

## Files

**Modify:**
- `crates/fdemon-app/src/mouse_regions.rs`

## Plan

1. **Update the module-level docstring.** Adjust the "registry hot path is allocation-free at steady state" claim:
   ```rust
   //! ...
   //! ## Allocation behavior
   //!
   //! The registry's `Vec` is reused across frames via `clear()`, which preserves
   //! capacity. Steady-state allocation is dominated by `MouseAction::Emit(Box<Message>)`
   //! pushes — each click region registered allocates one `Box<Message>` per frame.
   //! At peak (e.g., ~200 visible log rows × 20 fps) this is roughly 4,000 small
   //! allocations/sec; not a hot-path bottleneck on glibc/jemalloc but no longer
   //! "allocation-free."
   ```

2. **Update `MouseRegions::with_capacity()` docstring.** Replace the stale "32 entries (header + 9 tabs + ...)" with Phase 4 reality:
   ```rust
   /// Pre-allocate room for `cap` regions. The default `MouseRegions::default()`
   /// starts at 32, but Phase 4 click-region surfaces (log view, inspector tree,
   /// performance frame chart, network table) can push one region per visible
   /// row, so the working set typically grows to viewport-bounded sizes
   /// (~visible-row-count) on first render and stays there via Vec capacity reuse.
   pub fn with_capacity(cap: usize) -> Self { /* ... */ }
   ```

3. **Add an O(N) note to `hit_test`**:
   ```rust
   /// Find the topmost region containing `(x, y)` for the given button.
   ///
   /// Iterates every registered entry; runtime is O(N) in registry size.
   /// Phase-4-era N is bounded by viewport row count (~24–60 typical).
   /// If Phase 5+ pushes this past ~1k entries, consider a y-sorted index.
   pub fn hit_test(&self, x: u16, y: u16, button: MouseButton) -> Option<&MouseRegionEntry> { /* ... */ }
   ```

4. **Add SAFETY comments to `MouseRegionGuard::deref` and `deref_mut`**:
   ```rust
   impl Deref for MouseRegionGuard<'_> {
       type Target = MouseRegions;
       fn deref(&self) -> &Self::Target {
           // SAFETY: `regions` is initialized to `Some` in `MouseRegionGuard::new`
           // and is only consumed by `drop()`. Any access between construction and
           // drop must observe `Some`.
           self.regions.as_ref().expect("MouseRegionGuard is live until Drop")
       }
   }

   impl DerefMut for MouseRegionGuard<'_> {
       fn deref_mut(&mut self) -> &mut Self::Target {
           // SAFETY: same invariant as Deref.
           self.regions.as_mut().expect("MouseRegionGuard is live until Drop")
       }
   }
   ```

5. **Add a same-z last-pushed-wins note to `MouseRegionsBuilder::click`**:
   ```rust
   /// Register a click region at the given rect with `MouseAction`.
   ///
   /// **Push-order contract:** when two regions share the same `z_index` and
   /// overlap on a cell, hit-testing returns the *last-pushed* region for that
   /// cell. Inspector tree rows rely on this: row regions are pushed first (wide),
   /// then glyph regions (narrow, 1×1) — so a click on the glyph cell resolves
   /// to the glyph (`ToggleNode`) rather than the row (`SelectRow`). A future
   /// refactor that reorders pushes will silently break this contract.
   pub fn click(&mut self, rect: MouseRect, action: MouseAction, z_index: u8) -> &mut Self { /* ... */ }
   ```

6. **Add a usage note to `MouseAction::as_emit()`**:
   ```rust
   /// Returns the inner `Message` if this action is `Emit(_)`, otherwise `None`.
   ///
   /// Used by cross-crate tests (e.g., `fdemon-tui::widgets::devtools::inspector::tests`)
   /// to introspect emitted messages without resolving coordinates. Kept public for
   /// that reason; not intended for production use (the registry's `hit_test` +
   /// `resolve` flow is the primary action consumer).
   pub fn as_emit(&self) -> Option<&Message> { /* ... */ }
   ```

## Acceptance Criteria

- [ ] Module-level allocation note replaces the stale "allocation-free" claim.
- [ ] `with_capacity` docstring reflects Phase 4 viewport-bounded sizing.
- [ ] `hit_test` docstring notes O(N) runtime.
- [ ] `MouseRegionGuard::deref` and `deref_mut` carry SAFETY comments.
- [ ] `MouseRegionsBuilder::click` carries the push-order contract note.
- [ ] `MouseAction::as_emit` carries the cross-crate-test usage note.
- [ ] No behavioral changes; only doc-comment updates.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.

## Notes

- This is a docs-only task. No tests need to be added or modified.
- The push-order contract on `click()` is also already partially documented in `tree_panel.rs` (the consuming code). The `mouse_regions.rs` doc is the authoritative side; consumer docs may reference it.
- **Do not touch** any other file. Do not add an `#[cfg(test)]` cargo feature for `as_emit()` — the recommendation from planning was to keep the method public.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/mouse_regions.rs` | Added module-level `## Allocation behavior` section; updated `MouseRegions` struct doc; updated `with_capacity()` docstring for Phase 4 viewport-bounded sizing; added O(N) runtime note to `hit_test`; expanded SAFETY comments in `MouseRegionGuard::deref` and `deref_mut`; added push-order contract note to `MouseRegionsBuilder::click`; rewrote `MouseAction::as_emit()` docstring with cross-crate-test usage note |

### Notable Decisions/Tradeoffs

1. **Module-level "Allocation behavior" section**: Added as a new `##` heading after the `## Lifecycle` section rather than inline, which makes it easier to find and reference. The stale "allocation-free at steady state" claim was in the `MouseRegions` struct docstring (not the module docstring) — updated both to be consistent.
2. **SAFETY comments wording**: The existing comments were already correct but terse. Expanded them to explicitly name the invariant (set in `new`, consumed only in `drop`, no intermediate `take` call) rather than just asserting "is Some".
3. **`with_capacity()` note**: The existing doc said capacity 32 covers "header + 9 tabs + 9 device rows + 6 settings rows" — replaced with Phase 4 reality noting viewport-bounded growth.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app` - Passed (2068 unit tests, 1 doc-test)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Docs-only**: No behavioral changes; risk is effectively zero. The SAFETY comments use `// SAFETY:` which is the idiomatic Rust convention but these are safe code paths (the `Option::expect` is just for correctness assertion, not unsafe code).
