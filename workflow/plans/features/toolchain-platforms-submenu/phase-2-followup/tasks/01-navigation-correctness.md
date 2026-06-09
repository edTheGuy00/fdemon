## Task: Navigation correctness — directional expand/collapse + shared collapse helper (M1 + M2 + M3 + S3)

**Objective**: Fix the three must-fix navigation defects and the related clone, as one coherent change:
(M1) make `l`/`Right` expand and `h`/`Left` collapse directionally instead of both flipping; (M2) re-anchor
the cursor to the Platforms parent when collapsing away from a leaf row; (M3) reset `selected_command_index`
on every collapse path; (S3) eliminate the per-keystroke `ToolchainReport` clone via a borrow-split. The
unifying mechanism is **one shared `set_platforms_expanded` helper** in `navigation.rs` that every collapse/
expand path routes through, so the two paths can never diverge again.

**Depends on**: None.

**Agent:** implementor

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs` — add `InstallWizardExpand` + `InstallWizardCollapse` variants.
- `crates/fdemon-app/src/handler/update.rs` — dispatch the two new messages.
- `crates/fdemon-app/src/handler/keys.rs` — route `l`/`Right`→Expand, `h`/`Left`→Collapse; fix doc-comment.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` — the shared helper + `handle_expand` /
  `handle_collapse`; rewrite `handle_escape` collapse tier and `handle_toggle_expand` to use the helper.
- `docs/KEYBINDINGS.md` — update the install-wizard key list (implementor-editable).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/{state,types}.rs` — `build_steps` (pub), `WizardStepKind`,
  `WizardStepKind::is_platform_leaf()` (types.rs:69–81). **Do NOT write these files** — the helper lives in
  `navigation.rs`, not on `InstallWizardState`, so this task stays disjoint from Task 02.

### Details

> Line numbers are from a research snapshot and will drift — locate by symbol/test-name.

#### 1. The shared helper (`navigation.rs`) — fixes M2 + M3 + S3

Add a **private free function** in `navigation.rs` (not a method on `InstallWizardState`):

```rust
/// Apply a new `platforms_expanded` value and reconcile the step list + cursor.
///
/// Single source of truth for every expand/collapse transition (toggle, directional,
/// Esc-collapse). Rebuilds the projected step list, re-anchors the cursor to the
/// Platforms parent when collapsing away from a leaf row, clamps `selected_index`,
/// and resets `selected_command_index`.
fn set_platforms_expanded(wiz: &mut InstallWizardState, expanded: bool) {
    // Capture leaf-selection BEFORE the rebuild changes the list under us.
    let was_leaf = wiz
        .selected_step()
        .map(|s| s.kind.is_platform_leaf())
        .unwrap_or(false);

    wiz.platforms_expanded = expanded;

    // Borrow-split (S3): &wiz.report and &mut wiz.steps are disjoint fields, and
    // build_steps returns an owned Vec — no ToolchainReport clone needed.
    if let Some(report) = &wiz.report {
        let steps = build_steps(report, expanded);
        wiz.steps = steps;
    }

    // M2: collapsing away from a leaf returns focus to the parent it descended from.
    if !expanded && was_leaf {
        if let Some(idx) = wiz
            .steps
            .iter()
            .position(|s| s.kind == WizardStepKind::Platforms)
        {
            wiz.selected_index = idx;
        }
    }

    // Defensive bounds clamp.
    if wiz.selected_index >= wiz.steps.len() {
        wiz.selected_index = wiz.steps.len().saturating_sub(1);
    }

    // M3: always reset the guided-command cursor on a structural change.
    wiz.selected_command_index = 0;
}
```

(If the borrow-split fails to compile because `selected_step()` borrows `wiz` immutably across the later
`&mut` use, compute `was_leaf` first in its own statement as shown — it already ends before the mutation.)

#### 2. Rewrite the three handlers to route through the helper

- **`handle_toggle_expand`** (navigation.rs:88–107): keep the `is_parent` guard (Enter on the parent only),
  then `set_platforms_expanded(wiz, !wiz.platforms_expanded);`. Drop the now-duplicated inline rebuild/clamp/
  reset.
- **`handle_escape`** (navigation.rs:61–76): the collapse tier becomes
  `if state.install_wizard_state.platforms_expanded { set_platforms_expanded(&mut state.install_wizard_state, false); return UpdateResult::none(); }` then fall through to `maybe_dispatch_discovery_on_close(state)`.
  This is what gives Esc the M2 re-anchor + M3 reset for free.
- **Add `handle_expand`** (M1): no-op unless the selected step is the `Platforms` parent **and** not already
  expanded; otherwise `set_platforms_expanded(wiz, true)`.
- **Add `handle_collapse`** (M1): if `platforms_expanded`, `set_platforms_expanded(wiz, false)` (works from
  the parent **or** any leaf — the helper re-anchors); otherwise no-op. This is the "back out" key.

#### 3. Messages (`message.rs`)

After `InstallWizardToggleExpand` (message.rs:1766), add:

```rust
/// Expand the Platforms submenu (directional `l`/`Right`). Sets `platforms_expanded = true`.
/// No-op unless the selected step is the collapsed `Platforms` parent.
InstallWizardExpand,
/// Collapse the Platforms submenu (directional `h`/`Left`). Sets `platforms_expanded = false`.
/// No-op unless the submenu is currently expanded; re-anchors the cursor to the parent.
InstallWizardCollapse,
```

`InstallWizardToggleExpand` stays (Enter on the parent still toggles).

#### 4. Dispatch (`handler/update.rs`)

After the `InstallWizardToggleExpand` arm (update.rs:3254), add:

```rust
Message::InstallWizardExpand => install_wizard::handle_expand(state),
Message::InstallWizardCollapse => install_wizard::handle_collapse(state),
```

Ensure `handle_expand`/`handle_collapse` are re-exported through `handler/install_wizard/mod.rs`
(`pub use navigation::*` already covers it).

#### 5. Key routing (`keys.rs`)

Change the directional arms (keys.rs:472–473):

```rust
InputKey::Char('l') | InputKey::Right => Some(Message::InstallWizardExpand),
InputKey::Char('h') | InputKey::Left  => Some(Message::InstallWizardCollapse),
```

Leave the `Enter` arm (keys.rs:458–469) and the `Esc` arm (keys.rs:440–446, `CancelStep` when running else
`InstallWizardEscape`) unchanged — running-step Esc→cancel priority is preserved. Update the function
doc-comment (keys.rs:412–430) so the `l`/`Right` = expand and `h`/`Left` = collapse lines are now accurate
(they currently say "same as Enter on parent; `InstallWizardToggleExpand`").

#### 6. `docs/KEYBINDINGS.md`

Locate the install-wizard key table/section and update the `l`/`Right` and `h`/`Left` rows to read
"expand Platforms submenu" / "collapse Platforms submenu" (distinct from `Enter` = toggle on parent / run).
If no install-wizard section exists, add a short one consistent with the file's existing format. Keep
`Enter`, `Esc` (cancel→collapse→close tiering), `Tab`, `j/k`, `r`, `c`, `[`/`]` accurate.

### Acceptance Criteria

1. `l`/`Right` on a collapsed `Platforms` parent expands it; on an already-expanded parent or any non-parent
   step it is a no-op. `h`/`Left` collapses whenever the submenu is expanded (from parent or leaf), no-op
   otherwise. `Enter` on the parent still toggles; `Enter` elsewhere still runs.
2. Collapsing (via `Esc` or `h`/`Left`) while the cursor is on a platform leaf re-anchors
   `selected_step().kind == WizardStepKind::Platforms` — verified on Linux, macOS, and Windows reports.
3. Every collapse path resets `selected_command_index == 0`.
4. The rebuild/clamp/re-anchor/reset logic exists in exactly one place (`set_platforms_expanded`); both
   `handle_escape` and `handle_toggle_expand` call it; no inline duplicate remains.
5. No `ToolchainReport` clone on the expand/collapse path (no `.as_ref().cloned()` / `report.clone()`).
6. Running-step `Esc`→`InstallWizardCancelStep` priority is unchanged.
7. `navigation.rs` does not write/modify `state.rs`; the helper is local to `navigation.rs`.
8. `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

### Testing

Add/extend handler tests in `navigation.rs` (and `keys.rs` for routing):
- `expand_on_collapsed_parent_inserts_leaves` / `expand_noop_when_already_expanded` /
  `expand_noop_when_not_on_parent`.
- `collapse_from_leaf_reanchors_to_parent` — set cursor on a leaf (use `position(|s| s.kind == PlatformAndroid)`),
  collapse, assert `selected_step().kind == Platforms`. Add per-host variants (Linux/macOS/Windows reports)
  so the in-range-index case (e.g. Linux leaf index 3) is covered, not just the clamped case.
- `collapse_noop_when_already_collapsed`.
- **Tighten** the existing `esc_collapse_clamps_selected_index` to assert the landing `kind == Platforms`,
  not merely `selected_index < len`.
- `esc_collapse_resets_selected_command_index` and `collapse_resets_selected_command_index`.
- `toggle_and_esc_collapse_leave_identical_state` (equivalence — guards against future divergence).
- `enter_on_parent_emits_toggle`, `l_emits_expand`, `h_emits_collapse` (keys.rs routing).

```bash
cargo test -p fdemon-app --lib handler::install_wizard::navigation
cargo test -p fdemon-app --lib keys
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

### Notes

- The helper must take `&mut InstallWizardState` and live in `navigation.rs` — keeping it out of `state.rs`
  is what lets this task run in parallel with Task 02 (rollup cleanup).
- `handle_toggle_expand` toggling from the parent never has a leaf selected, so its re-anchor branch is a
  no-op there — that's fine; the helper is correct in both call contexts.
- Don't touch rendering (Task 03) or `build_steps` internals (Task 02). This task only reads `build_steps`.

---

## Completion Summary

**Status:** _(fill in)_
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
