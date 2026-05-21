# Action Items: DevTools Inspector Parity — Phase 1

**Review Date:** 2026-05-18
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 4 critical + 8 major

See `REVIEW.md` for full context.

---

## Critical Issues (Must Fix Before Phase 1 Is Accepted)

### C1. `expanded_groups` is never wired to user input — flagship chain demo unreachable

- **Source:** risks_tradeoffs_analyzer, logic_reasoning_checker
- **Files:**
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:154-160` (`InspectorNav::Expand`)
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:459-465` (`handle_inspector_toggle_node`)
- **Problem:** A folded chain leader (`RowGroup::LeaderCollapsed`) is rendered as collapsed iff `expanded_groups.contains(leader_id)`. The only production mutation of `expanded_groups` is `InspectorState::reset()`'s `.clear()`. `Right` key, `Enter` key, and mouse glyph click all write to `inspector.expanded` (the regular expand set) — never to `expanded_groups`. Net effect: a folded BlocProvider chain cannot be expanded by any user action. The whole feature demo doesn't work; tests pass because they manipulate `expanded_groups` directly in setup.
- **Required Action:**
  1. In the selected-row handler for `InspectorNav::Expand` / `InspectorNav::Collapse`, branch on the selected `InspectorRow.group`:
     - `RowGroup::LeaderCollapsed { leader_id, .. }` on Expand → `expanded_groups.insert(leader_id)`
     - `RowGroup::LeaderExpanded { leader_id }` on Collapse → `expanded_groups.remove(&leader_id)`
     - Otherwise fall through to the existing `expanded` set logic.
  2. Apply the same branch in `handle_inspector_toggle_node` (mouse glyph click).
  3. Add a `selected_row()` helper on `InspectorState` that returns the active `InspectorRow` from `inspector_rows()`, since the handlers currently only know `selected_index`.
- **Acceptance:**
  - New test: `expand_on_leader_collapsed_inserts_into_expanded_groups`
  - New test: `collapse_on_leader_expanded_removes_from_expanded_groups`
  - New test: `mouse_toggle_on_leader_glyph_mutates_expanded_groups_not_expanded`
  - Manual: open a Flutter app with `MultiBlocProvider`, see "+N more widgets" leader, press Right, see chain unfold.

---

### C2. Stale `details_open` / `details_node_id` / `expanded_groups` after refresh and hot-restart

- **Source:** logic_reasoning_checker
- **Files:**
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:21-78` (`handle_widget_tree_fetched`)
  - `crates/fdemon-app/src/handler/update.rs:222-244` (`SessionRestartCompleted`)
- **Problem:** `handle_widget_tree_fetched` clears `selected_index/expanded/layout/last_fetched_node_id` but does not touch `details_open`, `details_node_id`, `details_tab`, or `expanded_groups`. `SessionRestartCompleted` only flips `has_ever_rendered_tree`. Consequences: pressing `r` while Details is open leaves the Details panel open with `details_node_id` pointing at a value_id that no longer exists in the new tree (`selected_index = 0` after fetch, so the Properties tab renders for the new root with a stale snapshot). Hot restart has the same issue with a freed Dart object id.
- **Required Action:**
  - In `handle_widget_tree_fetched`, after the existing field resets, add: `inspector.details_open = false; inspector.details_node_id = None; inspector.details_tab = DetailsTab::Properties; inspector.expanded_groups.clear(); inspector.properties.clear(); inspector.properties_loading = false; inspector.properties_error = None;`
  - Apply the equivalent reset in `SessionRestartCompleted` (or call a shared `inspector.reset_details_and_groups()` helper from both sites).
- **Acceptance:**
  - New test: `widget_tree_fetched_clears_details_state_when_details_was_open`
  - New test: `session_restart_completed_clears_details_state`
  - Decide and document whether `expanded_groups` should survive refresh (matches `expanded` semantics — currently cleared on refresh).

---

### C3. `branch_x = 0` sentinel collides with valid x=0 coordinate

- **Source:** code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs:226-238`
- **Problem:**
  ```rust
  let branch_x = match tree_inner.x.checked_add(branch_col) {
      Some(x) if x < tree_inner.right() => x,
      _ => 0, // out of bounds — will be skipped below
  };
  // ...
  if branch_x > 0 && branch_x < tree_inner.right() { ... }
  ```
  The sentinel `0` is also a legitimate column. When `tree_inner.x == 0` and a depth-1 child has `branch_col == 0`, `branch_x` is `0` and the guard suppresses the branch tick render. Today the surrounding block guarantees `tree_inner.x >= 1` so the defect is latent; remove it before borderless layouts or wider test buffers expose it.
- **Required Action:** Replace with `Option<u16>`:
  ```rust
  let branch_x: Option<u16> = tree_inner.x.checked_add(branch_col)
      .filter(|&x| x < tree_inner.right());
  if let Some(bx) = branch_x {
      // draw ch1 at bx, ch2 at bx+1
  }
  ```
- **Acceptance:**
  - Existing tests pass.
  - New test: a tree rendered into a buffer with `tree_inner.x == 0` shows the branch tick at column 0 for depth-1 children.

---

### C4. Guideline `│` off-by-one in tick depth math

- **Source:** logic_reasoning_checker
- **Files:**
  - `crates/fdemon-core/src/widget_tree.rs:419-421` (`open_ticks.push(depth)`)
  - `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs:205-218` (renderer loop)
- **Problem:** Algorithm pushes `open_ticks.push(depth)` for a non-last node at depth N. Renderer iterates `for d in 0..row.depth` and draws `│` at `glyph_col(d)` when `d` is in ticks. The connecting `│` for descendants ends up at `glyph_col(N)`, which is the same column where the branch tick already renders (it overwrites at line 244). Result: the guideline line that should connect a parent's branch tick down to its grandchildren is invisible.
- **Required Action:** Either:
  - **(a)** Change `open_ticks.push(depth)` → `open_ticks.push(depth.saturating_sub(1))` in `widget_tree.rs:420`; **or**
  - **(b)** Change the renderer loop to check `row.ticks.contains(&(d + 1))` at `tree_panel.rs:211`.
  - Strengthen `tree_renders_guidelines_for_nonlast_sibling_ancestors` in `tests.rs` to assert the `│` is at the **exact column** expected (e.g. `glyph_col(0) == 0` for a depth-1 ancestor), not just that some `│` appears anywhere in the row.
- **Acceptance:**
  - Render a 3-level tree (root, two children of root, one grandchild of the non-last root child). Assert grandchild row's guideline `│` is at the same column as the parent's branch tick.

---

## Major Issues (Should Fix Before Merge)

### M1. Delete `get_selected_value_id`, consolidate on `InspectorState::selected_value_id()`

- **Source:** architecture_enforcer, code_quality_inspector
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:208` (definition) + call sites at lines 65, 225, 284
- **Problem:** Task 02 added `InspectorState::selected_value_id()` and noted task 05 should migrate the call sites. Task 05 shipped without doing it. Two parallel implementations (one uses `visible_nodes()`, the other `inspector_rows()`) are a future maintenance hazard.
- **Suggested Action:** Replace the 3 call sites with `state.devtools_view_state.inspector.selected_value_id()`. Delete the private function. No test changes needed.

### M2. Remove `_visible` placeholder parameter from `render_tree_panel_inner`

- **Source:** architecture_enforcer, code_quality_inspector, risks_tradeoffs_analyzer
- **Files:** `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs:154`, callers in `inspector/mod.rs:156, 237`, test helpers in `tests.rs`
- **Problem:** Task 07 added the parameter as a "task 09 will remove it" placeholder. Task 09 closed without removing it. It is now dead code and forces every caller (including all test helpers) to construct a throwaway `Vec`.
- **Suggested Action:** Drop the parameter, update the two production callers and the test helpers in `tests.rs`. Single PR-local change.

### M3. Move `save_settings()` off the input thread

- **Source:** risks_tradeoffs_analyzer, security_reviewer
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:585`
- **Problem:** Every Shift+H keystroke triggers a synchronous TOML serialize + atomic-rename on the TUI event-loop thread. On slow filesystems this stalls the render. Holding the key down generates back-to-back disk writes.
- **Suggested Action:** Introduce `UpdateAction::PersistSettings { settings }` so the engine handles the write off the input thread, OR debounce in-place with a 500ms idle write. Apply the same pattern to `settings_handlers.rs:173` which has the same flaw.

### M4. Chain count badge mismatches expanded length

- **Source:** logic_reasoning_checker
- **Files:** `crates/fdemon-core/src/widget_tree.rs:475-541` (`emit_chain_members`) vs `586-627` (`count_visible_chain_subordinates`)
- **Problem:** `count_visible_chain_subordinates` stops counting at the first un-expanded node; `emit_chain_members` continues walking as long as the chain shape holds. So `+1 more` may unfold into three rows.
- **Suggested Action:** Make `emit_chain_members` honour the `expanded` set the same way the counter does. Add a property test asserting `count_visible_chain_subordinates(node) == emit_chain_members(node).len()` for random trees (cheap to write with `proptest` or even a hand-rolled fuzz).

### M5. Memoize `inspector_rows()` per frame

- **Source:** risks_tradeoffs_analyzer
- **Files:** `inspector/mod.rs:156`, `tree_panel.rs:175`, `details/mod.rs:95`, plus handlers
- **Problem:** Each render frame calls `inspector_rows()` 3–4× (tree visible-count, tree-panel render, details panel via `visible_nodes()`). Each call rebuilds the whole row vector and calls `count_visible_chain_subordinates` per implementation node — O(n·k) where k is chain depth.
- **Suggested Action:** Cache the row list keyed on a revision counter on `InspectorState` that increments when `root`, `expanded`, `expanded_groups`, or `hide_implementation_widgets` mutate. Even a single-frame `OnceCell` cleared at the top of `render_impl` would collapse the redundant builds.

### M6. Remove stale `#[allow(dead_code)]` annotations in `details/*`

- **Source:** code_quality_inspector, risks_tradeoffs_analyzer
- **Files:** `details/properties_tab.rs:24, 29, 40, 82`; `details/render_object_tab.rs:13, 18`; `details/flex_explorer_tab.rs:13, 18`
- **Problem:** Task 09 wired the call sites; the annotations are now suppressing genuine clippy warnings rather than scaffolding deferred wiring.
- **Suggested Action:** Strip every `#[allow(dead_code)]` in those three files and verify `cargo clippy --workspace -- -D warnings` passes. Delete anything that becomes a real warning.

### M7. Sanitise VM Service strings before terminal rendering

- **Source:** security_reviewer
- **Files:** `tree_panel.rs:295-328`, `layout_panel.rs:137-158` (callers); ideally fix at the deserialize boundary in `fdemon-core/widget_tree.rs`
- **Problem:** `node.display_name()` / `creation_location.file` strings come from the Dart VM Service over WebSocket. No ANSI sanitisation; an ESC byte in a widget name would reach `buf.set_string`. Ratatui's crossterm backend incidentally escapes it but the protection is implicit and not asserted anywhere.
- **Suggested Action:** Apply `strip_ansi_codes()` (already in `fdemon-core/ansi.rs`, used by the daemon layer for logs) at `DiagnosticsNode` deserialization — either via a custom serde deserializer on the string fields, or in the accessor helpers (`display_name()`, `source_path()`).

### M8. Mouse click on `LeaderCollapsed` glyph silently mutates wrong set

- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/devtools/inspector.rs:433-468` (`handle_inspector_toggle_node`)
- **Problem:** Clicking a chain leader's glyph adds the leader's `value_id` to `inspector.expanded` (because the leader has children). Renderer ignores `expanded` for `LeaderCollapsed` rows — it consults `expanded_groups`. No-op, no error, no log.
- **Suggested Action:** Same fix as C1: branch on `RowGroup` in `handle_inspector_toggle_node` and route to `expanded_groups` for leaders. Test: `mouse_glyph_click_on_collapsed_leader_inserts_into_expanded_groups`.

---

## Minor Issues (Fix Soon — Suggested as Follow-Up Tasks)

| # | Issue | Suggested Action |
|---|-------|------------------|
| m1 | `widget_tree.rs` is 1,650 lines | File a follow-up task to split into `widget_tree/{diagnostics_node, inspector_row, row_builder}.rs`. |
| m2 | `details_tab` reset on open contradicts close-doc | Pick one behaviour. Recommended: drop the "preserved" comment and document the reset-to-Properties policy. |
| m3 | Narrow terminal + details_open silently invisible | Either render details over tree as a full-width overlay, or auto-close details with a toast. |
| m4 | `Message::ExitDevToolsMode` variant name misleading | Rename to `DevToolsEscape`, or add a doc comment at the definition explaining the dual behaviour. |
| m5 | `walk_node` `is_member` dead parameter + unreachable `RowGroup::Member` arm | Remove the parameter and the arm. |
| m6 | KEYBINDINGS.md ↔ `keys.rs:633-638` comment disagree about Up/Down in details mode | Update the keys.rs comment to reflect that the handler swallows nav while details_open. |
| m7 | `save_settings()` uses fixed temp filename | Use `tempfile::Builder::new().tempfile_in(&fdemon_dir)`. |
| m8 | `count_visible_chain_subordinates` is `pub` | Demote to `pub(crate)`. Drop from `fdemon-core/lib.rs:96-100` re-exports. |
| m9 | `walk_node` / `visible_node_count` recursion uncapped | Add `if depth > 512 { return; }` guard with a doc comment about the serde JSON recursion limit. |
| m10 | `group: group.clone()` in row push | Move `group` instead of clone. |
| m11 | `details/mod.rs:95-98` re-collects `visible` into `refs` | Pass `&visible` directly. |
| m12 | `_tab` misleading underscore prefix on used binding | Rename to `tab`. |
| m13 | Six copies of `collect_buf_text` test helper | Extract into a shared test helper module under `widgets/devtools/inspector/`. |
| m14 | Two single-slash `/` doc lines | `properties_tab.rs:28` → `///`; `tree_panel.rs:152` → `//`. |

---

## Nitpicks

- n1: Task 11 completion summary still says "Not Started" — update for traceability.
- n2: Downgrade the `save_settings()` info-level log to `debug!` or log only the filename.
- n3: Replace `unwrap_or("") + !is_empty()` with `is_some_and(...)` in `widget_tree.rs:378`.
- n4: Add `handle_open_details_is_no_op_when_already_open` test.
- n5: Two stub tabs duplicate `render_centered_text` — consolidate or accept (Phase 2 deletes both).

---

## Re-review Checklist

Before re-running this review, the following must hold:

- [ ] C1 fixed: `expanded_groups` is mutated by `InspectorNav::Expand/Collapse` and `handle_inspector_toggle_node` based on `RowGroup`, with new wired tests.
- [ ] C2 fixed: `handle_widget_tree_fetched` and `SessionRestartCompleted` clear `details_open`, `details_node_id`, `details_tab`, `expanded_groups`, and the properties caches, with regression tests for both paths.
- [ ] C3 fixed: `branch_x = 0` sentinel replaced with `Option<u16>`; test asserts tick at column 0 works.
- [ ] C4 fixed: guideline `│` renders at the correct column; test asserts exact column rather than substring.
- [ ] M1 fixed: `get_selected_value_id` deleted; all callers use `InspectorState::selected_value_id()`.
- [ ] M2 fixed: `_visible` parameter removed from `render_tree_panel_inner` and all callers.
- [ ] M3 fixed: `save_settings()` no longer runs synchronously in the handler.
- [ ] M4 fixed: chain count and unfold length agree; property test added.
- [ ] M5 addressed (or explicitly deferred to Phase 2 with a follow-up task).
- [ ] M6 fixed: `#[allow(dead_code)]` removed from `details/*` modules; clippy still green.
- [ ] M7 fixed: ANSI sanitisation applied to VM Service strings.
- [ ] M8 fixed: chain-leader mouse click writes to `expanded_groups`; test added.
- [ ] Verification commands pass (see `docs/DEVELOPMENT.md`):
  - [ ] `cargo fmt --all -- --check`
  - [ ] `cargo check --workspace --all-targets`
  - [ ] `cargo test --workspace`
  - [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Minor / nitpick items either fixed in this PR or filed as standalone follow-up tasks.
