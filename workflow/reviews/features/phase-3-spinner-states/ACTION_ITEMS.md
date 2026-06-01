# Action Items: Phase 3 — Spinner in More States

**Review Date:** 2026-05-28
**Verdict:** ⚠️ APPROVED WITH CONCERNS
**Blocking Issues:** 0 (none block merge; 2 recommended before `main`)

## Critical Issues (Must Fix)

None.

## Major Issues (Should Fix Before Merge to main)

### 1. Replace the tautological phase-coherence test
- **Source:** code_quality_inspector (corroborated: architecture_enforcer, logic_reasoning_checker)
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`
- **Test:** `test_tab_bar_phase_coherence_both_spinners_from_same_frame`
- **Problem:** Asserts `spinner_char(8 / SPINNER_TICKS_PER_FRAME) == spinner_char(8 / SPINNER_TICKS_PER_FRAME)` — a tautology that proves only determinism, not that the discovery line and tab bar actually share a frame in production.
- **Required Action:** Render a `TargetSelector` with `loading = true`, `refreshing = true`, `.animation_frame(4)` at a height showing both the tab-bar row and the loading line; assert both rendered regions contain `SPINNER_FRAMES[(4 / 2) % 10]` (`'⠹'`).
- **Acceptance:** The new test fails if either call site changes its operand or divisor; passes on current code.

### 2. Resolve the dead `TabBar.icons` field
- **Source:** code_quality_inspector, risks_tradeoffs_analyzer (also raised in task-03 validation)
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`
- **Problem:** `TabBar.icons` is dead after the refresh glyph moved to `spinner_char`; retained behind `#[allow(dead_code)]` citing "API stability (15 call sites)" — but `TabBar` is private with only 2 production call sites.
- **Required Action (pick one):**
  - **(a) Remove** the `icons` parameter from `TabBar::new` and the struct field; delete the `#[allow(dead_code)]`; update 2 production call sites (`target_selector.rs`, `new_session_dialog/mod.rs`) + ~12 in-file test constructions. Mechanical, low-risk. **(preferred)**
  - **(b) Keep** only if a concrete Unicode/Nerd-Font spinner-glyph variant is planned — then replace the "API stability" comment with the actual revival rationale and a tracking reference.
- **Acceptance:** Either no `#[allow(dead_code)]` remains on `TabBar`, or the field carries a documented, concrete future-use justification.

## Minor Issues (Consider / Follow-up)

1. **Deduplicate the discovery-loading renderer** — extract a `pub(super)` helper shared by `TargetSelector::render_loading` and `mod.rs::render_target_selector_regions` (currently identical `Paragraph` built in two places).
2. **Animate the compact tab-bar refresh glyph** — `render_tabs_compact` still uses static `icons.refresh()`; the `animation_frame` is already threaded into `TargetSelector`, so this is a low-cost consistency fix.

## Low / Hardening (Nice to Have)

1. Add compile-time guards in `spinner.rs`: `const _: () = assert!(!SPINNER_FRAMES.is_empty());` and `const _: () = assert!(SPINNER_TICKS_PER_FRAME > 0);`.
2. Introduce `let len = SPINNER_FRAMES.len() as u64;` in `spinner_char` for readability.
3. Replace hard-coded `/ 2` in `tab_bar.rs` comments with references to `SPINNER_TICKS_PER_FRAME`.

## Pre-existing Debt (Out of This PR's Scope — Track Separately)

- `Cell` write at `new_session_dialog/mod.rs:958` (`last_known_visible_height.set`) lacks the mandatory `// EXCEPTION:` annotation. Confirmed not introduced by this change.
- Stale "~100ms tick rate" comments in `LoadingState::tick` (`crates/fdemon-app/src/state.rs`) contradict the actual 50ms tick.

## Re-review Checklist

After addressing Major items:
- [x] M1 test renders both spinner sites and asserts the same glyph at a non-zero frame
- [x] M2 resolved: dead field removed, or `#[allow]` replaced with a concrete documented rationale
- [x] `cargo test -p fdemon-tui` passes
- [x] `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch
**Commit:** b657ab0

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | M1: Replaced tautological phase-coherence test with render-level buffer inspection. M2: Removed `icons: &'a IconSet` field, `#[allow(dead_code)]`, `icons` param from `TabBar::new`, lifetime `'a`, and `IconSet` import; updated ~10 in-file test call sites. |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | M2: Removed `&self.icons` argument from `TabBar::new` call site. |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | M2: Removed `self.icons` argument from `TabBar::new` call site. |

### Notable Decisions/Tradeoffs

1. **M1 approach — simultaneous render of both spinner sites**: `TargetSelector::render_full` renders the tab bar unconditionally at rows 0-2, and the loading-line content at rows 3+, so `loading=true` and `connected_refreshing=true` can coexist. We render at `animation_frame=4` and inspect both buffer regions independently for the expected glyph `'⠹'` (`SPINNER_FRAMES[(4/2)%10]`). This catches any future divergence of operand or divisor at either call site.

2. **M2 lifetime removal**: After removing `icons: &'a IconSet`, `TabBar` had no remaining lifetime-bounded fields. The `'a` lifetime parameter was dropped entirely. `TargetSelector.icons` (an owned `IconSet`) is untouched — it remains live for `render_tabs_compact`.

### Testing Performed

- `cargo test -p fdemon-tui` — Passed (1331 tests)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
