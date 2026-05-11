# Action Items: Mouse Support — Phase 3 (Region Registry)

**Review Date:** 2026-05-04
**Verdict:** ⚠️ APPROVED WITH CONCERNS
**Blocking Issues:** 0 critical / 4 major
**Branch:** `feat/mouse-support`

---

## Major Issues (Should Fix Before Phase 4)

### 1. Stale TODO referencing already-shipped Task 02
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/mouse_regions.rs:324`
- **Problem:** Test `click_left_middle_binds_both_buttons` middle-binding asserts `Message::CloseCurrentSession` with a `// TODO: switch to Message::CloseSessionAt(0) when Task 02 lands.` comment. Task 02 has shipped. The test misrepresents what production code emits.
- **Required Action:** Replace `Message::CloseCurrentSession` with `Message::CloseSessionAt(0)` and remove the TODO.
- **Acceptance:** `cargo test -p fdemon-app mouse_regions` passes; grep for `CloseCurrentSession` in `mouse_regions.rs` returns zero hits.

### 2. Dead `to_mouse_rect` helper
- **Source:** architecture_enforcer, code_quality_inspector
- **File:** `crates/fdemon-tui/src/widgets/mod.rs:34-47`
- **Problem:** Marked `#[allow(dead_code)]` with stale "Task 07 will add the call site" comment. Task 07 shipped and uses `MouseRect::new(...)` directly. Suppressing a legitimate dead-code warning.
- **Required Action:** Delete the function and the `pub use crate::render::MouseCtx;` reordering if necessary.
- **Acceptance:** `cargo clippy --workspace --all-targets -- -D warnings` passes without `#[allow(dead_code)]` suppression on this helper.

### 3. `docs/REVIEW_FOCUS.md` missing `mouse_regions` exception entry
- **Source:** architecture_enforcer
- **File:** `docs/REVIEW_FOCUS.md` (Approved TEA Exception → Current usage)
- **Problem:** The doc states "New `Cell`-based render-hint fields require explicit review and documentation here." `MouseRegionsCell` was added to `AppState` but not registered.
- **Required Action:** Add a bullet under "Current usage":
  > `AppState::mouse_regions` — the renderer populates per frame; `handler/mouse/normal.rs::handle_press` reads for click hit-tests. Wrapped in `MouseRegionsCell` newtype to satisfy `#[derive(Debug)]`.
- **Acceptance:** Doc lists both exceptions explicitly.

### 4. TASKS.md narrative contradicts implementation (Settings-mode regions)
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **File:** `workflow/plans/features/mouse-support/phase-3-region-registry/TASKS.md:172`
- **Problem:** Plan says "Settings mode does not render the header, so the header regions are not in the registry." Reality: regions ARE recorded; the dispatcher (`handler/mouse/mod.rs:54-58`) gates non-Normal modes with `_ => None`. Net behavior matches the smoke test, but the doc misleads future maintainers.
- **Required Action:** Replace the line with: *"Settings/DevTools/Loading modes still populate the header registry, but the dispatcher in `handler/mouse/mod.rs` returns `None` for `_ => ` (non-Normal modes), so header clicks are silently dropped at click-time. The probe test `view_header_regions_present_in_settings_mode_because_header_always_renders` documents this."*
- **Acceptance:** Plan and code agree; future-self knows where the gate actually lives.

---

## Minor Issues (Consider Fixing)

### 5. Magic literal `4` in `register_shortcut_clicks`
- **File:** `crates/fdemon-tui/src/widgets/header.rs:159`
- **Suggested Action:** Add `const SHORTCUT_SEGMENT_PREFIX: u16 = 4; // '[' + key + ']' + ' '` near `SHORTCUT_CLICK_WIDTH = 2`. Use it: `let segment_width = SHORTCUT_SEGMENT_PREFIX + (label.len() as u16);`.

### 6. Inconsistent saturating arithmetic in shortcut-clicks overflow guard
- **File:** `crates/fdemon-tui/src/widgets/header.rs:163`
- **Suggested Action:** `if click_x.saturating_add(SHORTCUT_CLICK_WIDTH) > area.x.saturating_add(area.width)`.

### 7. `padded_area.height.max(1)` hides empty-rect guard
- **File:** `crates/fdemon-tui/src/widgets/tabs.rs:138`
- **Suggested Action:** Drop `.max(1)` and let `MouseRegionsBuilder::click_left_middle`'s `is_empty` check handle zero-height. Optionally early-return on zero height.

### 8. `*msg.clone()` is roundabout
- **File:** `crates/fdemon-app/src/mouse_regions.rs:87`
- **Suggested Action:** `(**msg).clone()` clones the inner `Message` directly without first cloning the Box.

### 9. Missing doc on `handle_scroll`
- **File:** `crates/fdemon-app/src/handler/mouse/normal.rs:75`
- **Suggested Action:** Add `///` doc comment matching the level of `handle_press`.

### 10. `tag_filter_visible` gate should live in the dispatcher
- **File:** `crates/fdemon-app/src/handler/mouse/normal.rs:33-35`
- **Suggested Action:** Lift to `handler/mouse/mod.rs::handle_press`. Phase 4/5 handlers won't have to remember it.

### 11. `Cell` take/set is not panic-safe
- **Files:** `crates/fdemon-tui/src/render/mod.rs:108-336`, `crates/fdemon-app/src/handler/mouse/normal.rs:44-54`
- **Suggested Action:** Introduce a `MouseRegionGuard<'a>` RAII type that holds `&Cell<MouseRegions>` and a `MouseRegions`, putting the value back on `Drop`. Replace manual take/set sites.

### 12. Under-tested `SessionManager::remove_session` callers
- **File:** `crates/fdemon-app/src/session_manager.rs`
- **Suggested Action:** Add three tests: (a) remove non-selected pre-selected — selection follows id; (b) `evict_oldest_stopped` doesn't shift selected_index unexpectedly; (c) failed-spawn removal preserves user selection.

### 13. `MouseRegionsCell::Debug` doc comment ↔ impl mismatch
- **File:** `crates/fdemon-app/src/mouse_regions.rs` (Debug impl)
- **Suggested Action:** Either reword the comment to match `finish_non_exhaustive` behavior, or actually expose `len()` in the Debug output.

### 14. Task 07 reconciliation audit trail
- **File:** `workflow/plans/features/mouse-support/phase-3-region-registry/tasks/07-tabs-and-device-pill-regions.md`
- **Suggested Action:** Append a "Reconciliation note" to the completion summary listing what landed (`tabs.rs` + minimal `header.rs` wiring) vs what was discarded (the first implementor's `render_main_header`/`TitleRowHints` rewrite).

### 15. `TODO(phase-5)` in render tests may drift
- **File:** `crates/fdemon-tui/src/render/tests.rs:59,105,156`
- **Suggested Action:** Move Phase-5 update notes from outer doc comments to inline comments next to the asserted counts.

### 16. Document `EmitWithCoord` closure invariant
- **File:** `crates/fdemon-app/src/mouse_regions.rs` (EmitWithCoord variant)
- **Suggested Action:** Add doc note: closures should use `saturating_sub`/`checked_sub` for any coordinate offset arithmetic, and capturing closures (which require widening to `Box<dyn Fn(...)>`) should be added as a new variant rather than widening this one.

### 17. `(4 + label.len()) as u16` cast can silently truncate
- **File:** `crates/fdemon-tui/src/widgets/header.rs:159`
- **Suggested Action:** `u16::try_from(SHORTCUT_SEGMENT_PREFIX + label.len()).expect("shortcut label too long")` (after extracting the constant in #5). Alternatively, a const-time assertion on label lengths.

### 18. Update `docs/ARCHITECTURE.md` Module Reference
- **File:** `docs/ARCHITECTURE.md`
- **Suggested Action:** Add `mouse_regions` (registry, `MouseAction`, hit-test) to `fdemon-app` module reference; add `MouseCtx` threading to `fdemon-tui` notes. May warrant a "Mouse Region Registry" sub-section under "Key Patterns."

---

## Re-review Checklist

After addressing items 1–4 (Required for Approval):

- [ ] All four major issues resolved
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes (without `#[allow(dead_code)]` on `to_mouse_rect`)

After addressing items 10–12 (recommended before Phase 4 dispatch):

- [ ] `MouseRegionGuard` introduced and replacing the manual `take`/`set` pairs
- [ ] `tag_filter_visible` gate lifted to the dispatcher
- [ ] At least three new `remove_session` tests added covering the decrement branch

---

## Estimated Effort

- **Required (items 1–4):** ~30 minutes — all mechanical edits.
- **Pre-Phase-4 prep (items 10–12):** ~2 hours — the RAII guard requires care but is contained.
- **Polish (items 5–9, 13–18):** ~1.5 hours — mostly small constant extractions and doc tweaks.

Total to fully discharge ACTION_ITEMS: ~4 hours.
