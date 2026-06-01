# Code Review: Phase 6 — Reload Success Flash

**Review Date:** 2026-05-31
**Reviewer:** reviewer skill (5-agent consolidated)
**Change Type:** Feature implementation
**Diff Base:** `git diff 6d952f7f..HEAD` (commits `a118a5f9`, `661b51c3`)
**Branch:** `feat/ux-polish-and-multilaunch`

## Verdict: ⚠️ APPROVED WITH CONCERNS

The implementation is correct, well-tested, and architecturally clean. Four of five
reviewers returned PASS/Acceptable. One (`code_quality_inspector`) flagged a doc-accuracy
issue and minor style items — none are runtime bugs, and none block merge. The concerns
below are worth a quick follow-up but do not require re-implementation.

## Files Reviewed

| File | Change |
|------|--------|
| `crates/fdemon-app/src/session/session.rs` | New pure `Session::reload_flash_alpha(now) -> f32` helper + `RELOAD_FLASH_DURATION_MS` const + 7 tests |
| `crates/fdemon-tui/src/widgets/header.rs` | `reload_flash` field + builder on `MainHeader`; blend block bg via existing `lerp_color`; blend cap const + 2 tests |
| `crates/fdemon-tui/src/render/mod.rs` | Compute alpha from selected session, pass into `MainHeader` |

## Agent Verdicts

| Agent | Verdict | Headline |
|-------|---------|----------|
| architecture_enforcer | ✅ PASS | 0 violations; layer boundaries, TEA purity, `lerp_color` reuse all clean |
| code_quality_inspector | ⚠️ NEEDS WORK | 1 MAJOR (doc/clamp mismatch), 3 MINOR (const placement, test magic numbers) |
| logic_reasoning_checker | ✅ PASS | Decay math, suppression guard, borrow flow all sound; 2 optional notes |
| risks_tradeoffs_analyzer | ✅ Acceptable | 0 blocking; LOW notes on implicit redraw invariant + missing visual check |
| security_reviewer | ✅ PASS | 0 critical; 2 LOW informational; effectively no security surface |

## Consolidated Findings

### 🟠 MAJOR

**M1. `MainHeader::reload_flash` doc claims clamping the builder does not perform**
- **Source:** code_quality_inspector (MAJOR), logic_reasoning_checker (Note)
- **File:** `crates/fdemon-tui/src/widgets/header.rs:63-70`
- **Problem:** The builder doc states "Values outside `[0.0, 1.0]` are clamped," but the
  builder stores the raw `f32` (`self.reload_flash = alpha;`). Clamping actually happens
  downstream inside `lerp_color` (`shimmer.rs:27`), and only on the *product*
  `reload_flash * RELOAD_FLASH_BLEND_CAP`, not on `alpha` itself.
- **Runtime impact:** **None in practice.** `reload_flash_alpha` only ever returns
  `[0.0, 1.0]`, and `lerp_color` clamps its `t`, so no malformed color can reach the
  terminal. The issue is a misleading contract for any future caller that inspects
  `self.reload_flash` or passes an arbitrary float directly.
- **Recommended fix (pick one):**
  - (a) Make the doc true and the field a real invariant: `self.reload_flash = alpha.clamp(0.0, 1.0);`
  - (b) Reword the doc to state clamping is applied downstream by `lerp_color`.
  - Option (a) is preferred — it removes the NaN/inf propagation path and makes the
    field reliable.

### 🟡 MINOR

**m1. `RELOAD_FLASH_DURATION_MS` placed as an inherent associated const, not module-level**
- **Source:** code_quality_inspector (MINOR); architecture_enforcer judged it acceptable
- **File:** `crates/fdemon-app/src/session/session.rs:658`
- **Problem:** Placed as `impl Session { const RELOAD_FLASH_DURATION_MS … }` rather than a
  module-level `const`. Every other constant in this file/codebase and the
  `CODE_STANDARDS.md` magic-number example use module-level `const`. Not wrong, just
  diverges from the established idiom for a simple timing constant.
- **Note:** This was a deliberate choice per the task completion summary (co-location).
  Reviewers split on it — architecture called it fine, quality called it non-idiomatic.
  Low-stakes; fix only if aligning with codebase convention is desired.

**m2. Header test hardcodes RGB channel bounds (`21`, `185`) instead of deriving from palette**
- **Source:** code_quality_inspector (MINOR)
- **File:** `crates/fdemon-tui/src/widgets/header.rs:807-813` (`header_bg_tints_toward_green_with_flash`)
- **Problem:** The assertion `g > 21 && g < 185` embeds `CARD_BG.g` and `STATUS_GREEN.g`
  as literals in a comment-documented form. If either palette constant changes, the
  assertion silently goes stale — the exact maintenance hazard the no-magic-numbers
  standard targets.
- **Recommended fix:** Pattern-match `palette::CARD_BG` / `palette::STATUS_GREEN` to
  extract `card_g` / `green_g` and assert `g > card_g && g < green_g`, so the test
  self-updates.

### 🔵 LOW / Track (non-blocking)

**L1. Fade correctness rests on an undocumented "unconditional per-tick redraw" invariant**
- **Source:** risks_tradeoffs_analyzer (LOW)
- **Detail:** The 500 ms fade animates only because `runner.rs:333` calls `terminal.draw`
  on every ~50 ms loop iteration regardless of state change. This holds today (confirmed),
  but a future "redraw only on state change" optimization — which `REVIEW_FOCUS.md`
  explicitly invites — would silently freeze the flash at full intensity. No unit test
  guards this.
- **Recommendation:** Add a one-line comment at `runner.rs:333` noting the unconditional
  redraw is load-bearing for time-based animations (reload flash, shimmer, spinner).

**L2. Acceptance criterion 4 (visible fade in a live run) not manually verified**
- **Source:** risks_tradeoffs_analyzer, logic_reasoning_checker, task validator
- **Detail:** Math and blending are unit-tested; the `0.35` cap and 10-frame fade
  smoothness are aesthetic judgments unit tests cannot confirm.
- **Recommendation:** One manual run to confirm the tint reads as intended (closes
  criterion 4). Can be done via the `/verify` or `/run` skill.

**L3. Flash also fires on hot restart, not only hot reload**
- **Source:** risks_tradeoffs_analyzer (LOW)
- **Detail:** `complete_reload()` is called by both the hot-reload and hot-restart
  completion handlers (`handler/update.rs:222`, `:246`). The flash is arguably correct
  for both ("operation succeeded"), but the doc/task language says only "hot reload."
- **Recommendation:** Cosmetic — optionally reword docs to "reload/restart success."

**L4. No end-to-end test that a *failed* reload produces zero flash**
- **Source:** logic_reasoning_checker (optional)
- **Detail:** The suppression invariant ("failed reload leaves phase `Running` but does not
  stamp `last_reload_time` → alpha 0.0") is verified by inference from two separately
  tested pieces, not one integration test. A small handler test would lock it against
  future edits to `update.rs:231`.

**L5. Security (informational only)**
- **Source:** security_reviewer (2 LOW)
- **Detail:** `elapsed_ms as f32` is provably safe (guarded to `[0,499]`, exact in f32).
  A pre-existing `expect` in `register_shortcut_clicks` operates on compile-time `const`
  data and cannot fire. No action required.

## Documentation Freshness

✅ **No doc updates required.** No new crates/modules, no `Cargo.toml`/build changes, no
new conventions. Both task files explicitly note no `ARCHITECTURE.md` update is needed
(no `AppPhase`/`Message`/module-structure change).

## Strengths

- Pure, injectable helper with 7 unit tests including a property sweep over `[-100, +1000] ms`.
- Zero new state, no new timer, no new `AppState`/`Session` field — flash is a computed
  property over existing `last_reload_time` + the existing tick loop.
- Single blend point in `render_main_header` covers both single- and multi-session layouts.
- Correct reuse of Phase 2's `lerp_color` (no duplicated RGB math); graceful non-RGB
  terminal degradation.
- Clock-skew and phase-suppression guards correct and verified; borrow flow NLL-safe.
- Full workspace suite green (6147 tests), fmt + clippy clean.

## Recommended Next Steps

1. Address **M1** (clamp in builder or fix the doc) — quick, removes a latent contract trap.
2. Optionally address **m1**/**m2** for codebase-idiom alignment and test robustness.
3. Add the **L1** comment at `runner.rs:333` to protect the animation invariant.
4. Do one manual run (**L2**) to close acceptance criterion 4.

None of the above block merge.
