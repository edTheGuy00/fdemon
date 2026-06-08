## Task: Expand/collapse interactivity — message, toggle handler, key routing, Esc tiering

**Objective**: Make the Platforms submenu interactive. Add an `InstallWizardToggleExpand` message, a
`handle_toggle_expand` handler that flips `platforms_expanded` and rebuilds the projected step list,
route `Enter`-on-parent to the toggle (and `Enter`-on-leaf/other to the existing run), and make `Esc`
collapse an expanded submenu before closing — clamping `selected_index` back onto a visible row.

**Depends on**: Task 01 (enum + `platforms_expanded` field + `build_steps(report, expanded)` exist).

**Agent:** implementor

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs` — add `InstallWizardToggleExpand`.
- `crates/fdemon-app/src/handler/mod.rs` — dispatch the new message; rename leftover `AndroidTools`
  doc-comment mentions on `RunWizardStep` → `PlatformAndroid`.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` — `handle_toggle_expand`; Esc-collapse
  tiering in `handle_escape`.
- `crates/fdemon-app/src/handler/keys.rs` — `Enter`-conditional routing (parent → toggle); optional
  `l`/`h`/arrows.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/{types,state}.rs` — `WizardStepKind::Platforms`, `platforms_expanded`,
  `build_steps`, `selected_step()`.

### Details

> Locate by symbol/test-name; line numbers are approximate.

#### 1. Message (`message.rs`)

Add (no payload — the handler reads `selected_step().kind`):

```rust
/// Toggle expand/collapse of the Platforms submenu parent row.
/// No-op unless the selected step is the `Platforms` parent.
InstallWizardToggleExpand,
```

#### 2. Dispatch (`handler/mod.rs`)

In the `update()` match, near `Message::InstallWizardUp =>`:

```rust
Message::InstallWizardToggleExpand => install_wizard::handle_toggle_expand(state),
```

Also rename the `AndroidTools` mentions in the `RunWizardStep` doc-comments to `PlatformAndroid` (cosmetic).

#### 3. Toggle handler (`navigation.rs`)

```rust
pub fn handle_toggle_expand(state: &mut AppState) -> UpdateResult {
    let wiz = &mut state.install_wizard_state;
    let is_parent = wiz.selected_step().map(|s| s.kind == WizardStepKind::Platforms).unwrap_or(false);
    if !is_parent {
        return UpdateResult::none();
    }
    wiz.platforms_expanded = !wiz.platforms_expanded;
    if let Some(report) = wiz.report.as_ref().cloned() {
        wiz.steps = build_steps(&report, wiz.platforms_expanded);
    }
    // Cursor stays on the parent row (index unchanged); clamp defensively.
    if wiz.selected_index >= wiz.steps.len() {
        wiz.selected_index = wiz.steps.len().saturating_sub(1);
    }
    wiz.selected_command_index = 0;
    UpdateResult::none()
}
```

(The parent is at a fixed index, so expanding inserts leaves *after* it and the cursor remains on the
parent — good UX: the user expands, then presses `j` to descend into the leaves.)

#### 4. Esc tiering (`navigation.rs` `handle_escape`)

Current `handle_escape` delegates to `maybe_dispatch_discovery_on_close`. Make it tiered: **if expanded,
collapse first and return**; otherwise fall through to the existing close path. (The running-step Esc→cancel
tier is handled upstream in `keys.rs` via `CancelStep`; keep that ordering — cancel takes priority over
collapse, which takes priority over close.)

```rust
pub fn handle_escape(state: &mut AppState) -> UpdateResult {
    if state.install_wizard_state.platforms_expanded {
        state.install_wizard_state.platforms_expanded = false;
        if let Some(report) = state.install_wizard_state.report.as_ref().cloned() {
            state.install_wizard_state.steps = build_steps(&report, false);
        }
        let len = state.install_wizard_state.steps.len();
        if state.install_wizard_state.selected_index >= len {
            state.install_wizard_state.selected_index = len.saturating_sub(1);
        }
        return UpdateResult::none();
    }
    maybe_dispatch_discovery_on_close(state)
}
```

#### 5. Key routing (`keys.rs` `handle_key_install_wizard`)

Change the `Enter` arm to dispatch conditionally on the selected kind (the handler has `&AppState`):

```rust
InputKey::Enter => {
    let on_parent = state.install_wizard_state.selected_step()
        .map(|s| s.kind == WizardStepKind::Platforms).unwrap_or(false);
    Some(if on_parent { Message::InstallWizardToggleExpand }
         else { Message::InstallWizardRunSelectedStep })
}
```

Optionally add `l`/`Right` and `h`/`Left` → `InstallWizardToggleExpand` (additive; no conflicts). Keep the
existing `Esc` mapping (`CancelStep` when running, else `InstallWizardEscape`); `handle_escape` now does the
collapse-vs-close tiering internally.

### Acceptance Criteria

1. `Enter` on the `Platforms` parent toggles `platforms_expanded`; `state.steps` gains/loses the host-gated
   leaves; the cursor remains on the parent row.
2. `Enter` on a leaf or any non-parent step still dispatches `InstallWizardRunSelectedStep` (Android leaf
   installs; placeholder leaves show "Available in a later phase").
3. With the submenu expanded, `Esc` collapses (does **not** close) and clamps `selected_index`; a second
   `Esc` closes (or hands back, per origin).
4. A running step's `Esc` still cancels first (unchanged priority).
5. `cargo test --workspace --lib` green; fmt + clippy clean.

### Testing

Add handler tests in `navigation.rs` / `keys.rs`:
- `toggle_expand_on_parent_inserts_leaves` / `…collapse_removes_leaves` (assert `steps.len()` grows/shrinks
  and leaf kinds present/absent).
- `toggle_expand_noop_when_not_on_parent`.
- `esc_collapses_expanded_submenu_then_closes` (two-step).
- `esc_collapse_clamps_selected_index` (cursor on a leaf, collapse snaps it back).
- `enter_on_platforms_parent_emits_toggle` vs `enter_on_leaf_emits_run`.

```bash
cargo test -p fdemon-app --lib handler::install_wizard::navigation
cargo test -p fdemon-app --lib keys
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

### Notes

- `build_steps` must be in scope in `navigation.rs` (it is `pub`).
- Do not touch rendering here (Task 03). This task is logic-only; the caret/indent/height come from 03 but
  read the same `platforms_expanded`/`indent` state, so 02 and 03 are independent and parallelizable.
- Keep the run-step Esc→cancel behaviour exactly as-is.

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
