# Action Items: Toolchain Platforms Submenu — Phase 2

**Review Date:** 2026-06-09
**Verdict:** ⚠️ NEEDS WORK
**Blocking (Must Fix):** 3 · **Should Fix:** 4 · **Minor:** 6

All findings are confirmed against the code. None cause a crash or data corruption; the Must-Fix tier is genuine correctness/UX defects, two of which are currently untested.

---

## Critical Issues (Must Fix)

### M1. `h`/`Left` toggles expand instead of collapsing (directional semantics inversion)
- **Source:** risks_tradeoffs_analyzer (HIGH-1), architecture_enforcer (WARNING), code_quality_inspector (NITPICK)
- **File:** `crates/fdemon-app/src/handler/keys.rs` (Enter/`l`/`h`/arrow routing, ~lines 458–473)
- **Problem:** Both `l`/`Right` and `h`/`Left` dispatch `InstallWizardToggleExpand`, which flips `platforms_expanded`. The doc-comments promise "`l`/`Right` — expand … `h`/`Left` — collapse". On a collapsed parent, `h`/`Left` (the universal "back out" key) *expands*; on an expanded parent, `l`/`Right` *collapses*. Backwards half the time, and a documented-vs-actual contract violation.
- **Required Action:** Either (a) introduce `InstallWizardExpand` / `InstallWizardCollapse` messages whose handlers *set* (not flip) the flag — `l`/`Right`→expand, `h`/`Left`→collapse, `Enter`→toggle; or (b) if a single toggle is intentional, rewrite the doc-comments to say "toggle" and drop "expand"/"collapse" wording. Option (a) is the correct UX. If the binding semantics change, update `docs/KEYBINDINGS.md`.
- **Acceptance:** Pressing `h`/`Left` on a collapsed parent is a no-op (or stays collapsed); on an expanded parent it collapses. `l`/`Right` mirrors. A test asserts each direction's effect. Doc-comments match behavior.

### M2. Esc-collapse with cursor on a leaf lands on an unrelated step (no re-anchor to parent)
- **Source:** logic_reasoning_checker (C1)
- **File:** `crates/fdemon-app/src/handler/install_wizard/navigation.rs` `handle_escape` (lines 61–76)
- **Problem:** The collapse tier clamps only `selected_index >= len`. With the cursor on a leaf whose index stays in-range after collapse (Linux: index 3 = PlatformWeb → collapses to PathConfig; macOS: index 4 = iOS → Doctor), focus silently jumps to an unrelated top-level step instead of the Platforms parent it descended from. Bounds-safe (no panic). The existing test `esc_collapse_clamps_selected_index` only asserts `selected_index < len`, so the defect is untested and will regress silently.
- **Required Action:** In the collapse path, if the pre-collapse selection `is_platform_leaf()`, set `selected_index` to the Platforms parent's position *before* the bounds clamp. Apply the same re-anchor in `handle_toggle_expand`'s collapse direction (cursor is already on the parent there, but the shared helper from M3 should cover both).
- **Acceptance:** After Esc-collapse from any leaf (Linux/macOS/Windows reports), `selected_step().kind == WizardStepKind::Platforms`. Tighten `esc_collapse_clamps_selected_index` (and add per-host cases) to assert the landing `kind`, not just `< len`.

### M3. `selected_command_index` reset diverges between `handle_escape` and `handle_toggle_expand`
- **Source:** architecture_enforcer (WARNING), risks_tradeoffs_analyzer (MEDIUM-2)
- **File:** `crates/fdemon-app/src/handler/install_wizard/navigation.rs` (`handle_escape` 63–72 omits the reset; `handle_toggle_expand` 105 does it)
- **Problem:** `handle_toggle_expand` resets `selected_command_index = 0`; the `handle_escape` collapse tier clamps `selected_index` but leaves `selected_command_index` stale. Benign today (`selected_command()` reads via `.get()`; parent has no commands) but a latent stale-index bug the moment a clamped-to step gains guided commands in Phase 3. Classic duplicated-logic drift.
- **Required Action:** Extract a shared private helper (e.g. `collapse_platforms(&mut InstallWizardState)` or `rebuild_steps_preserving_cursor`) that rebuilds the collapsed list, re-anchors the cursor (M2), clamps `selected_index`, and resets `selected_command_index`. Call it from both `handle_escape` and `handle_toggle_expand`.
- **Acceptance:** Both collapse paths leave identical state. A test asserts `selected_command_index == 0` after Esc-collapse. The rebuild+clamp logic exists in exactly one place.

---

## Major Issues (Should Fix)

### S1. Self-contradictory comments in `step_list.rs` caret-fill logic
- **Source:** code_quality_inspector (MAJOR)
- **File:** `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` (~lines 242–285)
- **Problem:** Comments claim the caret is "plain unstyled text" and "NOT counted" in the fill width, but the code styles it with `row_style` and `suffix_len` explicitly counts it. Code is correct; the comments lie and will mislead a future editor of the fill math.
- **Suggested Action:** Delete the false "not counted"/"unstyled" clauses; keep only the accurate description (caret styled with `row_style`; `suffix_len` accounts for it so the fill starts at the right offset).

### S2. Parent-status rollup re-derives the leaf set inline (Phase-3 sync trap)
- **Source:** risks_tradeoffs_analyzer (forward-compat wart)
- **File:** `crates/fdemon-app/src/install_wizard/state.rs` (`build_steps` parent-rollup block vs leaf-emission block)
- **Problem:** The parent status is computed from a hand-built parallel `vec![android_status, Pending, …]` rather than rolling up the actual leaf `WizardStep`s emitted below. When leaves gain real statuses in Phases 3–5, the two host-gating copies must be kept in sync manually.
- **Suggested Action:** Build the leaf `Vec<WizardStep>` first, then compute the parent status by `rollup_step_statuses` over the built leaves' statuses. Single source for the leaf set.

### S3. `report.as_ref().cloned()` clones the full `ToolchainReport` on every toggle/Esc
- **Source:** architecture_enforcer (SUGGESTION), code_quality_inspector (MINOR)
- **File:** `crates/fdemon-app/src/handler/install_wizard/navigation.rs:65, 98`
- **Problem:** `build_steps` takes `&ToolchainReport` and returns an owned `Vec`, so the clone is only there to dodge the `&mut wiz` / `&wiz.report` borrow conflict. It allocates a full report clone on a user-interactive keystroke.
- **Suggested Action:** Borrow-split: `if let Some(report) = &wiz.report { let steps = build_steps(report, wiz.platforms_expanded); wiz.steps = steps; }` — the `report` borrow ends before `wiz.steps =`. (Folds naturally into the M3 shared helper.) If a clone is genuinely required, add a one-line comment explaining the borrow-split rationale.

### S4. `rollup_step_statuses` allocates a `Vec`; missing single-`Ok` test
- **Source:** code_quality_inspector (MINOR #6, #3)
- **File:** `crates/fdemon-app/src/install_wizard/state.rs` (`rollup_step_statuses`)
- **Problem:** Collects a filtered `Vec` just to call `.is_empty()`/`.contains()`. A single-pass scan with bool flags is allocation-free and clearer (CODE_STANDARDS: "avoid collect-then-iterate"). Direct unit tests lack the `[Ok]` and `[Ok, Pending] → Ok` case.
- **Suggested Action:** Rewrite as a single pass over the slice with `any_real`/`any_missing`/`any_partial` flags. Add the missing single-`Ok` unit tests.

---

## Minor Issues (Consider)

### N1. Magic literal `6` in dynamic-height clamp
- **File:** `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` (~line 248, `area.height.saturating_sub(6)`)
- Per Responsive Layout Principle 4, replace with a named constant (e.g. `MIN_DETAIL_ROWS: u16 = 6`) and a derivation comment.

### N2. Placeholder leaf copy reads like a TODO leak
- **File:** `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` ("Available in a later phase")
- Soften to user-facing, action-oriented wording (e.g. point to `flutter doctor`). Track per-leaf replacement as each platform becomes real in Phases 3–5.

### N3. Selected leaf can render off-screen on short terminals (expanded)
- **Source:** logic_reasoning_checker (C2), risks_tradeoffs_analyzer (LOW-4)
- **File:** `step_list.rs` (`take(visible_height)`, no scroll-to-selection)
- Pre-existing; the larger expanded list makes overflow reachable. Consider a scroll offset keyed to `selected_index`. Verify on an ~18-row terminal in macOS expanded mode.

### N4. TUI render-test coordinates are hardcoded literals
- **Source:** risks_tradeoffs_analyzer (tech-debt #2)
- **File:** `step_list.rs` render tests (`buf[(x,y)]` assertions)
- Derive coordinates from `HEADER_HEIGHT`/`INDENT_WIDTH`, or add a locate-glyph-by-kind buffer helper, to stop recurring manual coordinate fixups.

### N5. `make_steps()` fixture bypasses `build_steps` silently
- **File:** `step_list.rs` (`make_steps()` helper)
- Add a one-line comment noting it is a deliberately hand-rolled fixture not mirroring `build_steps`, to prevent silent staleness.

### N6. `step_caption` `_ => None` arm exhaustiveness note
- **Source:** security_reviewer (LOW)
- **File:** `step_detail.rs` (`step_caption`)
- Add `// Phase 2: only PlatformAndroid and Prerequisites have captions; new leaf captions need an executor arm` on the `_ => None` arm.

---

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] M1 resolved — directional keys behave per their doc-comments (or comments corrected to "toggle"); KEYBINDINGS.md updated if semantics changed
- [ ] M2 resolved — Esc-collapse from any leaf re-anchors to the Platforms parent; test asserts landing `kind` per host
- [ ] M3 resolved — single shared collapse helper; `selected_command_index` reset in both paths; equivalence test
- [ ] S1–S4 resolved or explicitly deferred with justification
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo test --workspace --lib` green (with the new/tightened tests)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
