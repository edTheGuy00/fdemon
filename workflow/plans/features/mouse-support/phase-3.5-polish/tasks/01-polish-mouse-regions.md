# Task 01: Polish `mouse_regions.rs`

**Status:** Not Started
**Estimated Hours:** 0.5h
**Depends On:** —
**Crate / Area:** `fdemon-app`

## Goal

Discharge four small Phase-3 review findings that all touch `crates/fdemon-app/src/mouse_regions.rs`:

1. **Stale TODO** (review item 1): `click_left_middle_binds_both_buttons` test asserts `Message::CloseCurrentSession` with a `// TODO: switch to Message::CloseSessionAt(0) when Task 02 lands.` comment. Task 02 has shipped — fix the assertion and remove the TODO so the test reflects what production code emits.
2. **`*msg.clone()` in `MouseAction::resolve`** (review item 8): `*msg.clone()` clones the `Box<Message>` then dereferences. The canonical form is `(**msg).clone()` — clone the inner value, not the Box.
3. **`MouseRegionsCell::Debug` doc/impl mismatch** (review item 13): The doc comment claims "Debug output shows only the entry count" but the impl uses `finish_non_exhaustive()` with no `field("len", ...)` call. Fix the comment to match the impl (the impl is fine — `&self` Debug can't `take`/`set`, so the count is genuinely not exposed).
4. **`EmitWithCoord` closure invariant doc** (review item 16): Add a `///` note that closures must use `saturating_sub`/`checked_sub` for offset arithmetic on `(x, y)`, and that capturing closures (which would require widening to `Box<dyn Fn(...)>`) should be added as a *new* enum variant rather than widening this one.

## Files Modified (Write)

- `crates/fdemon-app/src/mouse_regions.rs`

## Files Read

- `crates/fdemon-app/src/message.rs` — confirm `Message::CloseSessionAt(usize)` exists and is the right signature for the test fix

## Implementation Steps

1. **Fix the stale-TODO test assertion.** Locate the `click_left_middle_binds_both_buttons` test (around line 320–340 of `mouse_regions.rs`). Replace the middle-click `MouseAction::emit(Message::CloseCurrentSession)` with `MouseAction::emit(Message::CloseSessionAt(0))` and delete the `// TODO: switch to Message::CloseSessionAt(0) when Task 02 lands.` comment immediately above it. Update any nearby assertion that names the expected message.

2. **Replace `*msg.clone()` with `(**msg).clone()` in `MouseAction::resolve`.** Around line 87. The observable behavior is identical (`Message: Clone`); the new form is one-step clearer about what is being cloned.

3. **Reconcile the `MouseRegionsCell::Debug` doc comment with the impl.** Update the doc above the `impl Debug for MouseRegionsCell` block to read approximately:
   > Shows only the type name (the inner `Cell<MouseRegions>` cannot be inspected through `&self` without a `take`/`set` round-trip; debug-printing during a frame would corrupt the registry).

4. **Add an invariant note to `MouseAction::EmitWithCoord`.** Above the variant declaration, add a `///` block explaining:
   - Closures must use `saturating_sub` or `checked_sub` for any offset arithmetic on `(x, y)` — bare subtraction can underflow `u16` and panic.
   - The variant deliberately uses `fn(u16, u16) -> Message` (a function pointer) rather than `Box<dyn Fn>` to avoid heap allocation per region. If a future use-case needs to capture state, add a *new* variant (e.g., `EmitWithCoordCaptured(Box<dyn Fn(u16, u16) -> Message>)`) — do not widen `EmitWithCoord`.

## Acceptance Criteria

- [ ] `Message::CloseCurrentSession` does not appear anywhere in `mouse_regions.rs` (production or test)
- [ ] No `// TODO:` comments remain in `mouse_regions.rs` referencing already-shipped tasks
- [ ] `MouseAction::resolve` body uses `(**msg).clone()` for the `Emit` arm
- [ ] `MouseRegionsCell`'s `Debug` doc comment accurately describes the `finish_non_exhaustive` behavior
- [ ] `MouseAction::EmitWithCoord` carries a `///` doc block covering: (a) the saturating-arithmetic requirement on closures, (b) the rationale for `fn` over `Box<dyn Fn>`, (c) the rule that capturing variants are added as new enum cases
- [ ] `cargo test -p fdemon-app mouse_regions` passes (existing 11 tests + any unchanged)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes

## Notes

- This is a four-edit polish task in a single file. All changes are mechanical; no public API changes.
- Do not change the `MouseRegionsCell::Debug` impl itself — only the doc comment. The impl is correct as-is (a `&self` Debug genuinely cannot expose the count without taking the registry, and taking during render would be a bug).
- The `Message::CloseSessionAt(0)` substitution in the test is correct because the original test only checks that *both* the left and middle bindings were captured by `click_left_middle`; the specific message variant is incidental but should match production usage in `widgets/tabs.rs::render_session_tabs`.
