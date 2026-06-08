# Install Wizard Handback — Review Follow-ups

## Overview

Follow-up work for three findings raised in the
[install-wizard-informational-reopen review](../install-wizard-informational-reopen/REVIEW.md).
The `WizardOrigin` bug fix shipped correctly; these items harden the post-install handback
path and close test/UX gaps surfaced during review.

| # | Finding | Severity | Decision |
|---|---------|----------|----------|
| 1 | `flutter_now_live()` / `flutter_executable()` predicate divergence in the handback path | MAJOR (pre-existing) | **Reuse preflight's already-resolved SDK** — eliminate the second `find_flutter_sdk` call so `report Ok ⟺ resolved_sdk Some` always holds |
| 2 | Misleading "All set" hint / dead-end after install-via-`I` on a broken toolchain | MEDIUM | **Tailor the hint text** (keep strict Option-1; no handback) |
| 3 | Missing direct unit tests for `all_components_ok()` and `is_bootstrap()` | MINOR | Add co-located state-level tests |

---

## Finding 1 — Harden the handback against an unresolved SDK

### Root cause (corrected by codebase research)

The review flagged that the auto-close arm tests `flutter_now_live()` (reads
`InstallWizardState::report`) while the actual handback inside
`close_wizard_and_dispatch_discovery` requires `flutter_executable()` (reads
`AppState::resolved_sdk`). Research clarified the real picture:

- Both `Message::SdkResolved` and `Message::ToolchainPreflightCompleted` are emitted from a
  **single spawned task** in the `RunToolchainPreflight` executor
  (`crates/fdemon-app/src/actions/mod.rs`), in that order (`SdkResolved` first at ~`:841`,
  then `ToolchainPreflightCompleted` at ~`:858`). TEA processes messages one at a time, so under
  normal operation `resolved_sdk` is `Some` before `handle_preflight_completed` runs. **This is
  not a routine bug.**
- The genuine hole: `run_preflight` already calls `find_flutter_sdk` to build the report's
  `FlutterSdk` component status, but **discards** the resolved `FlutterSdk`. The executor then
  makes a **second** `find_flutter_sdk` call to populate `resolved_sdk`. If that second call
  fails (the `Ok(Err(_))` arm only logs at `debug` and skips `SdkResolved`), the report says
  `FlutterSdk: Ok` but `resolved_sdk` stays `None`. A **Bootstrap** handback then silently
  degrades to a bare close to `UiMode::Normal` — no `DiscoverDevices`, `handback_done` left
  `false`. There is also a small TOCTOU window between the two independent `find_flutter_sdk`
  calls.

### Fix

Have `run_preflight` **return the `FlutterSdk` it already resolved** (alongside the
`ToolchainReport`). The executor sets `resolved_sdk` from that single result and no longer makes a
second `find_flutter_sdk` call. This makes `report FlutterSdk Ok ⟺ resolved_sdk Some` an
invariant by construction, removing both the failure hole and the TOCTOU window.

---

## Finding 2 — Tailor the post-install hint for `UserInvoked` opens

### Behaviour

When a `UserInvoked` wizard reaches an all-Ok report, the header currently always shows
`All set — press Esc to return`. For a user who pressed `I` on a **broken** toolchain and then
installed Flutter, that message is misleading: `Esc` returns to `UiMode::Normal` (an empty log
view) with no session and no prompt to start one. The strict Option-1 no-handback decision is
intentional and is **kept**; only the affordance changes.

### Fix

Distinguish "toolchain was healthy throughout this wizard session" from "toolchain was broken at
some point and is now healthy" via a latched `observed_unhealthy` flag on `InstallWizardState`
(set in `apply_report` whenever any component is non-Ok; reset by `opening()`):

| Condition (`UserInvoked` + `all_components_ok()`) | Header hint |
|---|---|
| `!observed_unhealthy` (healthy throughout) | `All set — press Esc to return` |
| `observed_unhealthy` (was broken, now healthy) | `Flutter installed — press <key> to start a session` |

`<key>` must match the actual "start a session" / new-session binding (verify in
`handler/keys.rs` and `docs/KEYBINDINGS.md`).

---

## Finding 3 — Direct unit tests for the new predicates

`all_components_ok()` and `is_bootstrap()` (added by the original fix) are `pub` but only
covered indirectly (render + handler tests). `docs/CODE_STANDARDS.md` requires direct tests for
new public functions. Add co-located `#[cfg(test)]` cases in `install_wizard/state.rs`:

- `all_components_ok()`: no report → false; empty components → false; any `Unknown` → false;
  any non-Ok → false; all Ok → true.
- `is_bootstrap()`: `Bootstrap` → true; `UserInvoked` → false.

---

## Out of Scope

- The strict Option-1 decision itself (a `UserInvoked` wizard never auto-hands-back) is **not**
  revisited — Finding 2 only adjusts the hint text.
- Broader `phase-5-followup/` items unrelated to the handback path.
