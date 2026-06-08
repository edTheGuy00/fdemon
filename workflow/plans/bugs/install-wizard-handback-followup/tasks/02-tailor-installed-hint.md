## Task: Tailor the post-install header hint for `UserInvoked` opens (Finding 2)

**Objective**: When a `UserInvoked` wizard reaches an all-Ok report *after* the toolchain was
broken at open, show `Flutter installed — press <key> to start a session` instead of the generic
`All set — press Esc to return`. This removes the dead-end where a user installs Flutter via `I`
and is told only to press `Esc` (which drops them to an empty log view). The strict Option-1
no-handback decision is **kept** — this is a presentation-only affordance change backed by one
latched state flag.

**Depends on**: None (independent of task 01)

**Agent:** implementor

**Estimated Time**: 2–3 hours

> **Atomicity warning.** Adding the `observed_unhealthy` field to `InstallWizardState`
> (fdemon-app) and reading it in the TUI header (fdemon-tui) is a single compile unit. Do not
> split across the two crates.

### Scope

**Files Modified (Write):**

- `crates/fdemon-app/src/install_wizard/state.rs`:
  - Add `pub observed_unhealthy: bool` to `InstallWizardState` (defaults to `false`; reset by
    `opening()` via `..Self::default()` — confirm `opening()` does not set it explicitly).
  - In `apply_report` (~line 198), set `observed_unhealthy = true` (latch — never clear within a
    session) whenever the applied report contains **any** component with status `!= Ok`. Do not
    clear it when a later report is all-Ok; the latch records that the toolchain was broken at some
    point this session.
  - Add a small predicate, e.g. `fn show_installed_hint(&self) -> bool { !self.is_bootstrap() && self.all_components_ok() && self.observed_unhealthy }`, with a doc comment. (Name to taste; keep it terse and documented.)
  - Add co-located unit tests for the latch and the predicate (see Testing).
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`:
  - In `render_header` (~line 136), extend the subtitle selection to three cases:
    - `show_installed_hint()` → `"Flutter installed — press <key> to start a session"`
    - else `!is_bootstrap() && all_components_ok()` → `"All set — press Esc to return"`
    - else → `"Flutter toolchain setup"`
  - Reuse the existing `palette::TEXT_MUTED` style and the em-dash (`\u{2014}`) convention.
  - Add/extend render tests (see Testing).
- `docs/KEYBINDINGS.md` — only if the `I`/`Esc` rows need a note about the installed-hint; keep
  edits minimal. (Implementor-editable doc.)

**Files Read (Dependencies):**

- `crates/fdemon-app/src/handler/keys.rs` and `docs/KEYBINDINGS.md` — confirm the exact key that
  opens a new session / starts a session, and use it verbatim in the hint text. Do not guess.
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentStatus` for the non-Ok check.

### Acceptance Criteria

1. `InstallWizardState` has `observed_unhealthy`, latched to `true` in `apply_report` on any
   non-Ok component, reset on `opening()`.
2. A `UserInvoked` wizard that was broken at open and is now all-Ok shows the
   `Flutter installed — press <key> to start a session` hint.
3. A `UserInvoked` wizard that was healthy throughout shows `All set — press Esc to return`.
4. A `Bootstrap` wizard, any non-Ok/missing component, or the loading state shows
   `Flutter toolchain setup` (no installed/all-set hint).
5. `<key>` in the hint matches the actual new-session keybinding (verified against keys.rs).
6. Rendering does not panic and respects the existing 2-row header layout (`MIN_RENDER_HEIGHT`).
7. Full quality gate passes (`fmt`, `check`, `test`, `clippy`).

### Testing

State-level (`install_wizard/state.rs`):

```rust
#[test]
fn observed_unhealthy_latches_on_non_ok_report() { /* apply partial → true; apply all-ok → still true */ }

#[test]
fn observed_unhealthy_false_when_healthy_throughout() { /* apply all-ok report → false */ }

#[test]
fn opening_resets_observed_unhealthy() { /* latch true, reopen via opening(), assert false */ }
```

Render (`widgets/install_wizard/mod.rs`, using `TestTerminal`):

```rust
#[test]
fn installed_hint_shown_when_user_invoked_was_broken_now_ok() { /* buffer contains "Flutter installed" */ }

#[test]
fn all_set_hint_shown_when_healthy_throughout() { /* buffer contains "All set", not "Flutter installed" */ }
```

Run: `cargo test -p fdemon-app && cargo test -p fdemon-tui`, then `cargo clippy --workspace` and
`cargo fmt --all`.

### Notes

- Pure UX/presentation change plus one latch flag; no handler or handback-gating changes.
- `all_components_ok()` already returns `false` while loading (no report), so the loading case is
  covered without an extra guard.

---

## Completion Summary

**Status:** Not Started
**Branch:** <fill in>

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
