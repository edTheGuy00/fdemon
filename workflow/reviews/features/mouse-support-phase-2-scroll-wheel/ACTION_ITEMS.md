# Action Items: Mouse Support — Phase 2 (Scroll Wheel)

**Review Date:** 2026-05-03
**Verdict:** ⚠️ APPROVED WITH CONCERNS
**Blocking Issues:** 0
**Recommended for Phase 3 prep:** 3 Major + 9 Minor

---

## Critical Issues (Must Fix)

None — workspace builds clean and 4109+ tests pass.

---

## Major Issues (Should Fix Before Phase 3)

### 1. Inspector modifier-handling diverges from `is_shift_only()` discipline

- **Source:** `logic_reasoning_checker`
- **File:** `crates/fdemon-app/src/handler/mouse/devtools.rs:25`
- **Problem:** The guard `if !mods.shift && (mods.ctrl || mods.alt) { return None; }` allows `Shift+Ctrl+wheel` and `Shift+Alt+wheel` to produce `InspectorNav::Up/Down`. Every other handler (`normal.rs`, `link_highlight.rs`, `devtools.rs::handle_network_scroll`) returns `None` for those combos via the `is_shift_only()` discipline declared in `TASKS.md:142-144`. The "small UX win" rationale in the inline comment is unverified, undocumented at the plan level, and untested.
- **Required Action:** Pick one of:
  - **(a) Align to peers (recommended):** Replace the guard with `if mods.ctrl || mods.alt { return None; }`. Add a test asserting `Shift+Ctrl+wheel` → `None` in Inspector.
  - **(b) Document divergence:** Add a Notes entry in `TASKS.md` justifying why Inspector accepts `Shift+modifier` combos that other modes reject. Add a test asserting `Shift+Ctrl+wheel` → `Some(InspectorNavigate(...))`.
- **Acceptance:** A test in `devtools.rs` exercises Inspector with `KeyModSet::new(true, true, false)` and the assertion matches the chosen rule.

### 2. Settings vs NewSession dart-defines Edit-pane behavior is silently divergent

- **Source:** `architecture_enforcer`, `risks_tradeoffs_analyzer`, `code_quality_inspector`
- **Files:**
  - `crates/fdemon-app/src/handler/mouse/settings.rs:21` — Edit pane returns `None`
  - `crates/fdemon-app/src/handler/mouse/new_session.rs:24-32` — both panes route Up/Down
- **Problem:** Both surfaces show structurally identical dart-defines modals. Settings swallows scroll while editing (safer, doesn't move list under cursor); NewSession routes scroll regardless of pane (mirrors `keys.rs:851-855`). The asymmetry mirrors a pre-existing keyboard divergence and is "correct" in that narrow sense, but it is a real UX inconsistency. Only `new_session.rs` documents the rationale; `settings.rs` makes the opposite choice silently.
- **Required Action:** Pick one of:
  - **(a) Reconcile to safer Settings policy (recommended):** Change `new_session.rs:24-32` to gate Edit pane to `None`, matching Settings. Update the keyboard handler at `keys.rs:851-855` in a follow-up bug task.
  - **(b) Document divergence:** Add a cross-reference comment at `settings.rs:21` like:
    ```rust
    // Note: NewSession dart-defines modal routes Up/Down in both panes
    // (mirrors keys.rs:851-855). Settings differs intentionally — see
    // new_session.rs:25-32 for rationale. Mirror keys.rs:747-768.
    DartDefinesPane::Edit => None,
    ```
- **Acceptance:** Either both surfaces behave identically OR a code reader can find the divergence rationale from either side.

### 3. `docs/MOUSE.md` is load-bearing but missing

- **Source:** `risks_tradeoffs_analyzer`
- **File:** `docs/MOUSE.md` (does not exist; planned for Phase 6)
- **Problem:** Three medium risks are mitigated only by user docs that are scheduled for Phase 6:
  - Win11 Shift-mod drop (crossterm #986) → silently degrades to plain wheel
  - Modifier-handling asymmetry across modes → discoverability tax
  - Coordinate-free routing → user surprise when scrolling outside the log area still scrolls the log
- **Required Action:** Create a stub `docs/MOUSE.md` with one paragraph each:
  - Per-mode modifier table (which modes honor Shift, which ignore it)
  - "Scroll is global per `UiMode` regardless of cursor position" (deferred coordinate gating)
  - Win11 Shift caveat
- **Acceptance:** `docs/MOUSE.md` exists, is linked from `docs/CONFIGURATION.md`'s `enable_mouse` row, and covers the three risks above.

---

## Minor Issues (Consider Fixing)

1. **Add doc comment to `mod.rs::handle_scroll`** [Source: code_quality_inspector]
   - Even though `fn` (not `pub fn`), it is the central dispatcher and warrants a one-sentence `///` doc explaining per-mode dispatch.

2. **Comment ignored `_mods` parameters** [Source: code_quality_inspector]
   - `flutter_version.rs:12` and `new_session.rs:12` silently use `_`-prefixed params with no inline justification. One-line comment per function.

3. **Update `devtools.rs` module doc to match Inspector Shift behavior** [Source: code_quality_inspector]
   - Module doc says "Inspector → tree row navigation (Up/Down only; no page step)" but Shift+wheel still produces single-step navigation. Either fix the implementation (Major #1) or update the doc.

4. **Document `assert_scroll_routes_to` discriminant limitation** [Source: logic_reasoning_checker]
   - `handler/tests.rs:10227-10235` uses `std::mem::discriminant`. Currently safe because data-carrying tests use `matches!` directly, but a footgun for future test additions. Add a comment to the helper noting the limitation.

5. **Add Settings + NewSessionDialog positive assertions in `mod.rs::tests`** [Source: logic_reasoning_checker]
   - The four existing positive-assertion tests cover Normal, DevTools, LinkHighlight, FlutterVersion. A dispatcher-arm typo for Settings or NewSessionDialog would be caught only by integration tests.
   - Add `test_scroll_settings_routes_to_settings_prev_item` and `test_scroll_new_session_dialog_routes_to_device_up`.

6. **Strengthen Network filter-inactive integration test** [Source: logic_reasoning_checker]
   - `mouse_scroll_devtools_network_plain_up_produces_network_navigate_up` exercises the no-session `unwrap_or(false)` path, not the with-session-and-filter-inactive path. Attach a session for stronger assertion.

7. **Consider extracting shared scroll pattern** [Source: code_quality_inspector]
   - 12-line shared logic between `normal.rs` and `link_highlight.rs` (Shift→Page, Ctrl/Alt→None, plain→Scroll). Could be a private helper in `mod.rs`. Judgment call.

8. **Rename non-conforming tests** [Source: code_quality_inspector]
   - `normal.rs` tests (`plain_wheel_up_scrolls_up`, `shift_wheel_up_pages_up`, etc.) and `link_highlight.rs` tests (`plain_wheel_scrolls`, `shift_wheel_pages`) don't follow `test_<function>_<scenario>_<expected_result>` per `REVIEW_FOCUS.md`.

9. **Add `EmulatorSelector` to no-op test sweep** [Source: security_reviewer, logic_reasoning_checker]
   - `test_scroll_no_op_in_non_scrollable_modes` covers `EmulatorSelector` via the dispatcher's match arm but no test directly asserts it. One line in the existing test array.

10. **Add `scroll_during_reload` test** [Source: risks_tradeoffs_analyzer]
    - The plan claims scroll is safe during reload/restart (no `is_busy` gate). No test exercises this. Add one assertion that `update(state_with_busy_session, Message::Mouse(Scroll{..}))` still produces the expected message.

11. **Hoist `test_device()` helper** [Source: code_quality_inspector]
    - Duplicated between `devtools.rs` and `handler/tests.rs`. Could move to a shared `test_helpers` module.

---

## Process Feedback (Planning, Not Code)

### Shared-test-array surprise during orchestration

**Source:** `risks_tradeoffs_analyzer`, orchestrator merge log

The plan's File Overlap Analysis declared Wave 2 fully parallelizable (zero shared write files). In practice, all five Wave-2 implementors edited `mod.rs` to remove their `UiMode` from a shared `test_scroll_no_op_in_every_mode` array, producing 4 merge conflicts.

**Lesson for future planning:** Treat any test array enumerating an `enum` (or any other open type being progressively handled) as a shared-write surface during incremental rollouts. Two options:

1. **Shift ownership:** Have the integration-tests task (e.g., Task 07 in this phase) own the shared-array update. Per-mode tasks leave the array untouched; their per-submodule positive-assertion tests provide coverage during their own wave.

2. **Restructure the test:** Make the no-op array *derived* rather than enumerated — e.g., each submodule declares `pub(super) const HANDLES: &[UiMode] = &[...]` and the dispatcher test computes `UiMode::ALL - union(submodules)`. Over-engineered for 6 files but illustrates the pattern.

The simpler fix is option 1.

---

## Re-review Checklist

After addressing Major issues, the following must pass:

- [ ] Major #1 — Inspector modifier rule chosen, documented, and tested
- [ ] Major #2 — Dart-defines Edit-pane behavior reconciled or cross-referenced
- [ ] Major #3 — `docs/MOUSE.md` stub exists and covers the three deferred risks
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
