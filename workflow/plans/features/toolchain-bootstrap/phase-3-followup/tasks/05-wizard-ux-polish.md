## Task: Wizard UX polish + ergonomics (m2, m3, n4, n5, n6)

**Objective**: Tidy up the install-wizard's JDK-status logic and UX, plus small doc/comment
and re-export ergonomics. These findings are grouped because they share the install-wizard
files and are all low-risk.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1-2 hours

### Background (verified)

- **m2 (gate vs guided-command divergence):** `jdk_status` (`handler/install_wizard/actions.rs:381`)
  returns `Missing` when the report has no `Jdk` component → AndroidTools is gated with
  "see the command below". But `build_steps` (`install_wizard/state.rs:322`) only emits the
  guided command when a `Jdk` component **exists** and is non-Ok. On an empty report the
  status message promises a command that isn't rendered. Extract one shared helper so both
  paths derive "is JDK actionable" identically.
- **m3 (ordering hint):** the PathConfig arm (`handler/install_wizard/actions.rs:173-193`)
  writes `ANDROID_HOME` only from `settings.toolchain.android_sdk_root`, which is `None`
  until AndroidTools completes. Running PathConfig first silently skips the Android env.
  **Soft hint, not a gate** — a user with `ANDROID_HOME` already set in their profile must
  not be blocked. Surface a `status_message` (overwritten on next action), consistent with
  the existing "Install Flutter first" pattern (~actions.rs:191).
- **n4 (doc ref):** `install_wizard/state.rs:130` doc on `begin_step` references "task 09's
  handlers"; the caller is task 07. Simplest fix: name the calling functions instead of the
  task number, and/or add a `// Phase 3, Task 07` banner before `handle_copy_command`
  (~actions.rs:71) which currently sits under an untagged section.
- **n5 (comment):** `step_detail.rs:480-482` has opaque `bottom_area` height arithmetic
  (`content_area.height - (bottom_y - content_area.y)`); add a one-line derivation comment.
- **n6 (re-export gateway):** `install_wizard/mod.rs:21` should re-export
  `ToolchainReport`, `HostPlatform`, `HostShell`, `ComponentKind` so the TUI test code in
  `step_detail.rs` (and `step_list.rs`) imports wizard types via the single
  `fdemon-app::install_wizard` gateway instead of reaching into `fdemon_daemon` directly.

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs` — extract a shared JDK-actionable helper
  (co-locate with `build_steps`; a 6-line `fn` — do not create a new file); fix the n4 doc.
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — make `jdk_status` (or the gate)
  use the shared helper (m2); add the PathConfig ordering `status_message` hint (m3); add the
  task-07 section banner (n4).
- `crates/fdemon-app/src/install_wizard/mod.rs` — extend the `pub use fdemon_daemon::toolchain::{...}`
  re-export list (n6).
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — add the n5 comment; switch
  the `#[cfg(test)]` imports to the `fdemon-app::install_wizard` gateway (n6).
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` — switch any test imports to the
  gateway if present (n6); otherwise no-op.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/update.rs` (routing context, read-only).

### Acceptance Criteria

1. (m2) The AndroidTools gate and the guided-command derivation share one helper; on a
   report with **no** `Jdk` component the gate message and the rendered guided command agree
   (either both present or the message doesn't promise an unrendered command). A test covers
   the no-JDK-entry edge.
2. (m3) Running PathConfig with `android_sdk_root == None` sets a non-blocking
   `status_message` hinting the user to run Android Tools first; PathConfig still executes
   (writes the Flutter PATH). A test asserts the hint appears and the step is not blocked.
3. (n4) No doc/comment references a wrong task number for the wizard handlers.
4. (n5) The `bottom_area` arithmetic has a derivation comment.
5. (n6) `ToolchainReport`, `HostPlatform`, `HostShell`, `ComponentKind` are re-exported from
   `fdemon-app::install_wizard`; the TUI test imports use that path (no direct
   `fdemon_daemon::toolchain` import remains in the touched test modules).
6. `cargo fmt`/`check`/`test`/`clippy -D warnings` pass workspace-wide.

### Notes

- m3 is a **hint, not a gate** — do not add a hard block.
- n6 only adds `pub use` (additive); confirm no name clash in `install_wizard/mod.rs`.
- Keep the shared JDK helper in `state.rs` next to `build_steps` — do not add a new module
  for a single small function.

---

## Completion Summary

**Status:**
**Branch:**

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
