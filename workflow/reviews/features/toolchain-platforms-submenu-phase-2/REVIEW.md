# Code Review: Toolchain Platforms Submenu — Phase 2 (Expandable Platforms Submenu)

**Review Date:** 2026-06-09
**Change Type:** Feature (Phase 2 of multi-phase feature)
**Branch:** `feat/toolchain-platforms-submenu`
**Diff Base:** `git diff be63ecf..HEAD` (commits `a5255f4`, `af86f55`, `0999100`, `ebc9f5c`)
**Verdict:** ⚠️ **NEEDS WORK**

---

## Scope

Replace the flat `AndroidTools` step with an expandable **Platforms** submenu: add `WizardStepKind::Platforms` (non-executable parent) + 5 platform leaves (`PlatformAndroid` + 4 placeholders), `WizardStep.indent`, `InstallWizardState.platforms_expanded`, `build_steps(report, expanded)` projection with `rollup_step_statuses`, expand/collapse interactivity, and TUI indent/caret/dynamic-height. In Phase 2 only `PlatformAndroid` is functional; iOS/macOS/Web/Windows are host-gated placeholders (`Pending`, inert).

### Files Modified (source)

| File | Changes |
|------|---------|
| `install_wizard/types.rs` | `WizardStepKind` variants + `is_platform_leaf()` |
| `install_wizard/state.rs` | `WizardStep.indent`, `platforms_expanded`, `build_steps(report, expanded)`, `rollup_step_statuses` |
| `handler/install_wizard/actions.rs` | rename + `Platforms` no-op arm + placeholder leaf arms |
| `handler/install_wizard/navigation.rs` | `handle_toggle_expand`, Esc collapse tiering |
| `handler/keys.rs` | Enter routing parent→toggle; `l`/`h`/arrow bindings |
| `handler/{update,mod}.rs`, `message.rs`, `actions/mod.rs` | message + dispatch + executor catch-all |
| `widgets/install_wizard/{mod,step_list,step_detail}.rs` | indent, caret, dynamic height, footer hint |
| `docs/ARCHITECTURE.md` | Platforms submenu documentation |

---

## Agent Verdicts

| Agent | Verdict | Headline |
|-------|---------|----------|
| `architecture_enforcer` | ⚠️ CONCERNS | 0 layer violations; 2 warnings (directional keys, Esc/toggle divergence) |
| `code_quality_inspector` | ⚠️ APPROVED-WITH-CONCERNS | 1 major (contradictory comments), several minor |
| `logic_reasoning_checker` | ⚠️ CONCERNS | Esc-collapse-on-leaf lands on wrong step (C1); no crashes |
| `risks_tradeoffs_analyzer` | ⚠️ CONCERNS | 1 HIGH (directional keys), 1 MEDIUM (command-index drift) |
| `security_reviewer` | ✅ APPROVED | install trust boundary preserved; 2 LOW notes |

**Overall:** 3 CONCERNS + 1 approved-with-concerns + 1 approved → **NEEDS WORK**. No REJECTED, no CRITICAL/crash findings, but a cluster of real, cross-confirmed defects.

---

## What's Solid (confirmed)

- **Layer boundaries clean** — `fdemon-tui` reads `fdemon-app` state only; host gating reads `report.platform` (domain enum), not `cfg!`. No new cross-layer imports.
- **TEA respected** — `build_steps` is a pure function; mutation only in handlers; renderer reads `platforms_expanded`/`indent` and never writes them.
- **Security: install trust boundary preserved exactly** — `PlatformAndroid` keeps the same `is_jdk_actionable` gate, `AndroidStepParams` dispatch, and unchanged `sdkmanager`/license/PATH side effects. Placeholder leaves are double-stopped (handler `UpdateResult::none()` + executor `WizardStepFailed` catch-all). No new I/O, no injection surface (all titles static literals).
- **`rollup_step_statuses` precedence correct** — Missing > Partial > Ok, Pending neutral; collapsed and expanded parent statuses are consistent.
- **`build_steps` projection correct** — collapsed = 5 rows; expanded inserts host-gated leaves after the parent (Android+Web all hosts; iOS+macOS macOS-only; Windows Windows-only).
- **Phase 1 index-literal test trap retired** at the state layer (kind-lookup migration) — a genuine resilience win.
- **Forward-compat model is extensible** — `is_platform_leaf()`, pure host-gated `build_steps`, and the rollup set up Phases 3–5 cleanly.
- Quality gate green: fmt clean, **1491 lib tests pass**, clippy `-D warnings` clean.

---

## Findings (deduplicated, severity-ordered)

See `ACTION_ITEMS.md` for the full fix list. Summary:

### Must Fix
- **M1 — `h`/`Left` toggles instead of collapsing.** [risks HIGH-1, architecture WARNING, code_quality NITPICK] `keys.rs` maps both `l`/`Right` and `h`/`Left` to `InstallWizardToggleExpand` (a flip), but the doc-comments promise directional expand/collapse. `h` on a collapsed parent *expands* it. Fix: split into directional `Expand`/`Collapse` (set, not flip), or correct the doc-comments to "toggle".
- **M2 — Esc-collapse with cursor on a leaf lands on the wrong step.** [logic C1] `handle_escape` collapse tier clamps only `selected_index >= len`; with the cursor on a leaf at an index that stays in-range after collapse (e.g. Linux index 3 = PlatformWeb → PathConfig), focus silently jumps to an unrelated top-level step instead of the Platforms parent. Bounds-safe (no panic) but semantically wrong, and untested (the existing test only asserts `< len`). Fix: re-anchor to the parent when the pre-collapse selection `is_platform_leaf()`; tighten the test to assert the landing `kind`.
- **M3 — `selected_command_index` reset diverges between the two collapse paths.** [architecture WARNING, risks MEDIUM-2] `handle_toggle_expand` resets it (line 105); `handle_escape`'s collapse tier does not. Benign today (parent has no commands; `.get()` read is safe) but a latent stale-index bug once leaves gain guided commands in Phase 3. Fix: extract a shared `collapse_platforms`/`rebuild_steps_preserving_cursor` helper used by both; reset in both.

### Should Fix
- **S1 — Self-contradictory comments in `step_list.rs` caret-fill logic.** [code_quality MAJOR] Comment claims the caret is "plain unstyled text" / "NOT counted" while the code styles it with `row_style` and counts it in `suffix_len`. Code correct, comments lie — maintenance hazard. Remove the false clauses.
- **S2 — Parent-status rollup re-derives the leaf set inline** (a second copy of the host-gating match) rather than rolling up the actually-built leaf steps. [risks forward-compat] Phase-3 sync trap. Roll up over the built leaves.
- **S3 — `report.as_ref().cloned()` clones the full `ToolchainReport` on every toggle/Esc.** [architecture SUGGESTION, code_quality MINOR] Avoidable via borrow-split (`build_steps` returns owned `Vec`, ends the borrow). Minor perf + clarity.
- **S4 — `rollup_step_statuses` allocates a `Vec` to call `.is_empty()`/`.contains()`.** [code_quality MINOR] Single-pass scan with bool flags is cleaner and allocation-free; add the missing single-`Ok` unit test.

### Minor / Consider
- **N1** — Magic literal `6` in `mod.rs` `saturating_sub(6)` → named constant w/ derivation comment (Responsive Layout Principle 4).
- **N2** — Placeholder leaf copy "Available in a later phase" reads like a TODO leak; soften to user-facing, action-oriented wording (track per-leaf for Phases 3–5).
- **N3** — Selected leaf row can render off-screen on short terminals when expanded (no scroll-to-selection). Pre-existing, aggravated by the larger expanded list. [logic C2, risks LOW-4]
- **N4** — TUI render-test coordinates are hardcoded literals; derive from `HEADER_HEIGHT`/`INDENT_WIDTH` or add a locate-by-kind buffer helper to stop recurring manual fixups.
- **N5** — `make_steps()` fixture deliberately bypasses `build_steps`; add a one-line note to prevent silent staleness.
- **N6** — (security) Add a `// Phase 2: only PlatformAndroid/Prerequisites have captions` note on the `step_caption` `_ => None` arm.

---

## Documentation Freshness

`docs/ARCHITECTURE.md` was updated in-scope (Task 04, validated). No new crates/build-steps/deps. `docs/KEYBINDINGS.md` — the new expand/collapse keys (`Enter`/`l`/`h`/arrows on the parent) are not yet documented there; if M1 changes the binding semantics, update KEYBINDINGS.md as part of that fix. Website docs are intentionally deferred to the platform-content phases (per Phase 2 plan).

---

## Recommendation

**Address M1–M3 before considering Phase 2 closed** — they are contained (no data/state corruption) but are genuine correctness/UX defects, two of which are currently untested. S1–S4 are low-cost and worth folding into the same pass. The Minor items can be tracked into Phase 3. None of the findings require re-architecting; the data model and layering are sound. After M1–M3 + S1–S4, re-run the quality gate and this review converts to APPROVED.
