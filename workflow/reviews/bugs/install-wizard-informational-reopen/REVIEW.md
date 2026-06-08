# Code Review: Install Wizard Informational Re-open (`WizardOrigin`)

**Review Date:** 2026-06-08
**Change Type:** Bug Fix
**Diff Base:** `cfeb2dd..HEAD` (commits `717e37d`, `8709d86`, `eb8dba3`, `ff795ce`)
**Verdict:** ⚠️ **APPROVED WITH CONCERNS**

## Scope

Fixes a bug where pressing `I` to re-open the Install Wizard on a healthy toolchain
auto-advanced into the new-session dialog instead of showing a read-only informational
view. Introduces `WizardOrigin { Bootstrap, UserInvoked }`, threads it through
`Message::ShowInstallWizard { origin }` → `show_install_wizard(origin)` →
`InstallWizardState::opening(origin)`, and gates the post-install handback so only a
`Bootstrap`-origin wizard hands back to device discovery.

**Files (15):** `install_wizard/{types,mod,state}.rs`, `state.rs`, `message.rs`,
`handler/{update,keys}.rs`, `handler/install_wizard/{navigation,actions}.rs`,
`fdemon-tui/{runner.rs, widgets/install_wizard/mod.rs, widgets/install_wizard/step_detail.rs, render/tests.rs}`,
`docs/{ARCHITECTURE,KEYBINDINGS}.md`.

## Agent Verdicts

| Agent | Verdict | Headline |
|-------|---------|----------|
| bug_fix_reviewer | ✅ PASS | Root cause addressed; all close paths gated; single source of truth preserved |
| architecture_enforcer | ✅ PASS | 0 layer/TEA violations; correct enum placement & re-export hygiene |
| code_quality_inspector | ✅ PASS (minor) | Clean idioms; 1 typo + missing direct unit tests for 2 new pub fns |
| logic_reasoning_checker | ⚠️ CONCERN | Pre-existing predicate divergence (`flutter_now_live` vs `flutter_executable`) |
| risks_tradeoffs_analyzer | ✅ Acceptable | 1 MEDIUM UX concern (install-via-`I` dead-end); track follow-ups |
| security_reviewer | ✅ PASS | 0 critical/high/medium; 3 LOW observations, all safe-as-inherited |

## Verdict Rationale

The change correctly and completely fixes its targeted bug. It is additive, compile-time
enforced across all ~25 call sites, and well-tested (handler + render + startup paths).
No critical or blocking issues. The single CONCERN is a **pre-existing latent issue**, not a
regression introduced here — verified against the diff (see Finding 1). It is recorded as a
tracked follow-up rather than a merge blocker.

---

## Findings

### 🟠 MAJOR (track — not blocking this change)

**1. Predicate divergence: `flutter_now_live()` (report) vs `flutter_executable()` (`resolved_sdk`)** — *pre-existing*
[Source: logic_reasoning_checker]
`crates/fdemon-app/src/handler/install_wizard/actions.rs:62-69, 97-112`

The auto-close arm fires on `flutter_now_live()` (reads `report.components`), but the actual
handback inside `close_wizard_and_dispatch_discovery` requires `flutter_executable()` (reads
`AppState::resolved_sdk`, set by the separate `Message::SdkResolved` at `update.rs:3170`). If a
post-install report shows `FlutterSdk: Ok` before `resolved_sdk` is repopulated, a **Bootstrap**
wizard would close to `UiMode::Normal` with no `DiscoverDevices` and `handback_done` left
`false` — defeating the bootstrap handback.

**Verified pre-existing:** the diff shows both predicates existed before this change; this bugfix
only added the `is_bootstrap()` gate, which makes handback *less* likely to fire, never more. So
no regression is introduced. Still worth resolving:
- Confirm/document the invariant that `SdkResolved` always precedes `InstallWizardPreflightCompleted`
  when a managed install completes; **or**
- Align both arms on one data source; **or**
- Set `handback_done` + dispatch discovery whenever `flutter_now_live()` held.
- Add a regression test: live *report* with `resolved_sdk == None` → assert intended bootstrap mode/actions.

### 🟡 MEDIUM

**2. "All set" hint is misleading after install-via-`I` on a broken toolchain (strict Option-1 cost)**
[Source: risks_tradeoffs_analyzer]
`crates/fdemon-tui/src/widgets/install_wizard/mod.rs:140`

If a user presses `I` on a *broken* toolchain (origin `UserInvoked`), installs Flutter to
completion, the report becomes all-Ok and the "All set — press Esc to return" hint appears — but
Esc drops them to an empty logs view (`UiMode::Normal`) with no session and no prompt to start one.
The strict no-handback decision is intentional (BUG.md behaviour matrix), but the in-product
affordance doesn't match it. **Recommend a follow-up:** tailor the post-install-via-`I` hint
(e.g. "Flutter installed — press `n` to start a session"), or reuse the existing
`!has_running_sessions()` guard to allow handback in this specific transition.

### 🟡 MINOR

**3. Typo in inline comment: "informally" → "informationally"**
[Source: code_quality_inspector] `crates/fdemon-tui/src/widgets/install_wizard/mod.rs:137`
Only place using "informal"; rest of the codebase says "informational".

**4. New `pub` fns `all_components_ok()` and `is_bootstrap()` lack direct state-level unit tests**
[Source: code_quality_inspector, risks_tradeoffs_analyzer] `crates/fdemon-app/src/install_wizard/state.rs:172-184`
CODE_STANDARDS requires tests for all new public functions. Currently covered only indirectly via
render/handler tests. Add direct tests: no-report → false, empty-components → false,
`Unknown`-status → false, all-Ok → true; `is_bootstrap` for both origins.

**5. Doc-comment contradiction on `UserInvoked` close mode**
[Source: logic_reasoning_checker] `install_wizard/types.rs:12` says `Esc` "returns to the previous
mode"; `navigation.rs:18` and the implementation hardcode `UiMode::Normal`. Reconcile (the `I`
binding is only reachable from `Normal`, so "Normal" is accurate — fix the `types.rs` comment).

**6. `all_components_ok()` doc omits the `Unknown`-status exclusion** (stricter than `rollup_status`)
[Source: code_quality_inspector] `crates/fdemon-app/src/install_wizard/state.rs:176`

### 🔵 NITPICK / OBSERVATIONS

- **7.** Consider adding a `docs/REVIEW_FOCUS.md` "Approved Exception" entry documenting that
  `close_wizard_and_dispatch_discovery` is the single handback point and `UserInvoked` is
  intentionally inert — prevents future regressions. [Source: architecture_enforcer]
- **8.** `bootstrap_handback_skipped_when_session_running` (`actions.rs`) asserts `!= Startup`
  and no `DiscoverDevices`; also assert `ui_mode == Normal` for an unambiguous final state.
  [Source: bug_fix_reviewer]
- **9.** No direct test for `UserInvoked` + `HideInstallWizard` (Esc is covered; Hide delegates to
  the same helper, so low risk). [Source: bug_fix_reviewer]
- **10.** 3 LOW security observations — `"Copied: {command}"` echo and Linux prereq command
  construction both draw from static dispatch tables (not daemon-sourced strings), so safe;
  re-evaluate if a future `GuidedCommand` carries daemon-sourced strings. [Source: security_reviewer]
- **11.** `TEXT_SECONDARY → TEXT_MUTED` subtitle color change is justified (harmonises with the
  `[Esc] Close` hint). [Source: code_quality_inspector]

## Documentation Freshness

✅ Fresh. `docs/ARCHITECTURE.md` (module descriptions) and `docs/KEYBINDINGS.md` (`I`/`Esc`
behaviour) were both updated in this change. Optional: the `REVIEW_FOCUS.md` entry in Finding 7.

## Strengths

- Root cause fixed at the right layer: intent explicit at entry points, gated centrally.
- Single source of truth (`close_wizard_and_dispatch_discovery`) — auto-close and manual-close cannot drift.
- Safe-by-default `#[default] UserInvoked` (never accidentally hands back).
- Defensive `!has_running_sessions()` guard limits blast radius of the known Phase-5 CRIT.
- No backward-compat risk: `Message` is an internal type with no serde; enum change is pure compile-time fan-out.
- Strong render + handler test coverage for the primary invariant.

## Recommendation

**Merge-ready.** None of the findings block this bug fix. Address the quick wins (Findings 3, 5)
in a small follow-up commit if convenient, and file tracked follow-ups for Findings 1, 2, and 4
(test coverage). Findings 1 and 2 are good candidates to join the existing `phase-5-followup/`
task set.
