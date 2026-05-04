# Feature Review: Mouse Support — Phase 3 (Region Registry + Clickable Header & Tabs)

**Review Date:** 2026-05-04
**Reviewer:** Code Review Orchestrator
**Task Files Reviewed:** 8 tasks (all Done; Task 07 via manual reconciliation)
**Files Changed:** 15 files (+2053 / −216 lines)
**Diff Range:** `1ed7edf..HEAD` on `feat/mouse-support`

---

## Executive Summary

**Overall Verdict:** ⚠️ **APPROVED WITH CONCERNS**

Phase 3 lands a clean per-frame mouse-region registry in `fdemon-app`, threads a `MouseCtx` from `render::view` into header & tabs widgets, and wires header shortcut clicks, tab select/close, and the device pill — all behind a z-index-aware hit-test. Layer boundaries are preserved (`fdemon-app` has zero `ratatui` imports; `MouseRect` is local). 5,131 tests pass, fmt/clippy/check all green. The implementation is functionally correct but accumulates small but visible polish debt — most notably a stale TODO in tests, a dead `to_mouse_rect` helper kept under `#[allow(dead_code)]`, and a docs/code drift around Settings-mode region recording. None of the findings are critical bugs; they are tractable cleanups that should land before Phase 4 multiplies the surface area.

---

## Changes Overview

### Task Files Reviewed

| Task | Status | Files Modified |
|------|--------|----------------|
| `tasks/01-mouse-regions-module.md` | Done | 2 (NEW `mouse_regions.rs` + `lib.rs`) |
| `tasks/02-add-close-session-at-message.md` | Done | 4 |
| `tasks/03-state-field-and-exports.md` | Done | 3 |
| `tasks/04-tui-mouse-ctx-plumbing.md` | Done | 3 |
| `tasks/05-handle-press-normal-mode.md` | Done | 2 |
| `tasks/06-header-bracket-regions.md` | Done | 5 |
| `tasks/07-tabs-and-device-pill-regions.md` | Done (manual reconciliation) | 4 |
| `tasks/08-integration-tests-and-snapshot.md` | Done (CONCERN logged) | 2 |

### Files Changed (15)

```
crates/fdemon-app/src/handler/mouse/mod.rs           |   +29 / -7
crates/fdemon-app/src/handler/mouse/normal.rs        |  +297 / -10
crates/fdemon-app/src/handler/session_lifecycle.rs   |   +91 / -57
crates/fdemon-app/src/handler/tests.rs               |  +313 / -0
crates/fdemon-app/src/handler/update.rs              |    +1 / -0
crates/fdemon-app/src/lib.rs                         |    +5 / -0
crates/fdemon-app/src/message.rs                     |    +6 / -0
crates/fdemon-app/src/mouse_regions.rs               |  +427 / -0  (NEW)
crates/fdemon-app/src/session_manager.rs             |   +73 / -2
crates/fdemon-app/src/state.rs                       |   +35 / -0
crates/fdemon-tui/src/render/mod.rs                  |   +30 / -1
crates/fdemon-tui/src/render/tests.rs                |  +159 / -16
crates/fdemon-tui/src/widgets/header.rs              |  +271 / -29
crates/fdemon-tui/src/widgets/mod.rs                 |   +18 / -3
crates/fdemon-tui/src/widgets/tabs.rs                |  +298 / -91
```

---

## Subagent Review Summaries

### Architecture Enforcer
**Verdict:** ✅ PASS

Clean: `fdemon-app` has zero `ratatui` imports; the local `MouseRect` and the boundary-side `to_mouse_rect` helper preserve the layer invariant. `MouseCtx` threading correctly avoids exposing `&AppState` to widgets. Re-export hygiene is clean.

**Key Findings:**
- TEA exception annotation present at every Cell take/set site (`render::view`, `handle_press`).
- `docs/REVIEW_FOCUS.md` "Current usage" list still names only `TargetSelectorState::last_known_visible_height` — `AppState::mouse_regions: MouseRegionsCell` was added to state.rs with the EXCEPTION comment but not registered in REVIEW_FOCUS.md as the doc itself requires.
- `to_mouse_rect` is dead with a stale `// Task 07 will add the call site` comment — Task 07 has shipped and constructs `MouseRect::new(...)` inline.

### Code Quality Inspector
**Verdict:** ⚠️ NEEDS WORK

Core implementation is solid. Three concrete issues warrant attention before merge: a stale TODO referencing already-shipped Task 02, an undocumented magic `4` literal, and a dead helper kept alive by `#[allow(dead_code)]`.

**Quality Scores:**
| Metric | Score |
|--------|-------|
| Language Idioms | ⭐⭐⭐⭐ |
| Error Handling | ⭐⭐⭐⭐⭐ |
| Testing | ⭐⭐⭐⭐ |
| Documentation | ⭐⭐⭐⭐ |
| Maintainability | ⭐⭐⭐⭐ |

### Logic & Reasoning Checker
**Verdict:** ⚠️ PASS WITH CONCERNS

z-index hit-test (`enumerate + max_by_key((z, push_idx))`) is semantically correct and arguably cleaner than the spec's proposal. Take/put-back invariants are preserved across all early-return paths. `remove_session`'s three-branch logic was traced exhaustively against four scenarios — all correct. `CloseSessionAt` semantics, busy gate, and the OOB → no-op contract all check out.

**Key Findings:**
- TASKS.md narrative (line 172: *"Settings mode does not render the header, so the header regions are not in the registry"*) contradicts the implementation — header **is** rendered in Settings mode and regions **are** populated. Net runtime behavior is still correct because the dispatcher (`handler/mouse/mod.rs:54-58`) returns `None` for `_ => ` (non-Normal) modes, so a Settings-mode click is silently dropped at the dispatcher, not the registry. **Docs/code drift, not a runtime bug.**
- The `tag_filter_visible` early-return lives inside `normal::handle_press` rather than the dispatcher. Phase 4/5 per-mode handlers will each have to remember this gate — fragile.

### Risks & Tradeoffs Analyzer
**Verdict:** ⚠️ CONCERNS

Identified one item the analyzer flagged as MEDIUM-HIGH (Settings-mode header click hazard) — but the **logic checker correctly verified this is mitigated by the dispatcher's `_ => None` arm**, so the runtime hazard is dispelled. The remaining concerns center on panic-safety of the `Cell::take`/`set` pair and under-tested behavioral changes in `SessionManager::remove_session`.

**Identified Risks:**
| Risk | Severity | Mitigated? |
|------|----------|------------|
| Cell take/put-back not panic-safe (widget panic → mouse silently disabled until next render in `view`; permanently in `handle_press`) | Medium | No — recommend RAII guard before Phase 4 expands surface area |
| Settings-mode header regions emit live messages | Medium-High → **Low** | Yes — dispatcher gates non-Normal modes; smoke-test passes per logic trace |
| `remove_session` selected_index decrement under-tested across all callers (`evict_oldest_stopped`, `handle_session_spawn_failed`) | Medium | Partial — only `test_remove_session` exercises the new branch |
| `Box<Message>` per-region heap alloc | Low | Yes — alloc cost is negligible vs render cost |
| `MouseRegionsCell::Debug` doc comment promises behavior the impl doesn't deliver | Low | No — comment-fix needed |
| `EmitWithCoord(fn(u16,u16) -> Message)` cannot capture state (Phase 4 may force API widening) | Low | No — document constraint |
| Manual reconciliation of Task 07 left no audit trail of discarded vs landed deltas | Low | No — annotate completion summary |

### Security Reviewer
**Verdict:** ✅ PASS

No exploitable vulnerabilities. All external inputs (mouse coordinates, device names, session indices) are bounded and validated. `MouseRect::contains` uses guarded subtraction. `session_id_at` uses `slice::get` (no naked indexing). `truncate_name` is Unicode-safe with `chars().count()`.

**Security Findings:**
| Finding | Category | Severity |
|---------|----------|----------|
| `register_shortcut_clicks` overflow guard uses bare `u16` `+` instead of `saturating_add` (cursor uses saturating; the guard does not — inconsistent style) | Integer Overflow | Medium (theoretical only at u16::MAX widths) |
| `(4 + label.len()) as u16` cast can silently truncate if a future contributor adds a label longer than 65 531 chars | Integer Overflow | Low |
| Take/set window between lines 44–54 of `normal.rs` must remain panic-free; not enforced structurally | Panic Safety | Low |
| `EmitWithCoord` closures should use `saturating_sub` for any offset arithmetic — invariant undocumented | Input Validation | Low |

### Documentation Freshness
**Status:** ⚠️ Updates needed

| Doc | Needs Update? | Reason |
|-----|--------------|--------|
| `docs/ARCHITECTURE.md` | Yes | New `mouse_regions` module in `fdemon-app` (registry, hit-test, `MouseAction`); new `MouseCtx` type in `fdemon-tui`. The "Module Reference" and "Key Types" sections currently make no mention of the click-region registry or its threading model. |
| `docs/CODE_STANDARDS.md` | No | No new patterns established. |
| `docs/DEVELOPMENT.md` | No | No build/dep changes. |
| `docs/REVIEW_FOCUS.md` | Yes | "Approved TEA Exception → Current usage" lists only `TargetSelectorState::last_known_visible_height`. Per the doc's own rule ("New `Cell`-based render-hint fields require explicit review and documentation here"), `AppState::mouse_regions: MouseRegionsCell` must be added. |
| `docs/MOUSE.md` | Maybe | If user-facing, document the new clickable surfaces (header shortcuts, session tabs, device pill). |

---

## Consolidated Issues

### 🔴 Critical Issues (Must Fix)

None.

### 🟠 Major Issues (Should Fix)

1. **[Source: code_quality_inspector] Stale TODO referencing shipped Task 02 in `mouse_regions.rs:324`**
   - **File:** `crates/fdemon-app/src/mouse_regions.rs:324`
   - **Problem:** Test `click_left_middle_binds_both_buttons` middle-binding asserts `Message::CloseCurrentSession` with a `// TODO: switch to Message::CloseSessionAt(0) when Task 02 lands.` comment. Task 02 has shipped — `Message::CloseSessionAt` exists at `message.rs:239` and is used in production at `tabs.rs:142`. The test now misrepresents what the production code emits.
   - **Recommended Action:** Replace `Message::CloseCurrentSession` with `Message::CloseSessionAt(0)` and remove the TODO.

2. **[Source: architecture_enforcer, code_quality_inspector] Dead helper `to_mouse_rect` with stale comment**
   - **File:** `crates/fdemon-tui/src/widgets/mod.rs:43-47`
   - **Problem:** `to_mouse_rect` is `#[allow(dead_code)]` with a comment claiming "Task 07 will add the call site" — Task 07 has shipped and uses `MouseRect::new(...)` directly. Suppression is hiding a legitimate dead-code warning.
   - **Recommended Action:** Delete the helper. If Phase 4 needs it, re-add in 5 lines.

3. **[Source: architecture_enforcer] `docs/REVIEW_FOCUS.md` "Current usage" missing `AppState::mouse_regions`**
   - **File:** `docs/REVIEW_FOCUS.md`
   - **Problem:** The doc explicitly states "New `Cell`-based render-hint fields require explicit review and documentation here." A second exception (`MouseRegionsCell`) was added but not registered in the doc.
   - **Recommended Action:** Add a bullet under "Current usage" naming `AppState::mouse_regions` and pointing to its renderer (writes per frame) and consumer (`handle_press` reads for hit-test).

4. **[Source: logic_reasoning_checker, risks_tradeoffs_analyzer] TASKS.md narrative contradicts implementation regarding Settings-mode regions**
   - **File:** `workflow/plans/features/mouse-support/phase-3-region-registry/TASKS.md:172`
   - **Problem:** Plan claims "Settings mode does not render the header, so header regions are not in the registry." Reality: regions ARE recorded; the dispatcher (`handler/mouse/mod.rs:54-58`) is the actual gate that drops non-Normal-mode clicks. Net behavior is correct, but a future maintainer reading the plan will be confused or, worse, propose changes based on the false premise.
   - **Recommended Action:** Update TASKS.md to describe the actual gating mechanism (per-mode dispatcher arm), or move the gate to render-time so the doc becomes true. The probe test name `view_header_regions_present_in_settings_mode_because_header_always_renders` already documents reality — propagate that wording back.

### 🟡 Minor Issues (Consider Fixing)

5. **[Source: code_quality_inspector] Magic literal `4` in `register_shortcut_clicks` rect math**
   - **File:** `crates/fdemon-tui/src/widgets/header.rs:159`
   - **Suggestion:** Add `const SHORTCUT_SEGMENT_PREFIX: u16 = 4; // '[' + key + ']' + ' '` next to `SHORTCUT_CLICK_WIDTH` and use it in the formula.

6. **[Source: security_reviewer] Inconsistent saturating arithmetic in `register_shortcut_clicks` overflow guard**
   - **File:** `crates/fdemon-tui/src/widgets/header.rs:163`
   - **Suggestion:** Replace `click_x + SHORTCUT_CLICK_WIDTH > area.x + area.width` with `click_x.saturating_add(SHORTCUT_CLICK_WIDTH) > area.x.saturating_add(area.width)` to match the `cursor_x.saturating_add(...)` style on the line above.

7. **[Source: code_quality_inspector] `padded_area.height.max(1)` hides empty-rect guard in tabs**
   - **File:** `crates/fdemon-tui/src/widgets/tabs.rs:138`
   - **Suggestion:** Drop `.max(1)` and let `click_left_middle`'s built-in `is_empty` check handle zero-height. Optionally add an early `if padded_area.height == 0 { return; }` for clarity.

8. **[Source: code_quality_inspector] `*msg.clone()` in `MouseAction::resolve` is roundabout**
   - **File:** `crates/fdemon-app/src/mouse_regions.rs:87`
   - **Suggestion:** `(**msg).clone()` is the canonical form (clone the inner value, not the Box).

9. **[Source: code_quality_inspector] Missing doc comment on `handle_scroll`**
   - **File:** `crates/fdemon-app/src/handler/mouse/normal.rs:75`
   - **Suggestion:** `pub(super)` items deserve `///` docs per project standards. Mirror the level of detail in `handle_press`.

10. **[Source: logic_reasoning_checker] `tag_filter_visible` gate should live in the dispatcher, not the per-mode handler**
    - **File:** `crates/fdemon-app/src/handler/mouse/normal.rs:33-35`
    - **Suggestion:** Lift the early-return into `handler/mouse/mod.rs::handle_press`. Phase 4/5 will add per-mode handlers; placing the check at the dispatcher level prevents each from forgetting it.

11. **[Source: risks_tradeoffs_analyzer] Cell `take`/`set` is not panic-safe — RAII guard recommended before Phase 4**
    - **Files:** `crates/fdemon-tui/src/render/mod.rs:108-336`, `crates/fdemon-app/src/handler/mouse/normal.rs:44-54`
    - **Suggestion:** A `MouseRegionGuard` that puts the registry back on `Drop`. Phase 4 grows the take/set surface area substantially; locking down panic-safety now is cheaper than retrofitting later.

12. **[Source: risks_tradeoffs_analyzer] Under-tested call sites of `SessionManager::remove_session`**
    - **File:** `crates/fdemon-app/src/session_manager.rs`
    - **Suggestion:** Add tests covering (a) remove non-selected pre-selected — selection follows id; (b) `evict_oldest_stopped` path; (c) failed-spawn removal does not jolt the user's selection.

13. **[Source: risks_tradeoffs_analyzer] `MouseRegionsCell::Debug` doc comment doesn't match implementation**
    - **File:** `crates/fdemon-app/src/mouse_regions.rs` (Debug impl)
    - **Suggestion:** Either fix the comment to say "shows only the type name" (matching `finish_non_exhaustive`) or include a length field.

14. **[Source: risks_tradeoffs_analyzer] Annotate Task 07 completion summary with reconciliation audit trail**
    - **File:** `workflow/plans/features/mouse-support/phase-3-region-registry/tasks/07-tabs-and-device-pill-regions.md`
    - **Suggestion:** Note explicitly that only `tabs.rs` + a small `header.rs` wiring delta landed; the first implementor's `render_main_header`/`TitleRowHints` rewrite was discarded in favor of Task 06's version.

15. **[Source: code_quality_inspector] `TODO(phase-5)` doc comments in render tests may drift**
    - **File:** `crates/fdemon-tui/src/render/tests.rs:59,105,156`
    - **Suggestion:** Move the Phase-5 update notes from outer doc comments to inline comments next to the asserted counts (`len() == 6`, `len() == 3`) so future updates are colocated with the fragile assertion.

---

## Review Checklist

- [x] **Architecture Compliance**: Layer boundaries preserved; `fdemon-app` has zero `ratatui` imports; TEA exception annotated everywhere
- [⚠️] **Code Quality**: Solid core; stale TODOs and dead helper need cleanup
- [x] **Logical Consistency**: Hit-test semantics, take/put-back invariants, removal logic all traced correct
- [x] **Security**: No vulnerabilities; minor saturating-arithmetic hardening recommended
- [⚠️] **Risk Mitigation**: Panic-safety of Cell pattern not addressed; under-tested removal callers
- [x] **Testing Coverage**: 5,131 tests pass; Phase 3-specific suites cover all stated success criteria
- [⚠️] **Documentation**: Public API documented; `handle_scroll` lacks doc; module-level docs strong
- [⚠️] **Doc Freshness**: `ARCHITECTURE.md` and `REVIEW_FOCUS.md` need updates

---

## Actionable Items

### Required for Approval

1. [ ] **Fix stale TODO test assertion**
   - Files: `crates/fdemon-app/src/mouse_regions.rs:324`
   - Details: Replace `Message::CloseCurrentSession` with `Message::CloseSessionAt(0)`; remove TODO comment.

2. [ ] **Remove dead `to_mouse_rect` helper**
   - Files: `crates/fdemon-tui/src/widgets/mod.rs:34-47`
   - Details: Delete the function and the `#[allow(dead_code)]`. Phase 4 can re-add if needed.

3. [ ] **Update `docs/REVIEW_FOCUS.md` "Current usage"**
   - Files: `docs/REVIEW_FOCUS.md` (Approved TEA Exception section)
   - Details: Add a bullet for `AppState::mouse_regions: MouseRegionsCell` per the doc's own registration rule.

4. [ ] **Reconcile TASKS.md narrative with implementation**
   - Files: `workflow/plans/features/mouse-support/phase-3-region-registry/TASKS.md:172`
   - Details: Update the "header regions silently dropped in Settings" note to describe the dispatcher-level gate (`handler/mouse/mod.rs:54-58`).

### Recommended Improvements

5. [ ] **Lift `tag_filter_visible` check into the dispatcher**
   - Rationale: Phase 4/5 per-mode handlers will inherit the gate without remembering it.

6. [ ] **Extract `SHORTCUT_SEGMENT_PREFIX = 4` constant**
   - Rationale: Eliminates magic literal; CODE_STANDARDS Principle 4.

7. [ ] **Use `saturating_add` consistently in shortcut-clicks overflow guard**
   - Rationale: Matches the `saturating_add` style on the preceding line; eliminates u16 overflow at extreme widths.

8. [ ] **Drop `padded_area.height.max(1)` in tabs registration**
   - Rationale: Hides the natural empty-rect guard in `click_left_middle`.

9. [ ] **Document `handle_scroll` and the `EmitWithCoord` closure invariant**
   - Rationale: Public-API doc parity with `handle_press`.

10. [ ] **Add a `MouseRegionGuard` RAII wrapper around take/put-back (before Phase 4)**
    - Rationale: A single widget panic currently can leave the registry empty. Phase 4 expands the surface area substantially.

11. [ ] **Backfill `SessionManager::remove_session` tests for new selected_index branch**
    - Rationale: Three call sites (`close_session_internal`, `evict_oldest_stopped`, `handle_session_spawn_failed`) all rely on the new clamp/decrement logic; only one test exercises it.

12. [ ] **Update `docs/ARCHITECTURE.md` Module Reference for `mouse_regions` and `MouseCtx`**
    - Rationale: New cross-crate threading pattern not yet documented.

---

## Conclusion

**Final Assessment:** Phase 3 is functionally complete and mechanically correct. All 8 tasks merged, all quality gates green (fmt, check, clippy `-D warnings`, 5,131 tests passing). The architecture-level design (registry in app, MouseCtx in tui, Cell exception, z-index hit-test) is sound and well-suited to absorb Phase 4's larger surface area. The verdict of NEEDS WORK is driven by polish debt (stale TODO, dead helper, doc drift) rather than correctness — none of the findings indicate user-facing regressions.

**Next Steps:**
1. Address the 4 "Required for Approval" items (all are mechanical, ~30 minutes total).
2. Schedule "Recommended Improvements" 10 (RAII guard) and 11 (remove_session tests) before Phase 4 dispatch — these become harder to fix as the registry surface area grows.
3. Defer items 5–9 and 12 to a Phase 3.5 polish pass or fold into the first Phase 4 task.

**Blocking Issues Count:** 0 critical / 4 major
**Re-review Required:** No — addressing the major items can land as a follow-up commit on the same branch without re-orchestration.
