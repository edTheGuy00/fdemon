## Task: Wizard Handler Fixes + Daemon-Type Re-exports (fdemon-app)

**Objective**: Add the missing re-entrancy guard to the preflight re-run handler, remove the dead
`_effective` binding, re-export the four daemon display types through `fdemon-app` so the TUI no
longer needs a direct `fdemon-daemon` runtime dependency, and register the new `Cell` render-hint in
`REVIEW_FOCUS.md`. Addresses review findings **M3**, **m6**, **m4 (app side)**, **m9**.

**Depends on**: None (all changes are additive or local; compiles standalone)

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — re-run guard (M3).
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` — remove dead binding (m6).
- `crates/fdemon-app/src/install_wizard/mod.rs` — re-export daemon display types (m4 app side).
- `docs/REVIEW_FOCUS.md` — register the new render-hint (m9). *(Implementor-editable doc.)*

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `InstallWizardState`, `loading` field.
- `fdemon_daemon::toolchain` — the four types being re-exported.

### Details

**M3 — re-entrancy guard** (`actions.rs:24-35`, `handle_rerun_preflight`):

The handler unconditionally sets `loading = true` and returns `RunToolchainPreflight`. Add an
early-return when a preflight is already in flight, so mashing `r` cannot spawn N concurrent
preflights:

```rust
pub fn handle_rerun_preflight(state: &mut AppState) -> UpdateResult {
    // Already running — ignore the re-run request (prevents stacking concurrent
    // preflight tasks, each of which spawns `flutter doctor`).
    if state.install_wizard_state.loading {
        return UpdateResult::none();
    }
    state.install_wizard_state.loading = true;
    state.install_wizard_state.status_message = None;
    // ... unchanged: clone paths, return RunToolchainPreflight
}
```

**m6 — remove dead `_effective` binding** (`navigation.rs`, the `WizardPane::Detail` branch of
`handle_down`):

The branch computes `_effective` from `last_known_visible_height` and discards it, then advances
`detail_scroll` by 1 unconditionally. Choose one:
- **Preferred:** delete the `_effective` computation and keep the unconditional
  `detail_scroll = detail_scroll.saturating_add(1)`, with a one-line comment that the upper-bound
  clamp is intentionally applied at render time (the renderer already clamps via
  `compute_corrected_scroll`).
- *(Alternative, only if cheap):* actually consume the hint + selected-step content length to clamp
  the increment in the handler. Not required for Phase 1; the dead binding is the defect.

Do not leave a computed-but-unused binding.

**m4 (app side) — re-export display types** (`install_wizard/mod.rs`):

So `fdemon-tui` can drop its direct `fdemon-daemon` runtime dependency (task 05), re-export the
display-only types the widgets need:

```rust
// Re-export the daemon toolchain *display* types so presentation-layer widgets can
// consume them without a direct fdemon-tui -> fdemon-daemon dependency.
pub use fdemon_daemon::toolchain::{ComponentCheck, ComponentStatus, DoctorLine, DoctorMarker};
```

Keep the existing `pub use state::*; pub use types::*;`. This is purely additive.

**m9 — register the render-hint** (`REVIEW_FOCUS.md`, "Approved TEA Exception → Current usage"):

Add a bullet:

> - `InstallWizardState::last_known_visible_height` — the renderer writes the detail-pane content
>   height each frame; the `handle_down` handler / render-time clamp use it to keep the detail view
>   in range. Default 0 (safe fallback when no render has happened yet). Write site annotated in
>   `widgets/install_wizard/step_detail.rs`.

### Acceptance Criteria

1. `handle_rerun_preflight` returns `UpdateResult::none()` (no action, no state change beyond the
   no-op) when `state.install_wizard_state.loading` is already `true`.
2. `navigation.rs` contains no `_effective` (or other computed-but-unused) binding in `handle_down`.
3. `fdemon_app::install_wizard::{ComponentCheck, ComponentStatus, DoctorLine, DoctorMarker}` resolve
   (re-exported); the crate compiles standalone.
4. `REVIEW_FOCUS.md` lists `InstallWizardState::last_known_visible_height` under current Cell usage.
5. Existing install-wizard handler tests pass; quality gate green.

### Testing

```rust
#[test] fn test_rerun_preflight_noops_when_already_loading() {
    // loading = true -> handle_rerun_preflight returns no action, stays loading
}
#[test] fn test_rerun_preflight_spawns_when_idle() {
    // loading = false -> returns RunToolchainPreflight (existing behavior preserved)
}
```

- Keep the existing `test_rerun_preflight_sets_loading_and_returns_action` passing for the idle case
  (it constructs an applied report first, so `loading == false`).

### Notes

- The re-export is the **prerequisite** for task 05 dropping the `fdemon-daemon` runtime dep from
  `fdemon-tui/Cargo.toml`. Land it here first.
- Do not touch the TUI imports or `fdemon-tui/Cargo.toml` — that is task 05.
