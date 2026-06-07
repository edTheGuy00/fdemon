# Install Wizard Informational Re-open — Task Index

## Overview

Fix the Install Wizard so a user-invoked (`I`) re-open on a healthy toolchain shows a read-only
informational view (all components green, "All set" hint) instead of auto-advancing into the
new-session dialog. The post-install handback to device discovery is restricted to a
**Bootstrap**-origin wizard via a new explicit `WizardOrigin` enum. (Option 1 — strict: a
`UserInvoked` wizard never hands back.)

See [BUG.md](BUG.md) for the full root-cause analysis and behaviour matrix.

**Total Tasks:** 3
**Estimated Hours:** 5–8 hours

## Task Dependency Graph

```
┌───────────────────────────────┐
│  01-core-origin-fix           │   (fdemon-app + runner.rs; atomic compile unit)
└───────────────┬───────────────┘
                │
       ┌────────┴─────────┐
       ▼                  ▼
┌──────────────┐   ┌──────────────────┐
│ 02-tui-hint  │   │ 03-doc-arch       │
│ (fdemon-tui) │   │ (doc_maintainer)  │
└──────────────┘   └──────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-core-origin-fix](tasks/01-core-origin-fix.md) | Not Started | - | 3-5h | `fdemon-app/install_wizard`, `handler`, `state.rs`, `message.rs`, `fdemon-tui/runner.rs` |
| 2 | [02-tui-header-hint](tasks/02-tui-header-hint.md) | Not Started | 1 | 1-2h | `fdemon-tui/widgets/install_wizard` |
| 3 | [03-doc-architecture](tasks/03-doc-architecture.md) | Not Started | 1 | 0.5-1h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-core-origin-fix | `crates/fdemon-app/src/install_wizard/types.rs`, `crates/fdemon-app/src/install_wizard/mod.rs`, `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/install_wizard/navigation.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-tui/src/runner.rs`, `docs/KEYBINDINGS.md` | `crates/fdemon-app/src/session_manager.rs` (`has_running_sessions`), `crates/fdemon-daemon/src/toolchain/types.rs` (`ComponentStatus`) |
| 02-tui-header-hint | `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (if header lives there) | `crates/fdemon-app/src/install_wizard/state.rs` (read `origin`, `all_components_ok`) |
| 03-doc-architecture | `docs/ARCHITECTURE.md` | task 01 files for change context |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 02 | None (01 touches `runner.rs`; 02 touches `widgets/install_wizard/*`) | Sequential by dependency (02 → 01) |
| 01 + 03 | None | Sequential by dependency (03 → 01) |
| 02 + 03 | None | Parallel (worktree) — both depend only on 01 |

> **Note:** Task 01 is a single atomic compile unit. The `Message::ShowInstallWizard { origin }`
> variant change, the `show_install_wizard(origin)` / `handle_show(state, origin)` signature
> changes, and `InstallWizardState::opening(origin)` all ripple across `message.rs`, `keys.rs`,
> `runner.rs`, `update.rs`, `navigation.rs`, and ~25 in-crate test call sites simultaneously —
> the workspace will not compile until all are updated together. Do not attempt to split task 01
> into parallel sub-tasks across these files.

## Success Criteria

The fix is complete when:

- [ ] Pressing `I` on a healthy toolchain (with or without a running session) opens the wizard
      read-only: all components Ok, **no** new-session dialog, `Esc` returns to `UiMode::Normal`.
- [ ] A fresh-machine startup (no Flutter) still opens the wizard in `Bootstrap` origin, and after
      a successful install hands back to device discovery (`UiMode::Startup` + `DiscoverDevices`).
- [ ] `close_wizard_and_dispatch_discovery` only dispatches discovery when origin is `Bootstrap`
      and no session is running.
- [ ] An informational, all-Ok wizard shows an "All set — press Esc to return" header.
- [ ] All existing Phase-5 handback tests are updated to open `Bootstrap` and still pass; new
      informational tests pass.
- [ ] `cargo test --workspace` is green; `cargo clippy --workspace` and `cargo fmt --all` clean.
- [ ] `docs/KEYBINDINGS.md` and `docs/ARCHITECTURE.md` reflect the origin-gated behaviour.

## Keyboard Shortcuts

| Key | Mode | Action |
|-----|------|--------|
| `I` | Normal | Open Install Wizard (informational when toolchain is healthy) |
| `Esc` | InstallWizard | Close — returns to Normal for a `UserInvoked` wizard |

## Notes

- Decision: **Option 1 (strict)** — `UserInvoked` never auto-hands-back. A user who installs via
  a `UserInvoked` wizard presses `Esc` to return to Normal (no new-session dialog).
- `WizardOrigin::default()` is `UserInvoked` (the safe, no-handback default).
- Defensive guard: handback additionally skipped when `session_manager.has_running_sessions()`.
