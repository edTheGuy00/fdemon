# Phase 1.5 — Inspector Parity Fixes & Cleanup

## TL;DR

Remediation phase sitting between Phase 1 (merged) and Phase 2 (not started). Resolves the 4 critical correctness bugs the review flagged as blocking, the 8 major cleanups deferred from Phase 1, and selected minor items where the cost of fixing is small and the cost of carrying forward is high. Inputs: `workflow/reviews/features/devtools-inspector-parity-phase-1/REVIEW.md` + `ACTION_ITEMS.md`.

---

## Background

The Phase 1 implementation landed across 11 tasks with a green workspace test suite. A multi-agent code review (see `workflow/reviews/features/devtools-inspector-parity-phase-1/`) flagged:

- **4 CRITICAL correctness issues** (C1–C4): the flagship "BlocProvider chain" demo cannot be expanded by user input because `expanded_groups` is never mutated outside `reset()`; details state survives `r` refresh + hot-restart pointing at freed Dart objects; and two separate rendering bugs in the tree-panel produce visibly broken guidelines and branch ticks.
- **8 MAJOR deferred items** (M1–M8): stale duplicates and placeholder parameters from Phase 1 that were planned to be cleaned up but never were; performance hot spots; missing ANSI sanitisation; synchronous I/O in the TEA handler.
- **14 MINOR items** (m1–m14): doc drift, dead code, small Rust idiom fixes.

Phase 1 cannot be claimed complete until the criticals land. The majors and minors are bundled to keep Phase 2 from inheriting them.

---

## Affected Modules

- `crates/fdemon-core/src/widget_tree.rs` — chain folding correctness (C4, M4), depth cap (m9), dead parameter removal (m5), small idiom fixes (m10), ANSI sanitisation (M7)
- `crates/fdemon-core/src/lib.rs` — re-export demotion (m8)
- `crates/fdemon-core/src/ansi.rs` — read-only; consume `strip_ansi_codes()`
- `crates/fdemon-app/src/state.rs` — `selected_row()` helper (C1 prerequisite)
- `crates/fdemon-app/src/handler/mod.rs` — `UpdateAction::PersistSettings` variant (M3 infra)
- `crates/fdemon-app/src/actions/mod.rs` — `PersistSettings` dispatch (M3 infra)
- `crates/fdemon-app/src/handler/devtools/inspector.rs` — `expanded_groups` wiring (C1, M8), delete duplicate (M1), state reset on refresh/restart (C2 part 1), async save_settings (M3 consume), open/close detail policy fix (m2)
- `crates/fdemon-app/src/handler/update.rs` — `SessionRestartCompleted` state reset (C2 part 2)
- `crates/fdemon-app/src/handler/settings_handlers.rs` — async save_settings (M3 consume)
- `crates/fdemon-app/src/handler/keys.rs` — comment drift fix (m6 part 1)
- `crates/fdemon-app/src/config/settings.rs` — unique temp filename (m7)
- `crates/fdemon-app/src/message.rs` — variant rename `ExitDevToolsMode → DevToolsEscape` (m4)
- `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs` — `Option<u16>` sentinel (C3), drop `_visible` (M2)
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` — single per-frame `inspector_rows()` build (M5 in-frame consolidation), drop `_visible` (M2)
- `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs` — strengthened column assertions, drop `_visible` from helpers, shared `collect_buf_text` (M13)
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/{flex_explorer_tab,render_object_tab,properties_tab,mod}.rs` — strip `#[allow(dead_code)]` (M6), `_tab` rename (m12), single-slash doc fix (m14), refs re-collect cleanup (m11)
- `docs/ARCHITECTURE.md` — note on the `selected_row()` helper (doc_maintainer)
- `docs/KEYBINDINGS.md` — Up/Down doc drift fix (m6 part 2)

---

## Development Phases

This is a single phase (Phase 1.5). No further sub-phasing.

### Phase 1.5: Inspector Parity Fixes

**Goal:** Land all 4 CRITICALs + 8 MAJORs + the bundled MINORs in a single coordinated effort, restoring Phase 1 to a shippable state.

**Duration:** ~10–14 hours estimated.

#### Steps (mapped to tasks)

1. **Foundations** (Wave 1, 4 parallel):
   - Add `InspectorState::selected_row()` helper (task 01).
   - Add `UpdateAction::PersistSettings` variant + dispatch (task 02).
   - Strip stale `#[allow(dead_code)]` + minor cosmetics in `details/*` (task 03).
   - Tree rendering correctness: C3 sentinel + C4 off-by-one + M4 chain count + m5/m8/m9/m10 cleanups (task 04).

2. **Hardening** (Wave 2, 2 parallel — different files):
   - Sanitise VM Service strings at deserialize boundary (task 05).
   - Wire `expanded_groups` to nav + mouse + delete `get_selected_value_id` (task 06).

3. **Lifecycle reset** (Wave 3, sequential — same file as 06):
   - Reset details + groups state on `handle_widget_tree_fetched` and `SessionRestartCompleted` (task 07).

4. **Async persistence** (Wave 4, sequential — same file as 07):
   - Switch all `save_settings(...)` handler call sites to `UpdateAction::PersistSettings` (task 08).

5. **Render-time consolidation + symbol renames** (Wave 5, sequential — same files as 04):
   - Drop `_visible` placeholder, build `inspector_rows()` once per frame in `render_impl`, rename `ExitDevToolsMode → DevToolsEscape`, extract shared `collect_buf_text` test helper (task 09).

6. **Docs** (Wave 6, doc_maintainer):
   - Update `docs/ARCHITECTURE.md` and `docs/KEYBINDINGS.md` (task 10).

**Milestone:** Phase 1's flagship demo (BlocProvider chain expand via `Right`) works end-to-end on a real Flutter app. All review CRITICALs + MAJORs closed. Workspace quality gate green.

---

## Edge Cases & Risks

### Risk: Long handler-side sequential chain on `handler/devtools/inspector.rs`

- **Risk:** Tasks 06 → 07 → 08 all touch the same file and must run sequentially. Wall-clock bottleneck.
- **Mitigation:** Each task scoped tightly to its own region of the file (06 = nav/toggle/value-id handlers; 07 = lifecycle reset helper + call sites; 08 = `handle_toggle_hide_implementation` + sibling `handle_settings_*` consumers). Implementors should read the full review before starting to anticipate the next task's scope.

### Risk: Task 04 is large (4 files, 8 review items)

- **Risk:** Aggregating C3 + C4 + M4 + m5 + m8 + m9 + m10 in one task increases coordination complexity.
- **Mitigation:** All 8 items live in `widget_tree.rs` / `tree_panel.rs` and are correlated (most are inside the chain-folding algorithm or its immediate consumers). Atomic landing avoids a half-fixed renderer. If the implementor finds the task too large, it may be split into 04a (correctness: C3 + C4 + M4) and 04b (cleanups: m5 + m8 + m9 + m10).

### Risk: Chain count fix (M4) may invalidate existing chain-count tests

- **Risk:** Making `emit_chain_members` honour the `expanded` set changes what counts as "subordinate" in `count_visible_chain_subordinates` parity check.
- **Mitigation:** The property test added by task 04 (`count_visible_chain_subordinates(node) == emit_chain_members(node).len()` for random expanded/collapsed mixes) defines the contract. Existing unit tests may need re-baselining; the implementor will note any test updates explicitly.

### Risk: `Message::ExitDevToolsMode → DevToolsEscape` rename touches many files

- **Risk:** Many call sites referencing the variant name.
- **Mitigation:** Mechanical rename. The implementor uses `cargo check` after each rename batch to ensure no missed sites. No semantic change.

### Risk: ANSI sanitisation (M7) may be too aggressive

- **Risk:** If `strip_ansi_codes()` removes legitimate characters from a widget description (e.g. backslash sequences), users see distorted widget names.
- **Mitigation:** The function is already used in the daemon log layer with confidence (`fdemon-daemon/src/protocol.rs:380`). Same trust boundary applies here. Apply at deserialize boundary so all rendered uses are protected; if downstream features need raw values, expose a `_raw` field.

### Risk: Async settings persistence (M3) loses errors

- **Risk:** A failed `save_settings` running on a background tokio task can't propagate the error inline.
- **Mitigation:** Follow `AutoSaveConfig` precedent — emit a `Message::SettingsPersistFailed { error }` (or similar) handled at the engine level to surface a warning toast/log entry. Task 02 defines this handshake; task 08 consumes it.

---

## Configuration Additions

None.

---

## Keyboard Shortcuts Summary

No new keys. Phase 1.5 adjusts the **behaviour** of existing keys:

| Key | Mode | Behaviour change |
|-----|------|-----------------|
| `Right` / `l` / `Enter` | Inspector tree, on a `LeaderCollapsed` row | Now expands the chain via `expanded_groups.insert(leader_id)` (was: silently mutated `expanded`) |
| `Left` / `h` | Inspector tree, on a `LeaderExpanded` row | Now collapses the chain via `expanded_groups.remove(&leader_id)` |
| Mouse glyph click | Inspector tree, on a chain leader | Same branching as keys above |
| `r` (refresh) / hot-restart | Inspector tab, while details open | Now closes details + clears `expanded_groups` (was: stale `details_node_id`) |

---

## Success Criteria

### Phase 1.5 Complete When:

- [ ] **C1 closed:** `InspectorNav::Expand/Collapse` and `handle_inspector_toggle_node` branch on `RowGroup::LeaderCollapsed`/`LeaderExpanded` to mutate `expanded_groups`. Manual test on a real Flutter `MultiBlocProvider` app: the `+N more widgets` row expands and collapses correctly.
- [ ] **C2 closed:** `handle_widget_tree_fetched` and `SessionRestartCompleted` invoke `inspector.reset_details_and_groups()`; new regression tests verify both paths clear `details_open`, `details_node_id`, `details_tab`, `expanded_groups`, and the `properties_*` cache fields.
- [ ] **C3 closed:** `branch_x = 0` sentinel replaced with `Option<u16>`; new test asserts a depth-1 child renders its branch tick at exactly column 0 when `tree_inner.x == 0`.
- [ ] **C4 closed:** Guideline `│` renders at the correct column (parent's branch-tick column, not the child's branch-tick column); test strengthened to assert exact column rather than substring.
- [ ] **M1 closed:** `get_selected_value_id` deleted; 3 call sites use `InspectorState::selected_value_id()`.
- [ ] **M2 closed:** `_visible` parameter removed from `render_tree_panel_inner` and all callers/test helpers.
- [ ] **M3 closed:** No synchronous `save_settings(...)` calls remain in any handler; all go through `UpdateAction::PersistSettings`. Failure path emits a `Message` surfaceable to the user.
- [ ] **M4 closed:** Property test (or table-driven test) asserts `count_visible_chain_subordinates(node) == emit_chain_members(node).len()` for both expanded and collapsed sub-chains.
- [ ] **M5 closed:** Each render of the Inspector tab calls `inspector_rows()` exactly once; threaded through `render_tree_panel_inner` + `render_details_panel` via a shared parameter.
- [ ] **M6 closed:** No `#[allow(dead_code)]` remains in `details/*`. Clippy with `-D warnings` still green.
- [ ] **M7 closed:** `description` and `creation_location.{file,name}` strings on `DiagnosticsNode` pass through `strip_ansi_codes()` at deserialize time. Test with an injected ESC byte verifies the cell rendering is clean.
- [ ] **M8 closed:** Mouse click on a `LeaderCollapsed` glyph mutates `expanded_groups` (same branch as C1). New test added.
- [ ] **Bundled minors (m2, m4, m5, m6, m7, m8, m9, m10, m11, m12, m13, m14, n1, n2):** each fixed and verified by test or by `cargo clippy/check`.
- [ ] **Quality gate green:** `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.

---

## Out of Scope (deferred or won't fix)

| Review item | Status |
|------------|--------|
| m1 — split `widget_tree.rs` into submodules (~1,650 lines) | **Deferred** to a standalone follow-up task pre-Phase-2. File is the most-touched in this phase; splitting now compounds merge risk. |
| m3 — narrow-terminal details fallback (panel renders over tree or auto-closes) | **Deferred.** UX decision needed; not blocking. |
| n3 — `unwrap_or("") + !is_empty()` → `is_some_and()` | **Won't fix.** Pure style preference; current form is correct and readable. |
| n4 — add `handle_open_details_is_no_op_when_already_open` test | **Won't fix in 1.5.** Coverage gap acknowledged; existing early-return guard is correct. May be added in a separate "test coverage" pass. |
| n5 — `render_centered_text` duplicated in stub tabs | **Won't fix.** Phase 2 replaces both stub bodies; consolidation now is wasted work. |

---

## References

- Phase 1 plan: `workflow/plans/features/devtools-inspector-parity/PLAN.md`
- Phase 1 tasks: `workflow/plans/features/devtools-inspector-parity/phase-1/TASKS.md`
- Phase 1 review: `workflow/reviews/features/devtools-inspector-parity-phase-1/REVIEW.md`
- Phase 1 action items: `workflow/reviews/features/devtools-inspector-parity-phase-1/ACTION_ITEMS.md`
- Architecture: `docs/ARCHITECTURE.md`
- Code standards: `docs/CODE_STANDARDS.md`
- Review focus: `docs/REVIEW_FOCUS.md`
