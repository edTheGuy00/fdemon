# Feature Review: Mouse Support — Phase 2 (Scroll Wheel)

**Review Date:** 2026-05-03
**Reviewer:** Code Review Orchestrator
**Task Files Reviewed:** 7 tasks (Phase 2 wave 1–3)
**Files Changed:** 10 files (+1421 / -110 lines)
**Diff Base:** `e009afb` → HEAD (`767751d`)

---

## Executive Summary

**Overall Verdict:** ⚠️ APPROVED WITH CONCERNS

Phase 2 successfully replaces the no-op `handle_mouse` with per-`UiMode` scroll routing. Architecture, security, and dispatcher mechanics are all clean; the implementation faithfully mirrors the keyboard handler in five of six modes and reuses every existing scroll/nav `Message` variant as the plan required. One real logic divergence (Inspector's modifier-handling rule), one cross-handler inconsistency (Settings vs NewSession dart-defines Edit-pane behavior), and several documentation/test-coverage gaps should be tracked before Phase 3 lands. None are blocking — the workspace gate (fmt + check + test + clippy) is fully green.

---

## Changes Overview

### Task Files Reviewed

| Task | Status | Files Modified |
|------|--------|----------------|
| `01-mouse-handler-restructure.md` | Done | 8 (1 deleted, 7 created) + `input_mouse.rs` |
| `02-normal-mode-scroll.md` | Done | `mouse/normal.rs` + `mouse/mod.rs` |
| `03-devtools-mode-scroll.md` | Done | `mouse/devtools.rs` + `mouse/mod.rs` |
| `04-settings-mode-scroll.md` | Done | `mouse/settings.rs` + `mouse/mod.rs` |
| `05-new-session-dialog-scroll.md` | Done | `mouse/new_session.rs` + `mouse/mod.rs` |
| `06-simple-modes-scroll.md` | Done | `mouse/link_highlight.rs`, `mouse/flutter_version.rs` + `mouse/mod.rs` |
| `07-update-integration-tests.md` | Done | `handler/tests.rs` |

### Files Changed

```
crates/fdemon-app/src/handler/mouse.rs                    | 110 -------    DELETED
crates/fdemon-app/src/handler/mouse/devtools.rs           | 213 +++++++++++++
crates/fdemon-app/src/handler/mouse/flutter_version.rs    |  68 +++++
crates/fdemon-app/src/handler/mouse/link_highlight.rs     |  81 +++++
crates/fdemon-app/src/handler/mouse/mod.rs                | 171 ++++++++++
crates/fdemon-app/src/handler/mouse/new_session.rs        | 190 +++++++++++
crates/fdemon-app/src/handler/mouse/normal.rs             | 138 ++++++++
crates/fdemon-app/src/handler/mouse/settings.rs           | 193 ++++++++++++
crates/fdemon-app/src/handler/tests.rs                    | 350 +++++++++++++++
crates/fdemon-app/src/input_mouse.rs                      |  17 +
                                                  total: 10 files, +1421/-110
```

---

## Subagent Review Summaries

### Architecture Enforcer
**Verdict:** ✅ PASS

All ten files reside within `fdemon-app`. Layer boundaries respected (no `tui` or `daemon` cross-imports outside of `#[cfg(test)]` device fixtures). The `handler/mouse/` directory split mirrors the established `handler/devtools/` pattern. TEA purity is preserved — every `handle_scroll` is a pure `(state, input) -> Option<Message>` function with no `&mut AppState` argument. The `KeyModSet::is_shift_only()` addition is the only new public item and is correctly used by ≥3 modes.

**Key Findings:**
- Module decomposition mirrors `handler/devtools/` cleanly
- Dispatcher pattern matches `handle_key`/`handler/keys.rs` shape
- 0 critical violations, 1 warning (settings/new_session asymmetry documentation), 2 nitpick suggestions

### Code Quality Inspector
**Verdict:** ⚠️ APPROVED WITH CONCERNS *(corrected from initial NEEDS WORK — see note)*

> **Note:** The agent's primary "MAJOR" finding claimed Task 07 integration tests are missing. This is a false positive — `mod mouse_scroll` exists at `handler/tests.rs:10203` with 15 integration tests driving `update(state, Message::Mouse(...))`. Verified by direct grep. The remaining minor findings are valid.

**Quality Scores:**
| Metric | Score |
|--------|-------|
| Language Idioms | ⭐⭐⭐⭐⭐ |
| Error Handling | ⭐⭐⭐⭐⭐ |
| Testing | ⭐⭐⭐⭐ (integration tests do exist; minor gaps in modifier-axis coverage) |
| Documentation | ⭐⭐⭐ (handle_scroll undocumented; ignored modifiers lack inline justification) |
| Maintainability | ⭐⭐⭐⭐ (small duplication between normal.rs and link_highlight.rs) |

### Logic & Reasoning Checker
**Verdict:** 🟠 CONCERN

Five of six per-mode handlers correctly mirror the keyboard reference. One real inconsistency identified in `devtools.rs::handle_inspector_scroll`: Shift+Ctrl+wheel (and Shift+Alt+wheel) produce `InspectorNav::Up/Down` while every other handler returns `None` for those combos. This is contrary to the `is_shift_only()` discipline stated in TASKS.md and is untested. Modal precedence ordering is correct in Settings and NewSessionDialog; the asymmetric dart-defines Edit-pane behavior between Settings (no-op) and NewSession (routes Up/Down) faithfully mirrors the corresponding keyboard handlers and is correctly implemented and tested.

**Key Findings:**
- Inspector accepts `Shift+Ctrl+wheel` as navigation (asymmetric vs Normal/Network/LinkHighlight)
- `assert_scroll_routes_to` uses `std::mem::discriminant` — fragile for data-carrying variants if future tests use it for `NetworkNav::Up` vs `PageUp`
- mod.rs positive-assertion tests omit Settings and NewSessionDialog (covered only via integration tests)

### Risks & Tradeoffs Analyzer
**Verdict:** ⚠️ CONCERNS

**Identified Risks:**
| Risk | Severity | Mitigated? |
|------|----------|------------|
| Win11 Shift-mod drop (crossterm #986) | Medium | Partial — `docs/MOUSE.md` deferred to Phase 6 |
| Settings vs NewSession dart-defines Edit asymmetry | Medium | No — silently divergent behavior |
| Modifier-handling asymmetry across modes is undiscoverable | Medium | No — no user-facing docs |
| Coordinate-free routing (scroll-anywhere-scrolls-log) | Medium | Partial — explicitly deferred to Phase 3 |
| Inspector "swallow Shift" UX-win claim is unverified | Low | Justified inline only |
| No test for "scroll during reload" claim | Low | No |
| Six submodules for ~50 LOC of logic today | Low | Yes — split is reversible if Phase 3+ doesn't grow them |

### Security Reviewer
**Verdict:** ✅ PASS

Pure UI input routing with no new attack surface. All handlers accept immutable `&AppState` and return `Option<Message>` — zero `UpdateAction` produced (verified by code structure and by `result.action.is_none()` assertions in 15+ integration tests). Coordinate `(x, y)` fields are dropped at dispatch via `..` destructuring, so coordinate-confusion attacks are structurally impossible. `KeyModSet` modifier trust is bounded by type — worst case of a forged terminal sequence is moving a list cursor or paging the log view, equivalent to user keyboard input.

**Security Findings:**
| Finding | Category | Severity |
|---------|----------|----------|
| `EmulatorSelector` not in no-op test sweep | Defense in Depth | Low |
| `flutter_version` ignores all modifiers (consistent with plan) | Input Validation | Low — non-exploitable |

### Documentation Freshness
**Status:** ⚠️ Updates needed

| Doc | Needs Update? | Reason |
|-----|--------------|--------|
| `docs/ARCHITECTURE.md` | Optional | New `handler/mouse/` directory not listed (existing `handler/devtools/` only mentioned in passing — consistent with current convention) |
| `docs/CODE_STANDARDS.md` | No | No new patterns introduced |
| `docs/DEVELOPMENT.md` | No | No new build steps or dependencies |
| `docs/MOUSE.md` | **Yes (deferred)** | Load-bearing for Win11 Shift caveat, modifier asymmetry, coordinate-free routing — currently planned for Phase 6 |

---

## Consolidated Issues

### 🔴 Critical Issues (Must Fix)

None blocking — the workspace builds and all 4109+ tests pass.

### 🟠 Major Issues (Should Fix)

1. **[Source: logic_reasoning_checker] Inspector modifier-handling diverges from `is_shift_only()` discipline**
   - **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs:25`
   - **Problem:** The guard `if !mods.shift && (mods.ctrl || mods.alt) { return None; }` means Shift+Ctrl+wheel and Shift+Alt+wheel produce `DevToolsInspectorNavigate(InspectorNav::Up/Down)` while the same combos return `None` in `normal.rs`, `link_highlight.rs`, and `devtools.rs::handle_network_scroll`. The "small UX win" rationale in the inline comment is unverified and creates an undocumented per-mode inconsistency. Untested.
   - **Recommended Action:** Either (a) replace the guard with `if mods.ctrl || mods.alt { return None; }` to match every other handler, OR (b) document the divergence explicitly in TASKS.md and add a test asserting `Shift+Ctrl+wheel` → `Some(InspectorNavigate(...))` for Inspector.

2. **[Source: architecture_enforcer, risks_tradeoffs_analyzer] Settings vs NewSession dart-defines Edit-pane behavior is silently divergent**
   - **Files:** `crates/fdemon-app/src/handler/mouse/settings.rs:21`, `crates/fdemon-app/src/handler/mouse/new_session.rs:24-32`
   - **Problem:** Both surfaces show structurally identical dart-defines modals, but Settings' Edit pane swallows scroll while NewSession's Edit pane routes Up/Down to list navigation underneath the text cursor. The asymmetry mirrors a pre-existing keyboard divergence (`keys.rs:733-770` vs `keys.rs:839-866`) so it is "correct" in a narrow sense, but it is a real UX inconsistency. The reasoning is documented only in `new_session.rs`; `settings.rs` makes the opposite choice with no cross-reference.
   - **Recommended Action:** Either (a) reconcile to the safer Settings policy (Edit pane swallows scroll for both surfaces; update the keyboard handler at `keys.rs:851-855` in a follow-up), OR (b) add a cross-reference comment in `settings.rs:21` pointing to the divergent rationale in `new_session.rs:25-32`.

3. **[Source: risks_tradeoffs_analyzer] User-facing mouse documentation is load-bearing but deferred**
   - **File:** `docs/MOUSE.md` (does not exist)
   - **Problem:** Three medium risks (Win11 Shift drop, modifier asymmetry, coordinate-free routing) are mitigated only by user docs that are scheduled for Phase 6. Users on `main` between Phase 2 ship and Phase 6 ship have no reference for how mouse interaction differs by mode or platform.
   - **Recommended Action:** Stub `docs/MOUSE.md` now (one paragraph each: per-mode modifier table, "scroll is global per `UiMode` regardless of cursor position", "Win11 Shift drop"). Phase 6 expands it.

### 🟡 Minor Issues (Consider Fixing)

1. **[Source: code_quality_inspector] `mod.rs::handle_scroll` undocumented** — central dispatcher deserves a one-sentence `///` doc explaining per-mode dispatch and the modifier asymmetry note.

2. **[Source: code_quality_inspector] `_mods` ignored without inline justification** in `flutter_version.rs:12` and `new_session.rs:12`. One-line comment per function.

3. **[Source: code_quality_inspector] `devtools.rs` module doc comment** says "Inspector → tree row navigation (Up/Down only; no page step)" but Shift+wheel still produces single-step. Update to match implemented behavior (or fix the implementation per Major #1).

4. **[Source: logic_reasoning_checker] `assert_scroll_routes_to` uses `std::mem::discriminant`** — currently safe (data-carrying tests use `matches!` directly) but a footgun for future test additions. Document the limitation in the helper's comment, or tighten to `assert_eq!` with `PartialEq`.

5. **[Source: logic_reasoning_checker] mod.rs positive-assertion tests omit Settings and NewSessionDialog.** A dispatcher-arm typo for either mode would be caught only by the integration suite, not by mod.rs unit tests. Add `test_scroll_settings_routes_to_settings_prev_item` and `test_scroll_new_session_dialog_routes_to_device_up` to close the gap.

6. **[Source: logic_reasoning_checker] Network filter-inactive integration test exercises the no-session `unwrap_or(false)` path**, not the with-session-and-filter-inactive path. Attaching a session would strengthen the assertion. Unit-level `network_filter_active_swallows_scroll` covers the with-session path.

7. **[Source: code_quality_inspector] Shared 12-line scroll pattern duplicated** between `normal.rs` and `link_highlight.rs`. Could be extracted to a private `log_scroll_message(dir, mods)` helper in `mod.rs`. Judgment call.

8. **[Source: code_quality_inspector] Test naming convention** — several tests in `normal.rs` and `link_highlight.rs` use abbreviated names (`plain_wheel_up_scrolls_up`, `shift_wheel_pages`) that don't follow `test_<function>_<scenario>_<expected_result>` per `REVIEW_FOCUS.md`. The `devtools.rs`/`settings.rs`/`new_session.rs` tests are closer to convention.

9. **[Source: security_reviewer, logic_reasoning_checker] `EmulatorSelector` not in `test_scroll_no_op_in_non_scrollable_modes`** — no exploit risk, but a future routing change would lack a regression catch. Add to the array.

10. **[Source: risks_tradeoffs_analyzer] No test for "scroll during reload" claim.** Add one assertion that `update(state_with_busy_session, Message::Mouse(Scroll{..}))` still produces the expected message.

11. **[Source: code_quality_inspector] `test_device()` helper duplicated** between `devtools.rs` and `handler/tests.rs`. Could be hoisted to a shared `test_helpers` module.

### 🛠 Process Feedback (Planning, Not Code)

**[Source: risks_tradeoffs_analyzer, orchestrator]** All five Wave-2 implementors edited `mod.rs` to remove their `UiMode` from the shared `test_scroll_no_op_in_every_mode` array, producing 4 merge conflicts that were not predicted by the plan's File Overlap Analysis. For future incremental rollouts of an enum-keyed dispatcher, treat any test array enumerating the enum as a shared-write surface, or assign the central-dispatcher test ownership to the integration-tests task (e.g., Task 07 in this phase) and have per-mode tasks leave the array untouched.

---

## Review Checklist

- [x] **Architecture Compliance**: Layer boundaries respected; new `handler/mouse/` mirrors `handler/devtools/` pattern
- [x] **Code Quality**: Idioms clean; minor doc gaps and naming inconsistencies (see Minor #1, #8)
- [⚠] **Logical Consistency**: One genuine asymmetry (Major #1: Inspector modifier handling); mode-by-mode keyboard mirroring is otherwise faithful
- [x] **Security**: No vulnerabilities; trust boundaries preserved
- [⚠] **Risk Mitigation**: Load-bearing user docs deferred to Phase 6 (Major #3)
- [x] **Testing Coverage**: Per-submodule unit tests + 15 integration tests through `update()` (Task 07 verified present)
- [⚠] **Documentation**: `pub` items documented; central `handle_scroll` dispatcher and ignored `_mods` parameters lack inline rationale (Minor #1, #2)
- [⚠] **Doc Freshness**: `docs/MOUSE.md` is load-bearing but missing (Major #3)

---

## Actionable Items

### Required for Approval

None — verdict is APPROVED WITH CONCERNS, not NEEDS WORK. All Major issues should be resolved before Phase 3 lands but are not blocking the current branch.

### Recommended Before Phase 3

1. [ ] **Resolve Inspector modifier inconsistency** (Major #1)
   - Files: `crates/fdemon-app/src/handler/mouse/devtools.rs:25`
   - Details: Pick a uniform Shift+Ctrl/Alt rule and apply it. Add an explicit test for Inspector + `Shift+Ctrl+wheel`.

2. [ ] **Reconcile dart-defines Edit-pane behavior between Settings and NewSession** (Major #2)
   - Files: `crates/fdemon-app/src/handler/mouse/settings.rs:21`, `crates/fdemon-app/src/handler/mouse/new_session.rs:24-32`
   - Details: Either align both to the safer Settings policy, or add a cross-reference comment documenting the intentional divergence.

3. [ ] **Stub `docs/MOUSE.md`** (Major #3)
   - Files: `docs/MOUSE.md` (new)
   - Details: One paragraph each on per-mode modifier table, coordinate-free scroll, Win11 Shift caveat. Phase 6 expands.

### Recommended Improvements

1. [ ] **Add doc comment to `mod.rs::handle_scroll`** (Minor #1)
2. [ ] **Comment ignored `_mods` parameters** in `flutter_version.rs` and `new_session.rs` (Minor #2)
3. [ ] **Update `devtools.rs` module doc** to match implemented Inspector Shift behavior (Minor #3)
4. [ ] **Document `assert_scroll_routes_to` discriminant limitation** (Minor #4)
5. [ ] **Add Settings + NewSessionDialog positive assertions in `mod.rs::tests`** (Minor #5)
6. [ ] **Strengthen Network filter-inactive integration test** with attached session (Minor #6)
7. [ ] **Add `EmulatorSelector` to no-op test sweep** (Minor #9)
8. [ ] **Add `scroll_during_reload` test** locking the safety invariant (Minor #10)
9. [ ] **Update orchestration planning notes** with the shared-test-array lesson (Process Feedback)

---

## Conclusion

**Final Assessment:** Phase 2 is a clean, well-tested implementation of the wheel-routing dispatcher. The architecture and security are solid; the main concerns are a single real logic asymmetry (Inspector modifier handling), a known cross-handler UX inconsistency (Settings vs NewSession dart-defines Edit pane), and documentation gaps (no `docs/MOUSE.md` yet, sparse inline comments on intentional ignored-modifier choices). All quality gates (`cargo fmt --check`, `cargo check --workspace --all-targets`, `cargo test --workspace` — 4109+ tests, `cargo clippy --workspace --all-targets -- -D warnings`) pass.

The merge-resolution work during orchestration produced functional code, but the "every wave-2 task touched `mod.rs`" surprise indicates that the planner's File Overlap Analysis should evolve to flag enum-enumerating test arrays as shared-write surfaces.

**Next Steps:**
1. Address Major #1 (Inspector modifier asymmetry) — pick rule, add test, ~15 min
2. Address Major #2 (dart-defines Edit-pane) — at minimum add cross-reference comment, ~10 min
3. Stub `docs/MOUSE.md` (Major #3) — ~30 min
4. Schedule the recommended Minor improvements as a Phase 2.5 cleanup or absorb into Phase 3 prep

**Blocking Issues Count:** 0
**Re-review Required:** No (concerns are tracked; resolve before Phase 3)
